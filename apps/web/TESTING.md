# Testing

Three lanes. Push every check down to the cheapest lane that can prove it.

| Lane      | Files                     | Runs in                                   | Speed     | Use for                                                       |
| --------- | ------------------------- | ----------------------------------------- | --------- | ------------------------------------------------------------- |
| unit      | `src/**/*.test.ts`        | Node                                      | ms        | Pure logic: formatting, derivations, mappings.                |
| component | `src/**/*.svelte.test.ts` | Vitest browser mode, headless Chromium    | ~seconds  | One component's contract: props, snippets, interaction, a11y. |
| e2e       | `tests/e2e/**/*.e2e.ts`   | Playwright against a built preview server | ~a minute | A few full user flows. Routing, auth, live backend data.      |

Server rules (validation, counters, ordering, auth gates) are tested in `crates/server/tests` (`scripts/check-rust`). Do not re-prove them in e2e.

## Commands

From `apps/web`:

```bash
bun run test:unit        # unit only
bun run test:component   # component only
bun run test:vitest      # unit + component
bun run test:vitest:watch
bun run test:e2e         # Playwright (needs a running backend, see below)
bun run test:a11y        # e2e tests tagged @a11y
bun run test             # vitest, then e2e
```

From the repo root: `bun run web:test:unit`, `web:test:component`, `web:test:e2e`, `web:test:a11y`.

CI (`.github/workflows/check.yml`, `rust.yml`) runs unit, component and backend tests. e2e is not in CI.

## Policy

Tests first: write the failing test for the new behaviour or bug, then the code that turns it green (`tdd` skill).

When you add or change a page or user-visible component:

1. Move non-trivial logic out of the `.svelte` file into a `#lib/*.ts` module and unit-test it. Examples: `#lib/projects.ts`, `#lib/breadcrumbs.ts`, `#lib/user.ts`.
2. Add a component test for the component's own contract.
3. Touch e2e only if the change adds or breaks a user flow. Extend an existing flow before adding a new file.

New theme token or new text/surface pair: add the pair to `src/lib/theme-contrast.harness.svelte`.

Do not test internal classes or component-local state when a user-facing assertion is possible.

## Unit tests

- Plain Vitest in Node. No DOM, no Svelte.
- Prefer `test.each` tables over near-duplicate tests.
- Paraglide messages (`m.*`) work and return the base locale (English).

## Component tests

- `render()` from `vitest-browser-svelte` is async. Always `await` it.
- Pass props under `props`. Build snippet props with `createRawSnippet` from `svelte`.
- Query from the result (`screen.getByRole(...)`) and assert with `await expect.element(...)`.
- No module mocks (`anti-slop/no-module-mocking`). When a component needs a provider, add a `*.harness.svelte` beside it that mounts the real provider. Example: `theme-toggle.harness.svelte` mounts `ModeWatcher`.
- Components that need the QueryClient or the backend (`createQuery`, `browserApi`) are covered by e2e, not component tests.

```ts
import { createRawSnippet } from "svelte";
import { render } from "vitest-browser-svelte";
import { expect, test } from "vitest";

import PageShell from "./page-shell.svelte";

const content = createRawSnippet(() => ({ render: () => `<p>Shell content</p>` }));

test("renders its children", async () => {
  const screen = await render(PageShell, { props: { children: content } });

  await expect.element(screen.getByText("Shell content")).toBeVisible();
});
```

### Contrast

`src/lib/theme-contrast.svelte.test.ts` runs axe `color-contrast` over every text-on-surface token pair, in light and dark. It replaces per-route contrast checks. The shell e2e flow still runs axe once per theme on the composed page.

## E2E

Four flows:

- `tests/e2e/projects.e2e.ts`: create project, add tasks, complete one, see progress, delete.
- `tests/e2e/shell.e2e.ts` (`@a11y`): theme toggle with contrast checks, sidebar collapse, locale switch, settings (via the user menu) theme picker.
- `tests/e2e/admin.e2e.ts`: admin creates a user, that user signs in with username + password in a fresh context, admin deletes them. The sidebar shows the Workspace, Management and Admin groups, and `/admin/organizations` lists the `e2e` organization.
- `tests/e2e/roles.e2e.ts`: create a custom role (checked in the permission matrix), grant it to an Authentik group, remove both; a member's effective access names the role behind a permission.

The web server builds through turbo (`web:build`), so an unchanged app comes from the cache instead of being rebuilt. A running server on port 4173 is reused outside CI.

### Auth

Tests never drive Authentik (live IdP, MFA). `tests/e2e/auth.setup.ts` mints a session for the shared test user, an admin, instead:

1. Calls the test-only route `POST /api/test-auth/session` (with `role: "admin"`) through the preview server's `/api` proxy.
2. Plants the signed Better Auth session cookie.
3. Saves `tests/e2e/.auth/user.json` (gitignored). The `chromium` project loads it.

Requirements:

- A running server (`target/debug/app serve`) with `E2E_TEST_AUTH=true`.
- For `admin.e2e.ts`: `AUTH_PASSWORD_ENABLED=true` and `AUTH_TRUSTED_ORIGINS=http://127.0.0.1:4173` on that server (the preview origin differs from `SITE_URL`).
- `BACKEND_URL` in `apps/web/.env` (default `http://127.0.0.1:3000`).
- Or skip the preview: `E2E_BASE_URL=http://127.0.0.1:3000` runs against the server, which serves `apps/web/build` itself.

Without the env var the route answers 403.

For a test that must run signed out, add a Playwright project without `storageState`. Do not clear cookies mid-test. A flow with a second user (like `admin.e2e.ts`) opens its own `browser.newContext()` with empty storage.

### Selectors

Prefer, in order: `getByRole`, `getByLabel`, `getByText`, then locators scoped to a visible container (`page.getByRole("main")`, `[data-slot="sidebar"]`). Avoid CSS class chains and unscoped `.nth()`.
