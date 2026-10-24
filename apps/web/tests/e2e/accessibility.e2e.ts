import { expect, test, type Page, type Route } from "@playwright/test";
import type { PortalStatus } from "../../src/lib/api.js";
import { expectAccessible } from "./axe.js";

test.describe("@a11y", () => {
  const refuse = (route: Route, httpStatus: number, headers: Record<string, string> = {}) =>
    route.fulfill({ status: httpStatus, headers, body: "refused" });

  // SAFETY: the stack's own status answer (docs/spec.md section 8), the shape `PortalStatus` mirrors.
  const realStatus = async (route: Route): Promise<PortalStatus> =>
    (await route.fetch()).json() as Promise<PortalStatus>;

  /** The real status with only `state` and `expiry` replaced; `expiresIn` is seconds from the server's clock. */
  async function mockStatus(page: Page, state: PortalStatus["state"], expiresIn: number | null) {
    await page.route("**/api/status", async (route) => {
      const real = await realStatus(route);

      await route.fulfill({
        json: { ...real, state, expiry: expiresIn === null ? null : real.server_time + expiresIn },
      });
    });
  }

  test("Not enabled, Enabled, Troubleshooting and the Revoke confirmation", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByRole("heading", { name: "Access your apps." })).toBeVisible();
    await expectAccessible(page);

    await page.getByRole("button", { name: "Enable access" }).click();
    await expect(page.getByRole("heading", { name: "Access is enabled." })).toBeVisible();
    await expectAccessible(page);

    await page.getByText("Troubleshooting").click();
    await expect(page.getByText("Address", { exact: true })).toBeVisible();
    await expectAccessible(page);

    await page.getByRole("button", { name: "Disable access for this connection" }).click();
    await expect(page.getByRole("dialog")).toBeVisible();
    await expectAccessible(page);

    await page.getByRole("dialog").getByRole("button", { name: "Disable access" }).click();
    await expect(page.getByRole("heading", { name: "Access your apps." })).toBeVisible();
  });

  test("Connection changed", async ({ page }) => {
    // The Portal remembers the Address of the last Unlock; answer one from another Address, then ask again from this one.
    let elsewhere: PortalStatus | undefined;

    await page.route("**/api/status", async (route) => {
      elsewhere = { ...(await realStatus(route)), address: "10.89.10.99" };
      await route.fulfill({ json: elsewhere });
    });
    await page.route("**/api/unlock", (route) =>
      route.fulfill({
        json: { ...elsewhere, state: "active", expiry: (elsewhere?.server_time ?? 0) + 3600 },
      }),
    );
    await page.goto("/");
    await page.getByRole("button", { name: "Enable access" }).click();
    await expect(page.getByRole("heading", { name: "Access is enabled." })).toBeVisible();

    await page.unroute("**/api/status");
    await page.unroute("**/api/unlock");
    await page.reload();

    await expect(page.getByRole("heading", { name: "Your connection changed." })).toBeVisible();
    await expectAccessible(page);
  });

  test("Expired", async ({ page }) => {
    await mockStatus(page, "expired", -10_000);
    await page.goto("/");

    await expect(page.getByRole("heading", { name: "Your access has expired." })).toBeVisible();
    await expectAccessible(page);
  });

  test("Home network", async ({ page }) => {
    await mockStatus(page, "trusted", null);
    await page.goto("/");

    await expect(page.getByRole("heading", { name: "You're on your home network." })).toBeVisible();
    await expectAccessible(page);
  });

  test("Submitting", async ({ page }) => {
    const held = Promise.withResolvers<Route>();
    await page.route("**/api/unlock", (route) => held.resolve(route));
    await page.goto("/");
    await page.getByRole("button", { name: "Enable access" }).click();

    await expect(page.getByRole("heading", { name: "Enabling access…" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Enable access" })).toBeDisabled();
    await expectAccessible(page);

    await refuse(await held.promise, 503);
  });

  test("Service unavailable", async ({ page }) => {
    await page.route("**/api/unlock", (route) => refuse(route, 503));
    await page.goto("/");
    await page.getByRole("button", { name: "Enable access" }).click();

    await expect(page.getByRole("heading", { name: "We couldn't enable access." })).toBeVisible();
    await expectAccessible(page);
  });

  test("Service unavailable on the first status read", async ({ page }) => {
    await page.route("**/api/status", (route) => refuse(route, 503));
    await page.goto("/");

    await expect(page.getByRole("heading", { name: "We couldn't enable access." })).toBeVisible();
    await expectAccessible(page);
  });

  test("Service unavailable after a failed Revoke", async ({ page }) => {
    await mockStatus(page, "active", 3600);
    await page.route("**/api/revoke", (route) => refuse(route, 503));
    await page.goto("/");
    await page.getByRole("button", { name: "Disable access for this connection" }).click();
    await page.getByRole("dialog").getByRole("button", { name: "Disable access" }).click();

    await expect(page.getByRole("heading", { name: "We couldn't disable access." })).toBeVisible();
    await expect(page.getByRole("dialog")).toBeHidden();
    await expectAccessible(page);
  });

  test("Rate limited", async ({ page }) => {
    await page.route("**/api/unlock", (route) => refuse(route, 429, { "Retry-After": "30" }));
    await page.goto("/");
    await page.getByRole("button", { name: "Enable access" }).click();

    await expect(
      page.getByRole("heading", { name: "Please wait a moment before trying again." }),
    ).toBeVisible();
    await expectAccessible(page);
  });

  test.describe("German browser", () => {
    test.use({ locale: "de-DE" });

    test("Not enabled", async ({ page }) => {
      await page.goto("/");

      await expect(page.locator("html")).toHaveAttribute("lang", "de");
      await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
      await expectAccessible(page);
    });
  });
});
