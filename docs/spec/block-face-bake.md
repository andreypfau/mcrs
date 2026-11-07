# Faces of a block model: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of the game values: `assets/minecraft/version.json`.

A block model has elements. Each element is a box with up to six faces. A blockstate variant
can turn the whole model in steps of 90°. This document states how the client changes these
data into baked faces: four positions in block space, four texture coordinates, a direction, a
cull face and the data that the lighting step uses. The document does not prescribe an
architecture.

## 0. Conventions

### 0.1 Terms

- **Model file**: a JSON file under `assets/<namespace>/models/`. `block-model-resolution.md`
  (prefix `BMR`) states how the client finds the model file and the elements of a block state.
- **Element**: one entry of the `elements` array of a model file.
- **Face entry**: one value of the `faces` object of an element. Its key is the **face key**.
- **Direction**: one of `down`, `up`, `north`, `south`, `west` and `east`. The **direction
  order** is that order.
- **Normal of a direction**: `down` `(0, −1, 0)`, `up` `(0, 1, 0)`, `north` `(0, 0, −1)`,
  `south` `(0, 0, 1)`, `west` `(−1, 0, 0)` and `east` `(1, 0, 0)`.
- **Axis of a direction**: X for `west` and `east`, Y for `down` and `up`, Z for `north` and
  `south`.
- **Pixel space**: the space of the numbers in a model file. The unit is `1/16` block.
- **Block space**: the space of one block. The unit is the block. A full block fills `[0, 1]` on
  each axis. +X is east, +Y is up and +Z is south.
- **Texel coordinates**: the numbers of a `uv` array. A sprite is 16 texels wide and 16 texels
  high in these numbers, whatever its size in pixels. `u` goes to the right and `v` goes down.
- **Sprite coordinates**: texel coordinates divided by 16. `(0, 0)` is the top-left corner of
  the sprite and `(1, 1)` is the bottom-right corner.
- **Variant transform**: the fields `x`, `y`, `z` and `uvlock` of the blockstate variant that
  selects the model. BMR states how the client reads the variant.
- **Variant rotation**: the rotation `R` of BFB-VAR-001.
- **Baked face**: the output of this document for one face entry. BFB-OUT-001 lists its fields.
- **Nearest direction** of a vector `w`: the direction whose normal has the largest dot product
  with `w`. When two directions have the same dot product, the first in the direction order
  wins.
- **Content error**: the model file does not load. BMR states what the client shows instead.
- `Rx(a)`, `Ry(a)` and `Rz(a)` are right-handed rotations by the angle `a` about +X, +Y and +Z
  of block space. A product `A · B` applies `B` first.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". Each rule has a Milestone line. `1` means the first release
of this document, which replaces the present code.

The **face artefact** is the file
`crates/mcrs_minecraft_model/tests/fixtures/vanilla/block_faces.json`. The task
`dumpBlockFaces` of `tools/vanilla-oracle` writes it. The game bakes the geometry of each
distinct variant transform of each model file that holds elements, and the task writes the
baked faces. Each texture reference of the artefact maps to one whole sprite, so the `uv` values
of the artefact are sprite coordinates. A case names the model file that holds the elements,
not the child model that the blockstate names.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `BFB-IN` | the fields that this document reads, and their limits |
| `BFB-GEO` | which faces an element has, and their corners |
| `BFB-UV` | texture coordinates of a face |
| `BFB-ROT` | the rotation of an element |
| `BFB-VAR` | the variant transform |
| `BFB-DIR` | the direction and the vertex order of a baked face |
| `BFB-OUT` | the other fields of a baked face, and how the client uses them |
| `BFB-TEST` | parity tests |
| `BFB-DEV` | intentional deviations |

### 0.4 Scope

In scope: positions, texture coordinates, direction, cull face, tint index, shade face and light
emission of each baked face, and the directional shade factor of each dimension type. The rules
apply to block models and to the elements of item models. An item model uses no variant
transform.

Out of scope: how the client finds a model file, its parents and its textures (BMR); ambient
occlusion and smooth light (`block-smooth-light.md`, prefix `BSL`); the colour that a tint index
selects; when a neighbour block hides a face with a cull face.

---

## 1. Input fields

### BFB-IN-001: Fields of an element

