# The `locate` command: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

Source of these values: `assets/minecraft/version.json`.

This document states the tree of `locate`, the result of each search, and the messages. It
states what a search must find, not how it finds it. It does not prescribe an architecture.

## 0. Conventions

- **Node**, **source**, **error**: as `command-parser.md` defines them.
- **Source block**: the block position that contains the source position.
- **Found position**: the block position that a search returns.
- **Distance to the source**: the squared distance in three dimensions between the source
  block and a block position, in integers.
- **Name text**: the text that names what the command found (CMD-LOC-005).
- Provenance labels: `registry.md`, section 0.3. ID prefix: `CMD-LOC`. None of the experiments
  has been run.

---

### CMD-LOC-001: Tree

```
locate                                                       require command_level gamemasters
├── structure
│   └── <structure: minecraft:resource_or_tag_key>           executable; registry minecraft:worldgen/structure
├── biome
│   └── <biome: minecraft:resource_or_tag>                   executable; registry minecraft:worldgen/biome
└── poi
    └── <poi: minecraft:resource_or_tag>                     executable; registry minecraft:point_of_interest_type
```

The children of `locate` are in the order `structure`, `biome`, `poi`.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: run `dumpReports`
(`tools/vanilla-oracle`), and compare the node `locate` in `commands.json` with the tree above.
The report sorts the children by name, so it does not show their order.

### CMD-LOC-002: `locate structure`

- **Behavior:**
  1. Resolve the value against the structure registry of the source dimension: an entry gives
     one structure; a tag gives its structures. If the entry or the tag does not exist, fail
     with `commands.locate.structure.invalid`, with the text of the value
     (`argument-resource.md`, ARG-RES-005).
  2. Search for a start of one of the structures, as CMD-LOC-006 states.
  3. If the search finds nothing, fail with `commands.locate.structure.not_found`, with the
     text of the value.
  4. Otherwise send the result (CMD-LOC-005) with the key `commands.locate.structure.success`,
     without the vertical component.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CMD-LOC-003: `locate biome`

- **Behavior:**
  1. Search the sample points for a point with one of the biomes.
  2. If the search finds nothing, fail with `commands.locate.biome.not_found`, with the text of
     the value.
  3. Otherwise send the result with the key `commands.locate.biome.success`, with the vertical
     component.
- **Sample points:** let `(sx, sy, sz)` be the source block. A sample point is a block
  `(sx + 32·i, y, sz + 32·j)`, with these conditions:
  - `i` and `j` are integers from −200 to 200;
  - `y` is `c + 64·k` for an integer `k`, where `c` is `sy` clamped to `[min_y + 1, max_y + 1]`;
  - `y` is in `[min_y + 1, max_y + 1]`. `min_y` is the `min_y` of the dimension type of the
    source dimension, and `max_y` is `min_y + height − 1`.
- **Biome of a point:** the biome that the biome source of the dimension gives at the quart
  position of the point. It is not the biome that a chunk stores.
- **Result:** the sample point with one of the biomes and the smallest distance to the source.
  When two points have the same distance to the source, the choice is unspecified. The found
  entry is the biome at that point.

The contract is the nearest sample point in the whole area. Structures and points of interest
use the same definition of the nearest result.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. The grid and the limits are
from the reference; the choice of the nearest point is a deviation (CMD-LOC-008).

### CMD-LOC-004: `locate poi`

- **Behavior:**
  1. Search the points of interest of the source dimension for the nearest one of the types,
     occupied or not. A point of interest is in the search when its distance to the source is
     at most 256². The nearest one is the one with the smallest distance to the source. When
     two have the same distance to the source, the choice is unspecified.
  2. If the search finds nothing, fail with `commands.locate.poi.not_found`, with the text of
     the value.
  3. Otherwise send the result with the key `commands.locate.poi.success`, without the vertical
     component.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CMD-LOC-005: The result message

- **Inputs:** the value, the found position `(fx, fy, fz)` with the entry that it holds, the
  source block `(sx, sy, sz)`, a key, and the flag `with y`.
- **Behavior:**
  - The name text is the text of the value. When the value is a tag, a space follows that
    text. The identifier of the found entry in `(` and `)` follows the space.
  - The distance is:
    - with `y`: `floor(sqrt(dx² + dy² + dz²))`, with `dx = fx − sx`, `dy = fy − sy` and
      `dz = fz − sz`;
    - without `y`: `floor(sqrt(dx² + dz²))`.
    - The sum and the square root are exact. Compute them in 64-bit integers, with an integer
      square root. The reference rounds to binary32 (CMD-LOC-009).
  - The shown y is `fy` with `y`, and `~` without it.
  - The coordinates text is the translation `chat.square_brackets`. Its argument is the
    translation `chat.coordinates` with `fx`, the shown y and `fz`.
  - The style of the coordinates text is on `chat.square_brackets`. It has the color green,
    the hover text `chat.coordinates.tooltip`, and the click event `suggest_command` of
    `/tp @s <fx> <shown y> <fz>`.
  - A success message: the key with the name text, the coordinates text and the distance. The
    broadcast flag is clear.
