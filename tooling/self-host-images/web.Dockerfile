# syntax=docker/dockerfile:1
FROM oven/bun:1.3.5 AS bun
FROM rust:1.94-trixie AS builder
COPY --from=bun /usr/local/bin/bun /usr/local/bin/bun
RUN apt-get update && apt-get install -y --no-install-recommends cmake clang libclang-dev \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install --locked just --version 1.59.0 \
    && cargo install --locked wasm-pack --version 0.15.1 \
    && rustup target add wasm32-unknown-unknown
WORKDIR /app
COPY . .
# Build inputs contain source only; Git credentials and local config are excluded.
RUN bun install --frozen-lockfile
ARG VITE_PUBLIC_ORIGIN=https://delegate.lumintel.ai
ARG VITE_FUSIONAUTH_CLIENT_ID=bb8570aa-1529-5eaf-aea7-084078c76620
ARG VITE_FUSIONAUTH_TENANT_ID=c46c1f71-3fb9-5b56-903c-1a40e7fe9a27
ARG VITE_SYNC_SERVICE_HOST=delegate.lumintel.ai/sync
ENV VITE_PUBLIC_ORIGIN=${VITE_PUBLIC_ORIGIN} \
    VITE_FUSIONAUTH_CLIENT_ID=${VITE_FUSIONAUTH_CLIENT_ID} \
    VITE_FUSIONAUTH_TENANT_ID=${VITE_FUSIONAUTH_TENANT_ID} \
    VITE_SYNC_SERVICE_HOST=${VITE_SYNC_SERVICE_HOST} \
    SQLX_OFFLINE=true CARGO_BUILD_JOBS=3
WORKDIR /app/apps/web
# Serial WASM builds avoid eight concurrent Rust compilers on standard runners.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target,id=web-wasm-target,sharing=locked \
    for recipe in cache-wasm agent-fold-wasm database-sql-wasm pptx-engine-wasm docx-engine-wasm fig-engine-wasm psd-engine-wasm ai-engine-wasm; do \
      just "build-$recipe" || exit; \
    done
RUN export MIN_NATIVE_BUILD_ANDROID="$(bun -e "console.log(require('./min-native-build.json').android)")" \
    MIN_NATIVE_BUILD_IOS="$(bun -e "console.log(require('./min-native-build.json').ios)")"; \
    MODE=production NODE_ENV=production NODE_OPTIONS=--max-old-space-size=6144 bun run --bun build \
    && bun scripts/cache-wasm/cli.ts package-dist --dist dist
FROM nginx:1.28-alpine
COPY tooling/self-host-images/web.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/apps/web/dist/ /usr/share/nginx/html/app/
EXPOSE 80
