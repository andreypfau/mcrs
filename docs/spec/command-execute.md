# The `execute` command: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |
| Parser ids | `brigadier:double` 2, `minecraft:entity` 6, `minecraft:block_pos` 8, `minecraft:vec3` 10, `minecraft:block_predicate` 13, `minecraft:item_predicate` 15, `minecraft:nbt_path` 23, `minecraft:objective` 24, `minecraft:rotation` 29, `minecraft:score_holder` 31, `minecraft:swizzle` 32, `minecraft:resource_location` 36, `minecraft:function` 37, `minecraft:entity_anchor` 38, `minecraft:int_range` 39, `minecraft:float_range` 40, `minecraft:dimension` 41, `minecraft:resource_or_tag` 44, `minecraft:resource` 46, `minecraft:heightmap` 51, `minecraft:loot_predicate` 53, `minecraft:slot_source` 57 |

Source of the version values: `assets/minecraft/version.json`. Source of the parser ids:
`assets/mcrs/reports/registries.json`, registry `minecraft:command_argument_type`.

This document states the tree of the `execute` command, how each branch changes or tests the
source, how a chain of stages runs, and how `store` writes the result of a run. It also states
the messages and the errors. It does not prescribe an architecture.

Sections 1 to 3 state the tree, the source modifiers and the run of a chain. Section 4 states
the results and the `store` targets. Section 5 states the conditions, and section 6 the
messages and the errors. Section 8 names the core set of a first implementation, and lists what
each other branch needs.

## 0. Conventions

### 0.1 Terms

- **Node**, **literal**, **argument**, **redirect**, **executable**, **source**, **context**,
  **modifier stage**, **run stage**, **error**: as `command-parser.md` defines them.
- **Chain**: the contexts of one parse result, in order (`command-parser.md`, CMD-EXEC-001).
  Each context before the last one is a modifier stage. The last context is the run stage.
- **Suggestion type**, **operator**, **system message**: as `commands-packet.md` defines them.
- **Modifier node**: a node with a redirect to the node `execute` and a source modifier. A
  modifier node is a single node or a fork node.
- **Single node**: a modifier node that maps one source to one source, or fails with an error.
- **Fork node**: a modifier node that maps one source to zero or more sources.
- **Source list**: the sources of one stage, in order.
- **Forked chain**: a chain that has a modifier stage whose last node is a fork node
  (CMD-EX-032). The stages from that stage to the end are forked stages.
- **Output**: the receiver of the messages of a source: a player, the server console or a
  command block. A source modifier never changes the output.
- **Display name**: the name that a command shows for the source, for example in `say` and in
  `chat.type.admin`.
- **Anchor point**: the point of a source that ARG-KW-002 (`argument-keywords.md`), "Value",
  defines.
- **Result**: the pair that one run of the run stage gives: a success flag and a value. The
  value is a signed 32-bit integer.
- **Store target**: a place that a `store` node attaches to a source. A store target receives
  each result of that source (CMD-EX-041).
- **Condition**: the test of a branch under `if` or `unless`.
- **Expected value**: true for a condition under `if`, false for a condition under `unless`.
- **Counting condition**: a condition that gives a count: `entity`, `blocks`, `data`, `items`
  and `slots`. Each other condition gives a boolean.
- **Loaded chunk**: a chunk of the source dimension that the server holds at the full status.
  The server can read its blocks.
- **Ticking chunk**: a loaded chunk whose entities tick, and whose entities the server has
  loaded.
- **Block of a point**: the block position whose three axes are the floor of the three axes of
  the point.
- **Coordinate scale**: the field `coordinate_scale` of a dimension type.
- **Heightmap value**: the value of `heightmap.md`, HM-003: the first free y above the top
  block of a column that the block test of the heightmap type matches.
- **Spawnable bounds**: as `command-teleport.md` defines them.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". None of the experiments has been run.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CMD-EX` | the `execute` command, the run of a chain, and the results |

---

## 1. The tree

### CMD-EX-001: Tree

```
execute                                                  require command_level gamemasters
├── run                                                  redirect to the root
├── if                                                   (the condition subtree, CMD-EX-002)
├── unless                                               (the condition subtree, CMD-EX-002)
├── as
│   └── <targets: minecraft:entity>                      entities (0x00); fork
├── at
│   └── <targets: minecraft:entity>                      entities (0x00); fork
├── store
│   ├── result                                           (the store subtree, CMD-EX-003)
│   └── success                                          (the store subtree, CMD-EX-003)
├── positioned
│   ├── <pos: minecraft:vec3>                            single
│   ├── as
│   │   └── <targets: minecraft:entity>                  entities (0x00); fork
│   └── over
│       └── <heightmap: minecraft:heightmap>             single
├── rotated
│   ├── <rot: minecraft:rotation>                        single
│   └── as
│       └── <targets: minecraft:entity>                  entities (0x00); fork
├── facing
│   ├── entity
│   │   └── <targets: minecraft:entity>                  entities (0x00)
│   │       └── <anchor: minecraft:entity_anchor>        fork
│   └── <pos: minecraft:vec3>                            single
├── align
│   └── <axes: minecraft:swizzle>                        single
├── anchored
│   └── <anchor: minecraft:entity_anchor>                single
├── in
│   └── <dimension: minecraft:dimension>                 single
├── summon
│   └── <entity: minecraft:resource>                     single; registry minecraft:entity_type; suggestion type minecraft:summonable_entities
└── on
    ├── owner                                            fork
    ├── leasher                                          fork
    ├── target                                           fork
    ├── attacker                                         fork
    ├── vehicle                                          fork
    ├── controller                                       fork
    ├── origin                                           fork
    └── passengers                                       fork
```

- "single" and "fork" mark a modifier node. Each modifier node has a redirect to the node
  `execute`. CMD-EX-004 states the two kinds.
- The children of each node are in the order of the tree.
- Only the node `execute` has a requirement. Every other node has none.
- The node `execute` is not executable. A command line that ends after a modifier node, or
  after `run`, fails with `dispatcher_unknown_command` (`command-parser.md`, CMD-PARSE-007).

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: run `dumpReports`
(`tools/vanilla-oracle`), and compare the node `execute` in `commands.json` with the tree above.
The report sorts the children by name, so it does not show their order. Then decode the node
`execute` of a captured `commands` packet, and read the order of its children.

### CMD-EX-002: Condition subtree

The nodes `if` and `unless` each have their own copy of these nodes, in this order:

```
├── block
│   └── <pos: minecraft:block_pos>
│       └── <block: minecraft:block_predicate>           executable; fork
├── biome
│   └── <pos: minecraft:block_pos>
│       └── <biome: minecraft:resource_or_tag>           executable; fork; registry minecraft:worldgen/biome
├── loaded
│   └── <pos: minecraft:block_pos>                       executable; fork
├── dimension
│   └── <dimension: minecraft:dimension>                 executable; fork
├── score
│   └── <target: minecraft:score_holder>                 one (0x00); suggestion type minecraft:ask_server
│       └── <targetObjective: minecraft:objective>
│           ├── =                                        (the comparison shape)
│           ├── <                                        (the comparison shape)
│           ├── <=                                       (the comparison shape)
│           ├── >                                        (the comparison shape)
│           ├── >=                                       (the comparison shape)
│           └── matches
│               └── <range: minecraft:int_range>         executable; fork
├── blocks
│   └── <start: minecraft:block_pos>
│       └── <end: minecraft:block_pos>
│           └── <destination: minecraft:block_pos>
│               ├── all                                  executable; fork
│               └── masked                               executable; fork
├── entity
│   └── <entities: minecraft:entity>                     entities (0x00); executable; fork
├── predicate
│   └── <predicate: minecraft:loot_predicate>            executable; fork
├── function
│   └── <name: minecraft:function>                       fork; suggestion type minecraft:ask_server
├── stopwatch
│   └── <id: minecraft:resource_location>                suggestion type minecraft:ask_server
│       └── <range: minecraft:float_range>               executable; fork
├── data
│   ├── entity
│   │   └── <source: minecraft:entity>                   one entity (0x01)
│   │       └── <path: minecraft:nbt_path>               executable; fork
│   ├── block
│   │   └── <source: minecraft:block_pos>
│   │       └── <path: minecraft:nbt_path>               executable; fork
│   └── storage
│       └── <source: minecraft:resource_location>        suggestion type minecraft:ask_server
│           └── <path: minecraft:nbt_path>               executable; fork
├── items
│   ├── entity
│   │   └── <source: minecraft:entity>                   entities (0x00)
│   │       └── <slots: minecraft:slot_source>
│   │           └── <item_predicate: minecraft:item_predicate>   executable; fork
│   └── block
│       └── <source: minecraft:block_pos>
│           └── <slots: minecraft:slot_source>
│               └── <item_predicate: minecraft:item_predicate>   executable; fork
└── slots
    ├── entity
    │   └── <source: minecraft:entity>                   entities (0x00)
    │       └── <slots: minecraft:slot_source>           executable; fork
    └── block
        └── <source: minecraft:block_pos>
            └── <slots: minecraft:slot_source>           executable; fork

comparison shape:
└── <source: minecraft:score_holder>                     one (0x00); suggestion type minecraft:ask_server
    └── <sourceObjective: minecraft:objective>           executable; fork
```

- A node with "executable; fork" is executable and also has a redirect to `execute`. When the
  command line ends after it, the condition is the run stage (CMD-EX-052). Otherwise the
  condition is a modifier stage (CMD-EX-051).
- `function` `<name>` is not executable. `execute if function <name>` with nothing after it
  fails with `dispatcher_unknown_command`.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: as CMD-EX-001, for
the nodes `if` and `unless`.

### CMD-EX-003: Store subtree

The nodes `result` and `success` each have their own copy of these nodes, in this order:

```
├── score
│   └── <targets: minecraft:score_holder>                many (0x01); suggestion type minecraft:ask_server
│       └── <objective: minecraft:objective>             single
├── bossbar
│   └── <id: minecraft:resource_location>                suggestion type minecraft:ask_server
│       ├── value                                        single
│       └── max                                          single
├── entity
│   └── <target: minecraft:entity>                       one entity (0x01)
│       └── <path: minecraft:nbt_path>                   (the type list)
├── block
│   └── <target: minecraft:block_pos>
│       └── <path: minecraft:nbt_path>                   (the type list)
└── storage
    └── <target: minecraft:resource_location>            suggestion type minecraft:ask_server
        └── <path: minecraft:nbt_path>                   (the type list)

type list:
├── int
│   └── <scale: brigadier:double>                        single
├── float
│   └── <scale: brigadier:double>                        single
├── short
│   └── <scale: brigadier:double>                        single
├── long
│   └── <scale: brigadier:double>                        single
├── double
│   └── <scale: brigadier:double>                        single
└── byte
    └── <scale: brigadier:double>                        single
```

The vanilla server gives some of these nodes one node number (`commands-packet.md`,
CMDP-DEV-001).

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: as CMD-EX-001, for
the nodes `result` and `success`.

### CMD-EX-004: Single nodes and fork nodes

- A single node maps one source to one source, or fails with an error.
- A fork node maps one source to a source list. The list can be empty or have many sources.
- A redirect with or without a fork has the same form on the wire: flag `0x08` and the number of
  the target (`commands-packet.md`, CMDP-PKT-001). The client does not know which nodes fork.
- `run` is a redirect to the root with no source modifier. It passes the source list on with no
  change (CMD-EX-030).

| Kind | Nodes |
|------|-------|
| single | `positioned <pos>`, `positioned over <heightmap>`, `rotated <rot>`, `facing <pos>`, `align <axes>`, `anchored <anchor>`, `in <dimension>`, `summon <entity>`, each `store` node |
| fork | `as <targets>`, `at <targets>`, `positioned as <targets>`, `rotated as <targets>`, `facing entity <targets> <anchor>`, each node of `on`, each condition |

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: the experiments
of CMD-EX-032 decide the kind of each node.

### CMD-EX-005: Properties and suggestion types on the wire

| Argument | Properties (`commands-packet.md`, CMDP-ARG-001) |
|----------|--------------------------------------------------|
| `minecraft:entity`, entities | Byte `0x00` |
| `minecraft:entity`, one entity | Byte `0x01` |
| `minecraft:score_holder`, one | Byte `0x00` |
| `minecraft:score_holder`, many | Byte `0x01` |
| `minecraft:resource` of `summon` | Identifier: `minecraft:entity_type` |
| `minecraft:resource_or_tag` of `biome` | Identifier: `minecraft:worldgen/biome` |
| `brigadier:double` of `<scale>` | flags `0x00`: no bound |
| each other argument of this document | none |

- An argument with a suggestion type in CMD-EX-001 to CMD-EX-003 has the flag `0x10`. Each other
  argument has no suggestion type.
- The arguments with `minecraft:ask_server` get their suggestions from the server
  (`commands-packet.md`, CMDP-SUG). These are the score holders, the boss bar identifiers, the
  storage identifiers, the function identifiers and the stopwatch identifiers. Section 8 lists
  the documents that must state these suggestion sources.
- The node `execute` has the restricted flag `0x20` (`command-permissions.md`, PERM-SET-002).
  No other node of this document has it.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: decode the node
`execute` of a captured `commands` packet for an operator. Read the properties of
`as <targets>`, of `store result score <targets>` and of `summon <entity>`.

### CMD-EX-006: Choice between siblings

- The parse rules of `command-parser.md`, CMD-PARSE-002, decide each choice. In
  `execute positioned as @s`, the token `as` matches the literal, so the parse does not try
  `<pos>`. The same applies to `rotated as` and `facing entity`.
- A condition node is executable and has a redirect. When text follows it, the parse follows the
  redirect to `execute` (CMD-PARSE-004). When no text follows it, the condition is the run stage.
