# Block entity models of the client: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of the game values: `assets/minecraft/version.json`.

Some blocks have no visible shape in their block model. The block model gives only a particle
texture, and the game draws the shape from an entity model. This document states which blocks
these are, which shape and texture each block state shows, where the shape is in the block, and
how the client adds the shape to the terrain mesh. The shapes are files of the entity pack, in
the format of `entity-geometry.md`. The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Entity pack**, **file space**, **internal space**, **model unit**, **geometry**, **bone**,
  **cube**, **rest pose**, **content error**: as `entity-geometry.md`, section 0.1, defines
  them.
- **Model artefact**: the file `crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_models.json`.
  The task `dumpEntityModels` of `tools/vanilla-oracle` writes it.
- **Pose artefact**: the file
  `crates/mcrs_minecraft_model/tests/fixtures/vanilla/block_entity_poses.json`. The task
  `dumpBlockEntityPoses` writes it. It has two kinds of case: `pose` and `placement`.
- **Layer entry**: an element of `layers` in the model artefact, found by its `model` and its
  `layer`.
- **Shape**: one layer entry, in the pose that this document states for it.
- **Shape space**: the space of the parts and the cubes of a layer entry. The unit is the model
  unit (`1/16` block). +Y points down. This is the model space of the artefact header.
- **Block space**: the space of one block. The unit is the block. The block fills `[0, 1]` on
  each axis. +X is east, +Y is up and +Z is south.
- **Placement**: the affine map from shape space, divided by 16, to block space.
- **Static block entity**: a block state for which this document states at least one shape.
- **Yaw of a direction**: `south` 0°, `west` 90°, `north` 180° and `east` 270°.
- **Clockwise direction**: the horizontal direction 90° clockwise, seen from above: `north` →
  `east` → `south` → `west` → `north`.
- **Step of a direction**: `north` `(0, −1)`, `south` `(0, 1)`, `east` `(1, 0)` and `west`
  `(−1, 0)`, as `(x, z)`.
- `T(x, y, z)` is a translation. `S(x, y, z)` is a scale. `Ry(a)` and `Rz(a)` are right-handed
  rotations by the angle `a` about +Y and +Z of block space. A product `A · B` applies `B` first.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". Each rule has a Milestone line. `1` means the first release
of these files, which replaces the present code. `later` means a rule that a later release
implements.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `BEM-ART` | from an artefact entry to a geometry |
| `BEM-SPACE` | shape space and block space |
| `BEM-DATA` | the data file of a static block entity |
| `BEM-SHAPE` | the shapes, textures and placements of each block family |
| `BEM-BAKE` | how the shapes enter the terrain mesh |
| `BEM-LATER` | content that a later release adds |
| `BEM-TEST` | parity tests |
| `BEM-DEV` | intentional deviations |

### 0.4 Scope

In scope: chests, shulker boxes, banners, heads and skulls, decorated pots, the book of an
enchanting table, the book of a lectern and the body of a bell. These shapes are static. They
show the pose that the reference shows when nothing acts on the block.

Out of scope: signs, hanging signs, beds, conduits, end portals, end gateways, beacons, spawner
contents, copper golem statues, campfire items, shelf items and pistons. A later release can add
them.

---

## 1. From an artefact entry to a geometry

### BEM-ART-001: A layer entry becomes a geometry

- **Inputs:** a layer entry, and the pose case of section 3 for that layer entry, if one exists.
- **Behavior:**
  1. The conversion of `mob-pig.md`, MPIG-ART-001, applies, with the changes of items 2 to 6.
  2. When a pose case exists, each part takes the `position`, the `rotation` and the `scale` of
     the pose case, instead of its `pivot`, its `rotation` and its `scale`. The pose case names
     each part by its path, for example `root/head/jaw`.
  3. A part with a scale `(k, k, k)`, `k ≠ 1`, is accepted. The conversion multiplies by `k`,
     about the absolute pivot of that part, each of these values of the part and of each part
     below it:
     - the absolute pivot of each part below it;
     - the corner, the size and the inflate value of each cube.

     A rotation does not change. The texture coordinates of each face are the box UV
     coordinates of the unscaled size (`entity-geometry.md`, EGEO-UV-001). The cube states them
     in the per-face form (EGEO-UV-003).
  4. A cube with fewer than six names in `visible_faces` is accepted. The cube uses the
     per-face form. It lists only the faces of `visible_faces`, with their box UV coordinates.
     A face that is not in the list is not drawn.
  5. A cube `uv` with a negative element is accepted. The box UV rule applies without a change.
  6. A cube with a size that is not a whole number on an axis uses the per-face form
     (EGEO-UV-003). Each face rectangle is the rectangle of EGEO-UV-001 with the exact sizes
     `w = sx`, `h = sy` and `d = sz`, not the sizes rounded down. Item 3 and item 4 also use the
     exact sizes for such a cube.
