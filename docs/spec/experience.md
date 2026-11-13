# Experience: player points, experience orbs and experience drops

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| World version (`DataVersion`) | `5127` |
| Protocol version | `1073742169` |

This document states the experience of a player: the stored values, the level formula and the
operations that add points. It states the experience orb: its value, its spawn, its motion, its
attraction to a player, its merge, its lifetime and its pickup. It states the experience that a mob
and a player drop on death. It states the packets that a client sees and the data that a save keeps.

Mending is out of scope. A pickup always gives the whole value of the orb to the points of the
player (XP-PCK-004).

No experiment ran for this document. Each rule comes from the reference implementation and from
public documentation. Section 15 lists the proposed experiments.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **tick**, **entity**, **living entity**, **f32**, **widen**, **data pack**,
  **corpus**: as `entity-physics.md`, section 0.1, defines them.
- **Hit**, **dying state**, **death**: as `entity-damage.md`, section 0.1, defines them.
- **Points**, **level**, **progress**, **total**: the four values of the experience of a player
  (XP-STA-001).
- **Points for a level**: the number of points that a player must gain at a level to reach the next
  level (XP-LVL-001).
- **Orb**: an entity of the type `experience_orb`.
- **Value**: the integer that an orb gives to the points of a player in one pickup.
- **Count**: the number of pickups that an orb still gives. A merge adds to the count.
- **Value tier**: one of the values that the split of XP-VAL-001 produces.
- **Merge group**: the integer from 0 to 39 that an orb has. An orb merges only with an orb of the
  same merge group (XP-MRG-001).
- **Take delay**: a counter of a player that limits the orb pickups (XP-PCK-002).
- **Reward**: the number of points that a death drops, before the split of XP-VAL-001.
- **Followed player**: the player that an orb moves towards (XP-ATT-001).
- **Needs-sync mark**: as TRK-UPD-001 defines it.

### 0.2 Provenance

The labels are those of `docs/spec/registry.md`, section 0.3, and `entity-damage.md`, section 0.2.
Each rule states its kind and its confidence, for example `(source_suggested, semantic,
confidence: medium)`. The kinds are those of `docs/spec/README.md`. A rule with the label
`documentation` comes from the public documentation of the game and agrees with the reference.

### 0.3 ID prefix

Each rule has an ID with the prefix `XP`. The second part names the section.

| Prefix | Subject |
|--------|---------|
| `XP-STA` | the stored values of a player |
| `XP-LVL` | the points for a level |
| `XP-ADD` | the operations that change the values |
| `XP-VAL` | the value of an orb and the split of an amount |
| `XP-SPN` | the spawn of an orb |
| `XP-ORB` | the tick of an orb, its lifetime and its damage |
| `XP-MRG` | the merge of orbs |
| `XP-ATT` | the attraction to a player |
| `XP-PCK` | the pickup by a player |
| `XP-DRP` | the drop on death |
| `XP-NET` | the packets that a client sees |
| `XP-SAV` | the data that a save keeps |
| `XP-DEV` | intentional deviations |

### 0.4 Scope

This document states the rules above. Section 13 lists the subjects that are out of scope.

### 0.5 Artefacts cited

- `assets/mcrs/reports/packets.json`: the packet ids `add_entity` (1), `remove_entities` (78),
  `set_experience` (105) and `take_item_entity` (127).
- `assets/mcrs/reports/registries.json`: the entity type `experience_orb` (id 50), and the game
  rules `keep_inventory` and `mob_drops`.
- `entity-physics.md`: PHY-ORD-005 states two things. It states the gravity, the drag and the block
  friction of the orb row. Its item push states the push out of a solid block.
- `entity-damage.md`: DMG-DTH-001 for the place of the drop in a death.
- `item-entity.md`: IEN-NET-004 for the removal packets of a pickup.
- `entity-tracking.md`: the tracking range, the update interval and the spawn bundle.

## 1. Stored values of a player

**XP-STA-001** (`source_suggested`, semantic, confidence: high). A player has these values of
experience:

| Value | Type | Range | Start |
|-------|------|-------|-------|
| level | i32 | 0 and more | 0 |
| progress | f32 | 0.0 to less than 1.0 | 0.0 |
| total | i32 | 0 and more | 0 |
| score | i32 | any | 0 |

The level is the whole level that the client shows. The progress is the part of the next level
that the player has gained. The client shows the progress as the experience bar. The total and the
score are counters. The server does not compute them from the level and the progress.

**XP-STA-002** (`source_suggested`, semantic, confidence: medium). The total is a counter, not a
function of the level. These operations change the level or the progress and keep the total.
They are the spend of levels for an enchantment, the command that sets the level, and the command that sets

the points of the level. Only an add of points changes the total by the amount that it adds. A
level that falls below 0 sets the total to 0 (XP-ADD-002). The total is the source of the
scoreboard criterion `xp`. The client does not use the total to show the level.

## 2. Points for a level

**XP-LVL-001** (`documentation`, semantic, confidence: high). The points for a level `L` are:

| Level `L` | Points for the level |
|-----------|----------------------|
| 0 to 14 | `2 × L + 7` |
| 15 to 29 | `5 × L − 38` |
| 30 and more | `9 × L − 158` |

The values are 7 at level 0, 37 at level 15 and 112 at level 30. The progress is the fraction of
these points that the player has gained. A client and a server must use the same table, because the
progress is on the wire.

**XP-LVL-002** (`documentation`, semantic, confidence: high). The sum of the points for the levels
0 to `L − 1` is the least number of points that reach the level `L`. It starts from the level 0:

| Level `L` | Sum |
|-----------|-----|
| 0 to 16 | `L² + 6 × L` |
| 17 to 31 | `2.5 × L² − 40.5 × L + 360` |
| 32 and more | `4.5 × L² − 162.5 × L + 2220` |

This sum is a consequence of XP-LVL-001. The server does not store it and does not compare the
total with it (XP-STA-002).

## 3. Operations that change the values

**XP-ADD-001** (`source_suggested`, wire, confidence: medium). The operation "add `n` points" takes
any i32 `n`, also a negative one. Let `need(L)` be the points for the level `L`. Each f32 operation
below rounds to f32. The server does these steps in this order:

1. Add `n` to the score.
2. Set the progress to the progress plus `f32(n) / need(level)`.
3. Set the total to the total plus `n`, limited to the range from 0 to 2147483647. The sum has no
   overflow before the limit.
4. While the progress is less than 0:
   1. Let `r` be the progress times `need(level)`.
   2. Add −1 levels (XP-ADD-002).
   3. If the level before the step was more than 0, set the progress to `1 + r / need(level)`, with
      the level after the step. Otherwise set the progress to 0.
5. While the progress is 1.0 or more:
   1. Set the progress to `(progress − 1) × need(level)`.
   2. Add +1 levels (XP-ADD-002).
   3. Set the progress to the progress divided by `need(level)`, with the level after the step.
6. Mark the experience of the player as changed (XP-NET-002).

An add of 7 points at the level 0 gives the level 1, the progress 0.0 and the total 7. An add of 0
points changes no value and still marks the experience as changed.

**XP-ADD-002** (`source_suggested`, semantic, confidence: medium). The operation "add `n` levels"
takes any i32 `n`:

1. Set the level to the level plus `n`. The sum saturates at the limits of i32.
2. If the level is less than 0, set the level, the progress and the total to 0.
3. Play the sound `entity.player.levelup` and record the tick (XP-ADD-004) when all these
   conditions are true:
   - `n` is more than 0;
   - the new level is a multiple of 5;
   - the last level-up sound is more than 100 ticks old.
4. If `n` is not 0, mark the experience as changed (XP-NET-002).

**XP-ADD-003** (`source_suggested`, semantic, confidence: medium). The operation "set the points of
the level to `a`" fails and changes nothing when `a` is not less than `need(level)`. Otherwise it
sets the progress to `a / need(level)`, limited to the range from 0 to `(need − 1) / need`. It
keeps the level and the total. It marks the experience as changed when the progress changed. The
operation "set the level to `a`" sets the level to `a`. It keeps the progress and the total. It
marks the experience as changed when the level changed. The query of the points of the level gives the
progress times `need(level)`, rounded down. A command spec states the command syntax and the
messages.