- **Postconditions:** the result of the command is the distance. The duration of the search
  and any log line are unspecified. Log lines are not protocol.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run `locate
structure #minecraft:village` and read a name text such as `#minecraft:village
(minecraft:village_plains)`.

### CMD-LOC-006: The structure search

- **Inputs:** the structures, and the source block.
- **Searched area:**
  - For each placement of the type `random_spread`, a cell is a square of `spacing` by
    `spacing` chunks, with the `spacing` of that placement. The cells start at chunk 0 on
    each axis. The area is each cell that is at most 100 cells from the cell of the source
    chunk, on each axis.
  - For each placement of the type `concentric_rings`, the area is each ring position. The
    limit of 100 cells does not apply.
  - A placement of another type, for example `dimension_origin`, has no searched area.
- **Behavior:**
  1. If the world option `generate_structures` is false, the search finds nothing.
  2. If no placement of the source dimension places one of the structures, the search finds
     nothing.
  3. Otherwise the result is the start of one of the structures in the searched area with the
     smallest distance to the source. When two starts have the same distance to the source,
     the choice is unspecified.
- **Found position:** for a start in the chunk `(cx, cz)`, the found position is the block
  `(16·cx, 0, 16·cz)` plus the `locate_offset` of the placement. The found entry is the
  structure of the start.
- **Postconditions:**
  - The set of starts and their found positions are world generation, and they are exact.
    The order of the search is not world generation.
  - `locate` does not change the world. It changes no block and no entity, and it adds no
    reference to a start. Whether the search generates chunks is unspecified.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. The area, the found position
and the conditions of steps 1 and 2 are from the reference. The choice of the nearest start is a
deviation (CMD-LOC-007). Experiment: in a new world, stand at `200000 64 0`, and run `locate
structure minecraft:stronghold`. The search finds a stronghold near the origin, more than 170
000 blocks away.

---

## Intentional deviations

### CMD-LOC-007: Order of the structure search

- **Rule:** CMD-LOC-006, step 3.
- **Reference:** the search can return a start that is not the nearest one:
  - It examines the concentric-rings positions first. Inside one placement, it compares the
    ring chunks by the block `(16·cx + 8, 32, 16·cz + 8)`, not by the found position.
  - It then examines the `random_spread` cells ring by ring, from ring 0 to ring 100. Ring `r`
    is the cells at `r` cells from the cell of the source chunk.
  - For each placement, it takes the first start in its scan of the ring, x first, then z.
  - It stops at the first ring where a `random_spread` placement has a start. It returns the
    nearest of these starts and of the concentric-rings result.
- **Visible to the client:** yes. The coordinates and the distance in the message can differ,
  and so can the command result.
- **Test:** a differential test accepts the result when two conditions are true:
  - the result is a start of one of the structures;
  - its distance to the source is not more than that of the reference result.

  When the reference result is the nearest start, the two results must be equal.

### CMD-LOC-008: Order of the biome search

- **Rule:** CMD-LOC-003, result.
- **Reference:** the search examines the columns in a square spiral from the source block,
  first east, then south. In each column, it examines the `y` values from `c` outwards,
  alternately up and down. It returns the first sample point with one of the biomes.
- **Visible to the client:** yes. The coordinates and the distance in the message can differ,
  and so can the command result.
- **Test:** a differential test accepts the result when two conditions are true:
  - the biome of the result is one of the biomes;
  - its distance to the source is not more than that of the reference result.

### CMD-LOC-009: Arithmetic of the distance

- **Rule:** CMD-LOC-005, distance.
- **Reference:** with `y`, the game computes the squared distance in binary64, rounds it to
  binary32, and takes a binary32 square root. Without `y`, it computes `dx² + dz²` in 32-bit
  integers that wrap, converts the sum to binary32, and takes a binary32 square root.
- **Visible to the client:** yes, in the message and in the command result. From about
  4096 blocks, the reference can show `n` where the exact floor is `n − 1`. The 2D sum wraps
  when `dx² + dz²` is more than 2³¹ − 1.
- **Test:** a differential test compares the distance exactly only when it is below 4096
  blocks.

---

## Open questions

- CMD-LOC-006 does not state how to test a chunk for a start. That test is world
  generation, outside this document. A structure search exists in
  `mcrs_minecraft_worldgen_structure` and `mcrs_minecraft_worldgen_generator`, under the world
  generation exception
  (`docs/clean-room.md`). Its parity with CMD-LOC-006 is not confirmed.
- The reference search ignores a placement of the type `dimension_origin`, so `locate` cannot
  find its structure. No structure set of the vanilla data pack uses this type. Is this a
  defect that a deviation must correct, for a data pack that uses the type?
