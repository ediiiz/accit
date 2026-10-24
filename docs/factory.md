# Agent factory

Implements the V1 tickets unattended: one Claude Code session runs `/loop /factory`, dispatches background agents, and lands their PRs on `staging` after automated review, integration and CI.

## Pipeline

```text
frontier ─▶ triage (Haiku) ─▶ implementer (Sonnet │ Opus for deep) ─▶ PR [factory:review]
        ─▶ spec reviewer (Opus) ‖ standards reviewer (Sonnet) ─▶ [factory:approved] or [factory:changes] ─▶ fix
        ─▶ golden guard ─▶ integrator (Sonnet, merge slot) ─▶ [factory:merging] ─▶ CI green ─▶ merge, issue closed
oversized ticket ─▶ splitter (Opus) ─▶ child tickets back into the frontier
```

| Piece                                               | Where                             | Model                                                                                                             |
| --------------------------------------------------- | --------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Dispatcher (one tick per wake)                      | `.agents/skills/factory/SKILL.md` | the loop session; Sonnet is enough                                                                                |
| Control plane: frontier, slots, labels, merge, disk | `scripts/factory/factory`         | none (deterministic)                                                                                              |
| Agents                                              | `.claude/agents/factory-*.md`     | Haiku: triage · Sonnet: implementer, standards review, integrator · Opus: deep implementer, spec review, splitter |
| Hard guards                                         | `.claude/hooks/factory-guard.py`  | —                                                                                                                 |
| Factory permissions                                 | `.claude/factory/settings.json`   | —                                                                                                                 |

**State** lives in the tracker (labels `factory:*` on issues and PRs) and in `.factory/state.json` of the main checkout (slots, attempts, routes, runs). Both survive a compacted or restarted session.

**Frontier**: open issues labelled `ready-for-agent`, with a milestone, every `Blocked by` issue closed, no `factory:*` state label and no open PR, ordered by milestone then issue number.

## Build slots and disk

Builds run only in a fixed pool of **slots** under `.factory/slots/` (`impl-1…3`, `merge-1`): reused worktrees, each with a warm `target/`, its own database `factory_<slot>` and `node_modules`. A ticket never gets a fresh worktree, so disk is bounded by slots × cap instead of tickets × build.

| Control                 | Setting                                                                                                                                |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| Smaller builds in slots | `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_{DEV,TEST}_DEBUG=line-tables-only`, `CARGO_BUILD_JOBS=3` (passed through `scripts/cargo`)        |
| On every slot release   | drop leftover `#[sqlx::test]` databases, delete `target/debug/incremental`, delete `target/` when above `slot_target_cap_gb` (12)      |
| Each tick               | free disk below `disk_pause_free_gb` (30): `gc`, no new tickets · below `disk_critical_free_gb` (15): `gc --aggressive`, no build work |
| `gc`                    | shrinks idle slots, prunes worktrees, deletes merged `factory/*` branches, `docker image prune`                                        |

Never deleted: the Postgres volume (`docker volume rm`/`system prune` are denied), your own checkouts and their `target/`.

## Setup (once)

1. Postgres running: `docker compose -f docker/compose.yml up -d` (container `app-pg`, see `scripts/factory/config.json`).
2. `gh auth status` shows a login for the configured host (the factory reads the API token via `gh auth token`).
3. `scripts/factory/factory setup` creates the `factory:*` labels.
4. CI: with runners picking up pull requests (`rust.yml`, `check.yml`) keep `require_ci: true`. Without CI set `require_ci: false` in `scripts/factory/config.json`; the integrator's local gates are then the only gate. Either way `factory merge` refuses a branch that does not contain the current `staging` tip and sends it back to `approved` for re-integration, so what merges is what was tested.

## Run

From the main checkout:

```sh
claude --settings .claude/factory/settings.json --permission-mode dontAsk --model sonnet
```

then `/loop /factory`. The session stays open; `/loop` is session-scoped and expires after 7 days. `dontAsk` denies everything the factory settings do not allow; the guard hook blocks pushes to `staging`/`main`, plain force pushes, skipped hooks and merges outside `factory merge`.

**Watch**: `scripts/factory/factory status`, the `factory:*` labels on GitHub, and the milestone progress.

**Stop**: Esc in the session. Agents already running finish; nothing new is dispatched.

**Needs a person**: issues and PRs labelled `factory:human` (questions, three failed attempts, contradictory conflicts) and `parity-finding` issues. Remove `factory:human` after answering and the ticket re-enters the frontier.

## Tuning (`scripts/factory/config.json`)

| Key                    | Default                                  | Raise when                                     |
| ---------------------- | ---------------------------------------- | ---------------------------------------------- |
| `slots.impl`           | 3                                        | more cores and ~15 GB free disk per extra slot |
| `cargo_build_jobs`     | 3                                        | fewer slots (keep slots × jobs ≈ cores)        |
| `max_parallel_reviews` | 4                                        | reviews queue up                               |
| `max_attempts`         | 3                                        | —                                              |
| `golden_paths`         | `golden`, `/fixtures/`, `/oracle/cases/` | oracle tickets settle the fixture layout       |
