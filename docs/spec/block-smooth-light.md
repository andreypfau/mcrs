# Smooth light of block faces: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of the game values: `assets/minecraft/version.json`.

The client draws each face of a block in the chunk mesh with a shade and a light at each vertex.
The shade makes corners and edges darker where blocks touch. The light comes from the block
light and the sky light of the cells around the face. This document states how the client
computes these values for each vertex of a face. The document does not prescribe an
architecture.

## 0. Conventions

### 0.1 Terms

- **Block space**: the space of one block. The unit is the block. The block fills `[0, 1]` on
  each axis. +X is east, +Y is up and +Z is south.
- **Cell**: one block position of the world. A cell is found by its offset from the cell of the
  drawn block. The **own cell** is the cell of the drawn block, offset `(0, 0, 0)`.
- **Own state**: the block state in the own cell.
- **Direction**: one of `down` `(0, −1, 0)`, `up` `(0, 1, 0)`, `north` `(0, 0, −1)`, `south`
  `(0, 0, 1)`, `west` `(−1, 0, 0)` and `east` `(1, 0, 0)`. The vector of a direction is its
  offset in cells. `d + e` is the sum of the offsets.
- **Quad**: a face of the block model of the own state, after baking. A quad has four vertex
  positions in block space, in an order, and a direction `n`.
- **Cull face**: the direction that the block model gives to a quad, when a neighbour can hide
  the quad. A quad can have no cull face.
- **Shade override**: the direction in the field `shade_direction_override` of the element of a
  quad in the block model. The face shade then uses this direction in place of `n`.
- **Element emission**: the value of the field `light_emission` of the element of a quad in the
  block model, `0` to `15`. The default is `0`.
- **Normal axis**: the axis of `n`. The two other axes are the **in-plane axes**.
- **Bounding box**: the minimum `min` and the maximum `max` of the four vertex positions of a
  quad, on each axis.
- **Light level**: an integer from `0` to `15`. Each cell has a block light level and a sky
  light level.
- **Light value**: an integer from `0` to `240`, in sixteenths of a light level. A light value
  of a vertex is one value for block light and one value for sky light.
- **Occlusion shade**: the number `0.2` or `1.0` that a block state has. BSL-IN-002 states it.
- **Solid render**: the block state has an occlusion shape that fills the block. The block
  definition gives it.
- **Light dampening**: the number of light levels that a block state removes from light that
  goes through it, `0` to `15`. The block definition gives it.
- **Full collision**: the collision shape of the block state fills the block.
- **Emissive state**: a block state that the client always draws at full brightness. The block
  definition gives it as `mcrs:emissive_rendering`.
- **State emission**: the light emission of a block state, `0` to `15`. The block definition
  gives it as `minecraft:light_emission`.
- **Open cell**: a cell whose state is not solid render, or has a light dampening of `0`.
- **Smooth light**: the option of the client that turns on the smooth path of section 4. The
  default is on.
- **Golden artefact**: the file
  `crates/mcrs_minecraft_mesh/tests/fixtures/vanilla/block_smooth_light.json`. The task
  `dumpBlockSmoothLight` of `tools/vanilla-oracle` writes it.
- **Shade artefact**: the file
  `crates/mcrs_minecraft_mesh/tests/fixtures/vanilla/block_occlusion_shade.json`. The task
  `dumpBlockOcclusionShade` of `tools/vanilla-oracle` writes it.
- **f32**: an IEEE 754 single-precision number. Each operation rounds to the nearest f32.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". Each rule has a Milestone line. `1` means the first release
of these rules, which replaces the present code.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `BSL-IN` | data of a cell |
| `BSL-FACE` | geometry of a quad |
| `BSL-MODE` | the choice between the smooth path and the flat path |
| `BSL-SMOOTH` | the smooth path |
| `BSL-FLAT` | the flat path |
| `BSL-OUT` | the values of a vertex and their interpolation |
| `BSL-TEST` | parity tests |

### 0.4 Scope

In scope: the quads of block models in the chunk mesh, and full cube faces that the mesh draws
in a different form. Both forms must give the same values.

