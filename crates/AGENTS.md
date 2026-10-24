# Rust (`crates/`)

One binary, `accit` (crate `server`): the Portal listener (embedded web build, `/api/*`) and the verification listener (`/verify`, `/healthz`, `/readyz`). Spec: `docs/spec.md`. Terms: `GLOSSARY.md`.

## Crates

Dependencies flow one way: `config ← server`.

| Crate    | Holds                                                                                                      |
| -------- | ---------------------------------------------------------------------------------------------------------- |
| `config` | `/config/accit.yaml`: parsing and validation. Unknown keys are errors. No env reads beyond `ACCIT_CONFIG`. |
| `server` | Listeners, routers (`portal.rs`, `verify.rs`), readiness, static site, CLI (`cli.rs`).                     |

## Commands

No host toolchain needed: `scripts/cargo <args>` runs cargo in the pinned image (`docker/rust-dev`).

- Gate (run before every commit touching `crates/`): `scripts/check-rust`. It runs fmt, clippy `-D warnings`, the 300-line file cap and all tests.
- Format: `scripts/cargo fmt`.
- Dev server: `bun run dev:server` (config `dev/accit.yaml`, `profile: test`).

## Rules

- Lints are strict on purpose (`Cargo.toml` `[workspace.lints]`, `clippy.toml`): cognitive complexity ≤ 10, nesting ≤ 4, ≤ 50 lines per function, ≤ 5 arguments, no `unwrap`/`expect`/`panic`/`print`. Split code; do not `allow` a lint without a `reason`.
- Fail closed. Any doubt about the caller, the Address, the host or the state answers 403 or 503, never 2xx.
- The verification listener answers only `/verify`, `/healthz`, `/readyz`; the Portal listener never answers those. Unknown paths are 404 on both.
- Bodies of denied verifications reach the client through Traefik: keep them generic (`Forbidden`), no product name or detail.
- Never log tokens, cookies, request bodies, certificate material or full URLs.
- Tests first: every new behaviour or bug fix starts with a test that fails, at a public seam (a router through `tower::ServiceExt::oneshot`, or a pure function's public API), then the code that turns it green. One behaviour per cycle (`tdd` skill).
- Expected values come from `docs/spec.md`, never from running the code under test.
