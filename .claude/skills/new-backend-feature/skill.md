---
name: new-backend-feature
description: Scaffold a new admin feature in the Rust broker following the AdminCommand → Engine → Service → Controller → Route pattern. TRIGGER when user asks to add, create, or scaffold a new backend feature, REST endpoint, or admin operation.
---

# Scaffold New Backend Feature (CoreMQ)

Every admin operation in CoreMQ flows through the same 7-step wiring. Skipping any step causes hard-to-find bugs (controller hangs, command never runs, route 404s).

Read `.claude/skills.md` "Engine Command Pattern" before starting. Follow Rust conventions: `/* */` block comments only, snake_case functions, PascalCase types.

## Before Scaffolding

1. Check `server/coremq-server/src/api/router.rs` — does a similar route already exist?
2. Check `server/coremq-server/src/engine/commands.rs` — could you extend an existing variant instead of adding a new one?
3. Decide: does this need a oneshot reply (synchronous) or fire-and-forget (rare)? Default: oneshot.

## The 7 Steps

1. **Model** — `server/coremq-server/src/models/api/<feature>.rs`. Request/response structs with `Serialize`/`Deserialize`. Add `pub mod <feature>;` to `models/api/mod.rs`.
2. **Command variant** — Add to `AdminCommand` enum in `server/coremq-server/src/engine/commands.rs`. Include the request payload + a `reply: oneshot::Sender<Result<Resp, ApiError>>`.
3. **Service method** — Add to the relevant service in `server/coremq-server/src/services/`. Pure logic, no channels. Returns `Result<Resp, ApiError>`.
4. **Engine handler** — Add a `match` arm in `engine/engine.rs` `run()` loop. Calls the service, then `let _ = reply.send(result);`. **Always reply, even on error.** A dropped sender = hung controller.
5. **Controller** — `server/coremq-server/src/api/controllers/<feature>.rs`. Creates `oneshot::channel()`, sends the `AdminCommand`, awaits the reply, wraps in `ApiResponse<T>`. Add `pub mod <feature>;` to `controllers/mod.rs`.
6. **Route** — Register in `api/router.rs` with method + path + auth layer. Follow the `/api/v1/<resource>` convention.
7. **Auth policy** — If the route is admin-protected, add the Casbin policy entry. Check existing routes for the pattern.

## Verify

1. `cargo check -p coremq-server` — compiles?
2. `cargo clippy -p coremq-server --all-targets -- -D warnings` — clean?
3. Run the broker, hit the new endpoint with `curl`:
   ```bash
   TOKEN=$(curl -s -X POST http://localhost:18083/api/v1/public/login \
     -H 'content-type: application/json' \
     -d '{"username":"admin","password":"public"}' | jq -r .data.access_token)

   curl -s http://localhost:18083/api/v1/<your-route> \
     -H "authorization: Bearer $TOKEN" | jq .
   ```

## Gotchas

- **Never drop the `oneshot::Sender` without sending.** Forgetting `reply.send(...)` in an error path = controller hangs until timeout.
- **The engine `run()` loop is single-threaded.** Don't call long-blocking work from a match arm — spawn a task or call into a service that uses a worker pool.
- **`AdminCommand` variants must be exhaustively matched** — adding a variant without an arm is a compile error, which is the bug you want.
- **Frontend wiring is separate** — after the backend is done, see `.claude/skills/web-page/skill.md` for the React side (service function + Zustand store + section view).