**XP-ADD-004** (`source_suggested`, semantic, confidence: medium). The level-up sound has the
sound source `player`. It plays at the position of the player, for each player that hears it. Its
volume is 0.75 times 1.0 when the level is more than 30, and 0.75 times `level / 30` otherwise. Its
pitch is 1.0. The record of the last sound starts at tick 0 of the player entity. Thus a level-up
in the first 100 ticks of a player entity plays no sound. Section 14.2 lists this as an open point.

**XP-ADD-005** (`source_suggested`, semantic, confidence: high). The sources of points that this
document states are the pickup of an orb (XP-PCK-003) and the operations of XP-ADD-001 to
XP-ADD-003. Each other source spawns an orb or calls the operation "add points". Section 13 lists
these sources.

## 4. Value of an orb and split of an amount

**XP-VAL-001** (`source_suggested`, semantic, confidence: high). The split of an amount `a` of
points into orbs repeats this step while `a` is more than 0. Take the greatest value tier that is
not more than `a`. Remove that value from `a`. Make one orb of that value. The value tiers are:

`2477`, `1237`, `617`, `307`, `149`, `73`, `37`, `17`, `7`, `3`, `1`

The value 2 does not exist as a tier. An amount of 5 gives the orbs 3, 1 and 1. An amount of 10
gives the orbs 7 and 3. The sum of the values of the orbs is the amount.

**XP-VAL-002** (`source_suggested`, semantic, confidence: high). The value of an orb is a whole
number. The server stores it as the entity data entry in XP-NET-003. A spawn that does not use the
split gives the orb any value that the spawner states, for example a summon with a stated `Value`.
The client chooses the texture of an orb from its value. No server rule depends on the texture.

## 5. Spawn of an orb

**XP-SPN-001** (`source_suggested`, semantic, confidence: medium). A spawn of an amount at a
position does these steps for each part of the split (XP-VAL-001). It does them in the order of
the split:

1. Choose an integer `g` uniformly from 0 to 39.
2. Look for an existing orb with all these properties:
   - it is not removed;
   - its merge group is `g`;
   - its value is the value of the part;
   - its box intersects the cube with the edge 1.0 around the position.
3. If such an orb exists, add 1 to its count and set its age to 0. The part makes no new orb. When
   more than one orb qualifies, the choice is unspecified.
4. Otherwise make a new orb with the value of the part (XP-SPN-002).

A player can see the difference: an orb that gets a new part lives 6000 more ticks, and it gives
more than one pickup.

**XP-SPN-002** (`source_suggested`, semantic, confidence: medium). The server makes a new orb with
a position `p`, a direction `d` and a value. For a death the direction is the zero vector. These
steps give the start state:

1. Set the yaw to a uniform random value from 0 to 360 degrees. Set the pitch to 0.
2. Make the velocity `(x, y, z)`. Draw `x` and `z` uniformly from −0.2 to 0.2, and `y` uniformly
   from 0 to 0.4.
3. If `d` is not the zero vector and the dot product of `d` and the velocity is less than 0, negate
   the velocity.
4. Set the position to `p` plus the unit vector of `d` times 0.25. For the zero vector the offset
   is zero.
5. If the box of the orb at the new position overlaps a solid block, a free position can exist near the
   centre of the box. If so, the server moves the orb to it. Section 14.2 lists the exact
   search as an open point.
6. Set the health to 5, the age to 0 and the count to 1. Set the merge group to the network id of
   the orb modulo 40.

The box of the orb is 0.5 wide and 0.5 high. The tracking range is 6, and the update interval is 20
ticks (`entity-tracking.md`).

**XP-SPN-003** (`source_suggested`, semantic, confidence: medium). A thrown experience bottle gives
a direction. The direction is the normal of the block that it hits. When the bottle hits no block,
the direction is the negated velocity of the bottle. The other spawners that this document names use the zero direction. A spawner that
does not use the split makes one orb with XP-SPN-002 and does not merge it into an existing orb.
Breeding, villager trades, a hook of a fishing rod, a fox and a turtle use this path.

