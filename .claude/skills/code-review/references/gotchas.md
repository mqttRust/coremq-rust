# Gotchas — Common Mistakes in CoreMQ

## Comment style

### Rust (`server/coremq-server/src/**/*.rs`)

- **Block comments only** — Use `/* */`, never `//` line comments. Project rule.
- Multi-line:
    ```rust
    /*
     * Assigns the next packet ID for QoS 1/2 publishes.
     * Returns None if the packet ID counter has wrapped.
     */
    ```
- Do NOT use `// inline` or doc comments (`///`). Convert to `/* */` above the line, or delete if redundant.

### TypeScript (`client/src/**/*.{ts,tsx}`)

- **JSDoc only** — `/** */`, never `//` or `/* */` (non-JSDoc). Project rule.
- Single-line: `/** Disconnect the active session. */` is fine for one-liners.
- Do NOT use `// inline` or `// end-of-line` comments.

## Rust backend

- **`unwrap()` on `Option::None` panics** — especially around `packet_id` for QoS 1/2 publishes. Always use `ok_or(...)` or `if let Some(...)`. The `fix-panic_id-none-server-panic-bug` branch was created for exactly this class of bug.
- **AdminCommand wiring** — every new command needs all 7 steps wired (model → command variant → service method → engine match arm → controller → route → mod.rs). Forgetting the engine match arm causes the controller's `oneshot::Receiver` to hang forever.
- **Always reply on the oneshot** — the engine's `match` arm MUST send a response back through the `reply` sender, even on error paths. A dropped sender causes the controller to receive `RecvError` and return 500.
- **No blocking calls in async code** — `std::fs`, `std::thread::sleep`, `std::sync::Mutex` (long held) all block the Tokio runtime. Use `tokio::fs`, `tokio::time::sleep`, `tokio::sync::Mutex`.
- **Bounded channels for backpressure** — Use `mpsc::channel(N)` not `unbounded_channel()` for hot paths (publish, connect). Unbounded channels can OOM under load.
- **DashMap iteration locks shards** — never hold a `DashMap::iter()` guard across an `.await` point. Collect keys first, then process.
- **ReDB transactions** — `WriteTransaction` must be committed or it silently rolls back. Always end with `txn.commit()?`.

## Frontend (React + TypeScript)

- **`type` over `interface`** — Always. The only exception is `extend-theme-types.d.ts` (MUI module augmentation requires `interface`).
- **`export default function`** — All page and section components.
- **No raw colors** — Never hardcode hex/rgb in `sx`. Use theme tokens: `sx={{ bgcolor: 'background.paper' }}` not `sx={{ bgcolor: '#131825' }}`.
- **Responsive padding always** — `sx={{ p: { xs: 2, sm: 3 } }}`, never `sx={{ p: 3 }}`.
- **Pages are thin** — `pages/foo.tsx` should ONLY import and render the section view. Logic belongs in `sections/foo/foo_view.tsx`.
- **i18n is mandatory** — Any user-facing string must be `t('key')`. Adding a key requires updates to ALL THREE: `118n/en.json`, `118n/ko.json`, `118n/uz.json`. Missing one is a must-fix.

## Zustand stores

- **`reset()` is required** — every store must have `reset: () => set(initialState)` for logout cleanup. Missing reset = stale data after re-login.
- **Separate State and Actions types** — State is data only, Actions is functions only. Don't merge them.
- **Shared `initialState` object** — defined once, used as both default and for `reset()`. Don't duplicate the literals.
- **Selectors over full subscription** — `useFooStore(s => s.items)`, not `const { items } = useFooStore()`. The latter re-renders on every store change.

## API layer

- **Always wrap in `ApiResponse<T>`** — services return `ApiResponse<T>`, not raw `T`. The wrapper carries `success`, `data`, `error_message`.
- **Token refresh is automatic** — don't manually handle 401 in services; the axios interceptor does it. Adding manual refresh logic causes double-refresh races.
- **Bearer token from cookies** — never read `localStorage` for auth. The interceptor attaches the bearer from cookie helpers.

## Theme & MUI

- **Drawers use `bgcolor: '#131825'` literally** — this is the one documented exception. Everywhere else: theme tokens.
- **`JetBrains Mono Variable` for monospace data** — client IDs, topic names, ports. Don't use the default font.

## Build / Tooling

- **`Cargo.lock` is committed** — for reproducible broker builds. Don't add it to `.gitignore`.
- **Frontend uses `yarn`** (per Makefile) — `yarn dev`, `yarn install`. Don't switch to `npm install` casually; it produces a different lockfile and CI may break.
- **Default ports** — MQTT TCP `1883`, MQTT TLS `8883`, WS `8083`, REST `18083`, frontend dev `3039`. Don't hardcode different values in tests.
