# Releasing

Releases are cut by promoting `staging` to `main`. Versioning is automatic,
derived from [Conventional Commits](https://www.conventionalcommits.org/) — the
same commit style already required by `AGENTS.md`.

Built on [git-cliff](https://git-cliff.org) + the GitHub CLI: the version is
computed and the image built in one job, so a failed build never publishes a tag.

## Day-to-day

- Branch off `staging` and open PRs into `staging`.
- Use conventional commit subjects: `feat: …`, `fix: …`, `refactor: …`, etc.
  Add `!` (e.g. `feat!: …`) or a `BREAKING CHANGE:` footer for a major bump.

## Cutting a release

1. Open a PR from `staging` → `main` and merge it.
2. On the push to `main`, [`.github/workflows/release.yml`](.github/workflows/release.yml)
   runs one job that, in order:
   - computes the next version from commits since the last `v*` tag
     (`feat` → minor, `fix`/`perf`/etc → patch, breaking → major),
   - builds and pushes the single-container image to GHCR tagged with that version
     (plus `latest` and `sha-<sha>`), then
   - pushes the `vX.Y.Z` tag and creates a GitHub release with generated notes.

The image build happens **before** the tag/release steps and gates them: if the
build fails, the job aborts and no tag or release is published. There is no
separate build-image workflow, and no `RELEASE_TOKEN` is needed — nothing keys
off a tag-push event, so the automatic `GITHUB_TOKEN` pushes the tag.

To rebuild and push the image **without** cutting a release (e.g. to refresh
`latest`), run `release.yml` via `workflow_dispatch` with `build_only=true`.

If nothing since the last tag warrants a bump (only `chore`/`ci`/`style`), the
release run is a no-op. `commit_parsers` in [`cliff.toml`](cliff.toml) control
grouping and which types are release-worthy.

## One-time setup

- Create the `staging` branch and set it as the default branch for development.
- Protect `main` (branch protection or a ruleset) so changes land only via
  `staging → main` PRs. Tag pushes from the release workflow are unaffected.
- Settings → Actions → General: workflow permissions "Read and write", so
  `GITHUB_TOKEN` can push the tag, create the release and push to GHCR.
- Optional: `INFRA_REPO` variable + `INFRA_TOKEN` secret for the promote job.
- No image-build variables are required — the container image is origin-portable
  (`SITE_URL` is set at deploy time, not build). See
  [docker/single/README.md](docker/single/README.md).

The first merge to `main` after this flow lands will cut the initial release
(`v0.1.0`).