Out of scope: fluids, entities, block entity shapes (`block-entity-models.md`, BEM-BAKE-003 and
BEM-BAKE-004), the light engine that computes the light levels, the visibility of a face, the
tint, and the light map that turns a light value into a colour.

---

## 1. Data of a cell

### BSL-IN-001: Data that the client reads for each cell

- **Inputs:** a cell.
- **Behavior:** the client reads these values for the cell:
  1. the block state, with its occlusion shade, solid render, light dampening, full collision,
     emissive state and state emission;
  2. the block light level and the sky light level of the cell.
- **Postconditions:** the paths of sections 4 and 5 read only these values. They read no cell
  with an offset outside `[−2, 2]` on any axis.

Provenance: `source_suggested_black_box_confirmed` (golden artefact). Kind: client agreement.
Confidence: high. Milestone: 1.

### BSL-IN-002: Occlusion shade of a block state

- **Inputs:** a block state.
- **Behavior:**
  1. When the shade artefact lists the state in `exceptions`, the occlusion shade is the value
     `occlusion_shade` of that entry.
  2. Else, the occlusion shade is `0.2` when the state has full collision, and `1.0` when it
     does not.
- **Postconditions:** glass, stained glass, tinted glass, barriers and copper grates have the
  occlusion shade `1.0`. Mud, soul sand and snow with `layers=8` have the occlusion shade
  `0.2`.

The client must have the occlusion shade of each state at run time. The shade artefact is the
evidence. The block definition of a state carries the value in the component
`mcrs:occlusion_shade`, only when the value differs from item 2. The task `dumpDefinitions` of
`tools/vanilla-oracle` writes this component.

The occlusion shade has only the two values `0.2` and `1.0`. Another value of
`mcrs:occlusion_shade` is a content error of the block definition. This is a `design` choice:
the reference has no other value, and a flag is smaller than a number in the state data.

Provenance: `source_suggested_black_box_confirmed` (shade artefact). Kind: client agreement.
Confidence: high. Milestone: 1.

### BSL-IN-003: Light of a cell under a state

- **Inputs:** a cell `c`, and a block state `s`. The state `s` is not always the state of `c`.
  Each rule names `s`.
- **Behavior:**
  1. When `s` is an emissive state, the block light level is `15` and the sky light level is
     `15`.
  2. Else, the block light level is the maximum of the block light level of `c` and the state
     emission of `s`. The sky light level is the sky light level of `c`.
- **Postconditions:** the result is the **light of `c` under `s`**, two light levels.

Provenance: `source_suggested_black_box_confirmed` (golden artefact). Kind: client agreement.
Confidence: high. Milestone: 1.

---

## 2. Geometry of a quad

### BSL-FACE-001: Vertex corners

- **Inputs:** a quad with the direction `n`.
- **Behavior:** each vertex index of a quad has a corner. A corner is the side, minimum or
  maximum, on each in-plane axis:

  | `n` | vertex 0 | vertex 1 | vertex 2 | vertex 3 |
  |-----|----------|----------|----------|----------|
  | `down` | min x, max z | min x, min z | max x, min z | max x, max z |
  | `up` | min x, min z | min x, max z | max x, max z | max x, min z |
  | `north` | max x, max y | max x, min y | min x, min y | min x, max y |
  | `south` | min x, max y | min x, min y | max x, min y | max x, max y |
  | `west` | max y, min z | min y, min z | min y, max z | max y, max z |
  | `east` | max y, max z | min y, max z | min y, min z | max y, min z |

- **Postconditions:** a quad of a box face has its vertices at these corners of the box, in
  this order. The order is counter-clockwise, seen from the side that `n` points to. A rotated
  quad keeps the corners of its vertex indices. Sections 4 and 5 use the corner of the vertex
  index, not the position of the vertex.

Provenance: `source_suggested_black_box_confirmed` (golden artefact, field `positions`). Kind:
client agreement. Confidence: high. Milestone: 1.

### BSL-FACE-002: A quad on the boundary and a partial quad