## 6. Orb motion, lifetime and damage

**XP-ORB-001** (`source_suggested`, client agreement, confidence: medium). The client has local
authority for the motion of an orb, as it has for an item entity. Server and client must do the
same steps in each tick, so that the client does not jump when a position update arrives. The
parameters are the row "experience orb" of PHY-ORD-005: gravity 0.03, horizontal and vertical drag
0.98F, ground friction as `f32(0.98F × f)`, and no step height. The steps are:

1. Do the base tick. It adds 1 to the tick count of the orb.
2. Find whether the box of the orb overlaps a solid block at its position. Call this "stuck".
3. If the eye of the orb is in water, do the water step (XP-ORB-002). Otherwise, if the orb is not
   stuck, subtract the gravity 0.03 from `v.y`.
4. If the block at the position of the orb has lava, do the lava step (XP-ORB-003).
5. On the server, if the tick count modulo 20 is 1, do the merge scan (XP-MRG-002).
6. Do the attraction step (XP-ATT-002).
7. On the server, the orb can have no followed player, be stuck, and have its box moved by `v`
   also stuck. Then push the orb out of the solid block with the push of PHY-ORD-005, row "item".
   Set the needs-sync mark.
8. Remember `v.y`. Move the orb by `v`, and apply the block effects.
9. Multiply `v` by the drag factor. The drag factor is 0.98F. When the orb is on the ground, it is
   `f32(0.98F × f)`. `f` is the friction of the block that is 0.999999 below the position of the
   orb.
10. If the orb touched a floor in the move and the remembered `v.y` is less than −0.03, set
    `v.y` to the remembered `v.y` times −0.4.
11. Add 1 to the age. When the age is 6000 or more, remove the orb.

The eye of the orb is 0.425 above the bottom of its box. The orb row of PHY-ORD-005 must use the
bounce of step 10, not the bounce of the item row.

**XP-ORB-002** (`source_suggested`, client agreement, confidence: medium). The water step multiplies
`v.x` and `v.z` by 0.99F. It sets `v.y` to the lesser of `v.y + 5.0E-4F` and 0.06F. The orb in water
has no gravity.

**XP-ORB-003** (`source_suggested`, client agreement, confidence: medium). The lava step sets the
velocity to `(a, 0.2F, b)`. `a` and `b` are each the difference of two uniform random values from 0
to 1, times 0.2F. The step sets the needs-sync mark. The client cannot know the random values, so
the server correction in the same tick decides the visible motion. The source of the random values
is unspecified.

**XP-ORB-004** (`source_suggested`, semantic, confidence: high). An orb that has the age 6000 is
removed at the end of its tick. Thus an orb lives 6000 ticks of its own ticks, 5 minutes at 20
ticks each second. A merge keeps the smaller age (XP-MRG-002). A new part that joins the orb sets
its age to 0 (XP-SPN-001). The age grows only in a tick of the orb. An orb in an unloaded chunk does
not age.

**XP-ORB-005** (`source_suggested`, semantic, confidence: medium). An orb has the health 5. A hit
that the orb is not invulnerable to sets the health to the whole part of `health − amount`. The orb
is removed when the health is 0 or less. The orb has no damage cooldown and no armor. A player
cannot attack an orb. The damage of a fire, of lava, of an explosion and of a cactus uses this
rule. Section 14.2 lists the invulnerabilities of the orb as an open point.

## 7. Merge of orbs

**XP-MRG-001** (`source_suggested`, semantic, confidence: medium). Two orbs can merge only when
both are not removed, their values are equal, and their merge groups are equal. The merge group
spreads the orbs into 40 groups, so that a crowd of orbs of one value does not collapse at once.
A test must not compare which orbs merge, because network ids are not equal in two games.

