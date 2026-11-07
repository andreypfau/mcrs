# The pig of the client: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| Entity type | `minecraft:pig` |

Source of the game values: `assets/minecraft/version.json`.

This document states what the client draws for a pig: the geometries, the textures of each
variant, the pose in each frame and the saddle layer. It also states how the oracle artefacts
of the entity models, the keyframe clips and the poses become files of the entity pack.
`mob-frog.md` uses these artefact rules. The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Entity pack**, **file space**, **internal space**, **model unit**, **geometry**, **bone**,
  **cube**, **rest pose**, **content error**: as `entity-geometry.md`, section 0.1, defines
  them.
- **Clip**, **channel**, **keyframe**, **play list**: as `entity-animation.md`, section 0.1,
  defines them.
- **Client entity**, **render controller**, **layer**: as `client-entity-definition.md`,
  section 0.1, defines them.
- **Model artefact**: the file `crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_models.json`.
  The task `dumpEntityModels` of `tools/vanilla-oracle` writes it.
- **Clip artefact**: the file `crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_animations.json`.
  The task `dumpEntityAnimations` writes it.
- **Pose artefact**: the file `crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_poses.json`.
  The task `dumpEntityPoses` writes it.
- **Type artefact**: the file `crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_types.json`.
  The task `dumpEntityTypes` writes it. For each entity type, it gives the size, the eye
  height, the update interval, the fire immunity and the entity data indices with their wire
  types.
- **Artefact space**: the space of the model artefact and the pose artefact. +Y points down.
  The entity front points to −Z. +X points to the left side of the entity. The unit is the
  model unit. Rotations are in radians. Internal space is artefact space turned 180° about
  the Z axis: an artefact point `(x, y, z)` is the internal point `(−x, −y, z)`, relative to
  the same origin.
- **Layer entry**: one element of `layers` in the model artefact. It has a `model` (a resource
  location), a `layer` (a name), a texture size and a part tree.
- **Part**: a node of the part tree of a layer entry. A part has a `pivot`, a `rotation`, a
  `scale`, cubes and child parts. The pivot is relative to the pivot of the parent part.
- **Absolute pivot**: the sum of the pivots of a part and of all its ancestors.
- **Variant entry**: an entry of the registry `minecraft:pig_variant`.
- **Walk swing**: the angle `cos(m · 38.1704) · 80.2141 · s` in degrees, with
  `m = query.modified_distance_moved` and `s = query.modified_move_speed`
  (`molang-queries.md`, MOLQ-WALK-001). `38.1704°` is `0.6662` radians. `80.2141°` is `1.4`
  radians.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". The label `design` of `entity-geometry.md`, section 0.2,
applies.

Each rule has a line **Milestone**, with the values of `entity-geometry.md`, section 0.2.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `MPIG-ART` | the conversion of the oracle artefacts to the entity pack |
| `MPIG-DATA` | the entity data of the pig |
| `MPIG-GEO` | the geometries of the pig |
| `MPIG-VAR` | variants, textures and the selection of a geometry |
| `MPIG-ANIM` | the pose of the pig in a frame |
| `MPIG-LAYER` | the saddle layer |
| `MPIG-CONST` | constants of the pig that other specifications use |
| `MPIG-DEV` | intentional deviations |

### 0.4 Scope

In scope: the geometries, the textures, the variant selection, the pose, the saddle layer and
the per-type constants of the pig. The artefact rules for each mob also are in scope.

Out of scope: the client state of an entity (`client-entities.md`), the placement transform,
the hurt colour, the death rotation, the light, the shadow shape and the name tag
(`client-entity-render.md`). Sounds and particles are out of scope. A pig that a player rides
is out of scope, except where `client-entities.md` states it.

---

## 1. The oracle artefacts

### MPIG-ART-001: A layer entry becomes a geometry

