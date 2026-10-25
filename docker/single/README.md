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
| `/accit ready`           | Exit 0 when the local verification listener answers `/readyz`.                            |

## CI build & push (GitHub Actions)

`.github/workflows/release.yml` builds this image (as part of cutting a
release) and pushes it to GHCR (`ghcr.io/{owner}/{repo}`).

Setup:

- Nothing site-specific is baked in: hostnames, Policy and secrets come from the mounted configuration.
- Auth uses the automatic `GITHUB_TOKEN` (`packages: write`). On the first push
  the package is private; link it to the repo or change its visibility in the
  package settings.

The image is multi-arch (`linux/amd64`, `linux/arm64`; the Rust stage cross-compiles on the build host, no emulation) and carries SLSA provenance and a BuildKit SBOM. The binary is built with `cargo auditable`, so the SBOM and the Trivy scan list its Rust crates; the Portal's JavaScript is compiled in and not listed (`bun audit` covers it). Base images are pinned by digest in the `Dockerfile` (`ARG`s at the top, with the bump command); bump them deliberately, then rebuild. The job runs the dependency scan, pushes `sha-<commit>` only, then runs a Trivy scan of the amd64 image (HIGH and CRITICAL with a fix fail the release). Only after both pass does it add `latest` and the version tag; a failed Trivy scan leaves `sha-<commit>` in GHCR; the SBOM of the amd64 image is attached to the GitHub release. The Portal budget and the cargo-deny policy run in `scripts/ci`, not in the workflow. Budgets and measurements: [docs/performance.md](../../docs/performance.md).

Trigger: automatically when a release is cut (merge `staging → main`), or
manually via **workflow_dispatch** with `build_only=true` to rebuild without
releasing. Images are tagged `latest`, `sha-<commit>`, and the version tag on a
release build. See [RELEASING.md](../../RELEASING.md).

## Security

- Runs as the distroless `nonroot` user, without a shell or package manager.
- Run it with a read-only root filesystem, no capabilities, `no-new-privileges`, and only the data directory writable (`docs/spec.md` section 13).
- No published ports. Only Traefik, at its pinned address on the `accit` network, may reach either listener.
