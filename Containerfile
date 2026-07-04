FROM docker.io/rustlang/rust:nightly-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src src/
COPY tests tests/
COPY migrations migrations/

RUN cargo build --release 2>/dev/null; true

CMD ["cargo", "test"]
