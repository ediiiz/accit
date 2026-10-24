# Frontend Agent Instructions

**This project uses [bun](https://bun.sh/) as its JavaScript runtime and package manager.**

Use this file for work in `apps/web`.

The Portal is one prerendered route. Its API (`/api/status`, `/api/unlock`, `/api/revoke`) lives in `crates/` (`docs/spec.md` section 8); keep the TypeScript types in `#lib` in step with it by hand.

If adding, reading, validating, or renaming env vars, also follow `packages/env/AGENTS.md`. Env schemas live there only; import shared env exports instead of making app-local duplicates.

For any UI work, also read `apps/web/DESIGN.md`.

## UI copy

Applies to every user-visible string (`messages/*.json`, `aria-label`, `title`, placeholders).

- Minimum words. A label beats a sentence; a heading needs no description unless it adds a fact.
- Screen copy comes from `docs/spec.md` section 9. No subtext, no infrastructure words, no reassurance or marketing tone.
- Descriptions, hints, empty states: one short line, only when they tell the user something new or what to do next.
- Buttons: verb + object ("Enable access").
- When in doubt, delete the string.

Before adding or changing behaviour, read `apps/web/TESTING.md`: tests come first.

<!-- ShadCN-Svelte:BEGIN -->

## shadcn-svelte

For ANY question regarding **shadcn-svelte UI components**, load the `shadcn-svelte` skill. It carries project context, component docs, and usage examples.

To install components, use:

```bash
bun x shadcn-svelte@latest add -y -o ${ComponentName}
```

**Default UI policy for this repo:**

- Always prefer composing UI from existing shadcn-svelte components in `apps/web/src/lib/components/ui/`.
- Before building custom UI, check whether an existing shadcn component or subcomponent already covers the need.
- If a matching shadcn component does not exist yet, install/add the canonical shadcn component first when one exists upstream.
- If no upstream shadcn component exists for the pattern, build a local reusable component instead of duplicating ad-hoc styled markup across routes.
- When a shadcn component supports `child`/render snippet composition, prefer that over replacing it with raw custom elements.
- Raw HTML should be limited to semantic structure and content inside composed shadcn components, not used as a replacement for available shadcn surfaces, controls, dialogs, navigation, or form primitives.

For charts, load the `layerchart` skill instead — the shadcn-svelte `Chart.*` components are built on LayerChart.

<!-- ShadCN-Svelte:END -->

<!-- Svelte-MCP:BEGIN -->

## Svelte MCP server

The `svelte` MCP server (configured in `.mcp.json`) gives you comprehensive Svelte 5 and SvelteKit documentation. For idiomatic patterns and repo conventions, load the `sveltekit-best-practices` skill as well.

### Available MCP tools

1. `list-sections` — call this FIRST to discover available documentation sections. Returns titles, use_cases, and paths. When asked about Svelte or SvelteKit topics, ALWAYS use this tool at the start of the chat to find relevant sections.
2. `get-documentation` — retrieves full documentation content for specific sections. After `list-sections`, analyze the returned sections (especially `use_cases`) and fetch ALL sections relevant to the task.
3. `svelte-autofixer` — analyzes Svelte code and returns issues and suggestions. You MUST use this whenever writing Svelte code before sending it to the user. Keep calling it until no issues or suggestions are returned.
4. `playground-link` — generates a Svelte Playground link. Ask the user first, and NEVER call it if the code was written to files in their project.

<!-- Svelte-MCP:END -->

<!-- Data-Layer:BEGIN -->

## Data

- Plain `fetch` against the same origin; no client library. Every request sends `credentials: "same-origin"`; mutations send the CSRF token from `/api/status` and `Content-Type: application/json`.
- Status is fetched on load, on focus and immediately before Unlock. Nothing is cached offline.
- The only persisted value is the Address of the last successful Unlock (`localStorage`), used for the "connection changed" state.

<!-- Data-Layer:END -->

<!-- Playwright-Agent-CLI:BEGIN -->

## Browser verification

Use the [Playwright Agent CLI](https://playwright.dev/agent-cli/introduction) to drive a real browser. It replaces the `chrome-devtools` MCP server: plain shell commands instead of a loaded tool schema, a persistent daemon instead of per-call browser startup, and ref-based accessibility snapshots instead of screenshots — far cheaper in tokens.

It is a dev dependency (`@playwright/cli`). Run it from `apps/web`:

```bash
bun run playwright-cli --help            # full command list
bun run playwright-cli --help <command>  # flags for one command
```

Before a browser task, read the skill the package ships. `--help` prints its path on the first line (`playwright-core/lib/tools/skills/playwright-cli/SKILL.md`).

Typical loop against the dev server:

```bash
bun run dev &                                         # http://localhost:5173
bun run playwright-cli open http://localhost:5173
bun run playwright-cli snapshot                       # writes an accessibility-tree snapshot file with element refs
bun run playwright-cli click <ref>
bun run playwright-cli console error                  # console errors only
bun run playwright-cli close
```

Rules:

- Headless by default. Pass `--headed` only when the user wants to watch.
- Target elements by the `ref` returned from `snapshot` or `find`. Do not guess CSS selectors.
- Use `--json` for parsable output and `--raw` for the bare value.
- Always `close` when done. Use `kill-all` to clear stale or zombie sessions.
- To skip a login flow, reuse saved storage state: `state-save <file>` once, then `state-load <file>`.
- `requests` / `request <index>` / `console` replace the old devtools panels for network and console inspection.
- Every command writes snapshots, console logs, and other artifacts to `apps/web/.playwright-cli/` (gitignored). Read those files instead of re-running commands.

Browser automation verifies behavior. It does not replace tests. If user-visible behavior changed, still add or update a Playwright test — see `TESTING.md`.

<!-- Playwright-Agent-CLI:END -->
