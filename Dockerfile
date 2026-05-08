# ── Stage 1: Build React frontend ─────────────────────────────────────────────
FROM node:20-alpine AS frontend
WORKDIR /app/client
COPY client/package.json client/yarn.lock ./
RUN yarn install --frozen-lockfile
COPY client/ ./
RUN yarn build

# ── Stage 2: Build Rust binary ────────────────────────────────────────────────
FROM rust:1.83-slim AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY server/ server/
# Embed the built frontend into the binary
COPY --from=frontend /app/client/dist ./client/dist
RUN cargo build --release -p coremq-server

# ── Stage 3: Minimal runtime image ────────────────────────────────────────────
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Config and data directory
RUN mkdir -p /etc/coremq/data

# Copy binary
COPY --from=builder /app/target/release/coremq-server /usr/local/bin/coremq

# Copy default config files
COPY server/coremq-server/config/model.conf  /etc/coremq/model.conf
COPY server/coremq-server/config/policy.csv  /etc/coremq/policy.csv
COPY server/coremq-server/config/config.yaml /etc/coremq/config.yaml

EXPOSE 18083 1883 8083 8883

# Persist config and data across container restarts
VOLUME ["/etc/coremq"]

ENV COREMQ_CONFIG=/etc/coremq/config.yaml

CMD ["coremq"]
