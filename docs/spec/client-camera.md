# Camera of the client: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`. The rules were checked against the
behaviour of `26.4-snapshot-7`. In `26.4-snapshot-4`, the water test of CCAM-FOV-005 used only the
cell of the camera position.

This document states how the client places the camera of the local player in each frame, and
how the client calculates the field of view. The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Tick**, **frame**, **partial tick** (`pt`), **yaw**, **pitch**, **facing vector**,
  **linear interpolation** (`lerp`), **current value**, **previous value** and **eye height**:
  as `client-entities.md`, section 0.1, defines them.
- **Local player**: the player entity that the client controls.
- **Camera**: the point of view from which the client draws the world in a frame. The camera
  has a position, a yaw, a pitch and a field of view.
- **Camera tick**: the part of a client tick that updates the eye filter (CCAM-EYE-001) and the
  zoom filter (CCAM-FOV-004).
- **First person**: the camera mode where the camera is at the eye of the local player. The
  other modes are **third person back** and **third person front**.
- **Field of view** (`F`): the vertical angle of the view, in degrees, from the bottom edge to
  the top edge of the image.
- **FOV option** (`O`): the field of view that the user selects, in degrees.
- **Effect scale** (`k`): the option that scales the changes of the field of view that the
  state of the player causes. `k` is in the range `[0, 1]`.
- **Zoom modifier** (`m`): a factor that multiplies the FOV option. The client filters the zoom
  modifier in each tick.
- **Target modifier** (`m*`): the value toward which the zoom filter moves the zoom modifier.
- **Attribute value**: the value of an attribute of the local player, from its base value and
  its modifiers (CCAM-FOV-002).
- **Walking speed** (`w`): the value `walking speed` of the last packet
  `minecraft:player_abilities`. The value is `0.1` before the first packet.
- **Flying**: the value `flying` of the abilities of the local player. The packet
  `minecraft:player_abilities` sets it, and the input of the player changes it.
- **Use ticks** (`u`): the number of ticks since the local player started to use the item in
  its hand. `u` is 0 when the player uses no item.
- `f32` is a 32-bit IEEE 754 value. `f64` is a 64-bit IEEE 754 value.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". No artefact of the game confirms a rule of this document
yet. Each rule with the label `source_suggested` therefore has an experiment.

Each rule has a line `Milestone`. `1` means the first release of this document. `later` means a
rule that a later release implements. A client that does not implement a `later` rule uses the
value that the rule gives when its condition is false.

A parity test of this document compares the field of view with a tolerance of `1e-4` degree,
angles with `1e-4` degree, and positions with `1e-6` block.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CCAM-TIME` | the order of the camera tick and of the frame |
| `CCAM-EYE` | the eye height that the camera uses |
| `CCAM-POSE` | the position and the orientation of the camera |
| `CCAM-FOV` | the field of view |
| `CCAM-PROJ` | the projection |
| `CCAM-OPT` | the options |
| `CCAM-LATER` | behaviour that a later release adds |
| `CCAM-TEST` | parity tests |
| `CCAM-DEV` | intentional deviations |

### 0.4 Scope

In scope: the first-person camera of the local player, the zoom filter, the field of view of
the world, the eye filter and the projection.

Out of scope: the field of view of the hand and of the held item, view bobbing, the tilt of the
view when the player takes damage, the nausea and the portal distortion, the smooth camera
option, the cull frustum, the fog and the panorama mode. Section 7 lists the later camera modes.

---

## 1. Order

### CCAM-TIME-001: Camera tick

- **Inputs:** a client tick.
- **Behavior:**
  1. The camera tick runs one time in each client tick.
  2. The camera tick runs before the tick of the local player and before the ticks of the other
     client entities.
  3. The camera tick updates the eye filter (CCAM-EYE-001), then the zoom filter
     (CCAM-FOV-004).
- **Postconditions:** the camera tick reads the state of the local player at the end of the
  previous tick. A change of the state of the player therefore changes the target modifier one
  tick later.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: start to sprint
in a frame that is just after a tick. Record frames at 240 Hz. The field of view starts to
change at the second tick after the input, not at the first tick.
Milestone: 1.

### CCAM-TIME-002: Camera in a frame

- **Inputs:** a frame; `pt` of CENTS-TIME-001.
- **Behavior:**
  1. The client applies the look input of the frame to the local player (CCAM-POSE-002) before
     it places the camera.
  2. The client places the camera (CCAM-POSE-001, CCAM-POSE-002).
  3. The client calculates the field of view (CCAM-FOV-005).
  4. The client calculates the projection (CCAM-PROJ-001).
- **Postconditions:** the frame uses one camera position, one orientation and one field of view
  for all the world content that it draws.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.
Milestone: 1.

---

## 2. Eye height

### CCAM-EYE-001: Eye filter

- **Inputs:** the eye height `e` of the local player for its current pose; the eye filter value
  `h` and its previous value `h'`, both `f32`.
