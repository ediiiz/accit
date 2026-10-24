import type { PortalStatus } from "./api.js";

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
    dateStyle: "medium",
    timeStyle: "short",
    timeZone,
  }).format(expiry * 1000);
}
