# Numeric primitives: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of the game values: `assets/minecraft/version.json`.

The game computes many results with a small set of numeric operations: conversions from float to
integer, comparisons, interpolations, an angle wrap, a sine table, an arctangent and a colour
conversion. The bits of these results go into terrain, command output and registry data. This
document states the exact result of each operation. The document does not prescribe an
architecture, a module or a name of a function.

## 0. Conventions

### 0.1 Terms

- **f32**: the IEEE 754 binary32 format. **f64**: the IEEE 754 binary64 format. **i32** and
  **i64**: signed integers in two's complement, 32 and 64 bits.
- **Precision of a rule**: the format in which each step of the rule computes. Each step rounds
  to nearest, ties to even.
- **Step order**: a formula computes left to right, as written, with its parentheses. Each
  operator is one rounded step. A step never fuses a multiplication and an addition.
- **Widen**: the exact conversion from f32 to f64.
- **Narrow**: the conversion from f64 to f32, to nearest, ties to even.
- **`toI32(x)`**: the conversion of a float `x` to i32. A NaN gives `0`. A value at or above
  `2^31` gives `2^31 − 1`. A value below `−2^31` gives `−2^31`. Each other value truncates toward
  zero.
- **`toI64(x)`**: the same conversion to i64, with the bounds `2^63 − 1` and `−2^63`.
- **`low16(n)`**: the low 16 bits of the integer `n` in two's complement, as a value in
  `[0, 65535]`.
- **`toF32(n)`**: the conversion of an i32 to f32, to nearest, ties to even.
- **Wrapping integer step**: an i32 addition, subtraction or multiplication that keeps the low
  32 bits of the exact result.
- **Same zero**: a zero with the same sign as the input zero.
- **Numeric artefact**: the file `crates/mcrs_minecraft_core/tests/fixtures/vanilla/numeric.json`.
  The task `dumpNumeric` of `tools/vanilla-oracle` writes it. Each float in it is the hex text
  of its bits, big-endian: 8 digits for an f32, 16 digits for an f64. Each integer is decimal
  text.
- **Exact result**: the result has the same bits as the artefact. When the artefact has a NaN,
  each NaN agrees, whatever its sign and its payload.
- `S` is the f64 value `10430.378350470453`. It is the number of table entries for each radian,
  `65536 / 2π`, rounded to f64.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". Each rule of this document requires an exact result. A rule
states its users, so that a reader knows which output changes when the rule changes.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `NUM-CONV` | conversions from float to integer, and integer division |
| `NUM-CMP` | minimum, maximum, clamp and sign |
| `NUM-FN` | logarithm and power |
| `NUM-INTERP` | interpolation and range maps |
| `NUM-ANGLE` | the wrap of an angle in degrees |
| `NUM-TRIG` | the sine table and the three index forms |
| `NUM-ATAN` | the inverse square root estimate and the arctangent |
| `NUM-COLOUR` | the conversion from hue, saturation and value to RGB |
| `NUM-TEST` | parity tests |

### 0.4 Scope

In scope: the operations that mcrs code uses today and that must give the bits of the
reference. Out of scope: operations that the standard library of a language gives with the same
bits, for example an f32 square root, which is correctly rounded.

---

## 1. Conversions and integer division

### NUM-CONV-001: Floor and ceiling of an f32 to an integer

- **Inputs:** an f32 `x`.
- **Behavior:**
  1. The floor is `toI32(⌊x⌋)`. `⌊x⌋` is the largest integer value that is not more than `x`.
  2. The ceiling is `toI32(⌈x⌉)`. `⌈x⌉` is the smallest integer value that is not less than `x`.
  3. The truncation is `toI32(x)`.
- **Postconditions:** a NaN gives `0` for each of the three. An infinity gives the bound of its
  sign. A result never wraps.

Provenance: `source_suggested_black_box_confirmed` (numeric artefact, `floor`: each row is
`[x, floor, ceiling, truncation]`). Kind: semantic. Confidence: high. Users: the cell grid of
the density functions and the surface rules.

### NUM-CONV-002: Floor division and floor modulus of i32

