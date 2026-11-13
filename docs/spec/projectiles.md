# Projectiles: arrows and snowballs

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| World version (`DataVersion`) | `5127` |
| Protocol version | `1073742169` |
| Data pack format | `126.0` |

This document states the behaviour of two projectile types: the arrow and the snowball. It states
the launch, the flight, the hit on an entity and the hit on a block. It states the pickup of an
arrow and the packets that a client sees. It does not prescribe an architecture.

The rules come from the reference implementation and from the data pack. No experiment has run
yet. Each rule has the label `source_suggested`. Section 12 lists the experiments that decide the
open points.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **tick**, **entity**, **living entity**, **viewer**: as `login.md`,
  `entity-physics.md` and `entity-tracking.md`, section 0.1, define them.
- **Collider**, **box**, **velocity**, **gravity**, **drag**, **f32**, **f64**, **widen**: as
  `entity-physics.md`, section 0.1, defines them.
- **Hit**, **full hit**, **amount**, **damage source**, **causing entity**, **direct entity**:
  as `entity-damage.md`, sections 0.1 and 2, define them.
- **Projectile**: an arrow or a snowball.
- **Arrow**: an entity of the type `minecraft:arrow` (wire id 6).
- **Snowball**: an entity of the type `minecraft:snowball` (wire id 125).
- **Shooter**: the entity that launches a projectile. In this document the shooter is a player.
- **Owner**: the entity that the projectile records as its shooter. A projectile that the server
  loads from a save restores its owner from the field `Owner`. The owner can be absent.
- **Owner group**: the owner, the root vehicle of the owner and all passengers of that root
  vehicle.
- **Left-owner flag**: a boolean of a projectile. It is false at the launch (PRJ-TST-004).
- **Draw time**: the number of ticks between the start of the bow use and its release.
- **Power**: an f32 from 0 to 1 that the draw time gives (PRJ-LCH-001).
- **Speed**: the length of the velocity of a projectile, in blocks per tick.
- **Inaccuracy**: the f32 factor of the random spread of the launch direction (PRJ-LCH-003).
- **Segment**: the line from the position of a projectile at the start of its move to the
  position that the move asks for.
- **Margin**: the f32 distance by which the hit test makes an entity box larger (PRJ-TST-002).
- **In-ground state**: the state of an arrow that is stuck in a block (section 6).
- **Shake counter**, **despawn counter**: two integer counters of an arrow (PRJ-BLK-003).
- **Base damage**: the f64 value of an arrow that scales its damage. Its default is 2.0.
- **Critical arrow**: an arrow with the critical flag (PRJ-LCH-002).
- **Pickup status**: one of `disallowed`, `allowed` and `creative_only` (section 7).
- **Ammo-free shot**: a launch that takes no arrow from the inventory of the shooter
  (PRJ-LCH-005).
- **Tracker update**: as `entity-tracking.md`, section 7, defines an update.

### 0.2 Provenance

The labels are those of `docs/spec/registry.md`, section 0.3. Each rule states its kind and its
confidence. The kinds are those of `docs/spec/README.md`. Flight rules that the client also
computes have the kind "client agreement", because the client simulates a projectile between two
tracker updates (PRJ-NET-005).

### 0.3 ID prefix

Each rule has an ID with the prefix `PRJ`. The second part names the section.

| Prefix | Subject |
|--------|---------|
| `PRJ-LCH` | launch: bow, throw, start velocity |
| `PRJ-FLT` | flight: gravity, drag, rotation, order of a tick |
| `PRJ-TST` | the hit test along a move |
| `PRJ-ENT` | the hit of an arrow on an entity |
| `PRJ-SNW` | the hit of a snowball |
| `PRJ-BLK` | the hit of an arrow on a block, shake, despawn |
| `PRJ-PCK` | pickup of an arrow |
| `PRJ-NET` | the packets that a client sees |
| `PRJ-SAV` | the saved form |

### 0.4 Scope

This document states the arrow and the snowball that a player launches, and the arrow and the
snowball as they exist in the world. Section 9 lists what is out of scope. These are out of scope:
tridents, eggs and fireballs, and every other projectile type.

### 0.5 Artefacts cited

- `assets/mcrs/reports/packets.json`: the packet ids of section 8. These are the play clientbound
  packets: `add_entity` 1, `damage_event` 25, `entity_event` 34, `entity_position_sync` 35,
  `game_event` 39, `move_entity_pos_rot` 55, `remove_entities` 78, `set_entity_data` 101,
  `set_entity_motion` 103, `sound` 119 and `take_item_entity` 127.
- `assets/mcrs/reports/registries.json`: the wire ids of the entity types.
- The data pack: the damage types `arrow` and `thrown`. Both have `exhaustion` 0.1 and `scaling`
  `when_caused_by_living_non_player`. Both are in the damage tag `is_projectile`. Neither is in
  `no_knockback`, `no_impact` or `bypasses_armor`.

## 1. Launch

**PRJ-LCH-001** (`source_suggested`, semantic, confidence: high). A bow shot depends on the draw
time `t`. Let `p = f32(t) / 20`. The power is `min(1, (p × p + p × 2) / 3)` in f32. The shot does
not occur when the power is less than 0.1. Thus a draw time of 0, 1 or 2 ticks gives no shot,
and a draw time of 3 ticks is the shortest shot. The table gives the speed `3 × power`:

| Draw time `t` | Power | Speed |
|---------------|-------|-------|
| 3 | 0.1075 | 0.3225 |
| 5 | 0.1875 | 0.5625 |
| 10 | 0.4167 | 1.25 |
| 15 | 0.6875 | 2.0625 |
| 19 | 0.9342 | 2.8025 |
| 20 or more | 1.0 | 3.0 |

A release with no shot does not take an arrow, does not damage the bow and plays no sound.

**PRJ-LCH-002** (`source_suggested`, semantic, confidence: high). A bow shot with power exactly 1.0
makes a critical arrow. A shot with less power makes an arrow that is not critical. A critical
arrow shows its flag in entity data (PRJ-NET-002) and adds damage (PRJ-ENT-002).

**PRJ-LCH-003** (`source_suggested`, semantic, confidence: medium). The launch gives the velocity
of a projectile in these steps. The speed `s` and the inaccuracy `i` are the inputs.

1. Compute the direction from the yaw and the pitch of the shooter, in degrees, in f32:
   `x = -sin(yaw) × cos(pitch)`, `y = -sin(pitch)` and `z = cos(yaw) × cos(pitch)`.
2. Normalise the direction.
3. Add a random value to each of the three components. Each value is `0.0172275 × i × (u1 − u2)`.
   `u1` and `u2` are independent uniform values in the range from 0 to less than 1.
   Do not normalise again.
4. Multiply the vector by `s`.
5. Set the rotation of the projectile from this vector (PRJ-FLT-006). The motion of the shooter
   of step 6 does not change the rotation.
6. Add the motion of the shooter (PRJ-LCH-004).

The bow uses `s = 3 × power` and `i = 1.0`. The snowball uses `s = 1.5` and `i = 1.0`. Both
shooters use a pitch offset of 0.

**PRJ-LCH-004** (`source_suggested`, semantic, confidence: medium). The projectile inherits part
of the motion of the shooter. The inherited vector has the `x` and `z` components of the motion
of the shooter. Its `y` component is 0 when the shooter is on the ground, and the `y` component
of the motion of the shooter otherwise. For a player, the motion of the shooter is the movement
that the last movement packet of the client gave. It is zero when the client sent no movement
packet in its last tick (`movement-checks.md`, MVC-TICK-002).

**PRJ-LCH-005** (`source_suggested`, semantic, confidence: medium). A bow shot needs an arrow
item. The shooter uses the first stack that holds an arrow item. The order is the hand that
does not hold the bow, then the inventory slots in slot order. A player in creative mode that has
no arrow still shoots, and the projectile is an `arrow` item stack of count 1. When the shooter
has no arrow and is not in creative mode, the use of the bow does not start.

The shot takes one arrow from the stack. The shot takes no arrow in two cases. The shooter has infinite
materials (creative mode), or an enchantment of the bow sets the ammo use to 0. Such a shot is
an ammo-free shot. The enchantments are out of scope (section 9). Each shot of a bow also takes 1
point of durability from the bow.

**PRJ-LCH-006** (`source_suggested`, semantic, confidence: high). An arrow starts at the position
`(x, eyeY - 0.1, z)` of the shooter, with `0.1` as an f32 value that is widened. A snowball starts
at the same position. The start position is inside the box of the shooter, so the owner
immunity of PRJ-TST-004 applies from the first tick.

**PRJ-LCH-007** (`source_suggested`, semantic, confidence: high). The use of a snowball item does
the following. In the same tick the server plays the throw sound and creates the snowball with the
speed 1.5 (PRJ-LCH-003). It also takes one item from the stack of the shooter. A shooter with
infinite materials keeps the stack count. The snowball has the item stack of count 1 that the
shooter threw. The throw sound is `entity.snowball.throw` in the category `neutral`, with volume
0.5 and pitch `0.4 / (r × 0.4 + 0.8)`. Here `r` is uniform in the range from 0 to less than 1. The server also counts the statistic "item used" of the snowball item.

**PRJ-LCH-008** (`source_suggested`, semantic, confidence: medium). The bow shot plays the sound
`entity.arrow.shoot` in the category `players` at the position of the shooter, with volume 1.0
and pitch `1 / (r × 0.4 + 1.2) + power × 0.5`.

## 2. Flight

Each tick, an arrow that is not in the in-ground state and a snowball do the steps of this
section. The constants of the table are exact. An f32 constant is widened to f64 before it
multiplies a velocity component.

| Quantity | Arrow | Snowball |
|----------|-------|----------|
| Gravity (f64, subtracted from `y`) | 0.05 | 0.03 |
| Air drag (f32, all axes) | 0.99 | 0.99 |
| Water drag (f32, all axes) | 0.6 | 0.8 |
| Box (width, height) | 0.5, 0.5 | 0.25, 0.25 |

**PRJ-FLT-001** (`source_suggested`, client agreement, confidence: medium). An arrow does these
steps in this order in each tick.