- **Inputs:** the bounding box of a quad, its direction `n`, and the own state.
- **Behavior:**
  1. The quad is **planar** when `min` and `max` are equal on the normal axis.
  2. The quad is **flush** when one of these is true:
     - `n` points to the positive side of its axis, and `max` on that axis is more than
       `0.9999`;
     - `n` points to the negative side of its axis, and `min` on that axis is less than
       `0.0001`.
  3. The quad is **on the boundary** when it is planar, and it is flush or the own state has
     full collision.
  4. The quad is **partial** when, on one or more in-plane axes, `min` is `0.0001` or more, or
     `max` is `0.9999` or less.
- **Postconditions:** a quad on the boundary takes its light from the cell in front of it. A
  planar quad inside a block with full collision is on the boundary. The comparisons use f32.

Provenance: `source_suggested_black_box_confirmed` (golden artefact, cases `inset_in_full_block`,
`post`, `thin_layer`, `diagonal`). Kind: client agreement. Confidence: high. Milestone: 1.

### BSL-FACE-003: Edge order

- **Inputs:** the direction `n` of a quad.
- **Behavior:** each direction `n` has four **edge directions** `E1`, `E2`, `E3` and `E4`. `E1`
  and `E2` are opposite. `E3` and `E4` are opposite.

  | `n` | `E1` | `E2` | `E3` | `E4` |
  |-----|------|------|------|------|
  | `down` | `west` | `east` | `north` | `south` |
  | `up` | `east` | `west` | `north` | `south` |
  | `north` | `up` | `down` | `east` | `west` |
  | `south` | `west` | `east` | `down` | `up` |
  | `west` | `up` | `down` | `north` | `south` |
  | `east` | `down` | `up` | `north` | `south` |

- **Postconditions:** a **corner pair** is a pair `(Ea, Eb)`, with `Ea` one of `E1` or `E2`, and
  `Eb` one of `E3` or `E4`. Each corner pair points to one corner of the face. The **corner
  order** is `(E1, E4)`, `(E1, E3)`, `(E2, E3)`, `(E2, E4)`.

The order changes the result of f32 sums in the last bit, and BSL-SMOOTH-003 selects `E1`.

Provenance: `source_suggested_black_box_confirmed` (golden artefact). Kind: client agreement.
Confidence: high. Milestone: 1.

---

## 3. Choice of the path

### BSL-MODE-001: The smooth path and the flat path

- **Inputs:** the option smooth light, the own state, and the block model of the own state.
- **Behavior:** the client uses the smooth path of section 4 for each quad of the own state
  when all of these are true:
  1. the option smooth light is on;
  2. the state emission of the own state is `0`;
  3. the block model has ambient occlusion. For a block state with more than one model part,
     the chosen variant (`block-model-resolution.md`, BMR-PICK-005) of the first selected part
     gives this flag.

  Else, the client uses the flat path of section 5 for each quad of the own state.
- **Postconditions:** the choice is the same for all quads of one block state.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Milestone: 1. Experiment: put
a glowstone block next to a stone block on a floor of stone, with smooth light on. Take a
screenshot. The faces of the glowstone show no dark corners. The faces of the stone show dark
corners where they touch the floor.

---

## 4. The smooth path

All numbers in this section are f32, except light levels and light values, which are integers.

### BSL-SMOOTH-001: Base cell

- **Inputs:** a quad with the direction `n`.
- **Behavior:** the base cell `B` is the cell `n` when the quad is on the boundary
  (BSL-FACE-002). Else, `B` is the own cell.
- **Postconditions:** none.

Provenance: `source_suggested_black_box_confirmed` (golden artefact). Kind: client agreement.
Confidence: high. Milestone: 1.

### BSL-SMOOTH-002: Edge samples

- **Inputs:** the base cell `B`, the edge directions of the quad (BSL-FACE-003).
- **Behavior:** for each edge direction `E`:
  1. The **edge cell** of `E` is `B + E`.
  2. The **edge shade** of `E` is the occlusion shade of the state of the edge cell.
  3. The **edge light** of `E` is the light of the edge cell under its own state (BSL-IN-003).
  4. The edge `E` is **open** when the cell `B + E + n` is an open cell.
