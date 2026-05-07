# feat: add WebSocket metrics endpoint for real-time broker monitoring

**Branch:** `feat/ws-metrics-endpoint` → `main`

## Summary

- Add `GET /api/v1/ws/metrics` WebSocket endpoint that pushes a JSON metrics frame every second
- Each frame includes process memory (MB), CPU %, connected client count, and active topics
- New `GetStats` admin command wired through the engine's command channel for non-blocking stat queries

## Changes

| File | What Changed |
|------|-------------|
| `src/api/controllers/metrics.rs` | New WS handler; streams `MetricsFrame` every 1 s via `sysinfo` + engine query |
| `src/api/router.rs` | Registered `/api/v1/ws/metrics` route |
| `src/engine/commands.rs` | Added `GetStats(oneshot::Sender<(usize, Vec<TopicInfo>)>)` variant |
| `src/engine/engine.rs` | Handles `GetStats` — delegates to `client_service` and `topic_service` |
| `src/services/session.rs` | Added `client_count() -> usize` helper |
| `Cargo.toml` / `Cargo.lock` | Added `sysinfo` dependency for process-level metrics |

## Test Plan

- [ ] Connect a WebSocket client to `ws://<host>/api/v1/ws/metrics`
- [ ] Verify frames arrive at ~1 s intervals
- [ ] Confirm `client_count` changes as MQTT clients connect/disconnect
- [ ] Confirm `memory_mb` and `cpu_percent` reflect real process stats
- [ ] Verify connection closes cleanly when the client disconnects

## Related Issue

Closes #<!-- issue number -->
