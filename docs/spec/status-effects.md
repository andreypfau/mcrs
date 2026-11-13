# Status effects: how an effect lives on an entity

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

This document states how a status effect lives on a living entity. It states how an effect is
added, how two effects of one kind merge, how an effect ticks down, and how it ends. It states
what a server must apply each tick for each effect that acts, and the packets that a client sees.
It states what a player file keeps.

`entity-damage.md` states the reductions that `resistance` and `absorption` give. `hunger.md` states
the exhaustion and the natural regeneration.



## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **tick**, **living entity**, **f32**, **f64**, **data pack**: as
  `entity-physics.md`, section 0.1, defines them.
- **Hit**, **full hit**, **damage type**, **health**, **max health**, **absorption**, **hurt time**,
  **damage cooldown**: as `entity-damage.md`, section 0.1, defines them.
- **Effect kind**: an entry of the registry `minecraft:mob_effect`.
- **Effect**: one instance of an effect kind on one living entity. It has an amplifier, a
  duration, three flags, and an optional hidden effect.
- **Amplifier**: an integer from 0 to 255. The player sees it as the level minus 1.
- **Duration**: an integer number of ticks, or the value -1. The value -1 is the **infinite
  duration**.
- **Remaining duration**: the duration of an effect with a finite duration.
- **Ambient**, **visible**, **icon**: the three flags of an effect. A beacon sets ambient. Visible
  controls the particles. Icon controls the icon in the HUD of the affected player.
- **Hidden effect**: an effect of the same kind with a lower amplifier. It waits below the
  active effect and replaces it when the active effect ends (EFX-ADD-002).
- **Active effects**: the set of effects of one entity. It has at most one effect of each kind.
- **Effect tick**: the step of the base entity tick that ticks the active effects (EFX-TCK-001).
- **Potency**: an f64 factor for the instant effects that an item applies (EFX-ACT-007).
  A potion that a player drinks has the potency 1.0.
- **Undead entity**: an entity of a type in the entity type tag `#minecraft:inverted_healing_and_harm`.
  In the data pack this tag contains `#minecraft:undead`. That tag contains `#minecraft:skeletons`,
  `#minecraft:zombies`, `minecraft:wither` and `minecraft:phantom`.
- **Affected player**: a player that has the effect, as an observer of its own packets.
- **Base entity tick**, **order key**: as `entity-damage.md` defines them.

### 0.2 Provenance

The labels are those of `docs/spec/registry.md`, section 0.3. The rules come from the reference
implementation and from the artefacts of section 0.5. No experiment of section 13 has run yet. Thus
no rule has the label `black_box_confirmed`. Each rule states its kind and its confidence.

### 0.3 ID prefix

Each rule has an ID with the prefix `EFX`. The second part names the section.

| Prefix | Subject |
|--------|---------|
| `EFX-INS` | the effect and the set of active effects |
| `EFX-ADD` | adding an effect and the merge |
| `EFX-TCK` | the tick, the duration and the expiry |
| `EFX-REM` | the removal of an effect |
| `EFX-ACT` | what each effect does |
| `EFX-ATT` | the attribute modifiers |
| `EFX-NET` | the packets that a client sees |
| `EFX-SAV` | the player data |

### 0.4 Scope

This document states:

- the data of one effect and the rule that a living entity has one effect of each kind;
- the add, the merge, the hidden effect, the tick and the end of an effect;
- the removal by expiry, by a clear, by milk and by death;
- the action of these effects: `regeneration`, `poison`, `wither`, `instant_health`,
  `instant_damage`, `hunger`, `saturation`, `absorption`, `health_boost`, `resistance`,
  `fire_resistance`, `water_breathing`, `speed` and `slowness`;
- the other effects that only change an attribute;
- the packets `update_mob_effect` and `remove_mob_effect`, and the entity data that shows effects
  to other players;
- the field `active_effects` of the player file.

Section 10 lists the subjects that are out of scope.

### 0.5 Artefacts cited

- `assets/mcrs/reports/registries.json`, registry `minecraft:mob_effect`: the network id of each
  effect kind (section 5). The registry `minecraft:particle_type` gives the ids 28
  (`entity_effect`), 123 (`raid_omen`) and 124 (`trial_omen`). The registry `minecraft:attribute`
  gives the attribute names.
- `assets/mcrs/reports/packets.json`: `update_mob_effect` (play clientbound 135) and
  `remove_mob_effect` (play clientbound 79).
- The data pack tags of the registry `minecraft:entity_type`: `inverted_healing_and_harm`,
  `undead`, `immune_to_poison`, `immune_to_wither`, `immune_to_regeneration`,
  `immune_to_weakness`, `immune_to_freezing`, `immune_to_infested` and `immune_to_oozing`.
