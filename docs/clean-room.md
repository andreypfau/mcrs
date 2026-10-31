# Clean-room rules

mcrs implements the game's behaviour from specifications, not from the game's code. This page
states who can read what, and what can appear in the repository.

## Two roles

A specification author writes a specification under `docs/spec/`. The author can use any
source, the reference implementation included. A specification states behaviour: inputs,
outputs, order, timing and constants as data. It never states the shape of the reference code:
no class, method, field or variable names, and no structure of a function.

An implementer writes code, tests and design notes. The implementer reads only these sources:

- the specifications under `docs/spec/`;
- public documentation of player-facing behaviour;
- artefacts the game produces: the data pack, the reports under `assets/mcrs/reports/`,
  captured packets, saves, and the outputs of `tools/vanilla-oracle`;
- the code of this repository that these rules allow.

An implementer never reads decompiled, deobfuscated or mapped game code. One person does not
take both roles for the same behaviour.

## What the repository does not contain

- Names of classes, methods or fields of the game's code, in code, comments, tests, design
  notes, commit messages and issues.
- Text that says code is a port, a transcription or a mirror of the game's code.
- A test, a tool or a script that reads the game's code.

The output of the game is evidence and is allowed. A parity test compares our output with an
artefact the game produced.

## Provenance in specifications

Each rule carries a provenance label (`docs/spec/registry.md`, section 0.3). A rule with the
label `source_suggested` is allowed: the author read the reference implementation, and the
rule states only behaviour. An experiment that the game confirms changes the label to a
confirmed one.

## Exception: world generation

The world generation code stays as it is. The exception covers the crates
`mcrs_minecraft_worldgen*`, `mcrs_minecraft_biome` and `crates/mcrs_minecraft_vanilla_worldgen/`, with their
design notes `docs/worldgen.md`, `docs/aquifer.md`, `docs/surface.md`, `docs/structures.md`,
`docs/scattering.md` and `docs/heightmap.md`. New work in these places follows the rules above.
A change that touches a name of the game's code there removes the name.

## A new behaviour

1. An issue asks for a specification. The author writes it under `docs/spec/`.
2. A second issue asks for the implementation and cites the rule IDs of the specification.
3. The implementer works from the specification. A question goes back to the specification
   author, and the answer goes into the specification.