- **Inputs:** a layer entry of the model artefact.
- **Behavior:**
  1. The top part of the layer entry has the pivot `(0, 0, 0)`, no rotation and no cube. It is
     not a bone. Each child part of the top part becomes a bone without a parent.
  2. Each other part becomes a bone with the same name. The bone of its parent part is its
     parent bone.
  3. Let `A = (ax, ay, az)` be the absolute pivot of the part, without the top part. The bone
     pivot in file space is `(ax, 24 − ay, az)`.
  4. The bone rotation in file space is the part rotation, converted from radians to degrees.
     The signs do not change.
  5. Each cube of the part becomes a cube of the bone:
     - The cube origin in file space is `(ax + cx, 24 − (ay + cy + sy), az + cz)`. `(cx, cy, cz)`
       is the `corner` of the cube and `(sx, sy, sz)` is its `size`.
     - The cube size is the `size`. The cube `uv` is the `uv`. The cube `mirror` is the
       `mirror`.
     - The cube `inflate` is the first element of `inflate`.
  6. The texture size of the geometry is `texture_width` and `texture_height`.
  7. The geometry file uses the format of `entity-geometry.md`, EGEO-FMT-003, with box UV.
  8. Each element of `faces` of a cube has an `internal_direction`. It names the face in
     internal space: `up`, `down`, `east` (+X), `west` (−X), `north` (−Z) or `south` (+Z).
     The `normal` and the `vertices` of the face are in artefact space.
- **Postconditions:**
  1. A point of a cube with the artefact position `(x, y, z)`, relative to the absolute pivot
     of its part, is at the file position `(ax + x, 24 − (ay + y), az + z)` in the rest pose.
  2. The file position `y = 0` is the ground under the entity.
- **Conditions that the conversion refuses:** a part `scale` that is not `(1, 1, 1)`, an
  `inflate` whose three values are not all equal, a `uv_scale` that is not `(1, 1)`,
  and a `visible_faces` list with fewer than six faces. The conversion stops
  with an error that names the layer entry. No layer entry of this document or of `mob-frog.md`
  has one of these conditions.

Provenance: `source_suggested_black_box_confirmed`
(`crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_models.json`: the cube `faces` of
each layer entry give the vertices and the texture coordinates of the box UV). Kind: semantic.
Confidence: high. Milestone: 1. Experiment: convert the layer entry `minecraft:pig` / `main`.
Bake it with `entity-geometry.md`, EGEO-BAKE-001. Convert each baked vertex back to artefact
space with postcondition 1. Find the artefact face by its `internal_direction`. Each vertex
and each texture coordinate agree with the `faces` of the artefact within `1e-4`.

### MPIG-ART-002: A clip entry becomes a clip

- **Inputs:** an element of `clips` in the clip artefact.
- **Behavior:**
  1. The clip gets the identifier `animation.<owner>.<name>`, where `<owner>/<name>` is the
     `id` of the element.
  2. `animation_length` is `length`.
  3. `loop` is `true` when `looping` is true. Otherwise, the specification of the mob states
     `loop`.
  4. Each channel of a bone becomes a channel of the bone with the same name. Each keyframe
     becomes a keyframe at the time `time`, with `pre` and `post` from the artefact. The
     `lerp_mode` of the keyframe is the `interpolation` of the artefact keyframe.
  5. A value of a `position` channel `(x, y, z)` becomes `(x, −y, z)` in file space.
  6. A value of a `rotation` channel becomes degrees. The signs do not change.
  7. A value of a `scale` channel `(x, y, z)` becomes `(x + 1, y + 1, z + 1)`.
  8. The specification of the mob states `anim_time_update` and `blend_weight` where they are
     not the defaults.
- **Postconditions:** one clip instance with the weight `w` adds to each bone the same offset
  that the reference adds with the weight `w`.

Provenance: `source_suggested_black_box_confirmed`
(`crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_animations.json`, with the units
in its `units` object; `entity_poses.json`, the cases of `minecraft:frog`). Kind: semantic.
Confidence: medium. Milestone: 2.

