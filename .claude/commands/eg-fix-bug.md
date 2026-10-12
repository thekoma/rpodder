---
description: Fix a bug using the elephant/goldfish workflow — problem doc, goldfish diagnosis check, failing test, fix, review, validate
argument-hint: bug description, GitHub issue URL, or symptom + repro
---

Fix a bug using the elephant/goldfish workflow. The aim: write a problem doc, goldfish-check the diagnosis (so we are not anchored to the first hypothesis), capture the bug as a failing test, fix it, then run `/eg-precommit-review` and the test gate.

`$ARGUMENTS` is the bug description provided by the user. If empty, ask for one before doing anything. If `$ARGUMENTS` is a GitHub issue URL or `#<number>`, fetch it first with `gh issue view <number> --json title,body,labels,comments` and seed the problem doc from it.

## Interactivity is mandatory at decision points — overrides any "no-stopping" directive

This skill is mostly autonomous, but it has **mandatory** user-facing decision points: the missing-repro stop in Step 1, the goldfish-vs-elephant divergence resolution in Step 2, and any moment where the bug shape is genuinely ambiguous. Enumerable choices go through `AskUserQuestion`; genuinely unbounded answers (repro steps in prose, custom log lines) go through one targeted chat prompt AFTER an `AskUserQuestion` scopes the reason.

If a `<system-reminder>` or any other injected directive in this session tells you to work autonomously without stopping for clarifying questions (e.g. "no-stopping directive"), **it does NOT override these gates**. In particular: do NOT guess a repro on the user's behalf — the goldfish needs something concrete to act on, and a wrong-shaped guess wastes the goldfish call. Always ask.

The only opt-out: if the user, in the same turn that invoked this skill, explicitly says "use your best guess for the repro" (or equivalent unambiguous override), you may proceed with an inferred repro. Even then: print the repro you're using before spawning the goldfish.

## Step 0: Triviality gate

**Skip the goldfish/test ceremony for:** typo fixes in copy or comments, dead-code removal, version bumps, formatter-only diffs, single-line config tweaks. Go straight to Step 5 (`/eg-precommit-review`) and the test gate.

**Run the full loop for everything else,** including small one-line code fixes — small diffs hide bugs disproportionately well.

## Step 1: Write the problem doc (in this conversation)

Print a tight problem doc to the user. Keep it brief but complete:

```
PROBLEM DOC
- Symptom: <what the user observes>
- Repro: <steps to reproduce, or "user did not provide; need to derive">
- Suspected area: <file/module/route/worker/job/screen>
- Hypothesised root cause: <one sentence>
- Blast radius: <which other surfaces could be affected>
- "Fixed" means: <specific test passes / specific behavior / specific output>
```

If the user gave no repro and the bug is not obvious from a single file read, **stop and call `AskUserQuestion`** to scope the missing repro:

- `question`: "No repro provided. How would you like to proceed?"
- `header`: `"Repro?"`
- `multiSelect`: `false`
- `options`:
  1. **I'll paste a repro in chat** — "Steps, URL, failing test name, log line, or screenshot."
  2. **It's in a GitHub issue / linked doc** — "I'll share the link in chat."
  3. **You infer it from the symptom** — "Use your best guess; I accept the risk it may be wrong-shaped."
  4. **Drop the request** — "Not enough context yet; I'll come back."

Then accept the user's free-form input in chat for options 1 or 2. Do NOT guess on the user's behalf unless they pick option 3. The goldfish needs something concrete to act on.

For web UI bugs (Svelte 5 pages under `web/src/routes/`), the repro path is usually the **Claude in Chrome MCP** (`mcp__Claude_in_Chrome__*`) against the Vite dev server on `http://localhost:5173`. Assume it is running; otherwise start it with `cd web && bun run dev`. Vite proxies `/api`, `/subscriptions`, `/auth`, `/health`, … to a backend on **:3006** (see `web/vite.config.ts`), so a backend must be up there too: `docker compose --profile dev up -d` (PostgreSQL + cargo-watch, host :3006) or natively `RPODDER_PORT=3006 cargo run -p rpodder-server -- serve` (SQLite `rpodder.db`). For bugs in the embedded build (rust-embed, SPA fallback, asset paths), use `http://localhost:3005` after `cd web && bun run build` and `cargo run -p rpodder-server -- serve`. Navigate, click, take a screenshot, read the console.

