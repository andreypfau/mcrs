# The `minecraft:vec3` and `minecraft:rotation` arguments: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |
| Parser ids | `minecraft:vec3` 10, `minecraft:rotation` 29 (`commands-packet.md`, CMDP-ARG-001) |

Source of the version values: `assets/minecraft/version.json`. Source of the parser ids:
`assets/mcrs/reports/registries.json`, through `commands-packet.md`, CMDP-ARG-001.

This document states two argument types: a position in three axes, and a rotation in two
axes. It states the text that each accepts, the errors and their positions, the suggestions,
and how a command resolves the value against the source. It does not prescribe an
architecture.

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

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. None of the experiments has been run.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `ARG-COORD` | world coordinates, local coordinates, `vec3`, `rotation` |
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

## 4. Errors

| Translation key | When |
|-----------------|------|
| `argument.pos.mixed` | `~` or a number mixes with `^` |
| `argument.pos.missing.double` | a coordinate is missing at the end |
| `argument.pos3d.incomplete` | fewer than three coordinates |
| `argument.rotation.incomplete` | fewer than two coordinates |

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: as an operator, run
these commands from a command block: `summon pig ~ ^ ~`, `summon pig 1 2`, `summon pig 1 2 `
with a separator at the end, and `teleport @s ~ ~ ~ 90`. The outputs have the four keys of the
table, in this order: `argument.pos.mixed`, `argument.pos3d.incomplete`,
`argument.pos.missing.double`, `argument.rotation.incomplete`. The chat screen of the vanilla
client removes a separator at the end, so the third command needs a command block.

---

## 5. Intentional deviations

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
