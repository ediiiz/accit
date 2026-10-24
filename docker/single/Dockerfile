# syntax=docker/dockerfile:1.7
#
# accit as ONE executable on a distroless base: the Rust binary with the
# prerendered Portal compiled in. Portal listener :8080, verification
# listener :8081; configuration is /config/accit.yaml (mount read-only).
#
# Build (repo root as context):
#   docker build -f docker/single/Dockerfile -t accit .

# Base images are pinned by digest (the manifest list, so amd64 and arm64 both resolve). To bump:
#   docker buildx imagetools inspect <image:tag> --format '{{.Manifest.Digest}}'
ARG BUN_IMAGE=oven/bun:1.4.3-slim@sha256:14a223fa35747ec728bce300ef85888bc5f7958164d15bbba6a54b22b52a3288
ARG RUST_IMAGE=rust:1.97-trixie@sha256:b1b3c9c0d921d7fa0a6d1f9ec7e4eab87f8c8ec97644c3d791450f131dec813f
ARG RUNTIME_IMAGE=gcr.io/distroless/cc-debian13:nonroot@sha256:3ee00291cf386ec52a552e6a858d555debd74776eb4bfa53fa3df2cd6677515e
ARG TURBO_VERSION=2.11.10

# The Portal build is architecture-independent: it runs once, natively, whatever the target.
FROM --platform=$BUILDPLATFORM ${BUN_IMAGE} AS web-base
ENV HUSKY=0 \
    PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 \
    DO_NOT_TRACK=1
WORKDIR /app

FROM web-base AS web-pruner
ARG TURBO_VERSION
COPY . .
RUN bunx turbo@${TURBO_VERSION} prune web --docker

FROM web-base AS web-deps
COPY --from=web-pruner /app/out/json/ ./
RUN --mount=type=cache,target=/root/.bun/install/cache \
    bun install --ignore-scripts

FROM web-base AS web
ARG TURBO_VERSION
ENV NODE_ENV=production \
    SKIP_ENV_VALIDATION=1
COPY --from=web-deps /app/ ./
COPY --from=web-pruner /app/out/full/ ./
RUN bunx turbo@${TURBO_VERSION} run build --filter=web

FROM ${RUST_IMAGE} AS server
WORKDIR /src
# sqlx macros use the checked-in query cache (crates/store/.sqlx).
ENV SQLX_OFFLINE=true
COPY Cargo.toml Cargo.lock clippy.toml rustfmt.toml ./
COPY crates crates
# rust-embed reads this folder at compile time.
COPY --from=web /app/apps/web/build apps/web/build
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p server \
    && cp target/release/accit /accit

# glibc, CA certificates, a nonroot user; no shell, no package manager.
FROM ${RUNTIME_IMAGE} AS runner
COPY --from=server /accit /accit

EXPOSE 8080 8081

# No shell or curl in distroless: the binary probes itself.
HEALTHCHECK --start-period=10s --interval=15s --timeout=5s --retries=3 \
  CMD ["/accit", "healthcheck"]

ENTRYPOINT ["/accit"]
CMD ["serve"]