- The damage types `magic`, `indirect_magic` and `wither`, and the damage tags
  `bypasses_armor` and `no_knockback`. The data pack has these.
- The colour and the attribute modifiers of an effect kind are not in the data pack or in the
  reports. The vanilla game holds them as code. Sections 5 and 7 state them as data, so a later
  report can replace them.

## 1. The effect and the set of active effects

**EFX-INS-001** (`source_suggested`, semantic, confidence: high). A living entity has a set of
active effects with at most one effect of each effect kind. The set is empty for a new entity. An
effect has these fields:

| Field | Range | Meaning |
|-------|-------|---------|
| kind | `minecraft:mob_effect` | the effect kind |
| amplifier | 0 to 255 | a value outside the range clamps to the range |
| duration | an integer, or -1 | the remaining ticks, or the infinite duration |
| ambient | boolean | default false |
| visible | boolean | default true |
| icon | boolean | default is the value of visible |
| hidden effect | none, or an effect of the same kind | EFX-ADD-002 |

**EFX-INS-002** (`source_suggested`, semantic, confidence: high). An effect kind can be refused
for an entity type. The server then adds nothing, and the add returns "not changed". The data pack
decides with these entity type tags:

| Effect kind | Tag of the immune types |
|-------------|-------------------------|
| `poison` | `immune_to_poison` |
| `wither` | `immune_to_wither` |
| `regeneration` | `immune_to_regeneration` |
| `weakness` | `immune_to_weakness` |
| `freezing` | `immune_to_freezing` |
| `infested` | `immune_to_infested` |
| `oozing` | `immune_to_oozing` |

Another kind is refused for no entity type. A player is not in these tags with the vanilla data
pack, so a player gets each effect.

## 2. Adding an effect

**EFX-ADD-001** (`source_suggested`, semantic, confidence: high). To add an effect `n` to an
entity, the server does these steps in this order:

1. If EFX-INS-002 refuses `n`, stop. The result is "not changed".
2. If the entity has no effect of the kind of `n`, the active effects get `n`. The attribute
   modifiers of EFX-ATT-001 start. The result is "changed". The affected player gets the packet of
   EFX-NET-002 with the blend flag.
3. Else the merge of EFX-ADD-002 changes the existing effect `e`. If the merge returns "changed",
   the modifiers of `e` restart with the new amplifier and the affected player gets the packet of
   EFX-NET-003.
4. The start action of `n` runs (EFX-ACT-005). It runs in step 2 and in step 3, also when the
   merge returned "not changed". It uses the amplifier of `n`.

**EFX-ADD-002** (`source_suggested`, semantic, confidence: high). The merge of an added effect `n`
into the existing effect `e` of the same kind uses these terms. An effect `a` is **shorter** than
an effect `b` when `a` has a finite duration. In addition, the duration of `a` is less than the
duration of `b`, or `b` has the infinite duration. The merge does these steps in this order:

1. If the amplifier of `n` is more than the amplifier of `e`:
   - if `n` is shorter than `e`, `e` becomes the new hidden effect. The previous hidden effect
     of `e` becomes the hidden effect of that new hidden effect;
   - `e` takes the amplifier and the duration of `n`;
   - the merge is "changed".
2. Else, if `e` is shorter than `n`:
   - if the amplifiers are equal, `e` takes the duration of `n`, and the merge is "changed";
   - else, if `e` has no hidden effect, the hidden effect of `e` becomes a copy of `n`;
   - else the merge of `n` into the hidden effect of `e` applies, with these same steps. Its result
     does not change the result of this merge.
3. Else nothing changes in the amplifier and the duration.
4. If `n` is not ambient and `e` is ambient, or the merge is "changed", `e` takes the ambient flag
   of `n`. The merge is then "changed".
5. If the visible flag of `n` is not equal to that of `e`, `e` takes it, and the merge is "changed".
6. If the icon flag of `n` is not equal to that of `e`, `e` takes it, and the merge is "changed".

Thus a stronger effect replaces a weaker one. A longer effect of the same amplifier extends the
duration. A weaker and longer effect waits as the hidden effect. A weaker and shorter effect does
nothing, and a repeated effect of the same amplifier and a shorter duration changes only the flags.
The amplifiers never add, and the durations never add.

**EFX-ADD-003** (`source_suggested`, semantic, confidence: high). An effect with the duration 0 is
valid. The next effect tick removes it (EFX-TCK-002). It acts in no tick, because the tick of
EFX-TCK-002 ends it first.

## 3. The tick

