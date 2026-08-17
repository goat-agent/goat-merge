# goat-merge-core

The queue's reasoning. Given a snapshot of what GitHub says about a repository, decide what
should happen next.

## The boundary

No network, no database, no async, no clock, no logging. Every input arrives as an argument
and every decision leaves as a return value. The one comment in this crate is the note in
`Cargo.toml` guarding that; leave it there.

The reason is not tidiness. Merge safety is a set of claims about ordering and staleness that
are miserable to test through a live GitHub and trivial to test here. If a decision moves into
`crates/goat-merge`, it stops being provable.

Time enters as data. Functions that compare timestamps take them as arguments; nothing here
asks what time it is.

## Shape

- `state` — where a pull request is, why it waits, how it ended
- `config` — `.github/merge-queue.yml`, parsed and validated
- `snapshot` — what the adapter observed about a pull request, its base, and its checks
- `readiness` — may this pull request enter the running queue, and if not, why
- `plan` — what to do next: nothing, merge now, build a candidate, discard and redo

`plan` is the only thing the engine calls in anger. Everything else exists to make it correct.

## Rules

The safety rules in the root `AGENTS.md` are mostly claims about this crate. Three live nowhere
else:

- **The fast path compares against the last push to the base, not against "recently".** A
  required check is evidence only about the base that existed when it finished. `Snapshot`
  carries when the base last moved for exactly this comparison.
- **A plan never merges and verifies in the same step.** Verification produces a recorded
  result; merging consumes one. Collapsing them is how a stale verification gets used.
- **A plan is about one candidate.** When several are in flight, the one that merges is the only
  one that acts in that pass; the others are looked at again against a base that has actually
  moved.

## Testing

Everything here is testable with a literal `Snapshot`. There is no excuse for an untested
branch. Name each test as the sentence it proves.