- **Inputs:** an i32 `a` and an i32 `b`. `b` is not `0`.
- **Behavior:**
  1. The quotient `q` is the largest integer that is not more than the exact value `a / b`. The
     quotient keeps the low 32 bits. Only `a = −2^31`, `b = −1` wraps, and it gives `−2^31`.
  2. The modulus is `a − q · b`, in wrapping integer steps.
- **Postconditions:** the modulus is `0` or has the sign of `b`. The modulus has a magnitude
  less than the magnitude of `b`. A division by `0` is outside the domain, and no caller makes
  one. mcrs gives the quotient `0` and the modulus `a` for it, and does not stop. This is a
  `design` choice: the reference stops with an error, and no result is visible.

Provenance: `source_suggested_black_box_confirmed` (numeric artefact, `floor_div_mod`: each row
is `[a, b, quotient, modulus]`). Kind: semantic. Confidence: high. Users: the cell grid of the
density functions, the terrain adaptation around structures and the structure placement grid.

---

## 2. Comparisons

### NUM-CMP-001: Minimum and maximum of f32

- **Inputs:** two f32 values `a` and `b`.
- **Behavior:**
  1. When `a` or `b` is a NaN, the minimum and the maximum are a NaN.
  2. For this rule, `−0` is less than `+0`.
  3. The minimum is the lesser value. The maximum is the greater value.
- **Postconditions:** the minimum of `−0` and `+0` is `−0`, in each order. The maximum of `−0`
  and `+0` is `+0`, in each order.

Provenance: `source_suggested_black_box_confirmed` (numeric artefact, `min_max_signum`: each
row is `[a, b, minimum, maximum, sign of a]`). Kind: semantic. Confidence: high. Users: the
density functions, the splines of the density functions and their bounds.

### NUM-CMP-002: Clamp of an f32

- **Inputs:** an f32 `v`, and the f32 bounds `lo` and `hi`. Neither bound is a NaN, and `lo` is
  not more than `hi`.
- **Behavior:**
  1. Let `m` be the maximum of `v` and `lo` (NUM-CMP-001).
  2. The result is the minimum of `hi` and `m` (NUM-CMP-001).
- **Postconditions:**
  - A NaN `v` gives a NaN.
  - `v = −0` with `lo = +0` gives `+0`.
  - `v = +0` with `hi = −0` gives `−0`.
  - Bounds that are a NaN, or a `lo` more than `hi`, are outside the domain. The reference
    stops with an error for them.

Provenance: `source_suggested_black_box_confirmed` (numeric artefact, `clamp`: each row is
`[v, lo, hi, result]`). Kind: semantic. Confidence: high. Users: the density functions (the
clamp operation, the squeeze operation and the end islands) and the blend of old terrain.

### NUM-CMP-003: Sign of an f32

- **Inputs:** an f32 `x`.
- **Behavior:**
  1. A NaN gives a NaN.
  2. A zero gives the same zero.
  3. A value more than `0` gives `1`. A value less than `0` gives `−1`.

Provenance: `source_suggested_black_box_confirmed` (numeric artefact, `min_max_signum`, the
last cell). Kind: semantic. Confidence: high. Users: the density functions and their bounds.

---

## 3. Logarithm and power

### NUM-FN-001: Natural logarithm of an f32

- **Inputs:** an f32 `x`.
- **Behavior:**
  1. Widen `x` to f64.
  2. Compute the natural logarithm in f64.
  3. Narrow the result to f32.
- **Postconditions:** a NaN or a value less than `0` gives a NaN. Each zero gives `−∞`. `+∞`
  gives `+∞`. An f64 logarithm with an error of at most 1 unit in the last place is sufficient.
  The narrow step hides that error in each row of the artefact.

Provenance: `source_suggested_black_box_confirmed` (numeric artefact, `log_pow`: each row is
`[a, b, logarithm of a, a to the power b]`). Kind: semantic. Confidence: high. Users: the
density functions and their bounds.

### NUM-FN-002: Power of two f32 values

- **Inputs:** an f32 base `a` and an f32 exponent `b`.
- **Behavior:**
  1. Widen `a` and `b` to f64.
  2. Compute `a` to the power `b` in f64, with the special cases of the IEEE 754 operation
     `pow`, except these two:
     - when `b` is a NaN, the result is a NaN, also for `a = 1`;
     - when the magnitude of `a` is `1` and `b` is an infinity, the result is a NaN.
  3. Narrow the result to f32.