### MPIG-ART-003: A pose case is a parity test

- **Inputs:** an element of `cases` in the pose artefact.
- **Behavior:**
  1. `model` and `layer` name the layer entry. `inputs` give the values of the queries for
     the frame:

| Input | Query or state |
|-------|----------------|
| `walk_position` | `query.modified_distance_moved` |
| `walk_speed` | `query.modified_move_speed` |
| `head_yaw_degrees` | `query.target_y_rotation` |
| `head_pitch_degrees` | `query.target_x_rotation` |
| `age_ticks` | the age of the entity in ticks, with the partial tick |
| `swimming` | `query.is_in_water` (`mob-frog.md`) |
| `clip_start_ticks` | the tick at which each pose clip and the clip `idle_water` of `mob-frog.md` started; `null` when it is stopped |
| `clip_start_ticks.jump`, `.croak`, `.tongue` | `query.pose`: `'long_jumping'` when `jump` is not `null`, `'croaking'` when `croak` is not `null`, `'using_tongue'` when `tongue` is not `null`, and `'standing'` when all three are `null` |

  2. `parts` give, for each part path, the pose after the pose step of the reference. A path
     is the names from the top part, joined with `/`.
  3. A test converts each part pose to file space with the rules of MPIG-ART-001. A position
     `(x, y, z)` relative to the parent pivot is the bone offset `(x − px, −(y − py), z − pz)`
     in file space, where `(px, py, pz)` is the rest pivot of the part.
  4. The pose of the client agrees with each case within the limits of `entity-animation.md`,
     EANIM-GPU-003. The case rotations and the rotation limits of that rule are both in
     radians.
  5. A part with `visible: false` is a hidden bone (`entity-geometry.md`, EGEO-POSE-002).
  6. A test sets the time of each clip instance from the inputs, and truncates the time to a
     whole millisecond (MPIG-DEV-002). The truncation comes before the loop wrap of
     `entity-animation.md`, EANIM-TIME-002. For a pose clip, the time is
     `(age_ticks − start) / 20` s. `age_ticks` is the entity clock `C` of `client-entities.md`,
     section 0.1. For a walk clip, it is the `anim_time_update` of the clip.
  7. `clip_start_ticks` decides which pose clips and which idle clips play. A clip with a
     start tick plays with the weight 1. A clip with `null` does not play. These values
     replace the weight expressions of the play list of the mob for those clips. The weight
     expressions of the other clips, for example `walk` and `swim`, apply.
- **Postconditions:** a deviation of a mob specification names the cases that a test skips.

Provenance: `design` (the tolerance and item 7: the artefact gives the state of each clip, not
the pose of the entity); `black_box_confirmed` for the content
(`crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_poses.json`). Kind: semantic.
Confidence: high. Milestone: 1.

---

## 2. Entity data

### MPIG-DATA-001: Indices of the pig

- **Inputs:** the entity data of a pig (`client-entities.md`, CENTS-DATA-001).
- **Behavior:** the indices 0 to 17 are those of `client-entities.md`, CENTS-DATA-002. The pig
  also has these indices:

| Index | Wire type | Default | Meaning | Read by the client |
|-------|-----------|---------|---------|--------------------|
| 18 | `VarInt` (type id 1) | 0 | boost time, in ticks | no |
| 19 | `PigVariant` (type id 28) | the id of `minecraft:temperate` | variant | yes |
| 20 | `PigSoundVariant` (type id 29) | the id of `minecraft:classic` | sound variant | no |

  1. A value of index 19 is the numeric id of an entry of the registry `minecraft:pig_variant`.
     The numeric id is the position of the entry in the registry data that the server sent,
     from 0.
  2. The default is the id of `minecraft:temperate` when the synced registry has that entry.
     Otherwise, it is the id of one entry of the registry. Which entry is unspecified.
  3. The server does not send an index that has its default value. The default of index 19
     therefore selects the texture of most pigs that the server adds.
  4. `query.variant`, `query.variant_identifier`, `query.variant_asset` and
     `query.variant_model` read the entry of index 19 (`molang-queries.md`, MOLQ-STATE-001 and MOLQ-STATE-002).
