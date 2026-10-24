# Container image

One image, one process: the `accit` binary on a distroless base, with the prerendered Portal embedded.

```
                    ┌──── container (distroless, nonroot) ─────┐
  Traefik ─────────►│  :8080  Portal: web app + /api/*          │
  (Portal router,   │  :8081  /verify (ForwardAuth), /healthz,  │
   ForwardAuth)     │         /readyz                           │
                    └───────────────────────────────────────────┘
                       /config/accit.yaml (read-only)
```

Build: `docker build -f docker/single/Dockerfile -t accit .` (repo root as context). It runs `vite build`, then `cargo build --release -p server`, which embeds `apps/web/build`.

Run: mount the configuration read-only at `/config/accit.yaml` (or point `ACCIT_CONFIG` elsewhere). Publish no ports: Traefik reaches accit over the dedicated Docker network (`docs/spec.md` section 10).

## Commands

| Command                  | Does                                                                                      |
| ------------------------ | ----------------------------------------------------------------------------------------- |
| `/accit serve` (default) | Both listeners.                                                                           |
| `/accit healthcheck`     | Exit 0 when the local verification listener answers `/healthz` (the image `HEALTHCHECK`). |

## CI build & push (GitHub Actions)

`.github/workflows/release.yml` builds this image (as part of cutting a
release) and pushes it to GHCR (`ghcr.io/{owner}/{repo}`).

Setup:

- Nothing site-specific is baked in: hostnames, Policy and secrets come from the mounted configuration.
- Auth uses the automatic `GITHUB_TOKEN` (`packages: write`). On the first push
  the package is private; link it to the repo or change its visibility in the
  package settings.

Trigger: automatically when a release is cut (merge `staging → main`), or
manually via **workflow_dispatch** with `build_only=true` to rebuild without
releasing. Images are tagged `latest`, `sha-<commit>`, and the version tag on a
release build. See [RELEASING.md](../../RELEASING.md).

## Security

- Runs as the distroless `nonroot` user, without a shell or package manager.
- Run it with a read-only root filesystem, no capabilities, `no-new-privileges`, and only the data directory writable (`docs/spec.md` section 13).
- No published ports. Only Traefik, at its pinned address on the `accit` network, may reach either listener.