1. If the arrow is inside the collision shape of its block, go to section 6 (PRJ-BLK-001).
2. Let `v` be the velocity at the start of the tick.
3. If the arrow is in water, multiply the velocity by the water drag.
4. Set the rotation (PRJ-FLT-006).
5. Update the left-owner flag (PRJ-TST-004).
6. Find the hit along the segment from the position `P` to `P + v` (section 3). The segment uses
   `v`, not the velocity of step 3.
7. Move the arrow to the end of the segment, or to the nearest hit. Apply the hit (sections 4 and 6).
8. If the arrow is not in water, multiply the velocity by the air drag.
9. If the arrow is not in the in-ground state, subtract the gravity from the `y` component.

Thus an arrow moves with the velocity of the start of the tick. In air, a hit in this tick reads
that velocity. In water, a hit in this tick reads the velocity of step 3. The arrow is in water
when the water state that the previous tick left says so. A hit that moves an arrow into the
in-ground state sets its velocity to zero in step 7, and step 9 does not subtract gravity.

**PRJ-FLT-002** (`source_suggested`, client agreement, confidence: medium). A snowball does these
steps in this order in each tick.

1. Subtract the gravity from the `y` component of the velocity.
2. Multiply the velocity by the water drag when the snowball is in water, and by the air drag
   otherwise.
3. Find the hit along the segment from `P` to `P + v`, with the velocity of step 2 (section 3).
4. Move the snowball to the nearest hit, or to `P + v` when there is no hit.
5. Set the rotation (PRJ-FLT-006).
6. Update the left-owner flag (PRJ-TST-004).
7. If there is a hit, apply it (section 5).

The order of the gravity and the drag differs from that of the arrow. The snowball moves with the
velocity after the gravity and the drag of its own tick.

**PRJ-FLT-003** (`source_suggested`, semantic, confidence: medium). Water does not stop a
projectile. The segment test uses the collision shapes of blocks only. A fluid has no effect on
the segment. A bubble column adds `-0.03` to the `y` velocity of a projectile that is in it and
pulls down, and `+0.06` when it pushes up. Above the top of a bubble column the values are
`-0.03` and `+0.1`. An arrow in the in-ground state does not react to a bubble column, and a
fluid that flows does not push it.

**PRJ-FLT-004** (`source_suggested`, semantic, confidence: medium). Rain or water clears the fire
of an arrow in each tick.

**PRJ-FLT-005** (`source_suggested`, semantic, confidence: medium). The gravity is 0 for a
projectile with the entity flag "no gravity".

**PRJ-FLT-006** (`source_suggested`, client agreement, confidence: medium). The
rotation of a projectile follows its velocity. Let `v` be the velocity that the step of
PRJ-FLT-001 (arrow, step 4) or PRJ-FLT-002 (snowball, step 5) uses. The target yaw is
`atan2(vx, vz)` in degrees. The target pitch is `atan2(vy, sqrt(vx² + vz²))` in degrees. The new
angle is `old + 0.2 × (target - old)`, where the target first moves by a multiple of 360 degrees
to lie within 180 degrees of `old`. At the launch the rotation equals the target without the
factor 0.2 (PRJ-LCH-003, step 5).

## 3. The hit test

**PRJ-TST-001** (`source_suggested`, semantic, confidence: medium). The hit test of a move uses
the segment `P` to `P + v`. It finds the nearest block hit first. The block hit is the first
collider that the segment meets. The server ignores fluids. A block hit that is on the world
border face is a hit on the world border (section 11, question 4).

The test then finds the entity hits along the segment from `P` to the block hit, or to `P + v`
when there is no block hit. An entity that is behind the block hit does not count. A snowball
takes the nearest entity hit, which is the one with the smallest distance from `P` to the hit
point. An arrow takes each entity hit. It sorts the hits by the distance from `P` to the position of
the entity, which is not the hit point. A non-piercing arrow ends on the first entity that takes
the hit (PRJ-ENT-006). An entity hit replaces the block hit.

**PRJ-TST-002** (`source_suggested`, semantic, confidence: medium). The margin of a projectile is
`max(0, min(0.3, (n - 2) / 20))`, with `n` the age of the projectile in ticks. The age is 0 when
the projectile starts, and it increases by 1 at the start of each tick. Thus the margin is 0 for
ages up to 2, and it is 0.3 for ages of 8 and more. A projectile that is older than 2 ticks hits
an entity that it misses by less than the margin.

- A snowball hits an entity when the segment meets the box of the entity made larger by the
  margin on all sides. The hit point is the point where the segment enters that larger box.
- An arrow hits an entity in one of three ways. The first way: the segment meets the exact box.
  The hit point is where the segment enters the exact box. The second way: the segment meets only
  the larger box. The arrow clips a line from the entry point of the larger box to the centre of
  the entity, against the colliders. The arrow hits when that line reaches the exact box before
  a collider, and the hit point is the entry point of the larger box. The third way: the segment
  starts inside the exact box. The hit point is `P`.

**PRJ-TST-003** (`source_suggested`, semantic, confidence: medium). A projectile can hit an
entity when all these conditions are true:

1. The entity is alive and the player can aim at it. A spectator, an item entity, an experience
   orb and another arrow are not such entities. A projectile cannot hit an entity that is
   removed.