**EFX-TCK-001** (`source_suggested`, semantic, confidence: high). A living entity ticks its active
effects once in each base entity tick of the server. The tick runs after the decrease of the hurt
time and of the damage cooldown (`entity-damage.md`, DMG-HIT-006). The tick runs also for an entity in the dying
state. The order of the effects inside one effect tick is not specified. A parity test must not
compare it.

**EFX-TCK-002** (`source_suggested`, semantic, confidence: high). The effect tick does these steps
for each effect `e` of the entity:

1. If `e` has a finite duration of 0 or less, remove `e` (EFX-REM-001). Stop.
2. Let `t` be the entity tick count when `e` has the infinite duration. Else let `t` be the duration
   of `e` that remains.
3. If the effect kind acts in the tick `t` (section 6), the action runs. If the action returns
   "end", remove `e` and stop. Section 6 says which action ends its effect.
4. If `e` has a finite duration, decrease the duration by 1. A hidden effect of `e` also decreases
   its duration by 1 when it has a finite duration. This happens at all depths.
5. If the duration of `e` is 0 and `e` has a hidden effect, do these actions. `e` takes the
   amplifier, the duration and the three flags of that hidden effect. Then `e` gets the hidden effect of the
   hidden effect. The modifiers of `e` restart with the new amplifier. The affected player gets
   the packet of EFX-NET-003.
6. If `e` has a finite duration of 0 or less, or the duration has become 0 without a hidden effect,
   remove `e` (EFX-REM-001).
7. Else, if the duration of `e` is a multiple of 600, the affected player gets the packet of
   EFX-NET-003. The modifiers do not restart.

An effect with the duration `d` thus stays eligible to act in the ticks where the duration that
remains is `d`, `d - 1` to 1. It acts only in the ticks that its action selects (section 6). It
ends in the tick where the duration that remains is 1, or earlier when its action ends it. An
effect with the infinite duration never ends by this rule. Step 2 makes the periodic actions of an infinite effect follow the age of the entity. They do not
follow the age of the effect.

## 4. Removal

**EFX-REM-001** (`source_suggested`, semantic, confidence: high). To remove an effect `e`:

1. The active effects lose `e`, with its hidden effects.
2. The modifiers of `e` end (EFX-ATT-001). A change of an attribute value then applies at once. This
   includes the clamps of EFX-ACT-005 and EFX-ACT-006.
3. The affected player gets the packet of EFX-NET-004.

A hidden effect of `e` ends with `e`.

**EFX-REM-002** (`source_suggested`, semantic, confidence: high). These causes remove an effect, in
the same way as EFX-REM-001:

| Cause | Result |
|-------|--------|
| the end of the duration | EFX-TCK-002 |
| the command `effect clear` | the named effect, or each effect |
| the consume effect `clear_all_effects` of the data component `consumable` | each effect. The vanilla milk bucket has it |
| the end of the effect `absorption` | EFX-ACT-005 |

A clear of all effects returns "changed" only when the entity had at least one effect. The server
sends one packet of EFX-NET-004 for each removed effect.

**EFX-REM-003** (`source_suggested`, semantic, confidence: medium). When a living entity is
removed with the reason "killed" or "discarded", the server clears its active effects. It makes no
attribute change and sends no packet. The removal of a mob follows its death after 20 ticks
(`entity-damage.md`, DMG-DTH-003). The death of a living entity does not remove its effects before
that removal. A player is not removed at its death. The respawn of a player after a death gives
the new player no effects (EFX-NET-007). A player that leaves the server keeps its effects in the
player file (EFX-SAV-001). The effects do not tick while the player is away.

## 5. The effect kinds

**EFX-INS-003** (`source_suggested`, wire, confidence: high). The table lists each effect kind
with its network id and its colour. The network id is the VarInt of the packets in section 8. The
registry report `minecraft:mob_effect` gives it. The colour is the RGB value that the particle of
EFX-NET-005 uses.