- **Postconditions:** `b = ±0` gives `1`, also for a NaN `a`. An f64 power with an error of at
  most 1 unit in the last place is sufficient. The narrow step hides that error in each row of
  the artefact.

Provenance: `source_suggested_black_box_confirmed` (numeric artefact, `log_pow`; the row with
`a = 1` and a NaN `b` has a NaN). Kind: semantic. Confidence: high. Users: the density
functions and their constant folding.

---

## 4. Interpolation and range maps

### NUM-INTERP-001: Linear interpolation of floats

- **Inputs:** a factor `t`, a start `p0` and an end `p1`, all in one precision: f32 or f64.
- **Behavior:**
  1. The result is `p0 + t · (p1 − p0)`, in the precision of the inputs.
- **Postconditions:** the three steps are a subtraction, a multiplication and an addition, in
  that order. A factor outside `[0, 1]` extrapolates.

Provenance: `source_suggested_black_box_confirmed` (numeric artefact, `lerp.f32` and `lerp.f64`:
each row is `[t, p0, p1, result]`). Kind: semantic. Confidence: high. Users: the terrain shapes
of the built-in presets and the ore veins.

### NUM-INTERP-002: Linear interpolation of integers

- **Inputs:** an f32 factor `t`, an i32 start `p0` and an i32 end `p1`.
- **Behavior:**
  1. Let `d` be `p1 − p0`, in a wrapping integer step.
  2. Let `m` be `t · toF32(d)`, in f32.
  3. The result is `p0 + toI32(⌊m⌋)`, in a wrapping integer step.
- **Postconditions:** the result leaves `p0` only when `t · d` reaches a whole unit.

Provenance: `source_suggested_black_box_confirmed` (numeric artefact, `lerp_int`: each row is
`[t, p0, p1, result]`). Kind: semantic. Confidence: high. Users: none at this time. An
implementation can omit this rule until a user needs it.

### NUM-INTERP-003: Clamped range map

- **Inputs:** a value `v`, a source range `a`, `b` and a target range `c`, `e`, all in one
  precision: f32 or f64.
- **Behavior:**
  1. Let `k` be `(v − a) / (b − a)`.
  2. When `k < 0`, the result is `c`.
  3. When `k > 1`, the result is `e`.
  4. In each other case, the result is the interpolation of NUM-INTERP-001 with `t = k`,
     `p0 = c` and `p1 = e`.
- **Postconditions:** a NaN `k` gives a NaN. When `a = b`, `k` is an infinity or a NaN, by the
  IEEE 754 division rules.

Provenance: `source_suggested_black_box_confirmed` (numeric artefact, `map.clamped_f32` and
`map.clamped_f64`: each row is `[v, a, b, c, e, result]`; the last two rows of `clamped_f64`
have `a = b`). Kind: semantic. Confidence: high. Users: the aquifers, the speleothem features,
the tree placement and the structure templates.

### NUM-INTERP-004: Range map

- **Inputs:** an f64 value `v`, an f64 source range `a`, `b` and an f64 target range `c`, `e`.
- **Behavior:**
  1. The result is `c + ((v − a) / (b − a)) · (e − c)`, in f64.
- **Postconditions:** a value outside the source range maps outside the target range.

Provenance: `source_suggested_black_box_confirmed` (numeric artefact, `map.unclamped_f64`).
Kind: semantic. Confidence: high. Users: the aquifers and the surface rules.

---

## 5. Angles

### NUM-ANGLE-001: Wrap of an angle in degrees

- **Inputs:** an f32 angle `x`, in degrees.
- **Behavior:**
  1. Let `w` be the remainder of `x` divided by `360`, with the quotient truncated toward
     zero. The remainder is exact and has the sign of `x`.
  2. When `w ≥ 180`, `w` becomes `w − 360`, in f32.
  3. Then, when `w < −180`, `w` becomes `w + 360`, in f32.
  4. The result is `w`.
- **Postconditions:** a finite input gives a result in `[−180, 180)`. `180` gives `−180`.
  `−180` gives `−180`. A zero gives the same zero. A value already in `[−180, 180)` keeps its
  bits. A NaN or an infinity gives a NaN.

