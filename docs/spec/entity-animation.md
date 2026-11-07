# Entity animation: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| Format reference | Bedrock resource pack, `Mojang/bedrock-samples` `1.26.60.5` |

Source of the game values: `assets/minecraft/version.json`.

This document states how the client reads animation files and animation controller files, and
how it computes the pose of a geometry from them in each frame. The file formats are those of
the Bedrock resource pack. The document does not prescribe an architecture. It states the
results that the CPU path and the GPU path must both give.
`client-entity-definition.md`, CENT-SCR-006, states when the client can skip an evaluation of
a frame without a visible change.

## 0. Conventions

### 0.1 Terms

The terms of `entity-geometry.md`, section 0.1, apply. These terms are added:

- **Clip**: one entry of an animation file. A clip has channels for bones, and fields for time
  and weight.
- **Channel**: the `rotation`, the `position` or the `scale` of one bone in one clip.
- **Keyframe**: a time in a channel, with a value `pre` and a value `post`.
- **Controller**: one entry of an animation controller file. A controller is a state machine.
- **Instance**: one clip or one controller that plays for one entity. An instance has its own
  time and its own state.
- **Play list**: the ordered list of instances that the client entity plays in one frame
  (EANIM-BLEND-002).
- **Frame**: one evaluation of the poses of all entities that the client draws. `Δt` is the time
  in seconds since the previous frame.
- **Pose**: for each bone of a geometry, the accumulated rotation, position and scale in file
  space.
- **Expression**: a Molang expression (`molang.md`).

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply, including the label `design`. The kinds of rule are those of `README.md`, section "Semantics, not
accidents". The milestones are those of `entity-geometry.md`, section 0.2.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `EANIM-CLIP` | the fields of a clip |
| `EANIM-KEY` | channels, keyframes and the sample of a channel |
| `EANIM-TIME` | the time of an instance |
| `EANIM-BLEND` | the accumulation of a pose |
| `EANIM-CTRL` | controllers |
| `EANIM-FX` | effects |
| `EANIM-GPU` | the pose tolerance |
| `EANIM-DEV` | intentional deviations |

### 0.4 Scope

In scope: the animation file, the animation controller file, the time of an instance, the
sample of a channel, the accumulation of a pose, and the classification of channels.

Out of scope: the Molang language (`molang.md`) and the queries (`molang-queries.md`), the
client entity file that starts the play list (`client-entity-definition.md`), and the bone
transform (`entity-geometry.md`, EGEO-POSE-001).

---

## 1. Clips

### EANIM-CLIP-001: The animation file

- **Inputs:** a file under `animations/` of the entity pack (`entity-geometry.md`,
  EGEO-PACK-001).
- **Behavior:**
  1. The root object has `format_version` and the object `animations`.
  2. Each key of `animations` is a clip identifier, for example `animation.pig.walk`. Its value
     is the clip.
  3. The client accepts each `format_version` from `1.8.0` up. The fields do not change between
     these versions.
- **Postconditions:** each clip is a separate definition (`entity-geometry.md`,
  EGEO-PACK-002).

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1.

### EANIM-CLIP-002: Fields of a clip

- **Inputs:** a clip.
- **Behavior:** the fields are:

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `loop` | `true`, `false` or `"hold_on_last_frame"` | `false` | EANIM-TIME-002 |
| `animation_length` | number ≥ 0, seconds | the time of the last keyframe of all channels, or 0 | EANIM-TIME-002 |
| `anim_time_update` | expression | `query.anim_time + query.delta_time` | EANIM-TIME-001 |
| `blend_weight` | expression | `1.0` | EANIM-BLEND-002 |
| `override_previous_animation` | bool | `false` | EANIM-BLEND-003 |
| `start_delay` | expression, seconds | `0` | EANIM-TIME-003 |
| `loop_delay` | expression, seconds | `0` | EANIM-TIME-003 |
| `bones` | object | empty | bone name → channels (EANIM-KEY-001) |
| `sound_effects`, `particle_effects`, `timeline` | object | empty | EANIM-FX-001 |

  An expression is a string or a number. A number is a constant expression.
