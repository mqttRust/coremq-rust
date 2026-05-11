# Plexus

A high-performance MQTT broker written in async Rust.

Plexus runs the protocol layer (TCP, TLS, WebSocket), the management layer (REST API, embedded admin dashboard), and the persistence layer (ReDB) in a single binary. All transports route through one Tokio-driven engine over async channels — the same model that gives the project its name.

> Status: `0.1.0` · MQTT 3.1.1 · QoS 0 in production, QoS 1/2 wiring in progress.

---

## At a glance

- **Async, single-binary broker** — Tokio engine, mpsc command bus, no external services required.
- **Multi-transport** — plain TCP (`1883`), TLS (`8883`), MQTT-over-WebSocket (`/mqtt` on `8083`), WSS.
- **Built-in REST API** — sessions, topics, users, listeners, publish, real-time metrics stream.
- **Embedded admin dashboard** — React + Material-UI, served on port `18083` from the broker binary itself.
- **Auth out of the box** — Argon2 password hashing, JWT access + refresh tokens, Casbin RBAC.
- **Embedded storage** — ReDB. No external Postgres/Redis to operate.
- **i18n dashboard** — English, Korean, Uzbek.

---

## Quick start (Docker)

```bash
git clone https://github.com/mqttRust/coremq-rust.git plexus
cd plexus
docker compose up -d
```

Dashboard: http://localhost:18083 — default credentials `admin` / `public`. Change them before exposing the broker.

## Build from source

Prerequisites: Rust 1.83+, Node.js 20+, Yarn.

```bash
git clone https://github.com/mqttRust/coremq-rust.git plexus
cd plexus

make build            # builds the React dashboard, embeds it in the Rust binary
make install-config   # copies default config to /etc/coremq/  (Linux/macOS)
sudo ./target/release/coremq-server
```

Detailed install guides: [Linux](docs/install-linux.md) · [macOS](docs/install-macos.md) · [Windows](docs/install-windows.md).

## Development (hot reload)

```bash
make install   # install frontend deps
make dev       # React on :3039, Rust on :18083 — both hot-reload
```

> The crate is currently published as `coremq-server` and the dashboard as `coremq-dashboard`. The package and binary rename to `plexus` will land in a follow-up.

---

## Architecture

```
                   ┌───────────────────────────────┐
                   │       Admin Dashboard         │
                   │     (React + Material-UI)     │
                   └───────────────┬───────────────┘
                                   │ HTTP / WS  (port 18083)
                   ┌───────────────▼───────────────┐
                   │         REST API (Axum)       │
                   │  sessions │ topics │ users    │
                   │  listeners│ publish│ auth     │
                   └───────────────┬───────────────┘
                                   │ async mpsc + oneshot reply
                   ┌───────────────▼───────────────┐
                   │         Engine (Tokio)        │
                   │   ┌─────────┐ ┌───────────┐   │
                   │   │ Session │ │   Topic   │   │
                   │   │ Service │ │  Service  │   │
                   │   └─────────┘ └───────────┘   │
                   └─────┬─────────┬─────────┬─────┘
                         │         │         │
                  ┌──────▼──┐ ┌────▼───┐ ┌───▼──────┐
                  │   TCP   │ │  TLS   │ │ WebSocket│
                  │  (1883) │ │ (8883) │ │  (8083)  │
                  └─────────┘ └────────┘ └──────────┘
```

Transports and the REST API never share state directly. Every operation (`Connect`, `Subscribe`, `Publish`, `GetTopics`, …) is sent as an `AdminCommand` or `PubSubCommand` to the engine over an mpsc channel; the reply comes back via a `oneshot`. The engine's event loop is single-threaded by design — it removes the lock contention that traditional broker architectures pay for, and makes the in-memory state machine easy to reason about.

---

## Features

### MQTT protocol

MQTT 3.1.1, with publish / subscribe / unsubscribe, ping / keepalive, and wildcard topic matching (`+` single-level, `#` multi-level). QoS 0 is fully supported in production. QoS 1/2 packet handling exists; reliable delivery wiring is in progress.

### Transports

Plain TCP, TLS over TCP, MQTT over WebSocket, and Secure WebSocket (WSS). All four share the same engine, the same auth, and the same session model — picking a transport is a deployment choice, not a feature gate.

### Authentication & authorization

User database in ReDB. Passwords are stored as Argon2 hashes. Login returns a short-lived JWT access token and a refresh token. The REST API is protected by middleware that validates the bearer and runs a Casbin RBAC check for every request. Default roles: `admin` and `user`.

### REST API