Provenance: `source_suggested_black_box_confirmed` (numeric artefact, `wrap_degrees`: each row
is `[x, result]`). Kind: semantic. Confidence: high. Users: the rotation of an entity after a
move, the rotation arguments of commands, the rotation limits of entity selectors and the
rotation of placed entities in structures.

---

## 6. The sine table

### NUM-TRIG-001: The current sine table

- **Inputs:** none.
- **Behavior:**
  1. The table has 65536 f32 entries, with the indices `0` to `65535`.
  2. The entry `k` is the narrow of the f64 sine of the f64 value `k / S`.
  3. Then the entries `0`, `16384`, `32768` and `49152` become `0`, `1`, `0` and `−1`.
- **Postconditions:** step 3 changes only the entry `32768`. Before step 3, that entry is
  `1.2246469e-16` (bits `250d3132`). A sine with an error of at most 1 unit in the last place
  in f64 is sufficient for step 2, when the table agrees with the hash of the artefact.

Provenance: `source_suggested_black_box_confirmed` (numeric artefact, `sine_table`: the SHA-256
of the 65536 entries as little-endian f32 bits, and the bits of 19 probe entries). Kind:
semantic. Confidence: high.

### NUM-TRIG-002: The earlier sine table

- **Inputs:** none.
- **Behavior:**
  1. The earlier table is the table of NUM-TRIG-001 without step 3.
- **Postconditions:** the two tables differ only at the entry `32768`.

Provenance: `source_suggested` (the releases before `26.4-snapshot-2`). Kind: semantic.
Confidence: medium. Experiment: generate a world in such a release with a carver path that
passes an angle of π, and compare the carved blocks.

### NUM-TRIG-003: Sine and cosine, current form

- **Inputs:** an f64 angle `x`, in radians.
- **Behavior:**
  1. The table is the current table (NUM-TRIG-001). `E(k)` is its entry `k`.
  2. When `x ≥ 0`, the sine is `E(low16(toI64(x · S + 0.5)))`.
  3. In each other case, which includes a NaN `x`, the sine is
     `−E(low16(toI64((−x) · S + 0.5)))`.
  4. Let `u` be `x` when `x ≥ 0`, and `−x` in each other case.
  5. The cosine is `E(low16(toI64(u · S + 16384 + 0.5)))`.
- **Postconditions:**
  - The steps of items 2, 3 and 5 compute in f64 in the step order. The cosine adds `16384`,
    then adds `0.5`, in two steps.
  - The index is the nearest entry. The sine of `−x` is the negative of the sine of `x`.
  - A NaN gives the sine `−0` and the cosine `0`.
  - `−0` gives the sine `+0`. `−π` gives the sine `−0`.
  - `+∞` gives the sine of the entry `65535`. `−∞` gives the negative of that sine.

Provenance: `source_suggested_black_box_confirmed` (numeric artefact, `sin_cos`: each row is
`[x, sine, cosine]`, with inputs on both sides of each index boundary). Kind: semantic.
Confidence: high. Users: the local coordinates `^` of commands, and each user of the table in
this game version.

### NUM-TRIG-004: Sine and cosine, earlier double form

- **Inputs:** an f64 angle `x`, in radians.
- **Behavior:**
  1. The table is the earlier table (NUM-TRIG-002). `E(k)` is its entry `k`.
  2. The sine is `E(low16(toI64(x · S)))`.
  3. The cosine is `E(low16(toI64(x · S + 16384)))`.
- **Postconditions:** the index truncates toward zero, so a negative angle takes the entry
  below the nearest entry. The steps compute in f64 in the step order.

Provenance: `source_suggested` (the releases from the start of the double form to
`26.4-snapshot-1`). Kind: semantic. Confidence: medium. Experiment: as NUM-TRIG-002. Users: the
carvers, the ore features, the speleothem features and the trunk placers of world generation.

### NUM-TRIG-005: Sine and cosine, Beta form

- **Inputs:** an f32 angle `x`, in radians.
- **Behavior:**
  1. The table is the earlier table (NUM-TRIG-002). `E(k)` is its entry `k`.
  2. Let `p` be `x · 10430.378`, in f32. The f32 constant has the bits `4622f983`.
  3. The sine is `E(low16(toI32(p)))`.
  4. The cosine is `E(low16(toI32(p + 16384)))`. The addition is in f32.
