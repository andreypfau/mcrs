# The frog of the client: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| Entity type | `minecraft:frog` |

Source of the game values: `assets/minecraft/version.json`.

This document states what the client draws for a frog: the geometry, the texture of each
variant, the keyframe clips, the conditions that start and stop each clip, and the visibility
of the croak sac. The rules for the oracle artefacts are in `mob-pig.md`, section 1. The
document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Entity pack**, **file space**, **bone**, **content error**: as `entity-geometry.md`,
  section 0.1, defines them.
- **Clip**, **clip instance**, **channel**, **keyframe**, **play list**, **weight**: as
  `entity-animation.md`, section 0.1, defines them.
- **Client entity**, **render controller**: as `client-entity-definition.md`, section 0.1,
  defines them.
- **Model artefact**, **clip artefact**, **pose artefact**, **type artefact**, **layer entry**,
  **artefact space**: as `mob-pig.md`, section 0.1, defines them.
- **Pose of the entity**: the value of `query.pose` (`molang-queries.md`, MOLQ-STATE-001).
  This is the pose in the entity data, not the pose of the bones.
- **Pose clip**: one of the clips `jump`, `croak` and `tongue`. A pose of the entity starts and
  stops each pose clip.
- **Croak sac**: the bone `croaking_body`.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". The label `design` of `entity-geometry.md`, section 0.2,
applies.

Each rule has a line **Milestone**, with the values of `entity-geometry.md`, section 0.2. The
frog is milestone 2.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `MFROG-DATA` | the entity data of the frog |
| `MFROG-GEO` | the geometry of the frog |
| `MFROG-VAR` | variants and textures |
| `MFROG-CLIP` | the clips and their settings |
| `MFROG-ANIM` | the play list and the conditions of each clip |
| `MFROG-VIS` | the visibility of bones |
| `MFROG-CONST` | constants of the frog that other specifications use |
| `MFROG-DEV` | intentional deviations |

### 0.4 Scope

In scope: the geometry, the textures, the clips, the conditions of the clips, the walk target
of the frog and the croak sac.

Out of scope: the tadpole (another entity type), sounds and particles. The client draws no
line to the tongue target: the tongue is only the clip `tongue` (MFROG-CLIP-001), which
stretches the tongue cube to at most 5 times its length along the view of the frog,
whatever the distance to the target. The client reads the tongue target (index 19) for no
visible effect. The placement transform, the hurt colour, the death rotation, the light
and the shadow shape are in `client-entity-render.md`.

---

## 1. Entity data

### MFROG-DATA-001: Indices of the frog

- **Inputs:** the entity data of a frog (`client-entities.md`, CENTS-DATA-001).
- **Behavior:** the indices 0 to 17 are those of `client-entities.md`, CENTS-DATA-002. The frog
  also has these indices:

| Index | Wire type | Default | Meaning | Read by the client |
|-------|-----------|---------|---------|--------------------|
| 18 | `FrogVariant` (type id 27) | the id of `minecraft:temperate` | variant | yes |
| 19 | `OptionalUnsignedInt` (type id 19) | absent | network id of the tongue target | no (no visible effect) |

  1. A value of index 18 is the numeric id of an entry of the registry
     `minecraft:frog_variant`, with the rules of `mob-pig.md`, MPIG-DATA-001, items 1 to 3.
  2. `query.variant`, `query.variant_identifier`, `query.variant_asset` and
     `query.variant_model` read the entry of index 18.
  3. The pose of the entity is index 6 (`client-entities.md`, CENTS-DATA-002).
- **Postconditions:** a frog with no value at index 18 is a temperate frog.

Provenance: `black_box_confirmed` for the indices and the wire types
(`crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_types.json`, the type
`minecraft:frog`); `source_suggested` for the defaults. Kind: wire for the indices and the
types; semantic for the defaults. Confidence: medium. Milestone: 2. Experiment: summon a frog
with the variant `minecraft:warm` on the vanilla server. Capture the set entity data packets.
Entry 18 has the id of `minecraft:warm`.

