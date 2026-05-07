# [Feature] WebSocket Metrics Endpoint for Real-Time Broker Monitoring

## Summary

Add a real-time metrics streaming endpoint to the CoreMQ broker via WebSocket.
This allows dashboards, monitoring tools, or operators to observe live broker health
without polling REST endpoints.

## Motivation

As the broker scales to handle more clients and topics, operators need a low-latency
way to observe system health (memory, CPU, active clients, active topics) without
adding overhead through repeated HTTP polling.

## Solution

A new WebSocket endpoint `GET /api/v1/ws/metrics` is implemented on the
`feat/ws-metrics-endpoint` branch. The server pushes a JSON frame **every 1 second**
containing:

| Field          | Type     | Description                          |
|----------------|----------|--------------------------------------|
| `timestamp`    | `string` | RFC 3339 server time                 |
| `memory_mb`    | `f64`    | Process RSS memory in MB             |
| `cpu_percent`  | `f32`    | Process CPU usage %                  |
| `client_count` | `usize`  | Number of connected MQTT clients     |
| `topics`       | `array`  | Active topics with subscriber counts |

## Example Frame

```json
{
  "timestamp": "2026-05-07T14:00:00+00:00",
  "memory_mb": 42.1,
  "cpu_percent": 0.8,
  "client_count": 317,
  "topics": [
    { "name": "sensor/data", "subscriber_count": 12 }
  ]
}
```

## Files Changed

| File | Description |
|------|-------------|
| `server/coremq-server/src/api/controllers/metrics.rs` | New WS handler and 1-second streaming loop |
| `server/coremq-server/src/api/router.rs` | Register `/api/v1/ws/metrics` route |
| `server/coremq-server/src/engine/commands.rs` | New `GetStats` admin command variant |
| `server/coremq-server/src/engine/engine.rs` | Handle `GetStats`, return client count + topics |
| `server/coremq-server/src/services/session.rs` | Add `client_count()` helper method |
| `Cargo.toml` / `Cargo.lock` | Add `sysinfo` dependency for process-level metrics |

## Labels

`enhancement`, `observability`
