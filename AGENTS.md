# CoreMQ — Agent Rules

Project-wide rules for any AI coding agent working in this repo (Claude Code, Cursor, Codex, etc.). The canonical project conventions live in `.claude/skills.md`; the per-task playbooks live in `.claude/skills/<name>/skill.md`. This file is the rules-of-engagement layer on top of those.

## Conversational Style

- Keep answers short and concise.
- No emojis in commits, issues, PR comments, or code.
- No fluff, cheerful filler, or congratulatory text.
- Technical prose only; be kind but direct (e.g. "Thanks @user" not "Thanks so much @user!").

## Code Quality

### Universal

- No backwards-compatibility shims unless the user explicitly asks.
- Always ask before removing functionality or code that appears intentional, even if it looks dead.
- Never disable lints or hide warnings to make a build pass — fix the underlying issue, or surface it.
- Never hardcode default ports (`1883`, `8883`, `8083`, `18083`, `3039`) inside business logic. Use the broker config / Vite env / shared constants.
- No secrets in code. Default `admin/public` is for local dev only.

### Rust (`server/coremq-server/`)

- **Block comments only** — `/* */`, never `//` or `///`. Project rule.
- No `.unwrap()` on `Option::None` paths — particularly around `packet_id` for QoS 1/2. Use `ok_or(...)` / `if let Some(...)`. The `2f10d8d` commit exists because this rule was violated.
- No blocking syscalls in `async` functions (`std::fs`, `std::thread::sleep`, long-held `std::sync::Mutex`). Use `tokio::*` equivalents.
- Bounded channels (`mpsc::channel(N)`) on hot paths (publish, connect). `unbounded_channel()` is reserved for control-plane signals only.
- Every `AdminCommand` match arm in the engine MUST send on its `oneshot::Sender`, including error paths. A dropped sender hangs the controller until timeout.
- Never hold a `DashMap::iter()` guard across an `.await` point. Collect keys, drop the iterator, then await.
- `WriteTransaction` must always end in `txn.commit()?`. Uncommitted txns silently roll back.

### TypeScript / React (`client/`)

- `type` over `interface`. The only exception is `extend-theme-types.d.ts` (MUI module augmentation requires `interface`).
- `export default function ComponentName()` for all pages and section components.
- JSDoc-only comments (`/** */`). No `//`, no non-JSDoc `/* */`.
- Single quotes everywhere (enforced by prettier).
- No `any`. Check `node_modules` for upstream types instead of guessing.
- No hardcoded colors in `sx` — always theme tokens. Documented exception: drawer `bgcolor: '#131825'`.
- Responsive padding always: `sx={{ p: { xs: 2, sm: 3 } }}`.
- Pages are thin wrappers (import view + render). Logic lives in `sections/<feature>/<feature>_view.tsx`.
- Zustand stores: split `State` (data) and `Actions` (functions), shared `initialState` constant, mandatory `reset: () => set(initialState)` for logout cleanup. Subscribe via selectors, not full destructure.
- Never call `localStorage` for auth — the axios interceptor owns token refresh.
- Every user-facing string is `t('key')`, and every key must exist in **all three** of `client/src/118n/{en,ko,uz}.json`. Missing one is a bug, not a nit.

### Don't touch without coordination

- `Cargo.lock` — committed on purpose (binary crate). Don't `.gitignore` it.
- `client/yarn.lock` — frontend uses yarn (per Makefile). Don't switch to `npm install`; it produces a different lockfile and CI may break.
- `.claude/skills/code-review/references/gotchas.md` — the canonical pitfall list. When you discover a new gotcha, add an entry there, don't bury it in a PR description.

## Commands

After code changes, run the appropriate verification suite. Get full output, no tail. Fix all errors, warnings, and infos before committing.

| What changed                  | Run                                                                            |
| ----------------------------- | ------------------------------------------------------------------------------ |
| Rust (server)                 | `cargo check -p coremq-server && cargo clippy -p coremq-server --all-targets -- -D warnings && cargo fmt -p coremq-server -- --check` |
| Frontend (client)             | `cd client && npx eslint "src/**/*.{js,jsx,ts,tsx}" && npx prettier --check "src/**/*.{ts,tsx}"` |
| Both                          | Run the full skill: see `.claude/skills/verify/skill.md`                       |
| You wrote/changed Rust tests  | `cargo test -p coremq-server <test_name>` and iterate until green              |
| You wrote/changed integration | Tests live in `tests/`. Run them from the workspace root: `cargo test --test <name>` |

### Don't run

- `make dev` — long-running, holds two servers. Only the user starts this.
- `make server` / `cargo run -p coremq-server` — long-running broker. Same rule.
- `cd client && yarn dev` — long-running. Same rule.

If you need to verify behavior end-to-end, ask the user to start the broker / dev server, then drive REST calls with `curl` or hit the dashboard manually.

### Auto-fix is allowed