- **Postconditions:** a pig with no value at index 19 is a temperate pig.

Provenance: `black_box_confirmed` for the indices and the wire types
(`crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_types.json`, the type
`minecraft:pig`; the type ids are those of the entity data codec of
`crates/mcrs_minecraft_protocol_game/src/entity.rs`); `source_suggested` for the defaults and
item 3. Kind: wire for the indices and the types; semantic for the defaults. Confidence:
medium. Milestone: 1. Experiment: summon a pig with no variant and a pig with the variant
`minecraft:cold` on the vanilla server. Capture the add entity and set entity data packets.
The first pig has no entry 19. The second has entry 19 with the id of `minecraft:cold`.

---

## 3. Geometries

### MPIG-GEO-001: The four geometries

- **Inputs:** the model artefact.
- **Behavior:** the entity pack has four geometries of the pig, in one file
  `assets/mcrs/entity/geometry/pig.json` with the format version `1.12.0`. Each geometry is the
  conversion (MPIG-ART-001) of one layer entry:

| Geometry identifier | Layer entry | Texture size | Use |
|---------------------|-------------|--------------|-----|
| `geometry.pig` | `minecraft:pig` / `main` | 64 × 64 | adult, model `normal` |
| `geometry.pig.cold` | `minecraft:cold_pig` / `main` | 64 × 64 | adult, model `cold` |
| `geometry.pig.baby` | `minecraft:pig_baby` / `main` | 32 × 32 | each baby |
| `geometry.pig.saddle` | `minecraft:pig` / `saddle` | 64 × 64 | the saddle layer (MPIG-LAYER-001) |

- **Postconditions:**
  1. Each geometry has the six bones `body`, `head`, `left_front_leg`, `left_hind_leg`,
     `right_front_leg` and `right_hind_leg`. Each bone has no parent.
  2. `geometry.pig.cold` has a second cube on `body`, with an inflate of `0.5` and the
     texture offset `(28, 32)`.
  3. In `geometry.pig.saddle`, each cube has an inflate of `0.5`.
  4. In `geometry.pig` and `geometry.pig.cold`, the cubes of the left legs have `mirror: true`.
     In `geometry.pig.baby`, no cube has `mirror: true`, and each leg has its own texture
     offset.

Provenance: `black_box_confirmed`
(`crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_models.json`). Kind: semantic.
Confidence: high. Milestone: 1.

---

## 4. Variants

### MPIG-VAR-001: The variant data

- **Inputs:** the variant entry of the pig. Index 19 of the entity data gives it
  (MPIG-DATA-001). The registry `minecraft:pig_variant` comes from the server with the
  other synced registries.
- **Behavior:**
  1. A variant entry has the fields `asset_id`, `baby_asset_id` and `model`. `model` is
     `normal` or `cold`. When `model` is absent, it is `normal`.
  2. The texture of an asset id `ns:path` is the file `assets/ns/textures/path.png`.
  3. The built-in entries are:

| Entry | `asset_id` | `baby_asset_id` | `model` |
|-------|------------|-----------------|---------|
| `minecraft:temperate` | `minecraft:entity/pig/pig_temperate` | `minecraft:entity/pig/pig_temperate_baby` | `normal` |
| `minecraft:warm` | `minecraft:entity/pig/pig_warm` | `minecraft:entity/pig/pig_warm_baby` | `normal` |
| `minecraft:cold` | `minecraft:entity/pig/pig_cold` | `minecraft:entity/pig/pig_cold_baby` | `cold` |

- **Postconditions:** none.

Provenance: `black_box_confirmed` (the client jar, `data/minecraft/pig_variant/temperate.json`,
`warm.json` and `cold.json`; the textures under `assets/minecraft/textures/entity/pig/`). Kind:
semantic. Confidence: high. Milestone: 1.