- **Behavior:** in each camera tick:
  1. `h' = h`.
  2. `h = h + (e − h) · 0.5`.
- **Postconditions:** a change of the pose moves the camera to the new eye height over a few
  ticks. Half of the remaining distance goes in each tick.

The eye heights of a player, with the scale `s` of the attribute `minecraft:scale`, are:

| Pose | Eye height |
|------|------------|
| `standing` | `1.62 · s` |
| `crouching` | `1.27 · s` |
| `swimming`, `fall_flying`, `spin_attack` | `0.4 · s` |
| `sleeping` | `0.2` |
| `dying` | `1.62` |

A pose that the table does not show uses the value of `standing`. The pose names are the values
of the entity data type `pose`.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: press the sneak key
and record frames at 240 Hz. The camera moves down by `0.175`, `0.0875`, `0.04375` blocks in the
first three ticks.
Milestone: 1. A client that has only the pose `standing` uses the eye height `1.62`.

### CCAM-EYE-002: Start value of the eye filter

- **Inputs:** the first camera tick after the client gets a local player.
- **Behavior:**
  1. Before that tick, `h` and `h'` are equal to the eye height of the local player. The first
     camera tick or the first frame that has the local player sets them, whichever comes
     first.
  2. The filters do not start again while the client keeps the same local player, also after
     a respawn or a change of dimension.
  3. The zoom filter of CCAM-FOV-004 starts the same way, with the value of its target.
- **Postconditions:** the camera is at the eye in the first frame of a world. The filters
  reach a new eye height or a new target within a few ticks, so a new start gives no visible
  change.

Provenance: `design`. Kind: deviation (CCAM-DEV-001). Confidence: high.
Milestone: 1.

---

## 3. Position and orientation

### CCAM-POSE-001: Camera position

- **Inputs:** the current position `P` and the previous position `P'` of the local player, as
  `f64`; the eye filter values `h` and `h'`; `pt`.
- **Behavior:**
  1. The frame position of the player is `Q = lerp(pt, P', P)`, for each axis, in `f64`.
  2. The camera position is `Q + (0, lerp(pt, h', h), 0)`.
- **Postconditions:** the camera moves smoothly between two ticks. The frame position `Q` is also
  the position at which the client draws the local player and all content that the client
  attaches to the local player in that frame.

The previous position is the position at the start of the last tick of the local player.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: fly forward at a
constant speed and record frames at 240 Hz. The camera position changes in each frame, and the
change between two frames is constant.
Milestone: 1.

### CCAM-POSE-002: Camera orientation

- **Inputs:** the current yaw `y` and pitch `p` of the local player, the previous yaw `y'` and
  pitch `p'`; `pt`.
