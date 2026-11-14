# Stage 1: Build
FROM rust:1.82-slim-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml ./
COPY crates ./crates
COPY bins ./bins
COPY migrations ./migrations

RUN cargo build --release --bin api

# Stage 2: Runtime
FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/api /usr/local/bin/api
COPY --from=builder /app/migrations /app/migrations

EXPOSE 8080

CMD ["api"]
