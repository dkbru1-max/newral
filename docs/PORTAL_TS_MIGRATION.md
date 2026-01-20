# Portal TS + shadcn migration plan

## Goals
- Preserve current UX, routes, and layout while introducing shadcn base components.
- Keep visual identity and custom CSS intact, no default shadcn theme swap.
- Reduce regression risk with incremental, section-by-section changes.
- Maintain Docker/local-registry workflow and minimal build changes.

## Status
- TS/TSX migration complete.
- shadcn scaffold ready (Tailwind config, components.json, utils).
- Projects + Project Detail migrated to shadcn base components.

## Phased plan
### Phase 1 (done)
- Base components: Button, Input, Card, Table (no visual reset).
- Projects + Project Detail UI migration.
- Docs updated (PORTAL + run context).

### Phase 2 (done)
✔ Tasks page: table + filters + pagination controls using shadcn base components.
✔ Log Center: table + filter chips; preserve current styles and log colors.
✔ Audit: ensure table scroll + layout remain unchanged.

### Phase 3
✔ Dashboard + KPI cards: moved to Card; custom metrics layout preserved.\n+✔ Agents list + Agent detail: table layout and cards migrated.\n+✔ Route transitions and breadcrumbs preserved.

### Phase 4 (done)
✔ Settings + Integrations + System Health: migrated cards + buttons to shadcn base components.
✔ Remaining native inputs kept where appropriate (tabs/selects) to preserve layout.

### Phase 5 (done)
✔ Replaced native selects with shadcn Select (Radix).
✔ Settings tabs migrated to shadcn Tabs while keeping existing layout styles.

## Regression checklist
- Routes: /, /projects/:name, /tasks, /agents, /log-center.
- BPSW controls: sync/start/pause/stop actions.
- Live summary SSE + project log sorting.
- Filters/search in Tasks and Project Detail.
- Theme toggle: shadcn base components must respect CSS variables.

## Build
- Use local registry compose override.
- `docker compose -f docker-compose.yml -f docker-compose.local-registry.yml up -d --build`
