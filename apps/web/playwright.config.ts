import { defineConfig } from "@playwright/test";

const port = 4173;

// E2E_BASE_URL runs the suite against a running stack instead of a local preview.
const externalBaseUrl = process.env.E2E_BASE_URL;

export default defineConfig({
  testDir: "./tests/e2e",
  testMatch: "**/*.e2e.ts",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  use: {
    baseURL: externalBaseUrl ?? `http://127.0.0.1:${port}`,
    trace: "on-first-retry",
  },
  projects: [{ name: "chromium" }],
  webServer: externalBaseUrl
    ? undefined
    : {
        command: `bun run --cwd ../.. web:build --output-logs=errors-only && bun run preview -- --host 127.0.0.1 --port ${port}`,
        url: `http://127.0.0.1:${port}`,
        reuseExistingServer: !process.env.CI,
        timeout: 120_000,
      },
});