- **Postconditions:** when the quad is on the boundary, the open test reads the cell
  `2n + E`.

Provenance: `source_suggested_black_box_confirmed` (golden artefact). Kind: client agreement.
Confidence: high. Milestone: 1.

### BSL-SMOOTH-003: Diagonal samples

- **Inputs:** a corner pair `(Ea, Eb)`, and the edge samples.
- **Behavior:**
  1. When `Ea` is not open and `Eb` is not open, the **diagonal shade** is the edge shade of
     `E1`. The **diagonal light** is the edge light of `E1`.
  2. Else, the **diagonal cell** is `B + Ea + Eb`. The diagonal shade is the occlusion shade of
     its state. The diagonal light is the light of the diagonal cell under its own state.
- **Postconditions:** item 1 uses `E1` for all four corner pairs, also for a pair that does not
  contain `E1`. This asymmetry is visible at an inside corner, and the client keeps it.

Provenance: `source_suggested_black_box_confirmed` (golden artefact). Kind: client agreement.
Confidence: high. Milestone: 1. Experiment: put a stone block on a floor of stone. Put a second
stone block two cells west and one cell north of it, on the floor. Take a screenshot of the west
face of the first block, with a sky light level of `15` everywhere. The lower north corner of
that face is lighter than its lower south corner. Without item 1, the two corners have the same
shade.

### BSL-SMOOTH-004: Centre samples

- **Inputs:** the quad, the base cell `B`, the own state.
- **Behavior:**
  1. The **centre shade** is the occlusion shade of the state of `B`.
  2. The **centre light** is the light of the cell `n` under the state of the cell `n`, when one
     of these is true:
     - the quad is on the boundary;
     - the state of the cell `n` is not solid render.
  3. Else, the centre light is the light of the own cell under the own state.
- **Postconditions:** none.

Provenance: `source_suggested_black_box_confirmed` (golden artefact). Kind: client agreement.
Confidence: high. Milestone: 1.

### BSL-SMOOTH-005: Corner values

- **Inputs:** a corner pair `(Ea, Eb)`, the edge samples, the diagonal samples and the centre
  samples.
- **Behavior:**
  1. The **corner shade** is `(((shade(Eb) + shade(Ea)) + diagonal shade) + centre shade) ×
     0.25`. `shade(E)` is the edge shade of `E`. The sums use this order.
  2. The **corner light** has a block light value and a sky light value. The client computes
     them from three neighbour lights, the edge light of `Eb`, the edge light of `Ea` and the
     diagonal light, and from the centre light:
     1. When the centre light has a block light level more than `2`, or a sky light level more
        than `2`, the client changes each neighbour light:
        - a neighbour light with block light level `0` and sky light level `0` becomes equal to
          the centre light;
        - else, a neighbour light with sky light level `0` takes the sky light level of the
          centre light.
     2. Each light value is `4 ×` the sum of the four light levels of its kind: three neighbour
        lights and the centre light.
- **Postconditions:** a corner light value is a multiple of `4`, from `0` to `240`. A dark solid
  neighbour does not make a bright face dark, because item 2.1 replaces its light.

Provenance: `source_suggested_black_box_confirmed` (golden artefact). Kind: client agreement.
Confidence: high. Milestone: 1.

### BSL-SMOOTH-006: Vertex values of a quad that is not partial

- **Inputs:** a quad that is not partial (BSL-FACE-002), the corner values.
- **Behavior:** each vertex takes the corner shade and the corner light of the corner pair that
  points to the corner of the vertex (BSL-FACE-001). The corner pair `(Ea, Eb)` points to the
  side of the face in the direction `Ea` on one in-plane axis, and in the direction `Eb` on the
  other in-plane axis.
- **Postconditions:** the vertex **occlusion** is the corner shade. The client does not clamp
  it.

Provenance: `source_suggested_black_box_confirmed` (golden artefact). Kind: client agreement.
Confidence: high. Milestone: 1.

