# goat-merge

A self-hosted merge queue for GitHub. Pull requests land in a verified order, on a base
they were actually tested against, and nobody has to sit and watch them.

Add a label. Walk away. The queue does the rest.

## Why

Two things go wrong when a team merges by hand.

**`main` breaks even though every pull request was green.** A and B each passed their checks
against yesterday's `main`. A renames a function, B adds a call to the old name. Neither
conflicts. Both merge. `main` no longer builds.

**Merging costs an afternoon.** With *require branches to be up to date* switched on — the
setting that stops the first problem — every merge means clicking Update branch, waiting for
CI, and coming back. If somebody beats you to it while you were at lunch, you do it again.

goat-merge takes both away. It verifies each pull request against the `main` it will actually
land on, in order, and merges it for you.

## What it does

- **Nothing lands untested against the code it lands on.** Every pull request is verified
  against the current base, and the base and head are checked again in the moment before the
  merge. If either moved, the result is thrown away and the work is redone.
- **Merging is instant when nothing is in the way.** If no commit has landed on the base since
  the last required check started, that check already tested the tree that is about to land,
  so there is nothing left to run. On a quiet repository the queue costs you nothing.
- **A busy queue does not make everybody wait their turn one at a time.** Several pull
  requests go onto one candidate and one CI run decides all of them. If it passes they all
  land; if it fails, fewer travel together next time until the one at fault is on its own and
  leaves. Nobody is thrown out of the queue for somebody else's failure. The queue starts at
  one and only takes on more as the repository proves it can, so a flaky day shrinks it back
  by itself.
- **You can see why something is waiting.** Every decision names its reason — in the pull
  request, on the web console, and from the CLI.
- **Pull requests from forks are not quietly trusted.** Copying a stranger's commits onto a
  branch in your repository is how their test suite ends up holding your deploy keys.
  goat-merge refuses to do that unless the repository has declared a workflow that runs fork
  code without secrets.

## What it does not do

It does not run your CI, analyse it, hunt flaky tests, create stacked pull requests, backport
anything, automate labels and reviewers, or review your code. Those are other products. This
one merges.

## Requirements

- A GitHub App you create during setup. Setup asks you nothing; you name it on GitHub and
  choose there which account and repositories it goes on
- PostgreSQL
- An address GitHub can reach over HTTPS

Reverse proxy, tunnel, load balancer — pick whichever you like. goat-merge asks for an address
and nothing else about your infrastructure.

## Running it

```sh
cp .env.example .env
# fill in GOAT_MERGE_PUBLIC_URL, and put `openssl rand -hex 32` in GOAT_MERGE_MASTER_KEY
docker compose up -d
```

Then open `/setup` and follow it:

1. **Create the GitHub App.** GitHub shows you a page with the permissions and webhook already
   filled in, and asks you for a name.
2. **Install it** on the account the repositories are under — yours or an organization — and
   pick the repositories you want a queue on. Come back here to add another account later.
3. **Switch the queue on.** goat-merge checks the branch first and tells you what is missing.

The App is registered as public so that one server can serve repositories in more than one
organization. That does not open it up: an installation only counts if somebody started it
from this server, and a webhook for any other installation is dropped.

The last step matters. goat-merge will tell you if the `Merge Queue` check is not a required
check on the branch, because until it is, anybody can merge around the queue. It will not
change your protection rules for you.

## Using it

Add the `merge-queue` label to a pull request. That is the whole developer workflow.

The `Merge Queue` check then shows where it is and why:

```
Waiting    approvals: 1 of 2
Queued     2 pull requests ahead
Checking   6/8 checks passed
Merged     merged as 4a91c0e
```

From a terminal:

```sh
goat-merge login https://merge.example.com

goat-merge queue                       # what is in the queue right now
goat-merge enqueue                     # this branch's pull request
goat-merge explain                     # why it is where it is
goat-merge retry 4835
goat-merge queue pause
goat-merge expedite 4901 --reason "production incident"
goat-merge config validate
goat-merge config simulate 4835        # what a config change would do, before you push it
```

## Configuring a repository

Nothing is required. With no configuration file, goat-merge follows the repository's existing
rulesets and branch protection.

When you want more, `.github/merge-queue.yml`:

```yaml
version: 1

queues:
  - branch: main
    merge_method: squash
    batch_size: 5
    speculate: 2
```

`batch_size` is the most pull requests to put on one candidate. It is a ceiling, not a target
— the queue works up to it and back down on its own. Set it to `1` to verify one at a time.

`speculate` is how many candidates may have checks running at once. It is `1` — off — unless
you raise it. At `2` the queue builds the next candidate on top of the one in front instead of
waiting for it to land, so the second batch's checks are already finished when the first one
merges. It costs a CI run every time the one in front fails, because the one behind was
verified on a tree that will never exist, so it is thrown away. Raise it if your checks are
slow and reliable and you have runners to spare; leave it alone otherwise.

Everything it accepts is in [schema/merge-queue.schema.json](schema/merge-queue.schema.json).
Point your editor at it for completion and inline errors.

## Fork pull requests

If the repository takes pull requests from forks, copy
[.github/workflows/merge-queue-fork.example.yml](.github/workflows/merge-queue-fork.example.yml)
to `.github/workflows/merge-queue-fork.yml`. Until you do, fork pull requests show as
`Blocked` and are never merged.

## Building

```sh
cargo build --release
```

The console is built into the binary. `pnpm` is needed for that; set
`GOAT_MERGE_SKIP_WEB_BUILD=1` to skip it.

## Contributing

Read [AGENTS.md](AGENTS.md).

## Licence

Apache-2.0.
