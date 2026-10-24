# Issue tracker: GitHub

Issues and specs for this repo live as GitHub issues on `github.com/ediiiz/accit`. Use the [`gh`](https://cli.github.com/) CLI for all operations. It infers the repo from the `origin` remote when run inside the clone; check the login with `gh auth status`.

Prefer `--json <fields>` with `--jq` over parsing plain text.

## Conventions

- **Create an issue**: `gh issue create --title "<title>" --body-file <file> [--label <label>]`. Write multi-line bodies to a file under `$CLAUDE_JOB_DIR/tmp` (or a heredoc) first; without `--body`/`--body-file` it opens an editor.
- **Read an issue**: `gh issue view <n> --comments` (title, body and all comments).
- **List issues**: `gh issue list --label <label> --state open|closed|all`; also `--assignee`, `--author`, `--milestone`, `--search`.
- **Comment on an issue**: `gh issue comment <n> --body-file <file>`.
- **Apply / remove labels**: `gh issue edit <n> --add-label <label>` / `--remove-label <label>`. A label must exist first: `gh label list`, `gh label create <name> --color <hex> --description "<description>"` (older `gh` without `label`: `gh api repos/{owner}/{repo}/labels -f name=… -f color=…`).
- **Edit**: `gh issue edit <n> --title … | --body-file …`.
- **Close**: `gh issue close <n> --comment "<why>"` posts the comment and closes in one step.
- **Assign**: `gh issue edit <n> --add-assignee <user>` (`@me` for yourself).
- **Pull requests**: `gh pr create|view|status|comment|merge`. PRs target `staging`, never `main` (see `AGENTS.md`).

GitHub numbers issues and PRs in one sequence, so `#42` is either one.

Operations the CLI lacks (bulk queries, issue dependencies): `gh api repos/{owner}/{repo}/…` (REST) or `gh api graphql`. Reads need no confirmation; ask before any write through the API.

## Pull requests as a triage surface

**PRs as a request surface: no.** _(Set to `yes` if this repo treats external pull requests as feature requests; `/triage` reads this flag.)_

When set to `yes`, PRs run through the same labels and states as issues, using the `gh pr` equivalents:

- **Read a PR**: `gh pr view <n> --comments`, and `gh pr diff <n>` for the diff.
- **List external PRs for triage**: `gh pr list --json number,title,author,authorAssociation`, then keep only PRs whose `authorAssociation` is not `OWNER`, `MEMBER` or `COLLABORATOR`.
- **Comment / label / close**: `gh pr comment`, `gh pr edit <n> --add-label`, `gh pr close`.

## When a skill says "publish to the issue tracker"

Create a GitHub issue.

## When a skill says "fetch the relevant ticket"

Run `gh issue view <n> --comments`.

## Wayfinding operations

Used by `/wayfinder`. The **map** is a single issue with **child** issues as tickets. Create the `wayfinder:*` labels on first use (`gh label create`).

- **Map**: a single issue labelled `wayfinder:map`, holding the Notes / Decisions-so-far / Fog body. `gh issue create --label wayfinder:map`.
- **Child ticket**: an issue carrying `Part of #<map>` at the top of its body and the label `wayfinder:<type>` (`research`/`prototype`/`grilling`/`task`). Once claimed, the ticket is assigned to the driving dev.
- **Blocking**: a `Blocked by: #<n>, #<n>` line at the top of the body, the canonical representation. A ticket is unblocked when every blocker is closed.
- **Frontier query**: `gh issue list --label wayfinder:<type> --state open --json number,title,body,assignees`, keep the map's children (`Part of #<map>`), drop any with an open blocker in its `Blocked by` line or an assignee; first in map order wins.
- **Claim**: `gh issue edit <n> --add-assignee @me`, the session's first write.
- **Resolve**: `gh issue close <n> --comment "<answer>"`, then append a context pointer (gist + link) to the map's Decisions-so-far (`gh issue edit <map> --body-file <file>`).
