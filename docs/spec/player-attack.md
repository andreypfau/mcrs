# Player attack: the melee attack of a player on an entity

Version: see `assets/minecraft/version.json`. Packet ids: `assets/mcrs/reports/packets.json`.

This document states what occurs when a player attacks an entity with the main hand. It states
the attack packet, its checks and the attack strength. It states the amount, the damage source and
the critical hit. It states the knockback of an attack, the sweep attack and the kill credit. It
also states the packets and the sounds that a client receives for each result.

The hit itself uses the procedure of `entity-damage.md`, section 3. This document does not repeat
that procedure. It states only the inputs of the hit and the effects of the attack around the hit.

The rules come from the reference implementation. The experiments of section 13 confirm a part of
them against the vanilla server.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **tick**, **entity**, **living entity**, **hit**, **amount**, **full
  hit**, **partial hit**, **rejected hit**, **damage source**, **causing entity**, **direct
  entity**, **source position**, **dying state**, **tracking players**: as `entity-damage.md`,
  section 0.1, defines them.
- **Accepted hit**: a full hit or a partial hit.
- **Loaded client**: as `arm-swing.md`, section 0.1, defines it.
- **Server part of a player**: the step of PHY-ORD-010 step 6 of `entity-physics.md`.
- **Packet phase**: as `entity-physics.md`, section 0.1, defines it. In it, the server handles the
  packets of the clients one after the other, before the dimensions tick (PHY-ORD-018).
- **Apply step**, **sorted queue**, **writer key**: as `entity-physics.md` defines them (section
  0.1, PHY-CMP-015 and PHY-CMP-017). The apply step comes after the entity pass.
- **Attacker**: the player that sends the attack packet.
- **Attack packet**: the serverbound packet `attack` (ATK-PKT-001).
- **Target**: the entity that the attack hits. ATK-TGT-001 and ATK-TGT-006 select it.
- **Attack counter**: an integer counter of a player. It counts the server parts of the player
  since the last reset (ATK-STR-002).
- **Attack delay**: the f32 value `f32(1.0 / a × 20.0)`, with `a` the value of the attribute
  `attack_speed` of the attacker, in f64.
- **Attack strength**: the f32 value `s` of ATK-STR-001, from 0 to 1.
- **Full-strength attack**: an attack with `s > 0.9F`.
- **Sprint attack**: a full-strength attack while the sprint state of the attacker is on.
- **Critical attack**: an attack that meets the conditions of ATK-DMG-003.
- **Sweep attack**: an attack that meets the conditions of ATK-SWP-001.
- **Yaw angle**: the f32 value `r = θ × f32(π / 180)`, with `θ` the yaw of the attacker in degrees.
  The product is f32. It is widened to f64 for `sin` and `cos`.
- **`sin`, `cos`**: the table functions of `argument-coordinates.md`, ARG-COORD-007.
- **Look direction**: the horizontal vector `(-sin(r), cos(r))` of the attacker.
- **Extra knockback**: the knockback of an attack that adds to the default knockback of the hit
  (ATK-KB-002).
- **Player record**: a record of a living entity. It holds a player and a memory time
  (ATK-CRD-001).
- **Hurt-by record**: as `mob-combat-goals.md`, CGOAL-HURT-001, defines it.
- **Last-hit record**: as `mob-combat-goals.md`, CGOAL-MELEE-008, defines it.
- **Ally**: as `mob-combat-goals.md`, section 0.1, defines it. The test is true when the relation
  is true in one direction or in the other. An `allay` is also an ally of the player that it likes.
- **Kill credit**: the entity that a death names as its killer (ATK-CRD-002).
- **Edition parameter**: a value that differs between the Java and the Bedrock edition (section
  10).

### 0.2 Provenance

The labels are those of `registry.md`, section 0.3. Each rule states its kind and its confidence,
as `docs/spec/README.md` defines them.

### 0.3 ID prefix

Each rule has an ID with the prefix `ATK`. The second part names the section.

| Prefix | Subject |
|--------|---------|
| `ATK-PKT` | the attack packet and the checks before the attack |
| `ATK-TGT` | the target |
| `ATK-STR` | the attack strength |
| `ATK-DMG` | the damage source, the amount and the critical hit |
| `ATK-KB` | the knockback of an attack |
| `ATK-SWP` | the sweep attack |
| `ATK-CRD` | the records and the kill credit |
| `ATK-NET` | the packets and the sounds that a client receives |
| `ATK-ORD` | the place of an attack in the tick |
| `ATK-EDN` | the edition parameters |
| `ATK-DEV` | intentional deviations |

### 0.4 Artefacts cited

- `assets/mcrs/reports/packets.json`: the packet ids of this document.
- The data pack: the damage type `minecraft:player_attack`, the item tag `#minecraft:swords`, the
  entity type tag `#minecraft:redirectable_projectile` and the damage type tag
  `#minecraft:no_knockback`.
- The fixture `crates/mcrs_minecraft_entity/tests/fixtures/vanilla/attributes.json`: the base
  values of the player. They are `attack_damage` 1.0, `attack_speed` 4.0, `attack_knockback` 0.0,
  `sweeping_damage_ratio` 0.0 and `movement_speed` `0.10000000149011612`.
- The game rules `pvp` and `mcrs:dying_entities_targetable` (ATK-TGT-006).
- The sound events `entity.player.attack.crit`, `entity.player.attack.knockback`,
  `entity.player.attack.nodamage`, `entity.player.attack.strong`, `entity.player.attack.sweep` and
  `entity.player.attack.weak`.
- The particle types `minecraft:damage_indicator` and `minecraft:sweep_attack`.

## 1. The attack packet

**ATK-PKT-001** The serverbound packet `attack` (play 1) has one field: the network id of the
target, as a VarInt. The vanilla client sends it when the player attacks the entity under the
cursor. In the same client tick, it then sends `punch` (play 46), after `attack`. It sends
`set_carried_item` before `attack` when the selected slot changed.

Provenance: `source_suggested`. Kind: wire. Confidence: medium.

**ATK-PKT-002** The server ignores an attack packet in each of these cases:

- the attacker is not a loaded client;
- the attacker is in spectator mode.

An ignored packet does not hit, plays no sound and does not reset the attack counter. An attacker
in the dying state is not a loaded client (`player-death.md`, PLD-DTH-003). Creative mode,
survival mode and adventure mode attack in the same way.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E8; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

**ATK-PKT-003** The server tests the packet in this order. When a test fails, the packet has no
effect:

