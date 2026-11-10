# The `fill` command: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| Parser ids | `minecraft:block_pos` 8, `minecraft:block_state` 12 |

Source of these values: `assets/minecraft/version.json` and `assets/mcrs/reports/registries.json`,
registry `minecraft:command_argument_type`.

This document states the forms of `fill` without a filter: the plain form and the modes
`replace`, `outline` and `hollow`. It states the volume limit, the sets and the messages. It does
not prescribe an architecture.

## 0. Conventions

- **Node**, **source**, **error**: as `command-parser.md` defines them.
- **Region**: the box of block positions between the two corners, both corners included.
- **Border position**: a position of the region whose x, y or z is the smallest or the largest
  value of the region on that axis.
- **Inner position**: a position of the region that is not a border position.
- **Set**: one change of a block, as `block-simulation.md`, SIM-SET-002, states it.
- Provenance labels: `registry.md`, section 0.3. ID prefix: `CMD-FILL`. None of the experiments
  has been run.

## 1. Scope

The modes `keep`, `destroy` and `strict`, and the filter of `replace <filter>`, are out of scope.
CMD-FILL-001 shows them only to give the whole tree.

---

### CMD-FILL-001: Tree

```
fill                                                   require command_level gamemasters
└── <from: minecraft:block_pos>
    └── <to: minecraft:block_pos>
        └── <block: minecraft:block_state>             executable
            ├── outline                                executable
            ├── hollow                                 executable
            ├── destroy                                executable
            ├── strict                                 executable
            ├── replace                                executable
            │   └── <filter: minecraft:block_predicate>   executable
            │       ├── outline                        executable
            │       ├── hollow                         executable
            │       ├── destroy                        executable
            │       └── strict                         executable
            └── keep                                   executable
```

The permission is PERM-LEVEL-001, level `gamemasters`. `argument-coordinates.md`, ARG-COORD-011,
states `block_pos`. `argument-block.md`, section 6, states `block_state`.

Provenance: `source_suggested`. Kind: wire. Confidence: high. Experiment: run `dumpReports`
(`tools/vanilla-oracle`), and compare the node `fill` in `commands.json` with the tree above.

### CMD-FILL-002: Corners and the volume limit

- **Behavior:**
  1. Resolve `from` and `to` with the loaded check of ARG-COORD-013. A corner in a column that is
     not loaded fails with `argument.pos.unloaded`. A corner outside the build height fails with
     `argument.pos.outofworld`.
  2. The region is the box with these corners. The order of the corners does not matter.
  3. Let `v` be the product of the three side lengths of the region, as a 64-bit integer. Let `n`
     be the value of the game rule `max_block_modifications`. The default of `n` is 32768.
  4. If `v` is more than `n`, fail with `commands.fill.toobig`, with `n` and `v`. The command
     changes no block.
- **Postconditions:** the command tests only the two corners. A column between them that is not
  loaded loads at once (`block-simulation.md`, the note "block changes by commands").

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: run
`fill 0 -61 0 181 -61 181 minecraft:stone`. The region has 33124 positions. The reply is
`commands.fill.toobig` with 32768 and 33124.

### CMD-FILL-003: The block of each position

| Form | Border position | Inner position |
|------|-----------------|----------------|
| `fill <from> <to> <block>` | `block` | `block` |
| `... replace` | `block` | `block` |
| `... outline` | `block` | no change |
| `... hollow` | `block` | `air` |

`replace` without a filter is the same as the plain form.

A region with a side length of 1 has no inner position. Thus `outline` and `hollow` of a region
one block high set each position, as the plain form does. A ring of blocks needs four commands, one
for each side, or a region that is at least three blocks high.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: run
`fill 0 -60 0 4 -60 4 minecraft:oak_fence hollow`. All 25 positions hold a fence.

### CMD-FILL-004: The sets

- **Behavior:**
  1. Visit each position of the region in order: x from the smallest value, then y, then z, with x
     the fastest.
  2. At each position, set the block of CMD-FILL-003, with the steps of `block-simulation.md`,
     the note "block changes by commands", for `/fill` that is not strict. The command first
     adapts the state to its six neighbours. Then it sets the state with the flags 258.
  3. A position counts when its set changes the block state. A set to the state that the position
     already holds does not count.
  4. After all sets, run the follow-up of each set that changed a state, in the same order. The
     follow-up gives the neighbour updates.
- **Postconditions:**
  - A set with the flags 258 makes shape updates (SIM-SET-001). Thus each fence connects to the
    fences and the full blocks next to it, also to a fence of a later set.
  - The flags hold *sync-clients*, so each viewer of the column receives the change
    (SIM-SET-005).

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

### CMD-FILL-005: Feedback and result

- **Behavior:**
  - If no position counts, fail with `commands.fill.failed`.
  - Else send `commands.fill.success`, with the count as an int primitive, as a success message
    with the broadcast flag set (`commands-packet.md`, CMDP-EXEC-004). The result is the count.
- **Edge cases:** the same command a second time fails with `commands.fill.failed`, because each
  position already holds its block.

Provenance: `source_suggested`. Kind: semantic; the keys and the argument order are wire.
Confidence: high. Experiment: run `fill 0 -60 0 2 -60 0 minecraft:stone` two times. The first reply
is `commands.fill.success` with 3. The second reply is `commands.fill.failed`.

---

## 2. Use in a benchmark

- A superflat world already has a floor of `grass_block` at `y = -61`. A fence ring at `y = -60`
  needs four plain fills, one for each side, each one block wide.
- One fill changes at most 32768 positions. A side of a pen of more than 32768 blocks, or a floor
  of more than 32768 positions, needs more than one fill, or a larger
  `max_block_modifications`.
- A script that runs the same fill again must accept `commands.fill.failed`.
