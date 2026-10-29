# Coordinate arguments: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |
| Parser ids | `minecraft:block_pos` 8, `minecraft:vec3` 10, `minecraft:rotation` 29, `minecraft:swizzle` 32 (`commands-packet.md`, CMDP-ARG-001) |

Source of the version values: `assets/minecraft/version.json`. Source of the parser ids:
`assets/mcrs/reports/registries.json`, through `commands-packet.md`, CMDP-ARG-001.

This document states four argument types: a position in three axes (`vec3`), a block position
(`block_pos`), a rotation in two axes (`rotation`), and a set of axes (`swizzle`). It states the
text that each accepts, the errors and their positions, the suggestions, and how a command
resolves the value against the source. It does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Reader**, **cursor**, **position**, **source**, **error**: as `command-parser.md`,
  section 0.1, defines them.
- **World coordinate**: one axis value that is absolute or relative to the source.
- **Relative**: a world coordinate with the prefix `~`.
- **Local coordinates**: three axis values with the prefix `^`, along the axes of the source
  rotation.
- **Center correction**: the addition of 0.5 to a whole absolute horizontal value
  (ARG-COORD-003).
- **Anchor**: the point of the source that local coordinates start from: its feet or its eyes.
  The `execute anchored` command changes it. Each source starts at the feet: a player, the
  server console, an entity and a command block.
- **Yaw**: the rotation around the vertical axis, in degrees.
- **Pitch**: the rotation up and down, in degrees.
- **Block position**: three signed 32-bit integers `x`, `y` and `z`. It names one block.
- **Block floor**: of a binary64 value `v`, the largest integer that is not more than `v`, limited
  to the range −2 147 483 648 to 2 147 483 647. A value above the range gives 2 147 483 647. A
  value below the range gives −2 147 483 648.
- **Loaded chunk**: as `command-execute.md`, section 0.1, defines it.
- **Build height**: the values of `y` from `min_y` to `min_y + height − 1`, where `min_y` and
  `height` are the fields of the dimension type of the dimension.
- **Axis set**: a set of the axes x, y and z. Each axis is in the set one time or not at all.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. None of the experiments has been run.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `ARG-COORD` | world coordinates, local coordinates, `vec3`, `rotation`, `block_pos`, `swizzle` |
| `ARG-COORD-DEV` | intentional deviations from the reference |

---

## 1. Syntax

### ARG-COORD-001: One world coordinate

- **Inputs:** a flag `center` that enables the center correction for this axis.
- **Behavior:**
  1. If next is `^`, fail with `argument.pos.mixed` at the cursor.
  2. At the end of the command line, fail with `argument.pos.missing.double` at the end.
  3. If next is `~`, skip it. The value is relative.
  4. If the value is relative: at the end of the command line, or if next is a separator, the
     value is relative 0. Otherwise read a double (`command-parser.md`, CMD-RD-004).
  5. If the value is absolute and next is a separator, fail with `reader_expected_double` at
     the cursor (ARG-COORD-DEV-001). Otherwise read a double.
  6. If the value is absolute, `center` is set, and the number text has no `.`, add 0.5.
- **Edge cases:**

| Text | Value |
|------|-------|
| `~` | relative 0 |
| `~-2.5` | relative −2.5 |
| `~x` | `reader_expected_double` after `~` |
| `5`, with `center` | absolute 5.5 |
| `5.0`, with `center` | absolute 5.0 |
| `-0`, with `center` | absolute 0.5 |
| `5`, without `center` | absolute 5.0 |
| a separator at the cursor | `reader_expected_double` at the cursor |

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator at `0.5 64 0.5`, send `teleport 5 64 5`, then `teleport 5.0 64 5.0`, then `teleport ~
~1 ~`. The positions are `5.5 64 5.5`, `5.0 64 5.0` and `5.0 65 5.0`.

### ARG-COORD-002: Local coordinates

- **Behavior:** note the start.
  1. For each of three axes, in the order left, up, forwards:
     1. At the end of the command line, fail with `argument.pos.missing.double` at the end.
     2. If next is not `^`, set the cursor back to the start, and fail with `argument.pos.mixed`.
     3. Skip `^`. If the reader is not at the end and next is not a separator, read a double.
        Otherwise the value is 0.
  2. Between two axes: if next is a separator, skip it. Otherwise set the cursor back to the
     start, and fail with `argument.pos3d.incomplete`.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `summon pig ^ ~ ^`. The reply has `argument.pos.mixed`, and its context starts at