1. The network id names an entity in the dimension of the attacker. The id of a part of a
   multi-part entity names that part.
2. The block position of that entity is inside the world border.
3. The entity is in the attack reach of the attacker. `reach.md` states the reach check and its
   tolerances (RCH-CMP-001, RCH-CMP-002), and its baseline (RCH-VAN-001).
4. The item in the main hand does not have the component `piercing_weapon`. An empty hand passes
   this test.

The order is visible only through ATK-PKT-004: a packet that fails a test of this list does not
disconnect. Thus an attack on an `item` out of the attack reach has no effect.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E8; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

**ATK-PKT-004** When the tests of ATK-PKT-003 pass, the server disconnects the attacker with the
translation key `multiplayer.disconnect.invalid_entity_attacked` in each of these cases:

- the entity is the attacker;
- the entity is an `item` or an `experience_orb`;
- the entity is an `arrow`, a `spectral_arrow` or a `trident`. A projectile of the tag
  `#minecraft:redirectable_projectile` is a valid target.

The vanilla client does not select these entities, so it does not send such a packet. This rule
covers each such packet as one class.

When the entity is none of these, the server tests the item in the main hand two more times:

1. The features of the world enable the item.
2. The value `(t + 5) / D` is not less than the component `minimum_attack_charge` of the item. Here
   `t` is the attack counter and `D` the attack delay. An item without the component passes.

When a test fails, the packet has no effect, and the attack counter keeps its value. Items are out
of scope (section 11). An empty hand passes both tests.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E8; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

## 2. The target

**ATK-TGT-001** The entity of ATK-PKT-003 is the target, unless ATK-TGT-006 selects another one.
The target can be a living entity or another entity.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E1; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

**ATK-TGT-002** An entity that is not attackable ignores the attack. The attack then does not hit,
plays no sound and does not reset the attack counter. The types `falling_block`,
`firework_rocket` and `eye_of_ender` are not attackable.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E4; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

**ATK-TGT-003** Some entity types handle an attack with their own procedure. Examples are
`armor_stand`, `item_frame`, `painting`, `interaction`, `end_crystal`, a boat, a minecart and the
parts of `ender_dragon`. Their procedures are out of scope (section 11). This document states the
attack on a living entity with the procedure of `entity-damage.md`, section 3.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

**ATK-TGT-004** When the target is a player, the tests of DMG-PLR-001 of `entity-damage.md` apply
to the hit. Two of these tests read the attacker: the game rule `pvp` and the friendly fire of a
team. A rejected hit gives the result of ATK-DMG-005.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E7; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

**ATK-TGT-005** A target player in creative mode or spectator mode rejects the hit through
DMG-PLR-001. The attacker then hears `entity.player.attack.nodamage` (ATK-NET-001).

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E7; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

**ATK-TGT-006** The game rule `mcrs:dying_entities_targetable` controls an attack on an entity in
the dying state. Its default is false. The preset `java` sets it to true.

- When the rule is false, an entity in the dying state is not a target. The server casts its own
  ray from the attacker, with the eye, the look and the entity regions of `reach.md`, sections 4
  to 6. The ray leaves out each entity in the dying state. The target is the
  first entity on that ray within the attack reach and inside the world border (ATK-PKT-003 step
  2). RCH-CMP-007 of `reach.md` states the first entity. When the ray has no such entity, the attack has no effect.
- When the rule is true, the entity in the dying state is the target. Its hit is rejected
  (DMG-HIT-001 step 6), so the attack gives the result of ATK-DMG-005.

The vanilla client does not predict the damage, so it needs no correction when the server selects
another target. ATK-DEV-001 records the difference from the reference.

Provenance: `design`. Kind: deviation. Confidence: medium.

## 3. The attack strength

**ATK-STR-001** Let `t` be the attack counter and `D` the attack delay. The attack strength is
`s = clamp((f32(t) + 0.5F) / D, 0, 1)`, in f32. The attack uses the value of `s` at the moment when
the server handles the attack packet (ATK-ORD-001).

For a player with no modifier, `D` is 5.0. Then `t = 4` gives `s = 0.9F`, which is not more than
`0.9F`. Thus the attack is a full-strength attack only from `t = 5`. Each value of `t` from 5 gives
`s = 1`.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E2, ATK-E6; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

**ATK-STR-002** The server part of a player increases the attack counter by 1 in each server tick.
These events set the counter to 0:

1. an attack that passes the checks of section 1, on a target that does not ignore it
   (ATK-TGT-002). The reset occurs before the hit. It also occurs when the hit is rejected and
   when the amount is 0;
2. a `punch` packet that SWG-SRC-002 of `arm-swing.md` does not ignore;
3. a swing of a hand that `use_item`, `use_item_on` or `interact` starts (`arm-swing.md`,
   SWG-SRC-005 to SWG-SRC-008). An interaction that starts no swing does not reset the counter;
4. a drop from the selected slot with `player_action` `drop_item` or `drop_all_items`;
5. a change of the item in the main hand. In each server part, the server compares the item type
   in the main hand with the item type at the last server part. A change of components or of the
   count is not a change.

The reset of step 5 occurs after the increase of the same server part.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E2; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

**ATK-STR-003** The vanilla client keeps its own attack counter with the same rules. It shows the
attack indicator from that counter. The server must use the rules of ATK-STR-001 and ATK-STR-002.
Then, at a constant latency, a full indicator on the client gives a full-strength attack on the
server.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

## 4. The damage of an attack

**ATK-DMG-001** The damage source of an attack has these parts:

- the damage type `minecraft:player_attack`;
- the attacker as the causing entity;
- the attacker as the direct entity;
- no stored source position. The source position is thus the position of the attacker
  (DMG-KB-001).

An item can give another damage type. Items are out of scope (section 11).

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E1, ATK-E2; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

**ATK-DMG-002** The amount of an attack is computed in f32, in this order:

1. `A = f32(v) × (0.2F + s × s × 0.8F)`, with `v` the value of the attribute `attack_damage` of the
   attacker and `s` the attack strength.
2. If the attack is a critical attack, `A = A × 1.5F`.
3. The amount of the hit is `A`.

If `A` is 0 or less after step 1, the attack does not hit and plays no sound. The attack counter is
still reset (ATK-STR-002). Enchantments add a bonus to the amount. That bonus is out of scope
(section 11).

Thus a player with an empty hand and no modifier hits with the amount 1.0 at `s = 1`. A critical
attack gives 1.5. The attack strength is never 0: `t = 0` gives `s = 0.1F` and the amount
`0.208F`.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E1, ATK-E2, ATK-E6; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

