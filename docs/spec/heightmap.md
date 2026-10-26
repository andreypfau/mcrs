# Heightmaps: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |
| Block tags | `minecraft:blocks_motion_in_heightmap`, `minecraft:blocks_motion_in_heightmap_no_leaves` |

Source of the version values: `assets/minecraft/version.json`. Source of the block tags: the
vanilla data pack, `data/minecraft/tags/block/`.

This document states the heightmap types that the game keeps, the block test of each type, the
value of a heightmap, the life of each type, and how a command reads a value. It states the
observable values, not how an implementation computes them. The design note
`docs/heightmap.md` states one design that gives these values. That note calls a block column a
"strip".

## 0. Conventions

### 0.1 Terms

- **Block column**: the blocks of one dimension at one pair of block coordinates `x` and `z`, from
  `min_y` to `min_y + height − 1`. `min_y` and `height` are the fields of the dimension type.
- **Chunk**: the 16 × 16 block columns whose block coordinates `x` and `z` have the same values
  of `x >> 4` and `z >> 4`. The shift is arithmetic.
- **Local coordinates of a block column**: `x & 15` and `z & 15`.
- **Heightmap type**: one of the six types of HM-001.
- **Kept type**: a heightmap type that a complete chunk has (HM-005).
- **Generation type**: a heightmap type that only the world generator uses (HM-005).
- **Block test**: the predicate over a block state that a heightmap type selects (HM-002).
- **Counting block**: a block state that the block test of a type accepts.
- **Heightmap value**: the value `H` of a type at a block column (HM-003).
- **Complete chunk**: a chunk at the generation status `minecraft:full`.
- **Loaded chunk**: as `command-execute.md`, section 0.1, defines it.
- **Block floor**: as `argument-coordinates.md`, section 0.1, defines it.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". None of the experiments has been run.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `HM` | heightmap types, block tests, values, and reads |

---

## 1. Types

### HM-001: Heightmap types

| Name | Id | Block test | Usage |
|------|----|------------|-------|
| `WORLD_SURFACE_WG` | 0 | not air | generation |
| `WORLD_SURFACE` | 1 | not air | kept, sent to the client |
| `OCEAN_FLOOR_WG` | 2 | blocks motion | generation |
| `OCEAN_FLOOR` | 3 | blocks motion | kept |
| `MOTION_BLOCKING` | 4 | blocks motion, or has a fluid | kept, sent to the client |
| `MOTION_BLOCKING_NO_LEAVES` | 5 | blocks motion without leaves, or has a fluid | kept, sent to the client |

- The name is the key of the type in the `Heightmaps` compound of the save. The case is upper.
- The id is the key of the type in the heightmap map of the chunk packet.
- HM-002 states each block test.
- The type set is fixed. A data pack cannot add a type.

Provenance: `source_suggested`. Kind: wire for the names and the ids, semantic for the usage.
Confidence: medium. Experiment: open a region file of a world that the game saved. Read the
`Heightmaps` compound of a complete chunk. It has the four keys `WORLD_SURFACE`, `OCEAN_FLOOR`,
`MOTION_BLOCKING` and `MOTION_BLOCKING_NO_LEAVES`.

---

## 2. Block tests

### HM-002: Block tests

Each block test reads only the block state. It does not read the position, the neighbours or the
time.

| Block test | A block state is a counting block when |
|------------|----------------------------------------|
| not air | the block state is not air |
| blocks motion | the block of the state is in the tag `minecraft:blocks_motion_in_heightmap` |
| blocks motion, or has a fluid | the block is in the tag `minecraft:blocks_motion_in_heightmap`, or the fluid state of the block state is not empty |
| blocks motion without leaves, or has a fluid | the block is in the tag `minecraft:blocks_motion_in_heightmap_no_leaves`, or the fluid state of the block state is not empty |

- **Air:** the air block states are the states of `minecraft:air`, `minecraft:cave_air` and
  `minecraft:void_air`. The data pack does not state this property.