- **Behavior:**
  1. The look input of a frame adds the same change to the current value and to the previous
     value of the yaw, and the same change to both values of the pitch.
  2. At the start of each tick of the local player, `y' = y` and `p' = p`.
  3. The camera yaw is `lerp(pt, y', y)`. The camera pitch is `lerp(pt, p', p)`. No wrap occurs.
     A client that keeps the yaw in `[−180, 180)` interpolates the yaw along the shorter arc
     from `y'` to `y`. The result is the same when the change of the yaw in one tick is less
     than `180°`.
  4. The view direction of the camera is the facing vector of the camera yaw and the camera
     pitch.
  5. The up direction of the camera is the facing vector of the camera yaw and the pitch
     `camera pitch − 90`.
  6. The camera has no roll.
- **Postconditions:** a look input changes the view in the same frame. Only a change of the yaw
  or the pitch that a tick makes, for example a teleport with rotation, is interpolated.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: turn the mouse
between two ticks. The view turns in the frame of the input.
Milestone: 1.

---

## 4. Field of view

### CCAM-FOV-001: Target modifier

- **Inputs:** flying; the attribute value `v` of `minecraft:movement_speed`; the walking speed
  `w`; the item that the local player uses and the use ticks `u`; the camera mode; the effect
  scale `k`.
- **Behavior:** in `f32`:
  1. Let `a = 1`.
  2. If the local player is flying, `a = a · 1.1`.
  3. If `w` is not 0, `a = a · (v / w + 1) / 2`.
  4. If the local player uses the item `minecraft:bow`, let `t = min(u / 20, 1)`. Then
     `a = a · (1 − t² · 0.15)`.
  5. Else, if the local player uses the item `minecraft:spyglass` and the camera mode is first
     person, `m* = 0.1`. Step 6 does not occur.
  6. `m* = lerp(k, 1, a)`.
- **Postconditions:** with `k = 0`, only the spyglass changes the target modifier.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: set the effect
scale to 100 %. Fly in creative mode with no sprint. The field of view settles at `1.1 · O`.
Then sprint. The field of view settles at `1.265 · O`.
Milestone: 1 for steps 1 to 3 and 6. `later` for steps 4 and 5.

### CCAM-FOV-002: Attribute value

- **Inputs:** the base value `b` of an attribute; its modifiers, each with an amount and an
  operation; the range `[lo, hi]` of the attribute.
- **Behavior:** in `f64`:
  1. Let `b₁ = b +` the sum of the amounts of the modifiers with the operation `add_value`.
  2. Let `r = b₁ + b₁ ·` the sum of the amounts with the operation `add_multiplied_base`.
  3. For each modifier with the operation `add_multiplied_total`, `r = r · (1 + amount)`.
  4. The attribute value is `r`, clamped to `[lo, hi]`.
- **Postconditions:** the order of the modifiers in one operation does not change the value,
  apart from rounding.

For `minecraft:movement_speed` of a player:

- The base value is `0.1`, as `f32` and then widened to `f64`, before the packet
  `minecraft:update_attributes` gives a value.
- The range is `[0, 1024]`.
- While the local player sprints, the client adds a modifier with the id
  `minecraft:sprinting`, the amount `0.3` (as `f32`, widened to `f64`) and the operation
  `add_multiplied_total`. The client removes it when the player stops to sprint.
- The client narrows the attribute value to `f32` before step 3 of CCAM-FOV-001.

Provenance: `documentation` for steps 1 to 4. `source_suggested` for the values of the player.
Kind: semantic. Confidence: high. Experiment: give the player the effect `minecraft:speed` with
the amplifier 0, stand still and do not fly, with the effect scale at 100 %. The field of view settles at
`1.1 · O` (`v = 0.12`).
Milestone: 1. A client that has no attribute data uses the base value and the sprint modifier.

### CCAM-FOV-003: Camera entity that is not a player

- **Inputs:** a camera that follows an entity that is not a player, for example in spectator
  mode.
- **Behavior:** `m* = 1`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.
Milestone: later.

