import AxeBuilder from "@axe-core/playwright";
import { expect, type Page } from "@playwright/test";

const WCAG_22_AA = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"];

/** The page as it stands must have no WCAG 2.2 AA violation, in light and in dark. */
export async function expectAccessible(page: Page): Promise<void> {
  for (const colorScheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme });

    const { violations } = await new AxeBuilder({ page }).withTags(WCAG_22_AA).analyze();

    expect(violations, colorScheme).toEqual([]);
  }
}
