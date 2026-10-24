import { expect, test } from "@playwright/test";

// docs/spec.md section 9: the screens a Credential holder sees, reached through Traefik's mTLS.
test("Unlock, Enabled, then Revoke after confirming", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Access your apps." })).toBeVisible();

  await page.getByRole("button", { name: "Enable access" }).click();

  await expect(page.getByRole("heading", { name: "Access is enabled." })).toBeVisible();
  await expect(page.getByText(/^Available until /)).toBeVisible();
  await expect(page.getByRole("link", { name: "Gated" })).toHaveAttribute(
    "href",
    "https://gated.accit.test",
  );
  await expect(page.getByRole("button", { name: "Extend access" })).toBeVisible();

  const revoke = page.getByRole("button", { name: "Disable access for this connection" });
  await revoke.click();
  const dialog = page.getByRole("dialog", { name: "Disable access for this connection?" });
  await dialog.getByRole("button", { name: "Cancel" }).click();

  await expect(dialog).toBeHidden();
  await expect(page.getByRole("heading", { name: "Access is enabled." })).toBeVisible();

  await revoke.click();
  await dialog.getByRole("button", { name: "Disable access" }).click();

  await expect(page.getByRole("heading", { name: "Access your apps." })).toBeVisible();
  await expect(page.getByRole("button", { name: "Enable access" })).toBeVisible();
});