- **Postconditions:** none.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1 for `loop`,
`animation_length`, `anim_time_update`, `blend_weight` and `bones`; 2 for
`override_previous_animation`; later for the delays.

---

## 2. Channels and keyframes

### EANIM-KEY-001: Channels of a bone

- **Inputs:** the value of one bone in `bones`.
- **Behavior:**
  1. The object has the optional channels `rotation` (degrees), `position` (model units) and
     `scale` (a factor).
  2. The object can have `relative_to`, an object. `relative_to.rotation` with the value
     `"entity"` is EANIM-BLEND-004. The client ignores other values and logs them.
  3. When the geometry has no bone with this name, the client ignores the channels of this
     bone. This is not a content error.
  4. The client compares the bone name without regard to letter case.
- **Postconditions:** none.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1; later for
`relative_to`.

### EANIM-KEY-002: Forms of a value

- **Inputs:** a channel value or a keyframe value.
- **Behavior:**
  1. A number `n`, or an array `[n]`, is the vector `(n, n, n)`.
  2. An array of three elements is the vector of its elements.
  3. Each element can be a number or an expression.
  4. A channel that is a value, and not an object of keyframes, is one keyframe at time 0.
  5. Another form is a content error. The client ignores the channel.
- **Postconditions:** each value is a vector of three expressions.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1.

### EANIM-KEY-003: Keyframes

- **Inputs:** a channel that is an object.
- **Behavior:**
  1. Each key is a time in seconds, as a decimal string. The client sorts the keyframes by
     time as numbers. The order of the keys in the file has no meaning.
  2. A key that is not a number, or a negative time, is a content error. The client ignores
     that keyframe.
  3. A value is a value of EANIM-KEY-002, or an object with `pre`, `post` and `lerp_mode`.
  4. A plain value sets `pre` and `post` to the same value.
  5. When the object has only `post`, `pre` is equal to `post`. When the object has only
     `pre`, `post` is equal to `pre`.
  6. `lerp_mode` is `"linear"` or `"catmullrom"`. The default is `"linear"`. Another value is a
     content error, and the client uses `"linear"`.
- **Postconditions:** each keyframe has a time, `pre`, `post` and a lerp mode.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1.

### EANIM-KEY-004: The sample of a channel

- **Inputs:** the keyframes `k[0..n−1]` of a channel, and the sample time `t`
  (EANIM-TIME-002).
- **Behavior:**
  1. Find `a`, the last keyframe with a time ≤ `t`, and `b`, the first keyframe with a time >
     `t`.
  2. When only `b` exists, the value is `b.pre`.
  3. When only `a` exists, the value is `a.post`.
  4. Otherwise, `α = (t − a.time) / (b.time − a.time)`.
  5. When `a` or `b` has the lerp mode `catmullrom`, the value is EANIM-KEY-005.
  6. Otherwise, the value is `a.post + (b.pre − a.post) · α`.
  7. The client interpolates each axis separately. The client does not interpolate the
     rotation as a quaternion.
- **Postconditions:** a step is a keyframe with `pre` equal to the previous value and `post`
  equal to the new value.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1.

### EANIM-KEY-005: Catmull-Rom

- **Inputs:** the keyframes `a = k[i]` and `b = k[i+1]` of EANIM-KEY-004, and `α`.
- **Behavior:**
  1. `p1 = a.post` and `p2 = b.pre`.
  2. `p0 = k[i−1].post` when `k[i−1]` exists. Otherwise `p0 = p1`.
  3. `p3 = k[i+2].pre` when `k[i+2]` exists. Otherwise `p3 = p2`.
  4. The value is
     `0.5 · (2·p1 + (p2 − p0)·α + (2·p0 − 5·p1 + 4·p2 − p3)·α² + (3·p1 − p0 − 3·p2 + p3)·α³)`.
  5. The parameter is uniform in each segment. The time distance of the neighbours has no
     effect.
