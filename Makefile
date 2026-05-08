SHELL := /bin/bash
.PHONY: dev server client install build build-client build-server setup fmt lint fix docker-build docker-run docker-stop docker-logs docker-rm

# ── Development ──────────────────────────────────────────────────────────────

# Run backend + frontend dev server concurrently (hot-reload)
dev:
	@trap 'kill 0' EXIT; \
	$(MAKE) server & \
	$(MAKE) client & \
	wait

# Rust backend (uses local config fallback in dev)
server:
	COREMQ_CONFIG=server/coremq-server/config/config.yaml \
	COREMQ_DATA=data \
	cargo run -p coremq-server

# React dev server (hot-reload on port 3039)
client:
	cd client && yarn dev

# ── Build ─────────────────────────────────────────────────────────────────────

# Build React first, then embed into Rust binary
build: build-client build-server

build-client:
	cd client && yarn install && yarn build

build-server:
	cargo build --release -p coremq-server

# ── Docker ───────────────────────────────────────────────────────────────────

docker-build:
	docker build -t coremq:latest .

docker-run:
	docker run -d \
		--name coremq \
		--restart unless-stopped \
		-p 18083:18083 \
		-p 1883:1883 \
		-p 8083:8083 \
		-p 8883:8883 \
		-v coremq_data:/etc/coremq \
		-e COREMQ_CONFIG=/etc/coremq/config.yaml \
		coremq:latest

docker-stop:
	docker stop coremq

docker-rm: docker-stop
	docker rm coremq

docker-logs:
	docker logs -f coremq

# ── Setup ────────────────────────────────────────────────────────────────────

install:
	cd client && yarn install

setup:
	@command -v cargo >/dev/null 2>&1 || { echo "Installing Rust..."; curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y; source $$HOME/.cargo/env; }
	cd client && yarn install
	@echo "Setup complete. Run 'make dev' to start in development mode."
	@echo "Run 'make build' to produce a production binary."

# Install config files to /etc/coremq/ (Linux/macOS)
install-config:
	sudo mkdir -p /etc/coremq/data /etc/coremq/tls
	sudo cp server/coremq-server/config/config.yaml /etc/coremq/config.yaml
	sudo cp server/coremq-server/config/model.conf  /etc/coremq/model.conf
	sudo cp server/coremq-server/config/policy.csv  /etc/coremq/policy.csv
	@echo "Config installed to /etc/coremq/"
	@echo "Edit /etc/coremq/config.yaml before starting the broker."

# ── Code quality ─────────────────────────────────────────────────────────────

fmt:
	cd client && npx prettier --write "src/**/*.{ts,tsx}"

lint:
	cd client && npx eslint "src/**/*.{js,jsx,ts,tsx}"

fix:
	cd client && npm run fix:all