### BSL-SMOOTH-007: Vertex values of a partial quad

- **Inputs:** a partial quad, its bounding box, the corner values.
- **Behavior:** for each vertex:
  1. The **point** of the vertex has, on each in-plane axis, the value of `min` or `max` of the
     bounding box. The corner of the vertex (BSL-FACE-001) selects `min` or `max`.
  2. For each in-plane axis with the point value `p`, a corner pair has the **factor** `p` when
     its direction on that axis points to the positive side. Else, it has the factor `1 − p`.
  3. The **weight** of a corner pair is the product of its two factors.
  4. The client sums `corner shade × weight` over the four corner pairs, in the corner order of
     BSL-FACE-003. The vertex occlusion is this sum, clamped to `[0, 1]`.
  5. For each kind of light, the client sums `corner light value × weight` over the four corner
     pairs, in the corner order. The vertex light value is this sum, truncated to an integer.
- **Postconditions:** the vertex values are a bilinear interpolation of the corner values, at
  the point of the vertex. A rotated quad uses its bounding box, not the positions of its
  vertices.

Provenance: `source_suggested_black_box_confirmed` (golden artefact, cases `bottom_slab`, `post`,
`thin_layer`, `inset_in_full_block` and `diagonal`). Kind: client agreement. Confidence: high.
Milestone: 1.

---

## 5. The flat path

### BSL-FLAT-001: Light and occlusion of a flat quad

- **Inputs:** a quad, its cull face, the own state.
- **Behavior:**
  1. When the quad has a cull face `f`, the light of the quad is the light of the cell `f` under
     the own state (BSL-IN-003).
  2. When the quad has no cull face, and the quad is on the boundary (BSL-FACE-002), the light
     is the light of the cell `n` under the own state.
  3. Else, the light is the light of the own cell under the own state.
  4. All four vertices have the occlusion `1.0`. Each vertex light value is `16 ×` the light
     level of its kind.
- **Postconditions:** an emissive own state has the light values `240` and `240`. A block that
  emits light raises its block light value to its own emission.

Provenance: `source_suggested_black_box_confirmed` (golden artefact, fields
`flat_light_with_cull_face` and `flat_light_without_cull_face`). Kind: client agreement.
Confidence: high. Milestone: 1.

---

## 6. Values of a vertex

### BSL-OUT-001: Shade of a vertex

- **Inputs:** the vertex occlusion, the direction `n` of the quad, its shade override, and the
  cardinal light of the dimension type.
- **Behavior:**
  1. The **face direction** is the shade override when the element has one. Else, it is `n`.
  2. The **face shade** is the value of the face direction in the cardinal light set of the
     dimension type:

     | Set | `down` | `up` | `north` | `south` | `west` | `east` |
     |-----|--------|------|---------|---------|--------|--------|
     | `default` | 0.5 | 1.0 | 0.8 | 0.8 | 0.6 | 0.6 |
     | `nether` | 0.9 | 0.9 | 0.8 | 0.8 | 0.6 | 0.6 |

     The field `cardinal_light` of the dimension type selects the set. The default is
     `default`.
  3. The **occlusion byte** is `floor(occlusion × 255)`, in f32.
  4. The **shade byte** of the vertex is the occlusion byte `×` the face shade in f32, truncated
     to an integer and clamped to `[0, 255]`.
- **Postconditions:** the shade byte multiplies the colour of the texture and the tint. A face
  of a full cube on the smooth path has one of the occlusion bytes `51`, `102`, `153`, `204`
  and `255`.

Provenance: `source_suggested_black_box_confirmed` (golden artefact, fields `smooth_shade` and
`flat_shade`, overworld set only). Kind: client agreement. Confidence: high for the `default`
set, medium for the `nether` set. Milestone: 1. Experiment for the `nether` set: put a stone
block on the floor of the nether. Take a screenshot. The top face and the bottom face of a
floating stone block have the same brightness.

### BSL-OUT-002: Element emission