| Id | Effect kind | Colour |
|----|-------------|--------|
| 0 | `speed` | `33EBFF` |
| 1 | `slowness` | `8BAFE0` |
| 2 | `haste` | `D9C043` |
| 3 | `mining_fatigue` | `4A4217` |
| 4 | `strength` | `FFC700` |
| 5 | `instant_health` | `F82423` |
| 6 | `instant_damage` | `A9656A` |
| 7 | `jump_boost` | `FDFF84` |
| 8 | `nausea` | `551D4A` |
| 9 | `regeneration` | `CD5CAB` |
| 10 | `resistance` | `9146F0` |
| 11 | `fire_resistance` | `FF9900` |
| 12 | `water_breathing` | `98DAC0` |
| 13 | `invisibility` | `F6F6F6` |
| 14 | `blindness` | `1F1F23` |
| 15 | `night_vision` | `C2FF66` |
| 16 | `hunger` | `587653` |
| 17 | `weakness` | `484D48` |
| 18 | `poison` | `87A363` |
| 19 | `wither` | `736156` |
| 20 | `health_boost` | `F87D23` |
| 21 | `absorption` | `2552A5` |
| 22 | `saturation` | `F82423` |
| 23 | `glowing` | `94A061` |
| 24 | `levitation` | `CEFFFF` |
| 25 | `luck` | `59C106` |
| 26 | `unluck` | `C0A44D` |
| 27 | `slow_falling` | `F3CFB9` |
| 28 | `conduit_power` | `1DC2D1` |
| 29 | `dolphins_grace` | `88A3BE` |
| 30 | `bad_omen` | `0B6138` |
| 31 | `hero_of_the_village` | `44FF44` |
| 32 | `darkness` | `292721` |
| 33 | `trial_omen` | `16A6A6` |
| 34 | `raid_omen` | `DE4058` |
| 35 | `wind_charged` | `BDC9FF` |
| 36 | `weaving` | `78695A` |
| 37 | `oozing` | `99FFA3` |
| 38 | `infested` | `8C9B8C` |
| 39 | `breath_of_the_nautilus` | `00FFEE` |
| 40 | `freezing` | `9EEEEE` |

This document states the action of 14 kinds and of the kinds that act only through a modifier
(EFX-ATT-002). The other kinds have an effect on the client or on other systems. Section 10 lists
them.

## 6. What each effect does

In this section, `a` is the amplifier of the effect, and `t` is the value of EFX-TCK-002 step 2.
All shifts of this section are integer shifts to the right or to the left. They are specified for
`a` up to 28 for a shift to the left. They are specified for `a` up to 31 for a shift to the right.
See U1.

**EFX-ACT-001** (`source_suggested`, semantic, confidence: high). The effect `regeneration` acts
in the tick `t` when `i = 50 >> a` is 0, or when `t` is a multiple of `i`. The action heals the
entity by 1 when its health is less than its max health. The entity is a mob or a player. The
heal does not use the hit procedure and does not use the damage cooldown. A dying entity does not
heal (`entity-damage.md`, DMG-ATT-001). Thus the intervals are 50, 25, 12, 6, 3 and 1 ticks for the amplifiers 0 to 5. The effect acts in each tick for the amplifier 6 and more.

**EFX-ACT-002** (`source_suggested`, semantic, confidence: high). The effect `poison` acts in the
tick `t` when `i = 25 >> a` is 0, or when `t` is a multiple of `i`. The action does a hit of 1 with
the damage type `magic` when the health of the entity is more than 1. The hit has no causing
entity. The entity is a mob or a player. Thus poison never kills, and a mob at 1 health or less is
not hit. The intervals are 25, 12, 6, 3 and 1 ticks for the amplifiers 0 to 4, and each tick for
the amplifier 5 and more. The damage cooldown applies (`entity-damage.md`, DMG-HIT-003). Thus a
hit of 1 inside the cooldown is rejected. A full hit occurs at the first scheduled action after the
cooldown. For the amplifier 2 the interval is 6 ticks, so after a full hit the action at 6 ticks
is rejected and the next full hit comes at 12 ticks.

**EFX-ACT-003** (`source_suggested`, semantic, confidence: high). The effect `wither` acts in the
tick `t` when `i = 40 >> a` is 0, or when `t` is a multiple of `i`. The action does a hit of 1 with
the damage type `wither` and no causing entity. This hit can kill. The intervals are 40, 20, 10, 5, 2 and 1 ticks for the amplifiers 0 to 5. The effect acts in each tick for the amplifier 6 and more. The damage
cooldown applies, as for EFX-ACT-002. The damage types `magic` and `wither` have the tags
`bypasses_armor` and `no_knockback`. The effect `resistance` reduces the damage of both
(`entity-damage.md`, DMG-RED-002).

**EFX-ACT-004** (`source_suggested`, semantic, confidence: high). The effects `instant_health` and
`instant_damage` act in each tick with `t` of 1 or more. The action depends on the pair of the
kind and the entity:

| Kind | Entity is not undead | Entity is undead |
|------|----------------------|------------------|
| `instant_health` | heal by `4 << a` | hit of `6 << a`, type `magic` |
| `instant_damage` | hit of `6 << a`, type `magic` | heal by `4 << a` |