- **Inputs:** one element.
- **Behavior:**
  1. `from` and `to` are arrays of three numbers, in pixel space. Each number must be in
     `[−16, 32]`. Another value is a content error.
  2. `faces` is an object. Each key must be a direction name. Another key is a content error. An
     element with no face entry is a content error.
  3. `rotation` is optional. BFB-IN-003 states its fields.
  4. `shade_direction_override` is optional. Its value must be a direction name. Another value
     is a content error.
  5. `light_emission` is optional, with the default `0`. Its value must be an integer in
     `[0, 15]`. Another value is a content error.
  6. The client ignores other keys. The key `shade` of older packs has no effect.
- **Postconditions:** none.

Provenance: `source_suggested_black_box_confirmed` (the face artefact has `shade_face` and
`light_emission` values for 82 model files with `shade_direction_override` and for the two
models with `light_emission`; no vanilla model file has the key `shade`). Kind: semantic.
Confidence: high. Milestone: 1.

### BFB-IN-002: Fields of a face entry

- **Inputs:** one face entry.
- **Behavior:**
  1. `texture` is a string. It is required. BMR states how the client resolves it.
  2. `uv` is optional. It must be an array of exactly four numbers `[u1, v1, u2, v2]`, in texel
     coordinates. Another value is a content error.
  3. `cullface` is optional. A value that is not a direction name means that the face has no
     cull face.
  4. `rotation` is optional, with the default `0`. Its value modulo 360 must be 0, 90, 180 or
     270. A negative value is allowed. Another value is a content error.
  5. `tintindex` is optional, with the default `−1`. The value `−1` means that the face has no
     tint.
- **Postconditions:** none.

Provenance: `source_suggested_black_box_confirmed` (the face artefact). Kind: semantic.
Confidence: high. Milestone: 1.

### BFB-IN-003: Fields of an element rotation

- **Inputs:** the `rotation` object of an element.
- **Behavior:**
  1. `origin` is an array of three numbers in pixel space. It is required.
  2. When the object has `axis` or `angle`, the rotation is an **axis rotation**. Both fields are
     then required. `axis` is `x`, `y` or `z`, in any letter case. `angle` is a number of degrees.
     The client accepts each value of `angle`. Another value of `axis` is a content error.
  3. Otherwise, the object must have at least one of `x`, `y` and `z`. The rotation is then an
     **Euler rotation**. Each of the three fields is a number of degrees, with the default `0`.
  4. An object with none of `axis`, `angle`, `x`, `y` and `z` is a content error.
  5. `rescale` is optional, with the default `false`.
- **Postconditions:** none.

Provenance: `source_suggested_black_box_confirmed` (the face artefact has cases for Euler
rotations, for example `minecraft:block/template_hanging_sign_rot_3`, and for the angles `−90`,
`−67.5`, `0.0001` and `67.5`). Kind: semantic. Confidence: high. Milestone: 1.

---

## 2. Faces and corners

### BFB-GEO-001: The faces that an element draws

- **Inputs:** an element with `from` and `to`.
- **Behavior:**
  1. An axis is **flat** when `from` and `to` have the same value on it.
  2. A face entry produces a baked face only when each flat axis is the axis of its face key.
  3. Thus an element with no flat axis draws all its face entries. An element that is flat on
     one axis draws only the two faces of that axis. An element that is flat on two or three
     axes draws no face.
- **Postconditions:** a flat element, for example a plane of a `cross` model, draws only its two
  sides.

Provenance: `source_suggested_black_box_confirmed` (the face artefact). Kind: semantic.
Confidence: high. Milestone: 1.

### BFB-GEO-002: The corners of a face

- **Inputs:** a direction `d`; a low corner `lo` and a high corner `hi` in block space.
- **Behavior:**
  1. Each direction has a **right axis** and an **up axis**. They are the directions in which
     the texture goes right and up when you look at the face from outside:

     | Direction | Right axis | Up axis |
     |-----------|------------|---------|
     | `down` | +X | +Z |
     | `up` | +X | −Z |
     | `north` | −X | +Y |
     | `south` | +X | +Y |
     | `west` | +Z | +Y |
     | `east` | −Z | +Y |

  2. The face lies on the side of the box that the normal of `d` points to. On the axis of `d`,
     each corner takes `hi` for `up`, `south` and `east`, and `lo` for the other directions.
  3. On the other two axes, a corner takes `hi` where it is at the positive end of the axis and
     `lo` where it is at the negative end.
  4. The four corners are, in this order: top-left, bottom-left, bottom-right and top-right. Top
     and bottom refer to the up axis, left and right to the right axis.
