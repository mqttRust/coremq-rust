# Contributing to CoreMQ

Thanks for your interest in CoreMQ. This document covers how to set up your environment, what we expect from issues and PRs, and the conventions we use across the codebase.

If you are an AI agent, read `AGENTS.md` first — it is the authoritative rules-of-engagement file. This document is for humans contributing to the repo.

## Local Development

### Prerequisites

- Rust toolchain (latest stable). `make setup` will install it via `rustup` if missing.
- Node.js 18+ and `yarn`. The frontend uses yarn (not npm); using `npm install` will rewrite the lockfile.
- macOS or Linux. Windows is not actively tested.

### Setup

```bash
git clone <fork-or-repo>
cd coremq-rust
make setup        # installs Rust if needed, builds the broker, installs frontend deps
```

### Running

```bash
make dev          # broker + dashboard concurrently
make server       # broker only (cargo run -p coremq-server)
make client       # dashboard only (yarn dev)
```

### Default endpoints

| Service       | URL / Port                    |
| ------------- | ----------------------------- |
| MQTT TCP      | `localhost:1883`              |
| MQTT TLS      | `localhost:8883`              |
| MQTT WebSocket| `ws://localhost:8083`         |
| REST API      | `http://localhost:18083`      |
| Dashboard     | `http://localhost:3039`       |

Default login for the dashboard / REST API: `admin` / `public`. These are local-dev defaults only — never deploy them.

## Reporting Issues

Before opening an issue:

1. Search existing issues — your problem may already be tracked.
2. Reproduce on the latest `main`. Stale local builds account for a surprising fraction of reports.
3. Run the relevant verification (`cargo clippy`, `npx eslint`) — clippy / eslint catch most "is this a bug?" cases.

A good bug report includes:

- **What you expected** and **what actually happened**.
- **Reproduction steps** — exact commands, MQTT client used (mosquitto_pub / mqttx / etc.), QoS level, payload, broker config.
- **Logs** — broker stdout, dashboard browser console, any panics or stack traces.
- **Environment** — OS, Rust version (`rustc --version`), Node version (`node --version`), whether you are running `make dev` or just one component.

A good feature request includes:

- The problem you are solving (not the solution you have in mind).
- Why existing functionality doesn't cover it.
- What "done" looks like — concrete acceptance criteria.

Issues that don't meet this bar may be closed without a reply. Reopen after you've added the missing context — we don't hold it against you.

## Submitting Pull Requests

### Before you open a PR

1. **Branch** off `main`. Use the established naming pattern:
   - `fix-<short-slug>` for bug fixes
   - `feat-<short-slug>` for new features
   - `refactor-<short-slug>` for non-behavioral cleanup
   - `chore-<short-slug>` for tooling, deps, docs
2. **Match conventions**. The full list lives in `.claude/skills.md`. The most-violated rules:
   - Rust uses `/* */` block comments only — no `//`, no `///`.
   - TypeScript uses `type` over `interface`, `export default function` for components, JSDoc-only comments, single quotes.
   - Frontend pages are thin wrappers; logic lives in `sections/<feature>/<feature>_view.tsx`.
   - Zustand stores split `State` and `Actions`, share an `initialState`, and expose `reset()`.
   - Theme tokens via `sx`, never hardcoded colors (the one exception is drawer `bgcolor: '#131825'`).
   - Every user-facing string is `t('key')` and the key must exist in `client/src/i18n/{en,ko,uz}.json`.
3. **Run the verification suite** (full output, no tail; fix everything):

   ```bash
   # Rust
   cargo check -p coremq-server
   cargo clippy -p coremq-server --all-targets -- -D warnings
   cargo fmt -p coremq-server -- --check
   cargo test -p coremq-server

   # Frontend
   cd client
   npx eslint "src/**/*.{js,jsx,ts,tsx}"
   npx prettier --check "src/**/*.{ts,tsx}"
   yarn build
   ```

4. **Update `CHANGELOG.md`** under `[Unreleased]`. Don't modify already-released version sections.

### PR description

Include:

- **Summary** — 2-3 sentences on what changed and why.
- **Test plan** — what you ran and what to look for in review. Concrete commands beat narrative.
- **Linked issue** — `fixes #<n>` or `closes #<n>` if applicable. The merge will auto-close.
- **Screenshots** — for any visible dashboard change.

### What to expect

- The `claude-code-review.yml` workflow will leave an automated review comment within a minute or two of the push. It is not a substitute for human review, but it catches obvious issues fast.
- A maintainer will review. Tag `@claude` in a comment for targeted automated follow-ups.
- We may ask for changes. Push to the same branch — no force-push to `main`, ever.

## Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/). Format: `<type>: <short imperative summary>`.

| Type        | Use for                                            |
| ----------- | -------------------------------------------------- |
| `feat`      | New feature                                        |
| `fix`       | Bug fix                                            |
| `refactor`  | Non-behavioral cleanup                             |
| `chore`     | Tooling, dependencies, build, docs                 |
| `docs`      | Documentation only                                 |
| `test`      | Adding or fixing tests                             |
| `perf`      | Performance improvement without behavior change    |

Body (optional) explains the *why*, not the *what*. The diff already shows the *what*. If a commit closes an issue or PR, append `fixes #<n>` or `closes #<n>` on its own line.

Example:

```
fix(server): assign packet_id for QoS 1/2 publishes

The QoS 1/2 publish path was passing the packet_id field through unset,
which crashed the broker when the in-flight tracker tried to look it up.
Assigning at the publish boundary keeps the broker side of the protocol
correct without changing client APIs.

fixes #4
```

## Code Style

We do not relitigate style in code review. The lint and formatter are the source of truth.

- Rust: `cargo fmt`, `cargo clippy`. CI fails on warnings.
- Frontend: `npx prettier --write` (run via `make fmt`), `npx eslint --fix` (run via `make fix`).

For convention details that linters cannot enforce, see:

- `.claude/skills.md` — the canonical CoreMQ conventions doc (Rust + React, API, theme, i18n).
- `.claude/skills/code-review/references/gotchas.md` — recurring pitfalls (`unwrap()` on `Option::None`, dropped oneshots, missing `reset()`, etc.).

## Testing

- New behavior should come with a test. If a bug fix isn't covered by an existing test, add one — that's the bar for "fixed."
- Rust tests live alongside their module (`#[cfg(test)] mod tests`) or in `tests/` for integration tests. Run with `cargo test -p coremq-server` or `cargo test --test <name>` for a specific integration test.
- For QoS / stress regressions, follow the pattern in the recently added stress QoS test.
- Frontend tests are not yet wired up. If you want to add a test framework, open an issue first to align on tooling.

## Security

If you find a security issue, do not open a public issue. Email the maintainers (or DM on the project's primary channel) with reproduction steps. We'll coordinate on disclosure.

## License

By contributing, you agree your contributions are licensed under the same license as this repository.