- **Tags:** the test reads the tag of the loaded data packs. The test does not read the collision
  shape of the block. An implementation must derive the counting blocks from the tags, not from a
  list in code.
- **Fluid state:** a block state has a fluid when its fluid state is not empty. A water or lava
  block at each level has a fluid. A block state with `waterlogged=true` has a fluid. A block whose
  fluid state is always water also has a fluid, for example `minecraft:kelp`,
  `minecraft:seagrass` and `minecraft:bubble_column`.
- **Data pack values:** in the vanilla data pack, `minecraft:blocks_motion_in_heightmap` holds
  `#minecraft:blocks_motion`, which holds `#minecraft:blocks_motion_no_leaves` and
  `#minecraft:leaves`. `minecraft:blocks_motion_in_heightmap_no_leaves` holds
  `#minecraft:blocks_motion_no_leaves`. Examples from the vanilla data pack:

| Block | blocks motion | blocks motion without leaves |
|-------|---------------|------------------------------|
| `minecraft:stone` | yes | yes |
| `minecraft:oak_leaves` | yes | no |
| `minecraft:stone_pressure_plate` | yes | yes |
| `minecraft:glass_pane` | yes | yes |
| `minecraft:cobweb` | no | no |
| `minecraft:water` | no | no |
| `minecraft:snow` | no | no |

Provenance: `source_suggested`. `black_box_confirmed` for the tag values (the data pack of the
client jar). Kind: semantic. Confidence: medium. Experiment: in a default superflat world, the
top block of each block column is grass at y −61. Run these commands:

1. `setblock 0 -60 0 minecraft:water`
2. `setblock 0 -55 0 minecraft:oak_leaves[persistent=true]`
3. `setblock 0 -53 0 minecraft:cobweb`
4. `setblock 4 -60 0 minecraft:stone_pressure_plate`

Then run `execute positioned 0.5 0 0.5 positioned over <type> run tp @s ~ ~ ~` for each kept
type. The new y is −52 for `world_surface`, −60 for `ocean_floor`, −54 for `motion_blocking` and
−59 for `motion_blocking_no_leaves`. Then run
`execute positioned 4.5 0 0.5 positioned over ocean_floor run tp @s ~ ~ ~`. The new y is −59: the
pressure plate is a counting block, but it has no collision.

---

## 3. Values

### HM-003: Heightmap value

- **Behavior:** for a heightmap type `T` and a block column `c`:
  - if `c` has a counting block of `T`, `H` is 1 plus the largest `y` of a counting block of `T`
    in `c`;
  - otherwise `H` is `min_y`.
- **Postconditions:**
  - `H` is the lowest `y` above which the block column has no counting block. It is not the `y`
    of the top counting block.
  - `H` is in the range `min_y` to `min_y + height`.
  - Between two heightmap types, the relations of the block tests give relations of the values.
    For each block column: `OCEAN_FLOOR ≤ MOTION_BLOCKING ≤ WORLD_SURFACE`, and
    `MOTION_BLOCKING_NO_LEAVES ≤ MOTION_BLOCKING`. This is true only when the block tags keep
    these relations (`docs/heightmap.md`, section 2).
- **Storage:** the save and the chunk packet carry the values of one chunk as one array of 64-bit
  integers.
  - The entry of the block column with the local coordinates `x` and `z` has the index
    `x + 16 · z`.
  - Each entry holds `H − min_y` in `b` bits, where `b` is the smallest integer with
    `2^b ≥ height + 1`.
  - Each 64-bit integer holds `floor(64 / b)` entries, from the low bits. An entry never spans
    two integers. The array has `ceil(256 / floor(64 / b))` integers.
  - For a `height` of 384, `b` is 9, and the array has 37 integers.
- **Edge cases:**

| Block column | `H` |
|--------------|-----|
| only air | `min_y` |
| one counting block at `min_y` | `min_y + 1` |
| a counting block at `min_y + height − 1` | `min_y + height` |