- **Postconditions:** each layer entry of section 4 converts with no error. A part with a scale
  that is not uniform is still refused.

Provenance: `source_suggested_black_box_confirmed` (the model artefact: `minecraft:dragon_skull`
has the scale `0.75` on the part `head`; `minecraft:double_chest_left` lists five faces;
`minecraft:decorated_pot_sides` lists one face; `minecraft:decorated_pot_base` has the cube `uv`
`(−14, 13)`). Kind: semantic. Confidence: high. Milestone: 1. Experiment: BEM-TEST-001.

### BEM-ART-002: Identifiers of the geometries

- **Inputs:** a layer entry.
- **Behavior:**
  1. The identifier of the geometry is `geometry.mcrs.block.<model path>`, with `.<layer>` added
     when the layer is not `main`. For example, `minecraft:standing_banner` / `flag` becomes
     `geometry.mcrs.block.standing_banner.flag`.
  2. When one layer entry has two pose cases, each pose case makes its own geometry. The
     identifier is `geometry.mcrs.block.<model path>.<pose case>`. The two geometries of the
     book are `geometry.mcrs.block.book.enchanting_table_book` (BEM-SHAPE-007) and
     `geometry.mcrs.block.book.lectern_book` (BEM-SHAPE-008).
  3. The geometry files are in the entity pack, under `geometry/block/`. The name of a file is
     the identifier without the prefix `geometry.mcrs.block.`, with the extension `.json`. For
     example, `geometry.mcrs.block.standing_banner.flag` is in the file
     `geometry/block/standing_banner.flag.json`.
- **Postconditions:** each geometry of section 4 has one identifier.

Provenance: `design`. Kind: unspecified for the file names; semantic for item 2. Confidence:
high. Milestone: 1.

---

## 2. Spaces

### BEM-SPACE-001: From internal space to block space

- **Inputs:** a point `i` of a baked geometry in internal space (`entity-geometry.md`,
  EGEO-SPACE-002), and the placement `P` of the shape.
- **Behavior:**
  1. The point in file space is `f = (−i.x, i.y, i.z)`.
  2. The point in shape space is `a = (f.x, 24 − f.y, f.z)`. This inverts MPIG-ART-001,
     postcondition 1.
  3. The point in block space is `P · (a / 16)`.
- **Postconditions:** a vertex of the baked geometry is at the block-space position of the
  same vertex of the artefact face, transformed by the placement.

Provenance: `design` (the composition of MPIG-ART-001 and EGEO-SPACE-002). Kind: semantic.
Confidence: high. Milestone: 1. Experiment: BEM-TEST-001.

---

## 3. Data of a static block entity

### BEM-DATA-001: The data file

- **Inputs:** the files under `assets/mcrs/block_entity_model/`.
- **Behavior:**
  1. A file `<namespace>/<path>.json` describes the block `<namespace>:<path>`.
  2. The file is a JSON object with the field `layers`. `layers` is an array of layer objects.
  3. A layer object has these fields:

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `when` | Molang string | `true` | a condition in the `block` context (`molang-queries.md`, MOLQ-BLOCK-001) |
| `geometry` | string | required | the identifier of a geometry (BEM-ART-002) |
| `texture` | string | required | a texture id `ns:path`, which is the file `assets/ns/textures/path.png` |
| `tint` | integer | none | a dye id from 0 to 15 (BEM-BAKE-005) |
| `surface` | string | required | `opaque`, `cutout` or `blend` (BEM-BAKE-002) |
| `back_faces` | boolean | required | `true` when the back side of each face shows (BEM-BAKE-002) |
| `placement` | array of 3 rows of 4 numbers | required | the first three rows of `P`; the fourth row is `(0, 0, 0, 1)` |

  4. For each block state, the client evaluates `when` of each layer at load. Each layer with
     a true condition is drawn. The order of the layers is the order of the array.
  5. A block with no file is not a static block entity.
  6. A state value that the block does not have is an error, and the client draws nothing for that state.