the first `^`. Then send `teleport ^1 ^ ^`: the player moves one block to the left.

### ARG-COORD-003: `vec3`

- **Inputs:** a flag `center` of the argument. It is set unless the postconditions state
  otherwise.
- **Behavior:** note the start.
  - If next is `^`, read local coordinates (ARG-COORD-002).
  - Otherwise read three world coordinates. x and z use the flag `center` of the argument. y
    has `center` clear. Between two coordinates: if next is a separator, skip it. Otherwise
    set the cursor back to the start, and fail with `argument.pos3d.incomplete`.
- **Postconditions:** `vec3` on the wire has no properties (`commands-packet.md`,
  CMDP-ARG-001). Thus the client always reads a `vec3` with `center` set. Center correction is
  a property of the server command, not of the wire. The server applies the flag of its own
  command. A value that is a size, not a position, has `center` clear: vanilla does this for the
  `delta` argument of `particle`. The flag never changes whether a text is valid, so the client
  and the server agree on errors. The client uses no value of a `vec3`.
- **Edge cases:**

| Text | Result |
|------|--------|
| `1 2 3` | absolute 1.5, 2.0, 3.5 |
| `~ ~1 ~` | relative 0, 1, 0 |
| `^ ^ ^5` | local 0, 0, 5 |
| `~ ^ ~` | `argument.pos.mixed` at the `^` |
| `^ ~ ^` | `argument.pos.mixed` at the start |
| `1 2` | `argument.pos3d.incomplete` at the start |
| `1  2 3` | `reader_expected_double` at the second separator (ARG-COORD-DEV-001) |

- **Examples:** `0 0 0`, `~ ~ ~`, `^ ^ ^`, `^1 ^ ^-5`, `0.1 -0.5 .9`, `~0.5 ~1 ~-5`.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `summon pig 1 2` and `summon pig ~ ^ ~`. The first reply has
`argument.pos3d.incomplete` with the context at `1`. The second reply has `argument.pos.mixed`
with the context at `^`.

### ARG-COORD-004: `rotation`

- **Behavior:** note the start.
  1. At the end of the command line, fail with `argument.rotation.incomplete` at the end.
  2. Read a world coordinate without `center`: the yaw.
  3. If next is a separator, skip it. Otherwise set the cursor back to the start, and fail
     with `argument.rotation.incomplete`.
  4. Read a world coordinate without `center`: the pitch.
- **Edge cases:** `^` in either place fails with `argument.pos.mixed`. Local rotation does not
  exist.
- **Examples:** `0 0`, `~ ~`, `~-5 ~5`.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `teleport @s ~ ~ ~ 90 0` and `teleport @s ~ ~ ~ 90`. The first sets the yaw to 90
and the pitch to 0. The second reply has `argument.rotation.incomplete`.

---

## 2. Suggestions

### ARG-COORD-005: Suggestions

- **Behavior:** let `r` be the remaining input from the start of the argument. The candidate
  coordinates are `^ ^ ^` when `r` starts with `^`. Otherwise they are the coordinates that the
  source offers: `~ ~ ~` on the server. The client selects its own candidates, for example the
  point under its crosshair. The text of these candidates is unspecified.
  - For `vec3`, with candidate `(a, b, c)`:
    - `r` empty: if `vec3` reads `a b c` without an error, suggest `a`, `a b` and `a b c`;
    - `r` with one field `f`: if `vec3` reads `f b c` without an error, suggest `f b` and
      `f b c`;
    - `r` with two fields `f g`: if `vec3` reads `f g c` without an error, suggest `f g c`.
  - A field ends at a separator. A separator at the end of `r` does not start a new field.
    Thus `1` and `1 ` have one field, and `1 2 ` has two fields. When `r` has an empty field at
    the start or between two separators, or has three or more fields, the result is
    unspecified.
  - The test reads the complete triple, never the shorter texts. The test passes when the read
    gives no error, also when text is left after the triple.
  - The filter of `command-parser.md`, CMD-SUG-008, applies to each text.
  - `rotation` makes no suggestion.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

---

## 3. Values when a command runs

### ARG-COORD-006: Position of a world coordinate triple

For each axis, an absolute value is the value. A relative value is the value plus the same axis
of the source position.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator
at `10.5 64 10.5`, send `teleport ~1 ~ 3`. The new position is `11.5 64 3.5`.

