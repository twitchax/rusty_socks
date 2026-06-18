# syntax=docker/dockerfile:1

FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /build

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
# Compile dependencies in their own cached layer — only invalidated when the recipe changes.
RUN cargo chef cook --release --recipe-path recipe.json
# Build the application against the cached dependency layer.
COPY . .
RUN cargo build --release --bin rusty_socks

FROM debian:stable-slim AS runtime
COPY --from=builder /build/target/release/rusty_socks /usr/local/bin/rusty_socks
# Configure via CLI flags or RS_* env vars, e.g. `docker run -e RS_PORT=1080 -e RS_ACCEPT_CIDR=10.0.0.0/8 ...`.
ENTRYPOINT ["rusty_socks"]