- The parse resolves no value against a modified source. A requirement after a redirect tests
  the original source (CMD-PARSE-004).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `execute positioned as @s run say a`. The server says `a`. Then send
`execute if entity @s`. The reply is `commands.execute.conditional.pass_count` with 1.

---

## 2. Source modifiers

### CMD-EX-010: What each modifier changes

| Modifier | Entity | Position | Rotation | Dimension | Anchor |
|----------|--------|----------|----------|-----------|--------|
| `as` | each target | keep | keep | keep | keep |
| `at` | keep | of each target | of each target | of each target | keep |
| `positioned <pos>` | keep | `pos` | keep | keep | `feet` |
| `positioned as` | keep | of each target | keep | keep | keep |
| `positioned over` | keep | new y | keep | keep | keep |
| `rotated <rot>` | keep | keep | `rot` | keep | keep |
| `rotated as` | keep | keep | of each target | keep | keep |
| `facing <pos>` | keep | keep | toward `pos` | keep | keep |
| `facing entity` | keep | keep | toward each target | keep | keep |
| `align` | keep | floor of the axes | keep | keep | keep |
| `anchored` | keep | keep | keep | keep | `anchor` |
| `in` | keep | scaled x and z | keep | `dimension` | keep |
| `summon` | the new entity | keep | keep | keep | keep |
| `on` | each related entity | keep | keep | keep | keep |
| `store` | keep | keep | keep | keep | keep |
| condition | keep | keep | keep | keep | keep |

- Every modifier keeps the output, the permissions, the silent flag and the store targets. Only
  `store` adds a store target (CMD-EX-041).
- A modifier that changes the entity also changes the display name to the display name of the
  new entity. Each other modifier keeps the display name.
- A rotation is a pitch and a yaw, each a binary32 angle. "Of each target" means the pitch and
  the yaw of the entity, with no wrap.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: from the server
console, run `execute as @p run say hi`. The chat line has the name of the player, not `Server`.

### CMD-EX-011: `as <targets>`

- **Behavior:** find the entities of `targets` for the source (`argument-entity.md`,
  ARG-ENT-013). Give one source for each entity, in the order of the search. Each source has
  that entity.
- **Edge cases:** when the search finds no entity, the fork gives an empty source list. This is
  not an error.
- **Postconditions:** the messages of the commands after `as` go to the original output. The
  permissions are those of the original source, not of the entity.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator,
with one pig near, run `execute as @e[type=pig,limit=1] run summon cow ~ ~ ~`. The cow appears
at the operator, not at the pig. Then run `execute as @e[type=pig,limit=1] run tag @s add x`.
The operator receives the message, and the message names the pig.

### CMD-EX-012: `at <targets>`

- **Behavior:** find the entities of `targets`. Give one source for each entity, in order. Each
  source has the dimension, the position and the rotation of that entity.
- **Edge cases:** no entity gives an empty source list.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: put an armor
stand with the tag `n` in the Nether at `0 64 0`. From the Overworld, run
`execute at @e[tag=n] run summon pig ~ ~ ~`. The pig appears in the Nether at `0 64 0`.

### CMD-EX-013: `positioned <pos>`

- **Behavior:** resolve `pos` against the source (`argument-coordinates.md`, ARG-COORD-006 and
  ARG-COORD-007). The argument has the center correction set (ARG-COORD-003). The source gets
  that position and the anchor `feet`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`execute positioned 1 64 1 run tp @s ~ ~ ~`. The player moves to `1.5 64 1.5`. ARG-KW-002 has
the experiment for the anchor.

### CMD-EX-014: `positioned as <targets>`

- **Behavior:** find the entities of `targets`. Give one source for each entity, in order. Each
  source has the position of that entity.
- **Postconditions:** the source keeps its dimension, also when the entity is in another
  dimension.
- **Edge cases:** no entity gives an empty source list.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: put an armor
stand with the tag `n` in the Nether at `100 64 100`. From the Overworld, run
`execute positioned as @e[tag=n] run summon pig ~ ~ ~`. The pig appears in the Overworld at
`100 64 100`.

### CMD-EX-015: `positioned over <heightmap>`

- **Behavior:**
  1. Let `(x, y, z)` be the source position. If the chunk that contains the block column of
     `x` and `z` is not a loaded chunk, fail with `argument.pos.unloaded`.
  2. Let `h` be the heightmap value of the type `heightmap` at the block column of `x` and `z`
     (`heightmap.md`, HM-006).
  3. The source gets the position `(x, h, z)`. The values of x and z do not change.
- **Edge cases:** an empty column gives the minimum y of the dimension.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: on flat land
with the top block at y 63, run `execute positioned over world_surface run tp @s ~ ~ ~`. The
player moves to y 64. Then run
`execute positioned 100000 64 100000 positioned over world_surface run say a` where that chunk
is not loaded. The reply is `argument.pos.unloaded`.

### CMD-EX-016: `rotated <rot>` and `rotated as <targets>`

- **`rotated <rot>`:** resolve `rot` against the source (`argument-coordinates.md`,
  ARG-COORD-008). The source gets that rotation.
- **`rotated as <targets>`:** find the entities of `targets`. Give one source for each entity,
  in order. Each source has the rotation of that entity. No entity gives an empty source list.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: with the yaw 30,
run `execute rotated ~15 0 run tp @s ~ ~ ~ ~ ~`. The new yaw is 45 and the new pitch is 0.

### CMD-EX-017: `facing <pos>` and `facing entity <targets> <anchor>`

- **`facing <pos>`:** resolve `pos` against the source, with the center correction set. The
  source gets the rotation toward that point (CMD-EX-018).
- **`facing entity <targets> <anchor>`:** find the entities of `targets`. Give one source for
  each entity, in order. The target point of an entity is its position for `feet`. For `eyes`,
  it is its position plus its eye height on the vertical axis. Each source gets the rotation
  toward the target point of its entity.
- **Edge cases:** no entity gives an empty source list.
- **Postconditions:** the rotation starts at the anchor point of the source. Thus `anchored eyes`
  before `facing` changes the result.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: at `0.5 64 0.5`,
run `execute facing 0.5 64 10.5 run tp @s ~ ~ ~ ~ ~`. The yaw is 0 and the pitch is 0. Then run
`execute anchored eyes facing 0.5 64 10.5 run tp @s ~ ~ ~ ~ ~`. The pitch is positive: the
player looks down.

### CMD-EX-018: Rotation toward a point

- **Inputs:** the start point `F`, which is the anchor point of the source, and the target point
  `T`.
- **Behavior:** each step is in binary64 unless the step says otherwise.
  1. `dx = Tx − Fx`, `dy = Ty − Fy`, `dz = Tz − Fz`. `h = sqrt(dx · dx + dz · dz)`.
  2. Let `πf` be the binary32 value nearest to π, widened to binary64.
  3. The pitch is `W(f32(−(A(dy, h) · 180 / πf)))`. Compute the product, then the quotient,
     then the negation.
  4. The yaw is `W(f32(A(dz, dx) · 180 / πf) − 90)`. The subtraction of 90 is in binary32.
  5. `f32` rounds a binary64 value to the nearest binary32 value.