### ARG-COORD-007: Position of local coordinates

- **Inputs:** the anchor point `p` of the source, its yaw `y` and its pitch `x`, in degrees,
  and the local values `left`, `up` and `forwards`.
- **Behavior:** let `rad(d)` be the angle `d` in degrees, computed in binary32, times the
  binary32 value of π/180, in binary32. Sine and cosine take `rad` of the complete degree
  expression, widened to binary64, and use the table below. Let:
  - `cy = cos(rad(y + 90))` and `sy = sin(rad(y + 90))`;
  - `cx = cos(rad(−x))` and `sx = sin(rad(−x))`;
  - `cu = cos(rad(−x + 90))` and `su = sin(rad(−x + 90))`.

  Each of these values is binary32. Then:
  - `f = (cy · cx, sx, sy · cx)`, the forwards axis. Each product is binary32;
  - `u = (cy · cu, su, sy · cu)`, the up axis. Each product is binary32;
  - `l = −(f × u)`, the left axis, in binary64;
  - the result is `p + f · forwards + u · up + l · left`, in binary64.
- **Table:** `T[i]` is the binary32 value of `sin(i / 10430.378350470453)` for `i` from 0 to
  65535. Let `K` = 10430.378350470453, and let `t(v)` be `v` truncated to a 64-bit integer, then
  taken modulo 65536. For a radian value `a` in binary64:
  - `sin(a)` is `T[t(a·K + 0.5)]` when `a ≥ 0`, and `−T[t(−a·K + 0.5)]` otherwise;
  - `cos(a)` is `T[t(a·K + 16384.5)]` when `a ≥ 0`, and `T[t(−a·K + 16384.5)]` otherwise.
- **Precision:** the order of the binary64 operations is unspecified. A differential test
  compares each axis of the result with a tolerance of 1e-9 or 4 ulp, whichever is larger. The
  ulp is that of the larger magnitude of the anchor axis and the expected axis. Another order of
  the three additions of an axis adds at most three roundings, and 4 ulp keeps a margin. The
  table, its index rule, the binary32 angle and the binary32 products are required. Without
  the table, the result can differ by about 2e-4 at a distance of 5. Without the binary32
  steps, the result can differ in the sixth decimal, and the feedback of `teleport` shows six
  decimals.

  The functions `sin_modern` and `cos_modern` of `mcrs_minecraft_core::mth` have no `+ 0.5`
  and no sign branch. That form does not apply here.
- **Postconditions:** a source that no `execute anchored` changed has the anchor `feet`, so
  `p` is the source position (ARG-KW-002).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator at
`0.5 64 0.5` with yaw 0 and pitch 0, run `tp ^ ^ ^1`. The new position is `0.5 64 1.5`.

### ARG-COORD-008: Rotation

The value of `rotation` is a pitch and a yaw. An absolute component is the value. A relative
component is the value plus the same component of the source rotation. Each component of the
result is a binary32 angle, as a rotation is on the wire and in the save.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator
with the yaw 30 and the pitch 10, send `teleport @s ~ ~ ~ ~15 ~-5`. The new yaw is 45 and the
new pitch is 5.

### ARG-COORD-009: Relative flags

A command that moves an entity can ask whether each axis is relative. For world coordinates,
an axis is relative when it has `~`. For local coordinates, every axis is relative. For
`rotation`, the yaw and the pitch are each relative when they have `~`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator,
send `teleport @s ~ 70 ~`, and capture the `player_position` packet that follows. Its flags mark
x and z as relative and y as absolute.

---

## 4. `block_pos`

### ARG-COORD-010: One block coordinate

- **Behavior:**
  1. If next is `^`, fail with `argument.pos.mixed` at the cursor.
  2. At the end of the command line, fail with `argument.pos.missing.int` at the end.
  3. If next is `~`, skip it. The value is relative.
  4. If the value is relative: at the end of the command line, or if next is a separator, the
     value is relative 0. Otherwise read a double (`command-parser.md`, CMD-RD-004).
  5. If the value is absolute and next is a separator, fail with `reader_expected_int` at the
     cursor (ARG-COORD-DEV-002). Otherwise read an integer (`command-parser.md`, CMD-RD-002).
- **Postconditions:** an absolute value is an integer. A relative value can have a fraction. No
  axis gets the center correction.
- **Edge cases:**