Provenance: `source_suggested`. Kind: semantic for the value, wire for the storage. Confidence:
medium. Experiment: in the Overworld (`min_y` −64), fill a block column from y −64 to y 319 with
air. Run `execute positioned 0.5 0 0.5 positioned over world_surface run tp @s ~ ~ ~`. The new y
is −64. Then place stone at y −64 and run the command again. The new y is −63.

### HM-004: Value after a block change

- **Behavior:** after each change of a block state in a complete chunk, each kept type has the
  value of HM-003 for the new blocks. This is true before the next command or game rule reads the
  value.
- **Edge cases:**
  - Removal of the top counting block gives the value of the next counting block below it, or
    `min_y`.
  - A change below the top counting block does not change `H`.
- **Postconditions:** the update order of the four kept types is unspecified.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: after the
experiment of HM-002, run `setblock 0 -55 0 minecraft:air`. Then run
`execute positioned 0.5 0 0.5 positioned over motion_blocking run tp @s ~ ~ ~`. The new y is
−59, the value of the water at y −60.

### HM-005: Life of the types

- **Generation types:** the world generator uses `WORLD_SURFACE_WG` and `OCEAN_FLOOR_WG` before
  the generation status `minecraft:terrain`. From that status on, a chunk has the four kept types.
  The world generator rules state when the generation types change.
- **Complete chunk:** a complete chunk has the four kept types and no generation type.
- **Save:** the `Heightmaps` compound of a chunk has the types that the chunk has at its status.
- **Chunk packet:** the chunk packet has the three types that HM-001 marks "sent to the client":
  `WORLD_SURFACE`, `MOTION_BLOCKING` and `MOTION_BLOCKING_NO_LEAVES`. It does not have
  `OCEAN_FLOOR`.
- **Load:** when the save of a chunk has no array for a type that the chunk must have, or the
  array has the wrong length, the server computes the values from the blocks (HM-003).

Provenance: `source_suggested`. Kind: wire for the chunk packet and the save, semantic for the
rest. Confidence: medium. Experiment: capture a `level_chunk_with_light` packet. Its heightmap
map has the ids 1, 4 and 5 and no other id.

---

## 4. Reads

### HM-006: Read a heightmap value

- **Inputs:** a dimension, a kept type `T`, and the block coordinates `x` and `z`.
- **Behavior:** the result is the heightmap value of `T` at the block column of `x` and `z`, in
  the chunk that contains it. That chunk must be a loaded chunk. A reader that can meet a chunk
  that is not loaded checks this first.
- **Postconditions:** `execute positioned over` (`command-execute.md`, CMD-EX-015) uses this read.
  It takes `x` and `z` as the block floor of the source position, and fails with
  `argument.pos.unloaded` when the chunk is not loaded. The result does not depend on `y`.
- **Edge cases:** a read of a generation type is not possible from a command
  (`argument-keywords.md`, ARG-KW-003).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: on flat land
with the top block at y 63, stand at `0.5 100 0.5` and run
`execute positioned over world_surface run tp @s ~ ~ ~`. The player moves to `0.5 64 0.5`.

---

## 5. Unspecified behaviour

- **A read of a chunk that is not loaded.** The reference gives `min_y` for a block column in a
  chunk that is not loaded. It gives the sea level of the dimension plus 1 when `x` or `z` is not
  in the range −30 000 000 to 29 999 999. HM-006 requires a loaded chunk, so a parity test must
  not compare these values.
- **A save that disagrees with its blocks.** The reference uses a stored array of the right
  length as it is, also when the values do not agree with HM-003. A parity test must not compare
  a chunk whose save disagrees.

---

## 6. Open questions

- **A change of the block tags.** A data pack reload can change the two block tags of HM-002. It
  is not known if the reference computes the values of the loaded chunks again. Experiment: make a
  data pack that replaces `minecraft:blocks_motion_in_heightmap` with `"replace": true` and only
  `minecraft:stone`. Run step 4 of the experiment of HM-002, then run `reload`. Run
  `execute positioned 4.5 0 0.5 positioned over ocean_floor run tp @s ~ ~ ~`, and record the new
  y. Then unload the chunk, load it again, and record the y again.
