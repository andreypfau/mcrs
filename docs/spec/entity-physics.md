# Entity physics: collision and gravity

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

This document states how an entity moves in one tick. It states the bounding box, the collision
shapes and the collision algorithm. It also states gravity, drag, the block effects on motion, the
input of a player and the fall distance. It gives one model for players and for other entities. A
new entity type adds a row of data to the table of section 9, and does not add a new algorithm.

The vanilla client simulates the motion of its own player. Thus most rules for the player are of the
kind "client agreement", and these rules state the precision of each operation. The rules come from
the reference implementation. No experiment has confirmed them yet. Section 15 gives the experiments.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **clientbound**, **serverbound**: as `login.md`,
  section 0.1, defines them.
- **Tick**: one game tick. A vanilla server runs 20 ticks each second.
- **Entity**: an object of the world with a position. A player is an entity.
- **Living entity**: an entity with health and attributes, for example a player, a zombie or an
  armor stand.
- **Position**: the point `(x, y, z)` of an entity. It is at the centre of the bottom face of the
  box.
- **Velocity**: the displacement per tick that an entity keeps from one tick to the next. The
  data name of the velocity is `Motion`.
- **Box**: the axis-aligned bounding box of an entity, from `(xmin, ymin, zmin)` to
  `(xmax, ymax, zmax)`.
- **Dimensions**: the width, the height and the eye height of an entity type in one pose.
- **Pose**: the posture of an entity, for example `standing` or `crouching`.
- **Shape**: a union of axis-aligned boxes. A block state, an entity and the world border each
  can give a shape.
- **Empty shape**: a shape with no box.
- **Full cube**: the shape of one box from `(0, 0, 0)` to `(1, 1, 1)` relative to the block
  position.
- **Grid line**: on one axis of a shape, one coordinate of the grid that divides the shape into
  cells (section 3.2).
- **Cell**: the region between two adjacent grid lines on each of the three axes. A cell is
  solid or open.
- **Collider**: a shape that can stop the box of an entity.
- **Requested displacement**: the displacement that a mover asks for in one call of the move
  procedure (section 4).
- **Resolved displacement**: the part of the requested displacement that the colliders allow.
- **Clipped axis**: an axis on which the resolved displacement is not equal to the requested
  displacement.
- **Step-up**: the procedure that lifts an entity onto a low obstacle (section 4.3).
- **Step height**: the largest height of a step-up.
- **On-ground flag**, **horizontal collision flag**, **vertical collision flag**, **below
  collision flag**, **minor collision flag**: the five result flags of section 5.1.
- **Supporting block**: the block position that the move procedure records as the block under
  the entity (section 5.2).
- **Block friction**, **speed factor**, **jump factor**, **bounce restitution**: the four motion
  properties of a block (section 6.6).
- **Crowd damage**: the damage of the type `cramming` that an entity takes in a crowd (PHY-PSH-007).
- **Stuck multiplier**: a vector that a block sets on an entity inside it. The next move
  multiplies the requested displacement by it (section 6.5).
- **Fall distance**: the vertical distance that an entity has fallen after it last touched the
  ground (section 8).
- **Gravity**: the value that the entity subtracts from the vertical velocity in one tick.
- **Drag**: a factor that multiplies a component of the velocity at the end of a tick.
- **Input vector**: the movement intent of a living entity in its own frame: a sideways
  component, a vertical component and a forward component.
- **Shift key state**: the state of the shift key of a player. A player holds this key to sneak.
  The server knows the state from the `player_input` packet.
- **Flight speed**: the flight speed value of the `player_abilities` packet.
- **Fly permission**: the permission of a player to start the flight state. Bit 0x04 of the
  clientbound `player_abilities` packet gives it.
- **Sprint state**: the state of a player that sprints.
- **Flight state**: the state of a player that flies with the abilities of creative mode or
  spectator mode.
- **Crouch state**, **swim state**: the state of a player that crouches (PHY-INP-010), and the state
  of a player that swims. The fluid model sets the swim state (section 12).
- **Sprint timer**, **jump timer**: the two counters of the client for a double press of the
  forward key and of the jump key (PHY-INP-017, PHY-INP-018).
- **Item use state**: the state of an item use of a player (PHY-INP-019).
- **Jump input**: the intent of a living entity to jump in this tick. For a player, it is the jump
  key.
- **Jump delay**: the number of ticks before a living entity can jump again (PHY-INP-004).
- **Long-jump state**: the state of an entity whose AI does a long jump, for example a goat, a frog
  or a breeze (PHY-GRV-007).
- **Mover kind**: the source of one call of the move procedure (PHY-MOV-001).
- **Edge back-off**: the change of a requested displacement that keeps a player on a block edge
  (PHY-INP-012).
- **Query box**: the box for which a collision query finds the colliders (PHY-SHP-007).
- **Support position**, **effect position**, **movement position**: three block positions under an
  entity (PHY-FLG-005).
- **Effective gravity**: the gravity after the effect `slow_falling` (PHY-GRV-002).
- **Ground friction**: the friction factor of the ground under a living entity (PHY-GRV-006).
- **Horizontal air drag**, **vertical air drag**: the two drag factors of a living entity in air
  (PHY-GRV-005).
- **Input acceleration**: the change of the velocity that comes from the input vector
  (PHY-GRV-010, PHY-INP-002).
- **Air speed**: the speed of the input acceleration when the entity is not on the ground
  (PHY-GRV-011).
- **Restitution**: the factor that keeps a part of the velocity after a collision (PHY-RST-001).
- **Step action**, **bounce action**, **fall action**: the action of a block on an entity. The
  entity steps on the block, bounces on it or falls on it. This document states only one of these actions: the
  step action of the slime block (PHY-GRV-018).
- **Category**: a set of entity types that use the same parameters and the same order of steps
  (PHY-ORD-005).
- **Omnidirectional air mover**: a living entity that uses the horizontal air drag on all axes
  (PHY-GRV-005).
- **Base entity tick**: the part of the tick that each entity does before its motion. It updates
  the fluid state (section 12).
- **AI step**: the part of the tick of a living entity that reads the input and moves the entity.
  It is steps 4 to 15 of PHY-ORD-001, and steps 3 to 8 of PHY-ORD-002.
- **Travel step**: the step of the AI step that moves the entity (PHY-ORD-003, PHY-ORD-004).
- **Block effects step**: the step after the travel step that applies the effects of blocks
  (step 14 of PHY-ORD-001).
- **Local authority**: the side that simulates the motion of an entity. The client has local
  authority for its own player. The server has local authority for every other entity, except an
  entity that a player controls.
- **Server-simulated entity**: an entity for which the server has local authority.
- **External impulse**: a change of the velocity that does not come from this model, for example
  knockback or an explosion (section 11.3).
- **f32**, **f64**: an IEEE 754 binary32 value and an IEEE 754 binary64 value.
- **Widen**: convert an f32 value to the f64 value that is equal to it.
- **`f32(v)`**: the f32 value nearest to `v`, with ties to even.
- **Data pack**, **corpus**: as `registry.md`, section 0.4, defines them.

### 0.2 Provenance

- `black_box_confirmed`: shown by a captured result in section 16.
- `source_suggested_black_box_confirmed`: taken from the reference implementation, and consistent
  with a captured result in section 16.
- `source_suggested`: taken from the reference implementation, with no captured result. The
  confidence is at most medium. Section 15 attaches an experiment.
- `unresolved`: not decided. Section 14 collects these items.

Each rule states its kind and its confidence, for example
`(source_suggested, client agreement, confidence: medium)`. The kinds are those of
`docs/spec/README.md`. An invariant (section 10) has the kind "semantic".

### 0.3 ID prefix

Each rule has an ID with the prefix `PHY`. The second part names the section.

| Prefix | Subject |
|--------|---------|
| `PHY-NUM` | numbers and precision |
| `PHY-BOX` | bounding box and dimensions |
| `PHY-SHP` | collision shapes and colliders |
| `PHY-MOV` | the move procedure |
| `PHY-COL` | the collision algorithm |
| `PHY-STP` | step-up |
| `PHY-FLG` | result flags and the supporting block |
| `PHY-RST` | velocity after a collision |
| `PHY-GRV` | gravity, drag and block effects |
| `PHY-INP` | player input, jump, sneak and flight |
| `PHY-FALL` | fall distance |
| `PHY-ORD` | order of operations and entity categories |
| `PHY-INV` | invariants |
| `PHY-SRV` | the role of the server |
| `PHY-RCH` | the causal reach of one tick |
| `PHY-SKIP` | ticks that an implementation can skip |
| `PHY-CMP` | the compatibility policy of each category |
| `PHY-BLK` | writes of the block domain to entities |
| `PHY-EXP` | the timing of explosions |
| `PHY-DEV` | intentional deviations |

### 0.4 Scope

This document states:

- the motion of an entity on land and in air, for a player and for other entities;
- the hooks where another model replaces this one.

These subjects are out of scope:

- fluids;
- elytra flight and riptide;
- vehicles and riders;
- the motion that a piston gives;
- fall damage, which `entity-damage.md` states;
- the hit detection of projectiles;
- the movement checks of the server.

Section 12 lists the hooks of these subjects. The movement checks are in `movement-checks.md`, with
the prefix `MVC`.

### 0.5 Artefacts cited

- `assets/mcrs/block_definition/<block>.json`: the block definition corpus. The component
  `minecraft:collision_box` gives the collision shape of each block state for an entity that has
  no context (PHY-SHP-002). The component `minecraft:friction` gives the block friction.
- The data pack: the block tags `climbable`, `fall_damage_resetting`, `fences`, `walls`,
  `suppresses_bounce` and `can_glide_through`, and the entity type tags
  `powder_snow_walkable_mobs` and `fall_damage_immune`.
- `assets/minecraft/version.json`: the version of the corpus. When the author wrote this document,
  the file stated `26.4-snapshot-4`, not the game version of this document. Section 14 records
  this as U1.

No artefact in the repository gives the dimensions of an entity type, the speed factor, the jump
factor or the bounce restitution of a block. This document states these values as data where it
needs them. Section 14 records the gap as U2 and U3.

## 1. Numbers and precision

**PHY-NUM-001** (`source_suggested`, client agreement, confidence: medium). The position, the
velocity, the box, a displacement, a shape coordinate and the fall distance are f64 values. An
implementation must do each operation on them in f64, in the order that this document states.

**PHY-NUM-002** (`source_suggested`, client agreement, confidence: medium). Some constants are f32
values that the computation widens to f64. A rule writes such a constant as `0.91F`. The f64
computation uses the widened value, not the decimal value. For example, `0.91F` is
`0.9100000262260437`. The table gives the widened values of the constants of this document.

| Constant | Widened f64 value |
|----------|-------------------|
| `0.6F` | `0.6000000238418579` |
| `0.91F` | `0.9100000262260437` |
| `0.98F` | `0.9800000190734863` |
| `0.99F` | `0.9900000095367432` |
| `0.8F` | `0.800000011920929` |
| `0.9F` | `0.8999999761581421` |
| `0.15F` | `0.15000000596046448` |
| `0.1F` | `0.10000000149011612` |
| `0.3F` | `0.30000001192092896` |
| `0.42F` | `0.41999998688697815` |
| `0.02F` | `0.019999999552965164` |
| `0.025999999F` | `0.025999998673796654` |
| `0.05F` | `0.05000000074505806` |
| `0.03F` | `0.029999999329447746` |
| `0.2F` | `0.20000000298023224` |
| `0.4F` | `0.4000000059604645` |
| `0.95F` | `0.949999988079071` |
| `0.989F` | `0.9890000224113464` |
| `0.500001F` | `0.5000010132789612` |
| `1.0E-5F` | `9.999999747378752E-6` |
| `0.21600002F` | `0.2160000205039978` |
| `0.13962634F` | `0.13962633907794952` |
| `1.62F` | `1.6200000047683716` |
| `1.8F` | `1.7999999523162842` |
| `1.27F` | `1.2699999809265137` |

A constant without the suffix `F`, for example `0.08` or `0.003`, is an f64 value.

**PHY-NUM-003** (`source_suggested`, client agreement, confidence: medium). A product or a sum of
two f32 values is an f32 operation. The computation widens its result only when the result enters
an f64 operation. For example, the horizontal drag on the ground is `f32(fb × 0.91F)`, where `fb`
is the block friction. The computation then multiplies the f64 velocity component by the widened
product.

**PHY-NUM-004** (`source_suggested`, client agreement, confidence: medium). An attribute value is
an f64 value. Where a rule converts an attribute value to f32, the rule writes `f32(attribute)`.

**PHY-NUM-005** (`source_suggested`, client agreement, confidence: medium). The rotation of an
input vector uses a sine table, not the exact sine. The functions `sinT` and `cosT` of this rule
read the table.

1. The table has 65536 f32 entries. Entry `i` is `f32(sin(i / 10430.378350470453))`.
2. The entries 0, 16384, 32768 and 49152 are 0, 1, 0 and -1.
3. For an f64 angle `a` in radians, `sinT(a)` is entry `k`, with
   `k = truncate(a × 10430.378350470453 + 0.5) mod 65536`, when `a ≥ 0`.
4. When `a < 0`, `sinT(a)` is the negative of the entry for `-a`.
5. `cosT(a)` is entry `truncate(|a| × 10430.378350470453 + 16384 + 0.5) mod 65536`.

The angle from a yaw `θ` in degrees is `f32(θ × f32(π / 180))`. The computation does this
product in f32, and then widens it.

**PHY-NUM-006** (`source_suggested`, client agreement, confidence: medium). These epsilons occur
in the model. Each rule names the epsilon that it uses.

| Value | Use |
|-------|-----|
| `1.0E-7` | the tolerance of the collision algorithm (PHY-COL-003), the drop of a short move (PHY-MOV-004), the scan of blocks (PHY-SHP-007), the stuck multiplier (PHY-MOV-002) |
| `1.0E-5F` | the horizontal collision flag (PHY-FLG-001), the downward extension of a step-up (PHY-STP-002) |
| `1.0E-6` | the depth of the area under the box that finds the supporting block (PHY-FLG-004) |
| `0.003` | the zero cutoff of a small velocity component (PHY-GRV-008) |

**PHY-NUM-007** (`source_suggested`, client agreement, confidence: medium). Each operation of this
model is an f64 operation, except the operations in this list. These are f32 operations:

- the dimensions: the width, the height, the eye height, the half width and their scale
  (PHY-BOX-001, PHY-BOX-003, PHY-BOX-005);
- the step height and the candidate heights of a step-up (PHY-STP-001, PHY-STP-004);
- the yaw angle in radians and the entries of the sine table (PHY-NUM-005);
- the input vector of a player, from the keys to the end of PHY-INP-001;
- the input of another living entity and its decay by `0.98F` (PHY-INP-003);
- the speed of the input acceleration (PHY-GRV-010, PHY-GRV-011, PHY-INP-007, PHY-INP-014);
- the ground friction, the air drags, the product of friction and drag, and `mod` (PHY-GRV-005 to
  PHY-GRV-007);
- the speed factor of a living entity after the `movement_efficiency` mix (PHY-GRV-012);
- the jump power (PHY-INP-005);
- the vertical flight impulse factor, the flight speed times `3.0F` (PHY-INP-013);
- the block bounce restitution of an entity, `f32(restitution × k)` (PHY-RST-003);
- the speed multiplier of an item use and the `sneaking_speed` factor (PHY-INP-001, PHY-INP-019);
- the rotation values of a teleport (PHY-SRV-010).

An f32 result enters an f64 operation only after the computation widens it. A comparison between
an f32 value and an f64 value widens the f32 value. A value in a table of section 9 that ends in
`F` is an f32 constant.

## 2. Bounding box

**PHY-BOX-001** (`source_suggested`, client agreement, confidence: medium). The box of an entity
at the position `(x, y, z)`, with the width `w` and the height `h` (both f32), is:

- `xmin = x - hw`, `xmax = x + hw`, where `hw = f32(w / 2)`, widened;
- `ymin = y`, `ymax = y + h`, with `h` widened;
- `zmin = z - hw`, `zmax = z + hw`.

**PHY-BOX-002** (`source_suggested`, client agreement, confidence: medium). After each change of
the position, the entity makes its box again from the new position by PHY-BOX-001. The box does
not move by a displacement. Thus the box after a move is the box at `position + resolved`, not the
old box plus the resolved displacement.

**PHY-BOX-003** (`source_suggested`, client agreement, confidence: medium). The dimensions of a
living entity in a pose are the dimensions of its type in that pose, scaled by the `scale`
attribute. Let `s = f32(scale)`. When the dimensions are scalable and `s ≠ 1`, the entity
multiplies the width, the height and the eye height by `s` in f32. Fixed dimensions do not scale.

The `sleeping` pose always has fixed dimensions with the width 0.2F, the height 0.2F and the eye
height 0.2F. A baby of a living type has the dimensions of its type scaled by 0.5F. This scale
applies before the scale attribute, unless the type states other baby dimensions.

**PHY-BOX-004** (`source_suggested`, client agreement, confidence: medium). The player has these
dimensions before the scale attribute:

| Pose | Width | Height | Eye height | Fixed |
|------|-------|--------|-----------|-------|
| `standing` | 0.6F | 1.8F | 1.62F | no |
| `crouching` | 0.6F | 1.5F | 1.27F | no |
| `swimming` | 0.6F | 0.6F | 0.4F | no |
| `fall_flying` | 0.6F | 0.6F | 0.4F | no |
| `spin_attack` | 0.6F | 0.6F | 0.4F | no |
| `sleeping` | 0.2F | 0.2F | 0.2F | yes |
| `dying` | 0.2F | 0.2F | 1.62F | yes |
| other poses | as `standing` | | | |

**PHY-BOX-005** (`source_suggested`, client agreement, confidence: medium). The eye height of a
type without a stated eye height is `f32(0.85F × height)`.

**PHY-BOX-006** (`source_suggested`, client agreement, confidence: medium). At the end of its tick,
a player selects its pose:

1. Find the desired pose. It is the first pose of this list that applies:
   - `sleeping`, when the player sleeps;
   - `swimming`, when the player has the swim state;
   - `fall_flying`, during elytra flight;
   - `spin_attack`, during a riptide attack;
   - `crouching`, when the shift key state is on and the flight state is off;
   - `standing`, in all other cases.
2. If the box of the `swimming` pose at the position has a collision, do not change the pose.
3. Else, if the player is a spectator or a passenger, the pose is the desired pose.
4. Else, if the box of the desired pose has no collision, the pose is the desired pose.
5. Else, if the box of the `crouching` pose has no collision, the pose is `crouching`.
6. Else, the pose is `swimming`.

This rule shrinks each box by `1.0E-7` on each side before the test. A "collision" in this rule is
any block shape, any entity collider or the world border that intersects the box (PHY-SHP-001 to
PHY-SHP-011).

**PHY-BOX-007** (`source_suggested`, semantic, confidence: medium). When the dimensions of an
entity change, the entity makes its box again from its position. The server can then move a
server-simulated entity to a near free position. It can do this only when all these conditions are
true:

- the new box is larger;
- the width and the height are not more than 4;
- the entity is not a player;
- the entity is not in its first tick.

This document does not state that search. Section 14 records it as U9.

**PHY-BOX-008** (`unresolved`). The dimensions of every other entity type are data of the type.
Section 9 gives the dimensions of the categories that it uses. Section 14 records the absent
artefact as U2.

## 3. Collision shapes and colliders

### 3.1 Block shapes

**PHY-SHP-001** (`source_suggested`, client agreement, confidence: medium). The collision shape of
a block state at a block position is the shape of the state, moved by the block position. A
block state can give an empty shape, the full cube or another union of boxes.

**PHY-SHP-002** (`source_suggested`, client agreement, confidence: medium). The component
`minecraft:collision_box` of the corpus gives the shape of a state for an entity that has no
context. Each entry is one box in pixels, with these conversions to block units:

- `min.x = (origin[0] + 8) / 16`, `min.y = origin[1] / 16`, `min.z = (origin[2] + 8) / 16`;
- `max = min + size / 16`, on each axis.

An empty array is the empty shape. The corpus rounds each value to `1.0E-4` pixel. Section 14
records this as U4.

**PHY-SHP-003** (`source_suggested`, client agreement, confidence: medium). Some shapes depend on
the entity. For these states the corpus gives only the shape for no context. In the table, `by`
is the `y` of the block position.

| Block | Shape for an entity |
|-------|---------------------|
| `scaffolding` | If the bottom of the entity is above the top of the block cube by more than `1.0E-5F` (the bottom `y` is more than `by + 1 - 1.0E-5F`) and the shift key state is off: the stable shape, the top slab from `y = 14/16` to 1 and the four corner posts of 2 × 2 pixels. Else, if the state has `distance` not 0 and `bottom` true, and the bottom of the entity is more than `by - 1.0E-5F`: the slab from `y = 0` to `2/16`. Else: the empty shape. |
| `powder_snow` | If the fall distance of the entity is more than 2.5: the box from `(0, 0, 0)` to `(1, 0.9F, 1)`. Else, if the flag `powder_snow_solid` of the entity type is true (PHY-ORD-009): the full cube. Else, if the entity can walk on powder snow (PHY-GRV-016), its bottom is more than `by + 1 - 1.0E-5F` and its shift key state is off: the full cube. Else: the empty shape. |
| `water`, `lava` (source state) | The empty shape, except for an entity that can stand on that fluid. That case belongs to the fluid model (section 12). |

**PHY-SHP-004** (`unresolved`). Some shapes depend on a block entity or on the position:

- a `moving_piston` gives the shape of the block that it moves;
- a shulker box gives a shape that grows while the lid opens. PHY-SHP-014 states this shape;
- a block with a position offset has a shape that moves by an offset from a hash of the block
  position. Examples are `bamboo`, `pointed_dripstone`, `icicle` and `sulfur_spike`.

The corpus gives these shapes at the probe position `(0, 128, 0)` with no block entity. Section 14
records this as U5.

### 3.2 Grid lines

**PHY-SHP-005** (`source_suggested`, client agreement, confidence: medium). The collision
algorithm and the step-up use the grid lines of a shape, not only the faces of its boxes. A box
goes from `a` to `b` on each axis. Its grid lines come from these steps:

1. On each axis, find the smallest `n` from 0 to 3 that puts `a × 2^n` and `b × 2^n` near an
   integer. Near means less than `1.0E-7 × 2^n` from it.
2. On an axis with `a < -1.0E-7` or `b > 1.0000001`, no `n` exists.
3. If no `n` exists on one axis, the grid lines on each axis are only `a` and `b`.
4. Else, the grid lines on each axis are the multiples of `1/2^n` of that axis, from 0 to 1.

The largest `n` is 3. Thus a face that is not on the grid of `1/8` gives step 3. For example, a
carpet goes from `y = 0` to `1/16`. Its grid lines are 0 and `1/16` on `y`, and 0 and 1 on `x`
and `z`. Another box goes from `x = 0.25` to `0.75` and from `y = 0` to `1/16`. Its grid lines on
`x` are only 0.25 and 0.75.

Step 3 also removes the grid lines of the axes that have an `n`. The client does this. This rule
keeps it for client agreement.

The game builds a shape from boxes. The grid lines of a shape on an axis are the union of the grid
lines of these boxes. Two values that are `1.0E-7` or less apart are one grid line. The union
keeps the lowest value of such a group. A cell is solid when it is inside a box of the shape.

The boxes of the corpus do not always give the boxes from which the game built the shape. Section
14 records this as U6.