### MPIG-VAR-002: Selection of the geometry and the texture

- **Inputs:** the variant entry and `query.is_baby`.
- **Behavior:**

| Baby | `model` | Geometry | Texture |
|------|---------|----------|---------|
| no | `normal` | `geometry.pig` | the texture of `asset_id` |
| no | `cold` | `geometry.pig.cold` | the texture of `asset_id` |
| yes | any | `geometry.pig.baby` | the texture of `baby_asset_id` |

- **Postconditions:** a warm pig uses `geometry.pig`. A cold baby uses `geometry.pig.baby`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: 1. Experiment:
summon one pig of each variant, as an adult and as a baby, on the vanilla server. Take a
screenshot of each with the vanilla client. Compare the shape of the body and the texture with
the table.

### MPIG-VAR-003: A variant that the client does not know

- **Inputs:** a pig with a variant id that is not in the registry of the client, or a variant
  entry with an asset that has no image.
- **Behavior:**
  1. The client draws the body layer of the pig. It always draws this layer.
  2. The geometry is `geometry.pig`, or `geometry.pig.baby` for a baby.
  3. The texture is the missing texture of the resource pack (`client-entity-definition.md`,
     CENT-RC-007, item 6).
  4. When a later update of the entity data gives a known variant, the client uses the
     geometry and the texture of that variant from the next frame.
- **Postconditions:** the player sees each pig, also with a variant that the client does not
  know.

Provenance: `design` (the reference cannot receive an unknown id). Kind: semantic. Confidence:
medium. Milestone: 1. Experiment: none is possible with the vanilla server, because it sends
only known ids. Use a unit test.

---

## 5. Pose

### MPIG-ANIM-001: The walk

- **Inputs:** `query.modified_distance_moved` and `query.modified_move_speed`.
- **Behavior:** in each frame, the clip `animation.pig.walk` adds these rotations about X, in
  file space, to the legs:

| Bone | Rotation about X |
|------|------------------|
| `right_hind_leg` | the walk swing |
| `left_front_leg` | the walk swing |
| `left_hind_leg` | the negative of the walk swing |
| `right_front_leg` | the negative of the walk swing |

  The clip has `loop: true`, `animation_length: 0` and no keyframe.
- **Postconditions:**
  1. When the pig does not move, `query.modified_move_speed` becomes 0, and the legs return
     to the rest pose.
  2. A baby gets three times the walk position (`molang-queries.md`, MOLQ-WALK-001). Its legs
     swing three times faster for the same distance.

Provenance: `source_suggested_black_box_confirmed`
(`crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_poses.json`: each case of
`minecraft:pig`, `minecraft:cold_pig`, `minecraft:pig_baby` and the layer `saddle` agrees with
the table within `2.37e-5` radians, with the constants `38.1704` and `80.2141`). Kind:
semantic. Confidence: high. Milestone: 1.

### MPIG-ANIM-002: The head

- **Inputs:** `query.target_x_rotation` and `query.target_y_rotation`.
- **Behavior:** in each frame, the clip `animation.pig.look_at_target` adds the rotation
  `(query.target_x_rotation, query.target_y_rotation, 0)` in file space to the bone `head`.
  The clip has `loop: true`, `animation_length: 0` and no keyframe.
- **Postconditions:** a positive pitch turns the snout down. A positive yaw turns the snout to
  the right side of the pig.

Provenance: `source_suggested_black_box_confirmed`
(`crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_poses.json`). Kind: semantic.
Confidence: high. Milestone: 1.

### MPIG-ANIM-003: The play list

- **Inputs:** the client entity `minecraft:pig`.
- **Behavior:**
  1. The `animate` list has `walk`, then `look_at_target`. Each has the weight `1`.
  2. The pig has no other clip. The body has no rotation other than the rest rotation of the
     body in its geometry.
  3. The same pose applies to each geometry of the pig in the frame, by bone name.