2. The entity is not in the owner group, or the left-owner flag is true.
3. For an arrow: the entity is not a player when the owner is a player that cannot harm that
   player. The owner cannot harm the player when the server has `pvp` off, or when the team rules
   forbid it.
4. For a piercing arrow: the entity is not in the pierce list (section 9 states that piercing is
   out of scope).

**PRJ-TST-004** (`source_suggested`, semantic, confidence: medium). The left-owner flag of a
projectile starts false. While it is false, a projectile cannot hit an entity of the owner group
(PRJ-TST-003). The server updates the flag when it is false, at most one time in a tick. The flag becomes true when the box of the projectile meets no aimable entity of the owner group. The box is made larger by the velocity and by 1 block on each side. The flag also becomes true when the owner is
absent. A true flag stays true. An arrow updates the flag before its hit test of the tick. A
snowball updates the flag after its move in the tick. Thus the owner is safe in the first tick of
an arrow, and in the first two ticks of a snowball. An arrow that falls on its owner later hits
the owner.

## 4. The arrow hits an entity

**PRJ-ENT-001** (`source_suggested`, semantic, confidence: high). The damage source of an arrow
hit has the damage type `arrow`. Its direct entity is the arrow. Its causing entity is the owner
when the owner exists, and the arrow itself otherwise. The source has no source position. Thus
the credit of a kill, the anger of a mob and the cause chain of `damage_event` name the owner.
When the owner is absent, a client sees the arrow as the cause (PRJ-NET-007).

**PRJ-ENT-002** (`source_suggested`, semantic, confidence: high). The amount of an arrow hit comes
from the speed `s` of the arrow, as f32, at the hit (PRJ-FLT-001), and the base damage `b`:

1. `n = ceil(clamp(s × b, 0, 2147483647))`, as an integer. `s` is widened.
2. If the arrow is critical, add a random integer. The integer is uniform from 0 to
   `n / 2 + 1` with both limits allowed, with integer division. The sum has the upper limit 2147483647.
3. The amount is `n` as f32.

For a fresh arrow of base damage 2.0, the amount is 6 at the speed 3.0 and 10 at most for a
critical arrow. Modifiers of the base damage that come from the bow are out of scope.

**PRJ-ENT-003** (`source_suggested`, semantic, confidence: medium). The server does these steps
in this order for the first entity that the arrow hits:

1. Record the owner for the kill credit of the entity (`entity-damage.md`, DMG-DTH-001 step 1), when the owner is a living entity.
2. Save the fire ticks that remain on the entity.
3. If the arrow is on fire, set the fire ticks of the entity to the larger of its own value and
   100. The ignition also clears the freeze of the entity.
4. Do a hit on the entity with the damage source of PRJ-ENT-001 and the amount of PRJ-ENT-002.
   The procedure is that of `entity-damage.md`, section 3.
5. If the hit is accepted, do the steps of PRJ-ENT-004.
6. If the hit is rejected, do the steps of PRJ-ENT-005.

**PRJ-ENT-004** (`source_suggested`, semantic, confidence: medium). After an accepted hit, the
arrow does these steps:

1. If the target is a living entity, add 1 to its arrow count, unless the arrow pierces.
2. Apply the knockback of PRJ-ENT-007 when the hit is a full hit.
3. Send `game_event` with event 6 and value 0 to the owner (PRJ-NET-006). Do this when the target
   is a player, the owner is a different player and the arrow is not silent.
4. Play the arrow hit sound at the arrow position (PRJ-NET-008).
5. Remove the arrow, unless the arrow pierces.

The arrow count is a value of the living entity that the client shows as arrows in the body
(PRJ-NET-004). It decreases by 1 after `20 × (30 − count)` ticks, where `count` is the value at
the start of the wait. The wait starts again when the count is more than 0 and no wait is in
progress.

**PRJ-ENT-005** (`source_suggested`, semantic, confidence: medium). After a rejected hit, the
fire ticks of the target return to the value of PRJ-ENT-003, step 2. The arrow does not damage
the target. Its velocity is multiplied by `-0.1` on each axis. Its yaw increases by a random
value between 170 and 190 degrees. The owner does not change. The arrow stays in flight. Then the squared speed can be less than `1.0E-7`. If the pickup status
is `allowed`, the server creates an item entity with the arrow item at the arrow position
(`item-entity.md`). The arrow is removed. If the status is another one, the server removes
the arrow only. An arrow bounces in this way from a creative player, from a mob in its damage
cooldown, and from an entity that is invulnerable. Entities with their own rules (an enderman, a
shield that blocks) are out of scope (section 9).

**PRJ-ENT-006** (`source_suggested`, semantic, confidence: medium). An accepted hit of a
non-piercing arrow removes the arrow in the same tick, and no other entity takes that arrow. A
rejected hit does not remove it (question 3 of section 11).

**PRJ-ENT-007** (`source_suggested`, semantic, confidence: medium). A full hit of an arrow on a
living entity gives knockback. `entity-damage.md`, DMG-HIT-005 step 2, calls for this knockback
because the damage types `arrow` and `thrown` do not have the tag `no_knockback`. A partial hit
gives no knockback. A hit that is rejected gives none. Let `v` be the velocity of the projectile
at the hit. Let `r` be the value of the attribute `knockback_resistance` of the target. The
knockback does these steps:

1. `power = 0.4 × (1 - r)`. If `power ≤ 0`, stop.
2. Let `(dx, dz) = (−v.x, −v.z)`. While `dx² + dz² < 1.0E-5F`, set both to random values of the
   form `(u1 − u2) × 0.01`. `u1` and `u2` are uniform in the range from 0 to less than
   1.
3. Let `d = normalise((dx, 0, dz)) × power`.
4. The new `x` velocity is `old.x / 2 − d.x`. The new `z` velocity is `old.z / 2 − d.z`.
5. The new `y` velocity is `min(0.4, old.y / 2 + power)` when the target is on the ground, and
   `old.y` otherwise.

The entity data mark of DMG-NET-004 applies, so the target and its viewers receive
`set_entity_motion` at the end of the next tracker tick. Thus a player who is hit by an arrow
moves in the direction of the arrow.

## 5. The snowball hits

**PRJ-SNW-001** (`source_suggested`, semantic, confidence: high). A snowball that hits an entity
does a hit with the damage type `thrown`. Its direct entity is the snowball. Its causing entity is
the owner, or none when the owner is absent. The source has no source position. The amount is 3
when the entity is a `minecraft:blaze`, and 0 for each other entity. The hit uses the procedure of
`entity-damage.md`, section 3. The damage type `thrown` has the scaling
`when_caused_by_living_non_player`.

**PRJ-SNW-002** (`source_suggested`, semantic, confidence: medium). A hit with amount 0 has these
results for the target type:

- A living entity that is not a player: the hit is a full hit when the damage cooldown allows it
  (DMG-HIT-003). The target gets the damage event, the hurt sound, the knockback of PRJ-ENT-007
  and the red hurt flash. Its health does not change. When the damage cooldown rejects the hit,
  nothing happens.
- A player: the hit is rejected. The player gets no damage event, no knockback and no sound.
- An entity that is not a living entity: the rule of that entity decides. It is out of scope
  (section 9).

**PRJ-SNW-003** (`source_suggested`, semantic, confidence: high). A snowball ends on each hit,
also when the entity takes no damage and also when the hit is rejected. The server then does
these steps in this order:

1. Send `entity_event` with event 3 for the snowball to its viewers (PRJ-NET-009). The client
   shows 8 snowball item particles at the snowball position.
2. Remove the snowball.

A snowball that hits a block does steps 1 and 2 only. The block can have its own reaction to a
projectile hit. Such reactions are out of scope (section 9).

**PRJ-SNW-004** (`source_suggested`, semantic, confidence: medium). A snowball plays no hit sound.

## 6. The arrow hits a block, stays and despawns

**PRJ-BLK-001** (`source_suggested`, semantic, confidence: high). An arrow that the segment of
PRJ-TST-001 stops at a block, and that no entity hit precedes, does these steps:

1. Move the arrow to the hit point.
2. Subtract from the position the vector `0.05 × (sign(vx), sign(vy), sign(vz))`. The velocity of
   the hit gives the signs. The value 0.05 is an f32 that is widened.
3. Set the velocity to zero.
4. Play the arrow hit sound (PRJ-NET-008).
5. Set the in-ground state.
6. Set the shake counter to 7.
7. Clear the critical flag and the pierce level.
8. Set the hit sound to `entity.arrow.hit`.
9. Remember the block state at the hit position.

An arrow that is inside the collision shape of its own block at the start of a tick also gets the
steps 3 and 5. This occurs when a block appears around the arrow. A block can also react to an
arrow hit. Such reactions are out of scope.

**PRJ-BLK-002** (`source_suggested`, semantic, confidence: high). An arrow in the in-ground state
does not move, does not use gravity and does not use drag. It leaves the in-ground state when
both these conditions are true in a tick:

- the state of the block at the remembered position differs from the remembered state;
- no collider meets the box around the arrow position, made larger by 0.06 on each side.

The arrow then does three things. It multiplies each velocity component by a separate random f32 in the range from 0 to less than 0.2. It clears the in-ground state. It resets the despawn counter. The velocity
is zero, so the arrow falls from rest. An arrow also leaves the in-ground state and resets the
despawn counter when another source moves it, for example a piston.

**PRJ-BLK-003** (`source_suggested`, semantic, confidence: high). The shake counter is 0 for an
arrow that flies. The hit of PRJ-BLK-001 sets it to 7. It decreases by 1 at the start of each tick
until it is 0. The shake is an animation of the client that starts when the entity data
`in_ground` of the arrow becomes true (PRJ-NET-005). The shake counter also limits the pickup
(PRJ-PCK-001).

The despawn counter increases by 1 in each tick in which the arrow is in the in-ground state. The
server removes the arrow when the counter reaches 1200. The counter is 0 at each launch, and it is
reset in the cases of PRJ-BLK-002. Thus an arrow in a block lives 1200 ticks, or 60 seconds.

**PRJ-BLK-004** (`source_suggested`, semantic, confidence: medium). A save keeps the position,
the in-ground state and the counters of an arrow (PRJ-SAV-001). A loaded arrow continues with
these values.