- **`W(a)`**, on a binary32 angle `a`: let `r` be the remainder of `a` divided by 360, with the
  sign of `a`. If `r ≥ 180`, subtract 360. If `r < −180`, add 360. The arithmetic is binary32.
  The result is in `[−180, 180)`.
- **`A(y, x)`**, the arctangent of the game, in binary64:
  1. Let `d = x · x + y · y`. If `d` is NaN, the result is NaN.
  2. If `y < 0`, make `y` positive and note the sign. Do the same for `x`.
  3. If `y > x`, swap `x` and `y`, and note the swap.
  4. Let `b` be the bits of `d` as a signed 64-bit integer. Let `g` be the binary64 value
     whose bits are `0x5FE6EB50C7B537AA − (b >> 1)`, with an arithmetic shift.
     Let `r = g · (1.5 − ((0.5 · d) · g) · g)`.
  5. `x = x · r` and `y = y · r`.
  6. Let `k` be `y · 256`, rounded to the nearest integer, ties to even. Let `s = k / 256`.
  7. Let `φ = asin(s)` and `c = cos(φ)`.
  8. Let `e = y · c − x · s`, and `θ = φ + ((6 + e · e) · e) · 0.16666666666666666`.
  9. If step 3 swapped, `θ = π/2 − θ`. If `x` was negative, `θ = π − θ`. If `y` was negative,
     `θ = −θ`. π is the binary64 value nearest to π.
- **Edge cases:**
  - When `T` equals `F`, `A` gives 0 for both angles. The yaw is −90, and the pitch is negative
    zero.
  - When `T` is straight above `F`, the pitch is −90 and the yaw is −90.
- **Precision:** a differential test compares each angle with a tolerance of one binary32 ulp.
  The values `asin(s)` and `cos(φ)` of the reference come from the Java platform. Their last
  bit can differ from a correctly rounded value. An exact arctangent in place of `A` is not
  allowed: the error of `A` can exceed one binary32 ulp (section 10).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: at `0.5 64 0.5`,
run `execute facing 10.5 64 0.5 run tp @s ~ ~ ~ ~ ~`. The yaw is −90. Then run
`execute facing ~ ~ ~ run tp @s ~ ~ ~ ~ ~`. The yaw is −90 and the pitch is 0.

### CMD-EX-019: `align <axes>`

- **Behavior:** for each axis in `axes`, the source position on that axis becomes the floor of
  its value. The other axes do not change. `argument-coordinates.md`, ARG-COORD-016, states
  the floor and its range.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: at
`1.7 64 -0.2`, run `execute align xz run tp @s ~ ~ ~`. The player moves to `1.0 64 -1.0`.

### CMD-EX-020: `anchored <anchor>`

The source gets the anchor `anchor`. ARG-KW-002 states the anchor lifetime and its experiment.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CMD-EX-021: `in <dimension>`

- **Behavior:**
  1. If the server has no dimension with the identifier `dimension`, fail with
     `argument.dimension.invalid`, with the identifier.
  2. If the dimension is the source dimension, the source does not change.
  3. Otherwise let `q` = the coordinate scale of the source dimension divided by the coordinate
     scale of the new dimension. The new x is `x · q`, and the new z is `z · q`. The y does not
     change. The source gets the new dimension.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the Overworld
at `800 64 -160`, run `execute in minecraft:the_nether run tp @s ~ ~ ~`. The player moves to the
Nether at `100 64 -20`.

### CMD-EX-022: `summon <entity>`

- **Behavior:**
  1. If the entity type cannot be summoned, fail with `entity.not_summonable`, with the
     identifier of the type.
  2. If the block of the source position is outside the spawnable bounds, fail with
     `commands.summon.invalidPosition`.
  3. If the difficulty of the source dimension is peaceful and the type is not allowed in
     peaceful, fail with `commands.summon.failed.peaceful`.
  4. Make an entity of the type at the source position, with the spawn reason "command". The
     entity keeps the rotation that its type gives it. It does not take the source rotation.
     If the game cannot make the entity, fail with `commands.summon.failed`.
  5. A mob gets the first spawn setup of its type, as for the `summon` command.
  6. Add the entity and its passengers to the source dimension. If an entity with the same UUID
     exists, fail with `commands.summon.failed.uuid`.
  7. The source gets the new entity.
- **Postconditions:** the command sends no message for the new entity. The `summon` command
  sends `commands.summon.success`, but `execute summon` does not.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`execute summon minecraft:pig run tag @s add fresh`. One pig appears with the tag `fresh`. The
chat shows the message of `tag`, and no message of `summon`.

### CMD-EX-023: `on <relation>`

- **Behavior:** if the source has no entity, give an empty source list. Otherwise find the
  related entities of the source entity. Remove each entity that the game has removed from the
  world. Give one source for each entity that remains, in order.

| Relation | The related entities of an entity `e` |
|----------|----------------------------------------|
| `owner` | the owner of `e`, when `e` is an entity that can have an owner |
| `leasher` | the entity that holds the leash of `e` |
| `target` | the attack target of `e`, when `e` is a mob that can have one |
| `attacker` | the last entity that attacked `e`, when `e` keeps that entity |
| `vehicle` | the entity that `e` rides |
| `controller` | the passenger that controls `e` |
| `origin` | the entity that made `e`, for example the shooter of a projectile |
| `passengers` | the direct passengers of `e`, in the order of the passenger list |

- **Postconditions:** each relation except `passengers` gives zero or one entity.

The entity specifications must state which entities have each relation. Section 8 lists this
gap.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: ride a pig, and
run `execute on vehicle run tag @s add v`. The pig gets the tag. Then run
`execute as @e[type=pig,limit=1] on passengers run say a`. The player says `a`.

---

## 3. The run of a chain

These rules apply to every chain, not only to `execute`. `command-parser.md`, CMD-EXEC-002,
leaves them to a specification of the executor. This document states them until that
specification exists.

### CMD-EX-030: Stages run in order, each over the whole source list

- **Behavior:**
  1. The first source list holds the source of the command line.
  2. For each modifier stage in order, apply the modifier to each source of the list, in
     order. The new list is the concatenation of the results, in that order.
  3. A stage with no source modifier, for example `run`, keeps the list.
  4. After the last modifier stage, run the run stage for each source of the list
     (CMD-EX-035).
- **Postconditions:** each modifier stage ends for all sources before the next stage starts. No
  run starts before the last modifier stage ends.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: with two players
online and no pig loaded, run `execute as @a unless entity @e[type=pig] run summon pig`. Two
pigs appear. A run of each source to the end before the next source gives one pig.

### CMD-EX-031: Values resolve when their stage runs

