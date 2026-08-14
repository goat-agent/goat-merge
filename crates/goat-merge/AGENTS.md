# goat-merge

Everything that touches the outside world: HTTP, webhooks, GitHub, PostgreSQL, the CLI.

The decisions are not here. `goat-merge-core` says what should happen; this crate makes it
happen and writes down what it did.

## Shape

- `settings` — what the environment has to supply, and the message when it does not
- `store` — PostgreSQL. One module per concern, all hanging off `Store`
- `github` — the API client, App auth, webhook signatures, and `look`, which turns GitHub's
  answers into a `Snapshot` the core can reason about
- `engine` — `tend` is the whole loop: read GitHub, ask the core, do as it says. `react`
  turns a delivery into a scheduled tend and nothing more
- `api` — the routes the console and CLI call
- `web` — serves the console out of the binary
- `cli` — everything except `run`, talking to a server over HTTP
- `serve` — wiring, the worker, the periodic sweep, graceful shutdown

## Rules

- **`tend` is the only thing that changes a queue.** Webhooks, the API and the CLI all end in
  `ask_for(TEND, …)`. Nothing else merges, builds a candidate, or settles an entry.
- **`tend` reads before it decides.** Branch tip, repository settings, protection,
  configuration, then every entry. Cheap enough, and it is the only way the state can be
  trusted after a missed webhook.
- **The GitHub client is one object with one way in.** New calls go through `call`, so
  installation tokens, headers and error shapes stay in one place.
- **A `GithubError` that `is_worth_another_try` must be returned, not swallowed.** The job
  comes back with a longer wait. Swallowing it settles a pull request over a blip.
- **Secrets are sealed before they are written.** The App private key, webhook secret, client
  secret and OAuth tokens all go through `Seal`. Nothing readable lands in a column.
- **Permission is asked per request.** `must_be_at_least` re-reads the caller's standing from
  GitHub. There is no role table here and there should not be one.

## Gotchas

- **`hold_the_branch` returns a transaction that must stay alive for the whole tend.** The
  advisory lock lives on that connection; the writes go on others. Dropping it early lets two
  workers verify the same queue.
- **`merge_into_branch` answers 204 with no body when there is nothing to merge.** That
  deserialises to `None`, and the candidate is then the base itself. Do not treat it as an
  error.
- **The check run must be published as `success` before the merge call, not after.** It is a
  required check, so GitHub refuses the merge while it is pending.
- **`Path` extractors and locks do not mix with `await`.** Holding a `std::sync::MutexGuard`
  across an await makes a handler non-`Send` and the error message will not say so.
- **`base_moved_at` is only correct because we write it every time we see the tip.** Anything
  that learns the tip must go through `note_where_the_base_is`.
