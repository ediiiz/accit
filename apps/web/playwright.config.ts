import { fileURLToPath } from "node:url";
import { defineConfig } from "@playwright/test";

const baseURL = process.env.E2E_BASE_URL;

if (!baseURL) {
  throw new Error(
    "E2E_BASE_URL is not set: the suite runs against the Compose harness (scripts/e2e-web)",
  );
}

// The harness's test client certificate (`harness/prepare`); the suite is the Credential holder.
const clientDir =
  process.env.E2E_CLIENT_DIR ?? fileURLToPath(new URL("../../harness/run/client", import.meta.url));

export default defineConfig({
  testDir: "./tests/e2e",
  testMatch: "**/*.e2e.ts",
  // One accit, one source Address, a Grant that outlives a test: files and tests run in order.
  fullyParallel: false,
  workers: 1,
  forbidOnly: !!process.env.CI,
  use: {
    baseURL,
    clientCertificates: [
      {
        origin: new URL(baseURL).origin,
        certPath: `${clientDir}/client.pem`,
        keyPath: `${clientDir}/client.key`,
      },
    ],
    // Test PKI, and Playwright's client-certificate proxy re-signs the page's TLS.
    ignoreHTTPSErrors: true,
    locale: "en-US",
    reducedMotion: "reduce",
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium" }],
});