- **Postconditions:** the corners go counter-clockwise when you look at the face from outside.
  For example, `south` has the corners `(lo.x, hi.y, hi.z)`, `(lo.x, lo.y, hi.z)`,
  `(hi.x, lo.y, hi.z)` and `(hi.x, hi.y, hi.z)`.

Provenance: `source_suggested_black_box_confirmed` (the face artefact). Kind: semantic. The
order of the corners matters to BSL, which splits the face into two triangles. Confidence:
high. Milestone: 1.

### BFB-GEO-003: The corners of a face entry

- **Inputs:** an element; a face key `d`.
- **Behavior:**
  1. Divide `from` and `to` by 16. The results are `lo` and `hi` in block space.
  2. The four corners of the face entry are the corners of BFB-GEO-002 for `d`, `lo` and `hi`.
  3. BFB-ROT-001 and BFB-VAR-001 then move the corners.
- **Postconditions:** none.

Provenance: `source_suggested_black_box_confirmed` (the face artefact). Kind: semantic.
Confidence: high. Milestone: 1.

---

## 3. Texture coordinates

### BFB-UV-001: Texture coordinates of a face entry with no `uv`

- **Inputs:** an element with `from = (fx, fy, fz)` and `to = (tx, ty, tz)` in pixel space; a
  face key.
- **Behavior:** the `uv` of the face entry is the value of this table:

  | Face key | `u1` | `v1` | `u2` | `v2` |
  |----------|------|------|------|------|
  | `down` | `fx` | `16 − tz` | `tx` | `16 − fz` |
  | `up` | `fx` | `fz` | `tx` | `tz` |
  | `north` | `16 − tx` | `16 − ty` | `16 − fx` | `16 − fy` |
  | `south` | `fx` | `16 − ty` | `tx` | `16 − fy` |
  | `west` | `fz` | `16 − ty` | `tz` | `16 − fy` |
  | `east` | `16 − tz` | `16 − ty` | `16 − fz` | `16 − fy` |

- **Postconditions:** the texture of a full cube face covers the whole sprite. The texture of a
  smaller face shows the part of the sprite that the face covers on a full block, before any
  rotation.

Provenance: `source_suggested_black_box_confirmed` (the face artefact). Kind: semantic.
Confidence: high. Milestone: 1.

### BFB-UV-002: Texture coordinates of the corners

- **Inputs:** the `uv` `[u1, v1, u2, v2]` of a face entry, from the file or from BFB-UV-001; the
  `rotation` `r` of the face entry.
- **Behavior:**
  1. The four **uv corners** are, in this order: `(u1, v1)`, `(u1, v2)`, `(u2, v2)` and
     `(u2, v1)`.
  2. Let `k = (r modulo 360) / 90`.
  3. Corner `i` of BFB-GEO-003 gets the uv corner `(i + k) modulo 4`.
  4. Divide each value by 16. The results are sprite coordinates.
- **Postconditions:**
  - `u1 > u2` or `v1 > v2` is valid. It mirrors the texture. The client must not sort the
    values.
  - A face `rotation` of 90 turns the texture on the face by 90° clockwise.
  - An element rotation does not change the texture coordinates.

Provenance: `source_suggested_black_box_confirmed` (the face artefact). Kind: semantic.
Confidence: high. Milestone: 1.

---

## 4. The rotation of an element

### BFB-ROT-001: The element rotation moves the corners

- **Inputs:** the corners of BFB-GEO-003; the `rotation` object of the element, when it has one.
- **Behavior:**
  1. For an axis rotation, the matrix `M` is the right-handed rotation by `angle` degrees about
     the positive end of `axis`. An angle of `0` gives the identity.
  2. For an Euler rotation, `M = Rz(z) · Ry(y) · Rx(x)`. The rotation about X applies first.
  3. When `rescale` is `true` and `M` is not the identity, compute a scale factor for each axis
     `a` of X, Y and Z. Let `w = M · e_a`, where `e_a` is the unit vector of `a`. The factor is
     `1 / max(|w.x|, |w.y|, |w.z|)`. Set `M` to `M · S(factors)`.
  4. Let `o` be `origin` divided by 16. Each corner `p` becomes `o + M · (p − o)`.
- **Postconditions:**
  - With `rescale`, the element covers the same extent on each axis after the turn as before
    the turn. A `cross` plane from 0.8 to 15.2 pixels that turns by 45° thus still spans 0.8 to 15.2 pixels on X and on Z.
  - The texture coordinates stay with their corners.

