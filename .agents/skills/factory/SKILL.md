---
name: factory
description: One tick of the agent software factory: read factory status, apply finished agents' results, dispatch implementers, reviewers and the integrator, merge green PRs, schedule the next tick. Use when running `/loop /factory`.
---

You are the factory **dispatcher**. Each tick is idempotent: the tracker and `scripts/factory/factory status` are the state; anything you remember from earlier ticks is not. Agents run in the background; their completion notifications wake you between ticks.

Setup, launch and tuning live in `docs/factory.md`.

## Tick

1. **Status.** Run `scripts/factory/factory status`. Act only on this JSON.
2. **Apply results.** For every agent completion since the last tick, apply its result line (table below). Done when no completion is unapplied.
3. **Disk.** `disk.level` `red`: run `scripts/factory/factory gc --aggressive` and dispatch no build work this tick (no claim, checkout or integrator). `amber`: run `scripts/factory/factory gc` and claim no new tickets.
4. **Merge.** For each PR in `prs.merging`: `scripts/factory/factory merge <pr>`.
5. **Integrate.** If `free_slots.merge` is not empty and `prs.approved` is not: take the lowest PR, run `scripts/factory/factory checkout <pr> merge` and spawn **factory-integrator** with that JSON.
6. **Review.** For each PR in `prs.review` with no `review:<pr>` entry in `runs`, up to `max_parallel_reviews` running: `scripts/factory/factory run start review <pr>`, then spawn **factory-reviewer-spec** and **factory-reviewer-standards** in one message (prompt: `pr`, `issue`, `branch`).
7. **Fix.** For each PR in `prs.changes` while `free_slots.impl` is not empty: `scripts/factory/factory checkout <pr> impl`, then spawn **factory-implementer** in fix mode with that JSON. Pass `model: opus` when the PR's `route` is `deep`.
8. **New work.** Only on a green disk, while `free_slots.impl` is not empty, take frontier tickets in order:
   - `route` null: spawn **factory-triage** in mode `route <issue>` in the foreground, then `scripts/factory/factory route <issue> <route>`.
   - `split`: `scripts/factory/factory run start split <issue>`; spawn **factory-splitter**.
   - `std` or `deep`: `scripts/factory/factory claim <issue>`; spawn **factory-implementer** with that JSON, passing `model: opus` for `deep`.
9. **Stale runs.** A `busy_slots` or `runs` entry older than 180 minutes with no live agent is dead: release its slot or end its run, then `scripts/factory/factory fail <issue> stalled`.
10. **Report** one line per action taken, then the counts: frontier size, PRs per lane, slots busy, disk free.
11. **Wake.** `done` true and nothing dispatched: stop the loop and report the `human` list. Agents running: schedule a long fallback (1800 s); their completions wake you sooner. Only CI pending: 300 s. Otherwise 900 s.

Spawn every agent in the background except triage, and pass the JSON the script printed verbatim in its prompt. Every slot agent ends with `factory release <slot>` (the Results table), which runs `cargo clean` in that worktree; never skip it, also not for failed or stale agents.

## Results

| Agent | Result line | Apply |
|---|---|---|
| implementer | `RESULT pr=<n>` | `factory opened <issue> <n>`; `factory release <slot>` |
| implementer | `RESULT failed: split needed …` | `factory route <issue> split`; `factory release <slot>` |
| implementer | `RESULT failed: …` | `factory fail <issue> <reason>`; `factory release <slot>` |
| implementer | `RESULT human: …` | `factory human <issue> <question>`; `factory release <slot>` |
| reviewers (both done) | `VERDICT …` ×2 | `factory verdict <pr> approve` when both approve, else `changes`; `factory run end review <pr>` |
| integrator | `RESULT integrated` | `factory integrated <pr>`; `factory release <slot>` |
| integrator | `RESULT changes: …` | `factory verdict <pr> changes`; `factory release <slot>` |
| integrator | `RESULT human: …` | `factory human <pr> <question>`; `factory release <slot>` |
| triage | `ROUTE <route>: …` | `factory route <issue> <route>` |
| splitter | `SPLIT …` | `factory run end split <issue>` |

A result without its line counts as `RESULT failed: no result line`. All commands are `scripts/factory/factory …`.