To make the union, sort the values from the lowest to the highest. Keep the first value. Compare
each next value `w` with the last value `k` that the union kept. If `w - 1.0E-7 ≤ k`, `w` is the
grid line `k`. Else, keep `w`.

For example, the bottom slab from `y = 0` to `0.5` has the grid lines 0, 0.5 and 1 on `y`. The cell
from 0.5 to 1 is open.

**PHY-SHP-006** (`source_suggested`, client agreement, confidence: medium). The shape of an
entity box gets its grid lines from PHY-SHP-005, as one box, with the world coordinates of the
box. Usually, this gives two grid lines on each axis: the minimum and the maximum of the box. Step
4 of PHY-SHP-005 applies only to a box from `-1.0E-7` to `1.0000001` on all three axes. Its faces must also be on the grid of `1/2^n` on each axis. Then the grid lines
are the multiples of `1/2^n` from 0 to 1.

A box with an edge shorter than `1.0E-7` gives the empty shape. A block box for which step 3 of
PHY-SHP-005 applies has the two grid lines `a` and `b` on each axis.

### 3.3 Colliders of one move

**PHY-SHP-007** (`source_suggested`, client agreement, confidence: medium). The block colliders
for a query box `Q` come from this scan:

1. Let `x0 = floor(Q.xmin - 1.0E-7) - 1` and `x1 = floor(Q.xmax + 1.0E-7) + 1`. Find `y0`, `y1`,
   `z0` and `z1` in the same way.
2. Visit each block position in the region from `(x0, y0, z0)` to `(x1, y1, z1)`, with both ends
   included. `x` changes fastest, then `y`, then `z`.
3. On a position on one outer face of the region, use the block only if it has a large shape.
4. On a position on an outer edge of the region, use the block only if it is a `moving_piston`.
5. Ignore a position on an outer corner of the region.
6. Ignore a position in a chunk that is not loaded.
7. Use the collision shape of the block for the entity (PHY-SHP-003).
8. Keep the shape only if it intersects `Q` with a volume that is more than zero.

An outer edge is on two outer faces of the region. An outer corner is on three outer faces. A block
state has a large shape when its collision shape for no context goes out of the unit cube on an
axis. Fences, walls and fence gates have a large shape.

**PHY-SHP-008** (`source_suggested`, client agreement, confidence: medium). This rule finds the
entity colliders for an entity `E` and a query box `Q`. Grow `Q` by `1.0E-7` on each side. The
candidates are the other entities whose box intersects the grown box. An entity `O` is a collider
for `E` when all these conditions are true:

- `O` is not `E`, and `O` is not a spectator;
- `E` can collide with `O` (PHY-SHP-009);
- `E` and `O` do not ride the same root vehicle.

The shape of a collider entity is its box. The mean edge of `Q` is `(xsize + ysize + zsize) / 3`.
When the mean edge is less than `1.0E-7`, there are no entity colliders.

**PHY-SHP-009** (`source_suggested`, semantic, confidence: medium). Another entity can collide with
the entities of this table. It cannot collide with each other vanilla entity. A new entity type
states this as data.

| Entity | Condition |
|--------|-----------|
| boats and rafts | always |
| `shulker` | while alive |
| `happy_ghast` | an adult that is alive, when it has a still timeout, or when it carries a rider and `E` is a happy ghast. On the client, also when `E` is a player whose `y` is at or above the top of the box. |

A minecart and a boat also use a pushable entity as a collider for themselves. That rule is part
of the vehicle model (section 12).

**PHY-SHP-010** (`source_suggested`, client agreement, confidence: medium). The world border is a
collider for an entity `E` and a query box `Q` when both conditions are true:

- the distance from the position of `E` to the nearest border line, in `x` and `z`, is less than
  `2m`;
- the position of `E` is inside the border grown by `m` on each side.

Here `m` is the largest of `|Q.xsize|`, `|Q.zsize|` and 1. The distance is the smallest of
`x - xmin`, `xmax - x`, `z - zmin` and `zmax - z`, with the values of the border. It is less than
zero outside the border. The position is inside the grown border when `x ≥ xmin - m`,
`x < xmax + m`, `z ≥ zmin - m` and `z < zmax + m`. Thus the minimum side includes its line, and
the maximum side does not.

The shape of the border is all space outside a region, at all heights. On `x`, the region goes from `floor(xmin)` to `ceil(xmax)` of the
border. On `z`, it goes from `floor(zmin)` to `ceil(zmax)` of the border. The border values are
those of the current tick.

**PHY-SHP-011** (`source_suggested`, client agreement, confidence: low). The colliders of one
query are in this order: the entity colliders, then the world border, then the block colliders in
the order of PHY-SHP-007. The order changes the result only through the early exit of
PHY-COL-004.

**PHY-SHP-012** (`source_suggested`, client agreement, confidence: medium). The collision query
gives each collider with its source. A source is one of these:

- `block`, with the block position and the block state;
- `entity`, with the entity;
- `border`, for the world border.

The query takes the entity `E`, the query box `Q` and an optional feet height `yf`. Without `yf`,
the block shapes for `E` use the `y` of the position of `E` as the bottom of the entity
(PHY-SHP-003). With `yf`, they use `yf` in place of that value. For example, a server check can
give the `y` of the old position. Then a `scaffolding` or `powder_snow` shape is the shape from
before the move.

In the reference, a query with `yf` is also a placement query. A placement query gives the empty
shape for `scaffolding` and `powder_snow`, and has no `border` source. A checker that copies the
reference must use these two differences.

**PHY-SHP-013** (`source_suggested`, client agreement, confidence: medium). An intersection test
between a box and a shape, outside the collision algorithm, uses the box shrunk by `1.0E-5F` on
each side. The test widens this value. The test is true when the shrunk box and the shape overlap
with a volume that is more than zero. The scan of PHY-SHP-007 uses the shrunk box as `Q`.

For example, take a test that finds a shape that the new box touches and the old box does not
touch. This test shrinks the new box and the old box by `1.0E-5F`.

**PHY-SHP-014** (`source_suggested`, client agreement, confidence: medium). The collision shape of
a shulker box depends on the open progress `p` of its block entity. The shape is one box: the full
cube, grown by `e = f32(0.5F × p)` in the direction of the property `facing`. `e` is widened. A
shulker box without a block entity has the full cube.

The progress is an f32 value from 0 to 1. The block entity has one of four states, and it
changes `p` in each of its ticks:

- `closed`: `p` is 0;
- `opening`: add `0.1F` to `p`. At 1 or more, `p` changes to 1, and the state changes to `open`;
- `open`: `p` is 1;
- `closing`: subtract `0.1F` from `p`. At 0 or less, `p` changes to 0, and the state changes to
  `closed`.

The box starts to open when the first player opens it, and starts to close when the last player
closes it. The server sends each change of the count of players to the client in a
`block_event`. The client then computes the same progress, so this rule is client agreement.

An implementation that does not know the state of the block entity must not use the full cube. It
must treat a tick that can touch the box as coarse (PHY-ORD-008). It can use the full cube only
when it knows that the box is closed. This applies to the player and to each server-simulated
entity, because each client shows the shape that this rule gives.

## 4. The move procedure

### 4.1 Overview

**PHY-MOV-001** (`source_suggested`, client agreement, confidence: medium). A call of the move
procedure has a mover kind and a requested displacement `d`. The mover kinds are `self`,
`player`, `piston`, `shulker` and `shulker_box`. The tick of an entity calls the procedure with
`self`. The server calls it with `player` for the displacement that a client sends (section 11).

The kinds `piston` and `shulker_box` come from the block domain (PHY-BLK-002). The kind
`shulker` comes from a shulker that opens.

The procedure has these steps, in this order:

1. If the entity has no physics, add `d` to the position, set the four collision flags to false,
   and stop. A spectator player has no physics.
2. If the kind is `piston`, limit `d` by the piston model (section 12). Stop if the result is zero.
3. Apply the stuck multiplier (PHY-MOV-002).
4. Apply the edge back-off of a player (PHY-INP-012).
5. Resolve `d` against the colliders, with step-up (PHY-COL-001 to PHY-STP-005).
6. Apply the resolved displacement to the position (PHY-MOV-004).
7. Set the result flags and the supporting block (PHY-FLG-001 to PHY-FLG-005).
8. Change the velocity for the collision (PHY-RST-001 to PHY-RST-004).
9. Multiply the horizontal velocity by the speed factor (PHY-GRV-012).

Steps 5 to 9 do not change when the requested displacement is zero.

**PHY-MOV-002** (`source_suggested`, client agreement, confidence: medium). When the squared length
of the stuck multiplier `m` is more than `1.0E-7`:

1. If the kind is not `piston`, multiply `d` by `m`, component by component.
2. Set `m` to zero.
3. Set the velocity to zero.

**PHY-MOV-003** (`source_suggested`, client agreement, confidence: medium). The stuck multiplier
comes from the block effects of the previous tick (PHY-GRV-014). Thus it changes the move of the
next tick, and that move also clears the velocity.

**PHY-MOV-004** (`source_suggested`, client agreement, confidence: medium). Let `r` be the resolved
displacement. The position changes to `position + r` only when one of these is true:

- `|r|² > 1.0E-7`;
- `|d|² - |r|² < 1.0E-7`.

Else the position does not change. Thus the procedure drops a short resolved move when the
colliders removed most of a longer request. The flags of step 7 still use `r`.

### 4.2 The collision algorithm

**PHY-COL-001** (`source_suggested`, client agreement, confidence: medium). The colliders of a move
come from the query box: the box of the entity grown towards `d`. To grow a box towards a vector,
move one face for each component of the vector. A negative component moves the minimum face, and a
positive component moves the maximum face. The entity colliders use a larger query: the box grown
towards `d`, then grown upward by the step height (PHY-STP-001). The step-up uses the same entity
colliders.

If `|d|²` is zero, the resolved displacement is `d`, and the step-up does not occur.

**PHY-COL-002** (`source_suggested`, client agreement, confidence: medium). The algorithm resolves
one axis at a time. The order of the axes is:

- `y`, then `z`, then `x`, when `|d.x| < |d.z|`;
- `y`, then `x`, then `z`, in all other cases.

For each axis in this order:

1. If the component of `d` on the axis is exactly zero, the resolved component is `+0.0`. This
   is also true for a component of `-0.0`.
2. Else, move the box by the resolved components of the earlier axes. Clip the component against
   each collider (PHY-COL-003 and PHY-COL-004). The result is the resolved component.

If the list of colliders is empty, the resolved displacement is `d`. This case and the case
`|d|² = 0` of PHY-COL-001 keep each component of `d` with its sign. Thus a component of `-0.0`
stays `-0.0` only in these two cases. The sign changes the position only when the position is
exactly `-0.0`. The client does this. This rule keeps it for client agreement.

**PHY-COL-003** (`source_suggested`, client agreement, confidence: medium). This rule clips a
distance `t` on an axis `A` against one shape `S`, for the box `E`. Let `B` and `C` be the other
two axes. Let `g0 < g1 < …` be the grid lines of `S` on each axis.

1. If `S` is empty, the result is `t`.
2. If `|t| < 1.0E-7`, the result is 0.
3. A cell row `k` on axis `B` is in the cross-section when `g_k ≤ E.bmax - 1.0E-7` and
   `g_(k+1) > E.bmin + 1.0E-7`. Use the same test on axis `C`.
4. If `t > 0`, examine the cell layers `a` on axis `A` with `g_a > E.amax - 1.0E-7`. Examine them
   from the lowest to the highest. At the first layer with a solid cell in the cross-section, the
   result is `min(t, g_a - E.amax)`. If no layer has one, the result is `t`.
5. If `t < 0`, examine the cell layers `a` with `g_(a+1) ≤ E.amin + 1.0E-7`. Examine them from the
   highest to the lowest. At the first layer with a solid cell in the cross-section, the result is
   `max(t, g_(a+1) - E.amin)`. If no layer has one, the result is `t`.

A cell that the box already overlaps on axis `A` by more than `1.0E-7` does not stop the box. Thus
a shape of one cell does not stop a box that already penetrates it.

**PHY-COL-004** (`source_suggested`, client agreement, confidence: medium). To clip a distance
against a list of colliders, apply PHY-COL-003 to each collider in the order of PHY-SHP-011. The
result of one collider is the distance for the next. Before each collider, examine the distance.
If its absolute value is less than `1.0E-7`, the result is exactly 0, and the colliders after it
do not apply.

**PHY-COL-005** (`source_suggested`, client agreement, confidence: medium). For a positive `t`,
step 4 of PHY-COL-003 can give a result that is less than zero by up to `1.0E-7`. For a negative
`t`, step 5 can give a result that is more than zero by up to `1.0E-7`. An implementation must keep
these results. They do not change to zero, except by the early exit of PHY-COL-004.

### 4.3 Step-up

**PHY-STP-001** (`source_suggested`, client agreement, confidence: medium). The step height of a
living entity is `f32(step_height)`. A living entity that a player controls has a step height of
at least 1.0. Each other entity has a step height of 0, as data of its category (section 9).

The base value of the attribute is data of the entity type. The default of the attribute is 0.6,
for a type that does not give a base value. These types give a base value:

| Type | Base value of `step_height` |
|------|-----------------------------|
| `armor_stand` | 0 |
| `turtle`, `frog`, `axolotl`, `iron_golem`, `copper_golem`, `enderman`, `ravager`, `drowned` | 1.0 |
| `horse`, `donkey`, `mule`, `llama`, `trader_llama`, `skeleton_horse`, `zombie_horse` | 1.0 |
| `creaking` | 1.0625 |
| `camel`, `camel_husk` | 1.5 |

A `sulfur_cube` that carries a block has a step height of 0.

**PHY-STP-002** (`source_suggested`, client agreement, confidence: medium). Let `r` be the resolved
displacement of PHY-COL-002, and let `h` be the step height. Let `G` be true when `r.y` differs
from `d.y` and `d.y < 0`. A step-up occurs when all these conditions are true:

- `h > 0`;
- `G` is true, or the on-ground flag from before this move is true;
- `r.x` differs from `d.x`, or `r.z` differs from `d.z`. The comparison is exact.

The base box is the box moved by `(0, r.y, 0)` when `G` is true, else the box. The step area is the
base box grown towards `(d.x, h, d.z)`. When `G` is false, the step area also grows downward by
`1.0E-5F`.

**PHY-STP-003** (`source_suggested`, client agreement, confidence: medium). The step-up uses these
colliders:

- the entity colliders of PHY-COL-001;
- the world border for the step area (PHY-SHP-010);
- the block colliders for the step area.

**PHY-STP-004** (`source_suggested`, client agreement, confidence: medium). The candidate heights
come from the `y` grid lines of each step collider:

1. For each collider, examine its `y` grid lines from the lowest to the highest.
2. Let `c = f32(line - base.ymin)`.
3. Ignore `c` when `c < 0`, or when `c` is equal to `f32(r.y)`.
4. When `c > f32(h)`, stop the examination of this collider.
5. Else, add `c` to the set of candidates. The set keeps one copy of equal values.

Sort the candidates from the lowest to the highest.

**PHY-STP-005** (`source_suggested`, client agreement, confidence: medium). For each candidate `c`,
from the lowest to the highest:

1. Widen `c`. Resolve the displacement `(d.x, c, d.z)` from the base box against the step
   colliders, by PHY-COL-002.
2. Let `s` be the result. If `s.x² + s.z²` is more than `r.x² + r.z²`, the result of the move is
   `s - (0, box.ymin - base.ymin, 0)`. Stop.

If no candidate gives more horizontal distance, the result of the move is `r`. Thus the step-up
selects the lowest candidate that increases the horizontal distance, not the highest one.

## 5. Result flags and the velocity after a collision

### 5.1 Flags

**PHY-FLG-001** (`source_suggested`, client agreement, confidence: medium). After a move with a
requested displacement `d` (after PHY-MOV-002 and PHY-INP-012) and a resolved displacement `r`:

- The `x` axis is clipped when `|d.x - r.x| ≥ 1.0E-5F`. The `z` axis uses the same test.
- The horizontal collision flag is true when the `x` axis or the `z` axis is clipped.

**PHY-FLG-002** (`source_suggested`, client agreement, confidence: medium). The vertical flags
change only when `d.y` is not zero, or when the entity has local authority on this side. When they
change:

- the vertical collision flag is true when `r.y` is not equal to `d.y`, by exact comparison;
- the below collision flag is true when the vertical collision flag is true and `d.y < 0`;
- the on-ground flag is equal to the below collision flag.

When they do not change, the three flags keep their values.

**PHY-FLG-003** (`source_suggested`, client agreement, confidence: medium). The minor collision
flag is false, except for the player on its own client. On that client, the flag can be true only
when the horizontal collision flag is true. Then the flag is true when the angle between two
vectors is less than `0.13962634F` radians (8 degrees). The first vector is the intended direction.
The second vector is the horizontal part of `r`.

The intended direction is the input vector, rotated by the yaw as in PHY-INP-002. In this rotation,
the client widens the f32 sine and cosine before the f64 products. When the squared length of a
vector is less than `1.0E-5F`, the flag is false. The client uses this flag only to keep the
sprint state (PHY-INP-009).

The client computes the angle in f64 in these steps. Here `a` is the intended direction and `b`
is `(r.x, r.z)`.

1. Let `qa = a.x² + a.z²` and `qb = b.x² + b.z²`.
2. Let `angle = acos((a.x × b.x + a.z × b.z) / sqrt(qa × qb))`.
3. The flag is true when `angle < 0.13962634F`, with the limit widened.

When the f64 error puts the quotient of step 2 above 1, `acos` gives NaN, and the flag is false.
The client does this. This rule keeps it for client agreement.

### 5.2 Supporting block

**PHY-FLG-004** (`source_suggested`, client agreement, confidence: medium). When the on-ground flag
changes to true or stays true, the entity finds its supporting block:

1. Make the area from `(xmin, ymin - 1.0E-6, zmin)` to `(xmax, ymin, zmax)` of the box.
2. Find the block colliders of that area (PHY-SHP-007).
3. Select the block position whose centre is nearest to the position of the entity, by squared
   distance.
4. If two positions are equally near, select the greater position. Positions compare by `y`, then
   by `z`, then by `x`.

If the search finds a block, the result is the supporting block. If the search and the previous
search both found no block, the result is also the supporting block. Else, the entity moves the
area by `(-r.x, 0, -r.z)` and does the search again. The result of that search is the supporting
block. When the on-ground flag is false, there is no supporting block.

The flag "the last search found no block" changes in each call of this rule. When the on-ground
flag is true, the flag is true when the last search of the call found no block. That search is the
second search when the call made two. When the on-ground flag is false, the flag changes to false.
The rule is called only when PHY-FLG-002 changes the vertical flags. Else the flag keeps its value.

**PHY-FLG-005** (`source_suggested`, client agreement, confidence: medium). The entity finds three
positions under it from the supporting block, with an offset `o`:

| Name | Offset `o` | Use |
|------|-----------|-----|
| support position | `1.0E-5F` | the supporting block itself |
| effect position | `0.2F` | the bounce action, the step action, the fall action |
| movement position | `0.500001F` | the block friction, the speed factor and the jump factor below |

The rule for an offset `o`:

1. With no supporting block, the position is `(floor(x), floor(y - o), floor(z))`.
2. With a supporting block and `o ≤ 1.0E-5F`, the position is the supporting block.
3. With a supporting block that is a wall or a fence gate, the position is the supporting block.
4. With a supporting block that is a fence (tag `fences`) and `o ≤ 0.5`, the position is the
   supporting block.
5. Else, the position is the supporting block with `y` changed to `floor(y - o)`.

The entity widens `o` before the subtraction.

### 5.3 Velocity after a collision

**PHY-RST-001** (`source_suggested`, client agreement, confidence: medium). After the flags, the
velocity `v` changes when the entity simulates its own motion on this side, and one of these is
true:

- `d.y` is not zero and the vertical collision flag is true;
- the horizontal collision flag is true.

The restitution `e` of the entity is 0 when its shift key state is on. Else it is the `bounciness`
attribute for a living entity (default 0), or 0 for another entity.

**PHY-RST-002** (`source_suggested`, client agreement, confidence: medium). For a clipped `x` axis,
`v.x` changes to `-v.x × e`. For a clipped `z` axis, `v.z` changes to `-v.z × e`. With `e = 0`,
the component becomes zero, with the sign of `-v.x × 0`. The clip test is the one of PHY-FLG-001.

**PHY-RST-003** (`source_suggested`, client agreement, confidence: medium). When the vertical
collision flag is true:

1. If the below collision flag is true, compute a new `e`. The new `e` is 0 in each of these
   cases:
   - `-v.y ≤ g`;
   - the shift key state is on;
   - the block at the effect position has the tag `suppresses_bounce`.

   Else the new `e` is `max(e, b)`, where `b` is the bounce restitution of that block. For an
   entity type with the flag `block_bounce_factor` `k` (PHY-ORD-009), `b` is `f32(restitution × k)`.
2. If `e > 0`, let `p = r.y / v.y`. Then `v.y` changes to
   `(p × g - v.y) × (1 + p × (drag - 1)) × e`.
3. If `e = 0`, `v.y` changes to `(0 - v.y) × 1 × 0`, which is zero.

Here `g` is the effective gravity (PHY-GRV-002) and `drag` is the vertical air drag of the entity
(PHY-GRV-005). All values use `v` from before this rule.

**PHY-RST-004** (`source_suggested`, semantic, confidence: medium). A bounce occurs when `e > 0`
gives a horizontal or a vertical change. A bounce calls the bounce action of the block at the
effect position and emits the game event `bounce`. The server then sends the position of the
entity to the clients immediately.

## 6. Gravity, drag and block effects

### 6.1 Gravity

**PHY-GRV-001** (`source_suggested`, client agreement, confidence: medium). The gravity of an entity
is 0 when its `NoGravity` flag is set. Else it is the gravity of its category (section 9). For a
living entity, it is the `gravity` attribute (default 0.08).

**PHY-GRV-002** (`source_suggested`, client agreement, confidence: medium). The effective gravity of
a living entity with the effect `slow_falling` is `min(gravity, 0.01)` when `v.y ≤ 0`. Else the
effective gravity is the gravity.

In the travel step (PHY-ORD-004, step 6), `v.y` is the velocity after the move procedure of step
4. That value includes the changes of the move: the collision (PHY-RST-001 to PHY-RST-003) and the
stuck multiplier. It is the value before the climb rule of step 5 sets `v.y` to 0.2. The test
`v.y ≤ 0` is true for `-0.0`.

**PHY-GRV-003** (`source_suggested`, client agreement, confidence: medium). The effect `levitation`
with the amplifier `a` replaces gravity for a living entity. The vertical velocity after the move
changes by `(0.05 × (a + 1) - v.y) × 0.2`, before drag.

**PHY-GRV-004** (`source_suggested`, client agreement, confidence: low). On the client, a living
entity with local authority in a chunk that is not loaded does not use gravity. The chunk is the
chunk of the movement position. Its vertical velocity before drag is `-0.1` when `y` is above the
minimum height of the dimension, else 0. The server always uses gravity.

**PHY-GRV-020** (`source_suggested`, client agreement, confidence: low). The client of a player
moves in this way when a chunk near the player is not loaded:

1. A block position in a chunk that is not loaded gives no collider (PHY-SHP-007, step 6). The box
   can enter such a chunk without a collision.
2. When the chunk of the movement position is not loaded, the vertical velocity before drag is
   `-0.1`. At the minimum height, it is 0 (PHY-GRV-004). This value replaces gravity and
   levitation.
3. The horizontal rules do not change. The ground friction uses the block at the movement position.
   In a chunk that is not loaded, this block is air.
