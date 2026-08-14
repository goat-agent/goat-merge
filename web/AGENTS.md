# web

The console. Vite + React 19 + TypeScript + Tailwind v4, built into the server binary by
`crates/goat-merge/build.rs`.

It is an operations console, not a dashboard. Somebody opens it because a pull request is
stuck. Everything else is secondary.

## Layers

Feature-Sliced, imports go downward only and through a slice's `index.ts`:

```
app → pages → widgets → features → entities → shared
```

## Rules

- **Every screen answers "why is this waiting, and what do I do".** Numbers that answer no
  question do not go on a screen.
- **No literal colours, sizes or spacing in a component.** Everything through
  `src/shared/config/tokens.css`.
- **Purple is progress and selection only.** Success, warning and failure are green, amber and
  red, and every one of them carries an icon and words as well as a colour.
- **The queue is a list, in order, and the order is not draggable.** Jumping the queue happens
  through `Expedite`, which demands a reason and writes it down.
- **Untyped data enters at `src/shared/api/client.ts` and nowhere else.** No `as`, no `!`. An
  answer is only a `Row` once a reader in `src/shared/api/types.ts` has looked at every field; a
  200 that is not the shape we expect is `answer_was_not_json`, not an empty queue.
- **`shared` may not know what a queue is.** The layer check in `biome.json` only sees import
  paths, so it cannot catch a component in `shared/ui` that switches on a domain vocabulary — that
  is why `WhatWentWrong` lives in `entities/trouble` beside `Standing`.
- **The server writes the sentence; the console picks the control.** `Trouble.message` is rendered
  as it arrived, and `where` is followed as given — the console does not know the route to a
  repository's settings on GitHub, and should not learn it.
- **A row that is on screen stays where it is when the data refreshes.** Live updates must not
  move what somebody is reading, and a refresh that fails must not remove it either. `useAsked`
  enforces both: `about` identifies the thing and clears the answer when it changes, `when` is the
  beat and keeps it.

## Gotchas

- **`useAsked` takes two lists for a reason.** `about` is what is being looked at, `when` is the
  beat. Putting a poll counter in `about` throws the answer away fifteen times a minute; putting
  the pull request number in `when` shows one pull request's timeline under another's title.
- **The SSE stream is a hint, not the data.** An event means refetch, and the payload is only
  the repository that changed.
- **`/api/me` answering 401 is the signed-out state, not an error to show.** `App` branches on
  it before anything else renders.
