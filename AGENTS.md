# AGENTS.md

A self-hosted merge queue for GitHub. Pull requests land in a verified order, on a base they
were actually tested against. Rust workspace plus a React console embedded in the binary.

Read `README.md` for what the product is. This file is how to work on it.

## Repository layout

- `crates/goat-merge-core` — the queue itself. State machine, readiness, the config file,
  the plan for what to do next. Pure functions over a snapshot of GitHub.
  **No network, no database, no async, no clock.**
- `crates/goat-merge` — HTTP, webhooks, the GitHub adapter, storage, the CLI, the API the
  console reads.
- `web` — the console. Vite + React 19 + Tailwind v4, Feature-Sliced.
- `migrations` — sqlx migrations, applied at startup.

Each of the three has its own `AGENTS.md`. Read the nearest one.

## Setup

Rust from `rust-toolchain.toml` (stable, with `rustfmt` and `clippy`), Node 24+, and pnpm.
`cargo build` runs `pnpm install && pnpm run build` in `web/`; set `GOAT_MERGE_SKIP_WEB_BUILD=1`
to skip that while working on Rust only.

Anything touching `crates/goat-merge` needs a PostgreSQL to talk to:

```sh
docker compose up -d db
docker compose exec db createdb -U goat merge_test   # once
export DATABASE_URL=postgres://goat:goat@127.0.0.1:5432/merge_test
```

**Test against `merge_test`, never against a database a server is using.** The tests are
destructive: `app_credentials` holds one row, so they overwrite whatever GitHub App is
registered there, and GitHub shows a private key once. Losing it means creating the App again,
and the old one is left behind in the organization.

**Without `DATABASE_URL`, the store and engine tests print `SKIPPED` and pass without
running.** `cargo test --workspace` on a fresh clone therefore proves the queue logic and
nothing else. CI always sets it.

## Commands

| Task | Command |
|---|---|
| all tests | `cargo test --workspace` |
| queue logic only, no database | `cargo test -p goat-merge-core` |
| the whole flow against a fake GitHub | `cargo test -p goat-merge --test engine` |
| one test by name | `cargo test -p goat-merge-core a_quiet_base_lets_the_existing_checks_stand_in_for_a_candidate` |
| lint | `cargo clippy --workspace --all-targets` |
| format | `cargo fmt` |
| console typecheck | `pnpm --dir web typecheck` |
| console build | `pnpm --dir web build` |
| console dev server | `pnpm --dir web dev` (proxies `/api` to `127.0.0.1:8080`) |

Run the narrowest relevant check first, then widen in proportion to what you touched.

To run it by hand:

```sh
DATABASE_URL=postgres://goat:goat@127.0.0.1:5432/merge \
GOAT_MERGE_PUBLIC_URL=http://127.0.0.1:8080 \
GOAT_MERGE_MASTER_KEY=$(openssl rand -hex 32) \
cargo run -p goat-merge -- run
```

## Rules

These are not style preferences. Breaking one lets a broken commit onto somebody's `main`.

- **The tree that lands is the tree a passing check verified.** This is the whole product.
  Every path to a merge either builds a candidate from the current base and waits for its
  checks, or proves the existing checks already tested this exact combination. There is no
  third path. A candidate may carry several pull requests, in which case the verified tree is
  the one that exists once they have all landed; the states in between are on the way there,
  and nobody else can get in because our check is required and the tend holds the lock.
- **A candidate may also be built on one that has not landed yet.** The base it was verified
  against is then a tree that does not exist on the branch, so the result is evidence about
  nothing until the ones it assumed have landed, at the heads it assumed, with nothing else in
  between. `verification_assumptions` is that list; the branch tip is re-read and compared
  against the last of them in the moment before merging. If any of them left, failed, or landed
  as something else, the result is discarded, not used.
- **A verification is evidence about exactly who rode it.** `verification_members` is that
  list. If somebody joins, leaves, or pushes, the result is about a tree nobody is asking
  about any more and it is discarded. `plan::decide` compares the whole list, not the first
  one.
- **A batch that fails accuses nobody.** Two pull requests that fail together are two
  suspects. The queue halves how many it verifies at once and tries the smaller set; only a
  pull request that fails *on its own* has been shown to be at fault and leaves. Settling
  everyone on a shared red is how a merge queue earns a reputation for throwing out
  innocent work.
- **A candidate that assumed somebody else's work also accuses nobody.** Only a pull request
  verified on the branch's own tip, carrying nothing but itself, has been shown to be at fault.
  A candidate built ahead of the queue that fails is thrown away and built again without the
  assumption; it is not a verdict, and it does not move how many the queue verifies at once. The
  same goes for not merging into it: a tree that only exists because we assumed somebody else's
  work would land is not the branch, so a conflict with it says nothing about whether the pull
  request merges and must never settle one.