Provenance: `source_suggested_black_box_confirmed` (the face artefact, with an error of at most
`1e-7` between the formula in 64-bit arithmetic and the game). Kind: semantic. Confidence: high.
Milestone: 1.

---

## 5. The variant transform

### BFB-VAR-001: The variant rotation moves the corners

- **Inputs:** the corners after BFB-ROT-001; the values `x`, `y` and `z` of the variant
  transform, each with the default `0`.
- **Behavior:**
  1. Each of `x`, `y` and `z` modulo 360 must be 0, 90, 180 or 270. Another value is a content
     error. Let `qx`, `qy` and `qz` be the values modulo 360 divided by 90.
  2. One step of `x` maps `(px, py, pz)` to `(px, pz, −py)`. This is `Rx(−90°)`.
  3. One step of `y` maps `(px, py, pz)` to `(−pz, py, px)`. This is `Ry(−90°)`.
  4. One step of `z` maps `(px, py, pz)` to `(py, −px, pz)`. This is `Rz(−90°)`.
  5. The variant rotation `R` applies `qx` steps of `x`, then `qy` steps of `y`, then `qz` steps
     of `z`.
  6. Each corner `p` becomes `c + R · (p − c)`, with `c = (0.5, 0.5, 0.5)`.
- **Postconditions:**
  - `y = 90` turns a face that looks north to look east. Seen from above, this turn is
    clockwise.
  - `x = 90` turns a face that looks north to look down.
  - `R` has only the entries `0`, `1` and `−1`. An implementation must apply it exactly, with no
    trigonometric function. BFB-DIR-002 compares positions for equality.

Provenance: `source_suggested_black_box_confirmed` (the face artefact has cases for each
combination of `x`, `y`, `z` and `uvlock` that a vanilla blockstate uses). Kind: semantic.
Confidence: high. Milestone: 1.

### BFB-VAR-002: UV lock

- **Inputs:** a face entry with the face key `d`; its sprite coordinates after BFB-UV-002; the
  variant rotation `R`; the value `uvlock` of the variant transform, with the default `false`.
- **Behavior:**
  1. When `uvlock` is `false` or `R` is the identity, the texture coordinates do not change.
  2. Each direction has a **face frame** `B(d)`. It is the rotation that maps +Z to the normal of
     `d`: `south` the identity, `east` `Ry(90°)`, `west` `Ry(−90°)`, `north` `Ry(180°)`, `up`
     `Rx(−90°)` and `down` `Rx(90°)`.
  3. Let `d2` be the nearest direction of `R` applied to the normal of `d`.
  4. Let `F = B(d2)⁻¹ · R · B(d)`. `F` is a rotation about Z by a multiple of 90°.
  5. For each corner with the sprite coordinates `(u, v)`, compute
     `w = F⁻¹ · (u − 0.5, v − 0.5, 0)`. The new sprite coordinates are
     `(w.x + 0.5, w.y + 0.5)`.
- **Postconditions:** the texture of the face turns about the centre of the sprite by a multiple
  of 90°. The texture of a turned stair or fence stays aligned with the world.

Provenance: `source_suggested_black_box_confirmed` (the face artefact; exact quarter turns give
an error of at most `1e-7` against the game). Kind: semantic. Confidence: high. Milestone: 1.

---

## 6. Direction and vertex order

### BFB-DIR-001: The direction of a baked face

- **Inputs:** the four corners `p0`, `p1`, `p2` and `p3` after BFB-VAR-001.
- **Behavior:**
  1. Let `n = (p1 − p0) × (p2 − p0)`.
  2. When the length of `n` is zero or not finite, the direction of the baked face is `up`.
  3. Otherwise, consider only the directions whose normal has a positive dot product with `n`.
     The direction of the baked face is the one with the largest dot product. When two
     directions have the same dot product, the first in the direction order wins.
  4. When no direction has a positive dot product, the direction of the baked face is `up`.
- **Postconditions:** the direction of a face with no element rotation is the face key, turned
  by `R`. A face with an element rotation of 45° has two directions with the same dot product.
  BFB-DEV-001 records how the reference differs in that case.

Provenance: `source_suggested_black_box_confirmed` (the face artefact, except the ties of
BFB-DEV-001). Kind: semantic. Confidence: high. Milestone: 1.

### BFB-DIR-002: The vertex order after a variant rotation

- **Inputs:** the four corners after BFB-VAR-001, each with its texture coordinates; the
  direction `d` of BFB-DIR-001; the element.