- **Postconditions:** each block state has a list of drawn layers, which can be empty. An error
  in a file is a content error that names the file. A file with an error draws nothing.

Provenance: `design`. Kind: semantic. Confidence: medium. Milestone: 1.

### BEM-DATA-002: Content of the data files

- **Inputs:** the rules of section 4.
- **Behavior:**
  1. The data files contain exactly the shapes, the textures, the tints, the surfaces and the
     placements of section 4, for each block state of each block of section 4.
  2. A generator makes the data files and the geometry files of BEM-ART-002 from the model
     artefact, the pose artefact and the rules of section 4. The repository contains the files
     that the generator makes.
  3. A test runs the generator and compares its output with the files of the repository. A
     difference fails the test. The client does not read an artefact at run time.
- **Postconditions:** BEM-TEST-001 passes for each block state of section 4. A new version of
  the artefacts changes the files only through the generator.

Provenance: `design`. Kind: semantic. Confidence: high. Milestone: 1.

---

## 4. Shapes

The tables of this section use the yaw `y(d)` of a direction `d`. A placement matrix in the
pose artefact has the `shape` and the `state` that the rule names. Each placement formula of
this section agrees with the matrix of the pose artefact within `1e-6`.

### BEM-SHAPE-001: Chests

- **Inputs:** a block state of `minecraft:chest`, `minecraft:trapped_chest`,
  `minecraft:ender_chest`, `minecraft:copper_chest`, `minecraft:exposed_copper_chest`,
  `minecraft:weathered_copper_chest`, `minecraft:oxidized_copper_chest`, and of the four blocks
  `minecraft:waxed_<…>_copper_chest` that match the last four.
- **Behavior:**
  1. The property `type` selects the layer entry. A block with no `type` uses `single`.

| `type` | Layer entry | Texture suffix |
|--------|-------------|----------------|
| `single` | `minecraft:chest` / `main` | none |
| `left` | `minecraft:double_chest_left` / `main` | `_left` |
| `right` | `minecraft:double_chest_right` / `main` | `_right` |

  2. The texture is `minecraft:entity/chest/<material><suffix>`. The material of each block is:

| Block | Material |
|-------|----------|
| `chest` | `normal` |
| `trapped_chest` | `trapped` |
| `ender_chest` | `ender` |
| `copper_chest`, `waxed_copper_chest` | `copper` |
| `exposed_copper_chest`, `waxed_exposed_copper_chest` | `copper_exposed` |
| `weathered_copper_chest`, `waxed_weathered_copper_chest` | `copper_weathered` |
| `oxidized_copper_chest`, `waxed_oxidized_copper_chest` | `copper_oxidized` |

  3. The pose is the rest pose. The lid is closed.
  4. The placement is `P = T(0.5, 0, 0.5) · Ry(−y(facing)) · T(−0.5, 0, −0.5)`.
  5. The surface is `cutout`. `back_faces` is `false`.
- **Postconditions:** a single chest that faces `north` fills `[1/16, 15/16]` on X, `[0, 14/16]`
  on Y and `[0, 15/16]` on Z. The lock is on the side that the chest faces. The side of a
  half where it touches the other half has no face.

Provenance: `source_suggested_black_box_confirmed` (the placement cases `chest` of the pose
artefact; the five faces of the double-chest entries of the model artefact; the texture files
`assets/minecraft/textures/entity/chest/`). Kind: semantic. Confidence: high. Milestone: 1.
Experiment: BEM-TEST-001 and BEM-TEST-002.

### BEM-SHAPE-002: Christmas chests