- **Postconditions:** the curve passes through `p1` at `α = 0` and through `p2` at `α = 1`.

Provenance: `unresolved` for 2, 3 and 5. The format reference does not state the end points or
the parameter. Blockbench makes a shorter curve at the ends, and uses a parameter over the
times of all keyframes. Kind: semantic. Confidence: low. Experiment: export a clip with three
uneven Catmull-Rom keyframes from Blockbench. Sample it at 20 times with this rule and with
Blockbench. Record the largest difference. Milestone: 2.

### EANIM-KEY-006: Expressions in a keyframe

- **Inputs:** a keyframe value with an expression.
- **Behavior:**
  1. The client evaluates the expression in each frame. The client does not evaluate it one
     time at load.
  2. `query.key_frame_lerp_time` is `α` of EANIM-KEY-004 for this sample. It is 0 when only one
     of `a` and `b` exists.
  3. `this` is EANIM-BLEND-001.
  4. The client evaluates only the expressions that the sample uses: `a.post` and `b.pre`, and
     the neighbours for Catmull-Rom.
- **Postconditions:** none.

Provenance: `documentation`; `design` for 4. Kind: semantic. Confidence: medium. Milestone: 1.

---

## 3. Time

### EANIM-TIME-001: Time of an instance

- **Inputs:** a clip instance; the change `Δt` of the entity clock since the previous frame of
  the entity, divided by 20 (`molang-queries.md`, MOLQ-TIME-001, `delta_time`).
- **Behavior:**
  1. When the instance starts, `query.anim_time` is 0.
  2. In each frame, the start frame included, the client evaluates `anim_time_update`. The
     result is the new `query.anim_time`.
  3. In the start frame, `query.delta_time` is 0. In each later frame, `query.delta_time` is
     `Δt`.
  4. The client evaluates `anim_time_update` before it samples the channels of the frame.
  5. A result that is not finite is a runtime error. The time does not change in that frame.
- **Postconditions:** with the default `anim_time_update`, the time is 0 in the start frame and
  increases by `Δt` in each later frame. The time of an instance with the default
  `anim_time_update` is therefore `(C − C0) / 20`, with `C` the entity clock
  (`client-entities.md`, section 0.1) and `C0` its value in the start frame. The time does not
  increase while the entity does not tick. An `anim_time_update` that does not read
  `query.anim_time`, for example a time from the walk position, gives the same time in the start
  frame as in a later frame with the same inputs.

Provenance: `documentation` for 1 and 2; `design` for 3, 4 and 5. Kind: semantic.
Confidence: medium. Milestone: 1.

### EANIM-TIME-002: Loop modes and the sample time

- **Inputs:** an instance with the time `τ` (`query.anim_time`), the length `L` and the loop mode.
- **Behavior:**

| `loop` | `L` | Sample time `t` | The instance is finished when |
|--------|-----|-----------------|-------------------------------|
| any | `0` | `τ` | never |
| `true` | `> 0` | `τ` after the client sets `τ = τ mod L` | `τ` passes `L` for the first time |
| `"hold_on_last_frame"` | `> 0` | `min(τ, L)` | `τ ≥ L` |
| `false` | `> 0` | `τ` | `τ ≥ L` |

  1. For `loop: true`, the client reduces the stored time modulo `L` after each update, so
     that `query.anim_time` stays in `[0, L)`.
  2. A finished instance with `loop: false` adds nothing to the pose. Its channels are not
     sampled.
  3. A finished instance with `"hold_on_last_frame"` adds the pose at `t = L`.
- **Postconditions:** a clip with `L = 0` and time driven by distance never wraps.

Provenance: `documentation`; `design` for the row with `L = 0`. Kind: semantic. Confidence:
medium. Milestone: 1.

