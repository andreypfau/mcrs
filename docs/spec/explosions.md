# Explosions and Primed TNT: behavioural specification

| Field | Value |
|---|---|
| Game version | `26.4-snapshot-7` |
| Protocol version | `1073742169` |

This document states the behaviour of primed TNT, the sources that light it and its fuse. It states
the blocks that an explosion destroys and the entities that it damages and pushes. It also states
the cause that the explosion passes on.
It states the explosion of primed TNT completely. Other exploders use the same procedure with their
own power, damage source and options (section 6).

## 0. Conventions

### 0.1 Terms

- **Hit**, **Damage source**, **Causing entity**, **Direct entity**: as `entity-damage.md` defines them.
- **Explosion**: one run of the procedure of sections 3 to 6. It has a centre, a power, a source
  entity or none, a damage source, and a block interaction mode.
- **Power**: the f32 size of an explosion. Primed TNT has power 4.0.
- **Impact radius**: `2P` blocks, for the power `P`.
- **Source entity**: the entity that explodes. It is the direct entity of the damage source.
- **Owner**: the living entity that is the causing entity of a primed TNT, or none.
- **Block interaction mode**: `destroy`, `destroy_with_decay` or `keep`. Primed TNT uses
  `destroy_with_decay` when the game rule `tnt_explosion_drop_decay` is true, and `destroy` when it
  is false.
- **Resistance**: the explosion resistance of a block or of a fluid, from the block data.
- **Seen fraction**: the part of an entity that the centre can see (EXP-DMG-001).
- **Prime**: replace a TNT block, or a dispensed TNT item, with a primed TNT entity.

### 0.2 Rule IDs, provenance and game rules

Rule IDs have the prefix `EXP`. Provenance labels are those of `registry.md`, section 0.3. The
kinds are those of `docs/spec/README.md`.

These game rules apply:

| Game rule | Default | Effect |
|---|---|---|
| `tnt_explodes` | true | When false, no TNT is primed and a primed TNT does not explode (EXP-TNT-001, EXP-TNT-002). |
| `tnt_explosion_drop_decay` | false | When true, each drop of a destroyed block can be lost (EXP-BLK-003). |
| `mob_griefing` | true | Changes which projectiles can prime TNT (EXP-TNT-002). It does not change the blocks that TNT destroys. |

## 1. Primed TNT

### EXP-TNT-001: Spawn, fuse and explosion

A primed TNT spawns at the block position `(x + 0.5, y, z + 0.5)` when it comes from a block. It
has a fuse of 80 ticks, unless a rule of this document says otherwise. Its first velocity is
`(-sin(a) * 0.02, 0.2, -cos(a) * 0.02)`, where `a` is a uniform f64 from 0 to 2π. The motion of the
entity is the motion of primed TNT in `entity-physics.md`.

In each tick the entity moves, and then it decreases the fuse by 1. When the fuse is 0 or less, the
entity is removed in that tick. If `tnt_explodes` is true, it then creates an explosion with the
power `explosion_power` (default 4.0). The centre is `(x, y + 0.0625, z)` of the entity. The
explosion has no fire (EXP-FIRE-001). If `tnt_explodes` is false, the entity is removed and nothing
else happens.

A primed TNT takes no damage. Each hit on it is rejected. It is pushed by explosions (EXP-KNB-001).

The save data of the entity has four fields:

- the short `fuse`;
- the compound `block_state` (default `minecraft:tnt`);
- the float `explosion_power`, which the server writes only when it is not 4.0 and clamps to 0.0
  through 128.0 when it reads it;
- the UUID `owner`.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

### EXP-TNT-002: The sources that prime TNT

A TNT block becomes a primed TNT when one of these sources acts. In each case, the block is
replaced with air and the entity spawns as EXP-TNT-001 states. The game event `prime_fuse` occurs
at the block, and the sound `entity.tnt.primed` plays at the entity with volume 1.0 and pitch 1.0.
Each source states its owner.

