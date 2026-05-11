---
name: web-page
description: Scaffold a new page or feature section in the CoreMQ React dashboard following the page→view→table/drawer + Zustand store + service pattern. TRIGGER when user asks to create, add, or build a new page, route, or dashboard section in the client.
---

# Scaffold New Frontend Page (CoreMQ)

Create new pages and sections for `client/src/` following CoreMQ conventions.

Read `.claude/skills.md` (Frontend Architecture, State Management, API Layer, Theme & Styling, i18n) before starting. Adhere to: `type` over `interface`, `export default function`, JSDoc-only comments, single quotes, theme tokens via `sx`, responsive padding `{ xs, sm }`.

## Before Scaffolding

1. Check existing routes in `client/src/routes/` — pick the right place to register.
2. Check existing sections in `client/src/sections/` — find the closest pattern (e.g. `topics/`, `session/`).
3. Read `.claude/skills.md` API Layer to confirm the backend endpoint exists. If not, scaffold the backend first via `.claude/skills/new-backend-feature/`.

## Layout

For a feature called `feature`, you create:

```
client/src/
├── pages/feature.tsx                       — thin wrapper
├── sections/feature/
│   ├── feature_view.tsx                    — orchestrator
│   ├── feature_table.tsx                   — table component (optional)
│   └── feature_drawer.tsx                  — drawer/dialog (optional)
├── stores/feature_store.ts                 — Zustand store
├── services/feature.ts                     — axios calls
├── types/feature.ts                        — TS types
└── routes/                                  — register the route
```

## Steps

1. **Types** — `types/feature.ts`. Use `type` not `interface`. All API responses go through `ApiResponse<T>` (defined in `types/api_response.ts`).
2. **Service** — `services/feature.ts`. Wrap axios calls; return `ApiResponse<T>`. Don't handle 401 manually — the interceptor does it.
3. **Store** — `stores/feature_store.ts`. Split `FeatureState` (data) and `FeatureActions` (functions). Define `initialState`. Export `useFeatureStore`, selectors, and ensure `reset: () => set(initialState)`. Add to the barrel export `stores/index.ts`.
4. **Page** — `pages/feature.tsx`. Three lines: import view, default-export a function rendering `<FeatureView />`.
5. **View** — `sections/feature/feature_view.tsx`. Subscribe to the store via selectors, fire `useEffect(() => fetch(), [])`, render layout with responsive padding `sx={{ p: { xs: 2, sm: 3 } }}`, error alert, loading/empty/data states, and any drawer.
6. **Table / Drawer** — Split out for reuse. Tables receive data via props and emit `onAction(item)`. Drawers manage form state internally and receive `open`/`onClose`/initial data.
7. **Route** — Register in `client/src/routes/`.
8. **Sidebar** — Add the entry to the dashboard navigation in `client/src/layouts/`.
9. **i18n** — Add ALL user-facing strings as keys in `client/src/118n/en.json` AND `ko.json` AND `uz.json`. Missing one is a bug.

## Verify

```bash
cd client
npx eslint "src/sections/feature/**" "src/stores/feature_store.ts" "src/services/feature.ts"
npx prettier --check "src/sections/feature/**/*.{ts,tsx}"
yarn dev    # then open http://localhost:3039 and walk the feature
```

## Gotchas

- **Hardcoded colors** — never `sx={{ bgcolor: '#abc123' }}`. Always theme tokens (`background.paper`, `text.primary`, `divider`). The one exception is drawers using `bgcolor: '#131825'` (documented in `.claude/skills.md`).
- **Pages must be thin** — three lines: import + default export + render. Logic in `_view`.
- **Selector subscriptions** — `useFeatureStore(s => s.items)`, never destructure the whole store; otherwise the component re-renders on every store mutation.
- **`reset()` is required** — without it, logout leaves stale data in the next user's session.
- **i18n triplet rule** — every key must exist in en/ko/uz. Missing = raw key shown in UI for one language.
- **`JetBrains Mono Variable`** for monospace data (client IDs, topic strings, port numbers) — don't use the default font.
