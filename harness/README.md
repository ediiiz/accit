# Compose harness

Real Traefik in front of accit, for `crates/e2e`. Run it with `scripts/e2e` (needs a working Docker): it generates the test PKI and token (`harness/prepare`), starts the stack, runs the suite and removes the stack. Not part of `scripts/ci`. `scripts/e2e-web` runs the Portal's Playwright suite (`apps/web/tests/e2e`) against the same stack from the `browser` service (profile `browser`, Chromium at `10.89.10.13`).

```
 client (10.89.10.10-.12) ──► traefik ──► accit  (10.89.20.3, profile: test)
   edge 10.89.10.0/24         10.89.10.2   whoami-gated / whoami-open (10.89.30.3, .4)
                              10.89.20.2   (accit: 10.89.20.0/24, apps: 10.89.30.0/24, both internal)
```

- `traefik/traefik.yml`: static config, `core.strictTLSOptions`, the `accit@file` chain on `websecure` (`docs/spec.md` section 10). `traefik/accit.yml.in`: chain, routers, services, the Portal's mTLS options. `prepare` fills in the token.
- `accit.yaml`: selected mode, gated host `gated.accit.test`, open host `open.accit.test`, Portal `portal.accit.test`. The suite rewrites it to test reload.
- The client sends from `10.89.10.11` and `.12` (`curl --interface`); the suite reaches Traefik and accit only through `docker compose exec`.
- Hostnames resolve through `extra_hosts` on the client. Nothing is published to the host.
- `run/` is generated and ignored. CA keys stay in `run/ca`, which no container mounts.
