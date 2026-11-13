# Rendering of a player by the client: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`.

This document states how the client draws a player that is not the local player: the model, the
skin and the pose. The rules of `client-entity-render.md` apply to a player as to each living
entity. This document gives only what is particular to a player. It does not prescribe an
architecture.

## 0. Conventions

### 0.1 Terms

- **Skin**: the texture of a player, 64 by 64 texels. Section 2 defines it.
- **Model type**: `wide` or `slim`. A slim model has arms of 3 texels, a wide model arms of 4.
- **First layer** and **second layer**: the boxes of the body parts, and the boxes around them
  that are a little larger. The second layer shows a hat, a jacket, sleeves and trousers.
- **Default skin**: the skin of the resource pack that the client draws when it has no other
  skin of the player.
- **Host**: a program that uses the client crates with no server, and that adds and moves the
  players itself.
- File space, internal space and box UV: as `entity-geometry.md` defines them.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". The skin layout is public documentation of the game. No
artefact of the game confirms the pose numbers yet. Each pose rule has the label `design` and an
experiment.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CPR-MODEL` | the boxes of the model |
| `CPR-SKIN` | the skin image and where it comes from |
| `CPR-POSE` | head, walk and crouch |
| `CPR-HOST` | players that a host adds |
| `CPR-DEV` | intentional deviations |

### 0.4 Scope

In scope: a player that the server adds with the entity type `minecraft:player`, and a player
that a host adds. Out of scope: the name tag, the cape, the elytra, items in hands, armour, the
swing of an arm, sleep, swimming, flight with elytra and the skin parts that a player hides.

---

## 1. Model

### CPR-MODEL-001: Boxes of the model

- **Inputs:** the model type.
- **Behavior:**
  1. The model has these boxes, in file space, in model units. The skin has the UV of each box
     at the texel `[U, V]` in the box UV layout.

| Part | Pivot | Origin | Size | UV of the first layer | UV of the second layer |
|------|-------|--------|------|-----------------------|------------------------|
| head | `(0, 24, 0)` | `(−4, 24, −4)` | `(8, 8, 8)` | `[0, 0]` | `[32, 0]` |
| body | `(0, 24, 0)` | `(−4, 12, −2)` | `(8, 12, 4)` | `[16, 16]` | `[16, 32]` |
| right arm, wide | `(−5, 22, 0)` | `(−8, 12, −2)` | `(4, 12, 4)` | `[40, 16]` | `[40, 32]` |
| left arm, wide | `(5, 22, 0)` | `(4, 12, −2)` | `(4, 12, 4)` | `[32, 48]` | `[48, 48]` |
| right arm, slim | `(−5, 22, 0)` | `(−7, 12, −2)` | `(3, 12, 4)` | `[40, 16]` | `[40, 32]` |
| left arm, slim | `(5, 22, 0)` | `(4, 12, −2)` | `(3, 12, 4)` | `[32, 48]` | `[48, 48]` |
| right leg | `(−1.9, 12, 0)` | `(−3.9, 0, −2)` | `(4, 12, 4)` | `[0, 16]` | `[0, 32]` |
| left leg | `(1.9, 12, 0)` | `(−0.1, 0, −2)` | `(4, 12, 4)` | `[16, 48]` | `[0, 48]` |

  2. Each box of the second layer has the box of its part, grown by `0.5` on each side for the
     head and by `0.25` for the other parts. It moves with its part.
  3. `assets/mcrs/entity/geometry/player.json` is generated from the model artefact
     `entity_models.json`, layers `minecraft:player` / `main` and `minecraft:player_slim` /
     `main`.
- **Postconditions:** the top of the head is 2 blocks above the feet. A slim model is 1 texel
  narrower at each arm.

Provenance: `black_box_confirmed` (the model artefact `entity_models.json`). Kind: semantic.
Confidence: high.

## 2. Skin

### CPR-SKIN-001: Skin images

- **Inputs:** a PNG image.
- **Behavior:**
  1. An image of 64 by 64 texels is a skin.
  2. An image of 64 by 32 texels is a legacy skin. CPR-SKIN-002 makes it a skin.
  3. An image of another size is not a skin. The client draws the default skin
     (CPR-SKIN-004).
- **Postconditions:** none.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### CPR-SKIN-002: Legacy skins

- **Inputs:** a legacy skin.
- **Behavior:**
  1. The rows 0 to 31 of the skin are the rows of the legacy skin. The rows 32 to 63 are
     transparent.
  2. The left leg takes the texels of the right leg (UV `[0, 16]`). The left arm takes the
     texels of the right arm (UV `[40, 16]`).
  3. Each face of the left part is the face of the right part, mirrored left to right. The east
     face and the west face change places.
  4. When each texel of the hat rectangle `[32, 0, 32, 16]` has an alpha of 128 or more, the
     client sets the alpha of each texel of that rectangle to 0.
- **Postconditions:** a left limb of a legacy skin looks like the mirror image of the right limb.
  A legacy skin that fills the hat area with one opaque colour shows no hat.

Provenance: `documentation`. Kind: semantic. Confidence: high. Experiment: CPR-TEST-001.

### CPR-SKIN-003: Opaque first layer

- **Inputs:** a skin.
- **Behavior:** the client sets the alpha of each texel of these rectangles to 255. Each
  rectangle is `[x, y, width, height]` in texels: `[0, 0, 32, 16]`, `[0, 16, 64, 16]` and
  `[16, 48, 32, 16]`. They hold the first layer of each part.
- **Postconditions:** a player has no hole in the first layer. A texel of the second layer with
  an alpha under the cutout limit of `client-entity-render.md`, CEREN-SURF-001, is not drawn.

Provenance: `documentation`. Kind: semantic. Confidence: medium. Experiment: CPR-TEST-001.

### CPR-SKIN-004: Default skin

- **Inputs:** the model type; whether the client has a skin of the player.
- **Behavior:** while the client has no skin of the player, it draws the default skin of the
  model type: `minecraft:entity/player/wide/steve` for `wide`, and
  `minecraft:entity/player/slim/alex` for `slim`. The model type of a player with no profile is
  `wide`.
- **Postconditions:** a player is never drawn with no texture.

Provenance: `design` (CPR-DEV-001). Kind: semantic. Confidence: high.

### CPR-SKIN-005: Skin of a profile

- **Inputs:** the properties of the player in the player list (the action "add player").
- **Behavior:**
  1. The property `textures` holds base64 of a JSON object. Its field `textures.SKIN.url` is the
     address of the skin image.
  2. The model type is `slim` when `textures.SKIN.metadata.model` is `"slim"`, and `wide` in
     each other case.
  3. The client downloads the image only from the host `textures.minecraft.net`, always over
     HTTPS, and reads at most 1 MiB of it. A download that fails keeps the skin that the player
     has.
  4. Until the image arrives, the player has the default skin of its model type.
- **Postconditions:** a property with no `SKIN` gives no skin. The player then has the default
  skin `wide`.

Provenance: `documentation` (the public profile service of the game). Kind: wire for items 1
and 2, design for items 3 and 4. Confidence: high.

## 3. Pose

All rotations are animation rotations in file space (`entity-animation.md`): a positive X
rotation turns the front of a bone down.

### CPR-POSE-001: Head

- **Inputs:** the frame values of `client-entities.md`, CENTS-FRAME-001.
- **Behavior:** the head turns by the head pitch about X and by the head yaw less the body yaw
  about Y. These are the queries `target_x_rotation` and `target_y_rotation`. The second layer of
  the head turns with it.
- **Postconditions:** the body follows the movement and the head keeps its own yaw
  (`client-entities.md`, CENTS-BODY-002).

Provenance: `design`. Kind: semantic. Confidence: high.

### CPR-POSE-002: Walk

- **Inputs:** the walk position `p` and the walk speed `s` of `client-entities.md`,
  CENTS-WALK-002.
- **Behavior:** with `c = cos(p × 38.1704°)`, the parts turn about X by these angles in degrees:

| Part | Rotation about X |
|------|------------------|
| right leg | `80.2141 × c × s` |
| left leg | `−80.2141 × c × s` |
| right arm | `−57.2958 × c × s` |
| left arm | `57.2958 × c × s` |

- **Postconditions:** each arm swings against the leg of its side. A player that stands still
  has straight arms and legs.

Provenance: `design` (the leg swing is the one of the pig, `mob-pig.md`). Kind: semantic.
Confidence: medium. Experiment: CPR-TEST-002.

### CPR-POSE-003: Crouch

- **Inputs:** the pose of the entity data.
- **Behavior:** while the pose is `crouching`, the parts move by these values in model units and
  turn by these angles in degrees, in addition to the walk:

| Part | Move | Rotation about X |
|------|------|------------------|
| head | `(0, −6.2, 0)` | 0 |
| body | `(0, −5.2, 0)` | 28.6479 |
| right arm, left arm | `(0, −5.2, 0)` | 22.9183 |
| right leg, left leg | `(0, −2.2, 4)` | 0 |

- **Postconditions:** the body leans forward and the legs stay below it.

Provenance: `design`. Kind: semantic. Confidence: medium. Experiment: CPR-TEST-002.

## 4. Players of a host

### CPR-HOST-001: A player with no server

- **Inputs:** a UUID, a placement, the model type and the skin image that a host gives.
- **Behavior:**
  1. The host adds the player as a living client entity of the type `minecraft:player`. No
     packet of a server names it.
  2. The player ticks, turns and walks as a player that a server adds.
  3. A move of the host acts as a teleport of the server (`client-entities.md`, CENTS-MOVE-004):
     the player travels to the new placement over its update interval, or it is there at once
     when the distance is more than 64 blocks.
  4. The host can change the skin or the model type at any time. The next frame draws the change.
  5. When the host removes the player, its skin layer becomes free (CPR-DEV-002).
- **Postconditions:** the observer is a user of the library. The rules of sections 1 to 3 apply.

Provenance: `design`. Kind: semantic. Confidence: high.

## 5. Intentional deviations

### CPR-DEV-001: One default skin for each model type

- **Rule:** CPR-SKIN-004.
- **Reference:** the reference selects the default skin from the UUID of the player, among
  several default skins of both model types.
- **Visible to the client:** yes. A player with no skin is always Steve or Alex.
- **Test:** none.

### CPR-DEV-002: A fixed number of skins

- **Rule:** CPR-SKIN-001.
- **Reference:** the reference has no limit.
- **Visible to the client:** yes, only with more than 256 players with skins in view at the same
  time. A player past the limit has the default skin.
- **Test:** none.

### CPR-DEV-003: All skin parts show

- **Rule:** CPR-MODEL-001, item 2.
- **Reference:** a player can hide each part of the second layer.
- **Visible to the client:** yes. A hidden part shows.
- **Test:** none.

## 6. Experiments

### CPR-TEST-001: Skin images

Join a server with a vanilla client and a player with a legacy skin that has a transparent
texel in the face and a different colour on each face of the right arm. Take a screenshot of the
player from the front, the back and each side. The left arm shows the right arm mirrored, and the
face shows no hole.

### CPR-TEST-002: Walk and crouch

Record a vanilla client that watches a player walk 10 blocks and then crouch
(`tools/mcrs_minecraft_client_recorder`). Compare each bone angle of the frames with the rules
of section 3.