| Source | Owner | Notes |
|---|---|---|
| A player uses `flint_and_steel` or `fire_charge` on the block | the player | A creative player keeps the item. Else the flint and steel loses 1 durability, or the fire charge loses 1 item. The sound of the item plays. |
| A projectile that is on fire hits the block | the owner of the projectile, if it is a living entity | See the conditions below. |
| A redstone signal: the block is placed while powered, or a neighbour update finds it powered | none | |
| A dispenser with `flint_and_steel` faces the block | none | The item loses 1 durability. |
| Fire burns the block out (`block-simulation.md`, SIM-FIRE-006) | none | The fuse is 80. |
| A player that is not in creative mode breaks a TNT block with `unstable` true | none | The block drops no item. |
| An explosion destroys the block | the causing entity of the explosion | EXP-TNT-003. |

A dispenser also primes a TNT item that it dispenses, with no owner, in front of itself. The item
loses 1 count.

Conditions:

- A player in adventure mode primes the block only when the item can break that block.
- A projectile primes the block only when the projectile is on fire, and its owner can interact
  with the position. For a player owner, the interaction rules of the player apply. For a mob owner,
  the game rule `mob_griefing` must be true. A projectile with no owner can interact. Arrows that
  are on fire, `fireball` and `small_fireball` prime TNT. `wither_skull`, `dragon_fireball` and
  the wind charges do not.
- If `tnt_explodes` is false, no source primes the block. A player that uses the item on the
  block sees the overlay message `block.minecraft.tnt.disabled`. The item is not used.

Lava and burning neighbours do not prime the block in a direct way. Only SIM-FIRE-006 and the
sources above do.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

### EXP-TNT-003: The chain fuse

When an explosion destroys a TNT block, the block is replaced with air. It drops no item. If
`tnt_explodes` is true, a primed TNT spawns at `(x + 0.5, y, z + 0.5)`. Its owner is the causing
entity of the explosion. Its fuse is `10 + r`, where `r` is a uniform integer from 0 through 19.
Thus the chain fuse is 10 through 29 ticks.

A primed TNT that an explosion reaches does not change its fuse. It takes no damage, and it is
pushed (EXP-KNB-001).

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment E2 counts the fuses.

## 2. Exposure and damage

### EXP-DMG-001: Entities in reach

An explosion of power `P` acts on each entity in a box. The box extends `2P + 1` blocks from the
centre on each axis, rounded to whole blocks. The source entity is not in the set. An entity is skipped
when one of these is true:

- the entity is an invisible armor stand;
- the entity is a hanging entity, for example a painting or an item frame, and the source entity is
  in water;
- the entity is a vehicle, the explosion has a mob as its causing entity, and the game rule
  `mob_griefing` is false;
- the entity is a warden that digs or emerges.

Items, hanging entities, vehicles and visible armor stands are in reach of a TNT explosion. A wind
charge explosion skips them.

Let `d` be the distance from the position of the entity to the centre. The position is the foot
position, not the eye. An entity with `d > 2P` is not in reach. For the others, let
`q = (1 - d / 2P) * s`, where `s` is the seen fraction.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Question U2 holds the types that
this list does not cover.

### EXP-DMG-002: Seen fraction

The seen fraction `s` of an entity is the part of sample points on its box that the centre can see.
For an axis with box size `w`, let `n = floor(2w + 1) + 1`. The samples on that axis are
`n` points at the fractions `0, 1/(2w+1), 2/(2w+1), ...` of the box size. The samples on x and z are
shifted by `(1 - floor(2w + 1) / (2w + 1)) / 2` blocks. The samples on y start at the bottom of the
box. A
sample is seen when the straight segment from the sample to the centre meets no block collision
shape. Fluids do not stop the segment. The result is the count of seen samples divided by `n_x * n_y * n_z`.

This reads the blocks before the explosion destroys any block.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment E3.

### EXP-DMG-003: The damage amount