- **Inputs:** a block state of `minecraft:chest` or `minecraft:trapped_chest`, and the local
  date of the client.
- **Behavior:** from 24 December to 26 December, the material of both blocks is `christmas`.
  The client reads the date one time, when it loads the data.
- **Postconditions:** on those dates, a normal chest and a trapped chest look the same.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: later.
Experiment: set the date of the computer to 25 December. Start the game. Put down a chest and a
trapped chest. Both show the texture `entity/chest/christmas.png`.

### BEM-SHAPE-003: Shulker boxes

- **Inputs:** a block state of `minecraft:shulker_box` or of `minecraft:<colour>_shulker_box`
  for each of the 16 dye colours.
- **Behavior:**
  1. The layer entry is `minecraft:shulker_box` / `main`. The pose is the rest pose. The lid is
     closed.
  2. The texture is `minecraft:entity/shulker/shulker` for `shulker_box`, and
     `minecraft:entity/shulker/shulker_<colour>` for a dyed box.
  3. The placement depends on `facing`. Each matrix below is multiplied by `0.9995`. Then the
     translation `t` is added:

| `facing` | Rows of the 3 × 3 part | `t` |
|----------|------------------------|-----|
| `down` | `(1, 0, 0)`, `(0, 1, 0)`, `(0, 0, 1)` | `(0.5, −0.4995, 0.5)` |
| `up` | `(1, 0, 0)`, `(0, −1, 0)`, `(0, 0, −1)` | `(0.5, 1.4995, 0.5)` |
| `north` | `(−1, 0, 0)`, `(0, 0, 1)`, `(0, 1, 0)` | `(0.5, 0.5, −0.4995)` |
| `south` | `(1, 0, 0)`, `(0, 0, 1)`, `(0, −1, 0)` | `(0.5, 0.5, 1.4995)` |
| `west` | `(0, 1, 0)`, `(0, 0, 1)`, `(1, 0, 0)` | `(−0.4995, 0.5, 0.5)` |
| `east` | `(0, −1, 0)`, `(0, 0, 1)`, `(−1, 0, 0)` | `(1.4995, 0.5, 0.5)` |

  4. The surface is `cutout`. `back_faces` is `true`.
- **Postconditions:** for each `facing`, the box fills `[0.00025, 0.99975]` on each axis. The
  lid is on the side that `facing` names.

Provenance: `source_suggested_black_box_confirmed` (the placement cases `shulker_box` of the
pose artefact). Kind: semantic. Confidence: high. Milestone: 1. Experiment: BEM-TEST-001.

### BEM-SHAPE-004: Banners

- **Inputs:** a block state of `minecraft:<colour>_banner` or `minecraft:<colour>_wall_banner`,
  for each of the 16 dye colours.
- **Behavior:**
  1. A standing banner uses the layer entries `minecraft:standing_banner` / `main` and
     `minecraft:standing_banner` / `flag`. A wall banner uses `minecraft:wall_banner` / `main`
     and `minecraft:wall_banner` / `flag`.
  2. The `main` layer has the texture `minecraft:entity/banner/banner_base` and no tint.
  3. The `flag` layer has the texture `minecraft:entity/banner/base`. Its tint is the dye id of
     the colour of the block (BEM-BAKE-005).
  4. The `flag` layer uses the pose case `banner_flag` of the pose artefact for its layer entry.
     The flag turns by `−0.0025π` radians about X. The `main` layer uses the rest pose.
  5. The angle `A` is `22.5° · rotation` for a standing banner and `y(facing)` for a wall banner.
     The placement is `P = T(0.5, 0, 0.5) · Ry(−A) · S(2/3, −2/3, −2/3)`.
  6. The surface is `opaque`. `back_faces` is `false`.
- **Postconditions:** a standing banner reaches `44 · (2/3) / 16` block above its floor. A wall
  banner hangs into the block below it.

Provenance: `source_suggested_black_box_confirmed` (the placement cases `standing_banner` and
`wall_banner` and the pose cases `banner_flag` of the pose artefact). Kind: semantic.
Confidence: high. Milestone: 1. Experiment: BEM-TEST-001.

### BEM-SHAPE-005: Heads and skulls

