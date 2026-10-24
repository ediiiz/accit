import { resolve } from "node:path";
import { paraglideVitePlugin } from "@inlang/paraglide-js";
import adapter from "@sveltejs/adapter-static";
import { sveltekit } from "@sveltejs/kit/vite";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import { playwright } from "@vitest/browser-playwright";
import { readWebToolingEnv } from "@accit/env/server";
import { loadEnv } from "vite";
// `vitest/config`'s defineConfig is Vite's, widened to type the `test` block.
import { defineConfig } from "vitest/config";

export default defineConfig(({ mode }) => {
  // `.env` files (and the process env) for this mode, validated by packages/env.
  const env = readWebToolingEnv({ ...loadEnv(mode, process.cwd(), ""), ...process.env });

  // In dev and preview Vite forwards `/api/*` to the Portal listener; in
  // production the binary serves this app itself, on the same origin.
  // The server wants the client Address in `X-Forwarded-For` (`xfwd`) and, for mutations, the exact
  // `public_origin` of dev/accit.yaml as `Origin`, which a browser on http://localhost:<port> never sends.
  const apiProxy = {
    "/api": { target: env.BACKEND_URL, xfwd: true, headers: { origin: "https://localhost" } },
  };

  const kit = {
    preprocess: vitePreprocess(),
    // One prerendered shell (`prerender`, `ssr = false` in the root layout),
    // embedded and served by the binary (crates/server/src/static_site.rs).
    adapter: adapter({ precompress: true }),
  };

  return {
    // Expose PUBLIC_ vars on import.meta.env so packages/env/web can validate and
    // inline them at build time.
    envPrefix: ["PUBLIC_", "VITE_"],
    server: { proxy: apiProxy },
    preview: { proxy: apiProxy },
    plugins: [
      tailwindcss(),
      sveltekit(kit),
      paraglideVitePlugin({
        project: resolve("./project.inlang"),
        outdir: resolve("./src/lib/paraglide"),
        strategy: ["preferredLanguage", "baseLocale"],
        emitTsDeclarations: true,
      }),
    ],
    // Two Vitest projects:
    // - unit: plain `*.test.ts` in Node. Pure logic, milliseconds.
    // - component: `*.svelte.test.ts` in real headless Chromium (Playwright provider).
    // Playwright's `tests/e2e/**/*.e2e.ts` never matches either glob.
    test: {
      // The Portal grows tests slice by slice; an empty project is not a failure.
      passWithNoTests: true,
      projects: [
        {
          extends: true,
          test: {
            name: "unit",
            include: ["src/**/*.test.ts"],
            exclude: ["src/**/*.svelte.test.ts"],
            environment: "node",
          },
        },
        {
          extends: true,
          test: {
            name: "component",
            include: ["src/**/*.svelte.test.ts"],
            browser: {
              enabled: true,
              provider: playwright(),
              headless: true,
              instances: [{ browser: "chromium" }],
            },
          },
        },
      ],
    },
  };
});