- `cargo fmt -p coremq-server` — formatting
- `cd client && npx prettier --write "src/**/*.{ts,tsx}"` — frontend formatting
- `cd client && npm run lint:fix` — eslint auto-fixable rules

### Never commit unless the user asks.

## Skills & Project Guide

Before starting work, consult the relevant skill. Each one is self-contained and references `.claude/skills.md` for deeper convention details.

| Skill                                       | When to use                                                                |
| ------------------------------------------- | -------------------------------------------------------------------------- |
| `.claude/skills/coremq-guide/`              | Project layout, ports, build commands — orientation for new tasks         |
| `.claude/skills/code-review/`               | Reviewing changes against project conventions before committing or pushing |
| `.claude/skills/verify/`                    | Pre-commit / pre-push verification suite (cargo + eslint + prettier)       |
| `.claude/skills/debug-coremq/`              | Diagnosing broker, REST, MQTT, or dashboard issues                         |
| `.claude/skills/new-backend-feature/`       | Scaffolding a new admin feature through the AdminCommand pattern           |
| `.claude/skills/web-page/`                  | Scaffolding a new dashboard page / section / store                         |
| `.claude/skills.md`                         | Canonical CoreMQ conventions (Rust + React, API, theme, i18n)              |

## PR Workflow

- Analyze PRs without pulling locally first (`gh pr view`, `gh pr diff`).
- The `.github/workflows/claude-code-review.yml` workflow auto-reviews every PR push within a 4000-line budget. Do not duplicate that review by hand unless the user asks.
- Tagging `@claude` in a PR or issue comment triggers `.github/workflows/claude.yml` for targeted asks. Use it for follow-ups, not for the initial review.
- If the user approves changes: create a feature branch, pull the PR's branch, rebase on `main`, apply adjustments, run the verify skill, commit, merge into `main`, push, close the PR with a short comment.
- You never open PRs unprompted. Work on a feature branch until it matches user requirements, then merge into `main` and push.

## Issue / PR Comment Hygiene

- Write the full comment to a temp file, then `gh issue comment --body-file` or `gh pr comment --body-file`. Never pass multi-line markdown directly via `--body`.
- Preview the exact comment before posting.
- Post exactly one final comment unless the user asks for multiple.
- If a comment is malformed, delete it immediately, then post one corrected comment.
- Keep comments concise, technical, and in the user's tone.

When closing an issue via commit, include `fixes #<number>` or `closes #<number>` in the commit message.

## Branch Naming

Existing branches follow short, descriptive kebab-case names that hint at the change class (e.g. `fix-panic_id-none-server-panic-bug`, `feat-topic-new`). Match this pattern:

- `fix-<short-slug>` for bug fixes
- `feat-<short-slug>` for new features
- `refactor-<short-slug>` for non-behavioral cleanup
- `chore-<short-slug>` for tooling / docs

## Commit Messages

Existing history uses [Conventional Commits](https://www.conventionalcommits.org/) prefixes:

- `feat:` new feature
- `fix:` bug fix
- `chore:` tooling, deps, docs
- `refactor:` non-behavioral cleanup

Format: `<type>: <short imperative summary>`. Body (optional) explains the *why*, not the *what*. If an issue or PR is closed by the commit, append `fixes #<number>` or `closes #<number>` on its own line.

## **CRITICAL** Git Rules for Parallel Agents **CRITICAL**

Multiple agents may operate on different files in the same worktree simultaneously. Follow these rules without exception.

### Committing

- ONLY commit files YOU changed in THIS session.
- ALWAYS include `fixes #<number>` or `closes #<number>` when there is a related issue or PR.
- NEVER use `git add -A` or `git add .` — they sweep up changes from other agents.
- ALWAYS use `git add <specific-file-paths>` listing only files you modified.
- Before committing, run `git status` and verify only YOUR files are staged.
- Track which files you created / modified / deleted during the session.

### Forbidden Git Operations

These commands can destroy other agents' work:

- `git reset --hard` — destroys uncommitted changes
- `git checkout .` / `git restore .` — destroys uncommitted changes
- `git clean -fd` — deletes untracked files
- `git stash` — stashes ALL changes including other agents' work
- `git add -A` / `git add .` — stages other agents' uncommitted work
- `git commit --no-verify` — bypasses required checks; never allowed

### Safe Workflow

```bash
# 1. Check status first
git status

# 2. Add ONLY your specific files
git add server/coremq-server/src/services/topic.rs
git add client/src/sections/topics/topics_view.tsx

# 3. Commit (Conventional Commits)
git commit -m "fix(server): assign packet_id for QoS 1/2 publishes"

# 4. Push (pull --rebase if needed, NEVER reset/checkout)
git pull --rebase && git push
```

### If Rebase Conflicts Occur

- Resolve conflicts in YOUR files only.
- If a conflict appears in a file you didn't modify, abort and ask the user.
- NEVER force push.

### User override

If user instructions conflict with the rules above, ask for explicit confirmation that the user wants to override. Only then proceed.
