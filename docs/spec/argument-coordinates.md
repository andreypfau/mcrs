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
  The `execute anchored` command changes it. A player command line starts at the feet.
- **Yaw**: the rotation around the vertical axis, in degrees.
- **Pitch**: the rotation up and down, in degrees.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. None of the experiments has been run.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `ARG-COORD` | world coordinates, local coordinates, `vec3`, `rotation` |

---

## 1. Syntax

### ARG-COORD-001: One world coordinate

- **Inputs:** a flag `center` that enables the center correction for this axis.
- **Behavior:**
  1. If next is `^`, fail with `argument.pos.mixed` at the cursor.
  2. At the end of the command line, fail with `argument.pos.missing.double` at the end.
  3. If next is `~`, skip it. The value is relative.
  4. If the reader is not at the end and next is not a separator, read a double
     (`command-parser.md`, CMD-RD-004). Otherwise the number text is empty.
  5. If the value is relative and the number text is empty, the value is relative 0.
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

Provenance: `source_suggested`. Confidence: high.

### ARG-COORD-002: Local coordinates

- **Behavior:** note the start.
  1. For each of three axes, in the order left, up, forwards:
     1. At the end of the command line, fail with `argument.pos.missing.double` at the end.
     2. If next is not `^`, set the cursor back to the start, and fail with `argument.pos.mixed`.
     3. Skip `^`. If the reader is not at the end and next is not a separator, read a double.
        Otherwise the value is 0.
  2. Between two axes: if next is a separator, skip it. Otherwise set the cursor back to the
     start, and fail with `argument.pos3d.incomplete`.

Provenance: `source_suggested`. Confidence: high.

### ARG-COORD-003: `vec3`

- **Behavior:** note the start.
  - If next is `^`, read local coordinates (ARG-COORD-002).
  - Otherwise read three world coordinates: x with `center` set, y with `center` clear, and z
    with `center` set. Between two coordinates: if next is a separator, skip it. Otherwise set
    the cursor back to the start, and fail with `argument.pos3d.incomplete`.
- **Postconditions:** `vec3` on the wire has no properties. Every `vec3` applies the center
  correction (`commands-packet.md`, CMDP-ARG-001).
- **Edge cases:**

| Text | Result |
|------|--------|
| `1 2 3` | absolute 1.5, 2.0, 3.5 |
| `~ ~1 ~` | relative 0, 1, 0 |
| `^ ^ ^5` | local 0, 0, 5 |
| `~ ^ ~` | `argument.pos.mixed` at the `^` |
| `^ ~ ^` | `argument.pos.mixed` at the start |
| `1 2` | `argument.pos3d.incomplete` at the start |
| `1  2 3` | absolute 1.5, 0.0, 2.5, and ` 3` is left |

  In the last row, the y coordinate starts at the second separator. Its number text is empty,
  so y is absolute 0.0. The z coordinate is `2`, and the argument ends before ` 3`.

- **Examples:** `0 0 0`, `~ ~ ~`, `^ ^ ^`, `^1 ^ ^-5`, `0.1 -0.5 .9`, `~0.5 ~1 ~-5`.

Provenance: `source_suggested`. Confidence: high.

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

Provenance: `source_suggested`. Confidence: high.

---

## 2. Suggestions

### ARG-COORD-005: Suggestions

- **Behavior:** let `r` be the remaining input from the start of the argument. The candidate
  coordinates are `^ ^ ^` when `r` starts with `^`. Otherwise they are the coordinates that the
  source offers: `~ ~ ~` on the server. On the client, they are the whole block coordinates of
  the block under the crosshair, when there is one, else `~ ~ ~`.
  - For `vec3`, with candidate `(a, b, c)`:
    - `r` empty: if `vec3` reads `a b c` without an error, suggest `a`, `a b` and `a b c`;
    - `r` with one field `f` when split at separators: if `vec3` reads `f b c` without an
      error, suggest `f b` and `f b c`;
    - `r` with two fields `f g`: if `vec3` reads `f g c` without an error, suggest `f g c`.
  - The test reads the complete triple, never the shorter texts. The usual
    filter of `command-parser.md`, CMD-SUG-005, also applies: a text that does not contain
    `r` at its start or after a `.`, `_` or `/` is not suggested.
  - `rotation` makes no suggestion.

Provenance: `source_suggested`. Confidence: medium.

---

## 3. Values when a command runs

### ARG-COORD-006: Position of a world coordinate triple

For each axis, an absolute value is the value. A relative value is the value plus the same axis
of the source position.

Provenance: `source_suggested`. Confidence: high.

### ARG-COORD-007: Position of local coordinates

- **Inputs:** the anchor point `p` of the source, its yaw `y` and its pitch `x`, in degrees,
  and the local values `left`, `up` and `forwards`.
- **Behavior:** let `rad(d)` be the angle `d` in degrees, computed in binary32, times the
  binary32 value of π/180. Sine and cosine take `rad` of the complete degree expression, and use
  the table below. Then:
  - `f = (cos(rad(y + 90)) · cos(rad(−x)), sin(rad(−x)), sin(rad(y + 90)) · cos(rad(−x)))`, in
    binary32;
  - `u = (cos(rad(y + 90)) · cos(rad(90 − x)), sin(rad(90 − x)), sin(rad(y + 90)) · cos(rad(90 − x)))`,
    in binary32;
  - `l = −(f × u)`, the cross product, in binary64;
  - the result is `p + f · forwards + u · up + l · left`, in binary64.
- **Table:** `T[i]` is the binary32 value of `sin(i / 10430.378350470453)` for `i` from 0 to
  65535. Let `K` = 10430.378350470453, and let `t(v)` be `v` truncated to a 64-bit integer, then
  taken modulo 65536. For a radian value `a` in binary64:
  - `sin(a)` is `T[t(a·K + 0.5)]` when `a ≥ 0`, and `−T[t(−a·K + 0.5)]` otherwise;
  - `cos(a)` is `T[t(a·K + 16384.5)]` when `a ≥ 0`, and `T[t(−a·K + 16384.5)]` otherwise.

  The functions `sin_modern` and `cos_modern` of `mcrs_minecraft_core::mth` have no `+ 0.5`
  and no sign branch. That form does not apply here.
- **Postconditions:** for a player, the anchor is the feet: the player position.

Provenance: `source_suggested`. Confidence: medium. Experiment: as an operator at
`0.5 64 0.5` with yaw 0 and pitch 0, run `tp ^ ^ ^1`. The new position is `0.5 64 1.5`.

### ARG-COORD-008: Rotation

The value of `rotation` is a pitch and a yaw. An absolute component is the value. A relative
component is the value plus the same component of the source rotation.

Provenance: `source_suggested`. Confidence: high.

### ARG-COORD-009: Relative flags

A command that moves an entity can ask whether each axis is relative. For world coordinates,
an axis is relative when it has `~`. For local coordinates, every axis is relative. For
`rotation`, the yaw and the pitch are each relative when they have `~`.

Provenance: `source_suggested`. Confidence: high.

---

## 4. Errors

| Translation key | When |
|-----------------|------|
| `argument.pos.mixed` | `~` or a number mixes with `^` |
| `argument.pos.missing.double` | a coordinate is missing at the end |
| `argument.pos3d.incomplete` | fewer than three coordinates |
| `argument.rotation.incomplete` | fewer than two coordinates |

Provenance: `source_suggested`. Confidence: high.