- **Inputs:** a block state of one of the blocks of the table below, standing or on a wall.
- **Behavior:**
  1. The kind of head selects the layer entry, the texture and the pose:

| Blocks | Layer entry | Texture | Pose |
|--------|-------------|---------|------|
| `skeleton_skull`, `skeleton_wall_skull` | `minecraft:skeleton_skull` | `minecraft:entity/skeleton/skeleton` | rest |
| `wither_skeleton_skull`, `wither_skeleton_wall_skull` | `minecraft:wither_skeleton_skull` | `minecraft:entity/skeleton/wither_skeleton` | rest |
| `zombie_head`, `zombie_wall_head` | `minecraft:zombie_head` | `minecraft:entity/zombie/zombie` | rest |
| `creeper_head`, `creeper_wall_head` | `minecraft:creeper_head` | `minecraft:entity/creeper/creeper` | rest |
| `piglin_head`, `piglin_wall_head` | `minecraft:piglin_head` | `minecraft:entity/piglin/piglin` | pose case `piglin_head` |
| `dragon_head`, `dragon_wall_head` | `minecraft:dragon_skull` | `minecraft:entity/enderdragon/dragon` | pose case `dragon_head` |
| `player_head`, `player_wall_head` | `minecraft:player_head` | `minecraft:entity/player/wide/steve` | rest |

  2. The layer of each entry is `main`.
  3. A standing head has the placement `P = T(0.5, 0, 0.5) · Ry(−22.5° · rotation) · S(−1, −1, 1)`.
  4. A wall head with the direction `d = facing` and the step `(sx, sz)` of `d` has the
     placement `P = T(0.5 − 0.25 · sx, 0.25, 0.5 − 0.25 · sz) · Ry(−y(opposite of d)) · S(−1, −1, 1)`.
  5. The surface is `blend`. `back_faces` is `true`. BEM-BAKE-002, item 5, applies.
- **Postconditions:**
  1. A creeper head with `rotation` 0 fills `[0.25, 0.75]` on X and Z and `[0, 0.5]` on Y.
  2. A skeleton wall skull that faces `north` fills `[0.25, 0.75]` on X and Y and `[0.5, 1]` on
     Z. It is against the south side of its block.
  3. The ears of a piglin head turn by `∓0.7` radians about Z. The jaw of a dragon head turns by
     `0.2` radians about X, and the jaw goes below the floor of the block by about `2.3/16`
     block.

Provenance: `source_suggested_black_box_confirmed` (the placement cases `standing_head` and
`wall_head` and the pose cases `piglin_head` and `dragon_head` of the pose artefact); the
textures and the player skin are `source_suggested`. Kind: semantic. Confidence: high for the
geometry, medium for the player texture. Milestone: 1. Experiment: BEM-TEST-001; for the player
head, BEM-TEST-002 with a player head that has no profile.

### BEM-SHAPE-006: Decorated pots

- **Inputs:** a block state of `minecraft:decorated_pot`.
- **Behavior:**
  1. The pot has two layers:

| Layer entry | Texture |
|-------------|---------|
| `minecraft:decorated_pot_base` / `main` | `minecraft:entity/decorated_pot/decorated_pot_base` |
| `minecraft:decorated_pot_sides` / `main` | `minecraft:entity/decorated_pot/decorated_pot_side` |

  2. Both layers use the rest pose.
  3. The placement is `P = T(0.5, 0.5, 0.5) · Ry(180° − y(facing)) · T(−0.5, −0.5, −0.5)`.
  4. The surface is `opaque`. `back_faces` is `false`.
- **Postconditions:** the pot fills `[1/16, 15/16]` on X and Z. The neck ends at `19.9/16`
  block on Y. Each side shows the plain side texture.

Provenance: `source_suggested_black_box_confirmed` (the placement cases `decorated_pot` of the
pose artefact; the one face of each side in the model artefact). Kind: semantic. Confidence:
high. Milestone: 1. Experiment: BEM-TEST-001.

### BEM-SHAPE-007: The book of an enchanting table

