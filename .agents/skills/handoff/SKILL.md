---
name: handoff
description: Write the prompt that lets a new session or a new agent continue the current work in mcrs. Load when the owner asks for a prompt for a fresh session, when a context passes 200k tokens, or before you replace a long-lived agent. Triggers - handoff, hand off, prompt for a new session, fresh session, continue in a new chat, context is full.
---

# Hand-off prompt

Write the prompt to a file outside the tracked tree, and print the path. The reader has none
of this conversation. The prompt holds only what the reader can not get from the repository.

## Contents, in this order

1. **Goal.** One or two sentences: the result and who needs it.
2. **State.** The worktree, the branch, the last commit, the pull request and the issue. What
   is committed, what is in the working tree and not committed, what is pushed.
3. **Decisions.** Each decision the owner made in this session, with its reason in one line.
   Include the options the owner rejected, so that the reader does not propose them again.
4. **Next step.** The first action, the files with line numbers, and the check that proves it.
5. **Done when.** The result that someone can observe, and where to stop.
6. **Open points.** Questions without an answer, checks that fail and why, and work that
   another agent or another pull request blocks.

## Rules

- Give paths, commits and numbers. Do not tell the story of the session.
- Do not copy a specification, a design note or `CLAUDE.md` into the prompt. Name the file and
  the section.
- Quote the owner's words for a requirement that is easy to weaken.
- State what you did not verify.
- At most 60 lines. A longer prompt means the work is more than one session: split it.