**ATK-DMG-003** An attack is a critical attack when all these conditions are true:

- the attack is a full-strength attack;
- the fall distance of the attacker is more than 0;
- the attacker is not on the ground;
- the attacker is not on a climbable block;
- the attacker is not in water;
- the attacker does not have the effect `blindness`;
- the attacker is not a passenger;
- the target is a living entity;
- the sprint state of the attacker is off.

The fall distance and the ground flag come from the moves of the client (`entity-physics.md`,
section 11.1).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

**ATK-DMG-004** The attack hits the target with the damage source of ATK-DMG-001 and the amount of
ATK-DMG-002. The hit uses the procedure of `entity-damage.md`, section 3. After an accepted hit,
the attack does these steps, full hit or partial hit:

1. apply the extra knockback (ATK-KB-002);
2. if the attack is a sweep attack, do the sweep (ATK-SWP-002);
3. send the effects of ATK-NET-001 and ATK-NET-002;
4. set the last-hit record of the attacker (ATK-CRD-003);
5. send the damage particles (ATK-NET-005);
6. add the exhaustion of `hunger.md`, HNG-EXH-004.

A partial hit thus gives the extra knockback, the sweep and the exhaustion, but no default
knockback (DMG-HIT-005).

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E2, ATK-E5; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

**ATK-DMG-005** When the hit is rejected, the attack does no step of ATK-DMG-004. The server plays
`entity.player.attack.nodamage` (ATK-NET-001). The sound of a sprint attack still plays, because
it occurs before the hit (ATK-NET-001).

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E2, ATK-E4, ATK-E7; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

## 5. The knockback of an attack

**ATK-KB-001** A full hit of an attack applies the default knockback of DMG-KB-001, because the type
`player_attack` does not have the tag `no_knockback`. Its direction is away from the position of
the attacker. A partial hit applies no default knockback.

The vertical part follows DMG-KB-001 step 3. A target with the on-ground flag true gets
`min(0.4, v.y / 2 + power)`. A target with the flag false keeps its `v.y`. `entity-physics.md`,
PHY-ORD-017, states the flag of a new mob.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E1, ATK-E2, ATK-E9; `tools/mcrs_minecraft_probe/traces/player_attack.json`,
`tools/mcrs_minecraft_probe/traces/no_ai_ground.json`.

**ATK-KB-002** The extra knockback has the f32 strength `k = f32(w) / 2`, with `w` the value of the
attribute `attack_knockback` of the attacker. A sprint attack adds `0.5F` to `k`. Enchantments
change `k`; that change is out of scope. After an accepted hit with `k > 0`:

1. If the target is a living entity, apply the procedure of DMG-KB-001, steps 1 to 4, with the
   power `k` and the direction `(x, z) = (sin(r), -cos(r))`. The target thus moves along the look
   direction. The attribute `knockback_resistance` of the target reduces the power.
2. Else, add `(-sin(r) × k, 0.1, cos(r) × k)` to the velocity of the target.
3. Multiply the velocity of the attacker by `(0.6, 1.0, 0.6)`.
4. Set the sprint state of the attacker to off (`attributes.md`, ATR-SRC-002).

For a full hit, the extra knockback applies after the default knockback. Thus the target first
moves away from the attacker and then along the look direction. A player with no modifier has
`k = 0` except in a sprint attack. For a partial hit on a player target, step 1 does not change the
velocity of the target (ATK-KB-004). Steps 3 and 4 still apply to the attacker.

Step 4 occurs when the server handles the attack packet (ATK-ORD-002). A later packet of the same
client can set the sprint state again. Thus a sprint-start packet after a sprint attack leaves the
sprint state on (ATK-KB-003).

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E3; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

**ATK-KB-003** The vanilla client simulates the hit of its own attack. It applies steps 3 and 4 of
ATK-KB-002 to its own player only when that simulation accepts the hit. The simulation accepts the
hit when the target is another player. It also accepts the hit for some types with an own attack
procedure that are not living entities (ATK-TGT-003). For each other target, for example a mob,
the vanilla client keeps its velocity and its sprint state.

The server applies steps 3 and 4 to its own copy of the attacker for each target. The attacker and
the tracking players receive the new sprint state in `set_entity_data`. The attacker also receives
the change of the sprint modifier in `update_attributes`. The server sends no velocity to the
attacker for step 3.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

**ATK-KB-004** When the target is a player, the server computes its knockback from its own copy of
the velocity of that player (`entity-physics.md`, PHY-SRV-004). After a full hit, it sends the new
velocity to the target player in `set_entity_motion` (play 103), as an external impulse
(PHY-SRV-007). The packet carries the default knockback and the extra knockback together. Then
the server sets its copy back to the velocity before the hit. Issue #93 states how
the server brackets the packet and learns that the client applied it.

The server sends that packet when it handles the attack packet, in the packet phase (ATK-ORD-002).
On the connection of the target player, it comes before each packet of the entity pass of that tick.
The tracking players of the target player do not receive that velocity. A sweep hit on a player
target is different: the update after the entity pass sends its velocity (DMG-NET-004) to that
player and to its tracking players.

A partial hit gives the target player no velocity change (ATK-DEV-002).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

**ATK-KB-005** When the target is not a player, the knockback changes the velocity that the server
simulates. The hit applies when the server handles the attack packet, in the packet phase, before
the entity pass (ATK-ORD-002). The target then does its tick of the same server tick with the new
velocity. The update after the
entity pass sends the velocity in `set_entity_motion` (play 103) to the tracking players
(DMG-NET-004). The packet carries the velocity after the tick of the target:

- A mob with AI does its travel step (PHY-ORD-004). The travel step moves the mob by the knockback
  velocity and then changes the velocity by the gravity and the drag.
- A mob with `NoAI` true does no travel step (PHY-ORD-016). The packet carries the velocity
  directly after the default knockback and the extra knockback.
- A target that does not tick in that server tick also keeps the velocity directly after the
  knockback. A target in a frozen tick is such a target (PHY-ORD-011).

These examples are for an empty hand at full strength, with no modifier. The target is straight
ahead of the attacker, at the yaw 0, on a full block with the friction 0.6:

| Target before the hit | Velocity after the hit | Velocity in the packet |
|-----------------------|------------------------|------------------------|
| a mob with AI at rest on the ground: on-ground flag true, `v = (0, -0.0784000015258789, 0)`; its AI gives no input in that tick | `(0, 0.36080000519752503, 0.4000000059604645)` | `(0, 0.2751840104494096, 0.2184000286221508)` |
| a mob with `NoAI` true and the field `OnGround` true at the summon, `v = 0` | `(0, 0.4, 0.4000000059604645)` | `(0, 0.4, 0.4000000059604645)` |
| a mob with `NoAI` true and no field `OnGround` at the summon, `v = 0` | `(0, 0, 0.4000000059604645)` | `(0, 0, 0.4000000059604645)` |

The values are those before the wire encoding of the packet. The mob with AI starts to move in the
tick of the hit. Its on-ground flag is false after that tick. The number of `set_entity_motion`
packets for one hit in one tick is unspecified. It is at least one.

Provenance: `source_suggested_black_box_confirmed` for the values, `source_suggested` for the
tick of the packet (ATK-ORD-002). Kind: semantic; the number of packets is unspecified.
Confidence: high. Experiment: ATK-E1, ATK-E9. ATK-E9 step 7 shows the values of the first row on
the `x` axis, after one tick. The experiments read the velocity of the server with `data get`,
not from the packet.

## 6. The sweep attack

**ATK-SWP-001** An attack is a sweep attack when all these conditions are true:

- the attack is a full-strength attack;
- the attack is not a critical attack and not a sprint attack;
- the attacker is on the ground;
- `m < (p × 2.5)²`, in f64. Here `m` is the square of the horizontal length of the last movement
  of the attacker. `p` is the value of the attribute `movement_speed` of the attacker, as f32;
- the item in the main hand is in the tag `#minecraft:swords`.

The last movement of the attacker is the displacement of its last accepted move packet. The packet
`client_tick_end` sets it to zero when its client tick had no move packet. When the attacker rides
a vehicle that it does not control, the last movement is that of the vehicle.

Thus a player that walks can do a sweep attack, and a player that sprints cannot.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E5; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

**ATK-SWP-002** A sweep attack occurs only after an accepted hit on the target (ATK-DMG-004 step 2).
It does these steps:

1. Play `entity.player.attack.sweep` (ATK-NET-001).
2. Let the sweep amount be `(1.0F + f32(q) × A) × s`, in f32. Here `q` is the value of the
   attribute `sweeping_damage_ratio` of the attacker, `A` is the amount of ATK-DMG-002 step 1 and
   `s` is the attack strength.
3. Grow the box of the target by 1.0 on the `x` and `z` axes and by 0.25 on the `y` axis. Select
   the living entities whose box intersects that box. Leave out each entity that meets a condition of this
   list:
   - it is the attacker or the target;
   - it is a player in spectator mode;
   - it is an ally of the attacker;
   - it is an `armor_stand` with the flag `Marker`;
   - the square of its distance from the attacker is 9.0 or more. The distance is between the two
     feet positions, in f64.
4. Hit each selected entity with the damage source of ATK-DMG-001 and the sweep amount. After each
   accepted hit, full or partial, apply the procedure of DMG-KB-001, steps 1 to 4, with the power
   `0.4` and the direction `(sin(r), -cos(r))`. This sweep knockback is not the default knockback
   of ATK-KB-001. For a partial hit on a player target, it changes no velocity (ATK-DEV-002).
5. Send the sweep particle (ATK-NET-006).

A full sweep hit also applies the default knockback of its hit, before the sweep knockback. The
sweep sets no last-hit record and adds no exhaustion. ATK-ORD-003 states the place and the order
of the hits of step 4.

A player with no modifier has `q = 0`, so each sweep hit has the amount `s`. The sweep amount is
1.0 only at `s = 1`. For example, `s = 0.955` gives the sweep amount 0.955.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E5, ATK-E6; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

## 7. The records and the kill credit

**ATK-CRD-001** Each accepted hit whose causing entity is a player sets the player record of the
target. The record then holds that player and the memory time 100. This rule fills the hook of
DMG-HIT-005 step 1 for the kill credit. It applies to each source with a player as the causing
entity, not only to an attack.

A tame wolf as the causing entity sets the record to the owner of the wolf, with the memory time
100. A tame wolf with no owner clears the record.

In each base entity tick of the target, the memory time decreases by 1 when it is more than 0.
When the memory time is 0 at the start of that step, the record loses its player. The save keeps
the player as the UUID `last_hurt_by_player` and the memory time as
`last_hurt_by_player_memory_time`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

**ATK-CRD-002** The kill credit of a death (DMG-DTH-001 step 1) is selected in this order:

1. When the player record holds a player, the kill credit is that player. When that player is not
   in the dimension of the entity, the death has no kill credit.
2. Else the kill credit is the entity of the hurt-by record, when the record has one.
3. Else the death has no kill credit.

A death is a player kill when the memory time of the player record is more than 0. A player kill
lets the entity drop experience (`experience.md`, XP-DRP-002). It also gives the loot parameter
`last_damage_player` (`loot-conditions.md`, LOOT-CND-011). The death event (DMG-DTH-002) carries the
kill credit and the player of the record.

Thus a mob that a player hit, and that then dies from a fall within 100 ticks, drops experience.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

**ATK-CRD-003** Each accepted hit of an attack on the target sets the last-hit record of the
attacker (CGOAL-MELEE-008). When the target is not a living entity, the record holds no entity and
the tick count of the hit. The owner-hurt target goal of a tame mob reads this record
(CGOAL-OWNER-002). The hits of a sweep do not set it.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

**ATK-CRD-004** An accepted hit of an attack on a mob sets the hurt-by record of that mob, as
CGOAL-HURT-001 states. The hits of a sweep also set it.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

## 8. What a client receives

**ATK-NET-001** The server sends these sounds as `sound` packets (play 119), at the position of the
attacker. The source is `player`. The volume is 1.0 and the pitch is 1.0. Each player within the
range of DMG-NET-007 receives them, the attacker included.

| Sound event | Condition |
|-------------|-----------|
| `entity.player.attack.knockback` | a sprint attack with an amount more than 0, before the hit, also when the hit is rejected |
| `entity.player.attack.nodamage` | the hit is rejected (ATK-DMG-005) |
| `entity.player.attack.sweep` | a sweep attack after an accepted hit |
| `entity.player.attack.crit` | a critical attack after an accepted hit |
| `entity.player.attack.strong` | an accepted hit of a full-strength attack that is not a critical attack and not a sweep attack |
| `entity.player.attack.weak` | an accepted hit of an attack that is not a full-strength attack |