- **Inputs:** the vertex light values, the element emission `e` of the quad.
- **Behavior:** when `e` is more than `0`, for each kind of light:
  1. the light level of the vertex is `floor(light value / 16)`;
  2. the new light value is `16 × max(light level, e)`.
- **Postconditions:** the client applies this rule after the smooth path or the flat path. The
  rule raises the sky light value as well as the block light value. It removes the fraction of
  a smooth light value.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: 1. Experiment:
put an open eyeblossom in a dark cave, at block light level `0` and sky light level `0`. Take a
screenshot. The petals that the block model marks with `light_emission` show bright, and the
stem shows dark.

### BSL-OUT-003: Triangles of a quad

- **Inputs:** a quad with four vertices `v0`, `v1`, `v2` and `v3`, and their values.
- **Behavior:**
  1. The client draws the quad as two triangles, `(v0, v1, v2)` and `(v2, v3, v0)`. The common
     edge goes from `v0` to `v2`.
  2. In each triangle, the shade and each light value change linearly between the three
     vertices. The client does not interpolate across the full quad.
- **Postconditions:** the diagonal of a face does not change with the occlusion of its corners.
  A mesh that merges full cube faces into one larger face must give each block face the values
  of these two triangles.

Provenance: `source_suggested`. Kind: client agreement. Confidence: high. Milestone: 1.
Experiment: put one stone block on a floor of stone. Put a second stone block diagonal to it on
the floor, so one corner of a floor face is dark. Take a screenshot from above. The dark area of
the floor face is a triangle on one side of the diagonal from its vertex 0 to its vertex 2.

---

## 7. Parity tests

### BSL-TEST-001: Vertex values against the golden artefact

- **Inputs:** each case of the golden artefact.
- **Steps:**
  1. Make a world from the case. The own cell has the state `own` with the light `own_light`.
     `own` can name only some of the properties of its block. Each property that `own` does
     not name has the value of the default state of the block. The entry of `palette` with
     that full state gives the flags of the own cell.
     Each entry of `cells` is `[x, y, z, palette index, block light level, sky light level]`.
     Each other cell is air with the light `default_light`. The entries of `palette` give the
     state and its flags.
  2. Make a quad with the direction `direction` and the vertex positions `positions`. The quad
     has no shade override and no element emission.
  3. Compute the smooth path. Compare the shade bytes with `smooth_shade` and the light values
     with `smooth_light`, with the `default` cardinal light set.
  4. Compute the flat path with the cull face `direction`. Compare with
     `flat_light_with_cull_face`.
  5. Compute the flat path with no cull face. Compare with `flat_light_without_cull_face` and
     `flat_shade`.
- **Expected:** all values are equal. A light value is `[block, sky]`.

Provenance: `black_box_confirmed` (golden artefact). Kind: client agreement. Confidence: high.
Milestone: 1.

### BSL-TEST-002: Occlusion shade of each state

- **Inputs:** the shade artefact and the block definitions of `assets/mcrs/block_definition/`.
- **Steps:** for each block state, compute the occlusion shade with BSL-IN-002.
- **Expected:** a state in `exceptions` has its listed value. Each other state follows the rule
  of BSL-IN-002, item 2.

Provenance: `black_box_confirmed` (shade artefact). Kind: client agreement. Confidence: high.
Milestone: 1.

### BSL-TEST-003: Screenshot of a corner

- **Inputs:** a world in the reference and the same world in the client.
- **Steps:**
  1. Make a floor of stone. Put stone, glass, mud, a bottom slab and an oak fence on it.
  2. Set the time to noon. Turn smooth light on.
  3. Take a screenshot from the same camera position in the reference and in the client.
- **Expected:** the shade of each face looks the same. The faces next to the glass show no dark
  corner from the glass. The faces next to the mud show a dark corner.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: 1.

---

## 8. Intentional deviations

None.

---

## 9. Open questions

1. BSL-IN-001: the reference reads the light of a cell in an unloaded section from its light
   engine. This document does not state the light of such a cell. The light engine
   specification states it.
2. BSL-OUT-001: the golden artefact has only the `default` cardinal light set. A dump with the
   `nether` set confirms that row.