- **Behavior:** a modifier stage resolves its arguments for each source when that stage runs,
  against that source. This includes relative and local coordinates, rotations, selectors and
  `@s`. The run stage resolves its arguments in the same way, against each source of the last
  list.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: with two pigs
near, run `execute as @e[type=pig,limit=2] at @s run summon cow ~ ~ ~`. One cow appears at each
pig.

### CMD-EX-032: Forked chain

- **Behavior:** go through the modifier stages in order. When the last node of a modifier stage
  is a fork node, that stage and every later stage are forked. The run stage is forked when a
  modifier stage before it is forked.
- **Edge cases:** a condition that is the run stage does not make the chain forked. Thus in
  `execute if entity @s`, the run stage is not forked. In `execute as @s if entity @s`, the
  run stage is forked.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`execute run tp @s 0 25000000 0`. The reply is `commands.teleport.invalidPosition`. Then run
`execute as @s run tp @s 0 25000000 0`. No message arrives.

### CMD-EX-033: An error in a modifier stage

- **Behavior:** when the modifier of a stage fails for one source:
  - in a stage that is not forked, the output of that source receives the error as a system
    message in red (`commands-packet.md`, CMDP-EXEC-004). The command stops. No later stage
    runs, and no source runs the run stage;
  - in a forked stage, no message is sent. The source has no result in the new list. The stage
    continues with the next source.
- **Postconditions:** no store target receives a result for a source that a modifier error
  removed.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`execute in minecraft:nowhere run say a`. The reply is `argument.dimension.invalid` with
`minecraft:nowhere`. Then run `execute as @s in minecraft:nowhere run say a`. No message
arrives.

### CMD-EX-034: An empty source list

- **Behavior:** when the source list of a stage is empty, no later modifier runs, and the run
  stage does not run.
- **Postconditions:** the command sends no message and gives no error. No store target receives
  a result.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`scoreboard objectives add o dummy`, `scoreboard players set #s o 5` and
`execute store success score #s o as @e[tag=none] run say a`. No message arrives, and `#s` stays
5.

### CMD-EX-035: The run stage

- **Behavior:** for each source of the last source list, in order:
  1. Run the command of the run stage with that source.
  2. The run gives a result (CMD-EX-040). Pass it to the store targets of the source
     (CMD-EX-041).
  3. If the run failed and the run stage is not forked, the output receives the error as a
     system message in red. If the run stage is forked, no message is sent, and the next source
     runs.
- **Postconditions:** the run of one source, with all work that it starts, ends before the run
  of the next source starts. An example of such work is a function that the command calls.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: make a function
`t:f` with the lines `say a` and `say b`. With two pigs near, run
`execute as @e[type=pig,limit=2] run function t:f`. The chat lines are `a`, `b`, `a`, `b`.

### CMD-EX-036: Fork limit

- **Inputs:** the limit `L`, the value of the game rule `max_command_forks`. The default is
  65536, and the minimum is 0.
- **Behavior:** in a stage that has a source modifier, after the modifier of each source, let
  `n` be the length of the new list with the new sources. If `n ≥ L`, the command stops. The
  error is `command.forkLimit`, with `L`. The output of the original source receives it only
  when the stage is not forked.
- **Postconditions:**
  - A source list never holds `L` or more sources. With `L ≤ 1`, every stage that has a source
    modifier fails.
  - When the command stops, no source runs the run stage, also a source that a stage gave
    before the limit.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`gamerule max_command_forks 2`. With two pigs near, run `execute as @e[type=pig] run say a`. No
pig speaks, and no message arrives. With one pig, the pig speaks. Then run
`gamerule max_command_forks 1` and `execute positioned ~ ~ ~ run say a`. The reply is
`command.forkLimit` with 1.

### CMD-EX-037: Sequence limit

- **Inputs:** the limit `M`, the larger of 1 and the game rule `max_command_sequence_length`.
  The default of the game rule is 65536.
- **Behavior:**
  - Each modifier stage that has a source modifier costs 1, for any number of sources.
  - Each run of the run stage costs 1.
  - Before each run of the run stage, if the cost so far is `M` or more, the command stops.
    The server writes a log line. The output receives no message.
- **Postconditions:** a chain with `k` such modifier stages and `n` sources in its last list
  runs the run stage `max(0, min(n, M − k))` times. A function that a run calls adds its own
  cost to the same count.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`gamerule max_command_sequence_length 3`. With five pigs near, run
`execute as @e[type=pig] run say a`. Exactly two pigs speak.

---

## 4. Results

### CMD-EX-040: Result of one run

- **Behavior:** a run gives a result:
  - when the command completes with no error: success true, and the value of the command;
  - when the command fails with an error: success false, and the value 0.
- **Postconditions:**
  - `execute` has no result of its own. It gives no sum and no count of its runs. A caller that
    observes the results receives one result for each run.
  - A command block counts the results with success true. That count is its success count.
  - The value of a command and its success are independent. A command can complete with the
    value 0 and success true.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in a command
block, with three pigs near, run `execute as @e[type=pig] run say a`. Then run
`data get block <pos> SuccessCount` on that block. The value is 3.

### CMD-EX-041: Store targets

- **Behavior:**
  - A `store` stage resolves its target for the source, and attaches a store target to the
    source. CMD-EX-043 to CMD-EX-045 state what is resolved at this time.
  - Every later stage keeps the store targets. A fork gives each new source all the store
    targets of its source.
  - Each result of a run goes to each store target of its source. The store target that the
    first `store` node of the command line added receives the result first.
- **Postconditions:**
  - A store target receives one result for each run of a source that has it. When a fork gives
    many sources, the last run writes the final value.
  - A store target receives no result for a source that a modifier removed (CMD-EX-033,
    CMD-EX-034).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: put armor stands
at `0 64 0`, `0 70 5` and `0 80 10`. At `0 64 0`, run
`execute store result score #y o as @e[type=armor_stand,sort=nearest] run data get entity @s Pos[1]`.
`#y` is 80. Then run
`execute store result score #v o store success score #v o run data get entity @e[type=armor_stand,sort=nearest,limit=1] Pos[1]`.
`#v` is 1, because the success target writes last.

### CMD-EX-042: The value that `store` writes

- `store result` writes the value of the result.
- `store success` writes 1 when the success is true, and 0 otherwise.
- On a failed run, both write 0.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: with the
experience level 0, run `execute store success score #s o run experience query @s levels`. `#s`
is 1. Then run `execute store result score #r o run kill @e[tag=none]`. `#r` is 0, and the reply
is `argument.entity.notfound.entity`.

### CMD-EX-043: `store ... score <targets> <objective>`

- **At the `store` stage:** resolve the score holders of `targets` and the objective, for the
  source. An error here is an error of the modifier stage (CMD-EX-033).
- **For each result:** for each score holder, in order, set its score in the objective to the
  value of CMD-EX-042. A score that does not exist is made.
