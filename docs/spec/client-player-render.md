# Rendering of a player by the client: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states how the client draws a player that is not the local player: the model, the
skin and the pose. It also states the figure of the local player in the inventory screen
(section 5). The creative screen uses these rules for its figure. The rules of `client-entity-render.md` apply to a player as to each living entity.
This document gives only what is particular to a player. It does not prescribe an architecture.

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
- File space, internal space, model unit, box UV, bone and rest pose: as `entity-geometry.md`
  defines them.
- **Bend**: a turn in the middle of an arm, of a leg or of the body.
- **GUI unit**, **W**, **H** and **mouse position**: as `client-widget.md`, section 0.1, defines
  them. The mouse position (`mx`, `my`) is in whole GUI units.
- **Inventory image**: the background image of the inventory screen of the local player. It is
  176 by 166 GUI units. Its top left corner is the image origin (`ix`, `iy`).
- **Figure**: the model of the local player that the client draws in a GUI rectangle. The client
  draws it on the inventory image, right of the armour slots (section 5). It also draws it in the
  survival inventory tab of the creative screen (`client-creative-inventory.md`, CCI-LAY-007).
  That rule gives another figure rectangle and another size.
- **Cursor stack**: the stack that the player carries with the mouse, as `inventory.md` uses the
  term.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". The skin layout is public documentation of the game. No