- **Postconditions:** for an f32 angle, this form and NUM-TRIG-004 can select different
  entries, because `p` rounds to f32.

Provenance: `source_suggested` (the Beta releases). Kind: semantic. Confidence: medium.
Experiment: generate a Beta world with a known seed, and compare the caves and the ore veins of
one chunk. Users: the carvers and the ore features of the Beta world generation.

---

## 7. Arctangent

### NUM-ATAN-001: Inverse square root estimate

- **Inputs:** an f64 `x`, more than `0`.
- **Behavior:**
  1. Let `i` be the bits of `x` as an i64.
  2. Let `j` be `0x5fe6eb50c7b537aa − (i >> 1)`. The shift is arithmetic. The subtraction is a
     wrapping i64 step.
  3. Let `g` be the f64 with the bits `j`.
  4. Let `h` be `0.5 · x`.
  5. The result is `g · (1.5 − h · g · g)`, in f64 in the step order.
- **Postconditions:** the result is an estimate of `1 / √x`, with a relative error of less than
  `0.2 %`. It is not `1 / √x`. A user that needs the bits of the reference uses this estimate.

Provenance: `source_suggested_black_box_confirmed` (numeric artefact, `inverse_sqrt_estimate`:
each row is `[x, result]`). Kind: semantic. Confidence: high. Users: the terrain adaptation
around structures, and NUM-ATAN-002.

### NUM-ATAN-002: Arctangent of two f64 values

- **Inputs:** an f64 `y` and an f64 `x`.
- **Behavior:** all steps compute in f64 in the step order.
  1. Let `d` be `x · x + y · y`. When `d` is a NaN, the result is a NaN.
  2. When `y < 0`, `y` becomes `−y`, and the result changes sign at the end (item 9).
  3. When `x < 0`, `x` becomes `−x`, and item 8 applies at the end.
  4. When `y > x`, `x` and `y` change places, and item 7 applies at the end.
  5. Let `r` be the estimate of NUM-ATAN-001 for `d`. `x` becomes `x · r`. `y` becomes `y · r`.
  6. Let `v` be `2^44 + y`. Let `n` be the low 32 bits of the bits of `v`, as an i32. Let `s`
     be `v − 2^44`. For a finite `y` of this rule, `n` is `256 · y` rounded to the nearest
     integer, ties to even, and `s` is `n / 256`.
  7. Let `φ` and `c` be the entries `n` of the two arctangent tables of the artefact. Let `e`
     be `y · c − x · s`. Let `θ` be `φ + ((6 + e · e) · e) · 0.16666666666666666`. When item 4
     applied, `θ` becomes `π/2 − θ`.
  8. When item 3 applied, `θ` becomes `π − θ`.
  9. When item 2 applied, `θ` becomes `−θ`. The result is `θ`.
- **Postconditions:**
  - `π/2` and `π` are the f64 values nearest to them.
  - `−0` is not less than `0`. Each of the four signed zero pairs gives `+0`.
  - When `d` is a normal number, the result is within `1e-5` rad of the exact arctangent. It is
    not the arctangent of a standard library.
  - When `d` is `0`, `+∞` or a subnormal number, the result can be far from the arctangent.
    For example, `y = x = 1e-300` gives a value near `0`. The steps of this rule still give
    the bits of the artefact.
  - When `d` is a subnormal number, the reference can also stop with an error. No user makes
    such a call. mcrs gives a NaN when `n` is outside `[0, 256]`, and does not stop. This is a
    `design` choice.

The arctangent tables are the section `atan2_tables` of the artefact. Each row is `[n, φ, c]`,
for `n` from `0` to `256`. `φ` is the f64 arcsine of `n / 256`. `c` is the f64 cosine of `φ`.
The standard library of a language can give a different last bit for some rows, so an
implementation takes the values of the artefact. The code can contain a copy of the two tables
that a tool made from the artefact. NUM-TEST-001 then checks the copy through the rows of
`atan2`.

Provenance: `source_suggested_black_box_confirmed` (numeric artefact, `atan2_tables`, and
`atan2`: each row is `[y, x, result]`). Kind: semantic. Confidence: high. Users: the rotation
that the `facing` arguments of commands compute.