4. The client does not tick its player before the client has loaded the level.

On the server, a move can keep the fall distance of a player. This occurs when the box of the
player, grown by 1 on each side, touches a chunk that is not loaded (PHY-FALL-003).

### 6.2 Drag on land and in air

**PHY-GRV-005** (`source_suggested`, client agreement, confidence: medium). A living entity has two
air drags. They depend on the `air_drag_modifier` attribute `k` (default 1). Let
`mod(f, k) = clamp(f32(1 - f32((1 - f) × k)), 0, 1)`, computed in f32.

- The horizontal air drag `ah` is `mod(0.91F, f32(k))`.
- The vertical air drag `av` is `mod(0.98F, f32(k))`. An omnidirectional air mover uses the
  horizontal air drag on the vertical axis too. The bee and the parrot are omnidirectional air
  movers.

**PHY-GRV-006** (`source_suggested`, client agreement, confidence: medium). This rule gives the
ground friction `fg` of a living entity. When the on-ground flag is true at the start of the travel
step, `fg` is `mod(fb, f32(friction_modifier))`. Here `fb` is the block friction of the block at
the movement position. When the flag is false, `fg` is 1. The default of `friction_modifier` is
1. With this default, `mod` returns the block friction with no change.

**PHY-GRV-007** (`source_suggested`, client agreement, confidence: medium). At the end of the travel
step of a living entity, the velocity changes to:

- `v.x × f32(fg × ah)`, and the same for `v.z`;
- `v.y × av`.

An entity in the long-jump state keeps `v.x` and `v.z`, and it does not use the vertical drag. With
the default values on a block with the friction `0.6F`, the horizontal factor is
`0.546000063419342`.

**PHY-GRV-008** (`source_suggested`, client agreement, confidence: medium). In step 9 of
PHY-ORD-001 and step 3 of PHY-ORD-002, a living entity sets small velocity components to zero:

- a player sets `v.x` and `v.z` to zero when `v.x² + v.z² < 9.0E-6`;
- another living entity sets `v.x` to zero when `|v.x| < 0.003`, and `v.z` in the same way;
- each living entity sets `v.y` to zero when `|v.y| < 0.003`.

An entity that is not living does not do this step.

**PHY-GRV-009** (`source_suggested`, semantic, confidence: medium). This rule applies on a side
that does not have local authority for a living entity and does not interpolate it. At the start
of each AI step, that side multiplies the velocity by 0.98. This rule applies to the velocity of a
player on the server.

### 6.3 Acceleration of a living entity

**PHY-GRV-010** (`source_suggested`, client agreement, confidence: medium). In the travel step, a
living entity adds its input acceleration to the velocity. The speed of this acceleration is:

- on the ground: let `f` be the ground friction of PHY-GRV-006. Widen `f` and compare it with 0.6.
  If `f > 0.6`, the speed is `f32(speed × f32(0.21600002F / f32(f × f × f)))`. Else the speed is
  `speed`;
- not on the ground: the speed is the air speed of the entity.

For a block with the friction `0.6F`, the comparison is true and the quotient is exactly 1.0. The
`speed` of a player is `f32(movement_speed)`. The `speed` of another living entity is the speed
that its AI sets.

**PHY-GRV-011** (`source_suggested`, client agreement, confidence: medium). The air speed of a
living entity is `0.02F`. When a player controls it, the air speed is `f32(speed × 0.1F)`. The
air speed of a player is in PHY-INP-007 and PHY-INP-014.

### 6.4 Speed factor and jump factor

**PHY-GRV-012** (`source_suggested`, client agreement, confidence: medium). At the end of each move,
the entity multiplies the horizontal velocity by the speed factor. The entity finds the speed
factor in this order:

1. Use the block at the floor of the position.
2. If that block is `water` or `bubble_column`, use its speed factor.
3. Else, if its speed factor is not 1.0, use it.
4. Else, use the speed factor of the block at the movement position.

For a living entity, the value is then `f32(s + t × (1 - s))`, where `s` is the speed factor and
`t = f32(movement_efficiency)`.

**PHY-GRV-013** (`source_suggested`, client agreement, confidence: medium). The jump factor of an
entity is the jump factor of the block at the floor of the position. If that factor is 1.0, it is
the jump factor of the block at the movement position.

### 6.5 Blocks inside the box

**PHY-GRV-014** (`source_suggested`, client agreement, confidence: medium). After the travel step,
the entity applies the effects of the blocks that its box passed through in this tick. The entity
shrinks the box at each end of a path by `1.0E-5F` on each side. A block with an inside shape of
the full cube acts when that box touches its cell along the path. The block effects of this
document are:

| Block | Effect on an entity inside it |
|-------|-------------------------------|
| `cobweb` | stuck multiplier `(0.25, 0.05F, 0.25)`; `(0.5, 0.25, 0.5)` for a living entity with the effect `weaving` |
| `sweet_berry_bush` | stuck multiplier `(0.8F, 0.75, 0.8F)` |
| `powder_snow` | stuck multiplier `(0.9F, 1.5, 0.9F)`. When the flag `powder_snow_needs_inside` of the entity type is true, only when the block at the floor of its position is powder snow |
| `honey_block` | the slide of PHY-GRV-015 |

A block that sets a stuck multiplier also sets the fall distance to 0. When two blocks set a
multiplier in one tick, the last one applies. Section 14 records the order as U7.

An entity ignores the stuck multiplier of a block, and the reset of the fall distance that comes
with it, in these cases. The reset of PHY-FALL-002 for the tag `fall_damage_resetting` still
applies.

- the block is in the set `stuck_ignore` of the entity type (PHY-ORD-009);
- the entity is a player with the flight state on. This case applies to each block.

**PHY-GRV-015** (`source_suggested`, client agreement, confidence: medium). An entity slides on the
side of a honey block when all these conditions are true. Here `(bx, by, bz)` is the block
position of the honey block.

- the on-ground flag is false;
- `y ≤ by + 0.9375 - 1.0E-7`;
- `u < -0.08`, where `u = v.y / 0.98F + 0.08`;
- `|bx + 0.5 - x| + 1.0E-7 > 0.4375 + f32(width / 2)`, or the same test on `z` is true.

Then, if `u < -0.13`, the entity multiplies the horizontal velocity by `-0.05 / u`. In all cases,
`v.y` changes to `(-0.05 - 0.08) × 0.98F`, and the fall distance changes to 0. These are fixed
values. They do not use the gravity attribute.

**PHY-GRV-016** (`source_suggested`, client agreement, confidence: medium). An entity can walk on
powder snow in two cases. Its type has the tag `powder_snow_walkable_mobs`, or it is a living
entity with leather boots in the feet slot. In the tick after it was inside powder snow, such an
entity climbs as on a climbable block (PHY-GRV-019, step 3).

### 6.6 Block motion data

**PHY-GRV-017** (`source_suggested`, client agreement, confidence: medium). The motion properties of
a block are f32 values. The corpus gives the block friction. The other values are not in the
corpus (U3). The vanilla values are:

| Block | Friction | Speed factor | Jump factor | Bounce restitution |
|-------|----------|--------------|-------------|--------------------|
| default | `0.6F` | `1.0F` | `1.0F` | `0.0F` |
| `ice`, `packed_ice`, `frosted_ice`, `brittle_ice` | `0.98F` | | | |
| `blue_ice` | `0.989F` | | | |
| `slime_block` | `0.8F` | | | `1.0F` |
| `soul_sand` | | `0.4F` | | |
| `honey_block` | | `0.4F` | `0.5F` | |
| each bed, `shelf_mushroom` | | | | `0.75F` |

An empty cell has the default value. The `honey_block` has the tag `suppresses_bounce`.

**PHY-GRV-018** (`source_suggested`, client agreement, confidence: medium). The step action of a
slime block changes the horizontal velocity when all these conditions are true:

- the on-ground flag is true after the travel step;
- the block at the effect position is a slime block;
- `|v.y| < 0.1`;
- the shift key state is off.

Then the step action multiplies the horizontal velocity by `0.4 + |v.y| × 0.2`.

### 6.7 Climbable blocks

**PHY-GRV-019** (`source_suggested`, client agreement, confidence: medium). A living entity is on a
climbable block when all these conditions are true:

- the entity is not a spectator;
- the block at the floor of its position has the tag `climbable`, or it is a climbable trapdoor;
- the entity is not in elytra flight, or the block does not have the tag `can_glide_through`.

A trapdoor is climbable when it is open, a ladder is below it, and both face the same direction. On
a climbable block, the travel step changes:

1. Before the move, clamp `v.x` and `v.z` to the range from `-0.15F` to `0.15F`. Change `v.y` to
   `max(v.y, -0.15F)`.
2. Set `v.y` to 0 when `v.y < 0`, the entity is a player and its shift key state is on. This step
   does not apply when the block is `scaffolding`.
3. After the move, set `v.y` to 0.2 when the horizontal collision flag is true. Also set `v.y` to
   0.2 when the entity has the jump input. This step occurs before gravity and drag.

On a climbable block, the fall distance changes to 0 before the move.

## 7. Player input, jump, sneak and flight

**PHY-INP-001** (`source_suggested`, client agreement, confidence: medium). The client makes the
input vector of its player from the movement keys in each tick:

1. The forward value is 1 for the forward key, -1 for the back key, and 0 for both or none. The
   left value is 1 for the left key, -1 for the right key, and 0 for both or none.
2. Normalize the vector (left, forward) in f32. The length is `f32(sqrt(l² + f²))`, with the sum
   in f32. Divide each component by the length. A vector with a length less than `1.0E-4F`
   becomes zero.
3. If the vector is not zero, multiply it by `0.98F`.
4. If the player uses an item and is not a passenger, multiply the vector by the item multiplier.
   The item multiplier is the speed multiplier of the item use, with the default `0.2F`.
5. If the player crouches or crawls (PHY-INP-010), multiply the vector by `f32(sneaking_speed)`.
   The default of the attribute is 0.3.
6. Let `n` be the length. Let `u` be the vector multiplied by `f32(1 / n)`. Let `q` be the smaller
   absolute component of `u` divided by the larger one. Change the length to
   `min(n × sqrt(1 + q²), 1)`.

All operations are f32. The sideways component of the input vector is the left value, the
forward component is the forward value, and the vertical component is 0.

Step 6 makes a diagonal input as long as a straight input, up to the length 1.

**PHY-INP-002** (`source_suggested`, client agreement, confidence: medium). To add an input vector
`i` with a speed `s` to the velocity:

1. If `|i|² < 1.0E-7`, add nothing.
2. If `|i|² > 1`, normalize `i` in f64. The length is `sqrt(|i|²)`. Divide each component by
   the length. A vector with a length less than `1.0E-5F`, widened, becomes zero.
3. Multiply `i` by `s`, widened.
4. Let `a` be the yaw angle of PHY-NUM-005. Let `sn = sinT(a)` and `cs = cosT(a)`, widened.
5. Add `(i.x × cs - i.z × sn, i.y, i.z × cs + i.x × sn)` to the velocity.

Here `i.x` is the sideways component and `i.z` is the forward component.

**PHY-INP-003** (`source_suggested`, client agreement, confidence: medium). In step 4 of
PHY-ORD-002, a living entity that is not a player multiplies its sideways and forward input by
`0.98F`. Its AI then can set new values.

**PHY-INP-004** (`source_suggested`, client agreement, confidence: medium). A living entity has a
jump delay. The jump delay decreases by 1 in step 9 of PHY-ORD-001 and step 3 of PHY-ORD-002, when
it is more than 0. Then:

1. If the entity does not have the jump input, or fluids do not affect it, the jump delay changes to
   0. The entity does not jump.
2. Else, if the fluid model has a fluid for the jump, the fluid model gets the jump (section 12).
3. Else, if the on-ground flag is true and the jump delay is 0, the entity jumps (PHY-INP-005). The
   jump delay changes to 10.

Fluids do not affect a player with the flight state on. Thus such a player does not jump from the
ground in this rule.

**PHY-INP-005** (`source_suggested`, client agreement, confidence: medium). The jump power `P` is
`f32(f32(jump_strength) × 1.0F × J) + boost`, in f32. Here `J` is the jump factor (PHY-GRV-013). The
default of `jump_strength` is `0.42F`. With the effect `jump_boost` of amplifier `k`, `boost` is
`f32(0.1F × (k + 1))`. Else `boost` is 0.

If `P` is not more than `1.0E-5F`, the entity does not jump. Else:

1. Widen `P`. Change `v.y` to `max(P, v.y)`.
2. If the sprint state is on, add `(-sinT(θ) × 0.2, 0, cosT(θ) × 0.2)` to the velocity. Here `θ` is
   the yaw angle of PHY-NUM-005. Widen the f32 table values before the products.

**PHY-INP-006** (`source_suggested`, client agreement, confidence: medium). The sprint state of a
player adds a modifier of `0.3F` to the `movement_speed` attribute, with the operation "multiply
total". To compute the attribute value, do these steps:

1. Add the "add value" modifiers to the base.
2. For each "multiply base" modifier, add `base × amount`.
3. For each "multiply total" modifier, multiply by `1 + amount`.
4. Clamp the result to the range of the attribute.

The default base for a player is `0.1F`.

**PHY-INP-007** (`source_suggested`, client agreement, confidence: medium). The air speed of a
player without the flight state is `0.02F`, or `0.025999999F` with the sprint state on.

**PHY-INP-008** (`source_suggested`, client agreement, confidence: medium). The client starts the
sprint state when all these conditions are true:

- the sprint state is off, and the forward value of the input is more than `1.0E-5F`;
- the player does not have the effect `blindness`;
- the food level is more than 6, or the player has the fly permission;
- the player is not in shallow water, unless the flight state is on;
- the player does not use an item that slows it;
- the player is not in elytra flight and does not crouch or crawl, unless its eyes are in water;
- the sprint key is down, or the player presses the forward key again within the sprint window of
  the client options.

**PHY-INP-009** (`source_suggested`, client agreement, confidence: medium). The client stops the
sprint state of a player on land in each of these cases:

- a condition of PHY-INP-008 about effects, food and water is false;
- the forward value is not more than `1.0E-5F`;
- the horizontal collision flag is true and the minor collision flag is false.

The client sends each change of the sprint state to the server.

**PHY-INP-010** (`source_suggested`, client agreement, confidence: medium). At the start of its AI
step, the client sets the crouch state of the player. Here `shift0` is the shift key of the
previous tick, and `flight0` is the flight state at the start of the tick (PHY-ORD-007). The
crouch state is true when all these conditions are true:

- `flight0` is off, and the player is not in the swim state;
- the player is not a passenger;
- the box of the `crouching` pose has no collision;
- `shift0` is down, or the player does not sleep and the box of the `standing` pose has
  a collision.

The player crawls when it has the `swimming` pose and it is not in a fluid in which it can swim.

**PHY-INP-011** (`source_suggested`, client agreement, confidence: low). In step 5 of PHY-ORD-001,
the client pushes its player out of blocks that cause suffocation (PHY-INP-021). It tests four
points in this order. The offsets on `x` and `z` are `(-0.35w, +0.35w)`, `(-0.35w, -0.35w)`,
`(+0.35w, -0.35w)` and `(+0.35w, +0.35w)`, where `w` is the width.

Each offset is the f32 width widened and multiplied by 0.35 in f64. The point is the position
plus or minus that offset, in f64. For each point:

1. Let the cell be the block position of the point at the height `y`.
2. Make the column of the cell from `ymin` to `ymax` of the box. Shrink it by `1.0E-7`.
3. The cell is blocked when a block that causes suffocation intersects that column.
4. If the cell is blocked, examine west, east, north and south, in this order. Ignore a side where
   the adjacent cell is blocked.
5. Select the side with the smallest distance from the point to that side of the cell. When two
   sides are equally near, select the first one.
6. Set the velocity component of that axis to `0.1` towards the selected side.

This does not occur for a player without physics or for a passenger.

The four points use the position at the start of step 5 of PHY-ORD-001. The push does not move the player, so
each point uses the same position. Step 6 sets only one component and keeps the other two. A
later point that selects the same axis replaces the value of an earlier point. A later point that
selects the other axis keeps the value of the earlier point.

In step 5 of this rule, the distance on `x` is `px - cx` to the west side and `1 - (px - cx)`
to the east side. Here `px` is the `x` of the point
and `cx` is the `x` of the cell. The `z` axis uses the same formulas.

**PHY-INP-012** (`source_suggested`, client agreement, confidence: medium). The edge back-off
keeps a player on a block edge while the shift key state is on. It applies in the move procedure
when all these conditions are true:

- the flight state is off;
- `d.y ≤ 0`;
- the mover kind is `self` or `player`;
- the shift key state is on;
- the player is above ground.

Let `h` be the step height, and let `D` be the fall distance. The player is above ground when the
on-ground flag is true. It is also above ground when `D < h` and the player cannot fall `h - D`.

The player can fall a height `k` with an offset `(dx, dz)` when a test box has no collision. The
test box goes from `(xmin + 1.0E-7 + dx, ymin - k - 1.0E-7, zmin + 1.0E-7 + dz)` to
`(xmax - 1.0E-7 + dx, ymin, zmax - 1.0E-7 + dz)`. Do these steps:

1. Do a loop for `d.x`. The loop continues while `d.x` is not zero and the player can fall `h` with
   the offset `(d.x, 0)`. In each pass, if `|d.x| ≤ 0.05`, set `d.x` to 0 and stop the loop. Else,
   subtract `sign(d.x) × 0.05` from `d.x`.
2. Do the same loop for `d.z`, with the offset `(0, d.z)`.
3. Do a third loop. It continues while `d.x` and `d.z` are not zero and the player can fall `h`
   with the offset `(d.x, d.z)`. In each pass, change `d.x` as in step 1, and then change `d.z` as in step 2. This
   loop has no early stop.

The sign in steps 1 to 3 is the sign of the original component.

**PHY-INP-013** (`source_suggested`, client agreement, confidence: medium). A player with the flight
state on, on its own client, adds `n × f32(s × 3.0F)` to `v.y` in step 7 of PHY-ORD-001. Here `s` is the
flight speed, and `n` is +1 for the jump key, -1 for the shift key, and their sum for both. The
flight speed comes from the `player_abilities` packet. Its default is `0.05F`.

**PHY-INP-014** (`source_suggested`, client agreement, confidence: medium). The travel step of a
player with the flight state on, and that is not a passenger:

1. Keep `v.y` as `y0`.
2. Run the travel step of a living entity. The air speed is the flight speed `s`. With the
   sprint state on, the air speed is `f32(s × 2.0F)`.
3. Change `v.y` to `y0 × 0.6`.

Thus gravity and the vertical drag have no effect, and a ceiling does not stop `v.y`. The fall
distance of a player with the flight state on changes to 0 in step 8 of PHY-ORD-001.

**PHY-INP-015** (`source_suggested`, client agreement, confidence: medium). The client can toggle
the flight state of a player with the fly permission. It toggles the state when the player presses
the jump key two times, and the player is not in the swim state. PHY-INP-018 states the interval
between the two presses.

When the flight state starts and the on-ground flag is true, the player jumps immediately, in step
6 of PHY-ORD-001. This jump is the jump of PHY-INP-005, with the sprint
boost when the sprint state is on. It does not read the jump delay and does not change it. The
vertical impulse of PHY-INP-013 then adds in step 7, as in each tick with the flight state on.

A passenger can toggle the flight state only when its vehicle takes the jump of a rider, as a
horse does.

At the end of the AI step, the flight state stops when the on-ground flag is true, except for a
spectator. A spectator always has the flight state on.

**PHY-INP-016** (`source_suggested`, client agreement, confidence: low). The auto-jump option of the
client can add a jump input. This document treats it as a jump input of the player.

**PHY-INP-017** (`source_suggested`, client agreement, confidence: medium). The client changes the
sprint state of its player only in step 6 of PHY-ORD-001, and in an attack (below). In step 6:

1. Let `fwd0` be true when the forward value of the previous tick was more than `1.0E-5F`. Let
   `shift0` be the shift key of the previous tick.
2. Set the sprint timer to 0 when `shift0` is on or the back key is down. Also set it to 0 when
   an item use slows the player (PHY-INP-019).
3. Examine the start conditions of PHY-INP-008 without the last condition. If they are false, go to
   step 6.
4. If `fwd0` is false and the sprint timer is more than 0, start the sprint state. If `fwd0` is
   false and the sprint timer is 0, set the sprint timer to the sprint window. The sprint window is
   a client option, with the default 7.
5. If the sprint key is down, start the sprint state.
6. If the sprint state is on and the player is not in the swim state, examine PHY-INP-009. Stop
   the sprint state when one of its stop conditions is true. The swim state has its own stop
   conditions (section 12).

The sprint timer decreases by 1 at the start of step 4 of PHY-ORD-001, when it is more than 0. An
attack with the sprint knockback also stops the sprint state. The server sets the sprint state of
its copy from the `player_command` packet.

**PHY-INP-018** (`source_suggested`, client agreement, confidence: medium). The flight state of the
player changes only in these cases:

1. The `player_abilities` packet sets it, between two ticks.
2. In step 6 of PHY-ORD-001, a spectator gets the flight state on.
3. In step 6, a player with the fly permission can toggle it. The player must not be in the swim
   state. The jump key must change to down at most 6 ticks after the previous change to down. A
   press from auto-jump does not count. The jump key changes to down when the jump key of `K` is
   down and `jump0` is up (PHY-ORD-007).
4. In step 15, the flight state stops when the on-ground flag is true, except for a spectator.

The jump timer of case 3 decreases by 1 at the start of step 8 of PHY-ORD-001, when it is more
than 0. A toggle
sets it to 0. The client sends each change of the flight state to the server.

**PHY-INP-019** (`source_suggested`, client agreement, confidence: medium). The item use state is
part of the player state. It has three values:

- a flag that is true when the player uses an item;
- the speed multiplier of that use;
- a flag that is true when that use permits the sprint state.

The speed multiplier and the sprint permission come from the `use_effects` component of the item.
Their defaults are `0.2F` and false. The item use state changes between ticks, when the player
starts or stops an item use.

An item use slows the player when the player uses an item and the use does not permit the sprint
state. PHY-INP-001 step 4 applies the multiplier when the player is not a passenger.

**PHY-INP-020** (`source_suggested`, client agreement, confidence: medium). The movement input of
a living entity that is not a player decays in this way:

1. At the start of each AI step, the entity multiplies its sideways and forward input by `0.98F`
   (PHY-INP-003). The vertical input does not decay.
2. The AI then can set new values. A move control without a move target sets the forward input to
   exactly 0. It sets the sideways input only for a side step.
3. A value that only decays never reaches exactly zero. The f32 product stops at a small
   denormal value, because `0.98F` times that value rounds back to it.
4. An input vector with a squared length less than `1.0E-7` has no effect on motion
   (PHY-INP-002, step 1). This occurs after a finite number of ticks.

The input is not in the saved data of the entity and the server does not send it. Thus its value
below the limit of step 4 is not observable.

**PHY-INP-021** (`source_suggested`, client agreement, confidence: medium). A block state causes
suffocation (PHY-INP-011) in these cases:

1. The block is in this table. The table gives the result.
2. Else, the state causes suffocation when both conditions are true:
   - the block has the tag `causes_suffocation`;
   - the collision shape of the state for no context is the full cube.

| Blocks | Causes suffocation |
|--------|--------------------|
| the tag `leaves`, `mangrove_roots`, `glass`, `tinted_glass`, each stained glass block, each copper grate, `moving_piston` | never |
| `farmland`, `soul_sand`, `dirt_path`, `mud` | always |
| each shulker box | when its block entity is closed, or when it has no block entity |
| `piston`, `sticky_piston` | when the property `extended` is false |