### CCAM-FOV-004: Zoom filter

- **Inputs:** the target modifier `m*` of the camera tick; the zoom modifier `m` and its previous
  value `m'`, both `f32`.
- **Behavior:** in each camera tick:
  1. `m' = m`.
  2. `m = m + (m* − m) · 0.5`.
  3. `m = clamp(m, 0.1, 1.5)`.
- **Postconditions:** the zoom modifier is in `[0.1, 1.5]` after each camera tick. Half of the
  remaining distance to the target goes in each tick.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: as CCAM-FOV-001.
Record frames at 240 Hz. Count the ticks until the field of view settles.
Milestone: 1.

The start value of `m` and `m'` is the target modifier of the first camera tick
(CCAM-DEV-001). Provenance: `design`.

### CCAM-FOV-005: Field of view of a frame

- **Inputs:** the FOV option `O`; `m` and `m'`; `pt`; the effect scale `k`; the dying state and
  the death time `d` of the local player (CENTS-DEATH-002); the fluid that contains the camera
  position.
- **Behavior:** in `f32`:
  1. `F = O · lerp(pt, m', m)`.
  2. If the local player is dying, let `t = min(d + pt, 20)`. Then
     `F = F / ((1 − 500 / (t + 500)) · 2 + 1)`.
  3. If the camera is in water or in lava, `F = F · lerp(k, 1, 6/7)`.
- **Postconditions:** `F` is the vertical field of view of the projection (CCAM-PROJ-001).

The test of step 3 has these steps:

1. Make five points: the camera position plus the centre of the near plane of CCAM-PROJ-001, and
   the camera position plus each of the four corners of that plane, in the order top left, top
   right, bottom left, bottom right.
2. For each point in that order, find the block cell that contains the point. When the cell holds
   a fluid and the point `y` is at or below the cell bottom plus the height of the fluid surface
   in that cell, the result is that fluid: water or lava. Stop at the first such point.

The test belongs to the fog rules, which a later document states.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: with `O = 70`, die.
The field of view goes down to about `65` in 20 ticks. Swim with the camera under water. The
field of view is `60` with `k = 1`.
Milestone: 1 for step 1. `later` for steps 2 and 3.

---

## 5. Projection

### CCAM-PROJ-001: Projection

- **Inputs:** `F`; the width and the height of the image in pixels; the render distance `r` in
  chunks that the client uses; the cloud range `c` in chunks.
- **Behavior:**
  1. The projection is a perspective projection with the vertical field of view `F`.
  2. The aspect ratio is the width divided by the height.
  3. The near plane is at `0.05` blocks from the camera.
  4. The far plane is at `max(64 · r, 16 · c)` blocks or farther. The cloud range `c` is an
     option of the client, from `2` to `128` chunks. The default is `128`.
  5. A client that draws no clouds can use the far plane `64 · r`.
- **Postconditions:** world content at `0.05` blocks or more from the camera, and inside the
  far plane, is not clipped.

Provenance: `source_suggested` for items 1 to 3. `design` for item 4: a client with a reversed
or infinite depth range can put the far plane farther. Kind: semantic. Confidence: high.
Experiment: stand with the eye `0.1` blocks from a wall and look at the wall. The wall is not
clipped.
Milestone: 1.

---

## 6. Options

### CCAM-OPT-001: FOV option

- **Inputs:** the options of the user.
- **Behavior:**
  1. `O` is an integer in `[30, 110]`. The default is `70`.
  2. `k` is a number in `[0, 1]`. The default is `1`.
  3. The options file stores the line `fov:<x>` with `x = (O − 70) / 40` as a decimal number. A
     reader sets `O = trunc(x · 40 + 70)`.
  4. The options file stores the line `fovEffectScale:<k>`.
  5. The caption of the slider of the effect scale shows `k` as a percentage, and its handle is
     at the position `√k`. A position `q` of the slider sets `k = q²` (`client-menu.md`,
     MENU-VAL-002).
