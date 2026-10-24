# packages/config

Tiny shared config package.

## What it is

- `tsconfig.base.json` = base TypeScript rules for workspaces.
- Package has no runtime code.

## Rules

- Keep config strict.
- Change here affects many packages. Check dependents.
- Prefer one shared rule over copy-paste package rules.
- If adding exports/files, update `package.json` too.
- Use `bun run typecheck` from repo root after TS config changes.