artefact of the game confirms the pose numbers yet. Each pose rule has the label `design`. The rules of section 5 come from the reference and have
the label `source_suggested`.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CPR-MODEL` | the boxes and the bones of the model |
| `CPR-SKIN` | the skin image and where it comes from |
| `CPR-POSE` | head, walk and crouch |
| `CPR-HOST` | players that a host adds |
| `CPR-FIG` | the figure in the inventory screen |
| `CPR-DEV` | intentional deviations |

### 0.4 Scope

In scope: a player that the server adds with the entity type `minecraft:player`, and a player
that a host adds. The name tag of a player is in scope (`client-entity-render.md`,
CEREN-NAME-003 and CEREN-NAME-005). Out of scope: the cape, the elytra, items in hands, armour, the
swing of an arm, the bend and the skin parts that a player hides. `player-pose.md` states sleep,
swimming, crawl, flight with elytra and the spin attack of a player. The bones of the cape, of the elytra and of the items are in scope (CPR-MODEL-002). The
client draws nothing on them.

The figure of the local player in the inventory screen is in scope (section 5). The list above
applies also to the figure, except that the figure is not in the world, so it has no name tag. It has no cape, elytra, items in hands or armour. The slots
and the other parts of the inventory screen are out of scope.

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
     head and by `0.25` for the other parts.
- **Postconditions:** the top of the head is 2 blocks above the feet. A slim model is 1 texel
  narrower at each arm.

Provenance: `black_box_confirmed` (the model artefact `entity_models.json`). Kind: semantic.
Confidence: high.

### CPR-MODEL-002: Bones of the model

- **Inputs:** the model type.
- **Behavior:**
  1. The model has these bones. The name of a bone is the address that an animation and a
     resource pack use.
  2. A pivot is in file space, in model units. A model type that the table does not name
     separately has the pivot of the column "Pivot, wide".

| Bone | Parent | Pivot, wide | Pivot, slim | Boxes |
|------|--------|-------------|-------------|-------|
| `root` | none | `(0, 0, 0)` | | none |
| `waist` | `root` | `(0, 12, 0)` | | none |
| `body` | `waist` | `(0, 24, 0)` | | body, first layer |
| `jacket` | `body` | `(0, 24, 0)` | | body, second layer |
| `head` | `waist` | `(0, 24, 0)` | | head, first layer |
| `hat` | `head` | `(0, 24, 0)` | | head, second layer |
| `cape` | `waist` | `(0, 24, 2)` | | none |
| `elytra` | `waist` | `(0, 24, 2)` | | none |
| `right_arm` | `waist` | `(−5, 22, 0)` | | right arm, first layer |
| `right_sleeve` | `right_arm` | `(−5, 22, 0)` | | right arm, second layer |
| `right_item` | `right_arm` | `(−6, 12, −2)` | `(−5.5, 12, −2)` | none |
| `left_arm` | `waist` | `(5, 22, 0)` | | left arm, first layer |
| `left_sleeve` | `left_arm` | `(5, 22, 0)` | | left arm, second layer |
| `left_item` | `left_arm` | `(6, 12, −2)` | `(5.5, 12, −2)` | none |
| `right_leg` | `root` | `(−1.9, 12, 0)` | | right leg, first layer |
| `right_pants` | `right_leg` | `(−1.9, 12, 0)` | | right leg, second layer |
| `left_leg` | `root` | `(1.9, 12, 0)` | | left leg, first layer |
| `left_pants` | `left_leg` | `(1.9, 12, 0)` | | left leg, second layer |

  3. No bone has a rest rotation.
  4. A bone with no box draws nothing. An animation can turn it and move it, and its children
     follow.
  5. The body, the head and each arm are children of `waist`, not of each other. A turn of
     `body` does not turn the head or an arm.
- **Postconditions:** a turn of `root` turns the full model about the feet. A turn of `waist`
  turns the body, the head and the arms about the hip, and the legs stay. The front of the model
  is the side of negative Z.

Provenance: `black_box_confirmed` for the pivots of the bones with boxes (the model artefact
`entity_models.json`); `design` for the parents and for the bones with no box. Kind: semantic.
Confidence: high.

### CPR-MODEL-003: The second layer follows the first layer

- **Inputs:** the pose of a bone of the first layer.
- **Behavior:** each bone of the second layer is a child of the bone of the first layer of the
  same part. It has the same pivot. The client does not animate a bone of the second layer.
- **Postconditions:** a hat, a jacket, a sleeve and a trouser leg stay around their part in each
  pose.

Provenance: `design`. Kind: semantic. Confidence: high.

### CPR-MODEL-004: No bend

- **Inputs:** none.
- **Behavior:** each arm and each leg is one box of 12 model units on one bone. The body is one
  box on one bone. The model has no bend.
- **Postconditions:** an arm, a leg and the body stay straight in each pose.

Provenance: `design`. Kind: semantic. Confidence: high.

### CPR-MODEL-005: The bone set is stable

- **Inputs:** a later version of this document.
- **Behavior:**
  1. A later version can add a bone. A new bone has no rest rotation.
  2. A later version does not change the name or the pivot of a bone of CPR-MODEL-002, and it
     does not remove such a bone.
  3. A later version can put a new bone between a bone and its parent. In the rest pose, each
     box stays where it is.
- **Postconditions:** an animation that names only bones of CPR-MODEL-002 gives the same pose
  after such a change. A bend can come later with new bones for the lower half of a part.

Provenance: `design`. Kind: semantic. Confidence: high.

## 2. Skin

### CPR-SKIN-001: Skin images

- **Inputs:** a PNG image.
- **Behavior:**
  1. An image of 64 by 64 texels is a skin.
  2. An image of 64 by 32 texels is a legacy skin. CPR-SKIN-002 makes it a skin.
  3. An image of another size is not a skin. The client draws the default skin
     (CPR-SKIN-004).
  4. The client holds at most 256 skins at the same time. A player past them has the default
     skin of its model type (CPR-SKIN-004) until a skin frees (CPR-DEV-002).
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

Provenance: `documentation`. Kind: semantic. Confidence: high.

### CPR-SKIN-003: Opaque first layer

- **Inputs:** a skin.
- **Behavior:** the client sets the alpha of each texel of these rectangles to 255. Each
  rectangle is `[x, y, width, height]` in texels: `[0, 0, 32, 16]`, `[0, 16, 64, 16]` and
  `[16, 48, 32, 16]`. They hold the first layer of each part.
- **Postconditions:** a player has no hole in the first layer. A texel of the second layer with
  an alpha under the cutout limit of `client-entity-render.md`, CEREN-SURF-001, is not drawn.

Provenance: `documentation`. Kind: semantic. Confidence: medium.

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
  4. Until the image arrives, the player keeps the skin that it has. A player with no skin has
     the default skin of its model type.
  5. The web client downloads no image (CPR-DEV-004). Its players take the model type of the
     profile and the default skin of that model type.
- **Postconditions:** a property with no `SKIN` gives no skin. The player then has the default
  skin `wide`.

Provenance: `documentation` (the public profile service of the game). Kind: wire for items 1
and 2, design for items 3 to 5. Confidence: high.

## 3. Pose

All rotations are animation rotations in file space (`entity-animation.md`): a positive X
rotation turns the front of a bone down.

No rule of this section moves `root` or `waist`. Thus each angle and each move of this section
is relative to the model.

### CPR-POSE-001: Head

- **Inputs:** the frame values of `client-entities.md`, CENTS-FRAME-001.
- **Behavior:** the head turns by the head pitch about X and by the head yaw less the body yaw
  about Y. These are the queries `target_x_rotation` and `target_y_rotation`.
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
Confidence: medium.

### CPR-POSE-003: Crouch

- **Inputs:** the pose of the entity data.
- **Behavior:** while the pose is `crouching`, the parts move by these values in model units and
  turn by these angles in degrees, in addition to the walk:

| Part | Move | Rotation about X |
|------|------|------------------|
| head | `(0, −6.2, 0)` | 0 |
| body | `(0, −5.2, 0)` | 28.6479 |
| right arm, left arm | `(0, −5.2, 0)` | 22.9183 |
| right leg, left leg | `(0, −2, 4)` | 0 |

- **Postconditions:** the body leans forward and the legs stay below it.

Provenance: `design`. Kind: semantic. Confidence: medium.

## 4. Players of a host

### CPR-HOST-001: A player with no server

- **Inputs:** a UUID, a placement, the model type and the skin image that a host gives.
- **Behavior:**
  1. The host adds the player as a living client entity of the type `minecraft:player`. No
     packet of a server names it.
  2. The player ticks, turns and walks as a player that a server adds.
  3. A move of the host acts as a teleport of the server (`client-entities.md`, CENTS-MOVE-004):
     the player travels to the new placement over its update interval, or it is there at once
     when the distance is more than 64 blocks. As for a teleport, that snap keeps an active
     path, which moves with the player (CENTS-INTERP-003, item 3).
  4. The host can change the skin or the model type at any time. The next frame draws the change.
  5. When the host removes the player, its skin layer becomes free (CPR-DEV-002).
- **Postconditions:** the observer is a user of the library. The rules of sections 1 to 3 apply.

Provenance: `design`. Kind: semantic. Confidence: high.

## 5. Figure in the inventory screen

A player expects to see the local player as a figure in the dark rectangle of the inventory
image. The figure turns its body and its head to the mouse. It shows the skin, the crouch and the
walk of the local player. It does not take the light of the world.

All positions of this section are GUI positions in GUI units, with `y` down. A direction uses the
screen axes: +X to the right, +Y to the top of the screen and +Z toward the viewer.

### CPR-FIG-001: Figure rectangle

- **Inputs:** the image origin (`ix`, `iy`).
- **Behavior:**
  1. The figure rectangle has the left edge `ix + 26` and the top edge `iy + 8`. It is 49 GUI
     units wide and 70 GUI units high. The right edge is `ix + 75` and the bottom edge `iy + 78`.
  2. The client draws the figure only inside the figure rectangle. A part of the model outside
     the rectangle does not show.
  3. The client draws the figure at the resolution of window pixels, not of GUI units. An edge of
     the model can be between two GUI units.
- **Postconditions:** the figure rectangle is between the armour slots and the crafting grid. The
  figure never covers a slot.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### CPR-FIG-002: Size and position of the figure

- **Inputs:** the image origin (`ix`, `iy`); the pose of the local player; the box height `h` and
  the eye height `e` of the player in that pose before the scale attribute (`entity-physics.md`,
  PHY-BOX-004).
- **Behavior:**
  1. Let `cx = ix + 50.5` and `cy = iy + 43`. This is the centre of the figure rectangle.
  2. The feet point is (`cx`, `cy + (h / 2 + 0.0625) · 30`). The client puts the position of the
     player at the feet point.
  3. The eye point is (`cx`, `qy`), with `qy` = the `y` of the feet point less `e · 30`.
  4. The model has the size that it has in the world when the scale attribute is 1. One block
     is 30 GUI units.
  5. The projection is parallel. The figure has no perspective.
  6. With the mouse at the eye point, the front of the figure faces the viewer. The top of the
     model points to the top of the screen.
- **Postconditions:** the points of the figure have these `y` values:

| Pose | `h` | `e` | Feet point | Eye point |
|------|-----|-----|------------|-----------|
| `standing` | 1.8 | 1.62 | `iy + 71.875` | `iy + 23.275` |
| `crouching` | 1.5 | 1.27 | `iy + 67.375` | `iy + 29.275` |

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### CPR-FIG-003: The figure turns to the mouse

- **Inputs:** the mouse position (`mx`, `my`); the eye point (`cx`, `qy`) of CPR-FIG-002.
- **Behavior:**
  1. Let `dx = mx − cx` and `dy = qy − my`. `dy` is positive when the mouse is above the eye
     point.
  2. Let `a = 20 · atan(dx / 40)` and `t = 20 · atan(dy / 40)`. The function `atan` gives
     radians. The client uses `a` and `t` as angles in degrees.
  3. The body turns by `a` degrees about the vertical axis of the model. A positive `a` turns the
     front of the figure to the right of the screen.
  4. The head turns by `a` degrees relative to the body, in the direction of the body turn. This
     angle replaces the head yaw less the body yaw of CPR-POSE-001.
  5. The head pitch of CPR-POSE-001 is `−t`. A positive `t` turns the head up relative to the
     body.
  6. Then the full model turns by `t` degrees about the X axis of the screen through the feet
     point. A positive `t` moves the top of the model away from the viewer.
- **Postconditions:**
  - With the mouse at the eye point, the figure looks straight at the viewer.
  - Relative to the viewer, the head turns twice as far as the body: `2 · a` and `2 · t`.
  - `a` and `t` are always less than 31.4159 degrees in magnitude. They change in steps of the
    mouse position.
  - The figure turns to the mouse at each position in the window, also outside the inventory
    image.
  - After the pointer leaves the window, the figure uses the mouse position of
    `client-widget.md`, WID-PTR-005. It does not turn back to face the viewer.
  - While a mouse button is down, the figure follows the pointer also outside the window. The
    mouse position can then be negative, or more than W or H.
  - While no mouse button is down, the figure keeps the pose of the last mouse position in the
    window. It keeps that pose until the pointer comes back.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. The point that the figure
keeps after an exit with no button down has the kind unspecified (WID-PTR-005, item 3).

### CPR-FIG-004: State of the local player in the figure

- **Inputs:** the frame values of the local player (`client-entities.md`, CENTS-FRAME-001).
- **Behavior:** the client draws the figure by the rules of sections 1 to 3 and of
  `client-entity-render.md`, with these values:

| State | The figure |
|-------|------------|
| skin and model type | the skin and the model type of the local player: CPR-SKIN-004 and CPR-SKIN-005, with the entry of the player list for the UUID of the local player |
| second layer | shows (CPR-MODEL-001, item 2) |
| crouch | shows (CPR-POSE-003), with the box height and the eye height of the pose `crouching` (CPR-FIG-002) |
| walk | shows (CPR-POSE-002), with the walk position and the walk speed of the local player in the current frame (CPR-DEV-007) |
| body yaw, head yaw, pitch | CPR-FIG-003 replaces them |
| hurt | the red overlay of CEREN-SURF-004 shows |
| on fire | no flames show (CPR-DEV-005); the light does not change (CPR-FIG-005) |
| invisibility | CEREN-VIS-001 applies, with the local player as the entity and as the viewer |
| scale attribute | no effect: the model has the scale 1 (CPR-FIG-002) |
| shadow | none |

- **Postconditions:** an invisible local player with no team sees no model in the figure. In a
  team that can see friendly invisible members, the figure is translucent.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CPR-FIG-005: Light of the figure

- **Inputs:** the lightmap of the client world.
- **Behavior:**
  1. The figure has the block light 15 and the sky light 15. The client applies the lightmap of
     CEREN-SURF-003, item 3, to these levels. No coloured light tint and no fog apply.
  2. The face factor of CEREN-SURF-002, items 3 to 5, applies with two other light directions.
     In screen axes, they are `L0 = normalize(0.2, 1, 1)` and `L1 = normalize(−0.2, 1, 0)`.
  3. The light directions stay fixed on the screen. They do not turn with the figure, and the
     dimension does not change them.
- **Postconditions:** the face factor is 0.8201 for a face that points to the viewer, 1 for a face
  that points up, 0.4840 to the right, 0.5177 to the left and 0.4 down.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### CPR-FIG-006: Draw order of the figure

- **Inputs:** the inventory screen.
- **Behavior:** the figure is in front of the inventory image. The cursor stack and a tooltip are
  in front of the figure.
- **Postconditions:** a tooltip or the cursor stack over the figure rectangle covers the figure.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

## 6. Intentional deviations

### CPR-DEV-001: One default skin for each model type

- **Rule:** CPR-SKIN-004.
- **Reference:** the reference selects the default skin from the UUID of the player, among
  several default skins of both model types.
- **Visible to the client:** yes. A player with no skin is always Steve or Alex.
- **Test:** none.

### CPR-DEV-002: A fixed number of skins

- **Rule:** CPR-SKIN-001, item 4.
- **Reference:** the reference has no limit.
- **Visible to the client:** yes, only with more than 256 players with skins in view at the same
  time. A player past the limit has the default skin.
- **Test:** none.

### CPR-DEV-003: All skin parts show

- **Rule:** CPR-MODEL-001, item 2.
- **Reference:** a player can hide each part of the second layer.
- **Visible to the client:** yes. A hidden part shows.
- **Test:** none.

### CPR-DEV-004: No skin download on the web

- **Rule:** CPR-SKIN-005, item 5.
- **Reference:** the reference downloads the skin of each profile.
- **Visible to the client:** yes, only in the web client. Each player there is Steve or Alex.
- **Test:** none.

### CPR-DEV-005: No flames on the figure

- **Rule:** CPR-FIG-004, the row "on fire", also through `client-creative-inventory.md`,
  CCI-LAY-007.
- **Reference:** the reference draws the flames of a burning entity over the figure.
- **Visible to the client:** yes, while the local player burns and a screen shows the figure.
- **Test:** a screenshot test against the reference uses a local player that does not burn.

### CPR-DEV-006: The eye point with the scale attribute

- **Rule:** CPR-FIG-002, item 3, also through `client-creative-inventory.md`, CCI-LAY-007.
- **Reference:** the reference draws the model with a scale of 1, but it computes the eye point
  with the eye height after the scale attribute. With a scale of 2, the eye point is 48.6 GUI
  units above the eye of the figure. In the creative screen, it is 32.4 GUI units above. The
  figure then does not look at the mouse.
- **Visible to the client:** yes, only when the scale attribute of the local player is not 1.
- **Test:** a screenshot test against the reference uses a scale attribute of 1.

### CPR-DEV-007: The figure moves in each frame

- **Rule:** CPR-FIG-004, the row "walk", also through `client-creative-inventory.md`,
  CCI-LAY-007.
- **Reference:** the reference draws the figure with the values at the end of the current tick.
  The arms and the legs of the figure move only 20 times each second.
- **Visible to the client:** yes, while the local player walks and a screen shows the figure, at
  more than 20 frames each second.
- **Test:** a screenshot test against the reference uses a local player that stands still.

## 7. Unresolved

1. The reference draws each player at 15/16 of the size of the model, in the world and in the
   figure. While a player crouches, it also draws the model 2/16 block lower. CPR-MODEL-001 and
   CPR-POSE-003 do not state this, and the postcondition of CPR-MODEL-001 gives 2 blocks. Without
   the scale, the top of the head in the figure is 3.75 GUI units higher than in the game.
   Recommended: add both values to sections 1 and 3 in a separate change. The figure then gets
   them through CPR-FIG-002, item 4.
