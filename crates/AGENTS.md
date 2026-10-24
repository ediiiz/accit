# Rust (`crates/`)

One binary, `accit` (crate `server`): the Portal listener (embedded web build, `/api/*`) and the verification listener (`/verify`, `/healthz`, `/readyz`). Spec: `docs/spec.md`. Terms: `GLOSSARY.md`.

## Crates

Dependencies flow one way: `access ← config ← server` and `access ← store ← server`.

| Crate    | Holds                                                                                                      |
| -------- | ---------------------------------------------------------------------------------------------------------- |
| `access` | The domain, without I/O: Address, Hostname (canonical parsing), and later the gate decision.               |
| `config` | `/config/accit.yaml`: parsing and validation. Unknown keys are errors. No env reads beyond `ACCIT_CONFIG`. |
| `store`  | Grants: the in-memory map verification reads, SQLite (sqlx) behind it, audit events, retention cleanup.    |
| `server` | Listeners, routers (`portal.rs`, `verify.rs`), proxy trust (`trust.rs`), readiness, static site, CLI.      |

## Commands

`scripts/check-rust` uses `$CARGO`, else the host `cargo`, else `scripts/cargo` (cargo in the pinned image, `docker/rust-dev`) for hosts without a toolchain.

- Gate (run before every commit touching `crates/`): `scripts/check-rust`. It runs fmt, clippy `-D warnings`, the 300-line file cap, the sqlx query cache check and all tests.
- Format: `scripts/cargo fmt`.
- Dev server: `bun run dev:server` (config `dev/accit.yaml`, `profile: test`).
- SQL lives in `sqlx::query!` macros checked against the cache in `crates/store/.sqlx`; builds use it (`SQLX_OFFLINE=true`), and the gate fails when it is stale. Needs `sqlx-cli` 0.9 on the host (`cargo install sqlx-cli --version 0.9.0 --locked --no-default-features --features sqlite`; the `scripts/cargo` image has it).
- New migration: `crates/store/migrations/NNNN_name.sql`, next number.
- Regenerate the query cache after changing a query or a migration (also after a rebase that renumbered migrations):

  ```sh
  cd crates/store
  export DATABASE_URL="sqlite://$PWD/../../target/sqlx-dev.db"
  cargo sqlx database reset -y --source migrations && cargo sqlx prepare
  ```

## Rules

- Lints are strict on purpose (`Cargo.toml` `[workspace.lints]`, `clippy.toml`): cognitive complexity ≤ 10, nesting ≤ 4, ≤ 50 lines per function, ≤ 5 arguments, no `unwrap`/`expect`/`panic`/`print`. Split code; do not `allow` a lint without a `reason`.
- Fail closed. Any doubt about the caller, the Address, the host or the state answers 403 or 503, never 2xx.
- The verification listener answers only `/verify`, `/healthz`, `/readyz`; the Portal listener never answers those. Unknown paths are 404 on both.
- Bodies of denied verifications reach the client through Traefik: keep them generic (`Forbidden`), no product name or detail.
- Never log tokens, cookies, request bodies, certificate material or full URLs.
- Tests first: every new behaviour or bug fix starts with a test that fails, at a public seam (a router through `tower::ServiceExt::oneshot`, or a pure function's public API), then the code that turns it green. One behaviour per cycle (`tdd` skill).
- Expected values come from `docs/spec.md`, never from running the code under test.