- **Behavior:**
  1. When the element has a `rotation`, the order of the corners does not change.
  2. When the length of `n` of BFB-DIR-001 is zero or not finite, the order does not change.
  3. Otherwise, let `lo` and `hi` be the minimum and the maximum of the four corners on each
     axis.
  4. Put the corners in the order of BFB-GEO-002 for `d`, `lo` and `hi`. Each corner keeps its
     texture coordinates.
  5. When a corner of step 4 is not one of the four corners, the face is a content error of the
     model. The model then has a content error (`block-model-resolution.md`, BMR-MODEL-002), and
     each state that uses the model draws the missing model in place of that model. The client
     logs one error that names the model.
- **Postconditions:** for a face with no element rotation, corner 0 is the top-left corner of
  BFB-GEO-002 for the final direction. The texture keeps its place on the face.

Provenance: `source_suggested_black_box_confirmed` (the face artefact). Kind: semantic.
Confidence: high. Milestone: 1.

---

## 7. The baked face

### BFB-OUT-001: Fields of a baked face

- **Inputs:** one face entry that BFB-GEO-001 draws.
- **Behavior:** the baked face has these fields:
  1. `positions`: the four corners after BFB-DIR-002, in block space.
  2. `uv`: the four sprite coordinates after BFB-VAR-002, in the order of `positions`.
  3. `direction`: the direction of BFB-DIR-001.
  4. `cull`: the cull face of BFB-OUT-002, or none.
  5. `texture`: the `texture` string of the face entry. BMR resolves it to a sprite.
  6. `tint`: the `tintindex` of the face entry, or `−1`.
  7. `shade_face`: the `shade_direction_override` of the element, or none.
  8. `light_emission`: the `light_emission` of the element.
- **Postconditions:** the names of the fields are the keys of the face artefact. An
  implementation can use other names.

Provenance: `source_suggested_black_box_confirmed` (the face artefact). Kind: semantic.
Confidence: high. Milestone: 1.

### BFB-OUT-002: The cull face

- **Inputs:** the `cullface` of a face entry; the variant rotation `R`.
- **Behavior:**
  1. When the face entry has no cull face, the baked face has none.
  2. Otherwise, the cull face of the baked face is the nearest direction of `R` applied to the
     normal of `cullface`.
  3. The element rotation and `uvlock` do not change the cull face.
- **Postconditions:** a turned model hides the faces toward the turned neighbour. The mesh step
  states when a neighbour hides a face.

Provenance: `source_suggested_black_box_confirmed` (the face artefact). Kind: semantic.
Confidence: high. Milestone: 1.

### BFB-OUT-003: The directional shade factor

- **Inputs:** a baked face; the `cardinal_light` value of the dimension type
  (`registry-fields.md`), with the default `"default"`.
- **Behavior:**
  1. The **shade direction** of the face is its `shade_face` when it has one. Otherwise, it is
     its `direction`.
  2. The shade factor is the value of this table for the shade direction:

     | `cardinal_light` | `down` | `up` | `north`, `south` | `west`, `east` |
     |------------------|--------|------|------------------|----------------|
     | `"default"` | `0.5` | `1.0` | `0.8` | `0.6` |
     | `"nether"` | `0.9` | `0.9` | `0.8` | `0.6` |

  3. The client multiplies the colour of each vertex of the face by the shade factor. BSL states
     the other factors of the vertex colour.
  4. The factor depends on the dimension. A bake that does not know the dimension keeps the
     shade direction, and the draw step selects the factor.
  5. The faces of a fluid also use this table, with the `cardinal_light` of the dimension:
     - the top surface uses the factor of `up`, also when it slopes;
     - the bottom surface uses the factor of `down`;
     - a side face uses the factor of `up` multiplied by the factor of its own direction.
       In the overworld this gives `0.8` and `0.6`; in the nether it gives `0.72` and `0.54`.
- **Postconditions:** a model with `"shade_direction_override": "up"`, for example `cross`, has
  the factor `1.0` on each face in the overworld and `0.9` in the nether.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: 1. Experiment:
BFB-TEST-002.

### BFB-OUT-004: Light emission of a face

- **Inputs:** a baked face with `light_emission` `e`; the block light and the sky light of each
  vertex, from BSL.
- **Behavior:**
  1. When `e` is `0`, the light of the vertices does not change.
  2. Otherwise, the block light of each vertex becomes the larger of `e` and the block light,
     in whole light levels. The sky light becomes the larger of `e` and the sky light, in whole
     light levels.
  3. Step 2 drops the fraction of a smooth light level.