The heal changes the health with the clamp of DMG-ATT-001 and only when the health is more than 0.
The hit has no causing entity and uses the hit procedure. An effect of these kinds with the
duration `d` acts in `d` ticks. The command `effect give` with a number of seconds `s` for an
instant effect gives the duration `s` in ticks, so the effect acts `s` times. The effect with the
default duration acts once.

**EFX-ACT-005** (`source_suggested`, semantic, confidence: high). The effect `absorption` has these
actions:

1. At each add (EFX-ADD-001 step 4), the absorption of the entity becomes
   `max(absorption, f32(4 × (a + 1)))`, with `a` the amplifier of the added effect. The new value
   stays in the range of DMG-RED-004.
2. In each tick, the effect acts. The action returns "end" when the absorption is 0 or less. Then
   EFX-TCK-002 step 3 removes the effect. A hit that takes all the absorption ends the effect in
   the next effect tick.
3. The effect has the modifier of EFX-ATT-002. When the modifier ends, `max_absorption` falls, and
   the absorption clamps to the new `max_absorption`. Thus a base `max_absorption` of 0 makes the
   absorption 0 when the effect ends.

Thus a repeated add of the same effect refills the absorption to the level of the added amplifier.

**EFX-ACT-006** (`source_suggested`, semantic, confidence: high). The effect `health_boost` has no
action. It has the modifier of EFX-ATT-002. A start of the effect does not heal the entity. When
the effect ends, `max_health` falls, and a health that is more than the new max health clamps to it
(DMG-ATT-001).

**EFX-ACT-007** (`source_suggested`, semantic, confidence: medium). An item can apply an instant
effect with a potency `p` to an entity, for example a splash potion. The server does not add
an effect then. The amount is the integer `trunc(p × f64(4 << a) + 0.5)` for a heal. It is
`trunc(p × f64(6 << a) + 0.5)` for a hit. The undead inversion of EFX-ACT-004 applies. The hit has
the type `indirect_magic` with the source entity and its owner when the item has a source entity.
Else the hit has the type `magic`. The items are out of scope. This rule fixes the numbers only.

**EFX-ACT-008** (`source_suggested`, semantic, confidence: high). The effect `hunger` acts in each
tick. The action adds `0.005 × (a + 1)` to the exhaustion of a player, as an f32. The action does
nothing for a player whose abilities make it invulnerable, for example in creative mode. The action
does nothing for a mob. `hunger.md` states the exhaustion (HNG-EXH-007). The addition keeps the
exhaustion at 40 or less.

**EFX-ACT-009** (`source_suggested`, semantic, confidence: high). The effect `saturation` acts in
each tick with `t` of 1 or more, as an instant effect. The action does nothing for a mob. For a
player it changes the food level `f` and the saturation `s`:

1. `f = clamp(f + (a + 1), 0, 20)`.
2. `s = clamp(s + f32(a + 1) × 1.0 × 2.0, 0, f32(f))`, in f32.

**EFX-ACT-010** (`source_suggested`, semantic, confidence: high). The effect `resistance` has no
action. `entity-damage.md`, DMG-RED-002, states its reduction of a hit. The effect `fire_resistance`
has no action. `entity-damage.md`, DMG-HIT-001 step 7, states that a hit of a type with the tag
`is_fire` is rejected while the entity has the effect. The effect does not stop ignition. Fire
and lava still ignite the entity, and the rejected hits burn no health (ENV-FIR-008).

**EFX-ACT-011** (`source_suggested`, semantic, confidence: medium). While the eyes of an entity are
in water, and not in a bubble column, and the entity has the effect `water_breathing`:

- the air supply of the entity does not decrease;
- the air supply increases by 4 in each tick, up to the max air supply;
- the entity takes no drowning damage.

The effects `conduit_power` and `breath_of_the_nautilus` also stop the decrease. The refill of the
air supply that `breath_of_the_nautilus` alone allows is not stated here. The drowning damage and
the air supply are out of scope.

## 7. Attribute modifiers

**EFX-ATT-001** (`source_suggested`, wire, confidence: high). An effect that has a modifier in
EFX-ATT-002 gives the entity one modifier on the named attribute. The modifier is permanent. Its
identifier is `minecraft:<name>`, and the table gives `<name>`, for example `minecraft:effect.speed`. The amount of the modifier
is `base amount × f64(a + 1)`, in f64. When the effect starts, restarts or changes the amplifier,
the server first removes the modifier of that identifier, and then adds the new modifier. When the
effect ends, the server removes the modifier. A modifier is part of the attribute data of the
entity, so a player file keeps it (EFX-SAV-001) and the attribute packet sends it.

**EFX-ATT-002** (`source_suggested`, wire, confidence: high). The effect kinds with modifiers are:

| Effect kind | Attribute | Name | Base amount | Operation |
|-------------|-----------|------|-------------|-----------|
| `speed` | `movement_speed` | `effect.speed` | 0.20000000298023224 | `add_multiplied_total` |
| `slowness` | `movement_speed` | `effect.slowness` | -0.15000000596046448 | `add_multiplied_total` |
| `haste` | `attack_speed` | `effect.haste` | 0.10000000149011612 | `add_multiplied_total` |
| `mining_fatigue` | `attack_speed` | `effect.mining_fatigue` | -0.10000000149011612 | `add_multiplied_total` |
| `strength` | `attack_damage` | `effect.strength` | 3.0 | `add_value` |
| `weakness` | `attack_damage` | `effect.weakness` | -4.0 | `add_value` |
| `jump_boost` | `safe_fall_distance` | `effect.jump_boost` | 1.0 | `add_value` |
| `health_boost` | `max_health` | `effect.health_boost` | 4.0 | `add_value` |
| `absorption` | `max_absorption` | `effect.absorption` | 4.0 | `add_value` |
| `luck` | `luck` | `effect.luck` | 1.0 | `add_value` |
| `unluck` | `luck` | `effect.unluck` | -1.0 | `add_value` |
| `invisibility` | `waypoint_transmit_range` | `effect.waypoint_transmit_range_hide` | -1.0 | `add_multiplied_total` |

The base amounts of the first four rows are the f32 values 0.2, -0.15, 0.1 and -0.1, widened to
f64. A client sees them in the attribute packet, so the server must send these f64 values. The
amount for the amplifier `a` is the f64 product of the value and `a + 1`.

**EFX-ATT-003** (`source_suggested`, semantic, confidence: high). The value of an attribute is
computed in f64, in this order:

1. `s = base + the sum of the add_value amounts`.
2. `r = s + the sum of (s × amount) for each add_multiplied_base modifier`.
3. For each `add_multiplied_total` modifier: `r = r × (1 + amount)`.
4. The result clamps to the range of the attribute.

These are the ranges:

- `movement_speed` 0 to 1024, `attack_speed` 0 to 1024, `attack_damage` 0 to 2048;
- `max_health` 1 to 1024, `max_absorption` 0 to 2048;
- `safe_fall_distance` -1024 to 1024, `luck` -1024 to 1024;
- `waypoint_transmit_range` 0 to 60000000.

Thus `slowness` with `a` of 6 or more
gives a `movement_speed` of 0, because `1 - 0.15 × 7` is below 0. The order of the multipliers
does not change the product, but it can change the last f64 bits. A parity test compares the value
with a tolerance of one f64 ulp. This rule is the full modifier rule that `entity-damage.md`,
DMG-ATT-002, leaves open for effects.

## 8. Packets that a client sees

**EFX-NET-001** (`source_suggested`, wire, confidence: high). The packet `update_mob_effect` (play
clientbound 135) has these fields in this order:

| Field | Type | Value |
|-------|------|-------|
| entity | VarInt | the network id of the entity |
| effect | VarInt | the network id of the effect kind (EFX-INS-003) |
| amplifier | VarInt | the amplifier |
| duration | VarInt | the remaining duration, or -1 for the infinite duration |
| flags | Byte | the sum of the flags below |

The flag bits are `0x01` ambient, `0x02` visible, `0x04` icon and `0x08` blend. The blend flag
tells the client to fade the effect in. The HUD of the client shows the icon only when the icon
flag is set. The duration -1 shows as an infinite effect.

**EFX-NET-002** (`source_suggested`, wire, confidence: high). When a player gets a new effect (EFX-ADD-001 step 2), the server sends one `update_mob_effect` to that player. The packet has the entity id of
the player and the blend flag set. Other players get no packet. They learn the effect through the
entity data of EFX-NET-005.

**EFX-NET-003** (`source_suggested`, wire, confidence: high). The server sends one
`update_mob_effect` without the blend flag to the affected player when:

- a merge changed the effect (EFX-ADD-001 step 3);
- a hidden effect became the active effect (EFX-TCK-002 step 5);
- the duration of the effect is a multiple of 600 after a tick (EFX-TCK-002 step 7).

The packet holds the fields of the effect after the change. A merge that returns "not changed"
sends no packet. A change of a hidden effect that stays hidden sends no packet.

**EFX-NET-004** (`source_suggested`, wire, confidence: high). The packet `remove_mob_effect` (play
clientbound 79) has two fields. The field entity is a VarInt, the network id of the entity. The
field effect is a VarInt, the network id of the effect kind. The server sends one to the affected player for each effect that
EFX-REM-001 or EFX-REM-002 removes. EFX-REM-003 sends none.