### EANIM-TIME-003: Delays

- **Inputs:** an instance of a clip with `start_delay` or `loop_delay`.
- **Behavior:**
  1. The client evaluates `start_delay` one time when the instance starts.
  2. During the start delay, `query.anim_time` stays 0 and the instance adds nothing to the
     pose.
  3. For `loop: true`, the client evaluates `loop_delay` each time that the time wraps. During
     the loop delay, the instance adds the pose at `t = 0`.
  4. A delay counts down by the time step of the entity (EANIM-TIME-002), not by the time of
     the frame. The frame in which the instance starts does not count toward the start delay.
  5. When a loop delay ends, the time of the instance is 0. This also applies to a clip with
     `anim_time_update`: its time starts again at 0.
- **Postconditions:** none.

Provenance: `documentation` for 1; `unresolved` for 2 and 3; `design` for 4 and 5 (the
documentation states no unit for the countdown; the time step of the entity stops with the
entity clock). Kind: semantic. Confidence: low. Milestone: later.

### EANIM-TIME-004: Start and restart

- **Inputs:** an entry of the play list with a weight (EANIM-BLEND-002).
- **Behavior:**
  1. The instance starts when the entity first plays the entry with a weight that is not 0.
  2. When the weight of the entry is 0 in a frame, the instance stops. It does not update its
     time. Only the weight of the `animate` entry or of a state entry stops an instance. A
     `blend_weight` of the clip that is 0 removes the instance from the play list of that
     frame (EANIM-BLEND-002, item 6), but the time of the instance continues.
  3. When the weight becomes not 0 again, the instance starts again with the time 0.
  4. A clip in a state of a controller starts again each time that the controller enters the
     state (EANIM-CTRL-002).
- **Postconditions:** none.

Provenance: `unresolved` for 2 and 3; `design` for the `blend_weight` sentence of 2 (a clip
that fades out by its own weight keeps its phase). The format reference does not state if a
clip in the `animate` list starts again after its weight was 0. Kind: semantic. Confidence: low.
Experiment: make a client entity that plays a clip of 2 s with `loop: false` and the weight
`query.is_sneaking`. Sneak for 0.5 s, stand, and sneak again. Record if the clip plays from
its start. Milestone: 1 for 1; 2 for 2 to 4.

---

## 4. Accumulation of the pose

### EANIM-BLEND-001: The pose of a frame

- **Inputs:** a baked geometry, and the play list of an entity in this frame.
- **Behavior:**
  1. At the start of the frame, the pose of each bone is: rotation = the rest rotation of the
     file, position = `(0, 0, 0)`, scale = `(1, 1, 1)`.
  2. The client applies the instances of the play list in their order (EANIM-BLEND-002).
  3. For each channel of an instance, the client samples the channel (EANIM-KEY-004) and
     changes the pose of the bone:
     - rotation: `rot = rot + w · v`, for each axis;
     - position: `pos = pos + w · v`, for each axis;
     - scale: `scl = scl · (1 + (v − 1) · w)`, for each axis.
     `w` is the weight of the instance and `v` is the sample.
  4. `this` in an expression of a channel is the value of that axis of that channel of the
     bone before the instance applies it. The value is in file space.
  5. A sample that is not finite on an axis changes nothing on that axis. On a GPU, the pose pass clamps each sampled value to `[−1e6, 1e6]`. A NaN gives an unspecified finite value in that range. (`molang.md`,
     MOL-NUM-005, item 7).
  6. After the last instance, the client converts the pose to internal space
     (`entity-geometry.md`, EGEO-SPACE-002). The bone transform (EGEO-POSE-001) uses it.
- **Postconditions:** the order of two instances changes the pose only through `this` and
  through EANIM-BLEND-003.

Provenance: `documentation` for 1 to 4 (the format reference adds rotation and position per
axis, and starts from the rest pose); `design` for the scale and for 5 (EANIM-DEV-001).
Kind: semantic. Confidence: medium. Milestone: 1.

