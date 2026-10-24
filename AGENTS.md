# Monorepo Agent Routing

**This project uses [bun](https://bun.sh/) as JavaScript runtime + package manager.**

Keep this root `AGENTS.md` routing-only.

Load `caveman` skill at start of every session.

Load relevant repo-local skills from `.agents/skills/` when matching work:

- TypeScript work → load `typescript-best-practices`.
- Tailwind class work → load `tailwind-best-practices`.
- SvelteKit work → load `sveltekit-best-practices`.
- AI SDK work → load `ai-sdk`.
- Charts / data viz / LayerChart / shadcn-svelte `Chart.*` work → load `layerchart`.
- shadcn-svelte component work (adding, styling, composing, debugging) → load `shadcn-svelte`.
- Anti-slop Oxlint plugin install / update → load `install-anti-slop`.
- Starter repo initialization / post-clone customization → load `project-init`.
- Similar domain-specific work → load matching skill when available.

## YAGNI

- Build what the current task needs. No speculative options, extension points, config flags, generic layers, or "for later" code.
- No abstraction until the second real use. Delete unused code instead of keeping it around.
- A spec item not yet scheduled is not a reason to add code now.

## Prose: UI copy and code comments

- Minimum words. No subtext, no restating what the name or code already says, no reassurance or marketing tone.
- UI copy: label over sentence; descriptions only when they add a fact or next step. Details → `apps/web/AGENTS.md` "UI copy".
- Code comments: only the why, a constraint, or a non-obvious invariant. One line where possible. No narrating the code, no history ("was", "now", "changed from").
- When in doubt, delete the string or comment.

## Browser automation

- Use the Playwright Agent CLI (`@playwright/cli`), not a browser MCP server: shell commands cost fewer tokens than a loaded tool schema.
- Run it from `apps/web`: `bun run playwright-cli --help`.
- Details and the standard loop → `apps/web/AGENTS.md`.

## Fallow

- Use Fallow for dead code, duplication, and code health checks.
- Start with `bunx fallow` from repo root.
- Common focused commands:
  - `bunx fallow dead-code`
  - `bunx fallow dupes`
  - `bunx fallow health`
  - `bunx fallow fix --dry-run`
- For agent workflows, prefer structured output with `--format json`.
- Before digging through Fallow docs, fetch `https://docs.fallow.tools/llms.txt` and use it as page index.

- Frontend work in `apps/web` → follow `apps/web/AGENTS.md`.
- Server work (Rust: routes, SQL, migrations, Better Auth, permissions) in `crates/` → follow `crates/AGENTS.md`.
- Env var work → server vars in `crates/config`, web/tooling vars in `packages/env` (follow `packages/env/AGENTS.md`). Do not parse env anywhere else.
- Env package work in `packages/env` → follow `packages/env/AGENTS.md`.
- Shared config work in `packages/config` → follow `packages/config/AGENTS.md`.
- Deploy / container / Docker work → follow `docker/single/README.md` (one compiled binary: API + static web app).
- Agent factory work (`/loop /factory`, `scripts/factory`, `.claude/agents/factory-*`) → follow `docs/factory.md`.
- Release / versioning / changelog / CI-workflow work → follow `RELEASING.md`.
- Task spans multiple areas → read + apply each relevant file. More specific package-level instructions win.

## Agent skills

### Issue tracker

GitHub issues on `github.com/ediiiz/accit`, via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Default five: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `GLOSSARY.md` and `docs/adr/`. See `docs/agents/domain.md`.

## Branching & releases

- Work lands on `staging`; `main` is release-only. Open PRs into `staging`, not `main`.
- Merging `staging → main` cuts a release automatically: `.github/workflows/release.yml` derives the next version from conventional commits (git-cliff), builds + pushes the container image to GHCR, then tags `vX.Y.Z` and creates a GitHub release. The image build gates the release (build fails → no tag/release). So commit hygiene directly controls version bumps.
- Keep the git-cliff flow; do not add release-please alongside it. See `RELEASING.md`.
- CI runs locally: `scripts/ci` (Rust gate, web lint, format, typecheck, unit and component tests) must pass before anything merges into `staging`. GitHub Actions spends nothing by default: `ci.yml` is manual-only, and `release.yml` runs only when a release is cut on `main`. Do not add CI triggers without asking.

## Commit style

- Use conventional commits: `type(scope): summary`
- Keep subject imperative and specific
- Prefer lowercase types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`
- Scope when useful: `feat(web): add onboarding empty state`
- Keep subject tight. Explain why in body when not obvious
- One commit = one logical change. Do not mix refactor + feature + formatting noise
- Before commit, run checks relevant to changed area

Examples:

- `feat(web): add onboarding empty state`
- `fix(backend): prevent duplicate task creation`
- `docs: clarify post-clone setup`
