# Testing

Three lanes. Push every check down to the cheapest lane that can prove it.

| Lane      | Files                     | Runs in                                | Use for                                                       |
| --------- | ------------------------- | -------------------------------------- | ------------------------------------------------------------- |
| unit      | `src/**/*.test.ts`        | Node                                   | Pure logic: Portal state derivation, formatting.              |
| component | `src/**/*.svelte.test.ts` | Vitest browser mode, headless Chromium | One component's contract: states, interaction, accessibility. |
| e2e       | `tests/e2e/**/*.e2e.ts`   | Playwright through the Compose harness | Unlock and Revoke end to end, axe in light and dark.          |

Gate, Grant and API rules are tested in Rust (`crates/`, `scripts/check-rust`) and in the harness suite. Do not re-prove them in the Portal's tests.

## Commands

From `apps/web`: `bun run test:unit`, `bun run test:component`, `bun run test:vitest`. From the repo root: `bun run web:test:unit`, `web:test:component`.

The e2e lane needs the Compose harness and Docker: run `scripts/e2e-web` from the repo root. It starts the stack, runs Chromium in a container on the harness network with the test client certificate, and removes the stack. `bun run test:e2e` (and `test:a11y` for the `@a11y` tests) only works with `E2E_BASE_URL` pointing at a running harness.

`scripts/ci` (repo root) runs unit and component tests with the rest of the local CI; GitHub Actions does not run them. `scripts/e2e-web` is not part of it.

## Policy

Tests first: write the failing test for the new behaviour or bug, then the code that turns it green (`tdd` skill).

- Keep logic out of `.svelte` files: the screen state machine lives in a `#lib/*.ts` module with unit tests.
- A component test covers each screen state's copy, primary action and announced status.
- No module mocks (`anti-slop/no-module-mocking`). Pass data in through props, or mount the real dependency in a `*.harness.svelte` beside the component.
- Prefer user-facing assertions (`getByRole`, `getByLabel`, `getByText`) over classes or internal state.
- Paraglide messages (`m.*`) return the base locale (English) in tests.

## e2e

- One accit and one source Address: tests run in order, one worker. Mutations are limited to 5 a minute per Address, so the real Unlock and Revoke calls stay under that; screens the stack cannot reach on demand (expired, home network, submitting, service unavailable, rate limited, a changed connection) get the API answer they would get there through `page.route`, built from the real status with only `state` and `expiry` replaced. A held or refused request never reaches accit, so it costs no mutation.
- Axe checks every screen in light and dark (`expectAccessible` in `tests/e2e/axe.ts`), and the Not enabled screen under a German browser.