### EANIM-BLEND-002: The play list and the weights

- **Inputs:** the `animate` list of a client entity (`client-entity-definition.md`,
  CENT-SCR-003), the controllers and the clips.
- **Behavior:**
  1. The client walks the `animate` list in its order.
  2. An entry that names a clip adds one clip instance.
  3. An entry that names a controller adds the instances of the current state of that
     controller, in the order of the state (EANIM-CTRL-001). This applies again for a
     controller inside a state.
  4. The weight of a clip instance is the product of the weights on its path: the weight of
     the `animate` entry, the weight of each state entry on the path, the cross-fade factor
     of each state on the path (EANIM-CTRL-003), and the `blend_weight` of the clip.
  5. The client evaluates each weight expression in each frame. An absent weight is 1.
  6. An instance with a weight of 0 is not in the play list. Its channels are not evaluated.
- **Postconditions:** each instance in the play list has a weight that is not 0.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1 for the `animate`
list of clips; 2 for controllers.

### EANIM-BLEND-003: Override of the previous pose

- **Inputs:** an instance of a clip with `override_previous_animation` true.
- **Behavior:**
  1. Before the instance applies a bone, the client sets the rotation of that bone to the rest
     rotation and the position to `(0, 0, 0)`.
  2. When the clip has a `scale` channel for that bone, the client also sets the scale to
     `(1, 1, 1)`.
  3. The client does this only for the bones that the clip has channels for.
- **Postconditions:** the instances before this one have no effect on these bones.

Provenance: `documentation` for 1; `documentation` (Blockbench) for 2 and 3. Kind: semantic.
Confidence: medium. Milestone: 2.

### EANIM-BLEND-004: Rotation relative to the entity

- **Inputs:** a bone with `relative_to.rotation` equal to `"entity"` in an instance.
- **Behavior:**
  1. The rotation part of the model transform of the bone does not include the rotations of
     its parent bones.
  2. The position of the pivot of the bone still follows the parent bones.
  3. The bone keeps this property for the frame when one instance in the play list sets it.
- **Postconditions:** the bone keeps its rotation in the space of the entity when its parent
  bone turns.

Provenance: `documentation` for 1; `design` for 2 and 3. Kind: semantic. Confidence: low.
Milestone: later.

---

## 5. Controllers

### EANIM-CTRL-001: The animation controller file

- **Inputs:** a file under `animation_controllers/` of the entity pack.
- **Behavior:**
  1. The root object has `format_version` and the object `animation_controllers`. Each key is
     a controller identifier, for example `controller.animation.frog.move`.
  2. A controller has `initial_state` (string, default `"default"`) and `states` (object).
  3. A state has these fields:

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `animations` | array | empty | entries: a short name, or an object `{ short name: weight expression }` |
| `transitions` | array | empty | entries: an object `{ state name: condition expression }` |
| `blend_transition` | number ≥ 0, seconds | `0` | the time of the cross-fade when the controller leaves the state |
| `blend_via_shortest_path` | bool | `false` | EANIM-CTRL-003 |
| `variables` | object | empty | EANIM-CTRL-004 |
| `on_entry`, `on_exit` | array of strings | empty | expressions (EANIM-CTRL-002) |
| `sound_effects`, `particle_effects` | array | empty | EANIM-FX-001 |

  4. A short name resolves through the `animations` map of the client entity
     (`client-entity-definition.md`, CENT-SCR-003). It can name a clip or a controller.
  5. When `initial_state` names no state, this is a content error. The client rejects the
     controller.
  6. A transition to a state that does not exist is a content error. The client ignores that
     transition.
- **Postconditions:** none.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 2.

### EANIM-CTRL-002: A frame of a controller

