# Changelog

All notable changes to CoreMQ are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

**Lockstep versioning.** The broker (`server/coremq-server/Cargo.toml`) and the dashboard (`client/package.json`) share a single project version and are released together. This file is the canonical changelog for that single version stream. Entries may tag the affected side with `server:` / `client:` / `repo:` where useful; an unprefixed entry applies project-wide.

## [Unreleased]

### Added

- Stress test for MQTT QoS handling.
- `repo:` `.claude/skills/` — task-specific playbooks for code review, verification, debugging, scaffolding new backend features, and scaffolding new dashboard pages.
- `repo:` `.github/workflows/claude.yml` — `@claude` mention handler for PRs, issues, and reviews.
- `repo:` `.github/workflows/claude-code-review.yml` — auto PR review with a 4000-line cost guard, `.rs` + TS/JS file context, and tailored CoreMQ system prompt.
- `repo:` `AGENTS.md`, `CONTRIBUTING.md`, `CHANGELOG.md` at project root.

### Changed

- `repo:` Aligned `server/coremq-server/Cargo.toml` and `client/package.json` to a single shared project version (`0.1.0`). The dashboard was previously at `3.0.0` — that number was template-inherited and did not reflect project history.

### Fixed

- `server:` Assign `packet_id` for QoS 1/2 publishes to prevent server panic ([#5](../../pull/5)).

## Conventions

When adding entries:

- Append under the `## [Unreleased]` section. Never modify already-released version sections — they are immutable.
- Use the subsections **Breaking Changes**, **Added**, **Changed**, **Fixed**, **Removed** in that order. Skip subsections that have no entries.
- One entry per change, written as a complete sentence. Lead with the side tag (`server:` / `client:` / `repo:`) when the scope is non-obvious.
- Link the PR or issue at the end where applicable: `… ([#42](../../pull/42))`.
- For external contributions, attribute: `… ([#42](../../pull/42) by [@username](https://github.com/username))`.

## Releasing

CoreMQ is shipped as a binary + a static dashboard, not as published packages. Both share a single version and release together (lockstep).

1. Bump **both** version fields to the same value:
   - `server/coremq-server/Cargo.toml` → `[package].version`
   - `client/package.json` → `version`
2. Move `[Unreleased]` entries into a new `## [<version>] — YYYY-MM-DD` section. Leave a fresh empty `[Unreleased]` block above it.
3. Commit with `chore(release): v<version>`.
4. Tag with `git tag v<version>` (e.g. `v0.2.0`).
5. Push branch + tags: `git push && git push --tags`.

No major releases. Versioning is patch (fixes + additive features) and minor (API-breaking changes).

If the two manifests drift out of sync, the `Cargo.toml` value is authoritative — bump `package.json` to match before cutting any release.