**XP-MRG-002** (`source_suggested`, semantic, confidence: medium). In a tick of the orb in which the
tick count modulo 20 is 1, the server does the merge scan. This is the first tick of the orb and
then each 20th tick. The scan selects each orb that can merge with this orb (XP-MRG-001). The selected orb is not
this orb. Its box intersects the box of this orb enlarged by 0.5 in each direction. For each
selected orb, the server does these steps:

1. Add the count of the selected orb to the count of this orb.
2. Set the age of this orb to the lesser of the two ages.
3. Remove the selected orb.

The value of this orb does not change. The position of this orb does not change.

## 8. Attraction to a player

**XP-ATT-001** (`source_suggested`, client agreement, confidence: medium). Each tick, before the
attraction step, the orb keeps its followed player. This requires three conditions. The player is not removed. The
player is not a spectator. The player is at most 8 blocks from the orb. The distance is the distance between the position
of the orb and the position of the player. Otherwise the orb selects a new followed player:

1. Find the nearest player with a distance of less than 8 blocks from the orb, who is not a
   spectator. A player in creative mode counts.
2. If that player is dying or dead, the orb has no followed player.
3. Otherwise that player becomes the followed player.

**XP-ATT-002** (`source_suggested`, client agreement, confidence: medium). When the orb has a
followed player, the step adds this change to `v`:

1. Let `d` be the vector from the position of the orb to the point `(px, py + e / 2, pz)`. `p` is
   the position of the followed player, and `e` is its current eye height.
2. Let `s` be `1 − |d| / 8`.
3. Add `unit(d) × s² × 0.1` to `v`. For a zero `d`, add nothing.

The change is at most 0.1 for each tick, and it is 0 at the distance 8 on the horizontal axis. The
orb keeps gravity, drag and collision during the attraction (XP-ORB-001). The server marks no
needs-sync for the attraction, because the client runs the same step on the players that it knows.

## 9. Pickup by a player

**XP-PCK-001** (`source_suggested`, semantic, confidence: high). In its tick, a player that is not a
spectator and has health more than 0 looks for orbs in its pickup box. The pickup box is the box of
the player enlarged by 1.0 on `x` and `z` and by 0.5 on `y`, in each direction. For a passenger,
the pickup box is the union of two boxes. These are the box of the player and the box of the
vehicle, enlarged by 1.0 on `x` and `z`. A box intersects an orb when the intersection has a volume. When the box holds
more than one orb, the server chooses one orb uniformly at random. It handles only that orb in this
tick. Other entities in the box follow their own pickup rules, for example `item-entity.md`.

**XP-PCK-002** (`source_suggested`, semantic, confidence: high). A player has a take delay that
starts at 0. At the start of each tick of the player, the server decreases a take delay that is more
than 0 by 1. The server handles the chosen orb of XP-PCK-001 only when the take delay is 0. Thus a
player takes at most one orb pickup in each 2 ticks. A pickup in the tick `T` sets the take delay
to 2. The tick `T + 1` decreases it to 1 and takes no pickup. The tick `T + 2` can take a pickup.
The delay is for the player and not for the orb.

**XP-PCK-003** (`source_suggested`, semantic, confidence: high). When the server handles an orb for
a player (XP-PCK-002), it does these steps:

1. Set the take delay to 2.
2. Send `take_item_entity` to each viewer of the orb (XP-NET-004). The packet has the network id
   of the orb, the network id of the player and the count 1.
3. Add the value of the orb to the points of the player (XP-ADD-001).
4. Decrease the count of the orb by 1.
5. When the count is 0, remove the orb. `remove_entities` follows (XP-NET-004).

An orb with the count 3 and the value 7 gives 7 points three times, in three pickups that are at
least 2 ticks apart. It stays in the world between them.

**XP-PCK-004** (`design`, semantic, confidence: medium). In this document the pickup gives the whole
value to the points. Mending is out of scope. A later specification can state the repair of an item
from the value of an orb before the add. That specification must give the value that remains to
XP-ADD-001.

## 10. Experience of a death