- **Inputs:** a controller instance with its current state `S`.
- **Behavior:**
  1. When the instance starts, its state is `initial_state`. The client runs `on_entry` of that
     state.
  2. In each frame, the client first sets the variables of `S` (EANIM-CTRL-004).
  3. The client evaluates the conditions of the transitions of `S` in their order. The first
     condition that is not 0 selects the state `S'`. The client evaluates no more conditions.
  4. When a state `S'` is selected:
     1. The client runs `on_exit` of `S`.
     2. The client starts the cross-fade from `S` (EANIM-CTRL-003).
     3. The current state becomes `S'`.
     4. The client runs `on_entry` of `S'`.
     5. The clip instances of `S'` start with the time 0.
  5. The controller changes its state at most one time in a frame.
  6. The client then adds the instances of the current state to the play list
     (EANIM-BLEND-002).
  7. The client joins the strings of `on_entry` and of `on_exit` into one expression, as
     `client-entity-definition.md`, CENT-SCR-002, states for scripts.
- **Postconditions:** a state that the controller enters in a frame adds its pose in the same
  frame.

Provenance: `documentation` for 1, 3 and 5; `design` for the order of 2, 3 and 6. Kind:
semantic. Confidence: medium. Milestone: 2.

### EANIM-CTRL-003: Cross-fade

- **Inputs:** a change from the state `S` with `blend_transition` `B > 0` to the state `S'`.
- **Behavior:**
  1. For the time `e` in seconds after the change, while `e < B`, the controller adds the
     instances of `S` and of `S'`.
  2. The cross-fade factor of `S` is `1 − e / B`. The cross-fade factor of `S'` is `e / B`.
  3. The instances of `S` continue to update their time during the cross-fade.
  4. When `B` is 0, there is no cross-fade.
  5. When the controller changes its state again during a cross-fade, the old cross-fade
     stops. The new cross-fade starts from the state that the controller leaves.
  6. The client ignores `blend_via_shortest_path` and logs it one time for each file.
- **Postconditions:** the pose changes without a jump when `B > 0`.

Provenance: `documentation` for 1, 2 and 4; `design` for 3, 5 and 6. Kind: semantic.
Confidence: low. Milestone: 2; later for `blend_via_shortest_path`.

### EANIM-CTRL-004: Variables of a state

- **Inputs:** the object `variables` of a state.
- **Behavior:**
  1. Each key is a name `n`. Each value has `input` (an expression) and `remap_curve` (an
     object from numbers as strings to numbers).
  2. The client sets `variable.n` to the remap of the value of `input`.
  3. The remap is linear between the two points of the curve around the value. Below the first
     point, the result is the value of the first point. Above the last point, the result is the
     value of the last point.
  4. When `remap_curve` is absent, the result is the value of `input`.
- **Postconditions:** none.

Provenance: `documentation`; `design` for 3 and 4. Kind: semantic. Confidence: medium.
Milestone: later.

### EANIM-CTRL-005: Finished animations

- **Inputs:** an expression of a state of a controller.
- **Behavior:**
  1. `query.any_animation_finished` is 1 when at least one clip instance of the current state is
     finished (EANIM-TIME-002). Otherwise it is 0.
  2. `query.all_animations_finished` is 1 when all clip instances of the current state are
     finished. Otherwise it is 0.
  3. A looping instance counts as finished after its first pass.
  4. An instance of a clip with `L = 0` counts as finished after its first frame.
  5. Outside a controller, the two queries are 0.
- **Postconditions:** none.

Provenance: `documentation` for 1 to 3 and 5; `unresolved` for 4. Kind: semantic. Confidence:
medium. Milestone: 2.

---

## 6. Effects

### EANIM-FX-001: Effects are not played

- **Inputs:** `sound_effects`, `particle_effects` or `timeline` of a clip, and
  `sound_effects` or `particle_effects` of a state.
- **Behavior:**
  1. The client reads these fields and checks their shape. A wrong shape is a content error.
  2. The client does not play a sound or a particle from them.
  3. The client does not evaluate the expressions of `timeline` and of `pre_effect_script`.