## 7. Pickup

**PRJ-PCK-001** (`source_suggested`, semantic, confidence: high). The pickup status of an arrow is
one of three values. The status decides what a player gets when the arrow touches the player.

| Status | Saved byte | Result of a touch |
|--------|------------|-------------------|
| `disallowed` | 0 | no pickup |
| `allowed` | 1 | the arrow item goes into the inventory of the player |
| `creative_only` | 2 | the arrow goes away. The player gets no item. Only a player with infinite materials does this |

An arrow is picked up when all these conditions are true:

1. The arrow is in the in-ground state.
2. The shake counter is 0 or less.
3. The player is alive and not a spectator, and the box of the arrow meets the pickup box of the
   player (`item-entity.md`, IEN-PCK-001).
4. The status allows it, as the table states.

For the status `allowed`, the server inserts one arrow item with the rule of `item-entity.md`,
IEN-PCK-003. When there is no room, the pickup does not occur and the arrow stays. The arrow item
is the stack that the shooter fired, with its components. There is no owner check and no pickup
delay. Thus any player can pick up the arrow. An arrow is not available for pickup in the first 7
ticks after it stops.

**PRJ-PCK-002** (`source_suggested`, semantic, confidence: high). The launch sets the pickup
status in this way:

1. A shot with the item that the shooter loses sets `allowed`, when the owner is a player.
2. An ammo-free shot sets `creative_only`.
3. A shot of an owner that is not a player sets `disallowed`. A load from a save restores the
   saved value (PRJ-SAV-001).

Thus a normal arrow of a player in survival mode can be picked up by a player. A creative arrow
and an arrow that an enchantment made free give no item. Only a player in creative mode can
remove such an arrow by touch.

**PRJ-PCK-003** (`source_suggested`, wire, confidence: medium). A pickup that succeeds sends these
packets, in this order. The packets use `take_item_entity` and the rules of `item-entity.md`,
IEN-PCK-004:

1. `take_item_entity` with the network id of the arrow, the network id of the player and the
   count 1, to each viewer of the arrow.
2. `container_set_slot` with the changed slot to the player. This step does not occur for the
   status `creative_only`.
3. `remove_entities` with the network id of the arrow (PRJ-NET-003).

## 8. Packets that a client sees

**PRJ-NET-001** (`source_suggested`, wire, confidence: high). The spawn bundle of a projectile is
that of `entity-tracking.md`, TRK-SPN-001, with these values in `add_entity`:

| Field | Value |
|-------|-------|
| type | 6 for an arrow, 125 for a snowball |
| data | the network id of the owner, or 0 when the projectile has no owner |
| velocity | the velocity at the start of the tracker, as the last sent velocity |
| pitch, yaw | the packed rotation of the launch (PRJ-LCH-003) |

The data field is the only difference from the spawn of a mob. The client uses it to set the owner
of the projectile.

**PRJ-NET-002** (`source_suggested`, wire, confidence: medium). The entity data of the types, for
the index numbers that follow the data of the base entity (the indices 0 to 7):

| Type | Index | Type | Meaning | Default |
|------|-------|------|---------|---------|
| arrow | 8 | byte | bit 0: critical arrow. Bit 1: no-physics flag, not used in this document | 0 |
| arrow | 9 | byte | pierce level | 0 |
| arrow | 10 | boolean | in-ground state | false |
| arrow | 11 | VarInt | the colour of the effect. `-1` means no effect | -1 |
| snowball | 8 | item stack | the displayed item, always `minecraft:snowball` count 1 | that stack |

An entry whose value equals the default is not in the spawn bundle. A critical arrow therefore
has the entry 8 with value 1 in its bundle, and an ammo-free shot sets no entry.

**PRJ-NET-003** (`source_suggested`, wire, confidence: high). The removal of a projectile sends
`remove_entities` to its viewers (TRK-REM-001). A snowball sends `entity_event` with event 3
before it (PRJ-SNW-003). An arrow that hits an entity or reaches 1200 ticks sends only
`remove_entities`. A pickup sends `take_item_entity` first (PRJ-PCK-003).

**PRJ-NET-004** (`source_suggested`, wire, confidence: medium). The arrow count of PRJ-ENT-004 is
the entry of the living entity data that shows the arrows that stick in the body. The server sends
it in `set_entity_data` after each change.

**PRJ-NET-005** (`source_suggested`, wire, confidence: medium). The tracker of a projectile has
these values:

| Type | Tracking range (chunks) | Update interval (ticks) | Sends the velocity |
|------|-------------------------|-------------------------|--------------------|
| arrow | 4 | 20 | yes |
| snowball | 4 | 10 | yes |

A tracker update follows `entity-tracking.md`, section 7, with two additions:

- An arrow update always sends a position and a rotation, and so it sends `move_entity_pos_rot`.
  The position rules of TRK-UPD-003 choose `entity_position_sync` in the cases that the section
  states. A snowball update follows TRK-UPD-003 with no change.