- **Inputs:** a block state of `minecraft:enchanting_table`.
- **Behavior:**
  1. The layer entry is `minecraft:book` / `main`, in the pose case `enchanting_table_book`. The
     book is closed: the left cover turns by `π` radians about Y, and each page and the right
     cover keep the rotation 0.
  2. The texture is `minecraft:entity/enchantment/enchanting_table_book`.
  3. The placement is `P = T(0.5, 0.85, 0.5) · Rz(80°)`.
  4. The surface is `opaque`. `back_faces` is `false`.
- **Postconditions:** the book floats above the table, between `12.7/16` and `20.4/16` block on
  Y.

Provenance: `source_suggested_black_box_confirmed` for the pose (the pose case
`enchanting_table_book`); `source_suggested` for the placement. Kind: semantic. Confidence:
medium. Milestone: 1. Experiment: put an enchanting table in a closed room with no player near
it. Take a screenshot from the side at a known camera. Compare the book outline with
BEM-TEST-002. BEM-DEV-001 records the motion of the reference.

### BEM-SHAPE-008: The book of a lectern

- **Inputs:** a block state of `minecraft:lectern`.
- **Behavior:**
  1. The book shows only when `has_book` is `true`.
  2. The layer entry is `minecraft:book` / `main`, in the pose case `lectern_book`. The book is
     open. These are the values of the pose case:

| Part | Position (model units) | Rotation about Y (radians) |
|------|------------------------|----------------------------|
| `left_lid` | `(0, 0, −1)` | `π + 1.5` |
| `right_lid` | `(0, 0, 1)` | `−1.5` |
| `left_pages` | `(sin 1.5, 0, 0)` | `1.5` |
| `right_pages` | `(sin 1.5, 0, 0)` | `−1.5` |
| `flip_page1` | `(sin 1.5, 0, 0)` | `1.2` |
| `flip_page2` | `(sin 1.5, 0, 0)` | `−1.2` |
| `seam` | `(0, 0, 0)` | `π/2` |

  3. The texture is `minecraft:entity/enchantment/enchanting_table_book`.
  4. Let `c` be the clockwise direction of `facing`. The placement is
     `P = T(0.5, 1.0625, 0.5) · Ry(−y(c)) · Rz(67.5°) · T(0, −0.125, 0)`.
  5. The surface is `opaque`. `back_faces` is `false`.
- **Postconditions:** the open book lies on the top of the lectern, between `14.3/16` and
  `20.4/16` block on Y.

Provenance: `source_suggested_black_box_confirmed` for the pose (the pose case `lectern_book`);
`source_suggested` for the placement. Kind: semantic. Confidence: medium. Milestone: 1.
Experiment: put four lecterns with books, one for each `facing`. Take a screenshot from above
at a known camera. Compare each book outline with BEM-TEST-002.

### BEM-SHAPE-009: The body of a bell

- **Inputs:** a block state of `minecraft:bell`.
- **Behavior:**
  1. The layer entry is `minecraft:bell` / `main`, in the rest pose. The block model of the bell
     gives the frame. This rule gives only the body.
  2. The texture is `minecraft:entity/bell/bell_body`.
  3. The placement is `P = S(1, 1, 1)` for each value of `attachment` and `facing`.
  4. The surface is `opaque`. `back_faces` is `false`.
- **Postconditions:** the body fills `[4/16, 12/16]` on X and Z and `[4/16, 13/16]` on Y.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: 1. Experiment:
put a bell on the floor, on a wall and under a block. Take a screenshot of each. The body is at
the same place in each block.

---

## 5. The terrain mesh

### BEM-BAKE-001: Shapes are static faces of the block

- **Inputs:** a block state, its block model, and the drawn layers of BEM-DATA-001.
- **Behavior:**
  1. The client bakes each drawn layer one time for each block state: the geometry
     (`entity-geometry.md`, EGEO-BAKE-001) in the pose of section 4, then BEM-SPACE-001.
  2. The baked faces are added to the faces of the block model of the block state.
  3. A neighbour block never removes a face of a shape.
  4. The texture coordinates of each face are those of EGEO-UV-004, in the texture of the layer.
- **Postconditions:** the shapes change only when the block state changes. The chunk mesh draws
  them with the other faces of the block.