---

## 2. Geometry

### MFROG-GEO-001: The geometry

- **Inputs:** the layer entry `minecraft:frog` / `main` of the model artefact.
- **Behavior:**
  1. The entity pack has the geometry `geometry.frog` in the file
     `assets/mcrs/entity/geometry/frog.json`, with the format version `1.12.0`. It is the
     conversion of the layer entry with `mob-pig.md`, MPIG-ART-001.
  2. The texture size is 48 × 48.
- **Postconditions:**
  1. The bones and their parents are:

| Bone | Parent |
|------|--------|
| `root` | none |
| `body` | `root` |
| `croaking_body` | `body` |
| `head` | `body` |
| `eyes` | `head` |
| `left_eye`, `right_eye` | `eyes` |
| `left_arm`, `right_arm` | `body` |
| `left_hand` | `left_arm` |
| `right_hand` | `right_arm` |
| `tongue` | `body` |
| `left_leg`, `right_leg` | `root` |
| `left_foot` | `left_leg` |
| `right_foot` | `right_leg` |

  2. The bone `root` has the file pivot `(0, 0, 0)`.
  3. The feet, the hands, the tongue and one cube of the body and of the head have a size of 0
     on the Y axis. Each such cube has six faces (`entity-geometry.md`, EGEO-CUBE-002).
  4. The cube of the croak sac has an inflate of `−0.1`.

Provenance: `black_box_confirmed`
(`crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_models.json`). Kind: semantic.
Confidence: high. Milestone: 2.

---

## 3. Variants

### MFROG-VAR-001: The texture of a variant

- **Inputs:** the entry of the registry `minecraft:frog_variant` that the entity data of the
  frog gives (`client-entities.md`).
- **Behavior:**
  1. The texture is the texture of the field `asset_id` of the entry, with the rule of
     `mob-pig.md`, MPIG-VAR-001, item 2.
  2. The geometry is `geometry.frog` for each variant. A frog has no baby form.
  3. The built-in entries are:

| Entry | `asset_id` |
|-------|------------|
| `minecraft:temperate` | `minecraft:entity/frog/frog_temperate` |
| `minecraft:warm` | `minecraft:entity/frog/frog_warm` |
| `minecraft:cold` | `minecraft:entity/frog/frog_cold` |

  4. When the client does not know the entry, or the asset has no image, the client draws the
     frog with the missing texture of the resource pack (`client-entity-definition.md`,
     CENT-RC-007, item 6). The client always draws this layer.
- **Postconditions:** none.

Provenance: `black_box_confirmed` for 3 (the client jar, `data/minecraft/frog_variant/`, and
`assets/minecraft/textures/entity/frog/`); `design` for 4. Kind: semantic.
Confidence: medium. Milestone: 2. Experiment for 4: none is possible with the vanilla server.
Use a unit test.

---

## 4. Clips

### MFROG-CLIP-001: The six clips

- **Inputs:** the clip artefact.
- **Behavior:** the entity pack has six clips of the frog. Each is the conversion of one
  element of the clip artefact with `mob-pig.md`, MPIG-ART-002. The settings are:

| Clip identifier | Artefact `id` | `animation_length` | `loop` | `anim_time_update` |
|-----------------|---------------|--------------------|--------|--------------------|
| `animation.frog.jump` | `frog/jump` | `0.5` | `"hold_on_last_frame"` | default |
| `animation.frog.croak` | `frog/croak` | `3.0` | `"hold_on_last_frame"` | default |
| `animation.frog.tongue` | `frog/tongue` | `0.5` | `"hold_on_last_frame"` | default |
| `animation.frog.walk` | `frog/walk` | `1.25` | `true` | `query.modified_distance_moved * 0.075` |
| `animation.frog.swim` | `frog/swim` | `1.04167` | `true` | `query.modified_distance_moved * 0.05` |
| `animation.frog.idle_water` | `frog/idle_water` | `3.0` | `true` | default |