- **How many to verify at once is a queue's own business, not the config's.** `batch_size`
  is a ceiling. `queues.verify_at_once` is where the queue actually is, and it starts at 1,
  climbs by one on every pass and halves on every failure. A queue on a flaky repository
  therefore shrinks itself without anybody editing a file.
- **How deep to build ahead of the queue is a queue's own business too.** `speculate` is a
  ceiling and it is 1 — off — until somebody raises it. `queues.speculate_to` is where the queue
  is: it climbs when a candidate built ahead lands on the tree it assumed, and halves when one
  is thrown away unused. A queue that has *halved* its width down to 1 never builds ahead at
  all, because that is a queue telling us its checks are flaky, and flaky checks are exactly
  where a run made ahead of the queue is wasted.
- **Re-read the base and head in the moment before merging.** A verification that was true a
  minute ago is not evidence about now. If `base` or `head` moved, discard the result and
  start over, and if it assumed anything, check that the assumption materialised. Never merge
  on a stale verification to save a CI run.
- **A check is only evidence about the base it ran against, judged by when it started.** The
  fast path is allowed only when every required check *started* after the base last moved. A
  check that began at 09:50 and finished at 10:30 tested the base as it was before the 10:00
  push, so its completion time proves nothing. A check with no start time is never trusted.
- **GitHub's mergeable flag is not the only evidence that something merges.** GitHub computes
  it in the background and answers `null` for minutes after the base moves. A pull request whose
  candidate carried this exact head and passed has already been merged by us, into the tree the
  checks tested, so an unknown answer must not hold it back. Nothing else skips readiness: a
  conflict is an answer and still blocks, and a pull request with no passing candidate still
  waits.
- **We learn when the base moved by watching, not by asking.** `queues.base_moved_at` is set
  whenever we see the tip change. A commit's own timestamp is not when it landed on a branch,
  so it must never be used for this.
- **Never bypass approvals or required checks.** GitHub's rulesets and branch protection are
  the source of truth for whether a pull request *may* merge. This product only decides in
  what order and by what method. If the two ever disagree, GitHub wins.
- **Our own check never feeds readiness.** `Merge Queue` is a required check on the base
  branch, so counting it when deciding whether a pull request is ready deadlocks the queue
  against itself. `readiness::required_checks` excludes it; keep it that way.
- **Our own check turns green once, immediately before the merge.** It is the gate. Publishing
  `success` at any other moment lets somebody merge by hand around the queue.
- **Fork code never becomes trusted code.** Copying a fork's commits onto a branch in the base
  repository makes GitHub hand it secrets and a write token it would never have given the
  fork's own pull request. A fork is verified by the workflow at
  `.github/workflows/merge-queue-fork.yml`, checked for a permissions block and no mention of
  secrets, or it is `Blocked`. An explicit enqueue by a maintainer does not change this —
  review does not make a dependency safe.
- **Never pick a merge method.** Merge, squash and rebase are three different histories. If the
  repository allows more than one and the configuration is silent, the answer is `Blocked`,
  not a guess.
- **GitHub is the fact; the database is a work log.** Webhooks arrive more than once, out of
  order, and sometimes not at all. Treat one as a hint that something changed, then go ask
  GitHub. Never decide anything from a webhook payload alone.
- **A webhook handler persists and returns.** It writes the delivery down, schedules a tend,
  and answers. It does not merge, label, or create a branch. Deciding inside the handler makes
  duplicate deliveries into duplicate merges.
- **Every action is safe to run twice.** Somewhere a retry will run it twice.
- **One tend at a time per queue.** Held by `pg_advisory_xact_lock`, not by hope.
- **Never change a repository's settings.** Read rulesets and branch protection, diagnose them,
  explain what is missing. Do not fix it for them. Requesting `Administration: write` is
  itself the bug.
- **An installation counts only if somebody started it here.** The App is public, so anybody on
  GitHub can install it from its own page. `/setup/install` mints a state, the callback will not
  record an installation without it, and `react` drops the webhook for an installation it has
  never heard of. Without that, a stranger's repositories land in the database and their queues
  run on this server.
- **`Waiting` and `Blocked` always name a reason.** A state with no reason is a support ticket.
- **A sentence reaches a person only if somebody wrote it for that person.** Every other `Display`
  in this workspace — `StoreError`, `GithubError`, `serde`, `sqlx`, `jsonwebtoken`, axum's
  rejections — is for the log. The only text that crosses the wire is built inside `Fault`'s own
  `match`, and `Fault::Broken` cannot be constructed without an `Incident`, which logs the real
  failure and hands back a number to quote. `crates/goat-merge/tests/faults.rs` fails if anything
  leaks.
- **Reading a repository asks GitHub whether this person may.** A session says who somebody is, not
  what they may see. `ready_to_change` and `found_for_reading` authenticate before they admit a
  repository exists, so an anonymous request cannot use a 404 to map the estate.
