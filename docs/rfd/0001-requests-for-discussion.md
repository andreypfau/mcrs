# RFD 1: Requests for Discussion

- Authors: andreypfau
- State: published
- Labels: process

This page explains what an RFD is in mcrs, when to write one, what goes into it and how it
moves from a question to a decision. Read it before you write your first RFD, or when you
wonder whether a change needs one.

Agents write nearly all of the code in mcrs, and they write it faster than anyone can read it.
Code has become cheap. Knowing why the code is the way it is has not: which alternatives were
considered, and what would have to be true for the choice to be wrong. When that reasoning is
not written down, it lives in a chat session that ends, and the next agent or the next person
starts from the code and guesses.

mcrs writes that reasoning down in a Request for Discussion. The idea and the name come from
[Oxide's RFD 1](https://rfd.shared.oxide.computer/rfd/0001), which takes its spirit from RFC 3:

> The content of a note may be any thought, suggestion, etc. related to the software or other
> aspect of the network. Notes are encouraged to be timely rather than polished. [...] The
> minimum length for a note is one sentence.

The same holds here. An RFD can be a question without an answer, a survey of how others solved
a problem, or a finished design. One sentence is enough when one sentence is what is known.

mcrs differs from Oxide in who does what. Oxide is a company of engineers who each write their
own RFDs and read one another's. mcrs has one maintainer who decides, and agents that research
and write. So an RFD here has two readers. The maintainer makes a decision from it, which
means it has to carry the question, the facts and the options. Everyone who comes later, agents
included, reads it to learn what was decided and why.

## When to use an RFD

These examples are meant to be broad:

- An architectural or design decision, such as how state is owned, how a boundary is crossed
  or how the simulation is partitioned.
- A change to a public API of the library crates, or a new crate, dependency or subsystem.
- A decision to differ from the game on purpose, or to reproduce one of its accidents, when
  the choice shapes more than one rule.
- A change to how the project works: the clean room, the specification style, what a pull
  request must pass to merge, or this process.
- A design for testing or for measuring something.
- Something that was looked into and turned down.

A change that implements a rule a specification already states does not need an RFD, and
neither does a bug fix or a mechanical edit. When you are unsure, ask whether someone will
later wonder why the code is like this and find no answer in the code. If so, write one.

## What goes into an RFD

An RFD that leads to a determination lets its reader make that determination without having
been in the conversation. Oxide's RFD 5 uses the word for the point where a direction is
chosen and committed to. Such an RFD has five parts.

It states the problem in plain words: what is wrong or missing, and for whom.

It says what is known and how. A measurement, an experiment against the game, something read
in a document and an assumption are different kinds of knowledge, and the text says which one
each statement is. A number comes with the command that produced it.

It lays out the viable options with the benefits and drawbacks of each, and it includes the
options that were ruled out, with the reason. A later reader needs the rejected options more
often than the chosen one, because the rejected ones are what somebody will propose again.

It records the determination and who made it.

It says what would reopen the decision. When a decision rests on a measurement, the RFD names
the result that would make it wrong.

Where they apply, consider what the decision does to a developer who takes one crate and
nothing else, to parity with the game, to the tick budget, and to how much code it deletes.

RFDs differ in depth. Some are urgent and thin, and some only record what is known so far. A
two-paragraph RFD that exists is worth more than a thorough one that was never written.

An RFD is easy to confuse with three other documents. A specification in `docs/spec/` says what
mcrs must do. The page of a subsystem in the section "Concepts" of the site describes the system
as it stands today. An issue holds the steps of the work and their status. An RFD holds the
proposal and its reasons, and it can point at the other three. The Decision of an issue names
the RFD and repeats its determination in one line.

An RFD is also the design that comes before the code of a new crate, dependency or subsystem.
It names the crates, their layers and the path of migration. The files in `docs/` that are
called design notes predate RFDs. Each one moves to an RFD or to a page of the site when a task
changes it, and no new one is written.

## How an RFD gets written

The maintainer thinks and decides, and an agent writes.

An RFD usually starts as a conversation. The maintainer describes the problem in whatever
language and at whatever length is natural, and gives concepts and ideas more often than
finished statements. In that conversation the agent researches the options, runs the
measurement or the experiment when one would settle a question, argues against the
maintainer's first idea and against its own, and says what it could not find out. It asks
questions until it understands the idea well enough to state it better than it was first
said. Then it writes the document.

An implementer reads RFDs, so an RFD stays on the implementer's side of the clean room. A fact
about the game enters an RFD as a reference to a rule of a specification or to the result of an
experiment tool, never as a quotation of the reference code.

The document is the agent's formulation of the maintainer's thinking. A transcript or a
translation of what was said is a poor RFD: the agent restructures the ideas, names what was
left implicit, and puts them in an order a stranger can follow. The maintainer then reads the
result and says whether it is what was meant.

Two rules keep that document honest. The text marks whose choice each choice is, so that a
reader can tell what the maintainer decided from what the agent filled in and is proposing.
And the determination is the maintainer's: an agent writes that something was decided only
when it was.

An agent can start an RFD by itself. When work reaches a decision that is not the agent's to
make, the agent stops and writes up the question in its branch. The push and the pull request
wait for the maintainer's word, as RFD 3 says. Picking an answer and burying it in a pull
request is the failure this process exists to prevent.

Before an RFD goes to the maintainer for a determination, a second agent that did not write it
reads it with one instruction, which is to find what is wrong with it. Its strongest
objections go into the section "Objections", each with its answer or with a note that it
stands, so that the maintainer meets the best argument against a proposal before deciding. A
change to the proposal after the pass needs a new pass.

## Metadata and state

An RFD starts with its authors, its state and its labels. Authors are people. They own the RFD
and answer for it.

An RFD goes through the six states that Oxide uses. The first state lives only on GitHub, so
that `main` holds no empty placeholders. From the second state on, the field `State` of the
file holds it.

| State | Where | Meaning |
|---|---|---|
| ideation | an issue with the label `rfd` and no assignee | Only the topic is written down, and nobody is working on it. It marks a decision that will have to be made. |
| prediscussion | the issue has an assignee; a draft pull request holds the file | Research is under way and the text changes quickly. |
| discussion | the pull request is ready for review | The RFD is ready for the maintainer and is being weighed. |
| published | the pull request merges with this state | The determination is made. This is the direction until it is shown to be wrong. |
| committed | the pull request that finishes the work sets it | The idea is built. The RFD now explains how the system came to work the way it does. |
| abandoned | the pull request merges with this state | The idea was turned down. The RFD stays, so that the reasoning is not paid for twice. |

Anyone can write an RFD. A contributor from outside the team takes an issue in ideation the way
`CONTRIBUTING.md` says for any issue: ask in the issue and wait for the answer. An RFD from
outside the team starts only from an issue that the maintainer accepted.

## Life of an RFD

An RFD is a file, `docs/rfd/NNNN-short-title.md`. It is written on its own branch and reaches
`main` through a pull request, so the history of the file is the history of the RFD.

The number is the next free one when the issue gets its assignee, and the issue records it. A
number is never given twice, so a file is never renamed.

The draft pull request opens in prediscussion and becomes ready for review in discussion. It
merges when the maintainer has made the determination and the state says published or
abandoned. An agent's opinion that the discussion is over does not publish an RFD.

The pull request that finishes the work an RFD describes moves it to committed, in the same
change that updates the page of the subsystem on the site. That is also the moment to compare
the RFD with what was built. If a measurement it rested on came out differently, the RFD says
so.

## Changing an RFD

A published RFD can be corrected: a wrong number, a broken link, a clearer sentence. A change
to the determination is recorded and never overwritten. It is a dated note at the end of the
RFD that says what changed and why. When the direction is reversed, it is a new RFD that
supersedes the old one, and each points at the other. The old text stays, including reasoning
that turned out to be wrong, because the next person to have the same idea needs to see why it
failed.

## Changing this process

This RFD describes the process, so the process changes the way any RFD changes. It applies
from the day it merges.

## Options

These options were considered for the record of decisions.

- An issue with a section Decision, as before. It holds the steps of the work well, but it has
  no place for the rejected options, and it closes when the work ends.
- Short architecture decision records of ten lines. They are cheap, but they keep the choice
  and lose the reasoning, which is the part that this process exists to keep.
- GitHub Discussions. A thread holds the argument, but not a text that is corrected and that
  states one determination.

The determination is an RFD in the repository. It is the maintainer's.

## What would reopen this

- RFDs that stay in discussion for months, so that the process blocks work instead of keeping
  its reasons.
- RFDs that nobody reads later: no pull request or agent brief cites one.

## Objections

These are the strongest objections that the text does not already answer.

- An RFD from a contributor outside the team is a gate on any new crate or dependency, and its
  merge has no time limit. It stands. The maintainer accepts the issue first, so the work does
  not start without a signal that it is wanted.
- The claim that rejected options are read more often than the chosen one has no measurement.
  It stands as an assumption.

## References

- Oxide Computer Company. [RFD 1: Requests for Discussion](https://rfd.shared.oxide.computer/rfd/0001).
- Oxide Computer Company. [RFD 5: Phases of Engineering](https://rfd.shared.oxide.computer/rfd/0005).
- Steve Crocker. RFC 3: Documentation Conventions. 1969.
