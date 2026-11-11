# Specifications

Each document in this directory states the behaviour of one domain. Write these
documents in English, according to the rules of ASD-STE100 Simplified Technical English. The ASD-STE100 specification is free on request from
[asd-ste100.org](https://www.asd-ste100.org).

This file lists the rules that apply here and how this project adapts them.

## Words

- Use one word for one concept. When a document uses a term, use that term every
  time. Do not use a synonym to make the text less repetitive.
- You can use technical names freely. A technical name is one of these:
  - a term that the document defines in its Terms section;
  - a data or protocol name that the document allows;
  - the text of a code span.
- Define each term of the domain one time, in the Terms section.
- Do not put more than three nouns in a row. Use a preposition or a code span to
  break a longer noun cluster.
- Use a verb to show an action. Write "the server validates the pack", not "the
  server does a validation of the pack".
- In a rule, write a requirement with "must" or "must not". Write a permission or
  a possibility with "can". Do not use "shall", "should" or "may".

## Verbs

- Use the simple present tense for a rule: "The server sends the packet."
- Use the imperative for a step of an experiment or a conformance test: "Add the
  file to the pack."
- Use only the simple tenses: present, past and future.
- Use the active voice. Use the passive voice only when the agent is not known or
  is not important.
- Do not use the -ing form of a verb, except in a technical name. Write "the
  entries that remain", not "the remaining entries".

## Sentences and paragraphs

- Each sentence has one topic.
- A procedural sentence has a maximum of 20 words. A descriptive sentence has a
  maximum of 25 words.
- Write one instruction in each step. Put two actions in one step only when they
  occur at the same time.
- A paragraph has one topic and a maximum of six sentences.
- Do not omit words to make a sentence shorter. Keep the articles, "that" and the
  verb.
- Use a vertical list for a condition with more than two parts and for a sequence
  of steps.
- When you count words, a code span, a number with its unit and a rule id each
  count as one word.

## A rule needs an observer

A specification is not a description of the reference. A behaviour has no rule until a
specification states it. Write a rule only when one of these observers can see a difference:

- the vanilla client: the bytes on the wire, or a result that the client predicts;
- a player in normal play: a result in the world, in the GUI or in a message;
- a data pack or a save file: a field, a default value or an error;
- a user of the library: a public message or a plugin point.

The text of the rule states what that observer sees. Do not write a rule for these:

- the order of internal operations that no observer sees;
- the method of a computation, when each method gives the same result to the observer;
- a defect of the reference that a player does not meet in normal play;
- each separate input that a vanilla client does not send. Write one rule for the class of such
  input.

Players build on some defects of the reference. Only the owner decides to keep such a defect. The
rule then states the defect as the correct behaviour, and it cites the issue of that decision. The
rule has the kind of its observer, and it has no entry in "Intentional deviations".

## Questions before the reference

1. Write the behaviour that a player expects. Use the public documentation, the data pack and
   the artefacts of the game.
2. List the questions that these sources do not answer.
3. Read the reference only to answer a question of the list.

A fact of the reference that answers no question of the list does not go into the specification.

A new specification states the behaviour that one implementation issue needs. The next issue
extends it.

## Experiments

An experiment is a sequence of steps that a tool or a script runs with no person. When a tool for
the domain takes an input file, the experiment is such a file. Run each experiment that a tool can run
before the implementation starts. The report of the author lists each experiment that did not run.
These tools run an experiment against the game:

- `tools/vanilla-oracle` calls the game with no server and writes its output as a fixture.
- `tools/mcrs_minecraft_client_recorder` records a vanilla client that the input agent of
  `tools/vanilla-client` plays.
- `tools/vanilla-server` starts the vanilla server for a script.

A capture records the game version. A capture of a different version than
`assets/minecraft/version.json` does not confirm a rule.

## Review of a specification

A specification gets one review. A second review needs the word of the owner. The reviewer asks
these questions for each rule:

- Which observer sees the result?
- Which artefact or experiment decides the rule?
- Does the implementation become wrong for an observer when the rule is removed?

The reviewer removes a rule that has no observer. The reviewer does not search the reference for
behaviour that the specification does not state. An omission becomes known as a question of an
implementer or as a failed experiment. The answer goes into the specification.

A point that is not decided goes into the section "Unresolved" of the document. The label
`unresolved` refers to that section.

## Semantics, not accidents

The reference implementation and its libraries have defects. A specification states what the
behaviour means. It does not copy an accident of the reference code, because the next version
can change that accident. Each rule is of one of these kinds:

- **Wire**: bytes, ids and codecs that the vanilla client decodes. The rule is exact. A choice
  that the server makes inside the format, for example the order of node numbers, is not wire
  if the client accepts each valid choice.
- **Client agreement**: the vanilla client computes the same result from the data that the
  server sends, for example the parse of a command line. The server must agree with the client
  on the input that a player types in normal use.
- **Semantic**: the meaning of the behaviour, for example the result in the world or the
  message that a player receives. The rule does not state the order of internal operations,
  the class of an internal error or the text of a log line.
- **Layout**: a position, a size or a colour of the client GUI, in GUI units and ARGB values.
  A screenshot at a known GUI scale tests the rule.
- **Deviation**: a defect or an accident of the reference. The rule states the correct
  behaviour. The section "Intentional deviations" of the document records the reference
  behaviour.
- **Unspecified**: an implementation selects the behaviour. A parity test must not compare it.

A new or revised rule states its kind on its provenance line, for example
`Provenance: source_suggested. Kind: semantic. Confidence: medium.` A rule that corrects a
deviation has the kind of the correct behaviour, and its entry in "Intentional deviations"
names the rule.

A deviation inside a grammar that the vanilla client parses can make the server stricter than
the client. The server can reject such input with a syntax error. The server must not accept
input that the client rejects. The server must not give a different meaning to input that both
accept.

An entry in "Intentional deviations" has these parts:

- **Rule**: the id of the rule that states the correct behaviour.
- **Reference**: the behaviour of the reference implementation.
- **Visible to the client**: yes or no, and what the player sees.
- **Test**: what a differential test does when it finds this difference.

Do not write an experiment that only confirms an accident. Put the observation in the
deviation entry.

## Where these rules do not apply

Tables, code blocks, JSON and NBT examples, and text quoted from the game keep
their own form. Quoted text includes a translation, a log line and an error
message. Headings, conventions and notes use the same rules as the body.