In vanilla, the tag `causes_suffocation` contains the tag `blocks_motion`. The copper grates are
the eight blocks with the name `copper_grate`, with the prefixes of oxidation and wax.

The test of step 3 of PHY-INP-011 uses the collision shape of the state for the player
(PHY-SHP-003). A block intersects the column when that shape overlaps the shrunk column. The full
cube overlaps when the column touches the cell with a volume more than zero.

## 8. Fall distance

**PHY-FALL-001** (`source_suggested`, client agreement, confidence: medium). The side with local
authority updates the fall distance in the block effects step, after the travel step:

1. Let `dy` be the sum of the vertical displacements of the entity in this tick.
2. If `dy < 0`, subtract `dy` from the fall distance.
3. Multiply the fall distance by the fall multiplier of the fluids that the entity touched. Without
   a fluid, the multiplier is 1 (section 12).
4. If the on-ground flag is true and the fall distance is more than 0, apply the fall action.
   This is the fall action of the block at the effect position. It causes fall damage (section 12).
5. If the on-ground flag is true, set the fall distance to 0.

**PHY-FALL-002** (`source_suggested`, client agreement, confidence: medium). The fall distance
changes to 0 in each of these cases:

- the entity is on a climbable block (PHY-GRV-019);
- a block sets a stuck multiplier (PHY-GRV-014);
- the entity slides on a honey block (PHY-GRV-015);
- a living entity has the effect `slow_falling` or `levitation`, at the travel step;
- a player has the flight state on and is not a passenger, in step 8 of PHY-ORD-001;
- the position of the entity passes through a block with the tag `fall_damage_resetting` in this
  tick (PHY-FALL-004);
- the entity rides another entity.

**PHY-FALL-003** (`source_suggested`, semantic, confidence: medium). The server keeps the fall
distance of a player from the moves that the client sends. For each accepted move, it uses the
displacement and the on-ground bit of the packet in PHY-FALL-001. If the move goes up, the fall
distance changes to 0.

**PHY-FALL-004** (`source_suggested`, client agreement, confidence: medium). The reset for the
tag `fall_damage_resetting` applies to each entity that the blocks inside the box affect
(PHY-GRV-014), not only to a living entity. Thus an `item`, a `falling_block` and a `tnt` also
get the reset. The tag contains `cobweb`, `sweet_berry_bush` and the tag `climbable`.

The reset occurs in the step "the blocks inside the box" of PHY-ORD-001, step 14. The entity
examines each block that its box touches along its path (PHY-GRV-014). For such a block with the
tag, the entity also tests the part of the path in which the box touches the block. The reset
occurs when the position of the entity on that part touches the inside shape of the block
(PHY-RCH-008). The position is the point at the bottom centre of the box, not the centre of the
box. The test uses the segment from the position at the start of the part to the position at its
end.

A part with a squared length less than `1.0E-7` gives no reset. The client does this. This rule
keeps it for client agreement.

The fall distance of the tick (PHY-FALL-001) adds
after the reset. Thus an entity that falls into a cobweb ends that tick with the fall of that tick,
not with 0.

## 9. Order of operations and entity categories

### 9.1 The tick of a player on its own client

**PHY-ORD-001** (`source_suggested`, client agreement, confidence: medium). In one tick, the client
does these steps for its player, in this order:

1. Keep the old position and rotation.
2. If the player is a spectator, it has no physics. If it is a spectator or a passenger, the
   on-ground flag changes to false.
3. Do the base entity tick. It updates the fluid state (section 12).
4. Start the AI step: set the crouch state from `shift0` and `flight0` (PHY-INP-010). Then read
   the keys of this tick (PHY-INP-001).
5. Push out of blocks (PHY-INP-011).
6. Update the sprint state (PHY-INP-008, PHY-INP-009) and the flight state (PHY-INP-015).
7. Add the vertical flight impulse (PHY-INP-013).
8. If the flight state is on, set the fall distance to 0.
9. Decrease the jump delay. Set small velocity components to zero (PHY-GRV-008).
10. Set the input vector and the jump input from the keys.
11. Jump (PHY-INP-004).
12. If the effect `slow_falling` or `levitation` is active, set the fall distance to 0.
13. Do the travel step (PHY-ORD-004), or the flight variant (PHY-INP-014).
14. Apply the block effects in this order:
    - the step action of the block at the effect position, when the on-ground flag is true
      (PHY-GRV-018);
    - the blocks inside the box (PHY-GRV-014);
    - the fall distance (PHY-FALL-001).
15. If the on-ground flag is true and the flight state is on, stop the flight state, except for a
    spectator.
16. Clamp `x` and `z` to the range from `-2.9999999E7` to `2.9999999E7`.
17. Select the pose (PHY-BOX-006).
18. Send the position, the rotation, the on-ground flag and the horizontal collision flag to the
    server, as `movement-checks.md` states.

### 9.2 The tick of another living entity

**PHY-ORD-002** (`source_suggested`, semantic, confidence: medium). On the side with local
authority, a living entity that is not a player does these steps in one tick:

1. Keep the old position and rotation.
2. Do the base entity tick.
3. Decrease the jump delay. Set small velocity components to zero (PHY-GRV-008).
4. Multiply the input by `0.98F` (PHY-INP-003). Run the AI, which can set the input, the jump input
   and the speed. When the travel gate of the entity type is closed (PHY-ORD-009), the entity
   does not do the travel step.
5. Jump (PHY-INP-004).
6. If the effect `slow_falling` or `levitation` is active, set the fall distance to 0.
7. Do the travel step of its category (PHY-ORD-004 or the table of PHY-ORD-005).
8. Apply the block effects, as in step 14 of PHY-ORD-001.
9. Push other entities (PHY-PSH-001).

**PHY-ORD-003** (`source_suggested`, semantic, confidence: medium). The travel step of a living
entity selects one model:

- in a fluid that affects it, and on which it cannot stand: the fluid model (section 12);
- during elytra flight: the elytra model (section 12), except on a climbable block;
- else: the model of this document.

**PHY-ORD-004** (`source_suggested`, client agreement, confidence: medium). The travel step of a
living entity on land and in air:

1. Find the ground friction `f` (PHY-GRV-006).
2. Add the input acceleration (PHY-GRV-010, PHY-INP-002).
3. Apply the climb rules before the move (PHY-GRV-019, steps 1 and 2).
4. Call the move procedure with the kind `self` and the displacement `v`.
5. Apply the climb rule after the move (PHY-GRV-019, step 3, and PHY-GRV-016).
6. Compute the new `v.y`: with `levitation`, PHY-GRV-003; else subtract the effective gravity
   (PHY-GRV-002), or use PHY-GRV-004 on the client.
7. Apply the drag (PHY-GRV-007).

Thus a free fall from rest gives `v.y = (v.y - g) × 0.98F` in each tick, and the gravity of a tick
moves the entity in the next tick.

### 9.3 Entity categories

**PHY-ORD-005** (`source_suggested`, semantic, confidence: medium). Each entity type belongs to one
category. The category states the parameters and the order of the steps. A new entity type must
select a category and give its dimensions and parameters as data.

The column "Order" uses these letters:

- `G`: subtract gravity from `v.y`;
- `M`: call the move procedure with `v`;
- `D`: apply the drag;
- `E`: apply the block effects;
- `R`: move along a ray (section 12).

| Category | Examples | Gravity | Order | Horizontal drag | Vertical drag | Step height | Small velocity to zero | Collision |
|----------|----------|---------|-------|-----------------|---------------|-------------|------------------------|-----------|
| player | `player` | attribute, 0.08 | PHY-ORD-001 | `f × 0.91F` (PHY-GRV-007) | `0.98F` | attribute, 0.6 | horizontal length | box |
| living, walker | `zombie`, `armor_stand` | attribute, 0.08 | PHY-ORD-002 | `f × 0.91F` | `0.98F` | attribute, base of the type (PHY-STP-001) | per axis | box |
| living, omnidirectional | `bee`, `parrot` | attribute, 0.08 | PHY-ORD-002 | `f × 0.91F` | `0.91F` | attribute, base of the type (PHY-STP-001) | per axis | box |
| living, flier | `ghast` (speed `0.02F`), `phantom` (`0.2F`), `allay` (its speed), `happy_ghast` | none in this step | input, `M`, then `×0.91F` on all axes | `0.91F` | `0.91F` | attribute | per axis | box |
| item | `item` | 0.04 | `G`, `M`, `E`, `D`, bounce | `0.98F`, `f32(0.98F × f)` on the ground | `0.98F` | 0 | none | box |
| falling block | `falling_block` | 0.04 | `G`, `M`, `E`, ground rule, `D` | `0.98F` | `0.98F` | 0 | none | box |
| primed TNT | `tnt` | 0.04 | `G`, `M`, `E`, `D`, ground rule | `0.98F` | `0.98F` | 0 | none | box |
| experience orb | `experience_orb` | 0.03 | `G`, `M`, `E`, `D`, bounce | `0.98F`, `f32(0.98F × f)` on the ground | as horizontal | 0 | none | box |
| arrow | `arrow`, `spectral_arrow`, `trident` | 0.05 | `R`, `D`, `G` | `0.99F` | `0.99F` | none | none | ray |
| thrown item | `snowball`, `egg`, `ender_pearl` (0.03); `splash_potion`, `lingering_potion` (0.05); `experience_bottle` (0.07) | as listed | `G`, `D`, `R` | `0.99F` | `0.99F` | none | none | ray |
| llama spit | `llama_spit` | 0.06 | `R` with the velocity before `D`, then `D`, `G` | `0.99F` | `0.99F` | none | none | ray |
| accelerated projectile | `fireball`, `small_fireball`, `wind_charge`, `dragon_fireball`, `wither_skull` | none | `v ← (v + 0.1 × unit(v)) × 0.95F`, `R` | | | none | none | ray |

The parameters of the rows:

- `f` is the block friction at the movement position, used only when the on-ground flag is true.
- Item, bounce: after the drag, if the on-ground flag is true and `v.y < 0`, multiply `v.y` by
  -0.5.
- Item, rest: an item on the ground with `v.x² + v.z² ≤ 1.0E-5F` skips `M` and `D` in three ticks
  of four. Let `t` be the number of ticks that the item has existed, and let `n` be its phase
  offset (PHY-CMP-016). The item skips `M` and `D` when the remainder of `(t + n) / 4` is not 0.
  A tick that skips `M` and `D` also skips the bounce. It still applies `G`. Its `E` uses the
  path of the last tick that did `M`, not a new path. Thus the blocks inside the box continue to
  act on an item at rest, for example a hopper or a pressure plate.
- Item, stop: on the server, after the drag of a tick that does `M`, if the on-ground flag is
  true, set each of `v.x` and `v.z` with an absolute value of at most `1.0E-6` to 0. The reference
  has no such step. In the reference, the ground factor makes `v.x` and `v.z` smaller in each tick
  with `M`, but they do not become zero. With a factor above 0.5, for example `0.98F × 0.6F` on
  stone, a value stops at the smallest positive f64 value, `4.9E-324`, after about 6000 ticks
  (E53). PHY-DEV-014 records this step.
- Item, push: on the server, shrink the box of an item by `1.0E-7`. If that box has a collision,
  the item has no physics in that tick, and the server pushes it to a free side.
- `falling_block`, ground rule: on the server, before the drag, if the on-ground flag is true,
  multiply `v` by `(0.7, -0.5, 0.7)`. Then the entity tries to become a block. This document
  does not state that step.
- Primed TNT, ground rule: after the drag, if the on-ground flag is true, multiply `v` by
  `(0.7, -0.5, 0.7)`.
- Experience orb: `G` applies only when the box has no collision and the eyes are not in water.
  The ground factor applies to `v.y` too. Bounce: if the below collision flag is true and the
  `v.y` from before `M` is less than `-gravity`, `v.y` changes to `-0.4` times that value. An orb
  also accelerates towards a near player. This document does not state that acceleration.
- The drag of a category that is not living is a factor of `0.98F`, `0.99F` or `0.95F`. It is also
  the "vertical air drag" of PHY-RST-003 for that category.
- PHY-ORD-009 gives the motion flags of each category and the exceptions for each type.

The dimensions of the categories are:

| Type | Width | Height | Eye height |
|------|-------|--------|-----------|
| `item` | 0.25F | 0.25F | 0.2125F |
| `falling_block` | 0.98F | 0.98F | default |
| `tnt` | 0.98F | 0.98F | 0.15F |
| `experience_orb` | 0.5F | 0.5F | default |
| `arrow` | 0.5F | 0.5F | 0.13F |
| `snowball` | 0.25F | 0.25F | default |
| `zombie` | 0.6F | 1.95F | 1.74F |
| `armor_stand` | 0.5F | 1.975F | 1.7775F |
| `shulker` | 1.0F | 1.0F | 0.5F |

**PHY-ORD-006** (`source_suggested`, semantic, confidence: medium). These entities use models that
this document does not state:

- the boat category: gravity 0.04, drag `1.0F`;
- the minecart category: gravity 0.04, or 0.005 in water, drag `0.95F`;
- the squid: gravity 0.08, with its own swim model;
- the `fishing_bobber`: gravity `0.03F`, with its own model;
- the `shulker_bullet`: gravity 0.04, with its own model.

**PHY-ORD-009** (`source_suggested`, semantic, confidence: medium). Each entity type has the
motion flags of this table. The category of the type gives the default value. The exception list
below changes the value for one type. A new entity type sets its flags as data. It does not add a
rule.

| Flag | Meaning | Default, living category | Default, other category |
|------|---------|--------------------------|-------------------------|
| `stuck_ignore` | the set of blocks whose stuck multiplier the entity ignores (PHY-GRV-014) | the empty set | the set with `sweet_berry_bush` |
| `powder_snow_needs_inside` | the stuck multiplier of `powder_snow` applies only when the block at the floor of the position is powder snow (PHY-GRV-014) | true | false |
| `powder_snow_solid` | `powder_snow` gives the full cube when the fall distance is not more than 2.5 (PHY-SHP-003) | false | false |
| `block_bounce_factor` | the factor `k` on the bounce restitution of a block (PHY-RST-003) | `1.0F` | `0.8F` |
| `travel_gate` | a condition that stops the travel step (PHY-ORD-002) | for a mob, the `NoAI` flag; else none | not used |
| `pushable` | other entities can push the entity (PHY-ORD-012) | the condition of PHY-ORD-012 | false |
| `push_targets` | the entities that the entity pushes in its own tick (PHY-PSH-001) | each candidate | none |

The exception list:

| Type | Flag | Value |
|------|------|-------|
| `wither` | `stuck_ignore` | each block |
| `spider` | `stuck_ignore` | the set with `cobweb` |
| `frostbite` | `stuck_ignore` | the set with `powder_snow` |
| `fox`, `bee` | `stuck_ignore` | the set with `sweet_berry_bush` |
| `falling_block` | `powder_snow_solid` | true |
| `armor_stand` | `travel_gate` | the `Marker` flag or the `NoGravity` flag |
| `mannequin` | `travel_gate` | the immovable flag of the mannequin |
| `armor_stand`, `bat` | `pushable` | false |
| `parrot` | `pushable` | true |
| each horse type | `pushable` | the condition of PHY-ORD-012, and false when it carries a rider |
| `creaking` | `pushable` | the condition of PHY-ORD-012, and false when it cannot move |
| `warden` | `pushable` | the condition of PHY-ORD-012, and false while it digs down or comes up |
| each boat and raft, each minecart | `pushable` | true |
| `armor_stand`, `bat` | `push_targets` | none |
| `parrot` | `push_targets` | each candidate that is not a player |

A state of an entity, for example the flight state of a player or an effect, is not a flag of this
table. The rules that use such a state name it.

**PHY-ORD-010** (`source_suggested`, semantic, confidence: medium). In the exact-parity mode, the
ticks of entities in one server tick occur in this order:

1. Between two server ticks, the server handles the packets of the clients, in the order in which
   they arrive. A move packet changes the position of its player when the server handles it
   (PHY-SRV-003).
2. The server ticks each dimension in turn. The overworld is first. The other dimensions follow in
   the order in which the server created them.
3. In one dimension, other parts occur before the entities. They are the world border, the
   weather, the time, the scheduled block and fluid ticks, and the raids. Then the chunk tick, the
   block events and the fight with the ender dragon occur.
4. The dimension ticks each entity of its tick list in the order in which the entity entered the
   list. Players are in this list, with the other entities.
5. Then the block entities tick. Then the dimension adds and removes entities, and sends the
   changes of the entities to the clients.
6. After each dimension, the server ticks the connections in the order in which they were made. In
   this pass it does the server part of each player (PHY-SRV-004).

An entity enters the tick list when it is added to a chunk that ticks entities, or when its chunk
starts to tick entities. An entity that leaves the list and enters it again goes to the end.

The exact-parity mode keeps this order. In that mode, an entity sees each block change that occurs
before its own tick in the same server tick. The exact-parity mode does not use PHY-CMP-009 or
PHY-DEV-003.

**PHY-ORD-011** (`source_suggested`, semantic, confidence: medium). In the pass of step 4 of
PHY-ORD-010, these rules apply:

- An entity added during the pass does not tick in this pass. Its first tick is in the next
  server tick.
- An entity removed during the pass does not tick after its removal.
- An entity ticks only when it is a player, or when its chunk ticks entities. An entity that the
  tick command freezes does not tick.
- A passenger does not tick in its own place in the list. It ticks after its vehicle, as a part of
  the tick of the vehicle. The passengers of one vehicle tick in the order of the passenger list,
  and each passenger is followed by its own passengers.
- A passenger whose vehicle is removed, or that the vehicle does not carry, stops its ride. It then
  ticks in its own place in the list.

**PHY-ORD-012** (`source_suggested`, semantic, confidence: medium). The flag `pushable` of
PHY-ORD-009 states that other entities can push the entity. The push itself is out of scope
(section 12). For a living category, the default is true when all these conditions are true:

- the entity is alive and is not a spectator;
- it is not on a climbable block (PHY-GRV-019);
- it is a player, or its block position is in a chunk that ticks entities.

A boat and a minecart use a pushable entity as a collider for themselves (PHY-SHP-009).

**PHY-ORD-013** (`source_suggested`, semantic, confidence: medium). This model does not state the
effect of these blocks on motion. A checker must treat a tick as coarse when one of these blocks
is in the read zone of PHY-RCH-001:

| Blocks | Effect that this model does not state |
|--------|---------------------------------------|
| `water`, `lava`, `bubble_column`, and each block with a fluid in its state | the fluid model, and the current of a bubble column |
| `potent_sulfur` | the geyser adds `0.2F` to `v.y` |
| `nether_portal`, `end_portal`, `end_gateway` | a teleport |
| `piston`, `sticky_piston`, `piston_head`, `moving_piston` | a push, and a shape that the corpus does not give |
| `slime_block`, `honey_block`, when a piston moves them | a push that carries the entity |
| `shulker_box` and each dyed shulker box | a push, and a shape that changes while the lid opens |
| `big_dripleaf` | a shape that changes when it tilts |
| `bamboo`, `pointed_dripstone`, `sulfur_spike`, `icicle` | a shape that the corpus gives only at the probe position |

The rows of PHY-ORD-008 that are not blocks stay in that rule.

**PHY-ORD-014** (`source_suggested`, semantic, confidence: medium). A root vehicle and its
passengers tick as one unit. The root vehicle ticks first. Then each passenger ticks, in the order
of PHY-ORD-011. In the tick of a passenger, these rules apply:

1. The velocity of the passenger changes to zero.
2. The passenger does its tick.
3. The vehicle sets the position of the passenger. It uses its own position after its step in
   this tick, plus the attachment offset.

Thus a passenger reads the state of its vehicle after the step of the vehicle in this tick. The
client sees the passenger at the position of the vehicle in the same tick.

In the order-independent
pass (PHY-CMP-008, PHY-CMP-009, PHY-CMP-012), one unit is one task. Inside the unit, the order of
this rule applies. A unit reads the other units only at their state from the start of the tick.
A vehicle can have a passenger that controls it. The vehicle reads the input of that passenger
from its state at the start of the tick.

**PHY-ORD-007** (`source_suggested`, client agreement, confidence: medium). One tick of a player
is a function `step(S, F, K, R, W) → S'`. It is deterministic (PHY-INV-008). The arguments are:

- `S`, the player state at the start of the tick. It contains these values:
  - the position, the velocity and the pose;
  - the yaw and the pitch at the end of the previous tick;
  - the five result flags and the supporting block;
  - the flag "the last search found no block" (PHY-FLG-004);
  - the fall distance, the stuck multiplier and the jump delay;
  - the sprint timer and the jump timer;
  - `shift0`, the shift key of the previous tick, and the forward value of the previous tick;
  - `jump0`, the jump key of the previous tick, with a press of auto-jump included;
  - the sprint state `sprint0` and the flight state `flight0`;
  - the crouch state, the swim state and the item use state;
  - the attributes and the effects;
  - the fly permission and the flight speed;
  - a flag that is true when the player was inside powder snow in the previous tick;
- `F`, the facts of the player that the tick reads and does not change:
  - the spectator game mode;
  - the passenger state, and whether the vehicle takes the jump of a rider;
  - the sleep state;
  - the food level, or only whether it is more than 6;
  - the sprint window of the client options (PHY-INP-017);
- `K`, the keys of this tick: forward, back, left, right, jump, shift and sprint. `K` also has a
  flag that is true when the jump key of this tick comes from auto-jump (PHY-INP-016);
- `R`, the yaw and the pitch of this tick;
- `W`, the world view: the block states, the loaded chunks, the entity colliders and the world
  border.

The step keeps three values of the flight state and two values of the sprint state:

| Value | Set at | Used by |
|-------|--------|---------|
| `flight0` | start of the tick | the crouch state (step 4 of PHY-ORD-001), with `shift0` |
| `flight1` | after step 6 | steps 7 to 14: the vertical impulse, the jump, the edge back-off, the travel step, the stuck multiplier |
| `flight2` | after step 15 | the pose (step 17), and `flight0` of the next tick |
| `sprint0` | start of the tick | the double-tap test of PHY-INP-017 |
| `sprint1` | after step 6 | the `movement_speed` modifier, the air speed, the jump boost, and `sprint0` of the next tick |

The crouch state of this tick comes from `shift0`, not from the shift key of `K`. The other rules
of the tick use the shift key of `K`. `S'` contains the same values as `S`, at the end of the
tick. Its `shift0` is the shift key of `K`, and its `jump0` is the jump key of `K`.

The result flags of `S'` are the flags of the last move of the tick. The step does not read a
clock, a random value or a value that `S`, `F`, `K`, `R` and `W` do not contain. The rotation `R`
applies before step 1. The push of PHY-PSH-008 changes the velocity of `S` before the tick.

The old position and the old rotation are not in `S`. Step 1 of PHY-ORD-001 sets them before a
rule reads them. Thus they do not change the result of the tick, and step 7 of PHY-SRV-010 does
not change the physics. An implementation can keep these values outside `S`.

**PHY-ORD-008** (`source_suggested`, semantic, confidence: medium). This model does not cover the
effects in this list. A tick in which one of them can act is not exact. A checker must treat such
a tick as coarse.