The target plays its own hurt sound or death sound (DMG-NET-007). An attack with the amount 0 or
less plays no sound.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E1, ATK-E2, ATK-E5, ATK-E7; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

**ATK-NET-002** After an accepted hit of a critical attack, the server sends `animate` (play 2). It
holds the network id of the target and the action 1. The players that track the attacker and the
attacker receive it. The action 2 marks an enchantment bonus; it is out of scope (section 11).

Provenance: `source_suggested`. Kind: wire. Confidence: medium.

**ATK-NET-003** A full hit of an attack sends `damage_event` (DMG-NET-001). With the vanilla data
pack, the damage type id is 34 (`player_attack`). The causing entity and the direct entity are both
the network id of the attacker plus 1. The packet has no source position.

Provenance: `source_suggested`. Kind: wire. Confidence: medium.

**ATK-NET-004** An attack sends `hurt_animation` (play 43) as DMG-NET-006 of `entity-damage.md`
states. The fields are:

| Field | Type | Value |
|-------|------|-------|
| entity | VarInt | the network id of the target |
| yaw | f32 | `f32(atan2(dz, dx) × 180 / π - θ)`, in f64 |

Here `(dx, dz)` is the horizontal vector from the target to the source position, and `θ` is the yaw
of the target in degrees. For an attack, the source position is the position of the attacker. A
differential test allows a difference of 0.01 degree in the yaw.

Provenance: `source_suggested`. Kind: wire. Confidence: medium.

**ATK-NET-005** This rule applies after an accepted hit on a living target. Let `d` be the health
of the target before the hit minus its health after the hit, in f32. When `d > 2.0F`, the server sends
`level_particles` (play 48) with the particle `damage_indicator`. The players within 32 blocks
receive it. The fields are:

- the position: the `x` and `z` of the target, and the `y` of the target plus half its height;
- the count: `trunc(f64(d) × 0.5)`;
- the offsets: 0.1, 0.0 and 0.1;
- the speed: 0.2.

Absorption does not count in `d`. The hits of a sweep send no such particles.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

**ATK-NET-006** A sweep attack sends `level_particles` (play 48) with the particle `sweep_attack`
to the players within 32 blocks. Let `(dx, dz) = (-sin(r), cos(r))`, widened to f64. The fields are:

- the position: `(x + dx, y + h / 2, z + dz)`, with `(x, y, z)` the position of the attacker and
  `h` its height;
- the count: 0;
- the offsets: `dx`, 0.0 and `dz`;
- the speed: 0.0.

The packet goes also when the sweep hits no entity.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

**ATK-NET-007** The attack packet itself starts no swing (`arm-swing.md`, SWG-SRC-004). The swing
that a viewer sees comes from the `punch` packet that follows it.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E7; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

## 9. The place of an attack in the tick

**ATK-ORD-001** The attack reads the state of the attacker that the packets before it set, in the
order in which the client sent them. This state includes the attack counter, the sprint state,
the fall distance, the ground flag, the last movement and the main-hand item. The `punch` packet
that follows the attack does not change the strength of that attack. The increase of the attack
counter in the next server part does not change it.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
ATK-E2; `tools/mcrs_minecraft_probe/traces/player_attack.json`.

**ATK-ORD-002** The server handles the attack packet in the packet phase of a server tick
(PHY-ORD-010, step 1). Each hit that a client packet causes applies when the server
handles that packet, before the server handles the next packet (PHY-ORD-018). The primary hit of an
attack and its sweep hits are such hits. The packet phase comes before the dimensions tick. It also
runs when the tick command freezes the tick.

Each server tick thus has two apply points for the hits of other entities:

| Apply point | Place in the tick | Hits that it takes | Order of the hits |
|-------------|-------------------|--------------------|-------------------|
| the client packet | in the packet phase, before the entity pass | each hit that the packet causes | the order of the packets (ATK-ORD-004), then ATK-ORD-003 |
| the apply step (PHY-CMP-015) | after the entity pass, before the update | each hit that the tick of an entity writes on another entity, and the own hits of DMG-ORD-002 | PHY-CMP-015 |

The hits of the base entity tick apply at their step in the pass (DMG-ORD-002).

In mcrs, the hits of the packets of one tick apply after the last packet of that tick, before the
entity pass. ATK-DEV-006 states this difference from the reference.

A user of the library can change or cancel each hit before it applies. For the packet phase, the
user gets the hits of all packets of the tick together, in the order of the packets, before the
first of them applies. The hits of one attack packet are its primary hit and its sweep hits. In
the apply step, the user gets all hits of that step together, before the first of them applies.

The hit of a packet applies in the tick of its packet, also when the attacker and the target are in
different cells. The exception "write across a cell boundary" of PHY-INV-014 does not apply to it.
The update after the entity pass of the same tick sends the result of the hit: the entity data, the
sounds and the packets of section 8, and the velocity of ATK-KB-005.

Provenance: `source_suggested` for the place in the tick, `design` for the change of a hit by a
user of the library. Kind: semantic; deviation (ATK-DEV-006). Confidence: high. ATK-E1 to ATK-E9
show the hits of attacks in a frozen tick.

**ATK-ORD-003** Each hit of an attack has the writer key of the attacker. The sweep hits of
ATK-SWP-002 also have it. When the primary hit is accepted, its sweep hits apply directly after it,
before the hits of the next packet. Among themselves, the sweep hits apply in the order of
PHY-CMP-015. A rejected primary hit gives no sweep hit. Thus the entity pass of the same tick sees
the sweep hits (ATK-ORD-005). In the reference, a later packet of the same packet phase also sees
them (ATK-ORD-004). ATK-DEV-006 states the difference in mcrs.

The reference applies the sweep hits in the order in which it finds the sweep targets. Each sweep
hit changes only its own target. The order of the sweep hits thus changes only the order of their
packets on the wire. It is not a deviation.

Provenance: `source_suggested` for the place of the sweep hits, `design` for their order. Kind:
semantic. Confidence: medium.

**ATK-ORD-004** The packet phase handles the packets in the order of PHY-ORD-018. The packets of
one connection come in the order in which the client sent them. The packets of different
connections come in the order of their arrival at the server. Each attack packet sees each result
of the packets before it in the same packet phase. These results follow:

- The hits of one attacker apply in the order of its attack packets.
- When two players hit one target in one tick, the hit of the packet that arrived first applies
  first. The cooldown of DMG-HIT-003 then applies to the second hit. The knockback direction, the
  sounds of section 8 and the kill credit follow that order.
