# packages/env

Env schemas of the web app and its tooling. The server's env lives in `crates/config` (Rust).

## What it is

- `src/web.ts` = public/browser-safe env schema.
- `src/server.ts` = web build/dev tooling env (`readWebToolingEnv`, used by `apps/web/vite.config.ts`). The web app itself is static and reads no server env.
- Uses `@t3-oss/env-core` + `zod` to validate env at import time.

## How it works

- Add every new web/tooling env var here first; server vars go in `crates/config/src/lib.rs` and `.env.example`.
- Do not create app-local or package-local env schemas.
- Apps/packages import from `@accit/env/web` or `/env/server`.
- Add vars to file that owns them.
- Public vars must start `PUBLIC_`.
- Keep browser-safe and server-only vars split.
- `runtimeEnv` reads real process/import env, schema validates shape.
- If no server vars needed, keep `server: {}`. Preserve package export, add vars later.

## Example envs

`apps/web/.env.example`

```bash
BACKEND_URL=http://127.0.0.1:3000
```

`.env.example` (repo root) holds the server's (DATABASE_URL, BETTER_AUTH_SECRET, ...).

## Rules

- Add only vars app actually uses.
- Delete dead vars fast.
- No duplicate env vars, no duplicate schemas, no direct `process.env` validation outside this package.
- If export paths change, update `package.json` too.