- **Postconditions:**
  1. Each channel of `jump`, `croak`, `tongue` and `walk` uses linear interpolation. Each
     channel of `swim` and `idle_water` uses Catmull-Rom.
  2. No keyframe of the six clips has a `pre` that is different from its `post`.
  3. A pose clip holds its last pose after its length, until its condition stops it
     (MFROG-ANIM-001).
  4. The time of `walk` is `0.075` s for each walk unit, and the time of `swim` is `0.05` s.
  5. The conversion drops the `scale` channel of the bone `head` from `animation.frog.tongue`.
     The head keeps the scale 1 (MFROG-DEV-001).

Provenance: `source_suggested_black_box_confirmed`
(`crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_animations.json`; the cases of
`minecraft:frog` in `entity_poses.json`: a sample of the clips with these settings agrees with
each frog case within `5.3e-7`. The sample uses the times of `mob-pig.md`, MPIG-ART-003,
item 6, the clip weights of MPIG-ART-003, item 7, and the weights of section 5 for `walk` and
`swim`. It does not compare the head scale of MFROG-DEV-001). Kind: semantic. Confidence:
high. Milestone: 2.

---

## 5. Play list and conditions

### MFROG-ANIM-001: The pose clips

- **Inputs:** the pose of the entity.
- **Behavior:**

| Clip | Weight |
|------|--------|
| `jump` | `query.pose == 'long_jumping'` |
| `croak` | `query.pose == 'croaking'` |
| `tongue` | `query.pose == 'using_tongue'` |

  1. A pose clip starts with the time 0 when its pose of the entity begins
     (`entity-animation.md`, EANIM-TIME-004).
  2. A pose clip stops when the pose of the entity changes to another value.
  3. When the pose of the entity changes from one pose clip to another, the first clip stops
     and the second clip starts in the same frame.
- **Postconditions:** at most one pose clip plays at a time.

Provenance: `source_suggested_black_box_confirmed` (the cases with a start tick for `jump`,
`croak` or `tongue` in `crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_poses.json`
give the pose for each time after the start). Kind: semantic. Confidence: medium. Milestone: 2.
Experiment: put a frog and a small slime in a closed room on the vanilla server. Record the
entity data packets of the frog and a video of the vanilla client. Each tongue movement starts
in the tick in which the pose `using_tongue` arrives.

### MFROG-ANIM-002: The walk and the swim

- **Inputs:** `query.is_in_water`, `query.modified_distance_moved` and
  `query.modified_move_speed`.
- **Behavior:**

| Clip | Weight |
|------|--------|
| `walk` | `query.is_in_water ? 0 : math.min(query.modified_move_speed * 2.5, 1)` |
| `swim` | `query.is_in_water ? math.min(query.modified_move_speed * 2.5, 1) : 0` |

  1. The time of each clip is a function of the walk position (MFROG-CLIP-001). It does not
     depend on the time of a previous frame.
  2. The weight multiplies each channel value of the clip (`entity-animation.md`,
     EANIM-BLEND-002).
- **Postconditions:** a frog that stands still has no walk pose and no swim pose. A frog at
  the walk speed `0.4` or more has the full walk pose.

Provenance: `source_suggested_black_box_confirmed` (the cases with `walk_speed` greater than 0
in `crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_poses.json`). Kind: semantic.
Confidence: medium. Milestone: 2.

### MFROG-ANIM-003: The idle swim

- **Inputs:** `query.is_in_water` and `query.modified_move_speed`.
- **Behavior:**
  1. The weight of `idle_water` is
     `query.is_in_water && query.modified_move_speed <= 0.00001`.
  2. The clip starts with the time 0 when the weight becomes 1. It stops when the weight
     becomes 0.
- **Postconditions:** a frog that stops in the water starts the idle swim from its start.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: 2. Experiment:
put a frog in still water on the vanilla server. Push it, and let it stop. The idle swim starts
approximately 23 ticks after the push, when the walk speed is below `1e-5`.

### MFROG-ANIM-004: The play list