- **Edge cases:** the objective can have a read-only criterion. The store target writes it.

The argument types `score_holder` and `objective` and the scoreboard have no specification yet
(section 8).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`scoreboard objectives add h health` and
`execute store result score @s h run experience query @s levels` with the level 5. Read the
score of `@s` in `h` at once. It is 5.

### CMD-EX-044: `store ... bossbar <id> value|max`

- **At the `store` stage:** find the boss bar `id`. If it does not exist, fail with
  `commands.bossbar.unknown`, with the identifier.
- **For each result:** `value` sets the value of the boss bar. `max` sets its maximum. The boss
  bar specification states the progress that follows. No such specification exists yet.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`bossbar add t:b "b"` and `execute store result bossbar t:b max run experience query @s levels`
with the level 7. Then `bossbar get t:b max` gives 7.

### CMD-EX-045: `store ... entity|block|storage <target> <path> <type> <scale>`

- **At the `store` stage:**
  - `entity`: find exactly one entity (`argument-entity.md`, ARG-ENT-016).
  - `block`: read the block position (CMD-EX-050). If the position has no block entity, fail
    with `commands.data.block.invalid`.
  - `storage`: note the identifier.
- **For each result:**
  1. Let `v` = the value of CMD-EX-042, times `scale`, in binary64.
  2. Convert `v` to a tag of the type:

| Type | Tag value |
|------|-----------|
| `int` | `v` truncated toward zero to a 32-bit integer; values outside the range become the nearest bound; NaN becomes 0 |
| `long` | `v` truncated toward zero to a 64-bit integer, with the same rules |
| `short` | `v` converted as for `int`, then the low 16 bits as a signed value |
| `byte` | `v` converted as for `int`, then the low 8 bits as a signed value |
| `float` | `v` rounded to the nearest binary32 value |
| `double` | `v` |

  3. Read the data of the target. Set the element at `path` to the tag. Write the data back.
  4. If a step fails, the store target does nothing. It sends no message, and the run keeps
     its result.
- **Edge cases:** the data of a player cannot be written. A store into an entity that is a
  player does nothing.

The argument type `nbt_path`, the saved data of entities and block entities, and the command
storage have no specification yet (section 8).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: with the level
300, run `execute store result storage t:s v byte 1 run experience query @s levels`.
`data get storage t:s v` gives `44b`. Then run
`execute store result storage t:s w double 0.1 run experience query @s levels`.
`data get storage t:s w` shows `30.000000000000004d`.

---

## 5. Conditions

### CMD-EX-050: Read a block position

Some conditions and targets read a block position that must be readable. To read a block
position `p` against the source:

1. Resolve `p` (`argument-coordinates.md`, ARG-COORD-011 and ARG-COORD-012).
2. If the chunk of `p` is not a loaded chunk of the source dimension, fail with
   `argument.pos.unloaded`.
3. If `p` is outside the build height of the dimension, or outside `−30 000 000 ≤ x <
   30 000 000` and `−30 000 000 ≤ z < 30 000 000`, fail with `argument.pos.outofworld`.

Steps 2 and 3 are the loaded check of ARG-COORD-013.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`execute if block ~ 1000 ~ minecraft:air`. The reply is `argument.pos.outofworld`. Run
`execute if block 100000 64 100000 minecraft:stone` where that chunk is not loaded. The reply is
`argument.pos.unloaded`.

### CMD-EX-051: A condition as a modifier

- **Behavior:** for each source, compute the condition. When the condition equals the expected
  value, the source goes to the new list with no change. Otherwise the source is removed.
- **Postconditions:** a condition node is a fork node, so the stage is forked (CMD-EX-032). An
  error of the condition removes the source with no message (CMD-EX-033).
- **Edge cases:** for a counting condition, the condition is true when the count is greater
  than 0. The `blocks` condition is true when the regions match (CMD-EX-058).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`execute if block 100000 64 100000 minecraft:stone run say a`, where that chunk is not loaded.
No message arrives.

### CMD-EX-052: A condition as the run stage

- **Behavior of a boolean condition:**
  - When the condition equals the expected value: send `commands.execute.conditional.pass`.
    The value is 1.
  - Otherwise fail with `commands.execute.conditional.fail`.
- **Behavior of a counting condition under `if`:**
  - When the count is greater than 0: send `commands.execute.conditional.pass_count` with the
    count. The value is the count.
  - Otherwise fail with `commands.execute.conditional.fail`.
- **Behavior of a counting condition under `unless`:**
  - When the count is 0: send `commands.execute.conditional.pass`. The value is 1.
  - Otherwise fail with `commands.execute.conditional.fail_count`, with the count.
- **Postconditions:** each message is a success message with the broadcast flag clear
  (`commands-packet.md`, CMDP-EXEC-004). In a forked chain, each source that passes sends its
  own message.
- **Edge cases:** `blocks` follows CMD-EX-058 for its count.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: with three pigs
near, run `execute if entity @e[type=pig]`. The reply is
`commands.execute.conditional.pass_count` with 3. Run `execute unless entity @e[type=pig]`. The
reply is `commands.execute.conditional.fail_count` with 3. Run
`execute store result score #e o if entity @e[type=pig]`. `#e` is 3.

### CMD-EX-053: `entity <entities>`

The count is the number of entities that the search of `entities` finds for the source. No
entity gives 0, not an error.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`execute unless entity @e[tag=none]`. The reply is `commands.execute.conditional.pass`.

### CMD-EX-054: `dimension <dimension>`

- **Behavior:** if the server has no dimension `dimension`, fail with
  `argument.dimension.invalid`, with the identifier. Otherwise the condition is true when the
  source dimension is that dimension.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the
Overworld, run `execute in minecraft:the_nether if dimension minecraft:the_nether`. The reply is
`commands.execute.conditional.pass`.

### CMD-EX-055: `loaded <pos>`

- **Behavior:** resolve `pos`. The condition is true when the chunk that contains `pos` is a
  ticking chunk of the source dimension.
- **Edge cases:** the condition does not test y or the world bounds, and it gives no error for
  a position that CMD-EX-050 rejects.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`execute if loaded ~ 99999 ~`. The reply is `commands.execute.conditional.pass`. Run
`forceload add 10000 10000`, wait one second, and run `execute if loaded 10000 64 10000`. The
reply is `commands.execute.conditional.pass`.

### CMD-EX-056: `block <pos> <block>`

- **Behavior:** read the block position `pos` (CMD-EX-050). The condition is true when the
  block predicate `block` matches the block at `pos`. The predicate can test the block state,
  the tags and the data of the block entity.

`argument-coordinates.md`, ARG-COORD-011, states `block_pos`. `argument-block.md` states
`block_predicate`; its `{...}` part reads SNBT (`snbt.md`).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: stand on a grass
block, and run `execute if block ~ ~-1 ~ minecraft:grass_block`. The reply is
`commands.execute.conditional.pass`.

### CMD-EX-057: `biome <pos> <biome>`

