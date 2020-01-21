FROM rustlang/rust:nightly AS builder
WORKDIR /build

# Download the target for static linking.
RUN rustup target add x86_64-unknown-linux-gnu

RUN USER=root cargo new rusty_socks
WORKDIR /build/rusty_socks

COPY Cargo.toml Cargo.lock ./
RUN cargo build --release

# Copy the source and build the application.
COPY src ./src
RUN cargo build --release --target x86_64-unknown-linux-gnu

# Copy the statically-linked binary into a scratch container.
FROM ubuntu
COPY --from=builder /build/rusty_socks/target/x86_64-unknown-linux-gnu/release/rusty_socks .
COPY settings.toml .
CMD ["sh", "-c", "./rusty_socks settings.toml"]