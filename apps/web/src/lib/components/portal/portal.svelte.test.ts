import { afterEach, describe, expect, it } from "vitest";
import { render } from "vitest-browser-svelte";
import { page, userEvent } from "vitest/browser";
import { ApiError, type PortalStatus } from "#lib/api.js";
import { ADDRESS, deferred, fakeApi, memoryStore, status } from "#lib/fixtures.js";
import { PortalController } from "#lib/portal-controller.svelte.js";
import Portal from "./portal.svelte";

const active = status({
  state: "active",
  expiry: 1_794_608_000,
  apps: [
    { name: "Jellyfin", url: "https://jellyfin.example.com" },
    { name: "Plain", url: "http://plain.example.com" },
  ],
});

type Answers = Parameters<typeof fakeApi>[0];

async function mount(answers: Answers, options: { last?: string; languages?: string[] } = {}) {
  const { api, calls } = fakeApi(answers);
  const portal = new PortalController(api, memoryStore(options.last ?? null));

  await render(Portal, { portal, languages: options.languages ?? ["en-US"] });

  return { calls, portal };
}

const mutations = (calls: string[]) => calls.filter((call) => call !== "status");

const alert = (heading: string) => page.getByRole("alert").getByRole("heading", { name: heading });

const button = (name: string | RegExp) => page.getByRole("button", { name });

afterEach(() => {
  document.documentElement.removeAttribute("lang");
});

describe("not enabled", () => {
  it("shows the copy, one primary action, the Wi-Fi tip, and mutates nothing on load", async () => {
    const { calls } = await mount({ status: [status()] });

    await expect.element(page.getByRole("heading", { name: "Access your apps." })).toBeVisible();
    await expect
      .element(page.getByText("Enable access for your current connection."))
      .toBeVisible();
    await expect.element(button("Enable access")).toBeEnabled();
    await expect
      .element(page.getByText("To enable another device, connect this phone to the same Wi-Fi."))
      .toBeVisible();
    expect(mutations(calls)).toEqual([]);
  });

  it("announces the status in a polite live region", async () => {
    await mount({ status: [status()] });

    await expect
      .element(page.getByRole("status").getByRole("heading", { name: "Access your apps." }))
      .toBeVisible();
  });
});

describe("submitting and enabled", () => {
  it("one tap shows progress, then the expiry, app links and Extend access", async () => {
    const unlocked = deferred<PortalStatus>();

    const { calls } = await mount({
      status: [status(), status({ csrf_token: "fresh" })],
      unlock: [unlocked.promise],
    });

    await button("Enable access").click();

    await expect.element(page.getByText("Enabling access…")).toBeVisible();
    await expect.element(button("Enable access")).toBeDisabled();
    await expect.element(button("Enable access")).toHaveAttribute("aria-busy", "true");

    unlocked.resolve(active);

    await expect.element(page.getByText("Access is enabled.")).toBeVisible();
    await expect.element(page.getByText(/^Available until .*2026/)).toBeVisible();
    await expect.element(button("Extend access")).toBeEnabled();
    await expect
      .element(page.getByRole("link", { name: "Jellyfin" }))
      .toHaveAttribute("href", "https://jellyfin.example.com");
    expect(mutations(calls)).toEqual(["unlock:fresh"]);
  });

  it("only https app links are offered", async () => {
    await mount({ status: [active] });

    await expect.element(page.getByRole("link", { name: "Jellyfin" })).toBeVisible();
    await expect.element(page.getByRole("link", { name: "Plain" })).not.toBeInTheDocument();
  });

  it("no app links before the Unlock", async () => {
    await mount({ status: [status({ apps: active.apps })] });

    await expect.element(button("Enable access")).toBeVisible();
    await expect.element(page.getByRole("link", { name: "Jellyfin" })).not.toBeInTheDocument();
  });
});

describe("other states", () => {
  it("expired", async () => {
    await mount({ status: [status({ state: "expired", expiry: 1_700_000_000 })] });

    await expect.element(page.getByText("Your access has expired.")).toBeVisible();
    await expect.element(button("Enable access")).toBeEnabled();
  });

  it("connection changed", async () => {
    await mount({ status: [status({ address: "198.51.100.7" })] }, { last: ADDRESS });

    await expect.element(page.getByText("Your connection changed.")).toBeVisible();
    await expect.element(page.getByText("Enable access again.")).toBeVisible();
    await expect.element(button("Enable access")).toBeEnabled();
  });

  it("home network has no action but app links", async () => {
    await mount({ status: [status({ state: "trusted", apps: active.apps })] });

    await expect.element(page.getByText("You're on your home network.")).toBeVisible();
    await expect.element(page.getByText("Your apps work without enabling access.")).toBeVisible();
    await expect.element(page.getByRole("link", { name: "Jellyfin" })).toBeVisible();
    await expect
      .element(page.getByRole("button", { name: /access|again/i }))
      .not.toBeInTheDocument();
  });

  it("service unavailable is announced assertively and can be retried", async () => {
    await mount({
      status: [status(), status(), status()],
      unlock: [new ApiError("unavailable"), active],
    });

    await button("Enable access").click();

    await expect.element(alert("We couldn't enable access.")).toBeVisible();
    await expect.element(page.getByText("Please try again.")).toBeVisible();

    await button("Try again").click();

    await expect.element(page.getByText("Access is enabled.")).toBeVisible();
  });

  it("a failed first load is unavailable too", async () => {
    await mount({ status: [new ApiError("unavailable"), status()] });

    await expect.element(alert("We couldn't enable access.")).toBeVisible();

    await button("Try again").click();

    await expect.element(page.getByText("Access your apps.")).toBeVisible();
  });

  it("rate limited waits out the countdown", async () => {
    await mount({
      status: [status(), status()],
      unlock: [new ApiError("rate_limited", 1), active],
    });

    await button("Enable access").click();

    await expect.element(alert("Please wait a moment before trying again.")).toBeVisible();
    await expect.element(button("Try again in 1 s")).toBeDisabled();
    await expect.element(button("Try again"), { timeout: 3000 }).toBeEnabled();

    await button("Try again").click();

    await expect.element(page.getByText("Access is enabled.")).toBeVisible();
  });
});