| Text | Result |
|------|--------|
| `5` | absolute 5 |
| `-0` | absolute 0 |
| `5.0` | `reader_invalid_int` with `5.0`, at the start of `5.0` |
| `2147483648` | `reader_invalid_int` with `2147483648` |
| `~` | relative 0 |
| `~0.5` | relative 0.5 |
| `~x` | `reader_expected_double` after `~` |
| a separator at the cursor | `reader_expected_int` at the cursor (ARG-COORD-DEV-002) |

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `setblock 1.5 64 0 minecraft:stone`. The reply has `reader_invalid_int` with
`1.5`, and its context ends before `1.5`. Then stand at `10.7 64 0.5`, and send
`setblock ~0.5 63 0 minecraft:stone`. The stone appears at `11 63 0`.

### ARG-COORD-011: `block_pos`

- **Behavior:** note the start.
  - If next is `^`, read local coordinates (ARG-COORD-002).
  - Otherwise read three block coordinates (ARG-COORD-010). Between two coordinates: if next is
    a separator, skip it. Otherwise set the cursor back to the start, and fail with
    `argument.pos3d.incomplete`.
- **Postconditions:** `block_pos` on the wire has no properties (`commands-packet.md`,
  CMDP-ARG-001). The client reads the same grammar. Local coordinates keep their fractions.
- **Edge cases:**

| Text | Result |
|------|--------|
| `1 64 -3` | absolute 1, 64, −3 |
| `~ ~-1 ~` | relative 0, −1, 0 |
| `^ ^ ^2.5` | local 0, 0, 2.5 |
| `~ ^ ~` | `argument.pos.mixed` at the `^` |
| `^ ~ ^` | `argument.pos.mixed` at the start |
| `1 64` | `argument.pos3d.incomplete` at the start |
| `1 64 `, with a separator at the end of the command line | `argument.pos.missing.int` at the end |
| `1.5 64 0` | `reader_invalid_int` with `1.5` |
| `1  64 0` | `reader_expected_int` at the second separator (ARG-COORD-DEV-002) |

- **Examples:** `0 0 0`, `~ ~ ~`, `^ ^ ^`, `^1 ^ ^-5`, `~0.5 ~1 ~-5`.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, run these commands from a command block: `setblock 1 64`, `setblock 1 64 ` with a
separator at the end, and `setblock ~ ^ ~ stone`. The outputs have `argument.pos3d.incomplete`,
`argument.pos.missing.int` and `argument.pos.mixed`, in this order.

### ARG-COORD-012: Block position of a `block_pos`

- **Behavior:**
  1. Compute the point. For world coordinates, use ARG-COORD-006. For local coordinates, use
     ARG-COORD-007 with the anchor point of the source.
  2. The block position is the block floor of each axis of the point.
- **Edge cases:**

| Source | Text | Block position |
|--------|------|----------------|
| at `10.7 64 -0.3` | `~ ~ ~` | `10 64 -1` |
| at `10.7 64 -0.3` | `~0.5 ~-1 ~` | `11 63 -1` |
| at `0.5 64 0.5`, yaw 0, pitch 0, anchor `feet` | `^ ^ ^2` | `0 64 2` |
| any | `5 -3 7` | `5 -3 7` |
| at `0 64 0` | `~3000000000 ~ ~` | `2147483647 64 0` |

- **Postconditions:** the block position does not depend on the dimension. A command applies one
  check of ARG-COORD-013 to it.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator
at `10.7 64 -0.3`, send `setblock ~ ~-1 ~ minecraft:gold_block`. The gold block appears at
`10 63 -1`. ARG-COORD-016 has the experiment for the limit of the block floor.

### ARG-COORD-013: Checks of a block position

A command that reads a `block_pos` selects one of three checks. The checks belong to the
argument type: each command that selects a check gets the same keys in the same order. The
specification of each command states its check.

- **None:** the block position is the value. `execute if loaded` uses this check (CMD-EX-055).
- **Loaded:** the check uses a dimension `D`. `D` is the source dimension, unless the command
  names another dimension.
  1. If the chunk that contains the block position is not a loaded chunk of `D`, fail with
     `argument.pos.unloaded`.
  2. If `y` is not in the build height of `D`, fail with `argument.pos.outofworld`.
  3. If `x` or `z` is not in the range −30 000 000 to 29 999 999, fail with
     `argument.pos.outofworld`.
