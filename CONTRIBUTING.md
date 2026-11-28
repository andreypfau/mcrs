# Contributing

mcrs is built by a small team. Review time is our bottleneck, not ideas, code or tokens. A large
pull request takes more effort to review than to generate. These rules protect that review time.

The team is the logins in [.github/CODEOWNERS](.github/CODEOWNERS). The rules below apply to every
pull request from outside the team, drafts included. A pull request that follows the rules can still
be closed. We do not owe a review or a merge.

AI agents are welcome as tools. The person who opens the pull request is its author and answers for
every line of it.

## Before you write code

- Work from an open issue. Read its Decision and its specification. A small fix for an obvious bug
  needs no issue.
- Ask in the issue to take it, and wait for an answer. An issue with an assignee is taken.
- A new game behaviour starts with a specification, not with code. See
  [docs/clean-room.md](docs/clean-room.md).
- Search the open pull requests. A test and a comment on an existing pull request are better than a
  second pull request for the same problem.

## Rules for a pull request

### One problem

Fix one issue. Include every change that the fix needs: code, tests, docs. A rename, a cleanup, a
refactor and a second fix go in their own pull requests.

### Small

- At most 1000 added lines, generated data excluded.
- One crate group, such as `mcrs_minecraft_worldgen_*`.
- No new crate, no new dependency, no new subsystem without a published RFD. See
  [docs/rfd/0001-requests-for-discussion.md](docs/rfd/0001-requests-for-discussion.md).
- No change to `.github/`, `xtask/`, `.config/`, `clippy.toml` or `AGENTS.md` unless the issue asks
  for it.

Split a larger change into a stack of pull requests, and agree on the split in the issue first.

### Show that it works

Say what you changed, how you checked it, and what you saw. Paste the commands and their actual
output. "Tests pass" is not evidence. A change to the client, the renderer or the GUI needs a
screenshot before and after.

### Say which models you used

In the section "Models" of the pull request template, list the models you used and what each one
did, for example code, tests, a specification, a review or the pull request text. Write "none" if
you used no model. This helps the review.

Answer review questions in your own words.

### Keep it mergeable

A pull request that conflicts with `main` for 7 days with no activity is closed.

## When your pull request is closed

A pull request that breaks a rule gets a comment that names the rule. It is closed 72 hours later
unless you fix it. After a close, fix the problem and comment on the pull request, or open a new
one that links the old one.

## Bans

An account that floods the repository with automated pull requests or issues is banned. An account
that makes review harder, for example with repeated pings, a fight over a close, or a second
account to get around a limit, is banned.

## More

The full guide is at [mc.rs/docs/en/contributing](https://mc.rs/docs/en/contributing/).
Questions go to the issue you work on or to [Discord](https://discord.gg/ged3nMRzwG).
