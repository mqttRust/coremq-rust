---
name: verify
description: Run the full CoreMQ verification suite — cargo check/clippy/test on the Rust broker, lint and prettier on the React client. TRIGGER when user asks to verify, test, check, or validate changes before committing.
---

# CoreMQ Verification

Run all checks in order. Stop on first failure. Report a one-line PASS/FAIL per step.

## Steps

### Backend (`server/coremq-server`)

1. **Type check**: `cargo check -p coremq-server`
2. **Lint**: `cargo clippy -p coremq-server --all-targets -- -D warnings`
3. **Format check**: `cargo fmt -p coremq-server -- --check`
4. **Tests**: `cargo test -p coremq-server`

### Frontend (`client`)

5. **Lint**: `cd client && npx eslint "src/**/*.{js,jsx,ts,tsx}"`
6. **Format check**: `cd client && npx prettier --check "src/**/*.{ts,tsx}"`
7. **Type + build**: `cd client && yarn build`

### Final

8. **Git status**: ensure no uncommitted untracked artifacts (e.g., `target/`, `dist/`, `node_modules/`)

## Auto-fix

- If `cargo fmt --check` fails, auto-fix with `cargo fmt -p coremq-server`.
- If prettier check fails, auto-fix with `cd client && npx prettier --write "src/**/*.{ts,tsx}"`.
- If eslint fails on auto-fixable rules, run `cd client && npm run lint:fix`.

## Output

Report one line per step: `PASS` or `FAIL` with a short error summary. After all steps, print a final summary.
