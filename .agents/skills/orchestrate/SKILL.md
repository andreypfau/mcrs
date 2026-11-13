---
name: orchestrate
description: How to split work between agents in mcrs - one item per agent, the brief template, when to start a new agent instead of continuing one, when to hand off a session, how to wait. Load before you start a second agent, before you send a follow-up to an agent, and when a context passes 200k tokens. Triggers - orchestrate, delegate, subagent, brief, fan out, parallel agents, hand off, continue agent.
---

# Agents and context in mcrs

The cost of a call is the size of its context. An agent that works at 600k tokens pays about ten
times more for each call than a new agent. Measured on this repository: a small fix costs $0.3
to $1 in a new agent and $8 to $19 in an agent that already holds 600k tokens.

## Size of a job

- One agent gets one item: one change, one crate or one specification section.
- A brief that contains "and then" is two briefs.
- A job that builds, runs and measures in a loop is three jobs: the code path, the check, the
  tuning. Each one starts from the commit of the earlier one.
- Mechanical work (a rename, a file move, a list of fixes that is already written) goes to
  Sonnet. See the next section.

## Model and effort

Raise the effort before you change the model. Above `high`, one more effort level costs as much
as a larger model: use the next model at `medium` instead of `xhigh` or `max`.

| Job | Model | Effort |
| - | - | - |
| Main session: design, crate map, briefs, verification of review findings | Opus | `medium` |
| Main session: a large migration, a layer split, a long run without the owner | Fable | `medium` |
| `spec-author` | Opus; Fable for a large domain | `high` |
| `implementer` | Opus | `medium`, then `high` |
| `reviewer` | Opus | `medium` |
| `Explore`, `log-reader` | Haiku | default |
| Mechanical work, `mcrs-update-docs`, CI watch in `ship` | Sonnet | `medium` |

- Set the model of one job with the `model` parameter of the agent call. Do not edit the agent
  file for one job.
- A small model gets only work with an output that is easy to check. A code edit with written
  steps (a rename, a file move, a written list of fixes) goes to Sonnet. A code edit that needs
  a decision stays on Opus or Fable. Measured on one issue: Sonnet as `implementer` used as many tokens as
  Opus, left a path unfixed and wrote a test that passed without the fix.
- When an agent fails twice on the same item at `medium`, start a new agent with the same
  model at `high`. When that agent fails too, start one with the next model at `medium`. Do not
  raise the effort of a model above `high`.

## Start a new agent, do not continue one

- Continue an agent only to answer its question.
- A new item goes to a new agent, with a hand-off: the commit, the files, the open point.
- Do not send a new item to an agent that is above 200k tokens. Do not wake such an agent after
  an idle hour: the cache is gone and the whole context is written again.
- A fork inherits the whole context of its parent. Use a fork only when the job needs that
  context. An independent job gets an ordinary agent and a brief.

## The brief

```text
Goal: <one change, one sentence>.
Worktree and branch: <path>, <branch>. Base commit: <sha>.
Files: <paths with the line ranges you already know>. Do not inventory them again.
Read first: <at most two documents, with section names>.
Rules: load the skills rust-style and efficient-testing. <bevy-ecs when the job designs state.>
Sources: <implementer: the specification only, never the reference code; a gap is a question to me.>
Check: cargo nextest run -p <crate>. No workspace-wide run. No benchmark.
Done when: <the result that someone can observe>. Commit once. Do not push.
Stop and hand off if: the context passes 200k tokens, the check fails twice for the same
reason, or the change needs a file that is not in the list.
Report: commit, files changed, the tail of the check output, open points. At most 30 lines.
```

- Give the facts you already have: the file, the line, the constant, the decision. An agent
  that must find them again pays for the search a second time.
- An agent does not see the skills that its parent loaded. Name each skill in the brief, or use
  an agent type that loads them.
- A brief for a specification says: no class, method or field name of the reference code, and no
  path or line number of it, in the text. See `docs/clean-room.md`.

## Before parallel work

- Show the owner the crate map, the public names and the list of steps. Start the implementers
  after the owner agrees. A design that the owner has not seen is not built in parallel.
- Agents that run in parallel do not edit the same files. Order the steps that share a file.
- Do not stack pull requests while another agent merges a rename across the workspace. Merge
  the rename first.
- One agent for each worktree and each target directory.

## Review

- One review for each pull request, after its last commit, in one new agent. See the skill
  `review-pr`.
- Do not review an intermediate step. A second review round needs the owner's word.

## Waiting

- After you start background agents, end the turn. The notification wakes you.
- Do not poll with `sleep`. Do not write a status turn for each notification.
- Do not leave a question to the owner open in front of agents that run overnight. Take the
  recommended option, record it, and continue.

## The main session

- At 200k tokens, write a prompt for a new session (the skill `handoff`) and stop. Do not
  compact and continue.
- Findings from another session go into a new session with the findings file, not into the
  session that wrote the code.