For each entity in reach, the explosion makes one hit, with the damage source of EXP-SRC-002 and
the amount:

```text
amount = ((q * q + q) / 2) * 7 * 2P + 1     (f32)
```

For `P = 4`, the amount is `((q * q + q) / 2) * 56 + 1`. An entity that is in reach, with `s = 0`,
takes the amount 1.0. The amount is not rounded.

The hit goes through the procedure of `entity-damage.md`, section 3. The damage types `explosion`
and `player_explosion` have the tag `no_knockback`. Thus the hit makes no knockback of its own.
These types have the scaling `always`. For a player, the amount changes with the difficulty before the hit. These are the changes:

- on peaceful, the amount is 0, and the hit is rejected;
- on easy, the amount is `min(amount / 2 + 1, amount)`;
- on normal, the amount does not change;
- on hard, the amount is `amount * 3 / 2`.

The damage occurs before the blocks are destroyed (EXP-BLK-002).

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment E3.

## 3. Blocks

### EXP-BLK-001: The rays

An explosion makes 1352 rays from its centre. One ray goes through each cell on the surface of a
cube of 16 by 16 by 16 cells. For the cell `(i, j, k)`, with each index from 0 through 15, the cell
is on the surface when one index is 0 or 15. The direction is the unit vector of
`(2i/15 - 1, 2j/15 - 1, 2k/15 - 1)`.

Each ray does this:

1. The strength is `P * (0.7 + 0.6 * u)`, where `u` is a uniform f32 in [0, 1).
2. The position is the centre.
3. While the strength is more than 0:
   - Take the block position that contains the position. If it is outside the world, stop the ray.
   - If the block is not air, or the fluid is not empty, let `r` be the larger of the block resistance
     and the fluid resistance. Subtract `(r + 0.3) * 0.3` from the strength.
   - If the strength is still more than 0, add the block position to the set of the explosion.
   - Add `0.3 * direction` to the position. Subtract 0.225 from the strength.

Water and lava have resistance 100, so a ray stops in a fluid. Thus a centre in water destroys no
block. A block whose resistance takes all the strength is not in the set. No fixed limit of
resistance exists.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. See also PHY-EXP-003.

### EXP-BLK-002: The order of the parts

An explosion does these parts in this order:

1. the event `explode`;
2. the rays;
3. the damage and the push of the entities;
4. the destruction of the blocks;
5. the fire. No ray sees a block that the
same explosion destroys. The entities see the blocks before the destruction.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

### EXP-BLK-003: The destruction

When the block interaction mode is `keep`, no block changes. Otherwise the explosion handles the
positions of the set in a random order. For each position with a block that is not air:

1. If the block drops items from an explosion, it computes its loot with an empty tool. The loot
   knows the direct source entity. In the mode `destroy_with_decay`, the loot also knows the power,
   and each stack survives the condition `survives_explosion` with the chance `1 / P`. The block
   `tnt` drops nothing from an explosion.
2. When the causing entity is a player, a block that gives experience when broken gives it.
3. The block becomes air, with a block update to the neighbours.
4. The block runs its was-exploded behaviour. For `tnt`, this is EXP-TNT-003.

After all positions, the drops are merged: a stack joins a collected stack of the same item and
components, up to 16 items. Each collected stack spawns as one item entity at the first position
that gave it.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment E4.

### EXP-FIRE-001: Fire after an explosion

An explosion places fire only when its creator asks for fire. Primed TNT does not ask for fire.
Thus a TNT explosion places no fire, and the game rule `mob_griefing` has no effect on it. For an
explosion that asks for fire, each position of the set can get fire. A position gets fire with the
chance 1/3 when the position is air and the block below has a full solid shape. The fire block is the fire state for
that position.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

## 4. Knockback

### EXP-KNB-001: Explosion knockback

