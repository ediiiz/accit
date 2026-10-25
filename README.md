# accit

A gate in front of self-hosted web apps behind Traefik. A trusted person opens the Portal with a shared client certificate and taps **Enable access**. Their current public IP address can then reach the gated apps for a fixed time. Everyone else gets `403 Forbidden` before any app answers. Each app keeps its own login.

## Why

Self-hosted apps on a public domain face the whole internet: every login page, API and unpatched endpoint is one scan away. The usual answers each break something:

- **VPN**: every device needs a client, and most TV and console apps cannot run one.
- **SSO in front of the apps** (Authelia, Authentik): native mobile and TV clients cannot follow the login redirect, and the SSO login page itself stays public.
- **Static IP allowlist**: home and mobile addresses change daily.

accit allows **addresses**, not devices or users. One certificate-protected tap from a phone opens the current connection for every device on it, including the TV on the same Wi-Fi. Apps and their clients need no changes, and nothing is reachable from an address nobody enabled. It combines with SSO: put accit in front, keep Authelia behind it.

It is a network barrier, not an identity system: all certificate holders are equal, and the apps' own logins still apply.

## How it works

```
Phone ── HTTPS + client cert ──► Traefik ── Portal router ──► accit :8080 (Portal, /api)
                                                                   │ Unlock / Revoke
                                                                   ▼
App client ── HTTPS ──► Traefik ── ForwardAuth ──► accit :8081 /verify ── Grants (memory + SQLite)
                           │
                           └── 2xx only ──► app (its own login still applies)
```

- One ForwardAuth chain on Traefik's `websecure` entrypoint checks every request, so new apps start out checked ([ADR 0001](docs/adr/0001-entrypoint-gate-with-policy-in-accit.md)).
- The Policy inside accit decides which hosts need a Grant: `selected` (listed hosts) or `all` (every host except listed ones). It reloads without restarting Traefik.
- accit trusts only Traefik's pinned address on a dedicated Docker network, plus a proxy token ([ADR 0002](docs/adr/0002-trust-traefik-by-pinned-address-and-token.md)).
- Trusted networks let the home LAN pass without a Grant when app hostnames resolve to the LAN at home.
- It fails closed: while accit is down or unsure of its clock, every request on the entrypoint is refused.

Terms: [`GLOSSARY.md`](GLOSSARY.md). Full behaviour: [`docs/spec.md`](docs/spec.md).

## Deploy

| For                     | Read                                                                                                                     |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Saltbox                 | [`deploy/saltbox/README.md`](deploy/saltbox/README.md): Ansible role through saltbox_mod                                 |
| Any Traefik ≥ 3.7.16    | [`docs/guides/operator.md`](docs/guides/operator.md)                                                                     |
| Certificate holders     | [`docs/guides/onboarding.en.md`](docs/guides/onboarding.en.md), [German](docs/guides/onboarding.de.md), PDFs beside them |
| Limits and attack model | [`docs/guides/threat-model.md`](docs/guides/threat-model.md)                                                             |

The image is `ghcr.io/ediiiz/accit:<version>`, built when `staging` is merged into `main` ([`RELEASING.md`](RELEASING.md)). In use on one Saltbox host (Ubuntu 24.04, Traefik 3.7.16, Authelia behind accit), opened from Firefox on Android.

## Repository

- `crates/` — Rust: one binary, `accit`. See `crates/AGENTS.md`.
- `apps/web` — the Portal: one prerendered SvelteKit route, embedded into the binary. See `apps/web/AGENTS.md`.
- `deploy/saltbox` — the Saltbox role and its off-host checks.
- `docker/single` — the image: the binary on distroless. See `docker/single/README.md`.
- `harness/`, `crates/e2e` — Compose harness with real Traefik for end-to-end tests.
- `packages/env`, `packages/config` — web tooling env schema, shared TS config.

## Development

Needs bun and Docker (cargo runs in a container through `scripts/cargo`; a host toolchain works too).

```bash
bun install
bun run dev:server   # accit with dev/accit.yaml: Portal :3000, verify :3001
bun run dev          # Portal dev server :5173, proxies /api to :3000
```

| Command                                      | Does                                                  |
| -------------------------------------------- | ----------------------------------------------------- |
| `scripts/ci`                                 | Full local CI; must pass before merging to `staging`. |
| `scripts/check-rust`                         | Rust gate: fmt, clippy, file length, tests.           |
| `deploy/saltbox/check`                       | Role tests, lint and template renders.                |
| `scripts/e2e`                                | End-to-end tests through the Compose harness.         |
| `bun run check`                              | Portal typecheck.                                     |
| `bun run lint`, `bun run format:check`       | oxlint + eslint, oxfmt.                               |
| `bun run fallow`                             | Dead code, duplication, health.                       |
| `scripts/client-ca create\|rotate [dir]`     | Client CA, shared leaf and `.p12`.                    |
| `scripts/guides-pdf`                         | Rebuild the onboarding PDFs.                          |
| `docker build -f docker/single/Dockerfile .` | Production image.                                     |
| `scripts/web-budget`, `scripts/bench`        | Portal 150 KiB budget; RSS and latency.               |
| `scripts/audit`                              | Dependency advisories (needs the network).            |

Work lands on `staging` through PRs; `main` is release-only.

## Supported versions

| Component | Version                                                              |
| --------- | -------------------------------------------------------------------- |
| Traefik   | ≥ 3.7.16                                                             |
| Rust      | 1.97 (`rust-version`; the image and `scripts/cargo` build with 1.97) |
| Svelte    | 5 (SvelteKit 3), build time only                                     |
| Bun       | 1.4 (build time only)                                                |
| Image     | `linux/amd64`, `linux/arm64`, distroless `cc-debian13`, nonroot      |

Budgets and their measurements: [`docs/performance.md`](docs/performance.md).

## i18n

Paraglide, English + German: `apps/web/messages/{en,de}.json`. The browser's language wins, then the administrator's default. Add every new key to both files.