describe("revoke", () => {
  it("asks first, and Cancel changes nothing", async () => {
    const { calls } = await mount({ status: [active] });

    await button("Disable access for this connection").click();
    await expect
      .element(page.getByRole("dialog", { name: "Disable access for this connection?" }))
      .toBeVisible();

    await button("Cancel").click();

    await expect.element(page.getByRole("dialog")).not.toBeInTheDocument();
    expect(mutations(calls)).toEqual([]);
  });

  it("confirming revokes and shows the resulting state", async () => {
    const { calls } = await mount({
      status: [active, { ...active, csrf_token: "fresh" }],
      revoke: [status()],
    });

    await button("Disable access for this connection").click();
    await page.getByRole("dialog").getByRole("button", { name: "Disable access" }).click();

    await expect.element(page.getByText("Access your apps.")).toBeVisible();
    await expect.element(page.getByRole("dialog")).not.toBeInTheDocument();
    expect(mutations(calls)).toEqual(["revoke:fresh"]);
  });

  it("a failed revoke says so", async () => {
    await mount({ status: [active, active], revoke: [new ApiError("unavailable")] });

    await button("Disable access for this connection").click();
    await page.getByRole("dialog").getByRole("button", { name: "Disable access" }).click();

    await expect.element(alert("We couldn't disable access.")).toBeVisible();
  });

  it("is not offered when there is no Grant", async () => {
    await mount({ status: [status()] });

    await expect.element(button("Enable access")).toBeVisible();
    await expect.element(button("Disable access for this connection")).not.toBeInTheDocument();
  });
});

describe("troubleshooting", () => {
  it("discloses the Address and family", async () => {
    await mount({ status: [status({ address: "2001:db8::1", family: "ipv6" })] });

    await page.getByText("Troubleshooting").click();

    await expect.element(page.getByText("2001:db8::1")).toBeVisible();
    await expect.element(page.getByText("IPv6")).toBeVisible();
  });
});

describe("freshness", () => {
  it("refreshes the status when the window gains focus", async () => {
    const { calls } = await mount({ status: [status(), status({ state: "expired" })] });

    await expect.element(page.getByText("Access your apps.")).toBeVisible();
    window.dispatchEvent(new Event("focus"));

    await expect.element(page.getByText("Your access has expired.")).toBeVisible();
    expect(calls.length).toBeGreaterThanOrEqual(2);
  });
});

describe("language", () => {
  it("the browser language wins over the admin default", async () => {
    await mount({ status: [status({ default_language: "en" })] }, { languages: ["de-DE"] });

    await expect
      .element(page.getByRole("heading", { name: "Zugriff auf deine Apps." }))
      .toBeVisible();
    await expect.element(button("Zugriff aktivieren")).toBeEnabled();
    expect(document.documentElement.lang).toBe("de");
  });

  it("an unsupported browser language uses the admin default", async () => {
    await mount({ status: [status({ default_language: "de" })] }, { languages: ["fr-FR"] });

    await expect
      .element(page.getByRole("heading", { name: "Zugriff auf deine Apps." }))
      .toBeVisible();
  });

  it("German copy for the dated state", async () => {
    await mount({ status: [active] }, { languages: ["de"] });

    await expect.element(page.getByText("Zugriff ist aktiviert.")).toBeVisible();
    await expect.element(page.getByText(/^Gültig bis .*2026/)).toBeVisible();
    await expect.element(button("Zugriff verlängern")).toBeVisible();
  });
});

describe("keyboard", () => {
  it("the primary action is reachable and works with the keyboard", async () => {
    const { calls } = await mount({ status: [status(), status()], unlock: [active] });

    await expect.element(button("Enable access")).toBeVisible();
    await userEvent.tab();
    await expect.element(button("Enable access")).toHaveFocus();
    await userEvent.keyboard("{Enter}");

    await expect.element(page.getByText("Access is enabled.")).toBeVisible();
    expect(mutations(calls)).toEqual(["unlock:token"]);
  });
});