- **Postconditions:** the defaults give `m* = 1.1` for a player that flies and does not sprint.

Provenance: `source_suggested`. Kind: client agreement for items 3 and 4, semantic for the
other items. Confidence: high. Experiment: set the field of view to 90 in the options screen,
close the game, and read `options.txt`. The line is `fov:0.5`.
Milestone: 1 for items 1 and 2 with the default values. `later` for items 3 to 5.

---

## 7. Later work

### CCAM-LATER-001: Third person

- **Inputs:** the camera mode third person back or third person front.
- **Behavior:**
  1. The client places the camera as in first person (CCAM-POSE-001, CCAM-POSE-002).
  2. In third person front, the camera yaw is `yaw + 180`, and the camera pitch is `−pitch`.
  3. Let `D = max(s · a, s₂ · a₂)`. `s` and `a` are the scale and the value of the attribute
     `minecraft:camera_distance` of the local player (default `4`, range `[0, 32]`). `s₂` and
     `a₂` are those of the living entity that the player rides, else `s` and `a` again.
  4. For each of the 8 corner offsets `(±0.1, ±0.1, ±0.1)`, the client casts a ray against the
     visual shapes of the blocks. The ray starts at the camera position plus the offset, and
     goes a distance `D` against the view direction. Fluids do not stop the ray.
  5. When a ray hits at a distance `d` from the camera position (without the offset) with
     `d < D`, `D = d`.
  6. The client moves the camera a distance `D` against the view direction.
- **Postconditions:** the camera does not go into a block.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.
Milestone: later.

### CCAM-LATER-002: Other placements

- A sleeping player in first person: the camera yaw is the yaw of the bed direction minus 180,
  the pitch is 0, and the camera moves `0.15` blocks up along its up direction. A bed with no
  direction gives the yaw 0.
- A passenger of a minecart that interpolates its own movement: the camera follows the
  interpolated position of the minecart plus the seat offset and the eye filter value.

Provenance: `source_suggested`. Kind: semantic. Confidence: low.
Milestone: later.

---

## 8. Parity tests

### CCAM-TEST-001: Zoom filter sequence

- **Inputs:** `O = 70`, `k = 1`, a player that flies, starts to sprint and does not stop.
- **Behavior:** start with `m = m' = 1.1`. Run five camera ticks with `m* = 1.265`.
- **Postconditions:** `O · m` after each tick is `82.775`, `85.6625`, `87.10625`, `87.828125`
  and `88.18906`, within `1e-4`.

### CCAM-TEST-002: Orientation

- **Inputs:** the yaw and pitch pairs `(0, 0)`, `(−139.9493, 8.099984)`, `(90, −30)`,
  `(180, 45)`, `(−90, 90)`, `(45, −90)`.
- **Postconditions:** the view direction of the camera is the facing vector of each pair, within
  `1e-5` on each axis.

### CCAM-TEST-003: Field of view in a frame

- **Inputs:** `O = 70`, `m' = 1.1`, `m = 1.1825`, `pt = 0.25`.
- **Postconditions:** `F = 70 · 1.120625 = 78.44375`, within `1e-4`.

---

## 9. Intentional deviations

### CCAM-DEV-001: Settled start values

- **Rule:** CCAM-EYE-002, CCAM-FOV-004.
- **Reference:** the eye filter and the zoom filter start at 0 when the game starts, and they
  keep their values from one world to the next. In the first world, the camera rises from the
  feet to the eye, and the view opens from the minimum zoom, over the first ticks.
- **Visible to the client:** yes, in the first ticks of the first world only.
- **Test:** none.

---

## 10. Open questions

1. CCAM-FOV-001: the bow use ticks of the reference count from the start of the use on the
   client. This document does not state how a use that the server cancels resets `u`.
2. CCAM-FOV-005, step 3: the fluid test depends on the fog rules, which no document states yet.
