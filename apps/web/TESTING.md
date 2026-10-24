# Testing

Three lanes. Push every check down to the cheapest lane that can prove it.

| Lane      | Files                     | Runs in                                | Use for                                                       |
| --------- | ------------------------- | -------------------------------------- | ------------------------------------------------------------- |
| unit      | `src/**/*.test.ts`        | Node                                   | Pure logic: Portal state derivation, formatting.              |
| component | `src/**/*.svelte.test.ts` | Vitest browser mode, headless Chromium | One component's contract: states, interaction, accessibility. |
| e2e       | `tests/e2e/**/*.e2e.ts`   | Playwright through the Compose harness | Unlock and Revoke end to end, axe in light and dark.          |

Gate, Grant and API rules are tested in Rust (`crates/`, `scripts/check-rust`) and in the harness suite. Do not re-prove them in the Portal's tests.

## Commands

From `apps/web`: `bun run test:unit`, `bun run test:component`, `bun run test:vitest`, `bun run test:e2e`. From the repo root: `bun run web:test:unit`, `web:test:component`, `web:test:e2e`.

CI (`.github/workflows/check.yml`) runs unit and component tests.

## Policy

Tests first: write the failing test for the new behaviour or bug, then the code that turns it green (`tdd` skill).

- Keep logic out of `.svelte` files: the screen state machine lives in a `#lib/*.ts` module with unit tests.
- A component test covers each screen state's copy, primary action and announced status.
- No module mocks (`anti-slop/no-module-mocking`). Pass data in through props, or mount the real dependency in a `*.harness.svelte` beside the component.
- Prefer user-facing assertions (`getByRole`, `getByLabel`, `getByText`) over classes or internal state.
- Paraglide messages (`m.*`) return the base locale (English) in tests.
