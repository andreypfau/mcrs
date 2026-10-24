# The `locate` command: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

Source of these values: `assets/minecraft/version.json`.

This document states the tree of `locate`, the limits of each search, and the messages. It does
not state the search algorithms. It does not prescribe an architecture.

## 0. Conventions

- **Node**, **source**, **error**: as `command-parser.md` defines them.
- **Source block**: the block position that contains the source position.
- **Found position**: the block position that a search returns.
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

Provenance: `black_box_confirmed` (`commands.json`) for the nodes, the types and the registries.
`source_suggested` for the order, which the report sorts by name. Confidence: high.

### CMD-LOC-002: `locate structure`

- **Behavior:**
  1. Resolve the value against the structure registry of the source dimension: an entry gives
     one structure; a tag gives its structures. If the entry or the tag does not exist, fail
     with `commands.locate.structure.invalid`, with the text of the value
     (`argument-resource.md`, ARG-RES-005).
  2. Search for the nearest start of one of the structures from the source block, in a radius
     of 100 chunks, with no condition on unexplored chunks.
  3. If the search finds nothing, fail with `commands.locate.structure.not_found`, with the
     text of the value.
  4. Otherwise send the result (CMD-LOC-005) with the key `commands.locate.structure.success`,
     without the vertical component.

Provenance: `source_suggested`. Confidence: medium.

### CMD-LOC-003: `locate biome`

- **Behavior:**
  1. Search for the nearest position with one of the biomes, from the source block, in a radius
     of 6400 blocks, with a horizontal step of 32 blocks and a vertical step of 64 blocks.
  2. If the search finds nothing, fail with `commands.locate.biome.not_found`, with the text of
     the value.
  3. Otherwise send the result with the key `commands.locate.biome.success`, with the vertical
     component.

Provenance: `source_suggested`. Confidence: medium.

### CMD-LOC-004: `locate poi`

- **Behavior:**
  1. Search the points of interest of the source dimension for the nearest one of the types,
     from the source block, in a radius of 256 blocks, occupied or not.
  2. If the search finds nothing, fail with `commands.locate.poi.not_found`, with the text of
     the value.
  3. Otherwise send the result with the key `commands.locate.poi.success`, without the vertical
     component.

Provenance: `source_suggested`. Confidence: medium.

### CMD-LOC-005: The result message

- **Inputs:** the value, the found position `(fx, fy, fz)` with the entry that it holds, the
  source block `(sx, sy, sz)`, a key, and the flag `with y`.
- **Behavior:**
  - The name text is the text of the value. When the value is a tag, the name text is the text
    of the value, a space, and the identifier of the found entry in `(` and `)`.
  - The distance is:
    - with `y`: `floor(sqrt(d))`, where `d` is the squared distance between the two block
      positions and the square root is in binary32;
    - without `y`: `floor(sqrt(dx² + dz²))` with `dx = fx − sx` and `dz = fz − sz`, in integers,
      and the square root in binary32.
  - The shown y is `fy` with `y`, and `~` without it.
  - The coordinates text is the translation `chat.coordinates` with `fx`, the shown y and `fz`,
    in `[` and `]`, in green, with the hover text `chat.coordinates.tooltip`, and with the click
    event `suggest_command` of `/tp @s <fx> <shown y> <fz>`.
  - A success message: the key with the name text, the coordinates text and the distance. The
    broadcast flag is clear.
- **Postconditions:** the result of the command is the distance.

Provenance: `source_suggested`. Confidence: medium. Experiment: run `locate structure
#minecraft:village` and read a name text such as `#minecraft:village (minecraft:village_plains)`.

---

## Open questions

- The three search algorithms are not in this document. A structure search exists in
  `mcrs_minecraft_worldgen_structure` and `mcrs_minecraft_worldgen_generator`, under the world
  generation exception (`docs/clean-room.md`). Its parity with CMD-LOC-002 is not confirmed.
  The biome and point-of-interest searches need a specification before they are built.