---

## 8. Colour

### NUM-COLOUR-001: Hue, saturation and value to RGB

- **Inputs:** an f32 hue `h`, an f32 saturation `s` and an f32 value `v`. The hue is more than
  `−1/6`.
- **Behavior:** all float steps compute in f32 in the step order.
  1. Let `w` be `h · 6`.
  2. Let `i` be the remainder of `toI32(w)` divided by `6`. The remainder has the sign of the
     dividend.
  3. Let `f` be `w − toF32(i)`. `f` is not always in `[0, 1)`: the hue `2.5` gives `f = 12`.
  4. Let `p` be `v · (1 − s)`. Let `q` be `v · (1 − f · s)`. Let `t` be
     `v · (1 − (1 − f) · s)`.
  5. Select the channels `(red, green, blue)` by `i`:

     | `i` | red | green | blue |
     |-----|-----|-------|------|
     | 0 | `v` | `t` | `p` |
     | 1 | `q` | `v` | `p` |
     | 2 | `p` | `v` | `t` |
     | 3 | `p` | `q` | `v` |
     | 4 | `t` | `p` | `v` |
     | 5 | `v` | `p` | `q` |

  6. Each channel `a` becomes `toI32(a · 255)`, clamped to `[0, 255]`.
  7. The result is the i32 `red · 2^16 + green · 2^8 + blue`.
- **Postconditions:** the high byte of the result is `0`. A NaN hue gives `i = 0`. A hue of
  `−1/6` or less gives a negative `i`. The reference stops with an error for it, and the input
  is outside the domain. mcrs gives `0` for it, and does not stop. This is a `design` choice.

Provenance: `source_suggested_black_box_confirmed` (numeric artefact, `hsv_to_rgb`: each row is
`[h, s, v, result]`). Kind: semantic. Confidence: high. Users: the sky colour of the biomes of
the built-in presets. The registry data that the server sends carries this colour.

---

## 9. Parity tests

### NUM-TEST-001: Results against the numeric artefact

- **Inputs:** the numeric artefact.
- **Behavior:**
  1. Build the current table and the earlier table. Compute the SHA-256 of each table as 65536
     little-endian f32 bit patterns.
  2. Compare the hash of the current table with `sine_table.sha256_of_le_bits`. Compare each
     probe entry.
  3. Compare the earlier table with the current table. They differ only at the entry `32768`.
  4. For each other section of the artefact, apply the rule that the section names to the
     inputs of each row. Compare the result with the last cells of the row.
- **Postconditions:** each result is an exact result. The test fails at the first row that does
  not agree, and names the section and the row.

Provenance: `source_suggested_black_box_confirmed` (numeric artefact). Kind: semantic.
Confidence: high.

| Section | Rule |
|---------|------|
| `floor` | NUM-CONV-001 |
| `floor_div_mod` | NUM-CONV-002 |
| `min_max_signum` | NUM-CMP-001, NUM-CMP-003 |
| `clamp` | NUM-CMP-002 |
| `log_pow` | NUM-FN-001, NUM-FN-002 |
| `lerp` | NUM-INTERP-001 |
| `lerp_int` | NUM-INTERP-002 |
| `map` | NUM-INTERP-003, NUM-INTERP-004 |
| `wrap_degrees` | NUM-ANGLE-001 |
| `sin_cos` | NUM-TRIG-003 |
| `inverse_sqrt_estimate` | NUM-ATAN-001 |
| `atan2_tables`, `atan2` | NUM-ATAN-002 |
| `hsv_to_rgb` | NUM-COLOUR-001 |

The task `dumpNumeric` cannot write rows for NUM-TRIG-004 and NUM-TRIG-005, because the game
version of the oracle uses the current form. Their experiments are in their rules.

---

## 10. Intentional deviations

None.

---

## 11. Open questions

1. NUM-TRIG-003, NUM-TRIG-004: the game version of this document uses the current form for each
   read of the sine table. This includes world generation. The world generation of mcrs uses the
   earlier double form. A parity test of carvers, ore features, speleothem features and trunk
   placers against `26.4-snapshot-7` decides which form applies.
2. NUM-TRIG-005: no artefact of a Beta release confirms the f32 constant or the earlier table
   for that release.