| Source | Effect on motion |
|--------|------------------|
| water, lava, and each other fluid | the fluid model replaces the travel step |
| `bubble_column` | changes `v.y` inside it and above it |
| `potent_sulfur` and its geyser | adds `0.2F` to `v.y` of entities above it, when `v.y < 0.3F + 0.1 × n` for `n` water blocks |
| `nether_portal`, `end_portal`, `end_gateway` | teleport |
| a piston, a `moving_piston`, and slime or honey blocks that a piston moves | push with the mover kind `piston` |
| a shulker box that opens, a shulker that opens | push with the kinds `shulker_box` and `shulker` |
| `big_dripleaf` | its collision shape changes when it tilts |
| a block with a position offset, a `moving_piston`, a shulker box | the shape is not in the corpus (U5) |
| a block shape that the corpus rounds (U4) | the shape is not exact |
| a ridden entity, a boat, a minecart | the vehicle model |
| elytra flight, riptide | their own models |
| an entity that pushes the player | the push of PHY-PSH-007, which the client computes from its own view |
| knockback, explosion, wind charge, fishing rod pull, ender pearl, chorus fruit, a teleport command | external impulses and teleports (PHY-SRV-009, PHY-SRV-010) |
| a `happy_ghast` that moves while the player stands on it | an entity collider that moves |
| a chunk that is not loaded on the client | PHY-GRV-020 |

### 9.4 Causal reach of one tick

A design that splits the world into shards needs to know which state one tick of one entity reads
and writes. These rules state that reach for the physics part of the tick: sections 2 to 9. PHY-RCH-006
states the reach of the push between entities (section 9.6). The AI and the subjects of section 12
have their own reach.

**PHY-RCH-001** (`source_suggested`, semantic, confidence: medium). Let `B` be the box at the start
of the tick. Let `d` be the largest requested displacement of the tick, and `h` the step height.
To make the read zone `C`, start with the union of `B` and `B` moved by `d`. Grow it by 1 on `x`
and on `z`, and by `1 + h` up and down. One tick of the entity reads only this state:

- its own state: the fields of `S` in PHY-ORD-007, its type flags (PHY-ORD-009) and its
  dimensions. The yaw and the pose are part of this state. The input acceleration reads the yaw,
  and the box comes from the pose;
- the block states at the block positions that `C` touches. Under PHY-CMP-009, these are the
  block states at the end of the previous server tick;
- the boxes of the entities that can be collided with (PHY-SHP-009) and that intersect `C`. Under
  PHY-CMP-008, these are the boxes at the start of the server tick;
- the values of the world border in this tick;
- which chunks in `C` are loaded;
- the data pack tags of section 0.5.

The read zone covers these regions:

- the scan margin of PHY-SHP-007 and the step area;
- the edge back-off probes;
- the supporting block, the effect position and the movement position;
- the blocks inside the path.

**PHY-RCH-002** (`source_suggested`, semantic, confidence: medium). The physics part of a tick
writes these fields of its own entity: the position, the velocity, the flags and the supporting
block. It also writes the fall distance, the stuck multiplier and the pose. It does not write the position or the velocity of
another entity. It can write other state only through these block actions:

- the step action of the block at the effect position: `magma_block`, `redstone_ore`,
  `deepslate_redstone_ore`, `turtle_egg`, `sculk_sensor`, `calibrated_sculk_sensor`,
  `sculk_shrieker`, `brittle_ice` and `slime_block`;
- the inside action of a block in the box. PHY-RCH-007 gives the complete list;
- the fall action when the entity lands: fall damage to the entity and its passengers, and blocks such as
  `farmland` and `turtle_egg`;
- the bounce action and the game events `step`, `hit_ground` and `bounce`, which other blocks and
  entities can hear later.

An implementation must defer these writes, or apply them under the order of their domain. The
writes of the motion state stay inside the entity.

**PHY-RCH-003** (`source_suggested`, semantic, confidence: medium). In one call of the move
procedure, `|r_A| ≤ |d_A| + 1.0E-7` on each axis, without a step-up. A step-up adds at most
`h + 1.0E-7` to `r.y` (PHY-INV-004). The requested displacement is the velocity after the input
acceleration. No rule of this model limits the velocity itself. With constant attributes, no
effect change and no external impulse, the velocity tends to these limits:

- the vertical velocity under gravity `g` and vertical drag `D` tends to `-g × D / (1 - D)`. For a
  living entity with the defaults, this is about `-3.92` blocks per tick. For an item, it is about
  `-1.96`;
- with `levitation` of amplifier `a`, `v.y` tends to `0.2 × L × D / (1 - 0.8 × D)`, with
  `L = 0.05 × (a + 1)`. This is about `0.907 × L` for `D = 0.98F`;
- a horizontal input acceleration `s` with the horizontal factor `F` of PHY-GRV-007 gives a
  horizontal speed that tends to `s × F / (1 - F)`.

The displacement of one tick has no bound in these cases:

- a teleport (PHY-SRV-010) or an external impulse (PHY-SRV-009);
- a change of the velocity by a command;
- a change of an attribute or of an effect;
- each model of section 12.

**PHY-RCH-004** (`source_suggested`, semantic, confidence: medium). Only these steps of a tick read
other entities:

- the resolution of the move and of the step-up, through the entity colliders (PHY-SHP-008);
- the collision tests of the pose (PHY-BOX-006), of the crouch state (PHY-INP-010) and of the edge
  back-off (PHY-INP-012). These tests also use the entity colliders.

The supporting block, the blocks inside the box, the speed factor, the jump factor, gravity and
drag read only blocks and the entity itself. Thus the ticks of two entities are independent when
no entity that can be collided with is in the read zone of either of them. The vanilla server
ticks entities one after the other. An entity that ticks later sees the new box of a collider
that ticked earlier. PHY-CMP-008 states when an implementation can read the boxes from the start
of the tick. PHY-CMP-009 states the same for the block states.

With both rules, the tick of an
entity reads only state from the start of the server tick and its own state. Thus the ticks of all
server-simulated entities in one server tick are independent of each other.

**PHY-RCH-005** (`source_suggested`, semantic, confidence: medium). For an entity at rest, the
displacement `d` of PHY-RCH-001 is the largest requested displacement of its rest cycle
(PHY-SKIP-003):

| Category | `d` at rest |
|----------|-------------|
| living, walker | `(0, (+0.0 - g) × D, 0)`, the rest velocity |
| item | `(0, -0.16, 0)`, in the tick of the cycle in which it moves |
| primed TNT | `(0, -0.04, 0)` |
| experience orb | `(0, -0.03, 0)` |

Thus the read zone of an entity at rest is its box, grown by 1 on `x` and on `z`. It is also
grown by `1 + h` up, and by `1 + h + |d.y|` down. A change in this zone wakes the entity
(PHY-SKIP-002).

**PHY-RCH-006** (`source_suggested`, semantic, confidence: medium). The push reads the entities
whose box intersects the box of the entity. For each of them, it reads these values:

- the position, the box, and the flags `pushable` and `push_targets`;
- the spectator state and the physics state;
- the team and its collision rule;
- the root vehicle, and whether it carries a passenger or is a passenger.

Under PHY-CMP-012, these are the values at the start of the server tick. The box of the entity is
also its box at the start of the tick. Under PHY-CMP-012, the push writes only the velocity and the
health of its own entity.

**PHY-RCH-007** (`source_suggested`, semantic, confidence: medium). A block has an inside action
when it is in this table, or when it has the tag `fall_damage_resetting` (PHY-FALL-004). A fluid
in the box also has an inside action, but the fluid model covers it (PHY-ORD-008). No other block
has an inside action. The column "Entities" gives the entities on which the action can act. An
implementation can use that column to decide PHY-SKIP-004 for one entity.

| Blocks | Inside action | Entities |
|--------|---------------|----------|
| the tag `pressure_plates` | press the plate | each entity |
| the tag `buttons` | press the button | an arrow, only for a wooden button |
| `tripwire` | detect the entity | each entity |
| `detector_rail` | detect the entity | a minecart |
| `wheat`, `carrots`, `potatoes`, `beetroots`, `torchflower_crop`, `pitcher_crop` | break the crop | a `ravager`, with the game rule `mob_griefing` |
| `fire`, `soul_fire` | stop the freeze, set the entity on fire, damage | each entity |
| `campfire`, `soul_campfire` | damage, when the campfire is lit | a living entity |
| `water_cauldron`, `powder_snow_cauldron` | stop the fire of the entity, and lower the level of the cauldron | each entity |
| `lava_cauldron` | stop the freeze, set the entity on fire, damage | each entity |
| `cactus` | damage | each entity |
| `sweet_berry_bush` | stuck multiplier, damage | a living entity that is not a `fox` or a `bee` |
| `cobweb` | stuck multiplier | each entity |
| `powder_snow` | stuck multiplier, freeze | each entity (PHY-GRV-014) |
| `honey_block` | the slide of PHY-GRV-015 | each entity |
| `bubble_column` | changes `v.y` (PHY-ORD-008) | each entity |
| `wither_rose` | the effect `wither` | a living entity |
| `open_eyeblossom`, `closed_eyeblossom` | an effect | a `bee` |
| `big_dripleaf` | tilt the leaf | an entity on the top of the leaf |
| `lily_pad` | break the block | a boat |
| `frogspawn` | break the block | a `falling_block` |
| `hopper` | collect the entity | an `item` |
| `nether_portal`, `end_portal`, `end_gateway` | teleport | an entity that can use a portal |

The cauldron action lowers the level only when the entity is on fire. The step actions of
PHY-RCH-002 are the only step actions. A block without an entry has neither action.

**PHY-RCH-008** (`source_suggested`, client agreement, confidence: medium). Each block has an
inside shape. PHY-GRV-014 and PHY-FALL-004 test the box and the position against it. The inside
shape is the full cube, except for the blocks of this table. The values are in pixels of `1/16`.

| Blocks | Inside shape |
|--------|--------------|
| `water_cauldron`, `powder_snow_cauldron` with `level` `n` | the collision shape of `cauldron`, plus the box from `(2, 4, 2)` to `(14, 6 + 3n, 14)` |
| `lava_cauldron` | the collision shape of `cauldron`, plus the box from `(2, 4, 2)` to `(14, 15, 14)` |
| `powder_snow` | the collision shape for the entity (PHY-SHP-003). When that shape is empty, the full cube. |
| `end_portal` | the box from `(0, 6, 0)` to `(16, 12, 16)` |
| `tripwire` with `attached` true | the box from `(0, 1, 0)` to `(16, 2.5, 16)` |
| `tripwire` with `attached` false | the box from `(0, 0, 0)` to `(16, 8, 16)` |

For the full cube, the test of PHY-GRV-014 applies: the shrunk box touches the cell. For another
shape, the test also needs a box of the shape. Grow that box on each axis by half the size of the
entity box on that axis, minus `1.0E-7`. The block acts when the path of the centre of the entity
box touches the grown box. The path goes from the centre at the start of the part to the centre
at its end.

### 9.5 Ticks that can be skipped

**PHY-SKIP-001** (`source_suggested`, semantic, confidence: medium). A tick is a fixed point when
its physics part gives `S' = S`, bit for bit. The comparison uses each field of PHY-RCH-002 except
the pose. Let a full tick of an entity be a fixed point. Then the next tick is a fixed point too,
when nothing that PHY-RCH-001 lists changed between the two ticks. This follows from PHY-INV-008.

An implementation can skip the physics part of such a tick. The skip is observably identical to
the full tick.

**PHY-SKIP-002** (`source_suggested`, semantic, confidence: medium). A skipped entity must run a
full tick again when one of these occurs:

- a block state in its read zone changes;
- an entity that can be collided with enters, moves in or leaves its read zone;
- the world border changes, or a chunk in the read zone loads or unloads;
- its velocity, position, pose, `NoGravity` flag, attributes or effects change, or a teleport or an
  external impulse occurs;
- its input or its jump input is not zero. For a mob, the AI sets these values in each tick;
- the stuck multiplier is not zero;
- the phase of a periodic rule changes the result (PHY-SKIP-003).

**PHY-SKIP-003** (`source_suggested`, semantic, confidence: medium). These are the rest states of
the categories on a flat block top, with no input. An implementation can use them to detect a fixed
point. `D` is the vertical drag and `g` the effective gravity.

| Category | Velocity at the end of each tick at rest |
|----------|------------------------------------------|
| living, walker | `(±0, (+0.0 - g) × D, ±0)`. With the defaults, `v.y` is `-0.0784000015258789`. The signs of the zero components stay as they are. |
| item | a cycle of 4 ticks. Let `t` be the tick count of the item and `n` its phase offset (PHY-CMP-016). When the remainder of `(t + n) / 4` is 0, the item moves and `v.y` ends at 0. In the next three ticks, `v.y` ends at `-0.04`, `-0.08` and `-0.12`. `v.x` and `v.z` must be zero. |
| primed TNT | `(±0, -0.0, ±0)` |
| experience orb | `(±0, +0.0, ±0)`, when no player is near enough to attract it |

A living entity with `g < 0.003 / D` has a different rest state, because the zero cutoff of
PHY-GRV-008 removes `v.y`. The rest state of the item is periodic. The step "Item, stop" of
PHY-ORD-005 makes `v.x` and `v.z` of an item zero. On stone, an item that lands with a horizontal
speed of 0.12 gets to its rest state in 67 ticks (E53). A skip of a rest item must
give `v.y` from the phase, not from the last value.

**PHY-SKIP-004** (`source_suggested`, semantic, confidence: medium). A skip of the physics part does
not skip the other parts of the tick. Examples are the age, the fuse, the AI, the effects and the
fire. A skip is not identical when the block at the effect position has a step action. It is also
not identical when a block in the box has an inside action (PHY-RCH-002). The implementation must run these actions in each tick,
or it must not skip.

**PHY-SKIP-005** (`source_suggested`, semantic, confidence: medium). An implementation can wake an
entity at rest more often than PHY-SKIP-002 requires, for example in each tick. A full tick from
the correct rest state gives the same state as a skip. Thus the extra wake is not visible. These
limits apply:

- The skip must keep the state that the full tick would keep. For an item, this is the `v.y` of
  the phase of the cycle (PHY-SKIP-003), not the last value before the skip.
- The tick count of the entity must increase in each tick, with or without a skip, because the
  item cycle reads it.
- When a block with a step action or an inside action is present, a full tick can run that action
  (PHY-SKIP-004). The rule of PHY-SKIP-004 applies to such an entity in all cases.

**PHY-SKIP-006** (`source_suggested`, semantic, confidence: medium). The push changes the skip
rules of PHY-SKIP-001 and PHY-SKIP-002:

- An entity that gets a push in a tick is not at a fixed point in that tick. It gets a push when,
  for a neighbour of PHY-CMP-012, a direction applies and `m ≥ 0.01F` (PHY-PSH-004).
- An entity at rest must wake when an entity enters or leaves the push zone of PHY-RCH-006. It
  must also wake when a value of PHY-RCH-006 changes for an entity in that zone.
- An entity at rest must wake when the count of PHY-PSH-007 is more than `n - 1`, because crowd
  damage can occur.

Thus the exact skip condition with the push is: the entity gets no push, and the count of its
neighbours is not more than `n - 1`. Two entities whose boxes intersect with `m < 0.01F` get no
push. They can rest.

**PHY-SKIP-007** (`source_suggested`, semantic, confidence: medium). For PHY-SKIP-002, an input
counts as zero when the squared length of the input vector is less than `1.0E-7`. An idle mob
rests under these conditions:

- its move control has no target, so the forward input is 0;
- the squared length of the input vector is less than `1.0E-7` (PHY-INP-020);
- it has no jump input;
- the conditions of PHY-SKIP-001, PHY-SKIP-002 and PHY-SKIP-006 are true.

During the skip, the input can keep its last value. The value is not observable, and its decay
cannot make it larger than the limit. When the entity wakes, the AI sets the input again.

### 9.6 The push between entities

**PHY-PSH-001** (`source_suggested`, semantic, confidence: medium). Each living entity pushes other
entities in step 9 of its tick (PHY-ORD-002). The push occurs after the travel step and the block
effects of that tick. The server does it for each living entity that ticks. The client does it for
each living entity that it ticks, with the restriction of PHY-PSH-002. The server does it for a
player in the server part of the player (PHY-ORD-010, step 6).

The flag `push_targets` of PHY-ORD-009 states which candidates an entity pushes.

**PHY-PSH-002** (`source_suggested`, semantic, confidence: medium). The candidates of a pusher `P`
are the other entities whose box intersects the box of `P` with a volume that is more than zero.
The box of `P` is its box after its own move in this tick. The query does not grow the box. A
candidate `C` must also meet all these conditions:

- `C` is not a spectator, and its flag `pushable` is true (PHY-ORD-012);
- the team rule of PHY-PSH-003 lets `P` push `C`;
- on the client, `C` is the own player. The client does not push other candidates.

**PHY-PSH-003** (`source_suggested`, semantic, confidence: medium). The team rule uses the
collision rule of the team of each entity. An entity without a team has the rule `always`.

1. If the rule of `P` is `never`, `P` pushes no candidate.
2. If the rule of `C` is `never`, `P` does not push `C`.
3. Let `same` be true when `P` has a team and that team is allied to the team of `C`.
4. If one of the two rules is `pushOwnTeam` and `same` is true, `P` does not push `C`.
5. Else, if neither rule is `pushOtherTeams`, `P` pushes `C`.
6. Else, `P` pushes `C` only when `same` is true.

On the client, the client applies steps 2 to 6 only to the own player.

**PHY-PSH-004** (`source_suggested`, semantic, confidence: medium). For one pusher `P` and one
candidate `C`, the push is this computation. All values are f64. The constants `0.01F` and `0.05F`
are widened.

1. If `P` and `C` ride the same root vehicle, or one of them has no physics, stop.
2. `xa = P.x - C.x`, and `za = P.z - C.z`.
3. `m = max(|xa|, |za|)`. If `m < 0.01F`, stop.
4. `s = sqrt(m)`. `xa = xa / s`, and `za = za / s`.
5. `k = 1 / s`. If `k > 1`, `k = 1`. `xa = xa × k`, and `za = za × k`.
6. `xa = xa × 0.05F`, and `za = za × 0.05F`.
7. If `C` carries no passenger and its flag `pushable` is true, add `(-xa, 0, -za)` to the velocity
   of `C`.
8. If `P` carries no passenger and its flag `pushable` is true, add `(xa, 0, za)` to the velocity of
   `P`.

The distance `m` is the larger horizontal axis distance, not the Euclidean distance. The push has
no vertical part. Each component of one push has an absolute value of at most `0.05F`. An addition
occurs only when the three components are finite.

**PHY-PSH-005** (`source_suggested`, semantic, confidence: medium). PHY-PSH-004 is antisymmetric.
When `P` and `C` change roles, `xa` and `za` change their sign exactly, and `m`, `s` and `k` stay
the same. Thus the two entities of a pair get equal and opposite pushes, bit for bit.

**PHY-PSH-006** (`source_suggested`, semantic, confidence: medium). The reference applies the push
in sequence, in the order of the ticks (PHY-ORD-010). These effects follow:

- A pair of entities that push each other gets the push two times in one tick: one time in the tick
  of each entity.
- The two pushes use different positions. The first push uses the position of the earlier entity
  after its move, and the position of the later entity before its move.
- The later entity uses the push of the earlier entity in its own travel step of the same tick. The
  earlier entity uses the push of the later entity in its next tick.

**PHY-PSH-007** (`source_suggested`, semantic, confidence: medium). Crowd damage: only the
server does it, in step 9, before the pushes. Let `n` be the value of the game rule
`max_entity_cramming`, with the default 24.

1. If `n ≤ 0`, stop.
2. If the number of candidates of PHY-PSH-002 is not more than `n - 1`, stop.
3. Draw a random integer from 0 to 3 from the random source of the entity. If it is not 0, stop.
4. Count the candidates that are not passengers. If the count is more than `n - 1`, the entity
   takes 6.0 damage of the type `cramming`.

**PHY-PSH-008** (`source_suggested`, client agreement, confidence: medium). The push of the own
player:

- On the client, the tick of each other living entity pushes the own player, with the positions of
  the view of the client. The tick of a boat on the client also pushes the own player
  (PHY-PSH-009). The push adds to the velocity of the own player immediately, between two ticks of
  the own player. Thus it is part of `v` of `S` at the start of the next tick of the own player
  (PHY-ORD-007). The zero cutoff of step 9, the jump and the input acceleration of PHY-ORD-001 all
  see it. When the other entity ticks before the own player on the client, the push is in the
  same tick.
- The tick of the own player on the client pushes no entity (PHY-PSH-002).
- On the server, the tick of a server-simulated entity pushes the server copy of the player. The
  server does not send that velocity to the client of the player. Thus the server does not push the
  player in the result that the client sees.
- On the server, the player pushes server-simulated entities in its server part.

**PHY-PSH-009** (`source_suggested`, semantic, confidence: low). Vehicles push in other ways. This
document states only the hooks:

- A boat queries the pushable entities in its box, grown by `0.2F` on `x` and `z` and shrunk by
  `0.01F` on `y`. It can take a candidate as a passenger. Else it pushes the candidate with
  PHY-PSH-004, when the bottom of the candidate is not above the bottom of the boat. For another
  boat, the condition is that the bottom of that boat is below the top of this boat.
- A minecart pushes only on the server. It uses the Euclidean horizontal distance, with a minimum
  squared distance of `1.0E-4F`, and the factors `0.1F` and 0.5. The other entity gets one quarter
  of the push. A push between two minecarts has its own rule.
- An armor stand pushes only a minecart that a player can ride, when the squared distance is at
  most 0.2.
- The contact push of a `sulfur_cube` on a player is not part of this model. The side effects of a
  push by an `iron_golem`, a `warden` or a cube mob are also not part of it.

**PHY-PSH-010** (`source_suggested`, semantic, confidence: medium). `movement-checks.md` must treat
the push of the own player as a source of uncertainty, not as an exact value. The client computes it
from interpolated positions that the server does not know (PHY-CMP-001). The checker needs these
values:

- the list of the living entities and the boats whose box can touch the box of the player. The
  list uses the view of the client and the team rules of PHY-PSH-003;
- the bound: each such entity adds at most `0.05F` to `|v.x|` and to `|v.z|` in one tick, and
  nothing to `v.y`;
- the conditions in which the player gets no push: a spectator, an entity on a climbable block, a
  passenger, the team rule `never`;
- the fact that a push applies in the same tick or in the next tick, as PHY-PSH-008 states.

### 9.7 Writes of the block domain to entities

**PHY-BLK-001** (`source_suggested`, semantic, confidence: medium). The block domain changes
server-simulated entities in the same tick, as the reference does. Its writes occur after the
entity pass of the tick and before the next tick, in the order of the block domain. These are the
writes:

- a piston, a sticky piston, and the slime and honey blocks that a piston moves. They move
  entities with the mover kind `piston` (PHY-BLK-002). A slime block that moves can also set the
  velocity of an entity;
- a shulker box that opens: it moves entities with the mover kind `shulker_box`;
- a hopper: it claims items (PHY-CMP-017);
- a pressure plate, a tripwire and a detector rail: they read the entities in their box, and they
  change only blocks.

The block domain reads each entity at its state after the entity pass of the tick. The next entity
pass reads the result. This is the order of the reference, so this rule is not a deviation. A
block in one cell can write an entity in another cell. The owner of the entity must apply the write
before the next entity pass. A layout that applies it one tick later uses the exception of
PHY-INV-014, with the same bound of one tick, only for that entity.

**PHY-BLK-002** (`source_suggested`, client agreement, confidence: medium). A move with the kind
`piston` limits the requested displacement `d` before step 3 of the move procedure:

