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

## Where these rules do not apply

Tables, code blocks, JSON and NBT examples, and text quoted from the game keep
their own form. Quoted text includes a translation, a log line and an error
message. Headings, conventions and notes use the same rules as the body.
