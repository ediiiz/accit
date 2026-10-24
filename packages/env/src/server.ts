import { createEnv } from "@t3-oss/env-core";
import { z } from "zod";

/**
 * Web build/dev tooling env (apps/web/vite.config.ts). The Portal itself is
 * static and reads no env. Takes the values
 * explicitly: Vite reads `.env` files with `loadEnv`, not into process.env.
 */
export const readWebToolingEnv = (runtimeEnv: Record<string, string | undefined>) =>
  createEnv({
    server: {
      // Where the Vite dev/preview server proxies `/api/*`: the Portal listener (dev/accit.yaml).
      BACKEND_URL: z.url().default("http://127.0.0.1:3000"),
    },
    runtimeEnv,
    emptyStringAsUndefined: true,
  });
