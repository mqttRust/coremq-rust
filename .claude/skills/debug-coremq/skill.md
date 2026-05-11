---
name: debug-coremq
description: Diagnose CoreMQ broker, REST API, MQTT client, or React dashboard issues — common errors, port conflicts, auth failures, QoS panics, WebSocket disconnects. TRIGGER when user reports a bug, error, or unexpected behavior.
---

# Debug CoreMQ

Systematic debugging for the CoreMQ MQTT broker and admin dashboard.

## Symptom → Investigation Map

| Symptom                                | First check                                                                       | Then check                                                                          |
| -------------------------------------- | --------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| Broker won't start                     | `lsof -iTCP:1883 -sTCP:LISTEN` — is another process holding the port?             | Check `cargo run` output for `bind` errors and ReDB lock messages                   |
| `cargo run` panics on QoS publish      | Look for `unwrap()` on `packet_id` — must be a known bug class                    | Check the `fix-panic_id-none-server-panic-bug` branch / commit `2f10d8d` for fix    |
| MQTT client disconnects immediately    | `cargo run` log: auth failure? Casbin policy missing?                             | Verify default `admin/public` creds and that JWT secret env var is set              |
| REST API returns 401 after login       | Token cookie not being attached — check axios interceptor + cookie helpers         | Verify `/api/v1/public/login` returns the token and the refresh endpoint works      |
| REST controller hangs forever          | The engine `match` arm is not sending on the `oneshot::Sender` — must reply       | grep for the `AdminCommand` variant in `engine/engine.rs` `run()` loop              |
| Frontend shows stale data after logout | Some Zustand store is missing `reset()`                                           | Check every store in `client/src/stores/` has `reset: () => set(initialState)`      |
| MQTT WebSocket fails to connect        | Is `:8083` listening? Browser console: CORS or upgrade failure?                   | Check `client/src/sections/websocket/` for the connect logic and broker URL         |
| Topic publish doesn't match subscriber | Wildcard order — `+` matches one level, `#` matches multi (must be at end)        | Check `services/topic.rs` matcher; verify subscriber QoS is supported by broker      |
| `cargo build` fails on macOS           | Toolchain stale: `rustup update`                                                  | `cargo clean` if proc-macro errors persist                                          |
| Frontend dev server port conflict      | `lsof -iTCP:3039 -sTCP:LISTEN` — kill the stale process                           | Check `vite.config.ts` for the configured port                                      |
| i18n key shows as raw `feature.title`  | Key missing from one of `118n/en.json`, `118n/ko.json`, `118n/uz.json`            | Add to ALL THREE files — partial translations cause this                            |

## Quick Health Check

```bash
# Backend reachable?
curl -s http://localhost:18083/api/v1/public/login -X POST \
  -H 'content-type: application/json' \
  -d '{"username":"admin","password":"public"}' | jq .

# MQTT TCP listening?
nc -zv localhost 1883

# Frontend dev server up?
curl -s http://localhost:3039 -o /dev/null -w '%{http_code}\n'
```

## Gotchas

- **`packet_id` must be assigned for QoS 1/2 publishes** — missing assignment caused the server panic fixed in `2f10d8d`. Always set before passing to the engine.
- **ReDB write lock is exclusive** — only one `WriteTransaction` at a time. Long-held write txns block all writers.
- **DashMap iteration across `.await` deadlocks** — collect keys first, drop the iter, then await.
- **Token refresh races** — only the axios interceptor handles 401. Don't add manual refresh in services.
- **Frontend uses `yarn`, not `npm install`** — using `npm install` will rewrite lockfile and may break CI.
- **`Cargo.lock` IS committed** — don't `.gitignore` it; the broker is a binary and needs reproducible deps.