- A later attack packet reads the target after the hits of the earlier packets: the health, the
  damage cooldown, the dying state and the velocity after the knockback. Example: a second attack
  on an entity that the first attack killed finds that entity in the dying state (ATK-TGT-006).
- The last-hit record of the attacker (ATK-CRD-003) holds the target of the last attack packet of
  that attacker with an accepted hit.
- The attack reads the state of the attacker at its packet (ATK-ORD-001).

In mcrs, the hits apply in this order. But the checks of a later attack packet read the state from
before the hits of the same tick, so the third result differs. ATK-DEV-006 states the difference.

A player cannot select the order of arrival between connections. A differential test thus does not
compare a result that depends on that order (PHY-ORD-018).

Provenance: `source_suggested`. Kind: semantic; the order between connections is unspecified
(PHY-ORD-018); deviation (ATK-DEV-006). Confidence: high.

**ATK-ORD-005** The entity pass of a tick sees each result of the packet phase of that tick.
For a target that is a living entity and not a player, these results follow:

1. The base entity tick of the target decreases the hurt time and the damage cooldown of a full
   hit in the same tick (DMG-HIT-006). A full hit sets them to 10 and 20, so the target ends the
   tick with 9 and 19.
2. A hit of the entity pass on the same target in the same tick applies after the packet hit. After
   a full packet hit, the damage cooldown is more than 10. Thus such a hit is a partial hit or a
   rejected hit, unless its damage type has the tag `bypasses_cooldown` (DMG-HIT-003).
3. A target that the hit kills is in the dying state in the entity pass of that tick. Its base
   entity tick increases the death time to 1 in that tick (DMG-DTH-003), and DMG-DTH-004 applies to
   its tick. The pushes of that tick read it as not pushable.
4. The travel step of the target uses the knockback velocity in the same tick (ATK-KB-005).
5. The AI of the target reads the records of the hit in the same tick: the hurt-by record
   (ATK-CRD-004) and the damage record of the panic (`mob-goals.md`, GOAL-PNC-001). The owner-hurt
   target goal of a tame mob reads the last-hit record of the attacker (ATK-CRD-003) in the same
   tick.

These results also apply to each sweep target (ATK-ORD-003). For a player target, the damage
cooldown decreases in the server part of the player (DMG-PLR-002), after the dimensions tick.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

## 10. The edition parameters

