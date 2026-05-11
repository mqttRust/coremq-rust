---
name: code-review
description: Review changed code for CoreMQ project conventions — catches gotchas linters miss like wrong comment style, missing i18n keys, hardcoded colors, AdminCommand wiring mistakes, Zustand store leaks. TRIGGER when user asks to review, check, or audit code they wrote.
---

# CoreMQ Code Review

Review changed files against the project rules in `.claude/skills.md` and the codebase-specific pitfalls in `references/gotchas.md`.

## Steps

1. Run `cargo clippy --all-targets -- -D warnings` (server) and `npm run lint` + `npx prettier --check "src/**/*.{ts,tsx}"` (client) to catch automated issues
2. Read each changed file and check against `.claude/skills.md` rules and `references/gotchas.md`
3. Report findings grouped by severity: **must fix**, **should fix**, **nit**

Focus on things linters cannot catch: missing oneshot reply on an `AdminCommand` arm, blocking syscalls in async code, hardcoded MUI colors instead of theme tokens, i18n keys missing from one of `en.json`/`ko.json`/`uz.json`, Zustand store missing `reset()`, panics from `unwrap()` on `Option::None` (especially around `packet_id`).

## Output

```
[must fix|should fix|nit] file:line — description
```

Fix **must fix** and **should fix** automatically. Ask before fixing nits.

If more than 5 files, spawn a subagent for the review and implement fixes based on its findings.
