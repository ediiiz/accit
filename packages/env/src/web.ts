import { createEnv } from "@t3-oss/env-core";

// SAFETY: this module is only ever bundled by Vite (see apps/web/vite.config.ts,
// which maps PUBLIC_ vars onto `import.meta.env`), so `env` is present and holds
// string-valued definitions. The property is optional here and defaulted below,
// so a non-Vite loader yields an empty record instead of a crash.
const runtimeEnv = (import.meta as { env?: Record<string, string | undefined> }).env ?? {};

export const env = createEnv({
  clientPrefix: "PUBLIC_",
  // No public vars yet: the app reaches the backend on its own origin. Add
  // PUBLIC_* vars here; they are inlined into the static build.
  client: {},
  runtimeEnv,
  emptyStringAsUndefined: true,
  skipValidation: typeof process !== "undefined" && Boolean(process.env?.SKIP_ENV_VALIDATION),
});
