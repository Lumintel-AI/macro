# Build with the pinned fork as the context, after ticket 03 builds the service images.
FROM rust:1.94-trixie AS builder
RUN cargo install sqlx-cli --version 0.8.6 --locked --no-default-features --features rustls,postgres
FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx
COPY crates/macro_db_client/migrations /migrations
ENTRYPOINT ["sqlx", "migrate", "run", "--source", "/migrations"]