- **Spawnable:** the check reads no chunk and no dimension.
  1. If `y` is not in the range −20 000 000 to 19 999 999, fail with
     `argument.pos.outofbounds`.
  2. If `x` or `z` is not in the range −30 000 000 to 29 999 999, fail with
     `argument.pos.outofbounds`.
- **Postconditions:**
  - A check runs when the command runs, not when the parser reads the text. Thus the client
    never highlights these errors.
  - The three errors have no arguments and no position.
  - The loaded check tests the chunk first. Thus a block position above the build height in a
    chunk that is not loaded fails with `argument.pos.unloaded`.
- **Uses in the reference:** the loaded check: `setblock`, `fill`, `clone`, `fillbiome`, `place`,
  `loot`, the `block` targets of `data` and `item`, and `execute if block`, `if blocks`,
  `if biome`, `if data block` and `store ... block`. The spawnable check: `setworldspawn` and
  `spawnpoint`. No check: `execute if loaded`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator
in the Overworld, send these commands:

1. `setblock ~ 1000 ~ minecraft:stone`. The reply is `argument.pos.outofworld`.
2. `setblock 100000 1000 100000 minecraft:stone`, where that chunk is not loaded. The reply is
   `argument.pos.unloaded`.
3. `setworldspawn 0 20000000 0`. The reply is `argument.pos.outofbounds`.
4. `setworldspawn 0 19999999 0`. The reply has no `argument.pos.outofbounds`.

### ARG-COORD-014: Suggestions of `block_pos`

ARG-COORD-005 applies to `block_pos` as it applies to `vec3`, with one change: the test reads
`block_pos`, not `vec3`. The server offers the candidate `~ ~ ~`. The client selects its own
candidates, for example the block under its crosshair. The text of these candidates is
unspecified.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: with a modified
server, send a root literal `x` with a child `block_pos` and the suggestion type
`minecraft:ask_server`. Type `/x 1`. The server suggests `1 ~` and `1 ~ ~`. Type `/x 1.5`. The
server suggests nothing.

---

## 5. `swizzle`

### ARG-COORD-015: `swizzle`

- **Behavior:** start with an empty axis set.
  1. While the reader is not at the end and next is not a separator:
     1. Read one character `c`.
     2. If `c` is `x`, `y` or `z`, let `a` be that axis. Otherwise fail with
        `arguments.swizzle.invalid` at the cursor after `c`.
     3. If `a` is in the axis set, fail with `arguments.swizzle.invalid` at the cursor after `c`.
     4. Add `a` to the axis set.
  2. If the axis set is empty, fail with `arguments.swizzle.invalid` at the cursor
     (ARG-COORD-DEV-003).
- **Postconditions:** the value is the axis set. The order of the letters has no meaning.
  `swizzle` on the wire has no properties (`commands-packet.md`, CMDP-ARG-001).
- **Suggest:** the type makes no suggestion.
- **Examples:** `xyz`, `x`.
- **Edge cases:**

| Text | Result |
|------|--------|
| `xz` | x and z |
| `zx` | x and z |
| `yzx` | x, y and z |
| `xz run` | x and z; the cursor stops before the separator |
| `X` | `arguments.swizzle.invalid` after `X`: the match is case-sensitive |
| `xx` | `arguments.swizzle.invalid` after the second `x` |
| `xw` | `arguments.swizzle.invalid` after `w` |
| `x,z` | `arguments.swizzle.invalid` after `,` |

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `execute align xx run say a`. The reply has `arguments.swizzle.invalid`, and its
context ends after the second `x`. Then send `execute align X run say a`. The reply has the same
key, and its context ends after `X`.

### ARG-COORD-016: `align` with an axis set

`execute align` (`command-execute.md`, CMD-EX-019) changes each axis of the source position that
is in the axis set. That axis gets the block floor of its value, as a binary64 value. The other
axes do not change. The block floor limits the result to the range of a signed 32-bit integer.

- **Edge cases:**

| Source position | Axis set | New source position |
|-----------------|----------|---------------------|
| `1.7 64.2 -0.2` | x and z | `1.0 64.2 -1.0` |
| `1.7 64.2 -0.2` | y | `1.7 64.0 -0.2` |
| `3000000000.5 64 0` | x | `2147483647.0 64 0` |

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator,
run this command:

