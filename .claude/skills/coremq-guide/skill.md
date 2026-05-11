---
name: coremq-guide
description: Reference for navigating the CoreMQ project — directory layout, build commands, default ports, where things live across the Rust broker and React dashboard. TRIGGER when user asks about project structure, where to put files, or how the server and client connect.
---

# CoreMQ Project Guide

Cargo workspace + Vite-React app. One Rust crate (the broker), one TS app (the dashboard).

For deep conventions (TypeScript types, Zustand store shape, API routes, theme tokens, AdminCommand pattern), see `.claude/skills.md` — it's the canonical project doc.

## Structure

```
coremq-rust/
├── server/coremq-server/   — Rust MQTT broker (Tokio, Axum, ReDB, Casbin)
│   └── src/
│       ├── api/            — Axum REST API (controllers, router, auth)
│       ├── engine/         — Core event loop + command enums + workers
│       ├── services/       — session, topic, jwt
│       ├── protocol/       — MQTT 3.1.1 / 5 wire protocol
│       ├── transport/      — TCP / TLS / WebSocket listeners
│       ├── storage/        — ReDB persistence
│       ├── models/         — Serde structs (api/, engine/)
│       └── main.rs
├── client/                 — React 19 + TS + MUI 7 + Zustand admin dashboard
│   └── src/
│       ├── pages/          — thin route wrappers
│       ├── sections/       — feature views (UI + logic)
│       ├── stores/         — Zustand state
│       ├── services/       — axios API calls
│       ├── types/          — TypeScript types
│       ├── theme/          — MUI dark theme
│       └── 118n/           — en.json, ko.json, uz.json
├── docs/                   — ARCHITECTURE.md, COREMQ_AI_NATIVE_PLATFORM.md, drawio diagrams
├── tests/                  — integration / stress / qos tests
├── Cargo.toml              — workspace root
├── Makefile                — `make dev` / `server` / `client` / `fmt` / `lint` / `fix`
└── .claude/skills.md       — full project conventions
```

## Common Commands

| What                  | Command                                          |
| --------------------- | ------------------------------------------------ |
| Run both             | `make dev`                                       |
| Run broker only      | `make server` (= `cargo run -p coremq-server`)   |
| Run frontend only    | `make client` (= `cd client && yarn dev`)        |
| Install everything   | `make setup`                                     |
| Build broker         | `cargo build -p coremq-server`                   |
| Test broker          | `cargo test -p coremq-server`                    |
| Format Rust          | `cargo fmt -p coremq-server`                     |
| Lint Rust            | `cargo clippy -p coremq-server --all-targets`    |
| Format frontend      | `make fmt`                                       |
| Lint frontend        | `make lint`                                      |
| Lint + format fix    | `make fix`                                       |

## Default Ports

| Service       | Port    |
| ------------- | ------- |
| MQTT TCP      | `1883`  |
| MQTT TLS      | `8883`  |
| MQTT WebSocket| `8083`  |
| REST API      | `18083` |
| Frontend dev  | `3039`  |

## Default Credentials

Username: `admin` / Password: `public`

## Cross-cutting Wiring

The broker and dashboard talk over two channels:

- **REST API** (`http://localhost:18083/api/v1/...`) — admin operations: list sessions/topics/listeners/users, publish, login. See `server/coremq-server/src/api/router.rs` for the full route table.
- **MQTT WebSocket** (`ws://localhost:8083`) — the dashboard speaks MQTT directly to subscribe to live topic updates. See `client/src/sections/websocket/`.

## Gotchas

- `Cargo.lock` IS committed (binary crate — needs reproducible builds).
- Frontend uses **`yarn`**, not `npm install` (Makefile uses yarn, lockfile is `yarn.lock`).
- `target/` and `client/node_modules/` are gitignored — don't commit either.
- See `.claude/skills/debug-coremq/skill.md` for runtime issue diagnosis.