Each entity in reach gets the velocity change `v = n * k`, with `k = (1 - d / 2P) * s * (1 - e)`.
Here `n` is the unit vector from the centre to the entity. The value `e` is the attribute
`explosion_knockback_resistance` of the entity. It is from 0.0 to 1.0, with the default 0.0. It is
0.0 for an entity that is not a living entity. The point of the entity is the foot position for a primed TNT and the eye position for each
other entity. The entity adds `v` to its velocity. The change has no cap and no split into a
horizontal and a vertical part. An entity that is in reach and has `s = 0` gets `v = 0`.

A rejected hit does not stop the push. An invulnerable entity is pushed.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment E3.

### EXP-KNB-002: The packet of an explosion

The server sends the packet `explode` to each player whose squared distance to the centre is less
than 4096. The packet carries the centre, the power, the count of destroyed positions and the velocity change
of that player. It also carries the explosion particle, the sound `entity.generic.explode` and the block
particles. The velocity change is present only when the player was in reach, is not a spectator,
and is not a flying creative player. The client adds it to its own velocity. The source entity does not play the sound when it is silent.
The explosion particle is the small one when `P < 2` or the mode is `keep`. Else it is the large
one.

A projectile of the kind that explosions deflect, for example a `fireball`, gets the causing entity
of the explosion as its owner.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. The layout of the fields is in the
packet report. Experiment E5.

## 5. Cause chain

### EXP-SRC-001: Attribution

A kill by an explosion gives the credit and the experience to the causing entity of the
explosion. The rules of `entity-damage.md` (DMG-HIT-005) state the credit. The causing entity is:

- the source entity, when the source entity is a living entity;
- the owner, when the source entity is a primed TNT;
- the owner of the projectile, when the source entity is a projectile with a living owner;
- none, in each other case.

A primed TNT whose owner entity is not loaded has no causing entity.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

### EXP-SRC-002: The damage source of an explosion

The damage source has the direct entity `source entity` and the causing entity of EXP-SRC-001. The
damage type is `player_explosion` when both are present. Else it is `explosion`. A player does not
need to be the cause: any causing entity gives `player_explosion`. The death message keys are
`death.attack.explosion.player` and `death.attack.explosion`, as the `message_id` of each type
says.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

## 6. Other exploders

A creeper, a bed, a respawn anchor, a fireball, an end crystal, a wind charge and the TNT minecart
use the same procedure. Each has its own power, damage source, block interaction mode and options. A
later document states them. EXP-DMG-001 already names the wind charge.

## 7. Intentional deviations

None.

## 8. Unresolved

- **U1.** Block behaviours that run at the destruction (for example a block that stops its
  content from dropping) beyond `tnt`. A list of all such blocks is not known.
- **U2.** The list of entity types that skip an explosion (EXP-DMG-001) is the list of the types
  that this document names. Other types can exist.
- **U3.** The sample count of EXP-DMG-002 can differ by one on an axis when the f64 sum of the
  steps rounds. No artefact decides it.
- **U4.** The effect of the blast protection enchantment (damage and the attribute) is out of scope
  here. `entity-damage.md` section 12 holds the hook.

## 9. Experiments

None of these experiments has been run. Each one needs `tools/vanilla-server` and the recorder.

- **E1.** Ignite one TNT block with flint and steel in survival mode. Record the entity data
  packet of the fuse and the tick of the explosion packet. Expected: fuse 80, and the packet at
  tick 80 after the spawn.
- **E2.** Put a TNT block next to a primed TNT. Record the fuse of the new entity in 1000 trials.
  Expected: only the values 10 through 29, each with the chance 1/20.
- **E3.** Put a player at `d` = 2, 4 and 6 blocks from a TNT. Record the health change and the
  velocity of the packet on each difficulty. Repeat with one stone block between them. Compare with
  EXP-DMG-001 to EXP-KNB-001.
- **E4.** Explode TNT on a stone plane and on obsidian. Record the set of removed positions and the
  drops. Do it in water. Expected: no block change in water.
- **E5.** Record the `explode` packet for a player in reach, a player at 70 blocks, and a creative
  flying player.