| Method     | Endpoint                            | Description                                          |
| ---------- | ----------------------------------- | ---------------------------------------------------- |
| `POST`     | `/api/v1/public/login`              | Log in, return JWT access + refresh tokens           |
| `GET`      | `/api/v1/sessions`                  | List connected clients (paginated)                   |
| `DELETE`   | `/api/v1/sessions/:client_id`       | Force-disconnect a client                            |
| `GET`      | `/api/v1/users`                     | List users                                           |
| `POST`     | `/api/v1/users`                     | Create a user                                        |
| `GET`      | `/api/v1/listeners`                 | List active listeners                                |
| `DELETE`   | `/api/v1/listeners/:port`           | Stop a listener                                      |
| `GET`      | `/api/v1/topics`                    | List active topics with subscriber counts            |
| `POST`     | `/api/v1/publish`                   | Publish a message to a topic over HTTP               |
| `GET (WS)` | `/api/v1/ws/metrics`                | Stream broker metrics every 1s (mem, CPU, sessions)  |

### Topic monitoring & REST publish

`GET /api/v1/topics` returns every active topic with its subscriber count. `POST /api/v1/publish` sends a message to any topic without an MQTT client connection. Together they cover three operational needs that MQTT alone makes awkward:

- IoT operators sending commands to fleets from a browser.
- Developers driving a subscription manually without spinning up a client.
- HTTP-only systems integrating with the broker without a long-lived MQTT connection.

```bash
# Publish over HTTP
curl -X POST http://localhost:18083/api/v1/publish \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"topic": "devices/sensor/temperature", "payload": "{\"temp\":23.5}", "qos": 0, "retain": false}'

# List active topics
curl -H "Authorization: Bearer $TOKEN" http://localhost:18083/api/v1/topics
```

### Admin dashboard

Bundled React + Material-UI app served on `:18083`:

- **Home** — overview metrics
- **Sessions** — connected clients, search, force-disconnect
- **Topics** — subscriber counts, publish to any topic from the UI
- **Listeners** — start / stop transports
- **WebSocket Client** — built-in MQTT browser client for testing
- **Admin** — user management (create, list, role assignment)
- **Multi-language** — EN / KO / UZ

The dashboard is statically embedded into the broker binary at build time — no separate webserver to run.

---

## Defaults

| Service                    | Port    |
| -------------------------- | ------- |
| MQTT TCP                   | `1883`  |
| MQTT TLS                   | `8883`  |
| MQTT WebSocket             | `8083`  |
| Dashboard + REST API       | `18083` |
| Frontend dev server        | `3039`  |

| Path                          | Description                  |
| ----------------------------- | ---------------------------- |
| `/etc/coremq/config.yaml`     | Main config (Linux/macOS)    |
| `/etc/coremq/data/`           | ReDB database directory      |
| `C:\ProgramData\CoreMQ\`      | Config root (Windows)        |

Default credentials: `admin` / `public`. **Rotate before exposing the broker.**

---

## Examples

### MQTT over TCP

```bash
mosquitto_sub -h localhost -p 1883 -t test/topic
```

### MQTT over WebSocket

```javascript
mqtt.connect("ws://localhost:8083/mqtt", { protocol: "mqtt" });
```

### Real-time metrics stream

```javascript
const ws = new WebSocket("ws://localhost:18083/api/v1/ws/metrics");
ws.onmessage = (e) => console.log(JSON.parse(e.data));
// {"timestamp":"...","memory_mb":42.1,"cpu_percent":0.8,"client_count":317,"topics":[...]}
```

### REST publish (full flow)

```bash
TOKEN=$(curl -s -X POST http://localhost:18083/api/v1/public/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"public"}' | jq -r '.data.access_token')

curl -X POST http://localhost:18083/api/v1/publish \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"topic":"test/topic","payload":"hello","qos":0,"retain":false}'
```

---

## Performance

The engine is single-threaded by design and does not take long-held locks. Concurrent state (sessions, topic subscriptions) lives in `DashMap` for sharded reads and writes. Transports run on the Tokio multi-thread runtime and feed the engine through bounded mpsc channels — backpressure is real, not theoretical. The full stack is async; there are no blocking syscalls in the hot path.

## Security

TLS and WSS for transport. Argon2id for password hashing. JWT for session tokens with refresh-token rotation. Casbin enforces RBAC on every REST request. CORS middleware on the API. Default credentials are documented but expected to be rotated on first deploy — there is no production deployment story that ships with `admin/public`.

---

## Roadmap

- Full QoS 1 / QoS 2 reliable delivery
- MQTT v5
- Retained messages with persistence
- ACL-based topic permissions (Casbin policies on publish / subscribe)
- Webhooks (publish-side and management-side)
- Prometheus exporter
- Clustering & distributed mode
- Plugin system

---

## Contributing

Read [`CONTRIBUTING.md`](./CONTRIBUTING.md) for setup, the verification suite, and the issue/PR quality bar. The [`AGENTS.md`](./AGENTS.md) file documents rules-of-engagement for AI coding agents (Claude Code, Cursor, Codex). The [`CHANGELOG.md`](./CHANGELOG.md) follows Keep a Changelog and uses lockstep versioning across the broker and dashboard.

## License

MIT.

---

**Plexus — a single-binary MQTT broker, in Rust.**