**XP-DRP-001** (`source_suggested`, semantic, confidence: high). At the death step "drop the loot and
the experience" (DMG-DTH-001 step 4), the server spawns the reward at the position of the dying
entity. The spawn uses the split of XP-VAL-001 and XP-SPN-001. The position is the position of the feet. The
spawn uses the zero direction. A reward of 0 spawns no orb. The reward is computed once, at the death.

**XP-DRP-002** (`source_suggested`, semantic, confidence: high). A mob drops experience when all
these are true:

- no other system consumed the experience of the death;
- a player hit the mob in the last 100 ticks (`entity-damage.md`, DMG-DTH-001 step 4);
- the game rule `mob_drops` is true;
- the type of the mob drops experience. An adult always does. A baby does not, except a baby of
  a monster type and a baby hoglin, which do. A tadpole never does.

When one of these is false, the mob drops no experience. A mob that no player hit drops no
experience.

**XP-DRP-003** (`source_suggested`, semantic, confidence: medium). The reward of a mob is a datum
of its type, with these cases:

- The base value of a mob type is 0 by default. A monster type has the base value 5 by default.
  The table below lists the types with another base value.
- Animals and water animals roll a uniform integer from 1 to 3. A chicken jockey gives 10 in place
  of that roll. An adult sulfur cube rolls a uniform integer from 1 to 2, and a baby gives 0.
- A slime and a magma cube give their size as base value.
- A baby zombie gives its base value times 2.5, rounded down.
- A piglin and a hoglin give their base value and no equipment bonus. A baby hoglin has the base
  value 3 and an adult hoglin has the base value 5.
- A mob with a base value of more than 0 that is not in the lines above adds the equipment
  bonus. The bonus has one part for each equipment slot except the saddle slot. A slot gives a
  part when it holds an item and its drop chance is at most 1.0. The part is a uniform integer
  from 1 to 3.

| Type | Base value |
|------|-----------:|
| `wither` | 50 |
| `ravager`, `piglin_brute` | 20 |
| `blaze`, `guardian`, `evoker`, `breeze` | 10 |
| `zombie`, `ghast`, `phantom`, `shulker`, `warden`, `zoglin`, `illusioner`, `piglin` | 5 |
| `vex`, `endermite` | 3 |
| `creaking` | 0 |

A generic monster type that this table does not list has the base value 5. The ender dragon is out
of scope. A mob of another type with the base value 0 drops no experience.

**XP-DRP-004** (`source_suggested`, semantic, confidence: medium). The experience of the causing entity can
change the reward. This occurs when the causing entity is a living entity and its equipment holds an
enchantment with the effect `minecraft:mob_experience`. The corpus has no such enchantment, so this document states
no formula. The hook is the reward of XP-DRP-003 as the input, and the changed reward as the output.

**XP-DRP-005** (`source_suggested`, semantic, confidence: high). A player that dies drops the reward
`min(level × 7, 100)`. The reward uses the level only, and not the progress or the total. The
following apply:

- The reward is 0 when the game rule `keep_inventory` is true.
- The reward is 0 when the player is a spectator.
- The game rule `mob_drops` and the time since a player hit the dying player do not apply.
- A baby rule does not apply.

A death of a player drops the reward one time. A level of 50 gives 100 points, and a level of 10
gives 70 points.

**XP-DRP-006** (`source_suggested`, semantic, confidence: high). The experience of a respawned
player depends on how the new player entity starts:

- When the player returns from the end, or changes a dimension while alive, the level, the progress,
  the total and the score stay the same.
- When the player dies, the level, the progress, the total and the score of the dead player carry
  to the new player. This occurs when the game rule `keep_inventory` is true or the dead player
  was a spectator. The player that died with `keep_inventory` true dropped no experience (XP-DRP-005).
- Otherwise the new player has the level 0, the progress 0.0, the total 0 and the score 0.

The enchantment seed (XP-SAV-001) carries to the new player in each case.

## 11. Packets that a client sees

**XP-NET-001** (`source_suggested`, wire, confidence: high). The packet `set_experience` (play
clientbound 105) has these fields in this order:

| Order | Field | Type |
|-------|-------|------|
| 1 | progress | f32 |
| 2 | level | VarInt |
| 3 | total | VarInt |