- **Postconditions:** a face with `light_emission` `15` shows at full brightness in a dark
  place, for example the emissive layer of `cross_emissive`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: 1. Experiment:
BFB-TEST-002, item 4.

### BFB-OUT-005: The order of baked faces

- **Inputs:** the baked faces of one model and one variant transform.
- **Behavior:**
  1. Among the baked faces with the same cull face, the order is the order of the elements in
     the file. Inside one element, the order is the direction order of the face keys. The order
     of the keys in the file has no effect.
  2. When two baked faces are in the same plane and cover the same area, the later face shows
     over the earlier face.
  3. The order between faces with different cull faces is not specified.
- **Postconditions:** the tinted side overlay of `grass_block` shows over its side texture.

Provenance: `source_suggested_black_box_confirmed` (the face artefact lists the faces with no
cull face first, then the faces of `north`, `south`, `east`, `west`, `up` and `down`, each group
in the order of item 1). Kind: semantic for items 1 and 2; unspecified for item 3. Confidence:
medium. Milestone: 1.

---

## 8. Parity tests

### BFB-TEST-001: Baked faces against the face artefact

- **Inputs:** the face artefact; the vanilla model files of the client jar.
- **Behavior:**
  1. For each case, read the elements of the model file `model` of the case.
  2. Bake them with the variant transform `x`, `y`, `z` and `uvlock` of the case.
  3. Put the baked faces in groups by cull face. Compare each group with the faces of the case
     that have the same `cull`, in the order of BFB-OUT-005, item 1.
  4. `texture`, `cull`, `tint`, `shade_face` and `light_emission` must be equal.
  5. Each position and each sprite coordinate must agree within `1e-5`.
  6. `direction` must be equal. When the two directions have dot products with `n` of BFB-DIR-001
     that agree within `1e-5`, the test accepts both (BFB-DEV-001).
- **Postconditions:** the test fails when one case does not agree.

Provenance: `source_suggested_black_box_confirmed` (the face artefact). An independent
implementation of this document agreed with all 1011 cases. The largest error was `1e-7`. 42
model files had a tie of BFB-DEV-001. Kind: semantic. Confidence: high. Milestone: 1.

### BFB-TEST-002: Screenshots of shading and emission

- **Inputs:** a reference client and the client, at the same camera position and field of view.
- **Behavior:**
  1. In the overworld at noon, put a stone block, a poppy and an oak hanging sign on a flat
     floor. Take a screenshot.
  2. Do step 1 again in the nether.
  3. Compare the two planes of the poppy. Both planes have the factor `1.0` in the overworld and
     `0.9` in the nether. The north side of the stone has the factor `0.8` in both dimensions.
  4. In a dark room, put a block whose model uses `cross_emissive`. The emissive layer shows at
     full brightness.
  5. Compare the two sets of images.
- **Postconditions:** the colour of each face agrees within 3 of 255 in each channel. The
  outline of each shape agrees within one pixel at 1920 × 1080.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Milestone: 1.

---

## 9. Intentional deviations

### BFB-DEV-001: Ties of the face direction

- **Rule:** BFB-DIR-001, item 3.
- **Reference:** the reference computes in 32-bit floating point. When a face is at 45° between
  two directions, a rounding error selects one of the two.
- **Visible to the client:** sometimes. The shade factor of such a face can differ, for example
  `0.8` against `0.6`, when the element has no `shade_direction_override`. Examples are the
  sides of `lever`, `campfire`, the candle models, the stem models and the hanging sign
  models. BSL can also sample the light of a different neighbour.
- **Test:** BFB-TEST-001 accepts both directions of a tie.

### BFB-DEV-002: Exact quarter turns

- **Rule:** BFB-VAR-001 and BFB-VAR-002.
- **Reference:** the reference builds the face frames of BFB-VAR-002 with trigonometric
  functions in 32-bit floating point.
- **Visible to the client:** no. The difference is at most `1e-7` in sprite coordinates.
- **Test:** BFB-TEST-001 uses a tolerance of `1e-5`.

---

## 10. Open questions

1. Sprite coordinates outside `[0, 1]`: the reference then samples the neighbour sprites of its
   atlas. No vanilla model has such a face. The client can clamp the coordinates or repeat the
   sprite.
2. BFB-OUT-005, item 3: the reference draws the faces with no cull face first.
   `minecraft:block/template_azalea` has two faces in the same plane with different cull faces.
   Check whether the order is visible on an azalea.