- **Postconditions:** a saddle moves with the body, the head and the legs.

Provenance: `source_suggested_black_box_confirmed`
(`crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_poses.json`: the body rotation of
each case is the rest rotation of its layer entry, `(π/2, 0, 0)` for `minecraft:pig`,
`minecraft:cold_pig` and the layer `saddle`, and `(0, 0, 0)` for `minecraft:pig_baby`). Kind:
semantic. Confidence: high. Milestone: 1.

---

## 6. The saddle layer

### MPIG-LAYER-001: The saddle

- **Inputs:** the item in the saddle equipment slot of the pig (`client-entities.md`), its
  equipment asset, and `query.is_baby`.
- **Behavior:**
  1. The client draws the saddle layer after the body layer.
  2. The client draws the saddle layer only when all these conditions are true:
     - the pig is not a baby;
     - the saddle slot holds an item with an equipment asset;
     - the equipment asset has the layer type `pig_saddle`.
  3. The geometry of the layer is `geometry.pig.saddle`. The texture is the texture of the
     first `pig_saddle` layer of the equipment asset. The texture of an equipment layer
     `ns:name` is `assets/ns/textures/entity/equipment/pig_saddle/name.png`.
  4. The layer uses the render type of the body layer and the same hurt colour
     (`client-entity-render.md`).
- **Postconditions:** for the vanilla saddle, the texture is
  `assets/minecraft/textures/entity/equipment/pig_saddle/saddle.png`.

Provenance: `source_suggested` (the client jar, `assets/minecraft/equipment/saddle.json`, for
the asset). Kind: semantic. Confidence: medium. Milestone: 1. Experiment: put a saddle on an
adult pig and on a baby pig on the vanilla server. The vanilla client shows the saddle only on
the adult.

---

## 7. Constants

### MPIG-CONST-001: Constants of the pig

| Constant | Value | Specification that uses it |
|----------|-------|----------------------------|
| width | `0.9` blocks | `client-entity-render.md` (culling box) |
| height | `0.9` blocks | `client-entity-render.md` (culling box, upside-down offset) |
| eye height | `0.765` blocks | `client-entities.md` (eye position), `client-entity-render.md` (light) |
| size of a baby | the adult values × `0.5` | the same |
| update interval | `3` ticks | `client-entities.md` |
| fire immune | no | `client-entities.md` |
| moves in all directions in the air | no | `client-entities.md` (walk accumulator) |
| walk target factor | `4` | `client-entities.md` |
| head limit `L` | `75` degrees | `client-entities.md` (body yaw) |
| death flip angle `F` | `90` degrees | `client-entity-render.md` |
| shadow radius | `0.7` blocks | `client-entity-render.md` |
| shadow strength | `1.0` | `client-entity-render.md` |
| white flash progress | `0` | `client-entity-render.md` |
| model scale between the transform steps | none | `client-entity-render.md` |
| render type of the body layer | cutout, no face culling | `client-entity-render.md` |

Provenance: `black_box_confirmed` for the width, the height, the eye height, the update
interval and the fire immunity (`crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_types.json`,
the type `minecraft:pig`); `source_suggested` for the other values. Kind: semantic.
Confidence: medium. Milestone: 1. Experiment: place a pig on a flat surface in the vanilla
client. Measure the diameter of the shadow in a screenshot from above. It is 1.4 blocks.

---

## 8. Reference content (informative)

This section shows one entity pack content that satisfies the rules above. It is not a rule.
The textures come from registry data through `q.variant_asset` and `q.equipment_texture`
(`molang-queries.md`, MOLQ-STATE-002; `client-entity-definition.md`, CENT-RC-007).

`assets/mcrs/entity/entity/pig.entity.json`:

