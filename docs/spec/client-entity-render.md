# Rendering of a living entity by the client: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`. The rules were read against the
behaviour of `26.4-snapshot-7`. No difference between the two versions is known for this domain.

This document states how the client places, shades and tints a living entity in the world. It
uses the values of one frame from `client-entities.md` (CENTS-FRAME-001) and the bone
transforms from `entity-geometry.md` and `entity-animation.md`. The mob documents give the
geometry, the textures and the constants of each type. The document does not prescribe an
architecture.

## 0. Conventions

### 0.1 Terms

- **Internal space**: as `entity-geometry.md`, section 0.1, defines it. +Y is up, the front of
  the entity points to −Z, the right side points to +X, and the unit is the model unit
  (1/16 block).
- **World space**: the space of block coordinates. +Y is up. The unit is one block.
- **Frame values**: the values of CENTS-FRAME-001 for the current frame.
- **Entity transform**: the transform from internal space to world space for one entity in one
  frame (CEREN-PLACE-002).
- **Texel colour**: the colour that the entity texture gives at a point of a face, before tint,
  overlay and light.
- **Entity box** and **eye height**: as `client-entities.md`, section 0.1, defines them.
- **Light probe**: the block that contains the eye position of the frame values. The eye
  position uses the eye height.
- **Model tint**: an ARGB colour that multiplies the texel colour. It is `0xFFFFFFFF` when no
  rule gives another value.
- `Rx(a)`, `Ry(a)`, `Rz(a)`: a rotation by `a` degrees about the X, Y or Z axis, by the
  right-hand rule.

### 0.2 Provenance and confidence

Each rule has a line `Milestone`: `1` is the pig, `2` is the frog, and `later` is a rule that
milestones 1 and 2 do not use.

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". No artefact of the game confirms a rule of this document
yet. Each rule therefore has the label `source_suggested` and an experiment.

A parity test of this document compares screenshots with a tolerance. A position can differ by
at most 0.5 pixel at the test resolution. A colour channel can differ by at most 2 of 255.

### 0.3 Colours

