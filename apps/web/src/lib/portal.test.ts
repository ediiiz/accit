import { describe, expect, it } from "vitest";
import type { PortalStatus } from "./api.js";
import { formatUntil, localeFor, screenFor } from "./portal.js";

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
  it("renders Unix seconds as local date and time", () => {
    // 2026-11-13T22:13:20Z
    expect(formatUntil(1_794_608_000, "de", "UTC")).toBe("13.11.2026, 22:13");
  });
});