```json
{
  "format_version": "1.10.0",
  "minecraft:client_entity": {
    "description": {
      "identifier": "minecraft:pig",
      "materials": { "default": "entity_alphatest" },
      "geometry": {
        "default": "geometry.pig",
        "cold": "geometry.pig.cold",
        "baby": "geometry.pig.baby",
        "saddle": "geometry.pig.saddle"
      },
      "animations": {
        "walk": "animation.pig.walk",
        "look_at_target": "animation.pig.look_at_target"
      },
      "scripts": { "animate": ["walk", "look_at_target"] },
      "render_controllers": [
        "controller.render.pig",
        { "controller.render.pig_saddle": "q.equipment_texture('saddle', 'pig_saddle') != '' && !q.is_baby" }
      ]
    }
  }
}
```

`assets/mcrs/entity/render_controllers/pig.render_controllers.json`:

```json
{
  "format_version": "1.8.0",
  "render_controllers": {
    "controller.render.pig": {
      "geometry": "q.is_baby ? Geometry.baby : (q.variant_model == 'cold' ? Geometry.cold : Geometry.default)",
      "materials": [{ "*": "Material.default" }],
      "textures": [
        "q.variant_asset != '' ? q.variant_asset : 'mcrs:missing'"
      ]
    },
    "controller.render.pig_saddle": {
      "geometry": "Geometry.saddle",
      "materials": [{ "*": "Material.default" }],
      "textures": ["q.equipment_texture('saddle', 'pig_saddle')"]
    }
  }
}
```

`assets/mcrs/entity/animations/pig.animation.json`:

```json
{
  "format_version": "1.8.0",
  "animations": {
    "animation.pig.walk": {
      "loop": true,
      "bones": {
        "right_hind_leg": { "rotation": ["math.cos(q.modified_distance_moved * 38.1704) * 80.2141 * q.modified_move_speed", 0, 0] },
        "left_front_leg": { "rotation": ["math.cos(q.modified_distance_moved * 38.1704) * 80.2141 * q.modified_move_speed", 0, 0] },
        "left_hind_leg": { "rotation": ["-math.cos(q.modified_distance_moved * 38.1704) * 80.2141 * q.modified_move_speed", 0, 0] },
        "right_front_leg": { "rotation": ["-math.cos(q.modified_distance_moved * 38.1704) * 80.2141 * q.modified_move_speed", 0, 0] }
      }
    },
    "animation.pig.look_at_target": {
      "loop": true,
      "bones": {
        "head": { "rotation": ["q.target_x_rotation", "q.target_y_rotation", 0] }
      }
    }
  }
}
```

The identifier `mcrs:missing` has no image. It selects the missing texture for a variant that
the client does not know (MPIG-VAR-003).

---

## 9. Intentional deviations

### MPIG-DEV-001: Exact trigonometry

- **Rule:** MPIG-ANIM-001.
- **Reference:** the cosine of the reference differs from the exact cosine. In the pose
  artefact, the leg rotations of the reference differ from the exact walk swing by at most
  `2.37e-5` radians.
- **Visible to the client:** no.
- **Test:** the tolerance of MPIG-ART-003 covers the difference.

### MPIG-DEV-002: The time of a clip in whole milliseconds

- **Rule:** `entity-animation.md`, EANIM-TIME-001.
- **Reference:** the pose of a keyframe clip in the reference is the pose at the clip time
  truncated to a whole millisecond.
- **Visible to the client:** no. The difference is less than `0.001` s.
- **Test:** MPIG-ART-003, item 6, gives the truncated time to the client. Without the
  truncation, a fast channel differs by up to `0.007` in the frog cases.

---

## 10. Open questions

1. The pose artefact has no case for the head of a riding pig or for the names that turn an
   entity upside down. `client-entity-render.md` states these.
2. No oracle artefact records the client state of a pig tick by tick: the interpolation, the
   body yaw and the walk accumulator of `client-entities.md`. The oracle has no client world
   without a running game client. The experiments of `client-entities.md` stay proposals.
3. The clip identifiers of the clip artefact come from the dump task. The names are labels of
   this repository, for example `frog/croak`. A rename in a later game version changes the
   identifier of a clip.