Provenance: `design` (the reference draws these shapes in each frame, as entities). Kind:
semantic. Confidence: high. Milestone: 1. Experiment: BEM-TEST-002.

### BEM-BAKE-002: Surface and back faces

- **Inputs:** the `surface` and `back_faces` of a layer.
- **Behavior:**
  1. `opaque`: the client ignores the alpha of the texture.
  2. `cutout`: a texel with alpha below `0.1` is not drawn. The other texels are opaque.
  3. `blend`: the texel blends with what is behind it, by its alpha. A texel with alpha below
     `0.1` is not drawn.
  4. When `back_faces` is `false`, a face shows only from its front side. When it is `true`, a
     face shows from both sides.
  5. In milestone 1, the terrain mesh draws a `blend` layer as `cutout`. BEM-DEV-002 records
     this difference.
- **Postconditions:** none.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: 1. Experiment:
look into an open shulker box in the reference. The inner sides of the lid show. Look at a chest
from inside a block with spectator mode. The inner sides of the chest do not show.

### BEM-BAKE-003: Shading of a face

- **Inputs:** a baked face, with its normal in block space.
- **Behavior:**
  1. The shade factor of the face is the factor of `client-entity-render.md`, CEREN-SURF-002,
     for the normal of the face. The dimension rule of that rule also applies.
  2. A face that shows from its back side uses the negated normal for that side.
  3. The face has no ambient occlusion, also when the block model has ambient occlusion.
  4. The client computes the factor when it builds the mesh of the section. It uses the
     `cardinal_light` of the dimension type of the client world. The baked shape keeps the
     normal of each face, not the factor. When the `cardinal_light` of the client world changes,
     the client builds the mesh of each section again, as for the block faces
     (`block-face-bake.md`, BFB-OUT-003).
- **Postconditions:** a shape has the shading of an entity, not the shading of a block face.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: 1. Experiment:
put a chest next to a stone block in full daylight. Take a screenshot. The top of the chest and
the top of the stone have the same brightness. The north side of the chest is brighter than the
north side of the stone.

### BEM-BAKE-004: Light of a shape

- **Inputs:** the block light and the sky light of the block position of the static block
  entity.
- **Behavior:**
  1. Each vertex of each shape uses the block light and the sky light of the block position of
     the static block entity. The client does no smooth light for these faces.
  2. A later release: a chest with `type` `left` or `right` uses, for each light kind, the
     maximum of the light of its own position and the light of the other half. The other half
     is the neighbour in the direction that `type` and `facing` give. Until then, each half uses
     the light of its own position.
  3. Item 1 is the light of `block-smooth-light.md`, BSL-FLAT-001, item 3, for each face of a
     shape. A face of a shape on the boundary of the block also uses the own cell.
- **Postconditions:** a shape has one light value. A shape can be lighter or darker than the
  faces of its own block model.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: 1. Experiment:
put a double chest in a dark room. Put a torch next to one half only. Both halves have the same
brightness.

### BEM-BAKE-005: Dye tint of a banner flag

- **Inputs:** a layer with a `tint`.
- **Behavior:** the client multiplies the colour of each texel by the colour of the dye id:

| Id | Dye | Colour | Id | Dye | Colour |
|----|-----|--------|----|-----|--------|
| 0 | white | `0xF9FFFE` | 8 | light_gray | `0x9D9D97` |
| 1 | orange | `0xF9801D` | 9 | cyan | `0x169C9C` |
| 2 | magenta | `0xC74EBD` | 10 | purple | `0x8932B8` |
| 3 | light_blue | `0x3AB3DA` | 11 | blue | `0x3C44AA` |
| 4 | yellow | `0xFED83D` | 12 | brown | `0x835432` |
| 5 | lime | `0x80C71F` | 13 | green | `0x5E7C16` |
| 6 | pink | `0xF38BAA` | 14 | red | `0xB02E26` |
| 7 | gray | `0x474F52` | 15 | black | `0x1D1D21` |

- **Postconditions:** a banner flag has the colour of its block name.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Milestone: 1. Experiment: put
the 16 standing banners in a row in full daylight. Take a screenshot. Sample the centre of each
flag. Each colour is the texel of `entity/banner/base.png` multiplied by the table colour and by
the shade and light of the face.

