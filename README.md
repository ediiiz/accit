# accit

Lets trusted people temporarily allow their current public IP address to reach self-hosted web apps behind Traefik on a Saltbox host. A shared client certificate protects the Portal; each app keeps its own login. Spec: [`docs/spec.md`](docs/spec.md). Terms: [`GLOSSARY.md`](GLOSSARY.md). Decisions: [`docs/adr/`](docs/adr).

- `crates/` — Rust: one binary, `accit`, with a Portal listener (:8080) and a verification listener for Traefik ForwardAuth (:8081). See `crates/AGENTS.md`.
- `apps/web` — the Portal: one prerendered SvelteKit route, embedded into the binary. See `apps/web/AGENTS.md`.
- `packages/env`, `packages/config` — web tooling env schema, shared TS config.
- `docker/single` — the image: the binary on distroless. See `docker/single/README.md`.

## Request flow

```
Phone ── HTTPS + client cert ──► Traefik ── Portal router ──► accit :8080 (Portal, /api)
                                                                   │ Unlock / Revoke
                                                                   ▼
App client ── HTTPS ──► Traefik ── ForwardAuth ──► accit :8081 /verify ── Grants (memory + SQLite)
                           │
                           └── 2xx only ──► app (its own login still applies)
```

## Setup

Needs bun and Docker (cargo runs in a container through `scripts/cargo`; a host toolchain works too).

```bash
bun install
bun run dev:server   # accit with dev/accit.yaml: Portal :3000, verify :3001
bun run dev          # Portal dev server :5173, proxies /api to :3000
```

## Commands

| Command                                      | Does                                        |
| -------------------------------------------- | ------------------------------------------- |
| `bun run dev` / `bun run dev:server`         | Portal dev server / accit.                  |
| `bun run check`                              | Portal typecheck.                           |
| `scripts/check-rust`                         | Rust gate: fmt, clippy, file length, tests. |
| `bun run lint`, `bun run format:check`       | oxlint + eslint, oxfmt.                     |
| `bun run fallow`                             | Dead code, duplication, health.             |
| `docker build -f docker/single/Dockerfile .` | Production image.                           |
| `scripts/web-budget`, `scripts/bench`        | Portal 150 KiB budget; RSS and latency.     |
| `scripts/audit`                              | Dependency advisories (needs the network).  |

## Supported versions

| Component | Version                                                              |
| --------- | -------------------------------------------------------------------- |
| Traefik   | ≥ 3.7.16 (the Compose harness runs 3.7.16)                           |
| Rust      | 1.97 (`rust-version`; the image and `scripts/cargo` build with 1.97) |
| Svelte    | 5 (SvelteKit 3), build time only                                     |
| Bun       | 1.4 (build time only)                                                |
| Image     | `linux/amd64`, `linux/arm64`, distroless `cc-debian13`, nonroot      |

Budgets and their measurements: [`docs/performance.md`](docs/performance.md).

## i18n

Paraglide, English + German: `apps/web/messages/{en,de}.json`. The browser's language wins, then the administrator's default. Add every new key to both files.
