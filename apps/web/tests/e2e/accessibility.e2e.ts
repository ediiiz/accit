import { expect, test, type Page, type Route } from "@playwright/test";
import { expectAccessible } from "./axe.js";

test.describe("@a11y", () => {
  const status = {
    state: "none",
    expiry: null,
    server_time: 1_800_000_000,
    address: "10.89.10.13",
    family: "ipv4",
    apps: [{ name: "Gated", url: "https://gated.accit.test" }],
    title: "Harness",
    default_language: "en",
    csrf_token: "token",
  };

  const refuse = (route: Route, httpStatus: number, headers: Record<string, string> = {}) =>
    route.fulfill({ status: httpStatus, headers, body: "refused" });

  /** A Portal state the stack cannot be put in on demand: the page gets the API answer it would get there. */
  async function statusIs(page: Page, state: string, expiry: number | null) {
    await page.route("**/api/status", (route) =>
      route.fulfill({ json: { ...status, state, expiry } }),
    );
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
    await page.addInitScript(() => localStorage.setItem("accit.last-unlocked", "10.89.10.99"));
    await page.goto("/");

    await expect(page.getByRole("heading", { name: "Your connection changed." })).toBeVisible();
    await expectAccessible(page);
  });

  test("Expired", async ({ page }) => {
    await statusIs(page, "expired", 1_799_990_000);
    await page.goto("/");

    await expect(page.getByRole("heading", { name: "Your access has expired." })).toBeVisible();
    await expectAccessible(page);
  });

  test("Home network", async ({ page }) => {
    await statusIs(page, "trusted", null);
    await page.goto("/");

    await expect(page.getByRole("heading", { name: "You're on your home network." })).toBeVisible();
    await expectAccessible(page);
  });

  test("Service unavailable", async ({ page }) => {
    await page.route("**/api/unlock", (route) => refuse(route, 503));
    await page.goto("/");
    await page.getByRole("button", { name: "Enable access" }).click();

    await expect(page.getByRole("heading", { name: "We couldn't enable access." })).toBeVisible();
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
});