The order of the fields on the wire is progress, level, total. The client shows the experience bar
from the progress and the level number from the level.

**XP-NET-002** (`source_suggested`, semantic, confidence: medium). The server sends one
`set_experience` to the player in the tick of that player. It sends the packet when the
experience is marked as changed. It also sends it when the total differs from the total of the
last packet. At most one packet leaves in each
tick. The server marks the experience as changed:

- at the start of a connection: the first tick after the join sends a packet;
- on a respawn and on a dimension change;
- after each add of points (XP-ADD-001), also an add of 0;
- after an add of levels that is not 0;
- after a change of the level by a set;
- after a change of the progress by a set;
- after the spend of levels for an enchantment.

A player that has the same values sees the same packet. A pickup in the same tick as another add
sends one packet with the final values.

**XP-NET-003** (`source_suggested`, wire, confidence: medium). The spawn bundle of an orb
(TRK-SPN-001) has `add_entity` with the type id 50 and the velocity of the orb, then
`set_entity_data` with one entry. The entry has the index 8, the serializer 1 (VarInt) and the
value of the orb. The server sends the entry again when the value changes. The count, the age and
the merge group are not on the wire.

**XP-NET-004** (`source_suggested`, wire, confidence: medium). An orb that a player took sends
`take_item_entity` (127) for each pickup. The packet has the network id of the orb, the network id
of the player and the count 1. When the count of the orb falls to 0, `remove_entities` (78) follows in the same
tick. Each other removal of an orb sends `remove_entities` only: the end of the lifetime, the
damage, and the merge into another orb. The vanilla client plays the sound
`entity.experience_orb.pickup` when it gets `take_item_entity` for an orb. A merge that adds to the
count sends no packet, because the count is not on the wire.

## 12. Persistence

**XP-SAV-001** (`source_suggested`, semantic, confidence: high). The data of a player holds these
fields, with these types and start values:

| Field | Type | Start value when absent |
|-------|------|-------------------------|
| `XpP` | f32 | 0.0 |
| `XpLevel` | i32 | 0 |
| `XpTotal` | i32 | 0 |
| `XpSeed` | i32 | a random i32, also when the value is 0 |
| `Score` | i32 | 0 |

`XpSeed` is the seed of the enchantment offers. This document does not state its use. The server
loads the values as they are, and it does not clamp them. The take delay, the record of the last
level-up sound and the last packet are not saved.

**XP-SAV-002** (`source_suggested`, semantic, confidence: high). The data of an orb holds these
fields:

| Field | Type | Start value when absent |
|-------|------|-------------------------|
| `Health` | i16 | 5 |
| `Age` | i16 | 0 |
| `Value` | i16 | 0 |
| `Count` | i32 | 1 |

A `Count` that is 0 or less is read as 1. The merge group is not saved, because it follows from the
network id (XP-SPN-002). An orb with the value 0 gives 0 points in a pickup.

## 13. What this document does not state

| Subject | Reason |
|---------|--------|
| Mending, and the repair of an item from the value of an orb | Out of scope (XP-PCK-004). |
| The experience bottle, the furnace, the grindstone, the breeding orb, villager and trader orbs, fishing, the fox, the turtle, the ender dragon and the sculk catalyst | Each is a source that calls the spawn or the add. The spec of its domain states the amount. |
| The cost of an enchantment, and the enchantment seed in use | The enchanting spec states them. |
| The scoreboard criteria `xp` and `level`, and the death screen score | The scoreboard spec states them. |
| The command syntax and the messages of the experience command | A command spec states them (XP-ADD-003). |
| The texture of an orb, and the client sound and particle details | The client renders them from the value. |
| The experience of the death of a boss, and the experience that a player can take by other items | Out of scope. |
| The order of the death of a player and the screen of death | `entity-damage.md` and the respawn spec. |

## 14. Intentional deviations, open points and unresolved items

### 14.1 Intentional deviations

No entry. The reference behaviours that this document does not copy are in the unresolved items.

### 14.2 Unresolved