- **Behavior:** read the block position `pos` (CMD-EX-050). The condition is true when the biome
  of the source dimension at `pos` is the entry `biome`, or is in the tag `biome`.

The biome that the game gives at a block position has no specification yet (section 8).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in a plains
biome, run `execute if biome ~ ~ ~ minecraft:plains`. The reply is
`commands.execute.conditional.pass`.

### CMD-EX-058: `blocks <start> <end> <destination> all|masked`

- **Behavior:**
  1. Read the block positions `start`, `end` and `destination` (CMD-EX-050).
  2. The source box has the corners `start` and `end`. The destination box has the same size,
     and `destination` is its corner with the smallest x, y and z.
  3. Let `n` be the number of blocks in the source box. If `n > 32768`, fail with
     `commands.execute.blocks.toobig`, with 32768 and `n`.
  4. Compare each position of the source box with the position at the same offset in the
     destination box. With `masked`, skip each source position whose block is `minecraft:air`.
  5. Two positions match when their block states are equal and one of these is true:
     - the source position has no block entity;
     - the destination has a block entity of the same type, with equal components and equal
       saved data.
  6. When all compared positions match, the regions match, and the count is the number of
     compared positions. Otherwise the regions do not match.
- **Edge cases:**
  - `masked` skips only `minecraft:air`, not `minecraft:cave_air` or `minecraft:void_air`.
  - A destination block entity with no source block entity does not prevent a match.
  - Under `if` as the run stage, regions that match with the count 0 pass. The message is
    `commands.execute.conditional.pass_count` with 0, and the value is 0.
  - Under `unless` as the run stage, matching regions fail with
    `commands.execute.conditional.fail_count`, with the count. Other regions pass with the
    value 1.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in an area of
air, run `execute if blocks ~ ~10 ~ ~2 ~12 ~2 ~5 ~10 ~5 masked`. The reply is
`commands.execute.conditional.pass_count` with 0. Then run
`execute if blocks ~ ~ ~ ~32 ~32 ~32 ~ ~40 ~ all`. The reply is `commands.execute.blocks.toobig`
with 32768 and 35937.

### CMD-EX-059: `score`

- **Comparison forms:** read the score of `target` in `targetObjective` and the score of
  `source` in `sourceObjective`. When either score does not exist, the condition is false.
  Otherwise compare the first score to the second with `=`, `<`, `<=`, `>` or `>=`.
- **`matches <range>`:** when the score of `target` in `targetObjective` does not exist, the
  condition is false. Otherwise the condition is true when the score is in `range`, both bounds
  included.

The argument types `score_holder`, `objective` and `int_range`, and the scoreboard, have no
specification yet (section 8).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`scoreboard players set A o 1` and `scoreboard players set B o 2`. `execute if score A o < B o`
passes. `execute unless score C o = A o` passes, because `C` has no score.

### CMD-EX-060: `predicate <predicate>`

The condition is true when the loot predicate passes. The loot context has the source position
as `origin` and the source entity, if any, as `this`. Its parameter set is `minecraft:command`.

The argument type `loot_predicate` and loot predicates have no specification yet (section 8).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: with a predicate
`t:p` of the condition `minecraft:random_chance` with the chance 1, run
`execute if predicate t:p`. The reply is `commands.execute.conditional.pass`.

### CMD-EX-061: `function <name>`

- **Behavior:** the condition is a modifier only. For the whole stage:
  1. Find the functions of `name` for the original source of the chain. If `name` names no
     function and no tag, the command stops. The error is `arguments.function.unknown` or
     `arguments.function.tag.unknown`, with the identifier. The stage is forked
     (CMD-EX-032), so no message is sent.
  2. If the list of functions is empty, the command stops. No source passes, under `if` or
     under `unless`.
  3. Prepare each function with no macro arguments. When a function needs macro arguments,
     the error is `commands.execute.function.instantiationFailure`, with the identifier of the
     function and the reason. No message is sent. Section 10 states what follows.
  4. For each source in order, call the functions in order as one nested call. The called
     source has no store targets, has its output silent, and has at least the permission level
     `gamemasters`. A `return` in a function ends the nested call with its value. When no
     function returns, the call gives failure and the value 0.
  5. A source passes when the value is not 0 under `if`, and when the value is 0 under
     `unless`. A source that passes keeps its store targets.
  6. After all calls, the next stage runs with the sources that passed, in order.

The argument type `function`, functions, function tags, macros and `return` have no
specification yet (section 8).

Provenance: `source_suggested`. Kind: semantic. Confidence: low. Experiment: make a function
`t:one` with the line `return 1`, and a function `t:none` with the line `say x`.
`execute if function t:one run say a` says `a`. `execute if function t:none run say a` says
nothing, and the line `say x` sends no message.

### CMD-EX-062: `stopwatch <id> <range>`

- **Behavior:** if the stopwatch `id` does not exist, fail with
  `commands.stopwatch.does_not_exist`, with the identifier. Otherwise the condition is true when
  the elapsed time of the stopwatch, in seconds, is in `range`.

The argument type `float_range` and stopwatches have no specification yet (section 8).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`execute if stopwatch t:none 0..`. The reply is `commands.stopwatch.does_not_exist` with
`t:none`.

### CMD-EX-063: `data entity|block|storage <source> <path>`

- **Behavior:**
  - `entity`: find exactly one entity (`argument-entity.md`, ARG-ENT-016).
  - `block`: read the block position (CMD-EX-050). If the position has no block entity, fail
    with `commands.data.block.invalid`.
  - `storage`: the command storage with that identifier. A storage that does not exist reads
    as an empty compound.
- **Count:** the number of elements that `path` selects in the data of the source.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`execute if data entity @s Inventory[]` with three stacks in the inventory. The reply is
`commands.execute.conditional.pass_count` with 3.

### CMD-EX-064: `items` and `slots`

- **Targets:**
  - `entity <source>`: find the entities of `source`. When there is none, fail with
    `argument.entity.notfound.entity`.
  - `block <source>`: read the block position (CMD-EX-050). If the block entity at that
    position is not a container, fail with `commands.item.source.not_a_container`, with x, y
    and z.
- **Slots:** the slot source `slots` selects slots of each target, in the order of the targets.
  The loot context has the source position as `origin`, and the source entity, if any, as
  `this`.
- **Count of `items`:** the sum of the stack sizes of the selected items that match
  `item_predicate`.
- **Count of `slots`:** the number of selected slots.

The argument types `slot_source` and `item_predicate` have no specification yet (section 8).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: hold 5 dirt in
the main hand, and run `execute if items entity @s weapon.mainhand minecraft:dirt`. The reply is
`commands.execute.conditional.pass_count` with 5.

---

## 6. Messages and errors

### CMD-EX-070: Messages

| Translation key | Arguments | When |
|-----------------|-----------|------|
| `commands.execute.conditional.pass` | none | a boolean condition passes; a counting condition passes under `unless` |
| `commands.execute.conditional.pass_count` | the count | a counting condition passes under `if` |

