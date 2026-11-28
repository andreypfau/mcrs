---
name: review-pr
description: The one review pass of a branch in mcrs before its pull request - simplicity, tests, style and correctness in one new agent, then verify each finding and apply the valid ones. Load when the owner asks for a review of a branch, a diff or a pull request, and before you open a pull request. Triggers - review, code review, review the PR, simplify, check for bloat, over-engineering, review before merge.
---

# Review of a branch

One pass for each pull request, after the last commit of the change. Do not review an
intermediate step. A second round needs the owner's word.

The pass includes the simplify lens and the verbosity lens. Do not run another review skill
(`simplify`, `code-review`, `chisle-review`, `chisle-audit`) or a second reviewer beside it.

## Who reviews

A new agent that did not write the code. Give it the diff range, not your conclusions:

```text
Review `git diff origin/main...HEAD` in <worktree>. Load the skills rust-style and
efficient-testing; load bevy-ecs when the diff adds or changes a component, a resource, a
message or a system. Read CLAUDE.md. Read issue <N> and the specification rules <ids> that
the change implements, and work out the expected result before you read the diff. Apply every lens of the section "What the reviewer looks
for" of the skill review-pr, the simplify and verbosity lenses included. You are the only
reviewer. Do not edit files.
Do not run the workspace test suite. Report findings only, most severe first: file, line, the
defect in one sentence, the input or state that shows it, the smallest fix. No praise and no
summary of the change.
```

## What the reviewer looks for

1. **Correctness.** A wrong result, a panic in library code, an allocation or a wait on a hot
   path, state that two owners write, a difference from the specification rule the code cites.
2. **Less code (simplify).** A type, a parameter, a trait or an option that no caller or test
   needs. An abstraction with one implementation. Code that the standard library, Bevy or an
   existing crate of the workspace already gives (reuse). A compatibility path beside the new
   one. Code at the wrong altitude: a special case that a general rule one level up removes,
   or a general mechanism for one case.
3. **Tests.** The criteria of `efficient-testing`. A test that can not fail because of our
   code, that repeats another test, or that builds a copy of the system it tests. A changed
   behaviour with no test that fails without the change. Fixtures larger than the cases need:
   flag each added fixture file of more than 200 lines, and name the smaller case.
4. **Style and layout.** The checklist of `rust-style`.
5. **Verbosity.** A comment outside `examples/`, or a document paragraph that restates the
   code or its name, or tells the story of the task.
6. **Clean room.** A name of the reference code in code, comments, tests, notes or the commit
   message (`docs/clean-room.md`).

## What you do with the findings

- Check each finding against the code before you act on it. A reviewer is often wrong.
- Apply each valid finding. Fold the fix into the commit it belongs to
  (`git commit --fixup=<commit>`, then `git rebase --autosquash <base>`).
- Give each rejected finding one line with the reason, in your report to the owner.
- Run the scoped tests of the crates the fixes touch.