- **U1**: The level-up sound is silent in the first 100 ticks of a player entity (XP-ADD-004). The
  owner decides whether to keep this.
- **U2**: The total is not a function of the level (XP-STA-002). The owner decides whether a set of
  the level must change the total.
- **U3**: The exact search that moves an orb out of a solid block at the spawn (XP-SPN-002 step 5).
  The experiment E7 decides it.
- **U4**: The invulnerabilities of an orb, and the time that an orb survives in lava or fire
  (XP-ORB-005). The experiment E6 decides it.
- **U5**: The values of `need(level)` for a level above about 238 million. The reference uses 32-bit
  arithmetic there. This document states no result.
- **U6**: The reference has more than one call that drops the experience of a dying player. One
  experiment (E4) must show that a player drops the reward one time.
- **U7**: The eye of the orb is 0.425 above its box bottom (XP-ORB-001). This is the default eye
  height of an entity that has no own value. The experiment E8 confirms it.
- **U8**: The reference lets the orb keep a followed player that is dying, until that player is
  removed (XP-ATT-001). The effect is small, because the orbs of that death keep no such player.
  The owner decides whether a dying player must stop the attraction.

### 14.3 Reference behaviour that this document does not copy

- A baby zombie that has its reward computed two times gives 2.5 times 2.5 times the base value
  from the second call. The reward is computed once for a death, so no rule is needed.
- The sculk catalyst computes the reward before the drop, and the random parts can differ from the
  drop. This is out of scope.

## 15. Experiments

No experiment ran. Each experiment needs a vanilla server, and the first six need a player that the
input agent of `tools/vanilla-client` plays.

| ID | Steps | Rules |
|----|-------|-------|
| E1 | Set the level 0. Run `experience add @s 7 points`. Record `set_experience`. Repeat with 100, 1000 and -3 points. Record the progress as f32 bytes, the level and the total. | XP-ADD-001, XP-NET-001, XP-LVL-001 |
| E2 | Run `experience set @s 5 levels`, then `experience add @s 3 levels`, then `experience add @s -9 levels`. Record the packets and `XpTotal`. | XP-ADD-002, XP-STA-002 |
| E3 | Summon one zombie with `NoAI` next to a player. Kill it with `damage` as that player. Count the orbs, and record the values. Repeat 50 times for the distribution of the reward. Repeat with a baby zombie, a zombie that wears a helmet, a chicken and a slime of size 4. | XP-DRP-002, XP-DRP-003, XP-VAL-001 |
| E4 | Set the level 10, then kill the player with `keep_inventory` false. Record the orbs. Repeat with level 50, `keep_inventory` true, and spectator mode. | XP-DRP-005, XP-DRP-006, U6 |
| E5 | Summon an orb with `Value:7s` and `Count:3` at the feet of a player. Record the ticks of the three `take_item_entity` packets and the `set_experience` packets. | XP-PCK-001 to XP-PCK-003 |
| E6 | Summon an orb in lava, in a cactus, in fire and near a TNT blast. Record the tick of `remove_entities`. | XP-ORB-005, U4 |
| E7 | Summon an orb with the box inside a block and with a velocity of 0. Record its positions in the first 10 ticks. | XP-SPN-002, XP-ORB-001 |
| E8 | Drop an orb into 2 blocks of water. Record its positions in 40 ticks. Compare with the model of XP-ORB-001 and XP-ORB-002 for the eye height 0.425. | XP-ORB-001, U7 |
| E9 | Place 100 orbs of the value 1 in one block, one after another, with an amount of 1 each time and with an amount of 100 in one spawn. Wait 60 ticks and count the orbs. | XP-SPN-001, XP-MRG-001, XP-MRG-002 |
| E10 | Summon an orb at 7.9 blocks from a player and record its velocity in 10 ticks. Repeat at 8.1 blocks. | XP-ATT-001, XP-ATT-002 |
| E11 | Save a player with `XpP:0.5f`, `XpLevel:3`, `XpTotal:20`, `XpSeed:5` and `Score:9`. Load it, and record the packet and the data after a save. | XP-SAV-001 |