**EFX-NET-005** (`source_suggested`, wire, confidence: medium). Other viewers see the effects of an
entity in its entity data. They see two flags of the entity flags (entity data index 0,
serializer 0):

| Entity data | Index | Serializer | Value |
|-------------|-------|------------|-------|
| effect particles | 10 | 17 (list of particles) | one particle for each visible effect |
| all ambient | 11 | 8 (boolean) | true when each visible effect is ambient |

- A particle of an effect kind other than `trial_omen` and `raid_omen` is the particle type 28
  (`entity_effect`) with a colour option. The option is an ARGB integer. Its RGB part is the colour
  of EFX-INS-003. Its alpha is 255, or 38 when the effect is ambient.
- A particle of `trial_omen` is the particle type 124 with no option. A particle of `raid_omen` is
  the particle type 123 with no option.
- The list is empty when the entity has no effects. The list has no particle for an effect that
  is not visible. The order of the list is not specified.
- The value "all ambient" is true when the entity has no visible effect that is not ambient.
- The flag bit `0x20` (invisible) of the entity flags is set while the entity has the effect
  `invisibility`. The flag bit `0x40` (glowing) is set while the entity has the effect `glowing`.
- The server computes these values when an effect changes, before the next entity data packet of
  that tick (`entity-tracking.md`).

**EFX-NET-006** (`source_suggested`, wire, confidence: medium). A player that rides a living entity
gets the effects of that entity:

- When the player starts to ride, the server sends one `update_mob_effect` for each effect of the
  vehicle. The packet has the entity id of the vehicle and no blend flag.
- While the player rides, each add, change and removal of an effect of the vehicle sends the packet
  of EFX-NET-001 or EFX-NET-004 to the player. The packet has the entity id of the vehicle. The
  add has no blend flag.
- When the player stops to ride, the server sends one `remove_mob_effect` for each effect of the
  vehicle.

**EFX-NET-007** (`source_suggested`, wire, confidence: medium). The server sends one
`update_mob_effect` without the blend flag for each effect of a player:

- when the player joins, after the login packets and before the post effects;
- after the `respawn` packet of a respawn, before the post effects, and
- after the `respawn` packet of a dimension change, as `dimension-change.md`, DIM-CHG-003 step 17
  states.

A player that respawns after a death has no effects, so it gets no packet and the client drops its
effects. Extra packets before the `respawn` packet are not specified.

## 9. Player data

**EFX-SAV-001** (`source_suggested`, semantic, confidence: high). The player file has the list
`active_effects` in the compound of the player. The server omits the list when the player has no
effects. An entry is a compound with these fields:

| Field | NBT type | Default | Meaning |
|-------|----------|---------|---------|
| `id` | string | none | the identifier of the effect kind |
| `amplifier` | byte, read as 0 to 255 | 0 | the amplifier |
| `duration` | int | 0 | the remaining duration, or -1 |
| `ambient` | byte | 0 | the ambient flag |
| `show_particles` | byte | 1 | the visible flag |
| `show_icon` | byte | the value of `show_particles` | the icon flag |
| `hidden_effect` | compound | none | the hidden effect, with the same fields except `id` |

An entry with an unknown `id` or an unreadable field is an error, and the server does not load the
effect. The loaded entries replace the active effects. When two entries have the same `id`, the
later entry wins. The load does not run the add steps of EFX-ADD-001. It sends no blend packet. It starts no
modifier. It does not run the start action. The attribute modifiers of the effects
come from the list `attributes` of the same file (EFX-ATT-001). The packets of EFX-NET-007 send the
loaded effects when the player joins. The `AbsorptionAmount` of the file keeps the absorption.

**EFX-SAV-002** (`source_suggested`, semantic, confidence: high). The time that the server is off does not
change the duration of a saved effect. The effects tick only while the entity is
in a loaded level. A player that leaves with the duration `d` rejoins with `d`.

## 10. Out of scope

| Subject | Reason |
|---------|--------|
| the effects of a beacon, a conduit, a potion item, an arrow, a suspicious stew, a golden apple and a totem of undying | items and blocks that give effects are other domains |
| the commands `effect give` and `effect clear`, except the instant duration in EFX-ACT-004 | `command-effect.md` does not exist yet |
| the effects `nausea`, `blindness`, `darkness`, `night_vision`, `glowing`, `levitation`, `slow_falling`, `jump_boost` motion, `haste` mining speed, `conduit_power`, `dolphins_grace`, `bad_omen`, `trial_omen`, `raid_omen`, `hero_of_the_village`, `wind_charged`, `weaving`, `oozing`, `infested`, `freezing` | these effects act in the client or in a later domain; this document only keeps the effect and its modifier |
| the natural regeneration, the food level and the exhaustion rules | `hunger.md` states them |
| the advancement trigger that fires when the effects of a player change | advancements are another domain |
| the sound that an effect plays when it is added | no observer in G2 |
| the blend of the effect in the client | it is client rendering |