- **`queue_for` creates a queue; `queue_on` finds one.** A read path that calls `queue_for` turns a
  typo in the address bar into a queue row.

## Naming

The product is `goat-merge`. Inside GitHub it is not.

| Where | Name |
|---|---|
| product, CLI, binary | `goat-merge` |
| GitHub App | named on GitHub by whoever sets it up |
| check run | `Merge Queue` |
| label | `merge-queue` |
| config file | `.github/merge-queue.yml` |
| candidate branch | `merge-queue/candidate-*` |
| fork workflow | `.github/workflows/merge-queue-fork.yml` |

The App's name is not ours to set. The manifest leaves it out so GitHub can ask for it and say
there and then whether it is taken — App names are unique across the whole of GitHub, so a name
we chose would be refused for most people. It is the one name in this table a repository sees
that we do not control, so the setup page says what it is for rather than filling it in.

The App subscribes to `pull_request`, `pull_request_review`, `check_run`, `check_suite`,
`status` and `push`, and to nothing else. GitHub refuses a manifest that asks for an event the
requested permissions do not cover, so an event nobody handles is not a harmless extra — it
stops the App being created at all.

Nothing a repository sees carries the word `goat`. Do not add it.

## Adding things

- **A GitHub call** is a method on `Github` in `crates/goat-merge/src/github/calls.rs`, and a
  route on the fake in `crates/goat-merge/tests/support/fake_github.rs`. Both, always — a call
  the fake does not know about is a call no test covers.
- **A queue decision** belongs in `crates/goat-merge-core/src/plan.rs`, with a test built from
  a literal `Snapshot`. The engine calls `decide` and does as it is told; it does not reason.
- **A condition a person can hit** is a variant of `Fault`, a sentence, and a row in the table in
  `web/src/entities/trouble/ui/WhatWentWrong.tsx` saying what to offer them. Adding a variant
  without the row does not compile.
- **A screen** is a slice under `web/src/pages` with an `index.ts`.

## Database

sqlx with runtime-checked queries, not the compile-time macros. That keeps `cargo build`
working without a database and keeps `sqlx-cli` off the contributor's path; the store and
engine tests against a real PostgreSQL are what actually catch a wrong column.

Schema changes are a new file in `migrations/`. **Never edit a migration that has run
anywhere** — sqlx records its checksum and will refuse to start.

## Code style

- **No comments.** There are none in this repo and none should be added. If something needs
  explaining, rename it or restructure it until it does not. The one exception is the boundary
  note in `crates/goat-merge-core/Cargo.toml`; leave it there.
- Names read as prose: `already_verified`, `note_where_the_base_is`, `tidy_away`, `give_up`.
  Not `get_x`, `do_y`, `handle_z`, `process`, `manager`, `util`.
- Errors are written to the person reading them: full sentences, naming what to do next.
- No `unwrap` or `expect` on anything that can fail at runtime.
- No `unsafe` — the workspace forbids it.
- TypeScript: no type assertions. No `as`, no `!`. Untyped data enters at exactly one place,
  `web/src/shared/api/client.ts`.

## Testing

- Name a test as a sentence stating what must be true:
  `a_check_that_started_before_the_last_push_to_base_is_not_trusted`.
- The safety rules above each need a test that fails when the rule is broken. That is what
  keeps them true a year from now.
- `crates/goat-merge-core/tests` is the model: everything decided from a literal `Snapshot`,
  no I/O, instant.
- `crates/goat-merge/tests/engine.rs` runs the real engine against a fake GitHub on
  `127.0.0.1:0` and asserts what was merged, not what was logged.
- New behaviour needs a test that would have failed before it. A bug fix needs the test first.

## Commits

`type: description` — one line, lowercase throughout, no scope, no trailing period, no body.

Types: `feat`, `fix`, `chore`, `refactor`, `docs`, `test`, `perf`.

**One commit, one reason to exist** — not one file, not one function. A refactor and a bug fix
are two reasons even when they touch the same line; extracting a function and updating every
call site is one reason even across ten files. When in doubt, split: twenty small commits beat
five big ones.

- Any file whose changes serve more than one reason is staged with `git add -p`, not `git add`.
- Tests commit with the code they test, never bundled into one "add tests" commit.
- Formatting-only changes, pure renames and dependency bumps each get their own commit.
- Every commit should build on its own. Where splitting would break that, prefer the larger
  unit that builds.
- No `Co-Authored-By` or assistant attribution.

## Pull requests

Title: lowercase, under 70 characters, no trailing period. Reuse the first commit's subject
when the branch holds one logical change; otherwise summarise its net intent.

## Before you finish

- `cargo test --workspace` passes, with `DATABASE_URL` set.
- `cargo clippy --workspace --all-targets` prints nothing.
- `cargo fmt` leaves nothing to change.
- `pnpm --dir web typecheck` passes if you touched `web`.
- Anything you could not verify is stated plainly, with the reason.