```
execute positioned 3000000000.5 64 0 align x positioned ~-2147483647 ~ ~ run summon marker ~ ~ ~ {Tags:["m"]}
```

Then run `data get entity @e[tag=m,limit=1] Pos[0]`. The value is `0.0`. Without the limit, the
value is `852516353.0`.

---

## 6. Errors

| Translation key | When |
|-----------------|------|
| `argument.pos.mixed` | `~` or a number mixes with `^` |
| `argument.pos.missing.double` | a coordinate is missing at the end |
| `argument.pos3d.incomplete` | fewer than three coordinates |
| `argument.rotation.incomplete` | fewer than two coordinates |
| `argument.pos.missing.int` | a coordinate of a `block_pos` is missing at the end |
| `argument.pos.unloaded` | the loaded check finds a chunk that is not loaded (ARG-COORD-013) |
| `argument.pos.outofworld` | the loaded check finds a position outside the world (ARG-COORD-013) |
| `argument.pos.outofbounds` | the spawnable check fails (ARG-COORD-013) |
| `arguments.swizzle.invalid` | a `swizzle` text is not a valid axis set (ARG-COORD-015) |

The key `arguments.swizzle.invalid` starts with `arguments`, not with `argument`. No key of this
table has arguments.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: as an operator, run
these commands from a command block: `summon pig ~ ^ ~`, `summon pig 1 2`, `summon pig 1 2 `
with a separator at the end, and `teleport @s ~ ~ ~ 90`. The outputs have the four keys of the
table, in this order: `argument.pos.mixed`, `argument.pos3d.incomplete`,
`argument.pos.missing.double`, `argument.rotation.incomplete`. The chat screen of the vanilla
client removes a separator at the end, so the third command needs a command block.
ARG-COORD-011, ARG-COORD-013 and ARG-COORD-015 have the experiments for the other keys.

---

## 7. Intentional deviations

### ARG-COORD-DEV-001: An absolute coordinate without a number

- **Rule:** ARG-COORD-001.
- **Reference:** an absolute coordinate that starts at a separator has an empty number text.
  Its value is 0, or 0.5 on an axis with `center` set. Thus the `vec3` text `1  2 3` reads
  x 1.5, y 0.0 and z 2.5, and ` 3` is left for the next node.
- **Visible to the client:** yes, for text that does not come from the chat screen. The chat
  screen of the vanilla client changes each run of whitespace to one separator before it sends
  the command. A command block, a function or a modified client can send the text. The client
  highlights no error, and the server replies with `reader_expected_double`.
- **Test:** a differential test skips a command line with two separators in a row inside a
  coordinate argument.

### ARG-COORD-DEV-002: An absolute block coordinate without a number

- **Rule:** ARG-COORD-010.
- **Reference:** an absolute block coordinate that starts at a separator has the value 0. Thus
  the `block_pos` text `1  2 3` reads x 1, y 0 and z 2, and ` 3` is left for the next node.
- **Visible to the client:** yes, for text that does not come from the chat screen, as for
  ARG-COORD-DEV-001. The client highlights no error, and the server replies with
  `reader_expected_int`.
- **Test:** a differential test skips a command line with two separators in a row inside a
  coordinate argument.

### ARG-COORD-DEV-003: An empty axis set

- **Rule:** ARG-COORD-015.
- **Reference:** a `swizzle` that starts at a separator or at the end of the command line reads
  no character. Its value is the empty axis set, and `execute align` with it does not change the
  source. Thus `execute align  run say a`, with two separators, runs `say a`.
- **Visible to the client:** yes, for text that does not come from the chat screen. The chat
  screen of the vanilla client changes each run of whitespace to one separator, and removes a
  separator at the end. A command block, a function or a modified client can send the text. The
  client highlights no error, and the server replies with `arguments.swizzle.invalid`.
- **Test:** a differential test skips a command line with two separators in a row before a
  `swizzle`, and a command line that ends with a separator before a `swizzle`.

---

## 8. Open questions

- **A chunk that the server keeps but has not completed.** The loaded check of ARG-COORD-013
  passes in the reference when the server keeps the chunk at the level of the full status. The
  chunk can still be in generation. The reference then completes the chunk before the command
  reads it, and the tick waits. It is not known if a player can see this case. Experiment: make a
  function with `forceload add 20000 20000` and then `setblock 20000 64 20000 minecraft:stone`,
  in a world where that chunk was never generated. Run the function, and record the reply and the
  duration of the tick.
