import { describe, expect, it } from "vitest";
import type { PortalStatus } from "./api.js";
import { copyFor, formatUntil, httpsApps, localeFor, primaryAction, screenFor } from "./portal.js";

const status = (patch: Partial<PortalStatus>): PortalStatus => ({
  state: "none",
  expiry: null,
  server_time: 1_800_000_000,
  address: "93.184.216.34",
  family: "ipv4",
  apps: [],
  title: "Access",
  default_language: "en",
  csrf_token: "t",
  ...patch,
});

describe("screenFor", () => {
  it.each([
    ["none", "not_enabled"],
    ["active", "enabled"],
    ["expired", "expired"],
    ["trusted", "home"],
  ] as const)("%s without history is %s", (state, screen) => {
    expect(screenFor(status({ state }), null)).toBe(screen);
  });

  it("no Grant from another Address than the last Unlock is a changed connection", () => {
    expect(screenFor(status({ state: "none" }), "198.51.100.7")).toBe("changed");
  });

  it("no Grant from the same Address as the last Unlock is not enabled", () => {
    expect(screenFor(status({ state: "none" }), "93.184.216.34")).toBe("not_enabled");
  });

  it("an active Grant, an expired one and the home network ignore the history", () => {
    expect(screenFor(status({ state: "active" }), "198.51.100.7")).toBe("enabled");
    expect(screenFor(status({ state: "expired" }), "198.51.100.7")).toBe("expired");
    expect(screenFor(status({ state: "trusted" }), "198.51.100.7")).toBe("home");
  });
});

describe("localeFor", () => {
  it("takes the first supported browser language", () => {
    expect(localeFor(["fr-FR", "de-AT", "en"], "en")).toBe("de");
    expect(localeFor(["en-GB"], "de")).toBe("en");
  });

  it("falls back to the admin default, then English", () => {
    expect(localeFor(["fr", "ja"], "de")).toBe("de");
    expect(localeFor([], "de")).toBe("de");
    expect(localeFor(["fr"], "fr")).toBe("en");
    expect(localeFor(["fr"], undefined)).toBe("en");
  });
});

describe("formatUntil", () => {
  it("renders Unix seconds as date, time and timezone", () => {
    // 2026-11-13T22:13:20Z
    expect(formatUntil(1_794_608_000, "de", "UTC")).toBe("13. Nov. 2026, 22:13 UTC");
    expect(formatUntil(1_794_608_000, "en", "Europe/Berlin")).toBe("Nov 13, 2026, 11:13 PM GMT+1");
  });
});

describe("httpsApps", () => {
  it("keeps only https links", () => {
    const apps = [
      { name: "Jellyfin", url: "https://jellyfin.example.com" },
      { name: "Plain", url: "http://plain.example.com" },
      { name: "Script", url: "javascript:alert(1)" },
    ];

    expect(httpsApps(apps)).toEqual([apps[0]]);
  });
});

describe("primaryAction", () => {
  it.each([
    ["not_enabled", "unlock", "Enable access"],
    ["expired", "unlock", "Enable access"],
    ["changed", "unlock", "Enable access"],
    ["enabled", "unlock", "Extend access"],
    ["unavailable", "retry", "Try again"],
  ] as const)("%s offers %s as %s", (screen, command, label) => {
    expect(primaryAction(screen, 0)).toEqual({ command, label, disabled: false });
  });

  it("is disabled while submitting", () => {
    expect(primaryAction("submitting", 0)).toEqual({
      command: "unlock",
      label: "Enable access",
      disabled: true,
    });
  });

  it("a rate limit counts down, then offers Try again", () => {
    expect(primaryAction("rate_limited", 12)).toEqual({
      command: "retry",
      label: "Try again in 12 s",
      disabled: true,
    });
    expect(primaryAction("rate_limited", 0)).toEqual({
      command: "retry",
      label: "Try again",
      disabled: false,
    });
  });

  it("the home network and loading offer nothing", () => {
    expect(primaryAction("home", 0)).toBeNull();
    expect(primaryAction("loading", 0)).toBeNull();
  });
});

describe("copyFor", () => {
  const view = { until: "tomorrow", failedAction: null } as const;

  it("loading has no copy", () => {
    expect(copyFor("loading", view)).toBeNull();
  });

  it("enabled names the expiry, and says nothing without one", () => {
    expect(copyFor("enabled", view)).toMatchObject({
      heading: "Access is enabled.",
      body: "Available until tomorrow.",
    });
    expect(copyFor("enabled", { ...view, until: null })?.body).toBeUndefined();
  });

  it("unavailable names the action that failed", () => {
    expect(copyFor("unavailable", view)?.heading).toBe("We couldn't enable access.");
    expect(copyFor("unavailable", { ...view, failedAction: "revoke" })).toMatchObject({
      heading: "We couldn't disable access.",
      body: "Please try again.",
    });
  });
});