## 11. Intentional deviations

None. Players use some behaviour that differs from the expected one. The rules state it as
correct. This behaviour is the refill of EFX-ACT-005 and the ambient rule of EFX-ADD-002 step 4.
It is also the instant effect that acts each tick of its duration in EFX-ACT-004.

## 12. Unresolved

- **U1.** The reference computes the intervals and the instant amounts with integer shifts. The
  shift count wraps after 31, and the left shift overflows from the amplifier 29. Thus
  `poison` of the amplifier 32 acts at the interval of the amplifier 0. And `instant_damage` of the
  amplifier 29 gives a negative or a wrong amount. A player meets this only with a command or a
  data pack, because the amplifier of an item is small. The owner must decide: keep the wrap, or state
  that the interval is each tick from the amplifier 5, and that the amount saturates.
- **U2.** The reference keeps the effects in a hash map. The order of the effects in one tick, and
  the order of the particle list, depend on the hash. EFX-TCK-001 states the order as not specified.
  No observer sees it except through the order of a heal and a hit in one tick.
- **U3.** The respawn that keeps all player data (the return from the End) adds the effects of the old
  player to the new player. This occurs before the `respawn` packet. The client then gets blend packets before
  `respawn` and plain packets after it. EFX-NET-007 states only the packets after `respawn`.
  Experiment E6 decides if the first packets matter to the client.
- **U4.** No artefact gives the colour, the category and the modifiers of an effect kind. Sections 5
  and 7 state the values from the reference. A later report of the game can replace them.
- **U5.** The load of an effect does not start its modifier. A hand-edited file can have an effect and no modifier in `attributes`. The effect then has no
  attribute change until the effect changes or a hidden effect takes over. The owner must decide if the load must start the modifiers.

## 13. Experiments

All experiments need a vanilla server of `26.4-snapshot-7`. None has run yet. Use
`tools/vanilla-server` and the client recorder of `tools/mcrs_minecraft_client_recorder` with the
input agent of `tools/vanilla-client`. A rule keeps the label `source_suggested` until its
experiment has run.

| Id | Steps | Rules |
|----|-------|-------|
| E1 | Freeze the tick. Give a pig `poison` 0 of 100 ticks, then `poison` 1 of 200 ticks, then `poison` 0 of 400 ticks, then `poison` 2 of 50 ticks. Read `active_effects` with `data get` after each add. | EFX-ADD-001, EFX-ADD-002 |
| E2 | Freeze the tick. Give a pig `regeneration` 0 of 100 ticks. Step 100 ticks. Read the `Health` after each tick, with the health below the max. Repeat with the amplifiers 1 and 6, and with the infinite duration. | EFX-TCK-002, EFX-ACT-001 |
| E3 | Give a zombie and a pig `instant_health` and `instant_damage` of the amplifier 0 and 1. Read the `Health` before and after one tick. | EFX-ACT-004 |
| E4 | Give a pig `absorption` 1. Hit it for 100 with `/damage`. Step 2 ticks. Read `active_effects` and `AbsorptionAmount`. Repeat after you give `absorption` 0 for 600 ticks and let it end. | EFX-ACT-005 |
| E5 | With the client recorder, give the probe player `speed` 1 of 1300 ticks, then `speed` 3 of 100 ticks. Step 1300 ticks. Record the packets 135, 79 and the attribute packet. | EFX-ADD-002, EFX-TCK-002, EFX-NET-001 to EFX-NET-004, EFX-ATT-002 |
| E6 | With the recorder, join with `speed` saved in the player file. Then use an end portal to return from the End. Record packets 135 and 84. | EFX-NET-007, EFX-SAV-001, U3 |
| E7 | Run `/attribute <pig> minecraft:movement_speed get` after `slowness` of the amplifiers 0, 5 and 6. Record the value and the modifier amounts. | EFX-ATT-002, EFX-ATT-003 |
| E8 | Give a pig `invisibility` and `glowing`, and a visible and an ambient `speed`. Record the entity data of the pig for a second player. | EFX-NET-005 |
| E9 | Give a horse `speed`. Let the probe player mount the horse, then dismount. Record the packets 135 and 79. | EFX-NET-006 |
