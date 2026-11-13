# Hunger: food level, saturation, exhaustion, regeneration and eating

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` |
| World version (`DataVersion`) | `5127` |
| Protocol version | `1073742169` |
| Data pack format | `126.0` |

This document states the hunger state of a player. It states the three values of the state and the
sources of exhaustion. It states the natural regeneration, the starvation damage, the eat of food
items and the packet that tells the client. Health, damage and death are in `entity-damage.md`. This
document uses the terms of that document and does not state them again.

## 0. Conventions

### 0.1 Terms

- **Player**, **health**, **max health**, **damage type**, **damage source**, **hit**,
  **applied damage**, **absorption**, **dying state**: as `entity-damage.md`, section 0.1, defines them.
- **Hunger state**: the four values of one player: the food level, the saturation, the exhaustion
  and the hunger timer.
- **Food level**: an integer from 0 to 20.
- **Saturation**: an f32 value. It is the reserve that the exhaustion uses before the food level.
- **Exhaustion**: an f32 value from 0 to 40. Each action of a player adds to it.
- **Hunger timer**: an integer counter that the regeneration and the starvation share.
- **Heal**: a change of the health by natural regeneration. It stops at the max health.
- **Difficulty**: the difficulty of the world: `peaceful`, `easy`, `normal` or `hard`.
- **Regeneration rule**: the game rule `natural_health_regeneration`. Its default is true.
  The protocol id of the rule is 32 in `assets/mcrs/reports/registries.json`.
- **Food item**: an item with the data component `minecraft:food`.
- **Consumable item**: an item with the data component `minecraft:consumable`.
- **Use**: the right-click action of a player with an item in a hand.
- **Creative player**: a player with the invulnerable ability. Creative mode and spectator mode
  give this ability.
- **Hunger tick**: the step that this document states in HNG-TICK-001. It runs one time in each
  tick for each player.

### 0.2 Provenance

The labels are those of `docs/spec/registry.md`, section 0.3, and the kinds are those of
`docs/spec/README.md`. Each rule states its label, its kind and its confidence. No experiment of
section 9 has run. A rule that only the reference supports has the label `source_suggested`.

### 0.3 ID prefix

| Prefix | Subject |
|--------|---------|
| `HNG-STA` | the hunger state and its limits |
| `HNG-EXH` | the sources of exhaustion |
| `HNG-TICK` | the hunger tick and the order in a tick |
| `HNG-REG` | natural regeneration |
| `HNG-STV` | starvation |
| `HNG-EAT` | eating |
| `HNG-NET` | the packet that the client sees |
| `HNG-SAV` | the saved form |

### 0.4 Artefacts cited

- The item definitions `assets/mcrs/item_definition/<item>.json`: the components `food` and
  `consumable`. 44 items have `food`. 5 of the 44 have `can_always_eat`: `golden_apple`,
  `enchanted_golden_apple`, `chorus_fruit`, `honey_bottle` and `suspicious_stew`.
- The data-pack registry `minecraft:damage_type`: its field `exhaustion` (HNG-EXH-006) and its
  tag membership (HNG-STV-002).
- `assets/mcrs/reports/packets.json`: `set_health` is play clientbound 106.
- `assets/mcrs/reports/registries.json`: the game rule `natural_health_regeneration`.

## 1. The hunger state

**HNG-STA-001** (`source_suggested`, semantic, confidence: high). A new player has the food level
20, the saturation 5.0, the exhaustion 0.0 and the hunger timer 0. A player that respawns after a
death gets these values again. A player that changes dimension keeps its values.

**HNG-STA-002** (`source_suggested`, semantic, confidence: high). The food level stays from 0 to
20. An eat clamps the new food level to that range. The exhaustion stays from 0 to 40: an
addition that gives more than 40 gives 40.

**HNG-STA-003** (`source_suggested`, semantic, confidence: high). An eat adds the
saturation of the item to the saturation. The result is clamped to the range from 0 to the new food
level. Thus an eat never gives a saturation that is more than the food level. A drain of
the saturation (HNG-TICK-001) has no such clamp.

## 2. Sources of exhaustion

Each source adds a fixed amount to the exhaustion of the player. Section 2 states the amounts as
data. All additions are f32.

**HNG-EXH-001** (`source_suggested`, semantic, confidence: high). A creative player gets no
exhaustion from any source. The addition has no effect for such a player. A client
only sees the result as a food level that does not fall.

**HNG-EXH-002** (`source_suggested`, semantic, confidence: medium). The server adds exhaustion for
the move of a player that is not a passenger. It uses the distance of one accepted move. The
distance is rounded to a whole number `n` of centimetres, and a move with `n = 0` adds nothing.
The server checks these cases in this order, and only the first true case counts:

| Case | Distance | Exhaustion for one move |
|------|----------|-------------------------|
| the player is in the swimming pose | the full 3D distance | `0.01 × n × 0.01` |
| the eyes of the player are in water | the full 3D distance | `0.01 × n × 0.01` |
| the player is in water | the horizontal distance | `0.01 × n × 0.01` |
| the player is on a climbable block | none | 0 |
| the player is on the ground and sprints | the horizontal distance | `0.1 × n × 0.01` |
| the player is on the ground and does not sprint | the horizontal distance | 0 |
| any other case | none | 0 |

Thus a sprint of one block adds 0.1, and a swim of one block adds 0.01. A walk, a sneak and a
climb add nothing. Rounding to centimetres gives these amounts. A test must not
compare an exhaustion that differs from the result by less than 0.0001 for each move.

**HNG-EXH-003** (`source_suggested`, semantic, confidence: medium). A jump adds exhaustion once.
The amount is 0.2 when the player sprints, and 0.05 when it does not. The two amounts do not
add. The server detects a jump from a move packet. The player was on the ground, the client
says that it is not on the ground, and the move goes up.

**HNG-EXH-004** (`source_suggested`, semantic, confidence: medium). An attack of a player on an
entity adds 0.1 to the exhaustion when two conditions are true. The attack damage or the enchantment bonus
is more than 0. The target does not reject the hit (`entity-damage.md`, DMG-HIT-001 to
DMG-HIT-003). An attack with a rejected hit adds nothing. A target that is not a living entity
counts as a target that accepts the hit when the game accepts it. `player-attack.md` states the
attack itself.

**HNG-EXH-005** (`black_box_confirmed`, semantic, confidence: high). A survival break of a block
with a harvest adds 0.005. `block-breaking.md` states this rule as BRK-LOOT-002 and does not
change it here.

**HNG-EXH-006** (`source_suggested`, semantic, confidence: high). A player that loses health from
a hit adds the exhaustion of the damage type. The hit applies this when the applied damage that
passes the absorption is not 0 (`entity-damage.md`, DMG-HIT-004). The amount does not depend on the
amount of the damage. A hit that the absorption fully takes adds nothing. A hit of a damage type
with the field `exhaustion` of 0.0 adds nothing. The vanilla data pack gives these values:

| `exhaustion` | Damage types |
|--------------|--------------|
| 0.1 | `arrow`, `bad_respawn_point`, `cactus`, `campfire`, `dry_out`, `explosion`, `falling_anvil`, `falling_block`, `falling_stalactite`, `fireball`, `fireworks`, `hot_floor`, `in_fire`, `lava`, `lightning_bolt`, `mace_smash`, `mob_attack`, `mob_attack_no_aggro`, `mob_projectile`, `player_attack`, `player_explosion`, `spear`, `spit`, `sting`, `sulfur_cube_hot`, `sweet_berry_bush`, `thorns`, `thrown`, `trident`, `unattributed_fireball`, `wind_charge`, `wither_skull` |
| 0.0 | each other type, among them `starve`, `fall`, `drown`, `generic`, `generic_kill`, `cramming`, `freeze`, `magic`, `indirect_magic`, `on_fire`, `out_of_world`, `outside_border`, `in_wall`, `fly_into_wall`, `stalagmite`, `wither`, `ender_pearl`, `dragon_breath`, `sonic_boom` |

A data pack can change the field. The server must read it from the registry and must not use this
table as a constant.

**HNG-EXH-007** (`source_suggested`, semantic, confidence: high). Natural regeneration adds
exhaustion (HNG-REG-001 and HNG-REG-002). The effect `hunger` adds `0.005 × (amplifier + 1)` in
each tick. The enchantment entity effect `apply_exhaustion` adds an amount that its data states.
`status-effects.md` and the enchantment specification state the last two sources. This document names
them as the places where the exhaustion changes.

## 3. The hunger tick

**HNG-TICK-001** (`source_suggested`, semantic, confidence: high). In each tick, after the base
entity tick of the player, the server does the hunger tick. The server skips it when the player is
a spectator and touches an unloaded chunk. The hunger tick does these steps in this order:

1. Drain. If the exhaustion is more than 4.0, subtract 4.0 from it. Then:
   - if the saturation is more than 0, set it to `max(saturation - 1.0, 0)`;
   - else, if the difficulty is not `peaceful`, set the food level to `max(food level - 1, 0)`.
2. Pick one branch by the rules HNG-REG-001, HNG-REG-002 and HNG-STV-001, in that order. The
   first branch with a true condition runs. Each branch uses the hunger timer. When no branch
   has a true condition, set the hunger timer to 0.

The drain runs at most one time in a tick, so a large exhaustion falls by 4.0 in each tick. The
comparison is "more than 4.0", so an exhaustion of exactly 4.0 does not drain. Peaceful
difficulty stops the loss of the food level but not the loss of the saturation.

**HNG-TICK-002** (`source_suggested`, semantic, confidence: medium). The hunger tick of a tick
runs after the regeneration of peaceful difficulty (HNG-REG-004) and before the packet of
HNG-NET-001. A client sees the result of both steps in the one packet.

## 4. Natural regeneration

The branches of this section need the regeneration rule to be true. When the rule is false, the
branches of HNG-REG-001 and HNG-REG-002 never run.

**HNG-REG-001** (`source_suggested`, semantic, confidence: high). Saturated heal. This branch runs
when all these conditions are true:

- the regeneration rule is true;
- the saturation is more than 0;
- the food level is 20 or more;
- the health is more than 0 and less than the max health.

The branch adds 1 to the hunger timer. When the timer reaches 10, the branch does these steps and
sets the timer to 0:

1. Let `s = min(saturation, 6.0)`.
2. Heal by `s / 6.0`.
3. Add `s` to the exhaustion.

Thus a player with a saturation of 6 or more heals 1 health every 10 ticks, and it costs 6
exhaustion. A smaller saturation gives a smaller heal for a smaller cost.

**HNG-REG-002** (`source_suggested`, semantic, confidence: high). Normal heal. This branch runs
when at least one condition of HNG-REG-001 is false and all these conditions are true:

- the regeneration rule is true;
- the food level is 18 or more;
- the health is more than 0 and less than the max health.

The branch adds 1 to the hunger timer. When the timer reaches 80, the branch heals 1, adds 6.0 to
the exhaustion and sets the timer to 0. The saturation can be 0 here, or the food level can be 18
or 19.

**HNG-REG-003** (`source_suggested`, semantic, confidence: high). A heal changes the health to the
smaller of the health plus the amount and the max health. The change is not a hit. It causes no
damage event, no sound and no hurt animation. The absorption does not change.

**HNG-REG-004** (`source_suggested`, semantic, confidence: medium). Regeneration of peaceful
difficulty. This rule runs in addition to the rules above. It runs in each tick, before the hunger tick,
when the difficulty is `peaceful` and the regeneration rule is true. Let `t` be the number of ticks
that the player lived in the server, with `t` counted from 0:

- When `t` is a multiple of 20, do two things:
  - If the health is less than the max health, heal 1. Use the clamp of DMG-ATT-001. The heal
    has no exhaustion cost.
  - If the saturation is less than 20.0, add 1.0 to the saturation.
- When `t` is a multiple of 10: if the food level is less than 20, add 1 to the food level.

Thus a player on peaceful difficulty regains 1 health and 1 saturation each second and 1 food
level each half second. The saturation can pass the food level here (U1).

**HNG-REG-005** (`source_suggested`, semantic, confidence: high). When the regeneration rule is
false, the player gets no natural regeneration at any difficulty, and the peaceful rule of
HNG-REG-004 does not run. The effects `regeneration` and `instant_health`, and golden apples,
still heal. The exhaustion still drains the saturation and the food level (HNG-TICK-001). When
the food level is more than 0, the hunger timer stays at 0.

## 5. Starvation

**HNG-STV-001** (`source_suggested`, semantic, confidence: high). This branch runs when
HNG-REG-001 and HNG-REG-002 have no true condition, and the food level is 0 or less. It does not
read the regeneration rule, so it also runs when the rule is false. The branch adds 1 to the
hunger timer. When the timer reaches 80, the branch sets it to 0. Then it does one hit on the
player with the damage type `starve` and the amount 1.0. It does the hit when at least one of these conditions is
true:

- the health is more than 10;
- the difficulty is `hard`;
- the health is more than 1 and the difficulty is `normal`.

The hit uses the procedure of `entity-damage.md`, section 3, with no causing entity, no direct
entity and no source position. When no condition is true, the branch does not hit.

The result by difficulty, for a player at food level 0 and no other damage:

| Difficulty | The health at which the damage stops |
|------------|--------------------------------------|
| `peaceful` | the first value of 10 or less |
| `easy` | the first value of 10 or less |
| `normal` | the first value of 1 or less, so the player does not die |
| `hard` | none: the player dies |

Thus a hit with 1.0 on a health of 10.5 gives 9.5 on `easy`, and the starvation stops. The
`peaceful` row needs the food level to be 0, which the exhaustion does not give on `peaceful`
(HNG-TICK-001). It is possible after a command or a saved value.

**HNG-STV-002** (`source_suggested`, semantic, confidence: high). The damage type `starve` has the
tags `bypasses_armor`, `bypasses_effects` and `no_knockback`. Its `exhaustion` is 0.0 and its
scaling is "when caused by a living non-player entity". The source has no causing entity, so the
amount does not scale with the difficulty. A starvation hit is the full 1.0 on each difficulty, also
on `peaceful`. Armor and the effect `resistance` do not reduce it. The absorption reduces it
(`entity-damage.md`, DMG-RED-004). A creative player takes no starvation hit
(`entity-damage.md`, DMG-PLR-001). The cooldown of a hit applies: the interval of 80 ticks is
longer than the cooldown of 20 ticks.

## 6. Eating

**HNG-EAT-001** (`source_suggested`, semantic, confidence: high). The components of a food item
give these values. Both are data of the item definition:

| Field | Type | Meaning |
|-------|------|---------|
| `nutrition` | integer, 0 or more | the number of food levels that the item gives |
| `saturation` | f32 | the saturation that the item gives, as an absolute value |
| `can_always_eat` | boolean, default false | the item can start when the food level is 20 |

The `saturation` is not a factor of the nutrition. An apple has `nutrition` 4 and `saturation` 2.4,
and a golden apple has `nutrition` 4, `saturation` 9.6 and `can_always_eat`. The server must read
`saturation` as the f32 of the file (`item-stack.md`, STK-SHP-019).

**HNG-EAT-002** (`source_suggested`, semantic, confidence: high). A use of a consumable item starts
the consumption only when the player can eat. A player can eat in one of these cases:

- the player is a creative player;
- the item has `can_always_eat` true;
- the food level is less than 20.

A consumable item with no `food` component has no such test, so it always starts. When the player
cannot eat, the use fails: the player does not start to eat, and the stack does not change.

**HNG-EAT-003** (`source_suggested`, semantic, confidence: high). The consume time of an item is
the field `consume_seconds` of its `consumable` component. The default is 1.6. The consume time in
ticks is `trunc(consume_seconds × 20)`, with the multiplication in f32. Thus the default is 32
ticks. The item `dried_kelp` has 0.8, so 16 ticks. The item `honey_bottle` has 2.0, so 40 ticks. When
the consume time in ticks is 0, the use finishes at once, with no use state.

**HNG-EAT-004** (`source_suggested`, semantic, confidence: high). When the consume time is more
than 0, the player starts to use the item and the use state begins. The server counts the ticks that remain
down by 1 in each tick. The consumption finishes in the tick that gives 0. The consumption
does not finish in these cases:

- the player releases the use before that tick, then nothing is consumed and the hunger state does
  not change;
- the item in the used hand is not the same item at the finish, then the use stops with no effect.

**HNG-EAT-005** (`source_suggested`, semantic, confidence: high). At the finish, the server does
these steps in this order:

1. Play the consume sound of the item and the particles of the item, when the item has them.
2. When the item is a food item, apply the food: the food level and saturation change as
   HNG-STA-002 and HNG-STA-003 state. Play the sound `entity.player.burp`, with the volume 0.5.
3. Apply each entry of `on_consume_effects` of the consumable. The server does this only on the
   server side. The effects, for example `apply_effects` of a golden apple, are stated in
   `status-effects.md`.
4. Remove 1 item from the stack, unless the player is a creative player. A `use_remainder` of the
   item then applies. When the stack is empty after the removal, the remainder replaces the stack.
   When the stack is not empty, the stack stays with one item less, and the player receives the
   remainder as a new stack. A creative player gets no remainder.

Thus an apple at food level 10 and saturation 0 gives food level 14 and saturation 2.4. An apple at
food level 19 and saturation 5 gives food level 20 and saturation 7.4, clamped to 20.

**HNG-EAT-006** (`source_suggested`, semantic, confidence: medium). The hunger state of a player
that eats changes in the tick of the finish. The packet of HNG-NET-001 reports the new values in
that tick. The effect `saturation` gives food through the same eat step with `nutrition = amplifier +
1`, and with a saturation of `2 × nutrition`. `status-effects.md` states the effect.

**HNG-EAT-007** (`source_suggested`, semantic, confidence: medium). A cake bite gives 2 food levels
and a saturation of 0.4, through the same eat step. `block-placing.md` and the cake rules state the
block part. This document states only the change of the hunger state.

## 7. What the client sees

**HNG-NET-001** (`black_box_confirmed`, wire, confidence: high). The server part of a player sends
`set_health` (play clientbound 106). This extends DMG-NET-005. The fields are:

| Field | Type | Value |
|-------|------|-------|
| health | f32 | the health of the player |
| food | VarInt | the food level |
| saturation | f32 | the saturation |

The server sends the packet at the end of the player tick when at least one of these is true:

- the health differs from the health of the last packet;
- the food level differs from the food level of the last packet;
- the saturation is 0 and the saturation of the last packet was not 0, or the reverse.

The server sends the packet in the first tick after the player joins. It sends the packet again in
the first tick after a respawn and after a change of dimension. In these cases the server resets
the values that it remembers as sent. The server sends at most one packet in a tick.

**HNG-NET-002** (`source_suggested`, wire, confidence: high). A change of the saturation from one
value above 0 to another value above 0 sends no packet. The client keeps the saturation of the
last packet until the next packet. The exhaustion and the hunger timer never go to the client.
The client uses the food level for the hunger bar and for the permission to sprint. The client does
not start a sprint at a food level of 6 or less. A creative or flying player can sprint at any
level. The server does not check this permission when it reads the sprint command of the client.

**HNG-NET-003** (`source_suggested`, wire, confidence: medium). Starvation sends the packets of a
hit as `entity-damage.md`, section 9, states. A hit with the damage type `starve` is a full hit
and sends `damage_event` with the network id of that type. A heal sends no `damage_event`. The new
health goes to the client in the entity data (DMG-NET-002) and in `set_health`.

## 8. The saved form

**HNG-SAV-001** (`source_suggested`, semantic, confidence: high). The save file of a player holds
the four values as top-level fields of the player compound:

| Field | Type | Default when absent |
|-------|------|---------------------|
| `foodLevel` | int | 20 |
| `foodTickTimer` | int | 0 |
| `foodSaturationLevel` | float | 5.0 |
| `foodExhaustionLevel` | float | 0.0 |

The server writes all four fields. A reader that finds a field of the wrong tag type uses the
default for that field. The server does not clamp the values that it reads (U2).

## 9. Experiments

None of these experiments has run. Each one runs on a vanilla server of `26.4-snapshot-7` with the
console and an offline client that records the clientbound packet 106. Use `tick freeze` and `tick
step` for single ticks and `data get entity <player> foodLevel` for the values. A value that
the command does not show can be read from the saved player file.

| Id | Steps | Rules |
|----|-------|-------|
| H1 | Set `difficulty normal`. Give a player the food level 20 and the saturation 5. Do not move. Step 200 ticks. Record the saved exhaustion, the saturation and the packets. | HNG-STA-001, HNG-NET-001 |
| H2 | Set `foodExhaustionLevel` to 4.0, 4.001 and 40 in the saved player file while the player is offline, one value for each login. Step 3 ticks each time. Read the exhaustion, the saturation and the food level after each tick. | HNG-TICK-001, HNG-STA-002 |
| H3 | Repeat H2 with the saturation 0 on `peaceful`, `easy` and `hard`. | HNG-TICK-001 |
| H4 | Set the health 10, the food level 20 and the saturation 6. Step 40 ticks. Record the health, the exhaustion and the packets in each tick. Repeat with the saturation 3, and with the saturation 0 and the food level 18 for 200 ticks. | HNG-REG-001, HNG-REG-002 |
| H5 | Repeat H4 with the food level 17 and with `/gamerule natural_health_regeneration false`. | HNG-REG-002, HNG-REG-005 |
| H6 | On `peaceful`, set the health 5, the food level 5 and the saturation 0. Step 100 ticks. Record the health, food level and saturation in each tick. Repeat with the rule false. | HNG-REG-004, HNG-REG-005, U1 |
| H7 | Set the food level 0, the health 20, and the difficulty `easy`, `normal` and `hard` in turn. Step 1000 ticks each time. Record the health in each tick and the packets 25 and 106. Repeat on `peaceful` with the rule false. | HNG-STV-001, HNG-STV-002, HNG-NET-003 |
| H8 | Give a player an apple, a golden apple, a `dried_kelp`, a `honey_bottle` and a stew, at the food level 20 and 10, and in creative mode. Use each item with the input agent of `tools/vanilla-client`. Record the tick of the finish, the food level, the saturation, the stack count and the packets. | HNG-EAT-001 to HNG-EAT-005 |
| H9 | Release the use at the tick 20 of an apple. Change the held slot at the tick 20 of an apple. Record the hunger state and the stack. | HNG-EAT-004 |
| H10 | Set the food level 20, the saturation 5, with the exhaustion 0. Walk, sneak, sprint 10 blocks, jump, sprint-jump, swim 10 blocks on the surface and under water. Read the saved exhaustion after each step. | HNG-EXH-002, HNG-EXH-003 |
| H11 | Hit a zombie and a pig with a fist, and hit a zombie twice in the cooldown. Hit with a sword on a creative player target. Break a stone block with a pickaxe. Take the damage types of the table of HNG-EXH-006 by `/damage` with the amounts 0.5 and 5, also with absorption. Read the exhaustion. | HNG-EXH-004 to HNG-EXH-006 |
| H12 | Set the saturation 5 and the exhaustion 4.0 in the saved player file while the player is offline. Log in and step a tick, so that the saturation changes from 5 to 4. Then repeat with the saturation 1, so that it changes from 1 to 0. Record the packets 106. | HNG-NET-001, HNG-NET-002 |

## 10. Intentional deviations

None. This document states no behaviour that differs from the reference.

## 11. Unresolved

- **U1.** On peaceful difficulty, the regeneration of HNG-REG-004 adds 1.0 to the saturation when the
  saturation is below 20.0. The saturation can then reach 20.99. A food level below 20 then has a
  saturation that is more than the food level. A player can see the value in the packet.
  The question to the owner: keep this value as the correct behaviour, or clamp the saturation
  to the food level?
- **U2.** The reference does not clamp the values that it reads from a save. A food level of 25 or
  of -3 then stays until the next change. The question to the owner: state a clamp, or keep the
  stored value?
- **U3.** The condition of the attack exhaustion (HNG-EXH-004) is "the hit is not rejected". The
  rule for a target that is not a living entity needs experiment H11. The rule for a second hit
  in the cooldown that is a partial hit needs H11 also.
- **U4.** The tick of the first `set_health` after the join needs a capture of experiment H1 and of the
  join sequence. The packet after a respawn needs the same capture. The login specification can fix its
  place among the packets of the join.
- **U5.** The server does not check the sprint permission at a food level of 6 or less. A modified
  client can sprint and so can cause the exhaustion of HNG-EXH-002. The question to the owner:
  keep the server as it is, or reject the sprint command?
- **U6.** The effects `hunger`, `saturation` and `regeneration` and the cake are stated here only
  at the place where they change the hunger state. The specifications of those subjects must
  confirm the amounts.
