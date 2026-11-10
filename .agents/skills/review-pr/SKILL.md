---
name: review-pr
description: The one review pass of a branch in mcrs before its pull request - simplicity, tests, style and correctness in one new agent, then verify each finding and apply the valid ones. Load when the owner asks for a review of a branch, a diff or a pull request, and before you open a pull request. Triggers - review, code review, review the PR, simplify, check for bloat, over-engineering, review before merge.
---

# Review of a branch

One pass for each pull request, after the last commit of the change. Do not review an
intermediate step. A second round needs the owner's word.

## Who reviews

A new agent that did not write the code. Give it the diff range, not your conclusions:

```text
Review `git diff origin/main...HEAD` in <worktree>. Load the skills rust-style and
efficient-testing; load bevy-ecs when the diff adds or changes a component, a resource, a
message or a system. Read CLAUDE.md. Do not edit files. Do not run the workspace test suite.
Report findings only, most severe first: file, line, the defect in one sentence, the input or
state that shows it, the smallest fix. No praise and no summary of the change.
```

## What the reviewer looks for

1. **Correctness.** A wrong result, a panic in library code, an allocation or a wait on a hot
   path, state that two owners write, a difference from the specification rule the code cites.
2. **Less code.** A type, a parameter, a trait or an option that no caller or test needs. An
   abstraction with one implementation. Code that the standard library, Bevy or an existing
   crate of the workspace already gives. A compatibility path beside the new one.
3. **Tests.** A test that can not fail because of our code, that repeats another test, or that
   builds a copy of the system it tests. A changed behaviour with no test that fails without
   the change. Fixtures larger than the cases need.
4. **Style and layout.** The checklist of `rust-style`. Comments that restate the code or tell
   the story of the task.
5. **Clean room.** A name of the reference code in code, comments, tests, notes or the commit
   message (`docs/clean-room.md`).

## What you do with the findings

- Check each finding against the code before you act on it. A reviewer is wrong often: in one
  measured round 17 of 23 findings were rejected.
- Apply each valid finding. Fold the fix into the commit it belongs to
  (`git commit --fixup=<commit>`, then `git rebase --autosquash <base>`).
- Give each rejected finding one line with the reason, in your report to the owner.
- Run the scoped tests of the crates the fixes touch. Then continue with the skill `ship`.