- **Inputs:** the client entity `minecraft:frog`.
- **Behavior:**
  1. The `animate` list has `jump`, `croak`, `tongue`, `walk`, `swim`, then `idle_water`, with
     the weights of MFROG-ANIM-001 to MFROG-ANIM-003.
  2. The frog has no clip that turns the head to a target. `query.target_x_rotation` and
     `query.target_y_rotation` have no effect on the pose.
  3. The offsets of all clip instances add (`entity-animation.md`, EANIM-BLEND-001).
- **Postconditions:** a frog that croaks while it walks has the croak sac pose and the walk
  pose together.

Provenance: `source_suggested_black_box_confirmed` for 1 and 3 (the cases with a croak start
and a walk speed of `0.6` in `crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_poses.json`);
`source_suggested` for 2. Kind: semantic. Confidence: medium. Milestone: 2. Experiment for 2:
stand in front of a frog on the vanilla server and walk around it. The head of the frog does
not turn to the player.

---

## 6. Visibility

### MFROG-VIS-001: The croak sac

- **Inputs:** the pose of the entity.
- **Behavior:** the croak sac is visible only when `query.pose == 'croaking'`. Its child bones
  follow it (`entity-geometry.md`, EGEO-POSE-002).
- **Postconditions:**
  1. The croak sac stays visible after the croak clip ends, until the pose of the entity
     changes.
  2. In the first part of the croak clip, the croak sac has the scale 0 and is not seen.

Provenance: `source_suggested_black_box_confirmed` (the field `visible` of
`root/root/body/croaking_body` in the frog cases of
`crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_poses.json`). Kind: semantic.
Confidence: high. Milestone: 2.

---

## 7. Constants

### MFROG-CONST-001: Constants of the frog

| Constant | Value | Specification that uses it |
|----------|-------|----------------------------|
| width | `0.5` blocks | `client-entity-render.md` (culling box) |
| height | `0.5` blocks | `client-entity-render.md` (culling box, upside-down offset) |
| eye height | `0.425` blocks | `client-entities.md` (eye position), `client-entity-render.md` (light) |
| update interval | `3` ticks | `client-entities.md` |
| fire immune | no | `client-entities.md` |
| moves in all directions in the air | no | `client-entities.md` (walk accumulator) |
| head limit `L` | `5` degrees | `client-entities.md` (body yaw) |
| death flip angle `F` | `90` degrees | `client-entity-render.md` |
| shadow radius | `0.3` blocks | `client-entity-render.md` |
| shadow strength | `1.0` | `client-entity-render.md` |
| white flash progress | `0` | `client-entity-render.md` |
| model scale between the transform steps | none | `client-entity-render.md` |
| walk target factor | `25` | `client-entities.md` |
| walk target while the pose of the entity is `long_jumping` | `0` | `client-entities.md` |
| render type | cutout, no face culling | `client-entity-render.md` |

The walk target factor replaces the factor of the default walk rule of `client-entities.md`:
the target walk speed is `min(25 · d, 1)`, where `d` is the distance of the latest tick.

Provenance: `black_box_confirmed` for the width, the height, the eye height, the update
interval and the fire immunity (`crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_types.json`,
the type `minecraft:frog`); `source_suggested` for the other values. Kind: semantic.
Confidence: medium. Milestone: 2. Experiment: let a frog walk on a flat surface on the vanilla server. Record its positions for each tick.
Compute the walk speed with the factor `25`. The legs of the vanilla client move with the
full walk pose after the second tick of the walk.

---

## 8. Reference content (informative)

This section shows one entity pack content that satisfies the rules above. It is not a rule.
A converter writes the `bones` of the six clips from the clip artefact (MFROG-CLIP-001). The
texture comes from registry data through `q.variant_asset` (`molang-queries.md`,
MOLQ-STATE-002; `client-entity-definition.md`, CENT-RC-007).

`assets/mcrs/entity/entity/frog.entity.json`:

```json
{
  "format_version": "1.10.0",
  "minecraft:client_entity": {
    "description": {
      "identifier": "minecraft:frog",
      "materials": { "default": "entity_alphatest" },
      "geometry": { "default": "geometry.frog" },
      "animations": {
        "jump": "animation.frog.jump",
        "croak": "animation.frog.croak",
        "tongue": "animation.frog.tongue",
        "walk": "animation.frog.walk",
        "swim": "animation.frog.swim",
        "idle_water": "animation.frog.idle_water"
      },
      "scripts": {
        "animate": [
          { "jump": "q.pose == 'long_jumping'" },
          { "croak": "q.pose == 'croaking'" },
          { "tongue": "q.pose == 'using_tongue'" },
          { "walk": "q.is_in_water ? 0 : math.min(q.modified_move_speed * 2.5, 1)" },
          { "swim": "q.is_in_water ? math.min(q.modified_move_speed * 2.5, 1) : 0" },
          { "idle_water": "q.is_in_water && q.modified_move_speed <= 0.00001" }
        ]
      },
      "render_controllers": ["controller.render.frog"]
    }
  }
}
```

`assets/mcrs/entity/render_controllers/frog.render_controllers.json`:

```json
{
  "format_version": "1.8.0",
  "render_controllers": {
    "controller.render.frog": {
      "geometry": "Geometry.default",
      "materials": [{ "*": "Material.default" }],
      "textures": [
        "q.variant_asset != '' ? q.variant_asset : 'mcrs:missing'"
      ],
      "part_visibility": [{ "croaking_body": "q.pose == 'croaking'" }]
    }
  }
}
```

The settings of one clip in `assets/mcrs/entity/animations/frog.animation.json`:

```json
{
  "animation.frog.walk": {
    "loop": true,
    "animation_length": 1.25,
    "anim_time_update": "q.modified_distance_moved * 0.075",
    "bones": { "...": "from the clip artefact" }
  }
}
```

---

## 9. Intentional deviations

### MFROG-DEV-001: The head scale of the tongue clip

- **Rule:** MFROG-CLIP-001 (the clip `tongue`).
- **Reference:** the `scale` channel of the bone `head` in `frog/tongue` adds approximately
  `0.01745` to the head scale. The value in the clip artefact is `0.017453292`, which is one
  degree in radians. With MPIG-ART-002, item 7, the head scale is `1.01745` for the length of
  the clip.
- **Visible to the client:** yes, but hardly. The head is 1.7% larger for 0.5 s while the frog
  uses its tongue.
- **Test:** a test that compares with `entity_poses.json` skips the scale of
  `root/root/body/head` in the cases with a start tick for `tongue`.

### MFROG-DEV-002: The start of a pose clip

- **Rule:** MFROG-ANIM-001.
- **Reference:** the reference starts a pose clip at the start of the tick in which the pose
  arrives. The first frame after the start can show the clip at a time of up to one tick.
- **Visible to the client:** no. The difference is at most `0.05` s.
- **Test:** a test that compares a sequence of frames with the reference accepts a time offset
  of one tick for each pose clip.

### MFROG-DEV-003: Rules of the keyframe sample

- **Rule:** `entity-animation.md`, EANIM-KEY-004, EANIM-KEY-005 and EANIM-DEV-001.
- **Reference:** the pose of the reference can differ from the rules of `entity-animation.md`
  in three cases: a channel that uses two interpolations, a Catmull-Rom segment next to a
  keyframe with a `pre` that is different from its `post`, and two clips with a `scale`
  channel on one bone.
- **Visible to the client:** no, for the frog. No frog clip has one of these cases
  (MFROG-CLIP-001, postconditions 1 and 2; no two frog clips scale one bone). The frog cases
  of the pose artefact agree within `5.3e-7` (MFROG-CLIP-001).
- **Test:** no test skips a case for this entry.

---

## 10. Open questions

1. `query.modified_move_speed` of `molang-queries.md`, MOLQ-WALK-001, is the walk speed with the
   partial tick. The reference tests the idle swim condition with the walk speed of the latest
   tick. The idle swim can start one frame earlier than in the reference. A query with the
   value of the latest tick removes this difference.
