import type { PortalApp, PortalStatus } from "./api.js";
import { m } from "./paraglide/messages.js";
import type { Action } from "./portal-controller.svelte.js";

export type Screen =
  | "loading"
  | "not_enabled"
  | "submitting"
  | "enabled"
  | "expired"
  | "changed"
  | "home"
  | "unavailable"
  | "rate_limited";

/** The screen a status shows when nothing is in flight. */
export function screenFor(status: PortalStatus, lastUnlocked: string | null): Screen {
  switch (status.state) {
    case "trusted":
      return "home";
    case "active":
      return "enabled";
    case "expired":
      return "expired";
    case "none":
      return lastUnlocked !== null && lastUnlocked !== status.address ? "changed" : "not_enabled";
  }
}

const LOCALES = ["en", "de"] as const;

export type PortalLocale = (typeof LOCALES)[number];

function supported(tag: string | undefined): PortalLocale | undefined {
  const language = tag?.toLowerCase().split("-")[0];

  return LOCALES.find((locale) => locale === language);
}

/** Browser language first, then the admin default, then English. */
export function localeFor(
  languages: readonly string[],
  adminDefault: string | undefined,
): PortalLocale {
  for (const tag of languages) {
    const locale = supported(tag);

    if (locale) {
      return locale;
    }
  }

  return supported(adminDefault) ?? "en";
}

export function formatUntil(expiry: number, locale: string, timeZone?: string): string {
  return new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
    timeZoneName: "short",
    timeZone,
  }).format(expiry * 1000);
}

export function httpsApps(apps: readonly PortalApp[]): PortalApp[] {
  return apps.filter((app) => app.url.startsWith("https://"));
}

export interface PrimaryAction {
  command: "unlock" | "retry";
  label: string;
  disabled: boolean;
}

export function primaryAction(screen: Screen, secondsLeft: number): PrimaryAction | null {
  switch (screen) {
    case "not_enabled":
    case "expired":
    case "changed":
      return { command: "unlock", label: m.enable_access(), disabled: false };
    case "submitting":
      return { command: "unlock", label: m.enable_access(), disabled: true };
    case "enabled":
      return { command: "unlock", label: m.extend_access(), disabled: false };
    case "unavailable":
      return { command: "retry", label: m.try_again(), disabled: false };
    case "rate_limited":
      return {
        command: "retry",
        label: secondsLeft > 0 ? m.try_again_in({ seconds: secondsLeft }) : m.try_again(),
        disabled: secondsLeft > 0,
      };
    case "loading":
    case "home":
      return null;
  }
}

export type Icon = "check" | "clock" | "home" | "spinner" | "alert";

export interface Copy {
  icon: Icon;
  heading: string;
  body?: string;
}

export interface CopyView {
  /** The formatted expiry; `null` without a Grant. */
  until: string | null;
  failedAction: Action | null;
}

export function copyFor(screen: Screen, view: CopyView): Copy | null {
  switch (screen) {
    case "loading":
      return null;
    case "not_enabled":
      return { icon: "clock", heading: m.not_enabled_heading(), body: m.not_enabled_body() };
    case "submitting":
      return { icon: "spinner", heading: m.submitting_heading() };
    case "enabled":
      return {
        icon: "check",
        heading: m.enabled_heading(),
        body: view.until === null ? undefined : m.enabled_body({ until: view.until }),
      };
    case "expired":
      return { icon: "clock", heading: m.expired_heading() };
    case "changed":
      return { icon: "alert", heading: m.changed_heading(), body: m.changed_body() };
    case "home":
      return { icon: "home", heading: m.home_heading(), body: m.home_body() };
    case "unavailable":
      return {
        icon: "alert",
        heading:
          view.failedAction === "revoke" ? m.unavailable_revoke_heading() : m.unavailable_heading(),
        body: m.unavailable_body(),
      };
    case "rate_limited":
      return { icon: "clock", heading: m.rate_limited_heading() };
  }
}