- An update occurs in the tick in which the server changes the velocity of an arrow. A launch,
  a block hit or a bounce causes the change (PRJ-ENT-005). It also occurs when the data entry
  `in_ground` changes. The update of a block hit sends three packets in this order. The first is `set_entity_motion` with
  the zero velocity (TRK-UPD-007). The second is `move_entity_pos_rot`. The third is
  `set_entity_data` with the entry 10 (index of the arrow, `true`).

The vanilla client moves a projectile by itself between two updates, with the gravity and the drag
of section 2. It uses the packets to correct the state. Thus a server must use the constants of
section 2, or the client shows a visible jump at each update.

**PRJ-NET-006** (`source_suggested`, wire, confidence: medium). A hit that an arrow gives to a
player that the owner player shot sends `game_event` (play clientbound 39) to the owner. The
packet has the event id 6 and the value 0. The vanilla client plays the high-pitch arrow hit sound for this event.
The server sends no such packet when the target is the owner, or not a player, or when the arrow
is silent.

**PRJ-NET-007** (`source_suggested`, wire, confidence: high). A full hit on an entity sends
`damage_event` (DMG-NET-001) to the viewers of the target, and to the target when it is a player.
The causing entity of the packet is the network id of the causing entity of PRJ-ENT-001 plus 1.
The direct entity is the network id of the projectile plus 1. The damage type is the network id
of `arrow` or of `thrown` in the registry of the data pack. The source position is absent.

**PRJ-NET-008** (`source_suggested`, semantic, confidence: medium). The server plays these sounds
with `sound` packets, at the position of the projectile:

| Cause | Sound event | Category | Volume | Pitch |
|-------|-------------|----------|--------|-------|
| arrow hits an entity or a block | the hit sound of the arrow, `entity.arrow.hit` by default | `neutral` | 1.0 | `1.2 / (r × 0.2 + 0.9)` |

Here `r` is uniform in the range from 0 to less than 1. A snowball makes no hit sound (PRJ-SNW-004). The shot
and throw sounds are in PRJ-LCH-007 and PRJ-LCH-008.

**PRJ-NET-009** (`source_suggested`, wire, confidence: high). `entity_event` (play clientbound 34)
has the entity id as a 32-bit integer and the event as a byte. The event 3 on a snowball makes the
client show 8 particles of the item of the snowball at its position, with no velocity.

## 9. Out of scope

| Subject | Reason |
|---------|--------|
| the trident, the egg and the fireballs, and each other projectile type | other types with their own rules |
| the spectral arrow, the tipped arrow and the effects of an arrow | an arrow with an effect adds a potion rule |
| the crossbow, the dispenser, and the launch by a mob (skeleton, snow golem) | other shooters with their own inputs |
| the enchantments of the bow: Power, Punch, Flame, Infinity, Piercing, Multishot, Quick Charge | the enchantment system. The arrow base damage, the knockback power, the fire and the pierce list read their results |
| a pierce level above 0 and the pierce list | needs Piercing |
| the arrow that is on fire (the Flame effect and a burning arrow) | needs fire rules. PRJ-ENT-003 uses the flag |
| a shield that blocks, an enderman that dodges, an entity that deflects a projectile | entity-specific rules |
| a hit on an entity that is not living (a boat, an item frame, a painting, an armor stand) | the rule of that entity decides the result of a hit with amount 0 or more |
| a block reaction to a projectile hit (a target block, a bell, a candle, a TNT block that burns) | block rules |
| the advancement triggers and the statistics of a hit | advancement rules |
| the mob that wants to kill a player by an arrow, and the anger of a mob that an arrow hurts | `mob-goals.md` |
| the effect `levitation`, the portals and the end of the world below the lowest block | other documents |
| the sounds that the client plays from its own code, for example the hurt sound of `damage_event` | not sent by the server |

## 10. The saved form

**PRJ-SAV-001** (`source_suggested`, semantic, confidence: medium). A save keeps a projectile with
these fields, in addition to the fields of the base entity:

| Field | Type | Projectile | Default | Meaning |
|-------|------|------------|---------|---------|
| `Owner` | UUID | both | no value | the owner |
| `LeftOwner` | byte | both | 0 | the left-owner flag. A true value is written only when set |
| `HasBeenShot` | byte | both | 0 | the game event of the launch occurred |
| `life` | short | arrow | 0 | the despawn counter |
| `shake` | byte | arrow | 0 | the shake counter |
| `inGround` | byte | arrow | 0 | the in-ground state |
| `inBlockState` | block state | arrow | no value | the remembered block state |
| `pickup` | byte | arrow | 0 | the pickup status of PRJ-PCK-001 |
| `damage` | double | arrow | 2.0 | the base damage |
| `crit` | byte | arrow | 0 | the critical flag |
| `PierceLevel` | byte | arrow | 0 | the pierce level |
| `SoundEvent` | resource name | arrow | `minecraft:entity.arrow.hit` | the hit sound |
| `item` | item stack | arrow | one `minecraft:arrow` | the arrow item for a pickup |
| `weapon` | item stack | arrow | no value | the weapon that fired the arrow |
| `Item` | item stack | snowball | one `minecraft:snowball` | the displayed item |

An unknown value of `pickup` is read as 0. The save also keeps the fields `Motion`, `Pos` and
`Rotation` of the base entity.

