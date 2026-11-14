---
name: ship
description: Take a change in mcrs from a finished working tree to a merged pull request - branch, commits, checks, the one review, the pull request, CI, the merge. Load when the owner gives an issue or a pull request with no other instruction, and before you commit, open, update or merge a pull request. Triggers - ship, create PR, open a pull request, babysit the PR, merge to main, get it merged, rebase onto main, CI is red.
---

# From a change to `main`

When the owner gives an issue or a pull request and no other instruction, the job is this whole
sequence. Do not stop between the stages to ask.

Stop only for:

- a design choice that the issue does not settle;
- a check that fails for a reason the change did not cause;
- a merge that the rules below do not allow.

## 1. Branch

- One issue is one branch and one pull request. Cut the branch from the current `origin/main`:
  `git switch -c <type>/<scope>/<slug> origin/main`.
- `<type>` and `<scope>` are those of the pull request title. `<slug>` states the outcome in a
  few words: `ci/workspace/checks-on-every-pr`.
- Work in a worktree of your own. One agent for each worktree and each target directory.

## 2. Commits

- One logical step is one commit. Do the work in the tree, run the checks, commit at the end.
- Messages follow Conventional Commits 1.0.0: `<type>(<scope>): <description>`.
  - `type`: `feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `build`, `ci`, `chore`.
  - `scope`: a crate or a domain area (`registry`, `protocol`, `network`, `worldgen`, `client`).
  - The description is lowercase, imperative, and states the outcome in domain terms.
- The commit that completes an issue carries `Closes #N`, one line for each issue. A commit
  that does a part of an issue carries `Refs #N`.
- Add files by name. Read `git status` before each commit.
- A fix for a review finding or a failed check is folded into the commit it belongs to:
  `git commit --fixup=<commit>`, then `git rebase --autosquash origin/main`.
- No name of the reference code in a message (`docs/clean-room.md`).

## 3. Checks before the push

- `cargo nextest run -p <crate>` for each crate you touched and its direct dependents.
- Once, at the end: `cargo fmt --all`, `cargo clippy --workspace --all-targets`, then
  `cargo nextest run --workspace`.
- Do not run a command again without an edit between the two runs. Read the output you have.
- The `pre-push` hook runs `scripts/hooks/run-checks push`. A failure there is a finding to
  fix, not a reason for `--no-verify`.

## 4. Review

One pass, in a new agent, before the pull request exists: the skill `review-pr`.

## 5. Pull request

- `gh pr create --base main`. The title is in Conventional Commits form and states the outcome.
  The body lists what the change delivers and repeats each `Closes #N`.
- A pull request of more than 100 files, generated data excluded, is split along commit
  boundaries. The review bot skips a larger one.
- Each push to an open pull request can cost a review of the bot. Push when a batch of fixes is
  complete, not after each fix.
- Push a rewritten branch with `--force-with-lease=<branch>:<old tip>`. Read the old tip from
  `git ls-remote`; do not type it.

## 6. CI and review threads

- Wait with one background `gh pr checks <pr> --watch` or `gh run watch <id>`. Never with a
  `sleep` loop, and never with a turn for each poll.
- A red check: read the log, decide whether the change caused it. If it did, fix and fold. If
  it did not (a flaky test, a moved `main`), say so with the log line; rerun once.
- When `main` moves and the branch conflicts or a required check wants a current branch, rebase
  onto `origin/main`. Do not rebase for any other reason.
- Check each review thread against the code. Fix the valid ones and reply with the commit.
  Reply to the others with the reason. Do not accept a suggestion that pushes commits for you.

## 7. Merge

- The repository allows only squash merges: `gh pr merge <pr> --squash --subject "<title> (#<pr>)"
  --body "<body>"`. The squash drops the commit footers, so the body repeats each `Closes #N`
  and `Refs #N` of the commits.
- Merge when every required check is green and no review thread is open.
- After the merge, report the merge commit and stop.

## Not yours

- A release, a tag and a version commit belong to the owner. Do not create them.