---

## 6. Later work

### BEM-LATER-001: Content that the first release leaves out

- **Inputs:** none.
- **Behavior:** the first release does not draw these. A later release can draw them:
  - the patterns of a banner;
  - the sherd patterns of a decorated pot;
  - the skin of a player head from its profile;
  - the motion of each shape: an open lid, the motion of the enchanting book, the wind on a
    banner, a bell that rings, a pot that wobbles, the ears of a powered piglin head and the jaw
    of a powered dragon head;
  - the crack overlay of a block that a player breaks, on the faces of a shape;
  - the shared light of the two halves of a double chest (BEM-BAKE-004, item 2).
- **Postconditions:** none.

Provenance: `design`. Kind: semantic. Confidence: high. Milestone: later.

---

## 7. Parity tests

### BEM-TEST-001: Baked vertices against the artefacts

- **Inputs:** the model artefact, the pose artefact and the data files.
- **Behavior:**
  1. For each block state of section 4, bake the drawn layers (BEM-BAKE-001).
  2. For each layer, take the cubes of its layer entry in shape space, in the pose of section 4.
     Transform each corner by the part transforms of the artefact header (translate by the
     position, then rotate Z, Y, X, then scale), divide by 16, then apply the placement.
  3. For the shapes of BEM-SHAPE-001 to BEM-SHAPE-006, take the placement matrix of the pose
     artefact. For BEM-SHAPE-007 to BEM-SHAPE-009, take the formula of the rule.
  4. Each baked vertex agrees with a vertex of step 2 within `1e-5` block. Each face has the
     texture coordinates of the matching face of the model artefact within `1e-5`.
  5. The number of faces of each cube agrees with `visible_faces`.
- **Postconditions:** the test fails when one vertex, one texture coordinate or one face count
  does not agree.

Provenance: `source_suggested_black_box_confirmed` (both artefacts). Kind: semantic. Confidence:
high. Milestone: 1.

### BEM-TEST-002: Screenshot of the blocks

- **Inputs:** a world with one block state of each row of section 4, in full daylight.
- **Behavior:**
  1. Put the blocks in a line on a flat floor, with one empty block between them.
  2. Take a screenshot in the reference and in the client, at the same camera position and the
     same field of view.
  3. Compare the two images.
- **Postconditions:** the outline of each shape agrees within one pixel at 1920 × 1080. The
  colour of each face agrees within 3 of 255 in each channel. The texels of BEM-DEV-002 are not
  compared.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Milestone: 1.

---

## 8. Intentional deviations

### BEM-DEV-001: Static shapes

- **Rule:** BEM-BAKE-001.
- **Reference:** the reference draws these shapes in each frame. The enchanting book bobs up
  and down, turns slowly and opens when a player is near. A banner flag moves in the wind. A
  chest lid and a shulker lid open, a bell swings when it rings, a pot wobbles when a player
  hits it. A powered piglin head moves its ears, and a powered dragon head moves its jaw.
- **Visible to the client:** yes. The shapes do not move.
- **Test:** a differential test compares only the poses of section 4. A screenshot test uses a
  scene where nothing acts on the blocks, and it uses an enchanting table with no player near it
  at its first frame.

### BEM-DEV-002: Heads with blend

- **Rule:** BEM-BAKE-002, item 5.
- **Reference:** the reference blends each texel of a head by its alpha.
- **Visible to the client:** only for a texel with an alpha between `0.1` and `1`. Such a texel
  shows as opaque in the client. A later release draws `blend` layers with blend.
- **Test:** a differential test does not compare the colour of a head texel with an alpha
  between `0.1` and `1`.

---

## 9. Open questions

1. BEM-BAKE-002: the reference draws chests, banners, books, bells and pots with back faces
   hidden, and shulker boxes and heads with back faces shown. The experiment of BEM-BAKE-002
   checks only two of the families.
2. BEM-BAKE-004: the reference raises the block light to the light that the block itself
   emits. None of the blocks of section 4 emits light, so this document does not state it.
3. BEM-SHAPE-005: the reference can select a different default skin for a player head with no
   profile. This document uses the wide default skin.
