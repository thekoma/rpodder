# AGENTS.md

Guidance for AI agents working in this repo. General architecture, build/test
commands, and gotchas live in [`CLAUDE.md`](CLAUDE.md) — read that first. This
file documents the **release & versioning scheme**, which is easy to get wrong.

## Versioning: calendar, unpadded `YYYY.M.N`

rpodder uses calendar versioning. A version looks like `2026.5.4`:

- `YYYY` — year
- `M` — month, **unpadded** (`5`, not `05`)
- `N` — release counter within that month, starting at `0`

The month is unpadded on purpose: `2026.05.4` is **not** a valid SemVer string
(SemVer forbids leading zeroes in numeric identifiers), so `Cargo.toml` and
`package.json` would refuse it. `2026.5.4` is valid everywhere, which lets the
git tag, Docker image tag, Cargo version and npm version be byte-for-byte equal.

> Historical tags from before this scheme are zero-padded (e.g. `2026.05.1`,
> `2026.03.43`). Leave them as-is; only new releases are unpadded.

## Where the version lives — keep these aligned

| Location | Field | Notes |
|----------|-------|-------|
| `Cargo.toml` | `[workspace.package] version` | All crates inherit via `version.workspace = true` |
| `Cargo.lock` | the four `rpodder-*` package entries | Update to match, or let `cargo build` regenerate |
| `web/package.json` | `version` | |
| `CHANGELOG.md` | top heading `## YYYY.M.N — DATE` | |
| git tag / Docker image / `latest` | computed by CI | see below |

At runtime, `GET /api/2/me/build` (and `/health` internals) reports two things:

- `version` — compile-time `CARGO_PKG_VERSION` (from `Cargo.toml`)
- `build_tag` — the `RPODDER_BUILD_TAG` build-arg, set by CI to the git tag

These should print the **same** value on a released build. They diverge only on
local/dev builds (`build_tag` defaults to `dev`).

## How CI computes the tag

`.github/workflows/ci.yml` (job `release-tag`) runs on every push to `main` and
on tag pushes:

- **Tag push** → uses the tag name verbatim.
- **Push to `main`** → derives the next version: `PREFIX = $(date +'%Y.%-m')`,
  then finds the highest `N` among this month's existing tags (matching the
  month *numerically*, so legacy padded tags still count) and increments it.
- **PR / other** → `ci-<sha>`, no release.

So **pushing to `main` auto-creates a release** (tag + multi-arch image +
GitHub release). There is no separate "publish" step.

### Source-change gating

The workflow uses a `paths-ignore` filter so changes that never affect the built
image don't run anything — no build, no release, the calendar counter does not
advance. Ignored: `**.md`, `docs/**`, `deploy/**` (Helm/TrueCharts values),
`config/**`, `.github/**`, `LICENSE`. Any other change (crates, web, migrations,
`Cargo.*`, `Dockerfile`, …) runs the full pipeline.

Note: this is all-or-nothing per push — a push touching only ignored paths skips
the whole workflow (lint/test included), which is why `.github/**` is ignored
too (a pipeline tweak shouldn't cut a release). Validate workflow edits via a PR
that also touches source, or `workflow_dispatch`.

## Release checklist

Because the push auto-tags, the in-repo version files must be bumped to the
**predicted next tag** *before* pushing — otherwise `CARGO_PKG_VERSION` and the
git tag disagree.

1. Predict the next version: `<year>.<unpadded-month>.<lastSuffixThisMonth + 1>`.
   (First release of a new month → `.0`.)
2. Set that value in **`Cargo.toml`**, **`Cargo.lock`** (four `rpodder-*`
   entries), and **`web/package.json`**.
3. Add a `## YYYY.M.N — YYYY-MM-DD` section to **`CHANGELOG.md`**.
4. Commit and push to `main`. CI tags it with exactly that version, builds the
   amd64+arm64 image, and publishes the GitHub release.

If two releases land in the same month, only the `N` increments
(`2026.5.4` → `2026.5.5`). At a month boundary, `N` resets to `0`.
