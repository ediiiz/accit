# Vendored anti-slop Oxlint plugin

Source: [`dmmulroy/anti-slop`](https://github.com/dmmulroy/anti-slop) (GitHub), installed from the
`install-anti-slop` skill bundle vendored at `.claude/skills/install-anti-slop/assets/anti-slop`.

## Revision

Upstream commit: **unknown**. `skills-lock.json` records only the skill-manifest hash
`4031728fbe75bdcad6ee3208fd52b5d66e167b056fefee1fa9758e9a6cb9c0c8` for
`skills/install-anti-slop/SKILL.md`, which identifies the skill bundle but not an upstream commit.
Re-resolve an exact commit before the next update; do not treat the hash above as one.

## Installed paths

- `tools/oxlint/anti-slop/index.ts` — generic plugin entry point (registered in `.oxlintrc.json`).
- `tools/oxlint/anti-slop/rules/`, `shared/` — generic rules and helpers.
- `tools/oxlint/anti-slop/effect/` — opt-in Effect plugin, copied but **not registered**: this repo
  has no direct `effect` dependency.
- `tools/oxlint/anti-slop/vendor/eslint-stylistic/` — vendored MIT rule plus its `LICENSE` and
  `UPSTREAM.md`; keep both with every copy.

## Dependencies

- `oxlint` `^1.41.0` (resolved `1.58.0`, pre-existing).
- `@oxlint/plugins` pinned exactly to `1.58.0` to match the resolved `oxlint` version; upgrade both
  together.

## Local deviations

- Copied to `tools/oxlint/anti-slop/` (skill default).
- `.oxlintrc.json` keeps the pre-existing `poc/**/*` ignore and adds agent-tooling ignores plus
  `tools/oxlint/anti-slop/**`; `.deepwork/**` added for this repo's DeepWork assets.
- No rule source was modified.
- No lint findings were fixed: installation only. 109 pre-existing findings remain in owned source.

## Updating

Follow the skill's update procedure (`references/update.md`): stage a pristine upstream snapshot in a
temp dir, merge rather than replace, and refresh this record with the resolved commit.