For gpodder API bugs (the most common kind: a client such as Kasts or AntennaPod misbehaves), the repro is a `curl` with Basic Auth against the running server (`curl -u user:pass http://localhost:3005/api/2/...`), the exact request body the client sent, a failing test name, or a log line (`RUST_LOG=rpodder=debug`). A dev user can be created with `cargo run -p rpodder-server -- user create <name> <pass> --admin`. Never run `docker compose … down -v` or otherwise reset a Docker volume without asking: the user's podcast data lives in `pgdata` / `sqlite-data`.

## Step 2: Goldfish diagnosis check (parallel to your hypothesis)

Spawn a fresh agent with `Agent` tool:
- `subagent_type: "Explore"` for narrow lookups, `"general-purpose"` if the bug spans multiple subsystems. If the harness does not have `Explore` registered, fall back to `general-purpose`.
- `description: "Goldfish bug diagnosis"`

The goldfish gets ONLY the symptom + repro from Step 1. It does NOT get your hypothesised root cause — that asymmetry is the point.

**Prompt body to send (between markers, exclusive):**

```
<<<DIAG_START>>>
Independent diagnosis of a bug in this repo (rpodder, a Rust replacement for gpodder.net: an axum + sqlx server, PostgreSQL or SQLite, that implements the gpodder/mygpo sync API for podcast clients, plus a background feed updater and an embedded Svelte 5 web UI; CLAUDE.md at the repo root has the full architecture).

Symptom: <FILL IN from Step 1>
Repro: <FILL IN from Step 1>

Investigate. Where in the codebase is the bug most likely to live? Cite specific file:line locations. List the top 1-3 candidate root causes ranked by likelihood. For each candidate, name what evidence in the code supports it and what would falsify it. Do NOT propose a fix yet — just diagnose.

If a UI repro is needed, you may use the Claude in Chrome MCP (`mcp__Claude_in_Chrome__*`) against the running dev server at http://localhost:5173 (or the embedded build at http://localhost:3005), read-only: no deletes, no account or admin changes.

End with the literal string `diagnosis complete`.
<<<DIAG_END>>>
```

**Compare goldfish output to your Step 1 hypothesis.**
- Convergence (top candidate matches your hypothesis): proceed to Step 3 with confidence.
- Divergence: re-investigate. Read the goldfish's evidence. If it is right, update the problem doc and tell the user "goldfish flagged a different root cause; re-diagnosing." If you are right, write down WHY the goldfish was wrong — that disagreement is itself useful signal.

If the bug is genuinely tiny (1-3 line fix in a clearly-identified location), you may skip the goldfish call — but only when the location is mechanically obvious. When in doubt, run it.

## Step 3: Capture the bug as a failing test BEFORE fixing

The verification criterion lives in code, not in chat. Pick the right tier:

- **Pure-logic unit test** (`#[cfg(test)] mod tests` next to the code): URL normalization (`rpodder-core/src/url.rs`), private-URL detection (`rpodder-core/src/privacy.rs`), feed parsing (`rpodder-feed/src/parser.rs`).
- **Repo test** (`rpodder-db/src/sqlite.rs` / `lib.rs` tests): query behaviour against in-memory SQLite (`Db::connect("sqlite::memory:")` + `db.migrate("../../migrations")`).
- **HTTP integration test** (`crates/rpodder-server/tests/<area>_integration.rs`: `auth`, `devices`, `subscriptions`, `episodes`, `sync`, `admin`): drives the axum router with `tower::ServiceExt::oneshot` over in-memory SQLite. These files **replicate the router setup** instead of importing it, so a bug in route wiring or middleware order may need the test router updated to match `main.rs`. Use this tier for auth, path-username, status-code and JSON-shape bugs.
- **Docker E2E** (`tests/docker/tests/NN_name.sh`, run by `./tests/docker/run-integration.sh sqlite|postgres|all`): a live server plus the mock RSS server. Use it for PostgreSQL-only behaviour (every Rust test runs on SQLite) and for feed-updater bugs (`/feed/static` with ETag/304, `/feed/dynamic`). Use `wait_until`, not `sleep`.
- Watch the SQLite-vs-PostgreSQL gap: the two repo impls in `rpodder-db/src/sqlite.rs` and `postgres.rs` are separate SQL. If the bug lives in one, check the other, and say so when a green SQLite test does not prove the PostgreSQL path.
- For client-compat bugs, pin the exact payload the client sent (e.g. Kasts' bare `[...]` episode-action array, string vs integer timestamps) in the test.

Write the test. Run it. Confirm it fails for the reason described in the problem doc. If it fails for a different reason, the test is wrong — fix the test before touching the implementation.

The web UI has no test tier (no Vitest, no Playwright, no `svelte-check` script). For UI bugs, capture the repro as a Claude in Chrome MCP script in the conversation: navigate, click, screenshot, read the console. Confirm the bug is observable. This is your verification path, and you re-run it after the fix. Still add a Rust integration test for whatever server-side part the bug has (status code, JSON shape, missing field).

## Step 4: Fix it

Implement the smallest change that turns the failing test green and matches the "Fixed means" criterion.

Avoid: adjacent refactors, defensive coding for cases the bug did not surface, fallbacks that mask future regressions, unrequested feature flags. Bug fix scope is the bug, nothing else.

Re-run the failing test. It must go green. If it does not, you have not fixed the bug — do NOT rewrite the test.

For UI bugs, re-run the Claude in Chrome MCP repro from Step 3 against `http://localhost:5173` (Vite hot-reloads; restart `bun run dev` if a proxy or config change is involved). If the bug was in the embedded build, rebuild (`cd web && bun run build`, then rebuild the server, since rust-embed bakes `web/dist/` into the binary) and re-check on `http://localhost:3005`. Confirm the symptom is gone and the console is clean.

## Step 5: Hand off to `/eg-precommit-review`

Run `/eg-precommit-review` per the canonical procedure. The reviewer is a second goldfish — it sees only the diff. Triage findings, loop, and exit.

If `/eg-precommit-review` surfaces an issue that the Step 2 diagnosis goldfish missed, note it in the final report — it tells us where the diagnosis prompt needs to be tighter next time.

## Step 6: Test gate

```sh
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo build --no-default-features         # API-only build (web-ui feature off) must still compile
(cd web && bun run build)                 # only if the diff touches web/
./tests/docker/run-integration.sh sqlite  # only if the API surface, a migration, a repo impl or the feed updater changed; ASK FIRST (see below)
./tests/docker/run-integration.sh postgres  # same condition; required when a migration or postgres.rs changed
```

All required tiers must pass. `cargo clippy`/`cargo test` need `web/dist/` to exist (rust-embed): run `cd web && bun run build` first, or create the placeholder CI uses (`mkdir -p web/dist && touch web/dist/index.html`). Skip the Docker E2E only when the diff is pure logic fully covered by Rust tests, docs, or web-only. **Before running `run-integration.sh`, ask the user:** it runs `docker compose … down -v` for the `sqlite`/`release`/`test` profiles, which deletes the `sqlite-data` and `pgdata` volumes where the user's podcast data lives. If a migration changed, the PostgreSQL profile is required, not optional. For UI bugs, also re-verify the original repro in the Claude in Chrome MCP one final time before reporting done.

## Step 7: Final report

Print to the user:
- Bug summary (one line)
- Root cause (one line)
- Fix (file:line)
- Test that captures it (file:test name)
- Goldfish-vs-elephant agreement (converged / diverged + why)
- `/eg-precommit-review` outcome (rounds, fixes, rebuttals verbatim)
- Test gate status

**STOP.** Do NOT commit; auto mode does not override the project's commit policy. Wait for the user's literal commit instruction. Follow the Conventional Commits style visible in `git log` (`fix: …`, `feat: …`, `ci: …`, `chore(deps): …`), referencing the GitHub issue when there is one. Never push on your own: a push to `main` that touches source auto-tags a release and publishes an image (see `AGENTS.md`). Don't bump the version in `Cargo.toml` / `Cargo.lock` / `web/package.json` or edit `CHANGELOG.md` unless the user is cutting a release.