A colour is a 32-bit ARGB value in hexadecimal. A channel value `c` of 0 to 255 has the
normalised value `c / 255`.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CEREN-MATH` | numeric rules |
| `CEREN-PLACE` | the entity transform |
| `CEREN-SURF` | texture, tint, overlay and shading |
| `CEREN-VIS` | visibility and culling |
| `CEREN-LAYER` | layers |
| `CEREN-SHADOW` | the shadow |
| `CEREN-NAME` | the name tag |
| `CEREN-DEV` | intentional deviations |

### 0.5 Scope

In scope: living entities that are not the local player, in the default view of the world.

Out of scope: sleeping entities, riding, the spin attack, the leash, the fire overlay, the
outline of a glowing entity, items in hands, armour, and entities in a GUI. The shadow
(section 6) and the name tag (section 7) are in scope, but a later milestone implements them.

---

## 1. Numbers

### CEREN-MATH-001: Trigonometry

- **Inputs:** an angle in a formula that the client computes on the CPU and that is not a
  Molang expression, for example the frozen shake of CEREN-PLACE-004.
- **Behavior:** the client computes sine and cosine with a precision of at least `1e-6`.
- **Postconditions:** a Molang expression uses the precision of `molang.md`, MOL-NUM-004, not
  this rule.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Deviation: CEREN-DEV-001.
Experiment: none. The difference is less than one pixel.
Milestone: 1.

---

## 2. Placement

### CEREN-PLACE-001: Position in the frame

- **Inputs:** the frame position `P` of the entity; the camera position `E`.
- **Behavior:** the client places the origin of internal space at `P − E`, in camera-relative
  world space. It computes `P − E` in 64-bit floats before it converts the result to 32-bit
  floats.
- **Postconditions:** an entity far from the world origin does not shake.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: go to
X = 1 000 000. Look at a pig that walks. The pig moves smoothly.
Milestone: 1.

### CEREN-PLACE-002: The entity transform

- **Inputs:** the frame values: the body yaw `b`, the frame death time `D`, the scale `s`
  (CENTS-DATA-003); the script scale `(sx, sy, sz)` of `client-entity-definition.md`,
  CENT-SCR-004, which is `(1, 1, 1)` when the client entity has no scale expression; the flags
  of CEREN-PLACE-004 and CEREN-PLACE-005.
- **Behavior:** a point `q` of internal space goes to world space through these operations.
  The first operation applies first to `q`:
  1. Move `q` by `(0, 0.016, 0)`. This puts the model 0.001 block above the entity position.
  2. Scale by 1/16, so that model units become blocks.
  3. Scale by `(sx, sy, sz)`, about the entity origin.
  4. Apply the pose rotation `M`:
     - when `D` > 0, `M = Rz(death tilt)` (CEREN-PLACE-003);
     - otherwise, when the entity is upside down, `M` = move by `(0, (h + 0.1) / s, 0)` after
       `Rz(180)`, where `h` is the height of the entity box (CEREN-PLACE-005);
     - otherwise, `M` is the identity.
  5. Rotate by `Ry(180 − b')`, where `b'` is `b` plus the shake of CEREN-PLACE-004.
  6. Scale by `s`.
  7. Move by `P − E` (CEREN-PLACE-001).
- **Postconditions:** with `b` = 0, the front of the entity points to +Z of world space. The
  right side of the entity points to −X of world space. The baby state does not scale the
  model in this rule. A mob document selects a baby geometry, or a baby scale through the
  script scale of step 3.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: summon a pig
with `Rotation:[0f,0f]` and the tag `NoAI`. Its snout points south (+Z). Summon it with yaw 90.
Its snout points west (−X).
Milestone: 1.

### CEREN-PLACE-003: Death tilt

- **Inputs:** the frame death time `D` > 0; the flip angle `F`. `F` is 90 degrees. A mob document
  can give another `F`.
- **Behavior:**
  1. Let `f = sqrt((D − 1) / 20 · 1.6)`. When `(D − 1)` < 0, `f` is 0.
  2. When `f` > 1, `f` becomes 1.
  3. The death tilt is `f · F` degrees, about the Z axis of internal space.
- **Postconditions:** a positive tilt turns the top of the entity toward −X of internal space.
  That is the left side of the entity. The tilt reaches `F` when `D` is about 13.5.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: kill a pig that
faces south. Seen from the north, behind the pig, the pig falls to its left (east, +X of
world space).
Milestone: 1.

### CEREN-PLACE-004: Shake of a frozen entity

- **Inputs:** the fully frozen flag (CENTS-DATA-003); the frame age `A` in ticks.
- **Behavior:** when the entity is fully frozen, the client adds
  `cos(floor(A) · 3.25) · π · 0.4` degrees to the body yaw of CEREN-PLACE-002, step 5. The
  argument of `cos` is in radians.
- **Postconditions:** the shake changes one time in each tick, by at most 1.257 degrees.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: put a pig in
powder snow for 7 seconds. The pig shakes left and right.
Milestone: later.

### CEREN-PLACE-005: Upside-down names

- **Inputs:** the custom name of the entity (CENTS-DATA-002, index 2).
- **Behavior:**
  1. The entity is upside down when the plain text of its custom name is `Dinnerbone` or
     `Grumm`.
  2. An upside-down entity has the pose rotation of CEREN-PLACE-002, step 4, when `D` is 0.
  3. An upside-down entity also negates its relative head yaw and its pitch in the frame values.
- **Postconditions:** the queries `target_y_rotation`, `target_x_rotation` and
  `head_x_rotation` read the negated values.

Provenance: `documentation`; `source_suggested` for the offset and the negation. Kind: semantic.
Confidence: medium. Experiment: name a pig `Dinnerbone`. The pig is upside down with its feet
at 0.1 block above the top of its entity box. It looks up when the player stands above it.
Milestone: later.

---

## 3. Surface

### CEREN-SURF-001: Cutout and faces

- **Inputs:** a face of the baked geometry; the texel colour `t` at each point.
- **Behavior:**
  1. The client discards a point when the alpha of `t` is less than 0.1. The model tint does
     not change this test.
  2. The client draws the front and the back of each face. It does not cull back faces.
  3. A point that the client does not discard is opaque. It writes depth.
  4. A cube with a size of 0 on one axis has two faces in the same plane. The four other faces
     have no area and show nothing. Where both faces of the pair cover a point, and the texel
     of neither face is discarded, the point shows one face of the pair, from both sides:
     - for a size of 0 on Y, the face with the normal −Y of internal space;
     - for a size of 0 on X, the face with the normal −X, or the face with the normal +X when
       the cube has the mirror flag;
     - for a size of 0 on Z, the face with the normal +Z.
  5. Where the texel of the selected face is discarded, the point shows the other face of the
     pair, when its texel is not discarded.
  6. The point uses the texel colour of the face that it shows. Its face factor
     (CEREN-SURF-002) uses the side of that face that the camera sees.
- **Postconditions:** a face of zero thickness shows from both sides, with the texture of one
  face of the pair.

Provenance: `source_suggested`. Kind: semantic. Confidence: low. Experiment: look at the feet of
a frog from above and from below. Compare each foot with the two texture regions of the foot
cube. From both sides, the foot shows the region of the face that item 4 selects.
Milestone: 1.

### CEREN-SURF-002: Shading by the face direction

- **Inputs:** the normal `n` of a face in world space, after the entity transform; the side of
  the face that the camera sees; the light directions of the dimension.
- **Behavior:**
  1. The light directions are `L0 = normalize(0.2, 1, −0.7)` and
     `L1 = normalize(−0.2, 1, 0.7)`, in world space.
  2. For a dimension type with `"cardinal_light": "nether"`, `L1` is
     `normalize(−0.2, −1, 0.7)`. `L0` does not change.
  3. Let `m = n` when the camera sees the front of the face. Let `m = −n` when it sees the back.
  4. The face factor is `min(1, (max(0, m · L0) + max(0, m · L1)) · 0.6 + 0.4)`.
  5. The face factor multiplies the red, green and blue channels.
- **Postconditions:** the factor does not depend on the camera direction. A face that points up
  in the overworld has a factor of 1.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: take a
screenshot of a pig at full light. Compare the colour of the top and the side of the body with
the texture. The ratios match the factor.
Milestone: 1.

### CEREN-SURF-003: Light of an entity

- **Inputs:** the light probe; the block light and the sky light of the client world; the
  on-fire flag (CENTS-DATA-003).
- **Behavior:**
  1. The block light of the entity is the block light level at the light probe. When the
     entity is on fire, it is 15.
  2. The sky light of the entity is the sky light level at the light probe.
  3. Each point of the entity uses these two levels. The client applies the same lightmap to
     these levels as to terrain.
  4. The coloured light tint of the client world multiplies the colour of each point. The client
     reads the tint at the position of that point, as for terrain (CEREN-DEV-002).
  5. The client applies the same fog to the entity as to terrain at the same distance.
- **Postconditions:** the light levels do not change along the entity. They change when the eye
  position enters another block. The coloured tint can change along the entity.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: put a pig with
its head in a lit block and its feet in a dark block. The pig has the light of its head block.
Milestone: 1.

### CEREN-SURF-004: Red overlay

- **Inputs:** the colour after the model tint and the face factor; the hurt time and the
  frame death time (CENTS-FRAME-001).
- **Behavior:**
  1. The red overlay is on when the hurt time is more than 0 or the frame death time is more
     than 0.
  2. Let `c` be the colour after the model tint and the face factor (CEREN-SURF-006, step 4).
     When the red overlay is on, the red, green and blue channels become
     `mix(O, c, 178 / 255)`, where `O = (1, 0, 0)`. That is `c · 178/255 + O · 77/255`.
  3. The light (CEREN-SURF-003) and the fog apply after the overlay.
- **Postconditions:** each layer that uses the overlay (CEREN-LAYER-001) gets the same red
  overlay.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: hit a sheep in
full light. Measure a texel of the wool on a face that points up. For 10 ticks, the red channel
is `c · 178/255 + 77` and the green and blue channels are `c · 178/255`, within the tolerance.
Milestone: 1.

### CEREN-SURF-005: White flash

- **Inputs:** the colour `c` after the model tint and the face factor; a white progress `w` in
  `[0, 1]` that a mob document gives; the state of the red overlay.
- **Behavior:**
  1. When the red overlay is on, the red overlay applies and the white flash does not.
  2. Otherwise, let `u = floor(w · 15)` and `α = floor((1 − u / 15 · 0.75) · 255) / 255`.
  3. The red, green and blue channels become `mix((1, 1, 1), c, α)`.
- **Postconditions:** with `w` = 0, the colour does not change. With `w` = 1, the colour
  is 75% white, within the tolerance.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: ignite a creeper
with flint and steel. The creeper flashes white. Compare the brightest colour with the formula.
Milestone: 1.

### CEREN-SURF-006: Order of the colour operations

- **Inputs:** the texel colour; the results of CEREN-SURF-002 to CEREN-SURF-005.
- **Behavior:** the client computes the colour of a point in this order:
  1. the texel colour;
  2. the cutout test (CEREN-SURF-001);
  3. the product with the model tint;
  4. the product with the face factor (CEREN-SURF-002);
  5. the red overlay or the white flash;
  6. the product with the lightmap colour of CEREN-SURF-003;
  7. the fog.
- **Postconditions:** the red part of the overlay does not depend on the face factor. The light
  darkens the red part. This rule states the colour of the result. An implementation can
  compute the steps in different passes, and in a different place, when the colour is the
  same.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: hit a pig in a
dark cave. The red tint is dark, not bright. In full light, the red part is equal on the top
and on the side of the body.
Milestone: 1.

---

## 4. Visibility

### CEREN-VIS-001: Invisible entities

- **Inputs:** the invisible flag (CENTS-DATA-003); the game mode of the local player; the team
  of the entity and of the local player.
- **Behavior:**
  1. The entity is hidden from the local player when it is invisible, the local player is not a
     spectator, and no team rule shows it. A team rule shows it when both are in the same team
     and that team can see friendly invisible members.
  2. A visible entity draws with the rules of section 3.
  3. An invisible entity that is not hidden from the local player draws translucent. The model
     tint is `0x26FFFFFF`. The client culls back faces. The colour blends over the colour
     behind it, with the alpha of the point.
  4. An entity that is hidden draws nothing of its model. Its layers can draw
     (CEREN-LAYER-001).
  5. CEREN-VIS-003 applies also to a translucent entity. A small translucent entity can draw
     as an impostor with the model tint of item 3.
- **Postconditions:** a spectator sees an invisible pig as a faint ghost.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: give a pig the
invisibility effect. In survival mode the pig disappears. In spectator mode it shows as a ghost.
Milestone: later.

### CEREN-VIS-002: Culling box

- **Inputs:** the entity box at the frame position; the visible bounds of each geometry that
  the entity draws (`entity-geometry.md`, EGEO-BAKE-002); the entity transform (CEREN-PLACE-002); the
  view frustum.
- **Behavior:**
  1. The grown box is the entity box, made larger by 0.5 block on each side.
  2. When the entity box has a NaN value or a size of 0, the grown box is a cube of 4 blocks
     with its centre at the entity position.
  3. The bounds box is the box of the visible bounds, after the full entity transform: the
     script scale, the death tilt, the upside-down rotation, the yaw and the scale.
  4. The culling volume is the union of the grown box and the bounds box of each geometry that
     the entity draws (`client-entity-definition.md`, CENT-RC-001).
  5. The client can skip the entity only when the culling volume is fully outside the frustum,
     or it proves by other means that no point of the entity is visible.
- **Postconditions:** the culling never hides a part of an entity that is inside the culling
  volume. An animation that moves a bone outside the culling volume can be culled.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: stand so that
only the snout of a pig is at the edge of the screen. The snout shows. Item 5 allows a cull
sphere with its centre at the centre of the block of the entity position, when its radius is
the largest distance of the culling volume from the entity position plus `√3 / 2` block. That
sphere contains the culling volume (`docs/entity-render-budget.md`, section 4.11).
Milestone: 1.

### CEREN-VIS-003: Reduced detail of a small entity

- **Inputs:** the projected height `h_px` of the entity in the frame: the height of the culling
  volume (CEREN-VIS-002) on the screen, in pixels, at the distance of the entity position from
  the camera; the full-rig cap of the client; the test resolution.
- **Behavior:**
  1. An entity with `h_px ≥ 12` draws with its full rig: each bone, each cuboid and each face of
     sections 2 and 3.
  2. An entity with `h_px < 12` can draw as an impostor. The client can raise the threshold of
     12 px for a frame when the full rigs of an earlier frame are more than the full-rig cap.
  3. An impostor is one quad that turns about the vertical axis of the entity position to face
     the camera. Its height and width are those of the culling volume.
  4. The quad shows one image from a set that the client renders from the full rig of the
     entity at rest pose. The set has one entry for each combination of model and texture of
     the active layers (CEREN-LAYER-001). Each entry has 16 views of yaw and 3 views of
     elevation: 0, 30 and 60 degrees. An image is at least 32 × 32 pixels.
  5. The client selects the view of the nearest yaw and the nearest elevation of the camera
     relative to the body yaw of the entity. It does not blend two views.
  6. Each pixel of an image holds the texel colour after the tint, and the face normal in
     internal space. The client shades an impostor pixel with CEREN-SURF-002 from that normal
     after the body yaw, then applies CEREN-SURF-003 to CEREN-SURF-006. The overlay and the
     flash of an impostor are the same as those of the full rig.
  7. An entity that dies (CEREN-PLACE-003) and an entity that is upside down
     (CEREN-PLACE-005) always draw with the full rig.
  8. A screenshot parity test of this document applies only to an entity with `h_px ≥ 12` at
     the test resolution. The pose tolerance of `entity-animation.md`, EANIM-GPU-003, applies
     only to a full rig.
- **Postconditions:** a far crowd costs one quad for each entity. A near entity looks the same
  as with no impostor.

Provenance: `design` (at 1440p, more than 95% of the visible mobs of a large scene are below
12 px. The draw of their faces is 61–85% of the cost of a full rig, and an impostor brings the
100,000-pig field on the web tier from 14.7 ms to 5.3 ms: `docs/entity-render-budget.md`,
sections 4.1 and 4.2. A face below 12 px of entity height covers less than 4 px², so the walk
phase and the head angle are not visible at that size). Kind: semantic. Confidence: medium.
Test: render one pig and one frog of each variant at `h_px` of 11, 8 and 4 at 16 yaw angles,
once as an impostor and once as a full rig. The mean FLIP error of the entity pixels must be at
most 0.1. Milestone: later. Until the impostor exists, an entity of item 2 draws as a full rig.

---

## 5. Layers

### CEREN-LAYER-001: Layers of a model

- **Inputs:** the layers that a mob document lists, in order.
- **Behavior:**
  1. The client draws the layers after the model, in the order of the list.
  2. Each layer uses the entity transform and the bone transforms of the same frame.
  3. Each layer uses the light of CEREN-SURF-003.
  4. Each layer uses the red overlay of CEREN-SURF-004, unless the mob document marks it as
     emissive. An emissive layer uses no overlay and no lightmap.
- **Postconditions:** the saddle of a hurt pig is red.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: put a saddle on
a pig. Hit the pig. The saddle turns red with the pig.
Milestone: 1.

---

## 6. Shadow (later milestone)

### CEREN-SHADOW-001: Radius and strength

- **Inputs:** the shadow radius `r₀` and the shadow strength `k` of the type from the mob
  document; the scale `s`; the age scale `g` (0.5 for a baby mob, otherwise 1, unless the mob
  document gives another value); the squared distance `d²` from the camera to the entity; the
  option "entity shadows".
- **Behavior:**
  1. When the option is off or the entity is invisible, there is no shadow.
  2. The radius is `r = min(r₀ · s · g, 32)`. When `r` is 0, there is no shadow.
  3. The power is `w = (1 − d² / 256) · k`. When `w` ≤ 0, there is no shadow.
- **Postconditions:** the shadow fades out at 16 blocks from the camera.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: walk away from a
pig. The shadow fades and disappears at 16 blocks.
Milestone: later.

### CEREN-SHADOW-002: Shadow pieces

- **Inputs:** the frame position `(x, y, z)`; `r` and `w` of CEREN-SHADOW-001; the client world.
- **Behavior:**
  1. Let `depth = min(w / 0.5 − 1, r)`.
  2. The client visits each block position with X in `[floor(x − r), floor(x + r)]`, Z in
     `[floor(z − r), floor(z + r)]` and Y in `[floor(y − depth), floor(y)]`.
  3. For a block position `p`, the block below `p` must be visible, must have a full collision
     shape and must have a shape that is not empty. Otherwise, `p` has no piece.
  4. The raw brightness at `p` must be more than 3. The raw brightness is the larger of the
     block light and the sky light after the darkening of the sky.
  5. The alpha of the piece is `clamp((w − (y − p.y) · 0.5) · 0.5 · B, 0, 1)`. `B` is the
     brightness curve of the raw brightness `v / 15`: `c = v / (4 − 3v)`, then
     `B = lerp(ambient light of the dimension type, c, 1)`.
  6. The piece is a horizontal quad at the top of the shape of the block below. It covers the
     bounds of that shape in X and Z.
- **Postconditions:** a shadow does not show on a slab or on a stair.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: lead a pig over
a slab. The shadow disappears over the slab.
Milestone: later.

### CEREN-SHADOW-003: Drawing of a shadow piece

- **Inputs:** a piece with the corners `(x₀, z₀)` and `(x₁, z₁)` relative to the entity; `r`;
  the alpha `a` of the piece; the texture `minecraft:textures/misc/shadow.png`.
- **Behavior:**
  1. The texture coordinates of a corner `(x, z)` are `u = −x / (2r) + 0.5` and
     `v = −z / (2r) + 0.5`.
  2. The colour is white with the alpha `a`. The texture multiplies it.
  3. The piece blends over the colour behind it. It does not write depth. It uses full light.
- **Postconditions:** the centre of the texture is under the entity position.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: take a
screenshot from above of a pig on grass. The shadow is a disc with its centre under the pig.
Milestone: later.

---

## 7. Name tag (later milestone)

### CEREN-NAME-001: When the name tag shows

- **Inputs:** the custom name and its visible flag (CENTS-DATA-002, indices 2 and 3); the
  crosshair target; the squared distance `d²` from the camera; the crouching flag; the team
  rules; the state of the HUD.
- **Behavior:** a mob shows its name tag when all of these conditions are true:
  1. The mob has a custom name.
  2. The visible flag is true, or the mob is the crosshair target.
  3. `d²` is less than `64²`. A mob that crouches also needs `d²` less than 1024.
  4. The HUD is not hidden, the mob is not the camera entity, the mob is not hidden from the
     local player (CEREN-VIS-001), and the mob has no passenger.
  5. The team rule of the name tag visibility allows it.
- **Postconditions:** the query `has_rider` does not change the name tag.

Provenance: `documentation`; `source_suggested` for the distances. Kind: semantic. Confidence:
medium. Experiment: name a pig. Walk away. The name tag disappears at 64 blocks when you aim at
the pig.
Milestone: later.

### CEREN-NAME-002: Placement and colours of the name tag

- **Inputs:** the frame position; the height `h` of the entity box; the camera orientation;
  the text of the custom name; the option "text background opacity" `o` (default 0.25); the
  light of CEREN-SURF-003.
- **Behavior:**
  1. The anchor is the frame position plus `(0, h + 0.5, 0)`.
  2. The text faces the camera with the full camera orientation.
  3. The scale is 0.025 block per GUI unit. The text is centred on the anchor horizontally.
  4. The background is black with the alpha `o`. The text colour is white with the alpha
     `max((o + 0.75) / 2, 0.5)`.
  5. A mob that does not crouch draws its name tag two times. The first draw shows through
     blocks, with the text colour and the background. The second draw is normal, with opaque
     white text, no background, and light levels of at least 2.
  6. A mob that crouches draws its name tag one time, with depth, the text colour and the
     background.
- **Postconditions:** a name tag behind a wall shows faintly.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: name a pig. Put a
wall between you and the pig. The name shows faintly through the wall.
Milestone: later.

---

## 8. Intentional deviations

### CEREN-DEV-001: Exact sine and cosine

- **Rule:** CEREN-MATH-001.
- **Reference:** the sine and the cosine of the model formulas of the reference have an error of
  up to about `1e-4`. The error is a step function of the angle, with 65536 steps in one turn.
- **Visible to the client:** no. The difference in a bone angle is less than 0.01 degree.
- **Test:** a pose test uses the pose tolerance of `entity-animation.md`, EANIM-GPU-003. A
  screenshot test uses the tolerance of section 0.2.

### CEREN-DEV-002: Coloured light tint at each point

- **Rule:** CEREN-SURF-003, item 4.
- **Reference:** the reference has no coloured light tint. It uses one light value for the full
  entity, from the eye position.
- **Visible to the client:** yes. Near a coloured light source, a large mob shows a gradient of
  the tint along its body. With white light, the colour is the same as in the reference.
- **Test:** a screenshot test against the reference uses scenes with white light only.

### CEREN-DEV-003: Impostors for small entities

- **Rule:** CEREN-VIS-003.
- **Reference:** the reference draws each entity with all its cuboids at each distance.
- **Visible to the client:** yes, only below 12 px of entity height. A far entity does not move
  its legs or its head, and its outline follows one of 16 × 3 views.
- **Test:** a screenshot test against the reference uses only entities of at least 12 px.