**ATK-EDN-001** These values differ between the editions. The rules of this document state the
Java values. One game world has one value for each parameter (issue #669).

| Parameter | Java | Bedrock | Rule |
|-----------|------|---------|------|
| attack strength | ATK-STR-001 | no attack counter: `s` is always 1 | ATK-STR-001, ATK-DMG-002 |
| sweep attack | ATK-SWP-001 | no sweep attack | ATK-SWP-001 |
| the form of the knockback that the client applies | `set_entity_motion` | the edge sends the motion in the form that the Bedrock client applies (issue #655) | ATK-KB-004 |

The velocity that the server computes is the same for each edition. Only the edge of an edition
changes how the client receives it.

Provenance: `documentation`. Kind: semantic. Confidence: low.

## 11. Out of scope

| Subject | Where it is stated, or the reason | Hook |
|---------|-----------------------------------|------|
| weapons and the attribute modifiers of items; the item components `damage_type`, `minimum_attack_charge`, `piercing_weapon`, `attack_range` and `weapon`; the mace, the spear and the riptide of a trident | issue #121 | ATK-PKT-003 step 4, ATK-DMG-001, ATK-DMG-002 |
| enchantments: the bonus of the amount, the bonus of the knockback, the post-attack effects and the action 2 of `animate` | a later document | ATK-DMG-002, ATK-KB-002, ATK-NET-002 |
| the wear of the weapon | issue #118 | ATK-DMG-004 |
| armour | issue #122 | `entity-damage.md`, DMG-RED-001 |
| effects: `blindness`, `strength`, `weakness`, and the effects that act on a hit | issues #107 and #113 | ATK-DMG-003, ATK-DMG-002 |
| shields and the component `blocks_attacks` | issue #121 | `entity-damage.md`, DMG-HIT-005 |
| projectiles, and the deflection of a projectile of `#minecraft:redirectable_projectile` by an attack | issue #111, `projectiles.md` | ATK-PKT-004 |
| the reach check | `reach.md` | ATK-PKT-003 step 3, ATK-TGT-006 |
| the delivery of a velocity to a player | issue #93 | ATK-KB-004 |
| the entity types with an own attack procedure | a later document | ATK-TGT-003 |
| teams and the ally relation of teams | a hook, as in `mob-combat-goals.md` | ATK-TGT-004, ATK-SWP-002 |
| statistics, scoreboard criteria and advancement triggers of an attack and a kill | no document | ATK-CRD-002 |
| the client: the attack indicator, the pick of the target, the prediction of the attack | the client specifications | ATK-STR-003 |

## 12. Intentional deviations

### ATK-DEV-001: An entity in the dying state is not a target

- **Rule:** ATK-TGT-006.
- **Reference:** an entity in the dying state keeps its box and stays a target. The hit is
  rejected. The attacker hears `entity.player.attack.nodamage`. An entity behind the dying entity
  gets no hit.
- **Reason:** a player expects the attack to reach the entity behind a corpse. The owner decided
  this on issue #106.
- **Visible to the client:** yes. With the default, the entity behind the dying entity takes the
  hit.
- **Bug reference:** MC-1246.
- **Test:** a differential test sets `mcrs:dying_entities_targetable` to true. With false, it does
  not compare an attack on a dying entity.

### ATK-DEV-002: A partial hit does not change the velocity of a player target

- **Rule:** ATK-KB-004, ATK-SWP-002.
- **Reference:** the extra knockback and the sweep knockback of a partial hit change the server copy of the velocity of
  the target player (PHY-SRV-004). The server does not send that change to the target player. The copy keeps
  the change, and a later knockback starts from it.
- **Reason:** the client of the target never applies that change. A copy that differs from the
  client is a defect.
- **Visible to the client:** no. The target player sees no knockback in both cases. A later
  knockback can differ by a small amount.
- **Bug reference:** none.
- **Test:** a differential test compares no velocity of a player target after a partial hit.

### ATK-DEV-006: The hits of the client packets of a tick apply after the last packet of the tick

- **Rule:** ATK-ORD-002, ATK-ORD-003, ATK-ORD-004; `entity-physics.md`, PHY-ORD-018;
  `entity-damage.md`, DMG-ORD-001, DMG-ORD-004.
- **Reference:** each attack packet takes effect when the server handles it. Its hits apply before
  the server handles the next packet. A later packet of the same tick sees each result of the
  earlier attack packets.
- **mcrs:** the packets of one tick reach the handlers in the order of their arrival (PHY-ORD-018).
  The handler of an attack packet does its checks and writes its hits to a queue. After the last
  packet of the tick, and before the entity pass, one apply step applies the hits of the queue in
  the order of the queue. That order is the order of arrival. Each hit applies after the earlier
  hits. Thus the health, the damage cooldown, the death, the last-hit record and the knockback of
  the hits compose as in the reference. These differences remain:
  - The checks of a handler read the state from before the hits of the same tick. These checks
    are the live target, the reset of the attack counter and the reach.
  - A user of the library gets the hits of all packets of the tick as one batch, in the order of
    arrival (ATK-ORD-002). The user does not get the hits of one packet at a time.
  - Another packet of the same tick does not see the hits of the earlier attack packets. For
    example, a command that reads the health reads the health from before these hits.
- **Reason:** an apply for each packet needs a packet phase that is one serial loop over the
  packets, with the hit pipeline after each packet. That loop waits for the rework of the command
  and packet phase (issues #1099 and #1101).
- **Visible to the client:** yes, only when two packets of one tick depend on each other. Example:
  two players hit one mob in one tick, and the first hit kills it. In mcrs, the handler of the
  second packet still sees a live target. It resets the attack counter of the second player and
  writes a hit, and the apply step rejects that hit. In the reference, the second attack finds the
  mob in the dying state (ATK-TGT-006). With the default of that rule, the second attack skips the
  mob. When no other entity is on its ray, the attack has no effect, and the attack counter keeps
  its value.
- **Bug reference:** none.
- **Test:** a differential test does not send, in one tick, an attack packet and a later packet that
  reads the target or the attacker of that attack.

## 13. Experiments

The experiments ATK-E1 to ATK-E8 ran in `tools/mcrs_minecraft_probe/scenarios/player_attack.json`
and produced `tools/mcrs_minecraft_probe/traces/player_attack.json`. ATK-E9 ran in
`tools/mcrs_minecraft_probe/scenarios/no_ai_ground.json` and produced
`tools/mcrs_minecraft_probe/traces/no_ai_ground.json`. The world is flat and peaceful, with the
game rule `natural_health_regeneration` false. The ground is a full block, with its top at
`y = -60`. The attacker stands at `(0.5, -60, 0.5)` with the yaw 0, on the ground, in survival
mode, with no modifier. In ATK-E1 to ATK-E8, the attacker is `attackprobealice`, unless a step
names `attackprobebob`. Each mob target
of ATK-E1 to ATK-E8 has these save fields at the summon: `NoAI` true, `OnGround` true and `Motion`
`(0, 0, 0)`. Thus the knockback lifts it (DMG-KB-001 step 3), and it does not move
(PHY-ORD-016). The tick is frozen in ATK-E1 to ATK-E6 and in ATK-E9, so a target keeps its
velocity and its cooldown.

- **ATK-E1** (a full hit with an empty hand):
  1. Summon a `pig` at `(0.5, -60, 2.5)`, with `NoAI` true, `OnGround` true and `Motion` zero
     (`summon_targets`).
  2. Attack the pig at full strength (`a1_attack_pig_full`).
  - Result: the health goes from 10.0 to 9.0. The velocity goes from `(0, 0, 0)` to
    `(0, 0.4, 0.4)`. The exhaustion goes from 0.0 to 0.1. The attacker receives
    `entity.player.attack.strong` at its own position, with the source `player`, the volume 1.0
    and the pitch 1.0.
  - The vertical 0.4 comes from the field `OnGround` of the summon. The pig does no move
    procedure, so no tick changes that flag (PHY-ORD-017). ATK-E9 shows a pig without that field.
- **ATK-E2** (attack strength, partial hit and rejected hit):
  1. Send `punch` and then attack a `cow` at `(-1.5, -60, 2.5)`, with no delay
     (`a2_attack_cow_weak`).
  2. Wait 1500 ms and attack the cow again (`a2_attack_cow_partial`).
  3. Send `punch` and then attack the cow, with no delay (`a2_attack_cow_rejected`).
  - Result (step 1): the health goes from 10.0 to 9.792. The cow moves away from the attacker. The
    attacker receives `entity.player.attack.weak`.
  - Result (step 2): the health goes to 9.0. The velocity does not change. The attacker receives
    `entity.player.attack.strong` and no hurt sound of the cow.
  - Result (step 3): the health stays 9.0. The attacker receives
    `entity.player.attack.nodamage`.
  - Result: the exhaustion increases by 0.1 in step 1 and in step 2, and not in step 3.
- **ATK-E3** (extra knockback):
  1. Set the base of `attack_knockback` of the attacker to 2 (`a3_set_knockback`).
  2. Attack a `sheep` at `(2.5, -60, 0.5)` (`a3_attack_sheep`).
  3. Attack a `goat` with `knockback_resistance` 1 (`a3_attack_goat`).
  - Result (step 2): the velocity of the sheep is `(0.2, 0.4, 1.0)`.
  - Result (step 3): the goat loses 1.0 health, and its velocity stays `(0, 0, 0)`.
- **ATK-E4** (an invulnerable target and a target that is not attackable):
  1. Attack a `villager` with the flag `Invulnerable` (`a4_attack_invulnerable_survival`).
  2. Set the attacker to creative mode and attack the villager again
     (`a4_attack_invulnerable_creative`).
  3. Attack a `falling_block` (`a4_attack_falling_block`).
  - Result (step 1): the health stays 20.0, and the attacker receives
    `entity.player.attack.nodamage`.
  - Result (step 2): the health goes to 19.0, and the exhaustion does not change.
  - Result (step 3): the attacker receives no sound.
- **ATK-E5** (a sweep attack at full strength):
  1. Put a `diamond_sword` in the main hand of the attacker (`a5_prepare_sweep`).
  2. Summon a `pig` at `(0.5, -60, 2.5)`, a `sheep` at `(1.5, -60, 2.5)` and a `cow` at
     `(-0.5, -60, 3.5)`.
  3. Attack the pig at full strength (`a5_attack_sweep`).
  - Result: the pig loses 7.0 health and the sheep loses 1.0. The cow, at the squared distance 10,
    keeps its health. The velocity of the sheep is `(0.0894, 0.4, 0.5789)`.
  - Result: the attacker receives `entity.player.attack.sweep` and no
    `entity.player.attack.strong`. The exhaustion increases by 0.1 only.
- **ATK-E6** (a sweep attack at a strength below 1):
  1. Set the base of `attack_speed` of the attacker to 2.6, so that `D` is 100
     (`a6_prepare_slow_sword`).
  2. Summon a `pig` and a `sheep` with the health 100, at the places of ATK-E5.
  3. Send `punch`, wait 4750 ms and attack the pig (`a6_attack_sweep_partial`).
  - Result: the pig loses 6.50734 health, which gives `s = 0.955`. The sheep loses 0.955 health.
- **ATK-E7** (a player target):
  1. Put `attackprobebob` at `(0.5, -60, 2.5)`, in survival mode (`a7_prepare_pvp`).
  2. Set the game rule `pvp` to false and attack `attackprobebob` (`a7_attack_bob_pvp_off`).
  3. Set `pvp` to true and attack again (`a7_attack_bob_pvp_on`).
  4. Set `attackprobebob` to creative mode and attack again (`a7_attack_bob_creative`).
  5. Send `punch` from the attacker (`a7_punch_swing_control`).
  - Result (step 2): the health stays 20.0. Both players receive `entity.player.attack.nodamage`.
  - Result (step 3): the health goes to 19.0. Both players receive `entity.player.attack.strong`.
  - Result (step 4): the health stays 19.0. Both players receive
    `entity.player.attack.nodamage`.
  - Result: `attackprobebob` receives no `swing_animation` in steps 2 to 4 and one in step 5.
- **ATK-E8** (ignored and invalid packets):
  1. Set the attacker to spectator mode and attack `attackprobebob` (`a8_attack_bob_spectator`).
  2. Summon an `item` at the distance 10 from `attackprobebob`. Attack it from `attackprobebob`
     (`a8_attack_item_out_of_reach`).
  3. Summon an `item` at the distance 1 and attack it from `attackprobebob` (`a8_attack_item`).
  - Result (step 1): no player receives a sound, and the health stays 19.0.
  - Result (step 2): nothing occurs.
  - Result (step 3): the server disconnects `attackprobebob` with
    `multiplayer.disconnect.invalid_entity_attacked`.
- **ATK-E9** (the on-ground flag and the velocity of a mob with AI and with `NoAI` true): the
  attacker is `noaiprobealice`. A step that advances the frozen tick uses `tick step`. No mob has
  the field `OnGround` at the summon, except the second cow.
  1. Summon these mobs (`summon_targets`):
     - a `pig` with `NoAI` true at `(0.5, -60, 2.5)`, on the ground;
     - a `sheep` with `NoAI` true at `(2.5, -58, 0.5)`, 2 blocks above the ground;
     - a `cow` with AI at `(-1.5, -60, 0.5)`, on the ground;
     - a `cow` with AI and `OnGround` true at `(-2.5, -60, -2.5)`, on the ground.
  2. Read `OnGround` of each mob (`t0`).
  3. Advance one tick. Read `OnGround` and `Motion` of each cow (`t1`). Do this step again (`t2`).
  4. Advance 20 ticks. Read `OnGround` and `Pos` of the pig and the sheep (`t22`).
  5. Attack the pig at full strength, and then the sheep (`hit_pig`, `hit_sheep`). Read `Motion` of
     each.
  6. Advance 20 ticks. Read `Pos` and `Motion` of the pig and the sheep (`after_20`).
  7. Attack the first cow at full strength (`hit_cow`). Read its `Motion`. Advance one tick and
     read `Motion` and `OnGround` again (`cow_step`).
  - Result (step 2): `OnGround` is false for each mob, except the second cow.
  - Result (step 3): after the first tick, `OnGround` is false for both cows. After the second
    tick, it is true for both cows. After each of the two ticks, `v.y` of each cow is
    `-0.0784000015258789`.
  - Result (step 4): `OnGround` of the pig and the sheep stays false. They keep their positions.
    The sheep does not fall.
  - Result (step 5): `Motion` of the pig is `(0, 0, 0.4000000059604645)`. `Motion` of the sheep is
    `(0.4000000059604645, 0, 0)`. The hit lifts neither of them.
  - Result (step 6): the pig and the sheep keep their positions and the velocities of step 5.
  - Result (step 7): `Motion` of the cow is `(-0.4000000059604645, 0.36080000519752503, 0)` after
    the hit. After one tick, it is `(-0.2184000286221508, 0.2751840104494096, 0)`, and
    `OnGround` is false. The horizontal value is the value of the hit multiplied by the drag
    `f32(0.6F × 0.91F)` (PHY-GRV-007), so the AI of the cow gives no input in that tick.

## 14. Unresolved

- **U2.** The Bedrock values of ATK-EDN-001 come from the public documentation of the game. No
  Bedrock artefact confirms them. The conditions of a critical attack in the Bedrock edition are
  not known, for example whether a sprint attack can be critical.
- **U3.** The names of the game rules for the edition parameters of ATK-EDN-001 are not decided
  (issue #669).
- **U4.** The vertical form of the Bedrock knockback (MCPE-159055) is not stated. The Bedrock edge
  needs it for ATK-EDN-001.
- **U5.** No experiment confirms these rules yet. Each needs an input that the experiment tools do
  not have:
  - ATK-PKT-001, ATK-STR-003 and ATK-KB-003: a capture of the vanilla client during an attack on a
    mob and on a player.
  - ATK-DMG-003 and the sprint attack of ATK-NET-001: move packets with a chosen ground flag and a
    fall distance, and the sprint action of `player_command`.
  - ATK-KB-004, ATK-KB-005 and ATK-NET-002 to ATK-NET-006: an observer for `set_entity_motion`,
    `damage_event`, `hurt_animation` and `level_particles`.
  - The place of the hits of the packet phase before the entity pass (ATK-ORD-002, ATK-ORD-005):
    an observer for `set_entity_motion` in a tick that is not frozen. In a frozen tick, a hit before
    the pass and a hit after the pass give the same values.
  - ATK-ORD-004 for two attackers: a way to set the order of arrival of the packets of two clients
    in one tick.
  - The case "not a loaded client" of ATK-PKT-002: an attack within 60 ticks after the join.
- **U6.** ATK-TGT-003 and ATK-CRD-001 to ATK-CRD-004 have no experiment yet. The procedures of
  ATK-TGT-003 belong to later documents. A run of the kill credit needs a death after an attack
  and a count of the experience that drops.
