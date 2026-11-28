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
- a defect of the reference that a player does not meet in normal play, unless the owner keeps
  it as a rule of the kind quirk;
- each separate input that a vanilla client does not send. Write one rule for the class of such
  input.

Clients, data packs and save files depend on some accidents of the reference. Only the owner
decides to keep such an accident. The rule then has the kind quirk (section "Semantics, not accidents").

## Questions before the reference

1. Write the behaviour that a player expects. Use the public documentation, the data pack and
   the artefacts of the game.
2. List the questions that these sources do not answer.
3. Read the reference only to answer a question of the list.

A fact of the reference that answers no question of the list does not go into the specification.

A new specification states the behaviour that one implementation issue needs. The next issue
extends it.

## Experiments

An experiment is a short description of a run against the game. It is the provenance of a rule. Write
the steps in the imperative and the observed result in the present tense. Raw output goes to
a test fixture, never into the specification.

These tools run an experiment against the game:

- `tools/vanilla-oracle` calls the game with no server and writes its output as a fixture.
- `tools/mcrs_minecraft_client_recorder` records a vanilla client that the input agent of
  `tools/vanilla-client` plays.
- `tools/vanilla-server` starts the vanilla server for a script.

## Unresolved

A point that is not decided goes into the section "Unresolved" of the document. The label
`unresolved` refers to that section. Do not list an open item in another section.
A specification is ready when "Unresolved" is empty and no rule carries `source_suggested`.

## Forbidden in a specification

- A captured result, transcript or run log.
- An experiment that has not run.
- A "Milestone" field.
- Text about our code: a crate, a type or a file name, an implementation plan, a hook,
  parallelism, cost, "this server" or a plan for a parity test.
- A status record, an issue row or a history.
- A snapshot or protocol header row. The version is in `assets/minecraft/version.json`.

## Semantics, not accidents

The reference implementation and its libraries have defects. A specification states the
behaviour that a player expects. It does not copy an accident of the reference code, because the
next version can change that accident. Each rule is of one of these kinds:

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
- **Deviation**: mcrs differs from the reference on purpose. The rule states the expected
  behaviour, and the reference has a defect or an accident there. The section "Intentional
  deviations" of the document records the reference behaviour and the reason.
- **Quirk**: mcrs reproduces an accident of the reference, because an observer depends on it. The
  section "Known vanilla quirks" of the document records the observer.
- **Unspecified**: an implementation selects the behaviour. A parity test must not compare it.

The kinds wire, client agreement, semantic and layout state the expected behaviour, and the
reference agrees with them.

Deviation is the default kind for an accident of the reference. A rule has the kind quirk only
when one of these conditions is true:

- The vanilla client computes or predicts the same result, and a different result from the
  server gives the player a visible error, for example a setback.
- A data pack or a save file depends on the result.

Only the owner decides that a rule has the kind quirk. The entry of the quirk cites the issue of
that decision.

When the exact order or tick of a result has no meaning for a player, the rule has the kind
unspecified. The rule then states a bound, for example "at least once in 40 ticks".

A new or revised rule states its kind on its provenance line, for example
`Provenance: source_suggested. Kind: semantic. Confidence: medium.` An older rule keeps
its kind until a change touches the rule.

A deviation inside a grammar that the vanilla client parses can make the server stricter than
the client. The server can reject such input with a syntax error. The server must not accept
input that the client rejects. The server must not give a different meaning to input that both
accept.

An entry in "Intentional deviations" has these parts:

- **Rule**: the id of the rule that states the expected behaviour.
- **Reference**: the behaviour of the reference implementation.
- **Reason**: why mcrs differs from the reference.
- **Visible to the client**: yes or no, and what the player sees.
- **Bug reference**: the id of the bug report of the game, for example `MC-12345`, or `none`.
- **Test**: what a differential test does when it finds this difference.

An entry in "Known vanilla quirks" has these parts:

- **Rule**: the id of the rule that states the accident.
- **Observer**: the vanilla client, the data pack or the save file that depends on the accident,
  and what becomes wrong for it without the accident.
- **Decision**: the issue in which the owner decided to keep the accident.
- **Bug reference**: the id of the bug report of the game, or `none`.

For a rule of the kind deviation, do not write an experiment that only confirms the accident. Put
the observation in the deviation entry. A rule of the kind quirk gets an experiment as each other
rule does.

## Requirements that each specification inherits

Each specification inherits these requirements. A specification does not repeat them. A rule that
differs from one of them cites the issue in which the owner decided the difference.

- **Atomic file write**: a reader must see the old file or the new file, never a part of a file.
  A write of the content to a temporary file in the same directory, and then a rename of the
  temporary file to the target name, give this result.
- **Failed write**: when a write fails, the operation must report the error to its caller or in a
  message that a user can see. The operation must not continue as if the write succeeded. The
  old file must stay as it was.
- **Name case**: each rule that compares names must state whether the comparison ignores case. A
  comparison of player names must ignore ASCII case.
- **Configuration**: a user must configure a behaviour through the plugin that adds the
  behaviour. The behaviour must not read its configuration from an environment variable or from
  a file in the working directory.

## Where these rules do not apply

Tables, code blocks, JSON and NBT examples, and text quoted from the game keep
their own form. Quoted text includes a translation, a log line and an error
message. Headings, conventions and notes use the same rules as the body.
