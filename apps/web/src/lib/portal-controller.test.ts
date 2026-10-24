import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError, type PortalStatus } from "./api.js";
import { ADDRESS, deferred, fakeApi, memoryStore, status } from "./fixtures.js";
import { PortalController } from "./portal-controller.svelte.js";

const active = status({ state: "active", expiry: 1_800_604_800, csrf_token: "fresh" });

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("loading", () => {
  it("starts blank, then shows the status and never mutates", async () => {
    const { api, calls } = fakeApi({ status: [status()] });
    const portal = new PortalController(api, memoryStore());

    expect(portal.screen).toBe("loading");
    await portal.load();

    expect(portal.screen).toBe("not_enabled");
    expect(calls).toEqual(["status"]);
  });

  it("a failed first load is unavailable, and Try again loads again", async () => {
    const { api } = fakeApi({ status: [new ApiError("unavailable"), status()] });
    const portal = new PortalController(api, memoryStore());

    await portal.load();
    expect(portal.screen).toBe("unavailable");

    await portal.retry();
    expect(portal.screen).toBe("not_enabled");
  });

  it("shows a changed connection from the last unlocked Address", async () => {
    const { api } = fakeApi({ status: [status({ address: "198.51.100.7" })] });
    const portal = new PortalController(api, memoryStore(ADDRESS));

    await portal.load();

    expect(portal.screen).toBe("changed");
  });

  it("a failed refresh keeps the screen already shown", async () => {
    const { api } = fakeApi({ status: [active, new ApiError("unavailable")] });
    const portal = new PortalController(api, memoryStore());

    await portal.load();
    await portal.load();

    expect(portal.screen).toBe("enabled");
  });

  it("a refresh replaces the status", async () => {
    const { api } = fakeApi({ status: [active, status({ state: "expired" })] });
    const portal = new PortalController(api, memoryStore());

    await portal.load();
    await portal.load();

    expect(portal.screen).toBe("expired");
  });
});

describe("unlock", () => {
  it("refreshes the status, unlocks with the fresh token, and remembers the Address", async () => {
    const store = memoryStore();
    const unlocked = deferred<PortalStatus>();

    const { api, calls } = fakeApi({
      status: [status(), status({ csrf_token: "fresh" })],
      unlock: [unlocked.promise],
    });

    const portal = new PortalController(api, store);

    await portal.load();
    const pending = portal.unlock();
    await vi.advanceTimersByTimeAsync(0);

    expect(portal.screen).toBe("submitting");

    unlocked.resolve(active);
    await pending;

    expect(calls).toEqual(["status", "status", "unlock:fresh"]);
    expect(portal.screen).toBe("enabled");
    expect(store.value).toBe(ADDRESS);
  });

  it("does nothing when the fresh status says the connection is on the home network", async () => {
    const store = memoryStore();
    const { api, calls } = fakeApi({ status: [status(), status({ state: "trusted" })] });
    const portal = new PortalController(api, store);

    await portal.load();
    await portal.unlock();

    expect(calls).toEqual(["status", "status"]);
    expect(portal.screen).toBe("home");
    expect(store.value).toBeNull();
  });

  it("ignores a second tap while one is in flight", async () => {
    const unlocked = deferred<PortalStatus>();

    const { api, calls } = fakeApi({
      status: [status(), status()],
      unlock: [unlocked.promise],
    });

    const portal = new PortalController(api, memoryStore());

    await portal.load();
    const first = portal.unlock();
    await portal.unlock();
    unlocked.resolve(active);
    await first;

    expect(calls.filter((call) => call.startsWith("unlock"))).toHaveLength(1);
  });

  it("a refusal is unavailable and nothing is remembered; Try again unlocks again", async () => {
    const store = memoryStore();

    const { api, calls } = fakeApi({
      status: [status(), status(), status()],
      unlock: [new ApiError("unavailable"), active],
    });

    const portal = new PortalController(api, store);

    await portal.load();
    await portal.unlock();

    expect(portal.screen).toBe("unavailable");
    expect(store.value).toBeNull();

    await portal.retry();

    expect(portal.screen).toBe("enabled");
    expect(calls.filter((call) => call.startsWith("unlock"))).toHaveLength(2);
  });

  it("a status failure before the unlock is unavailable", async () => {
    const { api, calls } = fakeApi({ status: [status(), new ApiError("unavailable")] });
    const portal = new PortalController(api, memoryStore());

    await portal.load();
    await portal.unlock();

    expect(portal.screen).toBe("unavailable");
    expect(calls).toEqual(["status", "status"]);
  });

  it("a focus refresh does not clear a failed unlock", async () => {
    const { api } = fakeApi({
      status: [status(), status(), status()],
      unlock: [new ApiError("unavailable")],
    });

    const portal = new PortalController(api, memoryStore());

    await portal.load();
    await portal.unlock();
    await portal.load();

    expect(portal.screen).toBe("unavailable");
  });
});

describe("rate limit", () => {
  it("counts down, then retries the same action", async () => {
    const { api, calls } = fakeApi({
      status: [status(), status(), status()],
      unlock: [new ApiError("rate_limited", 3), active],
    });

    const portal = new PortalController(api, memoryStore());

    await portal.load();
    await portal.unlock();

    expect(portal.screen).toBe("rate_limited");
    expect(portal.secondsLeft).toBe(3);

    await portal.retry();
    expect(calls.filter((call) => call.startsWith("unlock"))).toHaveLength(1);

    await vi.advanceTimersByTimeAsync(2000);
    expect(portal.secondsLeft).toBe(1);

    await vi.advanceTimersByTimeAsync(1000);
    expect(portal.secondsLeft).toBe(0);

    await portal.retry();
    expect(portal.screen).toBe("enabled");
  });
});

describe("revoke", () => {
  it("refreshes the status, revokes with the fresh token and shows the result", async () => {
    const { api, calls } = fakeApi({
      status: [active, active],
      revoke: [status({ csrf_token: "after" })],
    });

    const portal = new PortalController(api, memoryStore(ADDRESS));

    await portal.load();
    await portal.revoke();

    expect(calls).toEqual(["status", "status", "revoke:fresh"]);
    expect(portal.screen).toBe("not_enabled");
    expect(portal.revoking).toBe(false);
  });

  it("does nothing when the Grant is already gone", async () => {
    const { api, calls } = fakeApi({ status: [active, status({ state: "expired" })] });
    const portal = new PortalController(api, memoryStore());

    await portal.load();
    await portal.revoke();

    expect(calls).toEqual(["status", "status"]);
    expect(portal.screen).toBe("expired");
  });

  it("a refusal is unavailable for revoking; Try again revokes again", async () => {
    const { api } = fakeApi({
      status: [active, active, active],
      revoke: [new ApiError("unavailable"), status()],
    });

    const portal = new PortalController(api, memoryStore());

    await portal.load();
    await portal.revoke();

    expect(portal.screen).toBe("unavailable");
    expect(portal.failedAction).toBe("revoke");

    await portal.retry();

    expect(portal.screen).toBe("not_enabled");
  });
});
