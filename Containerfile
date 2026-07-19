FROM docker.io/rustlang/rust:nightly-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY src/ src/
COPY migrations/ migrations/
RUN cargo build --release

FROM docker.io/debian:bookworm-slim

RUN apt-get update && apt-get install -y --only-upgrade --no-install-recommends \
    libssl3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/app-home-services /usr/local/bin/app-home-services

EXPOSE 3000

CMD ["app-home-services"]
