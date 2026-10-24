import { createEnv } from "@t3-oss/env-core";
import { z } from "zod";

/**
 * Web build/dev tooling env (apps/web/vite.config.ts). The app itself is
 * static; the Elysia backend's env lives in ./backend.ts. Takes the values
 * explicitly: Vite reads `.env` files with `loadEnv`, not into process.env.
 */
export const readWebToolingEnv = (runtimeEnv: Record<string, string | undefined>) =>
  createEnv({
    server: {
      // Where the Vite dev/preview server proxies `/api/*` (HTTP + WebSocket) to.
      BACKEND_URL: z.url().default("http://127.0.0.1:3000"),
    },
    runtimeEnv,
    emptyStringAsUndefined: true,
  });