1. If `|d|² ≤ 1.0E-7`, use `d` without a change.
2. Use only the first axis of `x`, `y` and `z` on which `d` is not zero.
3. Keep a sum of the piston displacement of the entity on each axis for the current game tick. At
   the first piston move in a new game tick, set the sums to 0.
4. Clamp the sum plus the component to the range from `-0.51` to `0.51`. The component is the
   clamped value minus the old sum. Store the clamped value as the new sum.
5. If the absolute value of the component is not more than `1.0E-5F`, the move stops. Else the
   requested displacement is the component on that axis.

A move with the kind `piston` does not multiply `d` by the stuck multiplier. PHY-MOV-002 still
sets the stuck multiplier and the velocity to zero, after the limit of this rule. A move that
stops in step 5 does not do PHY-MOV-002. A move with the kind `piston` does not apply the edge
back-off. The kinds `shulker` and `shulker_box` have no such limit.

**PHY-EXP-001** (`source_suggested`, semantic, confidence: medium). In the reference, an entity
that explodes does so in its own tick. A primed TNT explodes in the tick in which its fuse becomes
0 or less. The explosion acts in the tick of the exploder:

- It damages and pushes the entities in its reach. The push is an addition to the velocity.
- An entity that ticks later in the same tick uses the push in its own tick. An entity that
  ticked before uses it in the next tick.
- It removes blocks. The entities that tick later see the blocks as removed.
- A TNT block in the reach becomes a primed TNT with a short random fuse. The new entity ticks
  first in the next tick (PHY-ORD-011).
- The server sends the knockback of a player in the `explode` packet (PHY-SRV-009).

**PHY-EXP-002** (`source_suggested`, semantic, confidence: medium). Several explosions in one tick
occur in the order of the ticks of their exploders (PHY-ORD-010). A later explosion sees the
velocities, the positions and the blocks that the earlier explosions changed. Thus a TNT cannon
works: the charges explode first, and the projectile TNT ticks later in the same tick. The
projectile uses the push of the charges in its own move of the same tick. The client sees the
result as the position of the projectile.

**PHY-EXP-003** (`source_suggested`, semantic, confidence: medium). One explosion in the reference
has three parts, in this order:

1. It traces its rays. There are 1352 rays, one for each cell on the surface of a cube of
   16 × 16 × 16 cells. Each ray takes one random value from the random source of the dimension, in
   a fixed order of the rays. A ray reads blocks and adds block positions to a set. It does not
   remove a block.
2. It damages and pushes the entities. The exposure of an entity also reads the blocks before the
   removal.
3. It removes the blocks of the set, in an order that a random shuffle gives.

Thus no ray of one explosion sees a block that another ray of the same explosion removes. An
implementation can trace the rays of one explosion in parallel, from the state at the step of
that explosion. For the exact-parity mode, it must take the 1352 random values first, in the order
of the rays. In the semantic mode, it can take each value from a stream. The stream is a function
of the explosion and of the number of the ray. PHY-DEV-010 records this case.

## 10. Invariants

An implementation and a test can check these properties. Each property holds for one call of the
move procedure with a requested displacement `d` and a resolved displacement `r`, unless the rule
states otherwise.

**PHY-INV-001** (`source_suggested`, semantic, confidence: medium). Without a step-up, on each axis,
`|r_A| ≤ |d_A|`. Also, `r_A` has the sign of `d_A`, or `|r_A| < 1.0E-7`.

**PHY-INV-002** (`source_suggested`, semantic, confidence: medium). If `d_A` is zero, `r_A` is zero,
except for `r.y` after a step-up.

**PHY-INV-003** (`source_suggested`, semantic, confidence: medium). Before the move, let the box
not overlap any collider of the move by more than `1.0E-7` on all three axes together. Then the
same is true after the move.

**PHY-INV-004** (`source_suggested`, semantic, confidence: medium). After a step-up, let
`lift = r.y - (base.ymin - box.ymin)`, and let `h` be the widened step height. Then
`-1.0E-7 ≤ lift ≤ h + 1.0E-7`. The lower tolerance comes from PHY-COL-005. The upper tolerance
covers the f64 round-off of the subtraction. Thus the step-up never lifts the base box more than
the step height, within `1.0E-7`.

**PHY-INV-005** (`source_suggested`, semantic, confidence: medium). After a step-up,
`r.x² + r.z²` is more than the horizontal squared length of the result without the step-up.

**PHY-INV-006** (`source_suggested`, semantic, confidence: medium). When the vertical flags change
(PHY-FLG-002), the on-ground flag is true exactly when `r.y ≠ d.y` and `d.y < 0`.

**PHY-INV-007** (`source_suggested`, semantic, confidence: medium). Take a move that sets the
on-ground flag without a step-up. After this move, the bottom of the box is within `1.0E-7` of a
`y` grid line of a collider. The exception is a request with `0 < |d.y| < 1.0E-7` and at least one
collider. For this request, the early exit of PHY-COL-004 gives `r.y = 0`. The on-ground flag is
then true with no collider under the box.

**PHY-INV-008** (`source_suggested`, semantic, confidence: medium). The move procedure is
deterministic. For the same world, the same entity state and the same requested displacement, it
gives the same result, bit for bit. No rule of sections 2 to 8 uses a random value. The exception
is the push of an item or an experience orb to a free side (PHY-ORD-005), which is unspecified.

**PHY-INV-009** (`source_suggested`, semantic, confidence: medium). The order of the colliders does
not change `r` by more than `1.0E-7` on any axis (PHY-SHP-011).

**PHY-INV-010** (`source_suggested`, semantic, confidence: medium). A move with all colliders
removed gives `r = d`, after the stuck multiplier and the edge back-off.

**PHY-INV-011** (`source_suggested`, semantic, confidence: medium). Take a living entity on the
ground with the default attributes, no effect, the friction `0.6F` and no input. In each tick, its
horizontal velocity decreases by the factor `0.546000063419342`. This continues until PHY-GRV-008
sets the velocity to zero.

**PHY-INV-012** (`source_suggested`, semantic, confidence: medium). The edge back-off never
increases `|d.x|` or `|d.z|`, and never changes their signs.

**PHY-INV-013** (`source_suggested`, semantic, confidence: medium). After a horizontal collision of
an entity with the restitution 0, the velocity component of each clipped axis is zero.

**PHY-INV-014** (`source_suggested`, semantic, confidence: medium). Let an implementation use
PHY-CMP-008 and PHY-CMP-009. Let the world be split into cells. Each cell reads a copy of the
blocks and the collider boxes of its neighbours. The copy holds the state at the end of the
previous server tick. Thus it is the same copy in each layout.

The copy of each cell must cover the
read zone (PHY-RCH-001) of each entity in the cell. Then the physics result of each entity is the
same for one cell and for each split into many cells, bit for bit.

Exception "claim across a cell boundary": a claim from another cell can join the arbitration of
the next tick (PHY-CMP-017). The bound is one tick, only for the target of the claim.

Exception "action across a cell boundary": an entity can start a block action in a column that
another cell owns. This can be a step action, an inside action or a fall action. The owner applies that action one
tick later than in a layout of one cell. Then physics sees the result of the action at `T + 2`, not
at `T + 1`.

The bound is one extra tick. It applies only to the block that the action changes. The
other results stay the same in each layout.

A copy that a cell makes after it handles the client packets breaks this invariant. The cell that
owns a block would see a change by a packet in the same tick, and a neighbour cell one tick later.

**PHY-INV-015** (`source_suggested`, semantic, confidence: medium). Let an implementation use
PHY-CMP-008, PHY-CMP-009 and PHY-CMP-012. Then the tick of each server-simulated entity reads only
its own state and state from the end of the previous server tick. It writes only its own state,
except for the deferred block actions of PHY-RCH-002. Thus the physics result, with the push, is
independent of the order of the entities. It is also independent of the number of threads and of
the split into cells.

The copy of each cell must cover the zones of PHY-RCH-001 and PHY-RCH-006. The exception of
PHY-INV-014 stays.

Three ordered parts keep this property, because their order does not depend on the layout:

- the pre-pass of the colliders (PHY-CMP-018), which also reads the boxes of the colliders after
  their step;
- the explosion groups (PHY-CMP-020);
- the block domain after the entity pass (PHY-BLK-001) and the arbitration of claims
  (PHY-CMP-017).

## 11. The role of the server

### 11.1 Player and server-simulated entity

**PHY-SRV-001** (`source_suggested`, semantic, confidence: medium). The client has local authority
for the motion of its own player. The server does not run the travel step of a player. It must
not send the result of its own simulation of player motion as the position of that player.

**PHY-SRV-002** (`source_suggested`, semantic, confidence: medium). The server has local authority
for every server-simulated entity. It runs the tick of its category (section 9) and sends the
result to the clients.

**PHY-SRV-003** (`source_suggested`, semantic, confidence: medium). The server can apply the
displacement that a client sends again, through the move procedure with the mover kind `player`.
The server does not have local authority for the player. Thus in that call, the vertical flags
change only when the vertical displacement is not zero (PHY-FLG-002). `movement-checks.md` states
when the server does this, and what it does with the result.

**PHY-SRV-004** (`source_suggested`, semantic, confidence: medium). In the tick of a player on the
server, the server applies the block effects (PHY-GRV-014, PHY-GRV-018) to its own copy of the
player. It does not update the fall distance in that step (PHY-FALL-003). The velocity of that
copy decreases by 0.98 in each tick (PHY-GRV-009). PHY-CMP-023 states each change of that velocity
and its order.

**PHY-SRV-011** (`source_suggested`, client agreement, confidence: medium). A player that rides a
vehicle does not move by its own motion step. The vehicle sets the position of the player after
the step of the vehicle (PHY-ORD-014). On the server, this is a write of the vehicle to the state
of the player. While the player rides, the server uses only the rotation from the move packets of
the client. The movement checks of the player do not apply.

When the client controls the vehicle, the client sends the motion of the vehicle. The vehicle model
and `movement-checks.md` handle it. The client also sets the position of the player from its own
view of the vehicle. When the ride ends, the motion step of the client applies again.

### 11.2 The state that a checker can use

**PHY-SRV-005** (`source_suggested`, semantic, confidence: medium). An implementation must expose
this state of each entity at each tick boundary, so that the checks of `movement-checks.md` can
use it:

- the position, the velocity and the box, before and after the tick;
- the pose and the dimensions;
- the five result flags, and the supporting block;
- the fall distance and the stuck multiplier for the next move;
- the jump delay;
- for a player: the shift key state, the sprint state, the flight state and the crouch state;
- for a player: the keys of the last `player_input` packet, the flight speed and the fly
  permission;
- the attributes of sections 2 to 7: `gravity`, `step_height`, `movement_speed`, `jump_strength`,
  `scale`, `sneaking_speed`, `movement_efficiency`, `bounciness`, `friction_modifier`,
  `air_drag_modifier`;
- the effects `slow_falling`, `levitation`, `jump_boost`, `weaving` and `blindness`, with their
  amplifiers;
- the external impulses of the tick (PHY-SRV-006).

**PHY-SRV-006** (`source_suggested`, semantic, confidence: medium). A caller must also be able to
call the move procedure without a change of the entity state. A checker then can resolve a
displacement for a player box and read the flags. The server does not know the input vector of a
client. It knows only the keys of the `player_input` packet.

### 11.3 External impulses

**PHY-SRV-007** (`source_suggested`, semantic, confidence: medium). An external impulse changes the
velocity between two ticks, outside the travel step. Examples are the knockback of an attack, an
explosion and a wind charge. For a server-simulated entity, the server adds the impulse to the
velocity, and the next tick uses it.

For a player, the server sends the new velocity in the `set_entity_motion` packet. For an explosion,
it sends the knockback vector in the `explode` packet. The client then uses the impulse in its next
tick (PHY-SRV-009). The server records each impulse that it sends, with the tick. This model has no
grace time after an impulse.

**PHY-SRV-008** (`source_suggested`, client agreement, confidence: medium). This rule gives the
velocity at the end of a tick of a player from the observed displacement `Δ` of that tick. The
rule has these inputs:

- `Δ`;
- the on-ground flag `G0` at the start of the tick;
- the flags at the end of the tick;
- the clipped axes.

The result is the velocity before the zero cutoff of the next tick.

1. For each horizontal axis `A`, let `u_A = 0` when the axis is clipped, else `u_A = Δ_A`.
2. Multiply `u_x` and `u_z` by the speed factor at the end position (PHY-GRV-012).
3. Let `u_y = 0` when the vertical collision flag is true, else `u_y = Δ_y`. If the climb rule of
   PHY-GRV-019 step 3 applies, let `u_y = 0.2`.
4. Without the flight state: with `levitation`, `y = u_y + (0.05 × (a + 1) - u_y) × 0.2`. Else
   `y = u_y - g`, with the effective gravity `g` for `v.y = u_y` (PHY-GRV-002). Here `u_y` is
   the value before the climb rule of step 3.
5. Let `v_x = u_x × F` and `v_z = u_z × F`, where `F` is the widened f32 product of PHY-GRV-007.
   The ground friction of `F` uses `G0` and the block at the movement position at the start of the
   tick. Let `v_y = y × av`.
6. With the flight state `flight1` on: `v_x` and `v_z` as in step 5, and `v_y = 0.6 × u_y`. If the
   vertical collision flag is true, `v_y` is not known.
7. If a slime block acts (PHY-GRV-018), multiply `v_x` and `v_z` by `0.4 + |v_y| × 0.2`.

The packets of a client give only one horizontal collision flag. When the flag is true, the
checker must use each of three cases as a candidate: `x` clipped, `z` clipped, and both clipped. A
stuck multiplier from the previous tick makes `u = 0` on all axes before step 2.

The result is not exact when one of these occurred in the tick:

- an edge back-off that changed `d`;
- a push out of blocks (PHY-INP-011);
- a bounce with `e > 0`;
- a honey slide;
- a ceiling collision without a known `d.y`;
- an effect of PHY-ORD-008.

**PHY-SRV-009** (`source_suggested`, client agreement, confidence: medium). The client applies a
velocity packet between two ticks, before step 1 of the next tick:

- `set_entity_motion` for the own player replaces the velocity with the value of the packet;
- the client adds the player knockback vector of `explode` to the velocity, when each component is
  finite.

The next tick uses the new velocity. The zero cutoff of step 9 of PHY-ORD-001 is the first rule
that applies to it. On the server, an impulse to a server-simulated entity changes its velocity
immediately. The entity uses the new velocity in its next tick. If the entity ticks later in the
same server tick, it uses the velocity in that tick.

**PHY-SRV-010** (`source_suggested`, client agreement, confidence: medium). A `player_position`
packet has a position change `p`, a velocity change `m`, a yaw `ya`, a pitch `pi` and a set of
relative flags. The client applies it between two ticks, when the player is not a passenger. Let
`P`, `V`, `Ya` and `Pi` be the current position, velocity, yaw and pitch. Let `rad(α)` be
`α × 0.017453292519943295` in f64, for an angle `α` in degrees.

1. For each axis `A` of `x`, `y` and `z`, find the new position. It is `P_A + p_A` when the flag
   of `A` is set, else `p_A`.
2. Find the new yaw `Ya'`. It is `Ya + ya` in f32 when the yaw flag is set, else `ya`.
3. Find the new pitch `Pi'`. It is `Pi + pi` in f32 when the pitch flag is set, else `pi`. Clamp
   `Pi'` to the range from -90 to 90.
4. If the flag "rotate velocity" is set, rotate `V` about the `x` axis by `f32(rad(Pi - Pi'))`.
   Then rotate it about the `y` axis by `f32(rad(Ya - Ya'))`. The differences are f32.
5. For each axis `A`, find the new velocity. It is `V_A + m_A` when the velocity flag of `A` is
   set, else `m_A`. `V` is the result of step 4.
6. Set the position, and make the box again (PHY-BOX-002). Set the velocity, the yaw and the
   pitch.
7. Apply steps 1 to 3 to the old position and the old rotation in the same way. This step does
   not change the next tick of the physics (PHY-ORD-007).

Each rotation of step 4 uses `c = cosT` and `s = sinT` of the widened angle. A rotation about `x`
gives `(x, y × c + z × s, z × c - y × s)`. A rotation about `y` gives
`(x × c + z × s, y, z × c - x × s)`.

The teleport does not change the result flags, the supporting block or the fall distance. The
client then sends `accept_teleportation` with the new position. The server computes the same
values for its copy from its own current values. For the velocity, the server uses its record of
the last client displacement, not the velocity of the client. Thus a teleport with a velocity
flag can give the server and the client different velocities.

### 11.4 Compatibility policy

The architecture of mcrs gives each subsystem one of three policies: `exact`, `semantic` and
`intentional_deviation` (`docs/minecraft-bevy-sharding-architecture.md`, section 2). These rules
state the policy of each category of section 9, and what the vanilla client shows for a
difference.

**PHY-CMP-001** (`source_suggested`, client agreement, confidence: medium). The vanilla client
shows the entities of each category in this way. The update interval is the number of ticks between
two position updates of the server.

| Category | The client simulates it | A position update from the server | Update interval |
|----------|-------------------------|-----------------------------------|-----------------|
| own player | yes, with this model | only a teleport (PHY-SRV-010), with no interpolation | none |
| other player | no | interpolation over 3 ticks | 2 |
| living entity that is not a player | no. When no interpolation is active, the client multiplies its velocity by 0.98 in each tick. The client does not move it by the velocity. | interpolation over the update interval of the type | 3 by default |
| item, falling block, primed TNT, arrow, thrown item, llama spit, accelerated projectile | yes, with this model, from the last state that the server sent | the client sets the position with no interpolation | item 20, falling block 20, TNT 10, arrow 20, thrown item 10 |
| experience orb | yes, with this model | linear interpolation over 3 ticks | 20 |

When the update is more than 64 blocks from the position of the client, the client sets the
position with no interpolation.

**PHY-CMP-002** (`source_suggested`, wire, confidence: medium). The server sends a position update
in one of two forms:

- `move_entity_pos` and `move_entity_pos_rot` carry a change of position in units of `1/4096`
  block. The unit value is `round(x × 4096)` of each coordinate. A change of more than 32767
  units, about 8 blocks, needs the other form.
- `entity_position_sync` carries the position as three f64 values.

The server also uses the f64 form in these cases:

- the on-ground flag changed;
- the entity needs a precise position;
- more than 400 updates occurred after the last f64 form;
- the entity stopped a ride.

A `falling_block` and a primed TNT need a precise position after each tick with a collision flag.
An item needs it when it collides and the unit value loses precision. The server sends a position
only in two cases. In the first case, the change from the last sent position has a squared length
of at least `7.6293945E-6F`, about `0.00276` block. In the second case, the tick is each 60th tick
of the tracker. It sends `set_entity_motion` at an update when
the velocity changed by a squared distance of more than `1.0E-7`.

**PHY-CMP-003** (`source_suggested`, semantic, confidence: medium). Let `δ` be the difference
between the position of the server and the result of the exact model. A player sees this:

| Category | `δ = 1.0E-4` | `δ = 1.0E-2` |
|----------|--------------|--------------|
| own player | a correction by the movement checks, or a later difference of the client and the server | the same |
| other player, living entity | nothing. `δ` is less than one unit of `1/4096` block. | the entity shows 1 cm away. The interpolation hides the change. |
| a category that the client simulates | nothing visible. At a precise update, the client continues from the new state. | at each update, the entity jumps by up to 1 cm. An item at rest can show 1 cm above the ground, or the client item falls and then jumps back up. |

A difference can grow when it changes a collision result, for example when an item falls into a gap
on one side only. Then the client shows a jump at the next update.

**PHY-CMP-004** (`source_suggested`, semantic, confidence: medium). An f32 value replaces an f64
value with these errors:

- An f32 world coordinate has a step of `|x| × 2^-23` or less. At 2048 blocks from the origin, this
  step is `2.44E-4`, one unit of `1/4096`. At 16384 blocks it is `0.00195`, and at `3.0E7` blocks it
  is 2. Thus an f32 world coordinate is visible beyond 2048 blocks, and it breaks the collision
  algorithm.
- An f32 coordinate relative to an origin less than 16 blocks away has a step of less than
  `2.0E-6`.
- An f32 velocity with `|v| ≤ 4` has an error of less than `2.4E-7` for each operation.

**PHY-CMP-005** (`source_suggested`, client agreement, confidence: medium). Policy `exact`: the own
player. Each rule of sections 1 to 9 that applies to a player must give the result of the vanilla
client, bit for bit. The server re-application of PHY-SRV-003, the query of PHY-SHP-012 and the
velocity of PHY-SRV-008 have the same policy. An implementation must not use the tolerance of
PHY-CMP-006 for them, because `movement-checks.md` compares their results with the client.

PHY-SRV-008 is exact for its input, not for the velocity of the client. The client computes the
new position as `old + r`, in f64. The observed `Δ = new - old` can differ from `r` by the
round-off of that sum and of this difference. The error is at most one unit in the last place of
the larger coordinate.

The candidate velocity then carries an error of the same size. The checker
must compare positions with `offset_threshold` (MVC-PRED-009), which is much larger. It must not
use bit equality of the candidate velocity.

**PHY-CMP-006** (`source_suggested`, semantic, confidence: medium). Policy `semantic`, with a
tolerance: each server-simulated entity. This covers living entities that are not players, and
each category of PHY-ORD-005 except the player. An implementation can replace the f64 model with
a faster one, for example f32 arithmetic in a local frame, when all these conditions are true:

- From the same start state and world, each coordinate of the position after one tick differs
  from the exact model by at most `1.0E-6` block.
- Each velocity component differs by at most `1.0E-6` block per tick.
- The result flags and the supporting block are equal. They can differ only when the exact
  distance from the box to a grid line of a collider is at most `1.0E-6`.
- The rest states of PHY-SKIP-003 stay fixed points, with the same values within `1.0E-6`.
- The positions stay f64 in world coordinates, or f32 relative to an origin less than 16 blocks
  away (PHY-CMP-004).

The tolerance is 4 times less than the unit of `1/4096` block. PHY-DEV-001 records this
relaxation.

**PHY-CMP-007** (`source_suggested`, semantic, confidence: medium). Policy `exact`: the rules that
decide a discrete result for a server-simulated entity. These are the step-up candidates
(PHY-STP-004), the on-ground flag, the item rest cycle (PHY-SKIP-003) and the type flags of
PHY-ORD-009. An implementation can use the tolerance of PHY-CMP-006 for the values that these rules
read, but not for the rules themselves.

**PHY-CMP-008** (`source_suggested`, semantic, confidence: medium). Policy `semantic`: the order of
the ticks of server-simulated entities. An implementation can give each entity the boxes of the
colliders after the pre-pass of PHY-CMP-018. It gives the boxes of the other entities from the
start of the tick. The reference gives the boxes after the ticks of the earlier entities. The difference is
at most the displacement of one collider in one tick.

PHY-DEV-002 records this relaxation. The own player is not in this rule, because the client of the
player sees the colliders that the server sent (PHY-CMP-001). PHY-CMP-009 states the same
relaxation for the block states.

**PHY-CMP-009** (`source_suggested`, semantic, confidence: medium). Policy `semantic`: the block
states that the physics part of the tick of a server-simulated entity reads. An implementation can
read each block state from a copy. The copy holds the block states at the end of the previous
server tick. Thus the copy comes before the server handles the client packets (step 1 of
PHY-ORD-010). The copy serves these reads:

- the collision shapes (section 3) and the supporting block (PHY-FLG-004);
- the block friction, the speed factor and the jump factor (section 6);
- the stuck multipliers, the honey slide and the slime step (PHY-GRV-014, PHY-GRV-015,
  PHY-GRV-018);