Each message is a success message with the broadcast flag clear.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: as CMD-EX-052.

### CMD-EX-071: Errors

The errors of this document have no position (`command-parser.md`, CMD-ERR-001).

| Translation key | Arguments, in order | Rule |
|-----------------|---------------------|------|
| `commands.execute.conditional.fail` | none | CMD-EX-052 |
| `commands.execute.conditional.fail_count` | the count | CMD-EX-052, CMD-EX-058 |
| `commands.execute.blocks.toobig` | 32768, the number of blocks | CMD-EX-058 |
| `commands.execute.function.instantiationFailure` | the function identifier, the reason | CMD-EX-061 |
| `arguments.function.unknown` | the identifier | CMD-EX-061 |
| `arguments.function.tag.unknown` | the identifier | CMD-EX-061 |
| `command.forkLimit` | the limit | CMD-EX-036 |
| `argument.pos.unloaded` | none | CMD-EX-015, CMD-EX-050 |
| `argument.pos.outofworld` | none | CMD-EX-050 |
| `argument.dimension.invalid` | the identifier | CMD-EX-021, CMD-EX-054 |
| `entity.not_summonable` | the identifier of the type | CMD-EX-022 |
| `commands.summon.invalidPosition` | none | CMD-EX-022 |
| `commands.summon.failed.peaceful` | none | CMD-EX-022 |
| `commands.summon.failed` | none | CMD-EX-022 |
| `commands.summon.failed.uuid` | none | CMD-EX-022 |
| `commands.bossbar.unknown` | the identifier | CMD-EX-044 |
| `commands.data.block.invalid` | none | CMD-EX-045, CMD-EX-063 |
| `commands.item.source.not_a_container` | x, y, z | CMD-EX-064 |
| `commands.stopwatch.does_not_exist` | the identifier | CMD-EX-062 |

The errors of the argument types also apply: `argument-entity.md`, ARG-ENT-012 and
ARG-ENT-016, and the documents of section 8.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: run each experiment
of the rule column, and read the key and the arguments of each reply.

---

## 7. Intentional deviations

No rule of this document corrects the reference. Section 10 lists the cases where the correct
behaviour is not known.

---

## 8. Core set and dependencies

### 8.1 Core set

A first implementation can take these branches:

- `run`;
- `as`, `at`;
- `positioned <pos>`, `positioned as`, `positioned over`;
- `rotated <rot>`, `rotated as`;
- `facing <pos>`, `facing entity`;
- `align`;
- `anchored`;
- `in`;
- `if` and `unless` with `entity`, `dimension`, `loaded` and `block`.

The core set also needs sections 3 and 4 without the `store` targets, and CMD-EX-050 to
CMD-EX-056. The core set has no `store` node, so no store target exists. The results of
CMD-EX-040 still reach the caller, for example a command block.

Each argument type of the core set has a specification: `minecraft:entity`
(`argument-entity.md`); `minecraft:vec3`, `minecraft:rotation`, `minecraft:block_pos` and
`minecraft:swizzle` (`argument-coordinates.md`); `minecraft:entity_anchor` and
`minecraft:heightmap` (`argument-keywords.md`); `minecraft:block_predicate`
(`argument-block.md`, with SNBT in `snbt.md`); and `minecraft:dimension`
(`argument-resource.md`). `heightmap.md` states the heightmap values. Until a branch exists, the tree must not contain it.

### 8.2 Other branches

| Branch | Argument types without a specification | Subsystems |
|--------|----------------------------------------|------------|
| `summon` | none: `minecraft:resource` is in `command-time.md` | entity creation of a `summon` specification, the summonable flag of entity types, the first spawn setup of mobs, difficulty |
| `on` | none | the relations of entities: owner, leash, attack target, last attacker, vehicle, controlling passenger, origin |
| `if biome` | none | the biome at a block position |
| `if score` | `minecraft:score_holder`, `minecraft:objective`, `minecraft:int_range` | scoreboard; server suggestions of score holders |
| `if blocks` | none | block entities, their components and their saved data |
| `if predicate` | `minecraft:loot_predicate` | loot predicates and loot contexts |
| `if function` | `minecraft:function` | functions, function tags, macros, `return`, nested calls; server suggestions of functions |
| `if stopwatch` | `minecraft:float_range` | stopwatches; server suggestions of stopwatches |
| `if data` | `minecraft:nbt_path` | saved data of entities and block entities, command storage; server suggestions of storage |
| `if items`, `if slots` | `minecraft:slot_source`, `minecraft:item_predicate` | containers, slots of entities, item components |
| `store ... score` | `minecraft:score_holder`, `minecraft:objective` | scoreboard |
| `store ... bossbar` | none | boss bars; server suggestions of boss bars |
| `store ... entity`, `block`, `storage` | `minecraft:nbt_path` | saved data, command storage |

---

## 9. Conformance tests

A conformance test runs each experiment of this document on the reference and on the
implementation, and compares:

- the tree of CMD-EX-001 to CMD-EX-005 for the branches that the implementation has;
- the messages, their keys and their arguments;
- the world state after the command, and the values of the store targets.

A differential test of `facing` compares the angles with the tolerance of CMD-EX-018. It does
not compare the order of entities that `argument-entity.md` leaves unspecified.

---

## 10. Open questions

- **Partial preparation of `if function`.** When a tag has a function that needs macro
  arguments, the reference reports the error and still calls the functions of the tag before
  that function. The correct behaviour is not known: the stage can stop, or it can call no
  function. Experiment: make a tag `#t:mix` with `t:one` (`say one`, then `return 1`) and then
  `t:macro` (`$say $(x)`). Run `execute as @s if function #t:mix run say a`. Record whether `one`
  and `a` appear.
- **Blocks inside the box of `if blocks`.** CMD-EX-058 reads only the three corner positions
  with CMD-EX-050. A position inside the boxes can be in a chunk that is not loaded. The
  behaviour of the reference for such a position is not known. Experiment: with the boxes
  `0 64 0` to `5000 64 0` and `0 65 0`, unload the chunks between the ends, and run the
  condition. Record the reply and whether chunks load.
- **Error of the arctangent of the game.** CMD-EX-018 requires the procedure `A`. The largest
  difference between `A` and an exact arctangent, after the conversion to binary32 degrees, is
  not measured. Experiment: compute both over a grid of `(y, x)` with the procedure of
  CMD-EX-018, and record the largest difference in binary32 ulps.
- **Fork limit in a forked stage.** CMD-EX-036 sends no message when a forked stage reaches the
  limit. Whether the player sees any sign of the stop is not confirmed. Experiment: as in
  CMD-EX-036, record all system messages and the server log.
- **Read-only objectives.** CMD-EX-043 writes an objective with a read-only criterion. How long
  the stored value stays before the game updates the criterion is not known. Experiment: as in
  CMD-EX-043, read the score each tick for 20 ticks.