## 11. Unresolved

1. **The tick of the first move.** The question is whether a projectile that a player launches in
   tick `T` also moves in tick `T`. Experiment E6 decides. The rules of section 2 describe the
   moves from the first tick on.
2. **The arrow search box in water.** The search area of the entity candidates for an arrow uses a
   velocity that the water drag has reduced. An entity at the far end of the segment can be
   outside the search area. A player that hits the far end of a full move sees no difference in
   most cases. The question for the owner: must the area contain the whole segment? This document
   states the segment as the test (PRJ-TST-001).
3. **A bounce followed by a second entity.** A rejected hit does not end the loop of an arrow in
   its tick. The loop can hit the next entity with the new velocity. The question is whether that
   second hit is intended. Experiment E4 shows it.
4. **The world border.** An arrow that meets the world border face bounces back with the velocity
   factor `−0.1` and a random yaw turn. A snowball is removed. This document states neither of the
   two. Experiment E10 decides the result for each type.
5. **The left-owner timing.** The arrow updates the left-owner flag before its hit test, and the
   snowball after its move. This gives different safe ticks for the owner (PRJ-TST-004). The
   question for the owner: is one timing the correct behaviour?
6. **A burning arrow and a fire-immune entity.** PRJ-ENT-003 sets the fire ticks of every
   entity, also of a fire-immune one. The result for the client (a flame on the entity) is not
   known. Experiment E9 decides.
7. **The pickable non-living types.** PRJ-TST-003 uses "the player can aim at it". The list of
   non-living types that an arrow hits is not known here.
8. **The arrow item that Infinity gives.** The enchantment can make a tipped arrow not free. The
   rule is in the data of the enchantment, so PRJ-LCH-005 cannot decide it.
9. **The ammo search order.** The order of PRJ-LCH-005 (the other hand, then the inventory) is
   not checked in the reference. Its confidence is low. Experiment E7 decides it.
10. **The draw time boundary.** The draw time is the number of server ticks between the start of the
    use and the release packet. A boundary at the tick of the packet can differ by one tick.
    Experiment E1 decides.

## 12. Experiments (proposed, not run)

An experiment needs `tools/vanilla-server` for the script and
`tools/mcrs_minecraft_client_recorder` for the packets. A capture of a game version other than
`26.4-snapshot-7` does not confirm a rule.

- **E1, the draw time.** For each draw time from 1 to 25 ticks, shoot one arrow at a flat wall.
  Record `add_entity`, the first `set_entity_motion`, and the time of the first arrow. The result
  decides PRJ-LCH-001 and question 10.
- **E2, the spawn bundle.** Record the bundles of a bow arrow, an ammo-free arrow, a critical
  arrow and a snowball. Decide PRJ-NET-001 and PRJ-NET-002, and the data indices.
- **E3, the trajectory.** Shoot an arrow and a snowball at 0 degrees of pitch over 40 ticks, in
  air and across a block of water. Record the position of each tick from the client. Compare
  with PRJ-FLT-001 and PRJ-FLT-002.
- **E4, the damage.** Shoot arrows of draw times 3, 10 and 20 at a zombie that has 20 health. Use the
  distances 2, 10 and 30. Record the health change and the number of critical bonus values. Add a
  line of two zombies in a row for question 3.
- **E5, stuck arrows.** Shoot an arrow into a block. Record the in-ground entity data. Record the pickup
  at the ticks 1 to 8 after the hit and the removal at tick 1200. Remove the block and record
  the fall.
- **E6, the first tick.** Record the position of an arrow and of a snowball in the ticks of the
  launch. Decide question 1.
- **E7, the pickup status.** Shoot as a survival player, as a creative player and with the
  Infinity enchantment. Hold arrows in the other hand and in two slots, and record which stack the shot uses. Walk into each arrow and record the inventory and `take_item_entity`.
  Decide PRJ-PCK-001 and PRJ-PCK-002.
- **E8, the owner immunity.** Shoot straight up, and at the feet of the shooter. Record the tick
  at which the arrow and the snowball can hit the shooter. Decide PRJ-TST-004 and question 5.
- **E9, the bounce.** Shoot a creative player, a mob in its damage cooldown and an entity
  that is invulnerable. Record the velocity and the removal. Decide PRJ-ENT-005 and question 6.
- **E10, the world border.** Shoot an arrow and a snowball at the border. Decide question 4.
- **E11, the snowball hits.** Throw snowballs at a blaze, a pig, a player and an item frame.
  Record `damage_event`, `entity_event`, health and the knockback. Decide PRJ-SNW-002.

## 13. Intentional deviations

None. The owner has not decided to keep a defect of the reference for this domain.

## 14. Relation to other documents

- `entity-damage.md`: the procedure of a hit, the cooldown, the reductions and the packets of
  damage. This document supplies the damage source and the amount.
- `entity-physics.md`: the rules of numbers and of gravity. A projectile has its own flight rules,
  because the order of the gravity and the drag differs from the rule of a living entity.
- `entity-tracking.md`: the spawn bundle, the updates and the removal.
- `item-entity.md`: the pickup box, the insert into the inventory and the packets of a pickup.