- the climbable blocks (PHY-GRV-019) and the bounce restitution (PHY-RST-003);
- the selection of the step actions, the inside actions and the fall actions that the tick
  starts (PHY-RCH-002);
- the coarse blocks of PHY-ORD-013.

A block change after the copy becomes visible to physics at the next server tick. This applies to
a change by a client packet, for example a player who breaks a block. It also applies to a change
by a scheduled tick, a random tick, a block event or a block entity. A change by another entity
or by a deferred block action is also late by one tick. The delay is at most one tick for each change.

**PHY-CMP-010** (`source_suggested`, semantic, confidence: medium). Under PHY-CMP-009, a block
action that the tick starts still applies in the same server tick, under the order of its domain
(PHY-RCH-002). The action uses the block state at the time when it applies. If that block no
longer has the state that started the action, the action does nothing. The physics of each entity
sees the result of the action at the next server tick.

**PHY-CMP-011** (`source_suggested`, client agreement, confidence: medium). PHY-CMP-009 does not
apply to the own player. The client of the player has local authority, and it sees the block
changes when their packets arrive. The re-application of PHY-SRV-003 and the query of PHY-SHP-012
use the world view that `movement-checks.md` states. That view uses windows of acknowledgement for
block changes. PHY-CMP-009 also does not apply in the exact-parity mode (PHY-ORD-010).

**PHY-CMP-012** (`source_suggested`, semantic, confidence: medium). Policy `semantic`: the push
between server-simulated entities. An implementation can compute the push in this way:

1. Each entity `E` finds its neighbours. A neighbour is an entity whose box intersects the box of
   `E`. Both boxes are the boxes at the start of the tick.
2. For each neighbour `O`, count the directions that apply: `O` pushes `E`, and `E` pushes `O`. Each
   direction uses the conditions of PHY-PSH-002 to PHY-PSH-004 with the values at the start of the
   tick.
3. For each direction that applies, `E` gets the push that PHY-PSH-004 gives to it. Both directions
   give the same vector to `E` (PHY-PSH-005).
4. `E` adds the pushes to its own velocity in step 9 of its tick, one at a time. The order is the
   birth key of `O` (PHY-CMP-013), from the lowest to the highest. For one `O`, the push from `O` comes first.
5. The count of the crowd damage of PHY-PSH-007 uses the neighbours of step 1, without the
   neighbours that DMG-CRW-002 leaves out.

Each entity writes only its own velocity and its own health. The fixed order of step 4 makes the
f64 sum independent of the order of the ticks. PHY-DEV-004 records this relaxation. The own player
is not in this rule (PHY-PSH-008, PHY-PSH-010).

**PHY-CMP-013** (`source_suggested`, semantic, confidence: medium). The order key of each
order-independent rule is the birth key of the entity. The network id is not an order key, because
an implementation can give network ids in a way that depends on the layout. The birth key has three
parts, compared in this order:

1. the birth tick: the value of the birth clock of the world in the tick of the birth;
2. the source key: the key of the parent or of the source, as the table states;
3. the sequence: the number of the entity among the entities of the same source and tick, from 0.

| Way of birth | Source key |
|--------------|------------|
| another entity creates it, for example a drop, a projectile, a child or an experience orb | the kind `entity`, then the birth key of the parent |
| a block creates it, for example a dispenser, a spawner, a primed TNT from a block or the drops of a block | the kind `block`, then the block position, compared by `y`, then `z`, then `x` |
| a natural spawn | the kind `spawn`, then the chunk position, then the number of the spawn attempt in that chunk and tick |
| a load from a save that has no birth key | the kind `load`, then the chunk position, then the position of the entity in the saved list of that chunk |
| a command | the kind `command`, then the sequence number of the command in the tick |
| a player that joins with no saved birth key | the kind `player`, then the order of the login in that tick |

The kinds compare in this order: `load`, `player`, `command`, `spawn`, `block`, `entity`. The
sequence of a source depends only on the order of the work of that source. Thus the birth key does
not depend on the layout, on the thread count or on the network id.

The birth key is saved with the entity, as data of this implementation. An entity that the
implementation loads keeps its saved birth key. A change of dimension keeps the birth key. The
vanilla server ignores this data, and the client never sees it.

A player keeps its saved birth key at each login, and also at a respawn. The player is the same
entity with the same UUID, so it keeps its place in the order. Only a player without a saved
birth key gets a new key of the kind `player`.

The birth clock is a counter of each world, shared by all its dimensions, because a key survives a
change of dimension. It has these properties:

- It increases by 1 at the start of each server tick, also in a tick in which the tick rate is
  frozen. A command can create an entity in such a tick.
- It never gives a value two times and never goes back, also after a crash. It can jump
  forward.
- The world save keeps a value of the clock, as data of this implementation. A load continues
  from that value. The save can hold a value that is ahead of the clock by one autosave
  interval or more. Then a crash after the save can not give a value that a tick already used.
- No command changes it.
- A save without the value starts the clock at the game time of that save.

The birth clock is not the game time and not a clock of `command-time.md`. The game time of the
reference is saved, and no command sets it. But the game time does not advance while the tick
rate is frozen, so two ticks can have the same game time. Thus the game time can not be the birth
tick.

The source key of the kind `entity` contains the birth key of the parent. That key can nest to any
depth. An implementation can store it in a fixed size, as a digest of 128 bits of the birth key of
the parent. The digest is a fixed function of the full key. With this form, these order rules
apply:

- Two entities with the same birth tick and the same parent keep the order of their sequence.
- Two entities with the same birth tick and different parents rank by the digest of the parents,
  not by the full keys of the parents.
- Two different parent keys can have the same digest. Then the stored keys of two entities can be
  equal. PHY-CMP-024 states the comparison for this case.

The order of the digests does not follow the order of the full keys. This is permitted, because the
order only needs to be total, fixed and independent of the layout.

**PHY-CMP-014** (`source_suggested`, semantic, confidence: medium). Policy `semantic`: the random
value of the crowd damage (PHY-PSH-007, step 3). An implementation can take this value from a
stream. The stream is a function of the birth key (PHY-CMP-013) and of the tick number.

It takes one value for each entity and each tick, and it uses the value only when step 3 runs. Thus the value does not
depend on the order of the entities or on the other uses of a random value. The chance of 1 in 4
stays the same. PHY-DEV-006 records this relaxation.

**PHY-CMP-015** (`source_suggested`, semantic, confidence: medium). Policy `semantic`: a write from
the tick of one entity to another entity. Examples are damage, knockback, a pickup of an item and
the removal of an entity. In the order-independent pass, such a write does not change the target
in the same tick. The writer records it, and the target applies it at the start of the next tick,
before step 1 of its tick.

When several writes have the same target, the target applies them in the order of the birth key
of the writer (PHY-CMP-013). A claim, for example a pickup, uses PHY-CMP-017 instead. The push is
not such a write, because each entity computes its own push (PHY-CMP-012). The block domain does
not use this rule. Its writes apply in the same tick (PHY-BLK-001). PHY-DEV-005 records this
relaxation.

**PHY-CMP-016** (`source_suggested`, semantic, confidence: medium). A rule that reads the network
id for the choice of a tick reads the phase offset in its place. The item rest cycle of
PHY-ORD-005 and PHY-SKIP-003 is such a rule. The phase offset is a fixed function of the birth key,
from 0 to 3. It is saved with the birth key.

For a vanilla parity test, the test sets the
phase offset of each item to the remainder of the vanilla network id divided by 4. The test reads
that id from the `add_entity` packet of the vanilla server. PHY-DEV-007 records the difference.

A test without the packets can fit the phase offset to the record. The test selects the offset
from 0 to 3 that gives the recorded ticks with `v.y` at 0 (PHY-SKIP-003). Four rest ticks in a row
select one offset. The fit is valid only when the record has these four ticks.

**PHY-CMP-017** (`source_suggested`, semantic, confidence: medium). Policy `semantic`: claims on one
entity. A claim is a write that only one writer can make. Examples are these writes:

- the pickup of an item by a player, a mob or a hopper;
- the merge of two items or of two experience orbs;
- the removal of an entity.

The order-independent pass handles claims in this way:

1. During the tick, each writer records a claim. It does not change the target.
2. After the block domain of the tick (PHY-BLK-001), one arbitration step collects the claims of
   each target.
3. The claim with the lowest writer key wins. Only the winner gets the result, for example the
   item. The other claims have no effect.
4. The winner and the target apply the result before the server sends the changes of the tick.
5. One writer can win several claims in one tick, for example several items for one inventory. The
   writer applies its grants in the order of the birth key of the targets. The lowest key comes
   first. Thus the layout does not change the slots that the items fill. It also does not change
   which item does not fit into a full inventory.

The writer key is total over all writers. It compares first the kind of the writer, in this order:
`command`, `entity`, `block`. Then it compares a second value. For an entity, this is the birth key.
For a block, this is the block position, by `y`, then `z`, then `x`. For a command, this is the
sequence number of the command.

This order follows the reference: commands run before the entities, and the hoppers tick after the
entities. PHY-DEV-005 records the difference.

The arbitration of a tick must contain each claim of that tick, also a claim from another cell. An
implementation must deliver the claims of a tick to the owner of the target before the
arbitration of that tick. A layout that delivers a claim one tick later uses the exception "claim
across a cell boundary" of PHY-INV-014. Its bound is one tick, only for that target. The late claim
joins the arbitration of the next tick. The target still has one winner in each tick, so an item
cannot go to two writers.

**PHY-CMP-018** (`source_suggested`, semantic, confidence: medium). Policy `semantic`: the colliders
of PHY-SHP-009, for example boats, rafts, shulkers and a happy ghast. The order-independent pass
steps them in a pre-pass, before the other entities:

1. The colliders whose read zones (PHY-RCH-001) intersect form a group. Two groups that share a
   collider are one group.
2. Inside a group, the colliders step one after the other, in the order of the birth key. A
   collider reads the box after the step of each earlier collider of the group. It reads the box
   at the start of the tick of each other collider.
3. A collider reads the boxes of the entities that are not colliders from the start of the tick.
4. The pre-pass publishes the box of each collider after its step.

The main pass then gives each entity the boxes of the colliders after the pre-pass. It gives the
boxes of the other entities from the start of the tick (PHY-CMP-012). Two colliders of one group cannot pass
through each other, because the later one always sees the box of the earlier one after its step.
PHY-DEV-002 records the difference from the reference.

**PHY-CMP-019** (`source_suggested`, semantic, confidence: medium). Policy `semantic`: a limit on the
number of push neighbours. It applies only when the game rule `max_entity_cramming` is 0. Then the
push of PHY-CMP-012 uses only the `K` nearest neighbours of each entity, with `K = 24` by default.

1. The distance of a pair is `m` of PHY-PSH-004, from the positions at the start of the tick.
2. Each entity sorts its neighbours by the distance, and then by the birth key.
3. A pair pushes only when each entity of the pair is among the first `K` neighbours of the other.

Step 3 makes the rule symmetric, so each pair still gets equal and opposite pushes (PHY-PSH-005).
PHY-DEV-008 records the difference.

**PHY-CMP-020** (`source_suggested`, semantic, confidence: medium). Policy `semantic`, with the
visible behaviour of the reference: explosions in the order-independent pass.

1. At the start of the tick, find each entity that explodes in this tick by a known rule. A primed
   TNT with a fuse of 1 or less is such an entity.
2. Make an explosion group. It contains these entities, and each entity whose read zone touches
   the reach of one of their explosions. For a power `p`, the reach is a cube of edge `4p` around
   the centre. Grow it by the largest displacement of the exploder in this tick.
3. Add each entity that an explosion of the group can ignite or move, until the group does not
   grow. Two groups that share an entity are one group.
4. Two groups whose reaches intersect are also one group, even when they share no entity. Two
   such explosions can remove the same block or ignite the same TNT block. The merge keeps the
   order of the birth key for both.
5. The entities of a group tick one after the other, in the order of the birth key. In the group,
   an explosion changes the velocity and the health of the other members in that step (PHY-EXP-001). A
   member that ticks later uses the impulse in the same tick. A member that ticked before uses it
   in the next tick. The blocks that an explosion removes are absent for the later members.
6. The group reads the entities outside it at their state from the start of the tick.

Some explosions do not show at the start of the tick, for example a fireball that hits a block.
Such an exploder does not form a group. Its impulse and its damage are writes of PHY-CMP-015.
PHY-DEV-009 records this case. PHY-CMP-022 states an opt-in mode for a large group.

**PHY-CMP-021** (`source_suggested`, semantic, confidence: medium). This rule defines the state at
the start of the tick of a player. It is the state after the server accepted the client moves of
this tick (PHY-ORD-010, step 1). This state includes the position, the box and the pose. The rules
PHY-CMP-008, PHY-CMP-012 and PHY-CMP-018 read the player in this state. Thus an entity sees a
player move in the same tick, as in the reference.

An implementation can handle the moves inside the tick, before the entity pass, and it then
conforms. The copy of each cell must contain these player states before the entity pass
(PHY-INV-014).

An implementation can instead read each player at its state from the end of the previous tick, in
each cell. PHY-DEV-012 records this choice.

**PHY-CMP-022** (`source_suggested`, semantic, confidence: medium). Policy `semantic`, opt-in: the
wave mode of an explosion group. The mode is off by default. An implementation can turn it on for a
group with more than `N` members, where `N` is a setting. In the wave mode:

1. Each explosion of the group traces its rays and its exposures. It uses the state of the group
   at the start of the tick (PHY-EXP-003).
2. For each member, sum the impulses and the damage of all explosions. The order of the sum is the
   birth key of the exploders.
3. The members then move, in the order of the birth key, with the summed impulses.
4. The blocks of all explosions are removed after step 1. A block that two explosions remove is
   removed one time.

PHY-DEV-011 records this mode.

**PHY-CMP-023** (`source_suggested`, semantic, confidence: medium). The copy of the player on the
server gets these changes of its velocity. It never sends this velocity to the client of the
player.

1. When the server handles a move that goes up from the ground, the copy jumps (PHY-INP-005).
2. During the entity pass, a server-simulated entity can push the copy and can give it an external
   impulse. In the reference, the push lands in the tick of the pusher (PHY-PSH-006).
   Under PHY-CMP-012, the copy is a push neighbour with its state of PHY-CMP-021. The push to the
   copy then lands in the server part of the player.
3. In the server part of the player (PHY-ORD-010, step 6), the copy does these steps in this order:
   - It multiplies the velocity by 0.98 (PHY-GRV-009).
   - It sets small components to zero (PHY-GRV-008).
   - It applies the block effects (PHY-SRV-004).
   - It pushes the server-simulated entities near it, and it gets the opposite push
     (PHY-PSH-004).

The copy does not do the travel step. `movement-checks.md` reads this velocity as the velocity that
the server expects.

**PHY-CMP-024** (`source_suggested`, semantic, confidence: medium). The comparison of two stored
birth keys has these steps:

1. Compare the birth ticks.
2. Compare the kinds of the source keys, in the order of PHY-CMP-013.
3. Compare the values of the source keys. A digest compares as an unsigned integer of 128 bits.
4. Compare the sequences.
5. If all these are equal, compare the UUIDs of the two entities, as unsigned integers of 128 bits.
   The implementation must make the UUID when the entity comes into existence. The input is the
   stored key of the entity. For the kind `entity`, the input also holds the UUID of the parent.
   The function must be independent of the digest function.
6. If the UUIDs are also equal, the order of the two entities is not specified.

Step 6 occurs only when two independent functions of 128 bits both collide. Its probability for one
pair is about `2^-256`. Two children can have equal stored keys because the digests of their
parents collide. Their UUIDs then differ, unless the UUIDs of the parents are also equal. Thus a
collision upstream does not raise the probability of step 6 to `2^-128`.

**PHY-CMP-025** (`source_suggested`, semantic, confidence: medium). Policy `semantic`: the start
state of an item that a block drops. Let `(bx, by, bz)` be the block position. Let `U(a, b)` be
`a + r × (b - a)`, with `r` a uniform f64 random value in `[0, 1)`. Let `w` and `h` be half the
width and half the height of the item, both 0.125.

A drop at the block, for example a drop of a block destruction (SIM-SET-004), starts with these
values:

- the position `(bx + 0.5 + U(-0.25, 0.25), by + 0.5 + U(-0.25, 0.25) - h, bz + 0.5 + U(-0.25, 0.25))`;
- the velocity `(U(-0.1, 0.1), 0.2, U(-0.1, 0.1))`.

A drop from one face of the block has the unit normal `(sx, sy, sz)` of the face. On each axis with
a normal component of 0, the position and the velocity use the random values of the table. The
other axis uses fixed values.

| Axis | Position, component 0 | Position, component `s` | Velocity, component 0 | Velocity, component `s` |
|------|-----------------------|-------------------------|-----------------------|-------------------------|
| `x` | `bx + 0.5 + U(-0.25, 0.25)` | `bx + 0.5 + s × (0.5 + w)` | `U(-0.1, 0.1)` | `s × 0.1` |
| `y` | `by + 0.5 + U(-0.25, 0.25) - h` | `by + 0.5 + s × (0.5 + h) - h` | `U(0, 0.1)` | `s × 0.1 + 0.1` |
| `z` | `bz + 0.5 + U(-0.25, 0.25)` | `bz + 0.5 + s × (0.5 + w)` | `U(-0.1, 0.1)` | `s × 0.1` |

The random values come from a stream that is a function of the birth key of the item
(PHY-CMP-013). The block is the source of the birth key. PHY-DEV-013 records this choice. E54
confirms the drop at the block. The drop from a face has no experiment yet.

**PHY-CMP-026** (`source_suggested`, wire, confidence: medium). The entity data of an item entity
has one value after the values of each entity. Its index is 8. Its serializer id is 7, the item
stack. The value is the stack of the item, in the optional item stack form, with the count first.
The server sends it in `set_entity_data` after the `add_entity` of the item. It sends it again when
the stack changes. The client shows an item entity with an empty stack as nothing.

## 12. Out of scope

Each subject below has a hook in this model. This document does not state the model behind the
hook.

| Subject | Hook |
|---------|------|
| Fluids: swimming, currents, buoyancy, the jump in a fluid, the fall multiplier of a fluid | PHY-ORD-003 selects the fluid model. PHY-INP-004 gives the jump to the fluid model first. PHY-FALL-001 step 3 uses the fall multiplier. PHY-SHP-003 gives the source shape of a fluid to an entity that can stand on it. |
| Elytra flight | PHY-ORD-003 selects the elytra model. PHY-BOX-004 gives the pose. |
| Riptide | PHY-BOX-004 gives the `spin_attack` pose. The impulse is an external impulse (PHY-SRV-007). |
| Vehicles and riders | PHY-STP-001 gives a ridden living entity a step height of at least 1. PHY-SHP-008 excludes the root vehicle. PHY-ORD-006 lists the vehicle categories. A rider has no travel step of its own. |
| The geometry of a piston push | PHY-BLK-002 states the move. The selection of the entities that a piston moves, and the stick of slime and honey blocks, are in the block domain. |
| Fall damage amounts | step 4 of PHY-FALL-001 calls the fall action of the block at the effect position. `entity-damage.md` states the damage (DMG-FALL-001). |
| Hit detection of projectiles | the move `R` along a ray of PHY-ORD-005. |
| Movement checks of the server | section 11 and `movement-checks.md`. |

## 13. Intentional deviations

No rule of this document is a deviation for the own player. For the player, the vanilla client computes the same result. Thus parity with the client is
more important than a cleaner rule. This applies to PHY-COL-005, PHY-MOV-004, PHY-STP-004 and
PHY-STP-005. Each client sees the result of the server-simulated rules of section 9 as the position
of an entity. Thus this document keeps these rules too.

The rules PHY-CMP-006, PHY-CMP-008, PHY-CMP-009, PHY-CMP-012, PHY-CMP-014 to PHY-CMP-022 permit
the deviations of this section for server-simulated entities. An implementation that uses one of them must state it.

### PHY-DEV-001: Tolerance for server-simulated motion

- **Rule:** PHY-CMP-006.
- **Reference:** the reference computes the motion of each server-simulated entity in f64, in the
  order of this document.
- **Visible to the client:** no, within the tolerance. A difference of `1.0E-6` block is less than
  one unit of `1/4096` block. An entity that the client simulates continues from the state that the
  server sent, so the client does not see the difference either.
- **Test:** a differential test compares each position and velocity component with a tolerance of
  `1.0E-6` per tick. It compares the flags exactly, except within `1.0E-6` of a grid line.

### PHY-DEV-002: Collider boxes after a pre-pass

- **Rule:** PHY-CMP-008, PHY-CMP-018.
- **Reference:** the reference ticks entities one after the other. An entity sees the new box of a
  collider that ticked before it in the same tick. It sees the old box of a collider that ticks
  after it.
- **Visible to the client:** yes, rarely. Under PHY-CMP-018, each entity that is not a collider
  sees the new box of each collider. A boat, a shulker or a happy ghast can move. An entity that
  ticks before it in the reference can stop one tick earlier. The bound is the displacement of the collider
  in one tick. Two colliders do not pass through each other, as in the reference. Inside a group
  of colliders, the order is the birth key, not the order of the ticks.
- **Test:** a differential test skips an entity that has a collider that moves in its read zone.

### PHY-DEV-003: Block states from the start of the tick

- **Rule:** PHY-CMP-009.
- **Reference:** the reference changes blocks and ticks entities in one order (PHY-ORD-010). An
  entity sees each block change that occurred before its own tick in the same server tick. This
  includes the changes by the client packets that the server handled before the tick. Block
  ticks and block events occur before the entities. A block change by an entity is visible to the
  entities that tick after it.
- **Visible to the client:** yes, as a delay of one tick at most. Examples:
  - A player breaks the block under an item at rest. In the reference, the server handles the
    packet before the tick, and the item starts to fall in the next tick. Under PHY-CMP-009, the
    copy does not contain the change, and the item starts to fall one tick later.
  - A scheduled tick removes the block under an item at rest. In the reference, the item starts to
    fall in the same tick. Under PHY-CMP-009, it starts to fall one tick later.
  - A `falling_block` lands and places sand. In the reference, an entity that ticks later in the
    same tick collides with the sand. Under PHY-CMP-009, the entity collides with it one tick later,
    and it can move into the cell of the sand for one tick. In the next tick, the box
    already overlaps the shape of the sand. That shape does not stop the box on an axis where the
    overlap is more than `1.0E-7` (PHY-COL-003).
  - Second effect, in a layout of many cells: an entity lands on `farmland` in a column that a
    neighbour cell owns. The farmland changes to dirt one tick later than in a layout of one cell.
    Physics sees the dirt at `T + 2` (PHY-INV-014).
- **Test:** a differential test skips some entities. It skips an entity when a block in its read zone
  changed after the previous server tick. For the second effect, the test accepts a delay of one
  more tick for the block that the action changed. It compares the other blocks and entities with
  no extra delay. It compares the next tick with a delay of one tick.

### PHY-DEV-004: The push from the start of the tick

- **Rule:** PHY-CMP-012.
- **Reference:** the reference pushes in sequence (PHY-PSH-006). The later entity of a pair uses the
  push of the earlier entity in the same tick. The two pushes of a pair use different positions.