- **Postconditions:** an effect does not change the pose.

Provenance: `design`. Kind: semantic. Confidence: high. Milestone: 1. A later milestone replaces
this rule.

---

## 7. The processor and the pose tolerance

This document does not state which processor evaluates a slot, or when. A *slot* is a channel
of a clip, `anim_time_update` and `blend_weight` of a clip, and a weight expression of a play
list entry. The classification of slots and the transfer of values to the GPU are part of the
architecture (`docs/molang.md` and `docs/entity-render.md`). Only the result is specified.

### EANIM-GPU-003: The pose tolerance

- **Inputs:** a pose that an implementation computes, and the reference pose of the same
  inputs. The reference pose is the result of the rules of this document, with each Molang
  operation evaluated on the CPU as `molang.md` states.
- **Behavior:**
  1. The two poses must agree within these limits, for each bone and each axis of the pose:

| Value | Largest difference |
|-------|--------------------|
| rotation | `max(1e-3, 2⁻²⁰ · |r|)` radian for each Euler component, with `|r|` the larger of the two magnitudes |
| position | `max(1e-3, 2⁻²⁰ · |x|)` model unit (one model unit is one pixel, `1/16` block) |
| scale | `max(1e-4, 2⁻²⁰ · |x|)` |

     The relative term applies when one `f32` step of the value is larger than the absolute
     limit. That occurs above about 800 for a scale and above about 8000 for a rotation or a
     position.

  2. These limits apply whichever processor evaluates a slot, also when the client has no
     GPU compute and the CPU evaluates every slot.
  3. An implementation that cannot evaluate a slot of an entity type on the GPU, for example
     because the slots of the type exceed a GPU budget, evaluates the pose of that type on
     the CPU. The implementation must not replace the value of a slot with 0 or with another
     default. The full-rig cap of `client-entity-render.md`, CEREN-VIS-003, is a budget of
     another kind: over the cap, the client draws the entity as an impostor under that rule,
     and it does not compute the pose on the CPU.
  4. A locator, a leash point and a test use the reference pose.
  5. Each parity test of a pose cites this rule for its tolerance. The documents
     `client-entity-render.md`, `mob-pig.md` and `mob-frog.md` use these limits.
  6. The limits apply to an entity that the client draws with its full rig. An impostor of
     CEREN-VIS-003 has no pose to compare.
  7. A test does not apply the limits to a value at a discontinuity of an inexact function.
     A value is at a discontinuity when a change of `2⁻²⁰` relative to one input of the
     channel moves the reference value by more than the limit. Examples are `math.trunc`, a
     comparison or a branch on the result of `math.sin`, `math.cos`, `math.atan`,
     `math.atan2`, `math.asin`, `math.acos`, `math.exp`, `math.ln` or `math.pow`. The test
     reports the count of such values.
  8. A value that is subnormal on one processor and zero on the other agrees. The CPU does not
     flush subnormal inputs or results to zero; a GPU can.
- **Postconditions:** the pose that the player sees is the same within these limits, whichever
  processor evaluates a slot.

Provenance: `design` (the accuracy of the trigonometric functions of WGSL is about `2^−11`.
A vanilla walk clip multiplies a cosine by about 80°, which gives about `7e-4` radian. The
relative term of item 1 and item 7 come from the GPU fuzz of `mcrs_molang_wgsl`: above the
absolute limits one `f32` step is too large, and at a discontinuity no tolerance holds). Kind:
semantic. Confidence: medium. Milestone: 1.

---

## 8. Intentional deviations

### EANIM-DEV-001: Scale is multiplied

- **Rule:** EANIM-BLEND-001.
- **Reference:** the format reference states that channels add. It does not state the scale
  rule. Blockbench multiplies the scales, as EANIM-BLEND-001 states.
- **Visible to the client:** yes, only when two instances scale the same bone. The bone has a
  different size.
- **Test:** a differential test with two scale channels on one bone records the difference and
  passes.
