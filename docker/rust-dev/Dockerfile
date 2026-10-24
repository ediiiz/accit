# Toolchain for scripts/cargo: pinned Rust + clippy + rustfmt + sqlx-cli.
FROM rust:1.97-trixie
RUN rustup component add clippy rustfmt
RUN cargo install sqlx-cli --version 0.9.0 --locked --no-default-features --features sqlite