- **Visible to the client:** yes, as a small difference in the motion of a crowd. The bound: for
  each pair and each tick, each horizontal velocity component differs by at most `0.1`, which is two
  pushes of `0.05F`. A push can arrive one tick later than in the reference. Examples:
  - In a crowd in a hole of 1 × 1 blocks, the reference pushes the entities that tick later a bit
    more in the same tick. Under PHY-CMP-012, the push is symmetric, and the crowd spreads in the
    same way in each direction.
  - In a mob farm, the paths of single mobs differ, but the mean flow of the mobs stays the same.
  - The crowd damage uses the same count rule and the same chance of 1 in 4. Only the tick in which an
    entity takes damage can differ, because the count uses the boxes from the start of the tick.
- **Test:** a differential test compares one pair with a tolerance of `0.1` per tick for the
  velocity. For a crowd, it compares the distribution of the positions and the rate of the crowd
  damage. It does not compare each position.

### PHY-DEV-005: Writes to other entities at the next tick

- **Rule:** PHY-CMP-015.
- **Reference:** the reference applies a write to another entity when the writer makes it. A target that ticks later
  in the same tick sees the write in that tick.
- **Visible to the client:** yes, as a delay of one tick at most. Examples:
  - A mob hits another mob. In the reference, the target can use the knockback in its travel step
    of the same tick. Under PHY-CMP-015, it uses the knockback one tick later.
  - Two players or a player and a hopper touch one item in the same tick. In the reference, the
    writer that acts first gets the item. Under PHY-CMP-017, the writer with the lowest writer key
    gets it, in the same tick.
  - The health of a target, and its death, can change one tick later.
- **Test:** a differential test accepts a delay of one tick for each write to another entity. For a
  conflict, it accepts each winner.

### PHY-DEV-006: Keyed random value for crowd damage

- **Rule:** PHY-CMP-014.
- **Reference:** the reference takes the value from the random source of the entity. Other parts of
  the tick of the entity, for example the AI, take values from the same source.
- **Visible to the client:** no, as a distribution. The ticks with crowd damage differ from the
  reference, but the chance stays 1 in 4.
- **Test:** a differential test compares the rate of crowd damage, not the ticks.

### PHY-DEV-007: Phase offset from the birth key

- **Rule:** PHY-CMP-016.
- **Reference:** the phase of the item rest cycle comes from the network id of the item.
- **Visible to the client:** no in normal play. The phase changes the ticks in which an item at rest
  moves by less than `0.0032` block. It also changes the value of `v.y` in the cycle. A position update
  reaches the client only after a change of at least `0.00276` block (PHY-CMP-002).
- **Test:** a parity test sets the phase offset from the vanilla network id (PHY-CMP-016).

### PHY-DEV-008: Push limit without crowd damage

- **Rule:** PHY-CMP-019.
- **Reference:** the reference pushes every candidate, also when `max_entity_cramming` is 0.
- **Visible to the client:** yes, but only when the game rule is 0. More than 24 entities must
  touch one entity. The crowd presses closer together, because each entity gets at
  most 24 pairs of pushes.
- **Test:** a differential test with `max_entity_cramming` 0 skips each entity with more than `K`
  neighbours.

### PHY-DEV-009: Explosions that the start of the tick does not show

- **Rule:** PHY-CMP-020.
- **Reference:** each explosion acts in the tick of its exploder, in the order of the ticks.
- **Visible to the client:** yes, as a delay of one tick at most. A fireball or a wither skull can
  explode on a hit. It then pushes some entities one tick later than in the reference. These are
  the entities that tick after it in the reference. Primed TNT, and thus TNT cannons, keep the behaviour of the reference.
- **Test:** a differential test accepts a delay of one tick for the push and the damage of such an
  explosion.

### PHY-DEV-010: Keyed random values for the rays of an explosion

- **Rule:** PHY-EXP-003, in the semantic mode.
- **Reference:** each ray takes one value from the random source of the dimension, in the fixed
  order of the rays. The other users of that source change the values.
- **Visible to the client:** yes, as a different set of destroyed blocks in each run. The
  distribution of the destroyed blocks stays the same.
- **Test:** a differential test compares the distribution of the destroyed blocks over many
  explosions. It does not compare the blocks of one explosion.

### PHY-DEV-011: Wave mode of an explosion group

- **Rule:** PHY-CMP-022. The mode is off by default.
- **Reference:** the explosions of one tick act one after the other (PHY-EXP-002). A later
  explosion sees the blocks, the positions and the velocities after the earlier explosions.
- **Visible to the client:** yes.
  - Craters differ. In the reference, a later explosion traces through the blocks that an earlier
    explosion removed, so it can reach deeper. In the wave mode, each explosion sees the blocks of
    the start of the tick.
  - Cannons differ when their charges push each other in the same tick. In the reference, a charge
    that an earlier charge pushes explodes at its new position. In the wave mode, it explodes at its
    position from the start of the tick.
- **Test:** a differential test runs with the wave mode off. With the mode on, it compares only the
  number of removed blocks and the mean impulse.

### PHY-DEV-012: Player positions from the end of the previous tick

- **Rule:** PHY-CMP-021, the second paragraph.
- **Reference:** the entities of a tick see the player at the position that the server accepted
  before that tick.
- **Visible to the client:** yes, a little. Under this choice, the entities see a player move one
  tick later. A mob pushes the player, and the player pushes a mob, from a position that is one tick
  old. The difference is at most the displacement of the player in one tick. The push to the copy
  of the player on the server is not visible to the client.
- **Test:** a differential test accepts a delay of one tick. This applies to each interaction of an
  entity with the position of a player.

### PHY-DEV-013: Keyed random values for the drop of a block

- **Rule:** PHY-CMP-025.
- **Reference:** the offsets of the position come from the random source of the dimension. The
  velocity comes from the random source of the new item.
- **Visible to the client:** no, as a distribution. The values of one drop differ from the
  reference, but each value has the same range and the same uniform distribution.
- **Test:** a differential test compares the ranges of the start positions and velocities.

### PHY-DEV-014: Zero horizontal speed of an item on the ground

- **Rule:** PHY-ORD-005, the step "Item, stop".
- **Reference:** `v.x` and `v.z` of an item on the ground become smaller in each tick with `M`. On
  most blocks they never become zero, so the item never gets to the rest state of PHY-SKIP-003.
- **Visible to the client:** no. Each removed component is at most `1.0E-6`, the tolerance of
  PHY-CMP-006. The position of an item at rest does not change, because PHY-MOV-004 drops a move
  shorter than `sqrt(1.0E-7)`. When the item moves again, the removed speed gives at most
  `1.0E-6 / (1 - 0.98)`, `5.0E-5` block. This is less than the unit of `1/4096` block.
- **Test:** a differential test compares `v.x` and `v.z` of an item on the ground within `1.0E-6`.

### Known vanilla quirks

The vanilla client does these things, and they look like accidents of its code. The rules keep them
for client agreement. A server that computes another result gives false setbacks to its players.

| Rule | Quirk |
|------|-------|
| PHY-SHP-005 | A box that is off the grid of `1/8` on one axis loses the grid lines of its other axes too. |
| PHY-COL-002 | A request of `-0.0` on an axis gives `+0.0` when the move has colliders, and `-0.0` when it has none. |
| PHY-FLG-003 | A quotient above 1 from f64 error gives NaN, and the minor collision flag is false for a move that is exactly along the input. |
| PHY-FALL-004 | A part of the path with a squared length less than `1.0E-7` does not reset the fall distance, also inside a cobweb. |

## 14. Unresolved

- **U1**: the corpus in `assets/` is of `26.4-snapshot-4`. This document is of `26.4-snapshot-7`.
  Before an implementation uses `minecraft:collision_box` and `minecraft:friction`, a maintainer
  must update the corpus. If the corpus stays the same, a test must show that these components did
  not change.

  The physics fixtures of the vanilla server under
  `crates/mcrs_minecraft_collision_table/tests/fixtures/physics/` are also of `26.4-snapshot-4`.
  Snapshot 4 adds the fall of the tick in the move procedure, before the blocks inside the box. It
  resets the fall distance for the tag `fall_damage_resetting` only in a move of 1 block or more.
  Snapshot 7 uses PHY-ORD-001 step 14 and PHY-FALL-004. Thus the fall distances of the fixtures
  E7, E8 and E17 do not apply to this document. A maintainer must record them again with
  snapshot 7.
- **U2**: no artefact gives the dimensions of each entity type and pose. A `dumpDefinitions`
  task for entity types could write the width, the height, the eye height, the fixed flag and the
  pose dimensions.
- **U3**: the corpus does not give the speed factor, the jump factor, the bounce restitution and
  the fall distance reduction of a block. Section 6.6 gives the vanilla values as data. A corpus
  component would let a test compare them.
- **U4**: the corpus rounds each shape value to `1.0E-4` pixel. A shape value that is not a
  multiple of `1.0E-4 / 16`, for example a value that comes from an f32 constant, is not exact.
  The exact f64 values are necessary for client agreement.
- **U5**: the shapes of PHY-SHP-004. These are the offset function of a block position and the
  shape of a `moving_piston`. PHY-SHP-014 states the shape of a shulker box.
- **U6**: the corpus gives the boxes of a shape, not its grid lines. PHY-SHP-005 derives the grid
  lines from the boxes, and this can differ from the reference when the game merged boxes with
  different grids. A corpus component with the grid lines of each axis would remove the doubt.
- **U7**: the order in which the blocks inside the box act, when two blocks set a stuck multiplier
  in one tick.
- **U8**: PHY-GRV-020 states the motion of the client in a chunk that is not loaded. No
  observation confirms it.
- **U9**: the search for a free position after a size change of a server-simulated entity
  (PHY-BOX-007).
- **U10**: no tool in the repository records the motion of the vanilla client. The experiments
  for the player (E9 to E14) need a vanilla client and a proxy that records the serverbound move
  packets.

## 15. Experiments

Start a vanilla server with `tools/vanilla-server/run.sh serve`. Use the game version in the
table at the top of this document. Set `online-mode=false` and `level-type=minecraft:flat`. Set the
game rule `spawn_mobs` to false. Connect one scripted client as an operator.

Use `/tick freeze` before each step, and `/tick step 1` to run one tick. After each tick, read
`Pos`, `Motion`, `OnGround` and `fall_distance` with `/data get entity`. The `Pos` and `Motion`
values are f64 values. Compare them bit for bit with the model.

| Experiment | Steps | Rules |
|------------|-------|-------|
| E1 | Summon an armor stand 10 blocks above stone with `Motion:[0d,0d,0d]`. Step 40 ticks. Do it again with `NoGravity:1b`. Put a pig in a column of glass 1 block wide, give it the effect `slow_falling`, and do it again. | PHY-GRV-001, PHY-GRV-002, PHY-GRV-007, PHY-ORD-004, PHY-FLG-002 |
| E2 | Summon an armor stand on stone, on ice, on blue ice and on soul sand, with `Motion:[0.5d,0d,0.3d]`. Step until the motion is zero. | PHY-GRV-006, PHY-GRV-008, PHY-GRV-012, PHY-INV-011 |
| E3 | Summon an item, a falling block of sand, a primed TNT and an experience orb 5 blocks above stone, with a horizontal `Motion`. Step until they rest. Do it again on a slime block. | PHY-ORD-005, PHY-RST-003 |
| E4 | Summon an armor stand with `Motion:[0.6d,0d,0d]` towards a wall of stone, a fence, a slab, a stair and a carpet, from several distances. Step 5 ticks. | PHY-COL-001 to PHY-COL-005, PHY-RST-002, PHY-SHP-007 |
| E5 | Summon a zombie on stone in front of a slab, a full block and a snow layer of 2, 3 and 5 layers. Give it `Motion:[0.3d,0d,0d]`. Step 5 ticks. Do it again with the attribute `step_height` at 1.0 and at 0. | PHY-STP-001 to PHY-STP-005, PHY-INV-004 |
| E6 | Summon an armor stand 6 blocks above a slime block, a bed, a shelf mushroom and a honey block, with the attribute `bounciness` at 0 and at 0.5. Step until it rests. | PHY-RST-001 to PHY-RST-004, PHY-GRV-017 |
| E7 | Summon an armor stand and an item above cobweb, a sweet berry bush and powder snow. Step 20 ticks. | PHY-MOV-002, PHY-MOV-003, PHY-GRV-014, PHY-FALL-002 |
| E8 | Summon an armor stand next to a ladder and next to a honey block, 5 blocks up, with a small `Motion` towards the block. Step 30 ticks. | PHY-GRV-015, PHY-GRV-019, PHY-FALL-001 |
| E9 | With a vanilla client, walk, sprint and sneak on stone and ice for 40 ticks each. Record the serverbound positions. | PHY-INP-001, PHY-INP-002, PHY-INP-006, PHY-INP-007, PHY-GRV-010 |
| E10 | With a vanilla client, jump on stone, on honey and with `jump_boost` 1. Sprint and jump. | PHY-INP-004, PHY-INP-005, PHY-GRV-013 |
| E11 | With a vanilla client, sneak towards the edge of a block from 8 directions. | PHY-INP-012, PHY-INV-012 |
| E12 | With a vanilla client in creative mode, start the flight state, go up and down, sprint, and land. | PHY-INP-013 to PHY-INP-015 |
| E13 | With a vanilla client, walk into a wall at angles of 5, 8 and 12 degrees while the sprint state is on. | PHY-FLG-003, PHY-INP-009 |
| E14 | With a vanilla client, crouch under a slab of height 1.5 and stand up. Use the attribute `scale` at 0.5 and 2. | PHY-BOX-001 to PHY-BOX-006, PHY-INP-010, PHY-INP-011 |
| E15 | Set a world border of size 10. Summon an armor stand with `Motion` towards the border, at 1 and 3 blocks from it. | PHY-SHP-010 |
| E16 | Summon a boat and a shulker. Give an armor stand `Motion` towards each. Do it again with a minecart. | PHY-SHP-008, PHY-SHP-009 |
| E17 | Put an armor stand into scaffolding and powder snow from above, with `fall_distance` at 0 and at 3. | PHY-SHP-003, PHY-GRV-016 |
| E18 | Give an armor stand the attributes `friction_modifier` 0.5 and 2, `air_drag_modifier` 0.5 and 2, `movement_efficiency` 1. Repeat E2. | PHY-GRV-005, PHY-GRV-006, PHY-GRV-012 |
| E19 | Summon a zombie with `Motion:[0.1d,0d,0.0001d]` and with `[0.002d,0d,0.002d]`. Step 2 ticks. | PHY-GRV-008, PHY-MOV-004 |
| E20 | Repeat E1, E4 and E5 two times from the same world copy. Compare the results bit for bit. | PHY-INV-008, PHY-INV-009 |
| E21 | With a vanilla client, send `/tp @s ~1 ~ ~ ~10 ~` while the player falls. Then explode TNT near the player. Record the positions of the next 5 ticks. | PHY-SRV-009, PHY-SRV-010, PHY-SRV-008 |
| E22 | With a vanilla client and a view distance of 2, walk to the edge of the loaded chunks and drop from a height. | PHY-GRV-020 |
| E23 | With a vanilla client, double-tap forward, hold sprint while an item is in use, double-tap jump in creative mode, and land. | PHY-INP-017, PHY-INP-018, PHY-INP-019, PHY-ORD-007 |
| E24 | Put the scripted client on scaffolding and on powder snow. Send a move down into the block, with the shift key off and on. Read whether the server sends a correction. | PHY-SHP-012, PHY-SHP-013 |
| E25 | Summon a spider and a zombie in cobweb, a fox and a pig in a sweet berry bush, and an item in a sweet berry bush. Drop sand onto powder snow. Summon an armor stand with `Marker:1b` and `Motion:[0.3d,0d,0d]`. Step 10 ticks each. | PHY-ORD-009, PHY-GRV-014, PHY-SHP-003 |
| E26 | Record the clientbound packets for an item, a falling block, a primed TNT, an armor stand and a zombie that move and come to rest. Read the form of each position update and the ticks between them. | PHY-CMP-001, PHY-CMP-002 |
| E27 | With a vanilla client, change the `Pos` of an item at rest and of an armor stand by `1.0E-4` and by `1.0E-2` with `/data merge entity`. Take screenshots at a known position. | PHY-CMP-003 |
| E28 | Summon an armor stand, an item, a primed TNT with a long fuse and an experience orb on stone. Step 12 ticks and read `Motion` after each tick. | PHY-SKIP-001, PHY-SKIP-003 |
| E29 | Summon two armor stands next to a boat that moves. Repeat with the order of the summons changed. | PHY-RCH-004, PHY-CMP-008 |
| E30 | Summon three armor stands with `Motion` towards one boat in the order A, B, C, then in the order C, B, A. Summon an armor stand that rides another. Record the positions of each tick. | PHY-ORD-010, PHY-ORD-011 |
| E31 | Summon an armor stand, a bat, a parrot and a zombie next to a boat that moves. Record which entity the boat stops at. | PHY-ORD-012 |
| E32 | Step 40 ticks with an item at rest on stone, then run `/data merge entity` with no change in each tick. Compare `Motion` with E28. | PHY-SKIP-005, PHY-RCH-005 |
| E33 | Freeze the ticks. Put an item at rest on scaffolding with no support, so that a scheduled tick breaks the scaffolding. Step 1 tick and read `Pos` and `Motion`. Put an item at rest on stone, break the stone with the scripted client while the ticks are frozen, step 1 tick and read `Pos`. Summon sand above an armor stand that moves under it. | PHY-CMP-009, PHY-CMP-010, PHY-DEV-003 |
| E34 | Set the base `movement_speed` of two pigs to 0. Summon them with a horizontal distance of 0.3, then 0.05, then 0.005. Step 1 tick and read `Motion` of each. | PHY-PSH-002, PHY-PSH-004, PHY-PSH-005 |
| E35 | Summon two pigs with speed 0 in a box of 1 × 1 blocks, in the orders A, B and B, A. Step 5 ticks and read `Pos` and `Motion`. | PHY-PSH-006, PHY-DEV-004 |
| E36 | Put 30 pigs with speed 0 in a box of 1 × 1 blocks with `max_entity_cramming` at 24, 0 and 31. Count the ticks with damage of the type `cramming`. | PHY-PSH-007 |
| E37 | Put two pigs in teams with each collision rule. Step 1 tick and read `Motion`. Put a parrot and a bat next to a pig. | PHY-PSH-003, PHY-ORD-009 |
| E38 | With a vanilla client, stand inside a pig and inside a boat, with and without a team rule `never`. Record the positions of the player. | PHY-PSH-008, PHY-PSH-010 |
| E39 | Summon a zombie with `NoAI:0b` and give it a side step with a target, then remove the target. Read the input with a debug tool of mcrs, or read `Pos` for 200 ticks. | PHY-INP-020, PHY-SKIP-007 |
| E40 | Put a pig on a boat that moves. Step 1 tick and read `Pos` of both. | PHY-ORD-014 |
| E41 | Let a zombie hit a pig that ticks after it, then before it. Read `Motion` of the pig in the next 2 ticks. | PHY-CMP-015, PHY-DEV-005 |
| E42 | Put two pigs with speed 0 at a horizontal distance of 0.005 and of 0.3. Step 20 ticks and read `Motion`. | PHY-SKIP-006 |
| E43 | Drop items from a mob, a dispenser and a command in one tick, then save and load the world. Read the order of the items in each later rule that uses an order. | PHY-CMP-013, PHY-CMP-016 |
| E44 | Put a pig on a piston that pushes up and to the side, and on a slime block that a piston moves. Step 3 ticks and read `Pos` and `Motion`. | PHY-BLK-001, PHY-BLK-002 |
| E45 | Put one item in reach of a player and of a hopper in one tick. Read which one gets the item. | PHY-CMP-017 |
| E46 | Move two boats towards each other on ice, with the summons in both orders. Step 10 ticks and read `Pos`. | PHY-CMP-018, PHY-DEV-002 |
| E47 | Build a TNT cannon with 4 charges and 1 projectile TNT. Step each tick and read `Pos` and `Motion` of the projectile. Shoot a fireball into a crowd of pigs. | PHY-EXP-001, PHY-EXP-002, PHY-CMP-020 |
| E48 | With a vanilla client, ride a minecart and a boat. Record the positions that the client sends and the positions that the server sends. | PHY-SRV-011 |
| E49 | Put 40 pigs with speed 0 in a box of 1 × 1 blocks with `max_entity_cramming` at 0. Step 20 ticks and read `Pos`. | PHY-CMP-019, PHY-DEV-008 |
| E50 | With the scripted client, move the player into a pig in one move packet. Step 1 tick and read `Motion` of the pig. | PHY-CMP-021, PHY-DEV-012 |
| E51 | Drop 40 different items onto a player with a nearly full inventory in one tick. Read the slots after the tick. | PHY-CMP-017 |
| E52 | Read the velocity that the server expects for a player that stands in a crowd of pigs and on a slime block. | PHY-CMP-023 |
| E53 | From the server console, summon an item with `Motion` `[0.1, 0, 0.07]` and `Age` -32768 on stone, ice and a slime block. Read `Motion` and `Pos` after each of the first 400 ticks, then after each 1000 ticks up to 12,000 ticks. | PHY-ORD-005, PHY-SKIP-003, PHY-DEV-014 |
| E54 | From the server console, with the tick frozen, put dirt in the air and destroy it with `/setblock … air destroy`. Read `Pos` and `Motion` of the item, then remove it. Do this 1000 times. | PHY-CMP-025 |

The experiment of a supported rule also covers each `source_suggested` rule without a row above:

- E1 to E5 cover PHY-NUM;
- E2 and E6 cover PHY-FLG-004 and PHY-FLG-005;
- the experiments of `movement-checks.md` cover PHY-SRV;
- E4, E5 and E20 cover the invariants.

## 16. Captured results

The results come from a vanilla dedicated server of `26.4-snapshot-7`, with no client. Set
`pause-when-empty-seconds=-1`, or the server stops its ticks 60 seconds after the start.

**E53**, one item for each block:

| Block | Last change of `Pos` | First tick with each of `v.x` and `v.z` in `[-1.0E-6, 1.0E-6]` | `v.x` at tick 1008 | `v.x` from tick 6036 |
|-------|----------------------|----------------------------------------------|---------------------------|-----------------------------|
| `stone` | tick 27 | 67 | `5.37E-61` | `4.9E-324`, the same up to 12,000 ticks |
| `ice` | tick 318 | before tick 1008 | `2.42E-7` | `2.12E-29`, and smaller after each 1000 ticks |
| `slime_block` | tick 5 | 13 | `4.9E-323` | 0 from a tick between 1008 and 2013; `v.y` does not rest |

In the first 400 ticks on stone, `v.x` changes in each tick up to tick 7, then only in the ticks
11, 15, 19 and so on. This is the cycle of 4 ticks. On the slime block, the step action of the
block makes `v.x` smaller by a factor below 0.5, so the value gets to 0.

**E54**, 1000 drops of dirt from the block at `(10, -50, 30)`. The offset is the position minus
the block position.

| Value | Smallest | Largest | Mean | Distinct values |
|-------|----------|---------|------|-----------------|
| offset `x` | 0.2501 | 0.7500 | 0.5036 | 1000 |
| offset `y` | 0.1257 | 0.6245 | 0.3734 | 1000 |
| offset `z` | 0.2502 | 0.7490 | 0.4869 | 1000 |
| `v.x` | -0.0989 | 0.0996 | 0.0016 | 1000 |
| `v.y` | 0.2 | 0.2 | 0.2 | 1 |
| `v.z` | -0.0993 | 0.0998 | 0.0040 | 1000 |

The five bins of 0.1 of offset `x` hold 179, 204, 221, 197 and 199 drops. This agrees with a
uniform distribution on `[0.25, 0.75)`.
