# Captured from github.com

Real answers from the public GitHub API, recorded once so `real_shapes.rs` can check that the
types in `src/github/calls.rs` still fit what GitHub actually sends — without a token, a
network, or a rate limit.

| File | Where it came from |
|---|---|
| `repo.json` | `GET /repos/goat-agent/goat` |
| `pull.json` | `GET /repos/goat-agent/goat/pulls/89` |
| `reviews.json` | `GET /repos/goat-agent/goat/pulls/89/reviews` |
| `checkruns.json` | `GET /repos/goat-agent/goat/commits/{head}/check-runs` |
| `statuses.json` | `GET /repos/goat-agent/goat/commits/{head}/status` |
| `ref.json` | `GET /repos/goat-agent/goat/git/ref/heads/main` |
| `rules.json` | `GET /repos/rust-lang/rust/rules/branches/main` — goat has no rulesets, and a repository that does is the point |

To refresh one, fetch it with `gh api <path> | jq -c .` and overwrite the file. If a test then
fails, GitHub changed something and the adapter has to change with it.
