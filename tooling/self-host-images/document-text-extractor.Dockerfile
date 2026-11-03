# syntax=docker/dockerfile:1
FROM rust:1.94-trixie AS builder
ARG SERVICE_NAME=document_text_extractor_vm
ENV SQLX_OFFLINE=true PDFIUM_LIB_PATH=./pdfium-lib/linux
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends cmake clang libclang-dev \
    && rm -rf /var/lib/apt/lists/*
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=shared \
    --mount=type=cache,target=/usr/local/cargo/git/db,sharing=shared \
    --mount=type=cache,target=/app/target,id=rust-target-release,sharing=locked \
    cargo build --locked --release --bin ${SERVICE_NAME} \
    && strip /app/target/release/${SERVICE_NAME} \
    && cp /app/target/release/${SERVICE_NAME} /app/svc

FROM debian:trixie-slim
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates dumb-init libstdc++6 \
    && rm -rf /var/lib/apt/lists/*
COPY --chmod=755 --from=builder /app/svc /app/svc
COPY --from=builder /app/services/document_text_extractor/pdfium-lib/linux/libpdfium.so /app/pdfium-lib/linux/libpdfium.so
ENV PORT=8080
ENTRYPOINT ["dumb-init", "./svc"]
