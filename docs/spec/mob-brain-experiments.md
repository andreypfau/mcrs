# Mob brain: experiments

This file contains the experiments for [mob-brain.md](mob-brain.md).
Its target version, terms, rules and evidence conventions apply here.

## 12. Experiments

### 12.1 Experiments with a vanilla server

The status under each experiment separates captured observations from proposals. Use the version from `assets/minecraft/version.json`:
`26.4-snapshot-7`, world version `5127`. Record runtime and save metadata before the first run.

Use a flat world, no unrelated mobs and `pause-when-empty-seconds=-1`.
Connect a player only when the procedure requires one.
Execute player commands as that player. Keep it in the Overworld unless a step names another
dimension. Commands below use a flat platform at y=64, away from spawn interference.

Remove subjects and restore the test area between independent experiments. E-B4 and E-B8 can reuse E-B2.
Run this setup in each world. Remove a test's tagged entities before a repeat.
Do not edit a subject's NBT during a measured interval: reload replaces transient state and
chooses fresh sensor phases. `/tp` within a dimension does not reload the brain.

```mcfunction
/tick freeze
/gamerule spawn_mobs false
/forceload add -32 -32 32 32
/fill -16 63 -16 16 63 16 minecraft:stone
/time of minecraft:overworld pause
/time of minecraft:overworld set 1000
```

After each `/tick step 1`, wait for that step to finish. Then read the data.
Count step completion, not wall time. Use `Brain` for saved memories; read `Pos`, `Motion` and
`Rotation` for motion. Transient memories and active activities do not appear in saved NBT.

A negative saved-NBT result must not count as proof that such a transient value is absent.
Use at least 20 fresh subjects for a sensor probe. Use at least 200 for a distribution estimate.

### E-B1: Nearest-player scan and an observable player reaction

Status: proposal; captured trial is inconclusive. Rules: BRAIN-VAN-010, BRAIN-VAN-013, BRAIN-VAN-009.
Use one survival player with no disguise, invisibility or other targets. Keep the creaking within
12 blocks, on a clear route. The probe uses its response to a watched player, then movement toward
that player after the player turns away. Its activation sound/eyes provide an additional visible
scan witness. The player must remain connected; a spectator is not a valid positive subject.

```mcfunction
/gamemode survival @s
/effect clear @s
/tp @s 0.5 64 6.5 facing 0.5 65 0.5
/summon minecraft:creaking 0.5 64 0.5 {Tags:["brain_players"],PersistenceRequired:1b}
/data get entity @e[tag=brain_players,limit=1] Brain
/data get entity @e[tag=brain_players,limit=1] Pos
/tick step 1
/data get entity @e[tag=brain_players,limit=1] Pos
```

Repeat the last step/read pair through tick 20. Record the activation reaction each tick.
The nearest-player scan must occur at f in 1..19. Observation occurs before the next brain scan,
so the creaking's reaction to that result must begin by f+1, at most tick 20.
At tick 20 run this movement probe without an entity-data edit:

```mcfunction
/tp @s 0.5 64 6.5 0 0
/tick step 1
/data get entity @e[tag=brain_players,limit=1] Pos
/tick step 1
/data get entity @e[tag=brain_players,limit=1] Pos
/data get entity @e[tag=brain_players,limit=1] Brain
```

Movement toward the player must become possible after the observation restriction ends;
allow the navigation-before-brain delay of BRAIN-VAN-009. Record the first displacement and
continue one tick at a time if path establishment adds delay. The scan deadline is 19; the
movement alone does not establish its exact tick. No reaction in an inert subject is inconclusive.

A reaction by tick 20, followed by pursuit, is the positive transient-memory probe.
Repeat with the player in spectator mode as a negative control; it must not cause activation.
NBT must omit nearest-player memories even in a successful positive run.

#### Captured results

Date: 2026-11-11. Version and world: section 12.3.
The repository's headless join test connected `mcrs_test`, but failed after 121.92 seconds without completion.
A temporary protocol client then connected `brain_probe`, confirmed teleports and answered keep-alives throughout the probes.
No repository client code changed. The player had survival mode, no effects and an empty hand.

```text
> gamemode survival brain_probe
Set brain_probe's game mode to Survival Mode
> tp brain_probe 8.5 -60 14.5 facing 8.5 -59 5.5
> summon minecraft:creaking 4.5 -60 3.5 {Tags:["creak","creak0"],PersistenceRequired:1b}
Summoned new Creaking
> data get entity @e[tag=creak0,limit=1] Pos
Creaking has the following entity data: [4.5d, -60.0d, 3.5d]
```

The trial used 20 creakings in a grid, all within 12 blocks.
It read each position and brain after every completed tick through update 20.
All saved brains omitted player memories; subject 0 still had this reply at update 20:

```text
Creaking has the following entity data: {memories: {}}
```

The player then turned away. One-tick position reads continued for 40 updates.
First displacement after the turn occurred at these updates, relative to each subject's original position:

```text
1: 8 subjects; 6: 1; 7: 1; 15: 1; 18: 1; 21: 1; 24: 1; 25: 1; 29: 1; 35: 2; none: 2.
```

Eight subjects already moved during the watched interval. The raw subject-0 end position was:

```text
Creaking has the following entity data: [4.608310040645455d, -60.0d, 3.6083100406454545d]
```

The negative control used 20 fresh creakings and the same player in spectator mode.
Fifteen subjects also displaced during its 40 updates. Five remained stationary.
Neither trial captured eyes, activation sound or another unique activation signal.

Expected: a player-specific activation witness by update 20, then pursuit after the turn.
Observed: motion in both modes, without a unique scan witness.
Motion alone cannot separate ambient movement from player reaction. The probe remains proposal under U10.
No nearest-player memory or exact erasure claim gains confirmed provenance from this trial.

### E-B2: Injected expiry, zero boundary, freeze and AI suspension

Status: captured. Rules: BRAIN-VAN-003 through BRAIN-VAN-007.
No prey or other axolotl must be nearby. Put the subject in water.

```mcfunction
/fill -2 64 -2 2 66 2 minecraft:water
/summon minecraft:axolotl 0.5 65 0.5 {Tags:["brain_ttl"],NoAI:1b,Brain:{memories:{"minecraft:has_hunting_cooldown":{value:1b,ttl:2L}}}}
/data get entity @e[tag=brain_ttl,limit=1] Brain
/tick step 5
/data get entity @e[tag=brain_ttl,limit=1] Brain
/data merge entity @e[tag=brain_ttl,limit=1] {NoAI:0b}
/data get entity @e[tag=brain_ttl,limit=1] Brain
/tick step 1
/data get entity @e[tag=brain_ttl,limit=1] Brain
/tick step 1
/data get entity @e[tag=brain_ttl,limit=1] Brain
/tick step 1
/data get entity @e[tag=brain_ttl,limit=1] Brain
```

The first reads must show 2L. With AI enabled, eligible steps 1, 2 and 3 must show 1L, 0L and
absence respectively. Thus the first post-update save is N-1, not N. Frozen wall time between
commands must not alter it.

Repeat with `ttl:0L`, `ttl:-5L` and `ttl:9223372036854775807L`.
The first two must disappear at the first eligible update. The sentinel must save without `ttl`.
Do not refresh this injected value through a hunt during the measurement.

#### Captured results

Date: 2026-11-11. Version and world: section 12.3.
Both `/summon` NBT and `/data merge` produced the table below.
The subject stayed in water, without prey. It started with `NoAI:1b`.

Commands for N=1, repeated with 5, 40, 0, -5 and 9223372036854775807:

```text
> summon minecraft:axolotl 8 -59 8 {Tags:["ttl"],NoAI:1b,Brain:{memories:{"minecraft:has_hunting_cooldown":{value:1b,ttl:1L}}}}
Summoned new Axolotl
> data get entity @e[tag=ttl,limit=1] Brain
Axolotl has the following entity data: {memories: {"minecraft:has_hunting_cooldown": {value: 1b, ttl: 1L}}}
> data merge entity @e[tag=ttl,limit=1] {NoAI:0b}
Modified entity data of Axolotl
> tick step 1
Stepping 1 tick(s)
> data get entity @e[tag=ttl,limit=1] Brain
Axolotl has the following entity data: {memories: {"minecraft:has_hunting_cooldown": {value: 1b, ttl: 0L}}}
> tick step 1
Stepping 1 tick(s)
> data get entity @e[tag=ttl,limit=1] Brain
Axolotl has the following entity data: {memories: {}}
```

The merge trial first summoned the same subject without `Brain`, then used this command:

```text
> data merge entity @e[tag=ttl,limit=1] {Brain:{memories:{"minecraft:has_hunting_cooldown":{value:1b,ttl:40L}}}}
Modified entity data of Axolotl
```

Every initial read, a second read after 0.2 seconds frozen, and a read after five `NoAI` steps retained N.
Update 0 below is the read immediately after AI was enabled. A dash means no further read.
Each entry was observed independently through both input paths.

| Completed eligible updates | N=1 | N=5 |
| - | - | - |
| 0 | 1L | 5L |
| 1 | 0L | 4L |
| 2 | absent | 3L |
| 3 | - | 2L |
| 4 | - | 1L |
| 5 | - | 0L |
| 6 | - | absent |

| Completed eligible updates | N=40 |
| - | - |
| 0 | 40L |
| 1 | 39L |
| 2 | 38L |
| 39 | 1L |
| 40 | 0L |
| 41 | absent |

Between updates 2 and 39, the value decreased by 1 at every update, without a gap.

For both paths, 0L and -5L remained present until the first eligible update, then disappeared.
The sentinel omitted `ttl` immediately and after two eligible updates:

```text
Axolotl has the following entity data: {memories: {"minecraft:has_hunting_cooldown": {value: 1b}}}
```

Expected and observed: the first post-update expiry is N-1; zero remains present for one update.
Absence begins at update N+1 after injection. Freeze, reads and `NoAI` do not consume expiry.
BRAIN-VAN-004 is consistent with both input paths. BRAIN-VAN-005 through BRAIN-VAN-007 gain the stated partial evidence.

One merge-40 interval overlapped an unrelated cleanup and was rejected. An isolated repeat supplied the merge-40 column.

### E-B3: Schedule change with sleep as the observable

Status: captured. Rules: BRAIN-VAN-025, BRAIN-VAN-026, BRAIN-VAN-033.
The observation is NBT `sleeping_pos`, not the active activity id.

Start with a fresh subject and an unoccupied bed. Let T be game time at the summon; its initial
schedule request must occur then. There must be no old sleep behaviour to stop during setup.

```mcfunction
/time of minecraft:overworld set 12000
/setblock 0 64 0 minecraft:red_bed[part=foot,facing=south]
/setblock 0 64 1 minecraft:red_bed[part=head,facing=south]
/summon minecraft:villager 0.5 64 1.5 {Tags:["brain_schedule"],VillagerData:{type:"minecraft:plains",profession:"minecraft:nitwit",level:1},Brain:{memories:{"minecraft:home":{value:{dimension:"minecraft:overworld",pos:[I;0,64,1]}}}}}
/data get entity @e[tag=brain_schedule,limit=1] Brain
/tick step 1
/data get entity @e[tag=brain_schedule,limit=1] sleeping_pos
/time of minecraft:overworld set 10
/tick step 19
/data get entity @e[tag=brain_schedule,limit=1] sleeping_pos
/tick step 1
/data get entity @e[tag=brain_schedule,limit=1] sleeping_pos
/tick step 1
/data get entity @e[tag=brain_schedule,limit=1] sleeping_pos
/data get entity @e[tag=brain_schedule,limit=1] Brain
```

Require sleep at T+1 as the setup check. If absent, record a setup failure and restart with a
fresh subject after repair. Do not mutate an existing subject and retain the old T.

Expected: sleep must remain at T+1 and T+20. The accepted request at T+21 must wake the
villager in that update. The reads correspond to T+1, T+20, T+21 and T+22 respectively.
`sleeping_pos` must therefore be absent in the last two reads. Record an earlier wake as a
conflicting observation. E-B9 tests the other frame boundaries separately.

The mcrs behaviour fixture must give a villager-like body an ordinary schedule reader and a valid home.
It must accept a schedule request at T with the paused clock at 12000.
After the clock changes to 10, rest must remain active through T+20.
The T+21 request must select idle and stop sleep in that update.
Repeat with the baby attribute and its activity configuration.

#### Captured results

Date: 2026-11-11. Version and world: section 12.3.
The adult and baby trials used separate fresh nitwits, with the same accessible home.
The baby trial added `Age:-24000`. No entity-data edit occurred after either summon.

```text
> time of minecraft:overworld set 12000
Clock minecraft:overworld is already set to 12000 tick(s)
> setblock 8 -60 7 minecraft:red_bed[part=foot,facing=south]
> setblock 8 -60 8 minecraft:red_bed[part=head,facing=south]
> summon minecraft:villager 8.5 -60 8.5 {Tags:["schedule"],Age:0,VillagerData:{type:"minecraft:plains",profession:"minecraft:nitwit",level:1},Brain:{memories:{"minecraft:home":{value:{dimension:"minecraft:overworld",pos:[I;8,-60,8]}}}}}
Summoned new Nitwit
> tick step 1
Stepping 1 tick(s)
> data get entity @e[tag=schedule,limit=1] sleeping_pos
Nitwit has the following entity data: [I; 8, -60, 8]
> time of minecraft:overworld set 10
Set minecraft:overworld to 10 tick(s)
```

Each following tick used `tick step 1`, completion verification, then the same `sleeping_pos` read.
Both ages returned `[I; 8, -60, 8]` at every update T+1 through T+20.
Both ages returned this reply at T+21, T+22 and T+23:

```text
Found no elements matching sleeping_pos
```

The adult summon occurred at game time 972; the baby summon occurred at 1016.
The wake reads therefore occurred at 993 and 1037 respectively.
Expected and observed: the changed schedule first affects this subject at T+21, with wake in that update.
This confirms the tested interval of BRAIN-VAN-025 and the sleep consequence of BRAIN-VAN-033.
It does not expose the active activity id or every source of a schedule request.

### E-B4: Damaged entries, ignored registration and complete replacement

Status: captured. Rules: BRAIN-VAN-001, BRAIN-VAN-006, BRAIN-VAN-007.

```mcfunction
/summon minecraft:villager 8.5 64 0.5 {Tags:["brain_load"],NoAI:1b}
/data modify entity @e[tag=brain_load,limit=1] Brain set value {memories:{"minecraft:last_slept":{value:123L},"minecraft:unknown_brain_probe":{value:1b},"minecraft:attack_target":{value:{}},"minecraft:last_woken":{value:"bad"}}}
/data get entity @e[tag=brain_load,limit=1] Brain
/data modify entity @e[tag=brain_load,limit=1] Brain set value {memories:{}}
/data get entity @e[tag=brain_load,limit=1] Brain
/data modify entity @e[tag=brain_ttl,limit=1] Brain set value {memories:{"minecraft:has_hunting_cooldown":{value:1b,ttl:"bad"},"minecraft:liked_player":{value:[I;0,0,0,1]}}}
/data get entity @e[tag=brain_ttl,limit=1] Brain
```

No ticks occur between these commands. The villager must retain only `last_slept:123L`, then
lose it on complete replacement. Record diagnostics for the three bad entries. The axolotl
must retain hunting cooldown permanently and ignore valid but unregistered `liked_player`.

Repeat its replacement with `ttl:2`, `ttl:2.9d` and `ttl:{}`; immediate reads must show 2L, 2L
and omitted `ttl`. A missing `value` must drop only that entry.

#### Captured results

Date: 2026-11-11. Version and world: section 12.3.
The subjects had `NoAI:1b`. No tick separated each replacement from its immediate read.

```text
> summon minecraft:villager 8 -60 8 {Tags:["load"],NoAI:1b}
Summoned new Villager
> data modify entity @e[tag=load,limit=1] Brain set value {memories:{"minecraft:last_slept":{value:123L},"minecraft:unknown_brain_probe":{value:1b},"minecraft:attack_target":{value:{}},"minecraft:last_woken":{value:"bad"}}}
Modified entity data of Villager
> data get entity @e[tag=load,limit=1] Brain
Villager has the following entity data: {memories: {"minecraft:last_slept": {value: 123L}}}
> data modify entity @e[tag=load,limit=1] Brain set value {memories:{}}
Modified entity data of Villager
> data get entity @e[tag=load,limit=1] Brain
Villager has the following entity data: {memories: {}}
```

The server logged these diagnostic excerpts for the rejected entries:

```text
Not a number
Unknown registry key in ResourceKey[minecraft:root / minecraft:memory_module_type]: minecraft:unknown_brain_probe
Memory module minecraft:attack_target cannot be encoded
No key value
```

The axolotl trial used this replacement, with each `ttl` from the table:

```text
> summon minecraft:axolotl 8 -60 8 {Tags:["load_ax"],NoAI:1b}
Summoned new Axolotl
> data modify entity @e[tag=load_ax,limit=1] Brain set value {memories:{"minecraft:has_hunting_cooldown":{value:1b,ttl:2.9d},"minecraft:liked_player":{value:[I;0,0,0,1]}}}
Modified entity data of Axolotl
> data get entity @e[tag=load_ax,limit=1] Brain
Axolotl has the following entity data: {memories: {"minecraft:has_hunting_cooldown": {value: 1b, ttl: 2L}}}
```

| Input `ttl` | Saved entry fields |
| - | - |
| `"bad"` | `value: 1b` |
| `2` | `value: 1b, ttl: 2L` |
| `2.9d` | `value: 1b, ttl: 2L` |
| `{}` | `value: 1b` |
| `0L` | `value: 1b, ttl: 0L` |
| `-5L` | `value: 1b, ttl: -5L` |

Every axolotl read omitted `minecraft:liked_player`. A missing `value` then erased the hunting entry:

```text
> data modify entity @e[tag=load_ax,limit=1] Brain set value {memories:{"minecraft:has_hunting_cooldown":{ttl:2L}}}
Modified entity data of Axolotl
> data get entity @e[tag=load_ax,limit=1] Brain
Axolotl has the following entity data: {memories: {}}
```

A separate no-AI villager received valid `last_slept:123L` beside an unknown id and an invalid `last_woken` value.
After `save-all flush`, `stop` and one restart, its read still contained only `last_slept:123L`.
The command load rejected damaged fields before save; the restart tested persistence of the retained subset.
No external edit placed damaged NBT on disk.
Expected and observed: independent rejection, complete replacement, ignored registration and tolerant numeric expiry conversion.
This confirms these clauses of BRAIN-VAN-006 and BRAIN-VAN-007, without confirming the complete registration inventory.

### E-B5: Sensor-written expiry and first appearance

Status: captured (listed probes). Rules: BRAIN-VAN-004, BRAIN-VAN-010, BRAIN-VAN-018.
Use a new water enclosure, with no entity-data edit after the summon.

```mcfunction
/fill 8 64 8 12 66 12 minecraft:water
/summon minecraft:frog 10.5 65 10.5 {Tags:["brain_water"]}
/data get entity @e[tag=brain_water,limit=1] Brain
/tick step 1
/data get entity @e[tag=brain_water,limit=1] Brain
```

Repeat step/read through tick 19. `minecraft:is_in_water` must first appear at f in 1..19,
with no `ttl`. Tick 1 must have probability 2/20; each other possible tick must have 1/20.
For finite inside-write expiry use a separate subject:

```mcfunction
/summon minecraft:armadillo 12.5 64 0.5 {Tags:["brain_danger"]}
/summon minecraft:zombie 14.5 64 0.5 {Tags:["brain_threat"],NoAI:1b,Silent:1b}
/tick step 1
/data get entity @e[tag=brain_danger,limit=1] Brain
```

Repeat until `minecraft:danger_detected_recently` first appears at tick T. Its first saved
`ttl` must be 80L, not 79L. Immediately move the threat away:

```mcfunction
/tp @e[tag=brain_threat,limit=1] 30.5 64 30.5
/tick step 1
/data get entity @e[tag=brain_danger,limit=1] Brain
```

Repeat the pair through T+81. Without another refresh or species erasure, expect 79L at T+1,
0L at T+80 and absence at T+81. If rolling becomes unavailable and the sensor erases danger,
record that interfering cause; the trial cannot certify expiry. E-B2 remains the isolated test.

#### Captured results

Date: 2026-11-11. Version and world: section 12.3.
The water probe used 200 fresh frogs, without data edits or a player.
It read each subject before any update, then after every single update through 19.
The enclosure had water through y=-53 and a glass roof at y=-52.

The commands below show the first subject. Tags and coordinates varied across the cohort.

```text
> fill 0 -60 0 15 -52 15 minecraft:glass
> fill 1 -60 1 14 -53 14 minecraft:water
> gamerule max_entity_cramming 0
Game rule max_entity_cramming is now set to 0
> summon minecraft:frog 2.5 -57 2.5 {Tags:["water","water0"]}
Summoned new Frog
> data get entity @e[tag=water0,limit=1] Brain
Frog has the following entity data: {memories: {}}
```

At each first positive read, the water entry had this field excerpt:

```snbt
"minecraft:is_in_water": {value: {}}
```

| First positive update | Subjects |
| - | - |
| 1 | 23 |
| 2 | 8 |
| 3 | 9 |
| 4 | 9 |
| 5 | 9 |
| 6 | 6 |
| 7 | 8 |
| 8 | 6 |
| 9 | 5 |
| 10 | 13 |
| 11 | 11 |
| 12 | 8 |
| 13 | 9 |
| 14 | 11 |
| 15 | 14 |
| 16 | 12 |
| 17 | 13 |
| 18 | 12 |
| 19 | 14 |

All 200 first writes occurred in 1..19, with no `ttl`. The counts are consistent with the proposed distribution.
They do not establish exact probabilities or independence.

A second cohort of 20 frogs supplied a departure probe. After update 19, this command removed the water:

```text
> fill 1 -60 1 14 -53 14 minecraft:air
```

One-tick reads continued through update 40. Each subject lost the water entry exactly 20 updates after its first write.
The observed first-write/erase pairs were:

```text
2/22: 1 subject; 5/25: 2; 6/26: 2; 7/27: 4; 10/30: 2;
11/31: 1; 14/34: 2; 16/36: 2; 17/37: 1; 18/38: 3.
```

The finite-write probe used 20 fresh armadillos and one stationary zombie:

```text
> summon minecraft:armadillo 8.5 -60 8.5 {Tags:["danger","danger0"]}
Summoned new Armadillo
> summon minecraft:zombie 11.5 -60 8.5 {Tags:["threat"],NoAI:1b,Silent:1b}
Summoned new Zombie
```

Nineteen subjects first showed danger by update 19; one still lacked it at update 20.
First appearances were 2:2, 4:1, 6:2, 8:1, 9:1, 11:1, 12:1, 13:2, 14:1, 16:1, 17:4 and 19:2.
Every first positive read contained this field excerpt:

```snbt
"minecraft:danger_detected_recently": {value: 1b, ttl: 80L}
```

The danger probe depends on another sensor's output. Its first-appearance distribution does not measure one isolated sensor phase.
After update 20, the threat moved away:

```text
> tp @e[tag=threat,limit=1] 40 -60 40
Teleported Zombie to 40.500000, -60.000000, 40.500000
```

For subject 0, the last 80L write occurred at update 19.
The read at update 20 showed 79L. After departure, updates 1..79 showed 78L through 0L; update 80 omitted danger.
Subject 3 last refreshed at update 17, then showed 77L at update 20 and 76L after departure's first step.
It showed 0L after departure's step 77, then absence at step 78.
All 19 positive subjects reached zero before erasure, without a refresh after departure.

Expected and observed: an inside-update finite write first saves N, then decrements on the next update.
This supports BRAIN-VAN-004, the tested cadence of BRAIN-VAN-010 and the water write/erase of BRAIN-VAN-018.
Nearest-player cadence and sensor order remain separate proposals.

### E-B6: Partial observation of equal-priority activity order

Status: proposal; captured trial is inconclusive. Rule: BRAIN-VAN-027. Limit: U2.
Use a previously established adult villager with a profession, offers and a claimed accessible
bed. A player must hold an item accepted by one offer, without a trade screen.
Require a displayed trade sample before this sequence:

```mcfunction
/tag @e[type=minecraft:villager,sort=nearest,limit=1] add brain_order
/time of minecraft:overworld set 2000
/tick step 100
/tick freeze
/time of minecraft:overworld set 12000
/tick step 1
/data get entity @e[tag=brain_order,limit=1] Brain
/data get entity @e[tag=brain_order,limit=1] Pos
/data get entity @e[tag=brain_order,limit=1] sleeping_pos
```

Repeat the step and three reads for 40 ticks. Record the visible held sample and sleep/movement
order. Expect an old sample-display behaviour to be able to survive selection of rest.
Rest/core ties must follow the table's equal-number order; behaviours at earlier priorities
must act first. A conflicting observed action order can refute the relevant table entry.
A display that survives confirms lack of blanket cancellation, not the entire tie table.

A complete ordering test needs competing observable actions at the same priority; commands
alone cannot expose all such participants. Do not mark unobserved table entries confirmed.

#### Captured results

Date: 2026-11-11. Version and world: section 12.3.
Twenty fresh farmers had accessible beds and one offer: one emerald buys one bread.
The protocol player held an emerald. The paused phase was 2000 for 100 updates.
The setup used a separate home and tag for each farmer.

```text
> item replace entity brain_probe weapon.mainhand with minecraft:emerald
> time of minecraft:overworld set 2000
> summon minecraft:villager 1.5 -60 2.5 {Tags:["trade","trade0"],VillagerData:{type:"minecraft:plains",profession:"minecraft:farmer",level:1},Xp:1,Brain:{memories:{"minecraft:home":{value:{dimension:"minecraft:overworld",pos:[I;1,-60,2]}}}},Offers:{Recipes:[{buy:{id:"minecraft:emerald",count:1},sell:{id:"minecraft:bread",count:1},uses:0,maxUses:1000}]}}
Summoned new Farmer
```

Eight subjects displayed bread at some point. Their first display updates were 2, 3, 4, 6, 6, 14, 64 and 69.
These are composite behaviour observations, not the distribution of one sensor phase.
Seven still displayed bread at update 100.
After the clock changed to 12000, all 20 subjects received another 40 single-update reads.
An example before the change was:

```text
> data get entity @e[tag=trade8,limit=1] equipment
Farmer has the following entity data: {mainhand: {count: 1, id: "minecraft:bread"}}
```

Bread remained visible in several subjects through post-change update 5.
At update 6, thirteen subjects slept; none of those subjects still displayed bread.
An example pair from that update was:

```text
> data get entity @e[tag=trade8,limit=1] equipment
Found no elements matching equipment
> data get entity @e[tag=trade8,limit=1] sleeping_pos
Farmer has the following entity data: [I; 10, -60, 5]
```

Expected: a retained display can expose behaviour across an activity transition.
Observed: a valid display setup, followed by sleep and display erasure; no display persisted together with sleep.
The capture does not identify the exact activity transition or each display's stop condition.
It cannot decide equal-priority order or generic cancellation. BRAIN-VAN-027 remains source-suggested; U2 still applies.

### E-B7: Stuck pursuit pause and target displacement

Status: proposal; captured trial is inconclusive. Rules: BRAIN-VAN-034, BRAIN-VAN-035. Limit: U5.
Use a fresh corridor separate from other subjects. Keep the Overworld clock paused.

```mcfunction
/gamemode spectator @s
/fill -1 64 -1 13 67 1 minecraft:stone
/fill 0 64 0 12 66 0 minecraft:air
/summon minecraft:piglin 0.5 64 0.5 {Tags:["brain_walk"],IsImmuneToZombification:1b}
/summon minecraft:item 8.5 64 0.5 {Tags:["brain_item"],NoGravity:1b,Item:{id:"minecraft:gold_ingot",count:1}}
/tick step 1
/data get entity @e[tag=brain_walk,limit=1] Pos
```

Repeat the step/read pair until pursuit starts while the piglin remains before x=3.
Then close its existing route, allow a genuine stuck stop and reopen it:

```mcfunction
/fill 4 64 0 4 66 0 minecraft:stone
/tick step 1
/data get entity @e[tag=brain_walk,limit=1] Pos
/data get entity @e[tag=brain_walk,limit=1] Brain
```

Repeat the last three commands until a stationary pursuit interval is established. Record its
start S as an observation, not proof of the exact internal stuck-stop tick.

```mcfunction
/fill 4 64 0 4 66 0 minecraft:air
/tp @e[tag=brain_item,limit=1] 10.5 64 0.5
/tick step 1
/data get entity @e[tag=brain_walk,limit=1] Pos
/tp @e[tag=brain_item,limit=1] 11.5 64 0.5
/tick step 1
/data get entity @e[tag=brain_walk,limit=1] Pos
```

Continue one-tick reads for at least 40 game ticks with an available pursuit target. Expect a
sampled pause in 0..39;
with K=3, route computation is eligible on the fourth attempt after the true stop.
Exactly 2 blocks from the old target must not itself trigger recomputation; 3 blocks must.
An initial no-route failure must erase the target without creating this pause.

Random admiration expiry and hidden target re-creation can interfere. Saved `Brain` cannot
show the transient route or failure timestamp. Travel time alone must not certify exact K.

#### Captured results

Date: 2026-11-11. Version and world: section 12.3.
The first corridor allowed escape through its ends. That trial was rejected.
The repeat used 20 fresh piglins, a corridor with closed ends, and a wanted item with pickup delay.

The player stayed in spectator mode. The clock stayed paused at 12000.

```text
> fill 0 -60 0 15 -56 2 minecraft:stone
> fill 1 -60 1 14 -58 1 minecraft:air
> summon minecraft:piglin 1.5 -60 1.5 {Tags:["walk","walk0"],IsImmuneToZombification:1b}
Summoned new Piglin
> summon minecraft:item 8.5 -60 1.5 {Tags:["gold"],NoGravity:1b,PickupDelay:32767s,Item:{id:"minecraft:gold_ingot",count:64}}
Summoned new Item
```

The gate closed after update 4, when the first observed x coordinate exceeded 1.6:

```text
> data get entity @e[tag=walk19,limit=1] Pos
Piglin has the following entity data: [1.603915874199104d, -60.0d, 1.5d]
> fill 5 -60 1 5 -58 1 minecraft:stone
Successfully filled 3 block(s)
```

Single-update reads continued through update 160.
Some subjects acquired `admiring_item`; others had an empty saved brain.
The capture then reopened the gate, moved the item by two blocks, then by three blocks from its original position:

```text
> fill 5 -60 1 5 -58 1 minecraft:air
> tp @e[tag=gold,limit=1] 10.5 -60 1.5
> tick step 1
Stepping 1 tick(s)
> tp @e[tag=gold,limit=1] 11.5 -60 1.5
```

Position reads continued for 60 updates after reopening.
Expected: a known stuck stop, a retained target and an observable retry pause.
Observed: crowd displacement and intermittent admiration, without a saved route, target or exact stuck-stop timestamp.
The first displacement did not prove that a route already existed when the gate closed.
No trial qualified for an exact retry-pause measurement. BRAIN-VAN-034 and BRAIN-VAN-035 remain source-suggested under U5.

### E-B8: Player-passenger freeze and the dying-camel exception

Status: captured (direct passenger, control and death). Rules: BRAIN-VAN-005, BRAIN-VAN-008. Limit: U3.
Use E-B2's subject, without a saddle or controlling passenger.

```mcfunction
/data modify entity @e[tag=brain_ttl,limit=1] Brain set value {memories:{"minecraft:has_hunting_cooldown":{value:1b,ttl:100L}}}
/ride @s mount @e[tag=brain_ttl,limit=1]
/data get entity @e[tag=brain_ttl,limit=1] Brain
/data get entity @e[tag=brain_ttl,limit=1] Brain
/ride @s dismount
/data get entity @e[tag=brain_ttl,limit=1] Brain
```

Wait ordinary wall time between the two mounted reads without a tick step. Expected: expiry can
advance while the player is a passenger, despite freeze. After dismount its expiry must stop.
A separate saddled, player-controlled camel trial must instead retain expiry during control.
For a deterministic death-window comparison run these commands while frozen:

```mcfunction
/summon minecraft:camel 8.5 64 8.5 {Tags:["brain_death"],Health:0f,Brain:{memories:{"minecraft:temptation_cooldown_ticks":{value:1000,ttl:0L}}}}
/data get entity @e[tag=brain_death,limit=1] Brain
/tick step 1
/data get entity @e[tag=brain_death,limit=1] Brain
/data get entity @e[tag=brain_death,limit=1] DeathTime
```

Expected from provisional evidence: present before the first tick step, absent on the first corpse
update.
Use an explicit UUID for each dying subject and query that UUID; `@e` excludes these dead subjects.
Repeat with `minecraft:camel_husk` and `minecraft:goat` with distinct UUIDs. The goat must retain
its zero-duration memory while dying. Reject trials with grazing/rearing or rider suppression.

#### Captured results

Date: 2026-11-11. Version and world: section 12.3.
The death probe needed explicit UUIDs: an ordinary `@e` selector returned no dead subject even before the first step.
The original selector-based procedure therefore supplied no corpse measurement.

The successful commands used UUID words `[I;1,2,3,1]`, `[I;1,2,3,2]` and `[I;1,2,3,3]` for the three bodies.

```text
> summon minecraft:camel 8 -60 8 {UUID:[I;1,2,3,1],Health:0f,Brain:{memories:{"minecraft:temptation_cooldown_ticks":{value:1000,ttl:0L}}}}
Summoned new Camel
> data get entity 00000001-0000-0002-0000-000300000001 Brain
Camel has the following entity data: {memories: {"minecraft:temptation_cooldown_ticks": {value: 1000, ttl: 0L}}}
> tick step 1
Stepping 1 tick(s)
> data get entity 00000001-0000-0002-0000-000300000001 Brain
Camel has the following entity data: {memories: {"minecraft:gaze_cooldown_ticks": {value: 193}}}
> data get entity 00000001-0000-0002-0000-000300000001 DeathTime
Camel has the following entity data: 1s
```

The camel husk also lost the injected value at `DeathTime:1s`:

```text
Camel Husk has the following entity data: {memories: {"minecraft:gaze_cooldown_ticks": {value: 247}}}
```

The goat retained it at the same death boundary:

```text
Goat has the following entity data: {memories: {"minecraft:temptation_cooldown_ticks": {value: 1000, ttl: 0L}}}
Goat has the following entity data: 1s
```

The later passenger probe used the connected survival player `brain_probe`, with the game frozen.

```text
> summon minecraft:axolotl 8.5 -59 8.5 {Tags:["passenger"],Brain:{memories:{"minecraft:has_hunting_cooldown":{value:1b,ttl:100L}}}}
Summoned new Axolotl
> ride brain_probe mount @e[tag=passenger,limit=1]
brain_probe started riding Axolotl
```

The first mounted read and a second read after 0.4 seconds showed the expiry values recorded below.
After dismount, two reads separated by 0.4 seconds showed the same remaining expiry.

```text
Axolotl has the following entity data: {memories: {"minecraft:has_hunting_cooldown": {value: 1b, ttl: 100L}}}
Axolotl has the following entity data: {memories: {"minecraft:has_hunting_cooldown": {value: 1b, ttl: 91L}}}
Axolotl has the following entity data: {memories: {"minecraft:has_hunting_cooldown": {value: 1b, ttl: 91L}}}
Axolotl has the following entity data: {memories: {"minecraft:has_hunting_cooldown": {value: 1b, ttl: 91L}}}
```

The controlled-camel trial equipped the saddle through the command, then mounted the player:

```text
> item replace entity @e[tag=control,limit=1] saddle with minecraft:saddle
Replaced 1 slot(s) on Camel with [Saddle]
> ride brain_probe mount @e[tag=control,limit=1]
brain_probe started riding Camel
Camel has the following entity data: {memories: {"minecraft:temptation_cooldown_ticks": {value: 1000, ttl: 100L}}}
```

The second read after 0.4 seconds and the immediate dismount read still showed 100L.
Expected and observed: a player passenger permits frozen axolotl AI; client control suppresses camel AI.
The corpse comparison confirms the dying camel-family exception of BRAIN-VAN-005.
This does not test replacement/travel retention in BRAIN-VAN-008 or an indirect passenger.

### E-B9: Schedule frame boundaries by age and dimension

Status: partial capture; work/play/meet activity consequences remain proposal. Rules: BRAIN-VAN-025 and BRAIN-VAN-026. Limit: U1.
Repeat E-B3 at each row below with a claimed bed and an established ordinary schedule reader.

1. Set the paused clock to the before value.
2. Advance 21 ticks.
3. Read `sleeping_pos`.
4. Read `Brain`.
5. Set the paused clock to the after value.
6. Advance 21 ticks.
7. Read `sleeping_pos`.
8. Read `Brain`.

Use these command templates with the literal phase from each table cell:

```mcfunction
/time of minecraft:overworld set 9
/tick step 21
/data get entity @e[tag=brain_schedule,limit=1] sleeping_pos
/data get entity @e[tag=brain_schedule,limit=1] Brain
/time of minecraft:overworld set 10
/tick step 21
/data get entity @e[tag=brain_schedule,limit=1] sleeping_pos
/data get entity @e[tag=brain_schedule,limit=1] Brain
```

| Age/setup | Before -> after | Expected sampled change |
|-----------|-----------------|-------------------------|
| Adult with valid job site | 1999 -> 2000 | idle -> work |
| Adult with valid meeting point | 8999 -> 9000 | work -> meet |
| Adult | 10999 -> 11000 | meet -> idle |
| Adult or baby with home | 11999 -> 12000 | awake -> rest, then sleep when eligible |
| Baby | 2999 -> 3000 | idle -> play |
| Baby | 5999 -> 6000 | play -> idle |
| Baby | 9999 -> 10000 | idle -> play |
| Adult or baby | 23999 -> 0 | rest remains rest |
| Adult or baby | 9 -> 10 | rest -> idle, wake |

For a baby, create a distinct subject with `Age:-24000` and its own home before measurement.
Motion toward a known workstation/meeting point and sleep are consequences, not saved activity
ids. An absent required site must instead allow default idle. The work/play/meet transitions
need their species fixtures; the shared sleep probe alone cannot confirm those action lists.

Repeat bed probes in Nether and End with `/execute in minecraft:the_nether run summon ...`
and `/execute in minecraft:the_end run summon ...`. Use each world's prepared platform and home
dimension. Keep a player there and query in that dimension with `/execute in`.
The paused Overworld clock must control each probe. Each frame value must start at its exact
clock phase. An accepted request must occur no earlier than 21 game ticks after the last.

#### Captured results

Date: 2026-11-11. Version and world: section 12.3.
Both ages used fresh nitwits, with their own `home`, at each sampled phase.
Each trial read sleep after one and 21 completed updates.
The Overworld phase set was 0, 9, 10, 1999, 2000, 2999, 3000, 5999, 6000, 8999, 9000, 9999, 10000, 10999, 11000, 11999, 12000 and 23999.

For both ages, phases 0, 9, 12000 and 23999 returned this reply after both reads:

```text
Nitwit has the following entity data: [I; 8, -60, 8]
```

Every other sampled phase returned:

```text
Found no elements matching sleeping_pos
```

A separate live-clock probe started three ticks before 12000, 0 and 10.
It read one completed tick at a time through update 23, for both ages.
At the 12000 boundary, sleep began at update 22. The first later schedule sample became eligible at update 21.

The rest behaviour first started in the next update.
At the 10 boundary, sleep ended at update 21. Sleep continued across 23999 -> 0.
The schedule value changed at its frame; the sleep consequence waited for the body's eligible request.

Nether and End repeated the sleep probe at 0, 9, 10, 11999, 12000 and 23999, for both ages.
Their home used the body's dimension and `[I;8,64,8]`.
The setup used `/execute in <dimension> run` with the same summon, bed, force-load and read commands.
Only the Overworld clock changed. Example replies were:

```text
> execute in minecraft:the_nether run data get entity @e[tag=dim_schedule,limit=1] sleeping_pos
Nitwit has the following entity data: [I; 8, 64, 8]
> execute in minecraft:the_end run data get entity @e[tag=dim_schedule,limit=1] sleeping_pos
Nitwit has the following entity data: [I; 8, 64, 8]
```

Both dimensions slept at 0, 9, 12000 and 23999, and stayed awake at 10 and 11999.

A native environment predicate supplied the missing schedule-value probe, without an activity-state capture facility:

```text
> time of minecraft:overworld set 2000
Set minecraft:overworld to 2000 tick(s)
> execute positioned 8.5 -60 8.5 if predicate {type:"minecraft:environment_attribute_check",attribute:"minecraft:gameplay/villager_activity",value:"minecraft:work"} run time query gametime
The game time is 4476 tick(s)
> execute positioned 8.5 -60 8.5 if predicate {type:"minecraft:environment_attribute_check",attribute:"minecraft:gameplay/villager_activity",value:"minecraft:idle"} run time query gametime
```

The first condition passed; the idle condition returned no reply.
The capture tested idle, work, play, meet and rest separately for each attribute at every listed phase.
Exactly one condition passed per attribute and phase. Every value matched BRAIN-VAN-026's interval table.

The live-clock repeat read each attribute at frame-3 through frame+3, one completed tick at a time.
Adult frames were 0, 10, 2000, 9000, 11000 and 12000.
Baby frames were 0, 10, 3000, 6000, 10000 and 12000.
Each value changed at the exact nonzero frame; phase 0 retained rest.
For example, the adult 1999/2000 replies were:

```text
> time query minecraft:villager_schedule
Timeline minecraft:villager_schedule is at 1999 tick(s)
> execute positioned 8.5 -60 8.5 if predicate {type:"minecraft:environment_attribute_check",attribute:"minecraft:gameplay/villager_activity",value:"minecraft:idle"} run time query gametime
The game time is 4490 tick(s)
> time query minecraft:villager_schedule
Timeline minecraft:villager_schedule is at 2000 tick(s)
> execute positioned 8.5 -60 8.5 if predicate {type:"minecraft:environment_attribute_check",attribute:"minecraft:gameplay/villager_activity",value:"minecraft:work"} run time query gametime
The game time is 4491 tick(s)
```

Expected and observed: all schedule attribute intervals, the rest/wake boundary and the Overworld-clock effect in three dimensions.
Work/play/meet activity membership and their species actions remain proposal; the predicate reads the environment value, not the active activity.
The original sleep-only probe cannot distinguish those three activities.

### 12.2 Proposals that need a brain capture facility

E-B10 through E-B19 need a new capture facility in `tools/vanilla-oracle`.
The current tool has no brain capture task. These proposals do not claim that such a task exists.

Each capture must include version metadata and the inputs below. It must report memory values,
activity membership, behaviour actions, query results and random outcomes at each specified boundary.
Saved NBT alone cannot supply these observations. Keep the game and mcrs captures separate.

A future mcrs behaviour test must run the production brain with the same inputs.
It must compare the stated results and action order with the captured game results.
A specification author must supply game captures before any rule gains confirmed provenance.

### E-B10: Status requests, memory writes and brain replacement

Status: proposal. Rules: BRAIN-VAN-001, BRAIN-VAN-002, BRAIN-VAN-003, BRAIN-VAN-008.

1. Configure explicit, sensor-declared and nested behaviour entry ids, with one inactive activity.
2. Add separate ids only to an activity condition and its exit list.
3. Capture the registered set.
4. Exercise each BRAIN-VAN-002 request for a present, absent and unregistered id.
5. Write false, zero, an empty list and an empty visibility result to their registered kinds.
6. Rewrite the same finite value with a different expiry.
7. Capture a read after each write in the same update.

Expect the union in BRAIN-VAN-001, without the two activity-only ids.
Expect the status table and invalid-request results of BRAIN-VAN-002.
Only the empty list must erase its memory. The last write must supply both value and expiry.

Use a body with one persisted finite value and one transient value for each independent case below.
Record both values, expiry, behaviour stop actions and fresh sensor phases before and after the
operation.

| Operation | Expected result |
|-----------|-----------------|
| Unload/reload or dimension travel | Persisted value and expiry survive; transient value does not. |
| Teleport within the dimension | Both values and the existing brain state survive. |
| Villager age-boundary or profession refresh | Stop effects occur before preservation; fresh sensors and activities replace the old state. |
| Goat age boundary | The existing brain survives. |
| Happy ghast maturity | Stop effects occur; all memories become absent. |
| Tadpole conversion to frog | The destination has its initial brain, without the complete old memory set. |

Repeat refresh with a persisted memory that a stop action erases.
That value must not survive. A replacement brain must discard bed retry records and choose fresh
sensor phases.

### E-B11: Nearest-player memory and its exact erasure scan

Status: proposal. Rules: BRAIN-VAN-003, BRAIN-VAN-010, BRAIN-VAN-011, BRAIN-VAN-013.
The word "expires" in issue #230 means erasure by a later scan for this memory.
It does not mean the finite expiry countdown of BRAIN-VAN-004.

Use no explicit memory ids. BRAIN-VAN-011 declares all four output ids for this sensor.

1. Configure a stationary test mob with F=8 and the nearest-player sensor.
2. Add a memory probe.
3. Use normal difficulty.
4. Put one unallied survival player at feet offset `(0,0,7)` in the same dimension, with clear sight.
5. Advance one brain update at a time until the first positive scan at update f.
6. Record all four player outputs immediately after that sensor.
7. Move the player to feet offset `(0,0,8)` before update f+1.
8. Record those outputs after each update through f+20.
9. Move the player back to `(0,0,7)` before update f+21.
10. Record those outputs through f+40.

All four outputs must contain the player at f, with permanent expiry.
They must retain that result through f+19. They must become absent at f+20, the first due scan
outside the strict sphere.
They must remain absent through f+39 and return at f+40.

Repeat with a spectator, F=0, and a different-dimension player. Each negative case must produce no
player output.
Repeat with two equal-distance players in both dimension encounter orders. Their list order must
follow that encounter order.

For the mcrs behaviour fixture, control each initial offset O in 0..19 through the sensor's random input.
Expect f=1 for O=0 or 1, and f=O otherwise. The first write must occur within the 20-update sensor
interval.
The fixture must assert the exact f+20 erasure and f+40 return with the production sensor and memory
storage.

E-B1 supplies a separate visible player-reaction probe.
After its positive run, move the player beyond F and record the creaking's response one tick at a time.
That visible response can corroborate loss of the player result. It cannot replace the direct memory
capture above.

### E-B12: Entity query, visibility and dependent adult/temptation scans

Status: proposal. Rules: BRAIN-VAN-011, BRAIN-VAN-012, BRAIN-VAN-014, BRAIN-VAN-019, BRAIN-VAN-020.

1. Configure a stationary body with F=4 and known hitbox dimensions.
2. Supply the candidate cases below in a recorded encounter order.
3. Execute a living-entity scan.
4. Capture membership, distance order and the visibility result.
5. Request the visibility answer for one candidate.
6. Move that candidate behind a wall before the next update.
7. Request its answer again before the next scan.
8. Execute the next scan.
9. Request the answer again.

| Candidate case | Expected result |
|----------------|-----------------|
| Hitbox with face contact only | Excluded from the raw set. |
| Hitbox with positive overlap at a box corner, but feet beyond F | Included in the raw set; ordinary range visibility fails. |
| Alive creative player, ally or invulnerable entity inside F | Included; noncombat visibility passes with clear sight, but attack restrictions still apply. |
| Dead entity or the body | Excluded from the raw set. |
| Equal feet distance | Raw order follows encounter order. |
| No candidates | `minecraft:mobs` absent; empty `minecraft:visible_mobs` present. |

The first requested answer must survive the wall change until the next scan.
After that scan, the wall must make a fresh sight-required answer fail.

Run separate range cases at equality and just beyond `max(R*V,2)`.
V must include sneak, invisibility, occupied armour slots and the defined head disguise.
Repeat with the candidate as attack target. Only the V factors must be bypassed.
For R=0, range must not reject; required sight must still reject the wall.

Run each adult sensor against a distance-ordered result with a baby, a different-type adult and a
same-type adult.
Expect the first visible nonbaby that satisfies that sensor's type filter.
Repeat with absent input and with a present empty input. The former must retain the old output; the
latter must erase it.

For each temptation sensor, set Q=10 and put a player with valid food behind a wall at distance 10.
Expect that player, including with food only in the off-hand. Distance just above 10 must fail for V=1.
Repeat at Q=0 with a distant player; range must not reject it.

A direct passenger must fail, but a passenger of another vehicle must remain eligible.
A dead player or spectator must fail. Equal-distance survivors must retain encounter order.

### E-B13: Damage-source age and independent attacker retention

Status: proposal. Rule: BRAIN-VAN-015.

1. Record living-attacker damage at game time T.
2. Place due scans at T+40 and T+60.
3. Capture source and attacker memories after each scan.
4. Apply environmental damage without a responsible living entity.
5. Execute a due scan.
6. Move the remembered attacker far away in the same dimension.
7. Execute a due scan.
8. Move that attacker to another dimension.
9. Execute a due scan.

At T+40, both memories must remain. At T+60, only the source memory must become absent.
Environmental damage must replace the source while the old attacker remains.
Distance alone must retain the attacker. Dimension travel must erase it on the next scan.

Repeat the last case with attacker death. Repeat source-age cases with a scan at T+41; that scan
must erase the source.

### E-B14: Item query and first eligible result

Status: proposal. Rules: BRAIN-VAN-011, BRAIN-VAN-016.

1. Configure a body whose item-desire predicate accepts gold ingots.
2. Supply item hitboxes at the query-box faces, with contact-only and positive-overlap variants.
3. Add a nearer unwanted item, an occluded wanted item and a farther visible wanted item.
4. Execute a due scan.
5. Capture the ordered candidates, predicate results and selected item.
6. Remove every eligible item.
7. Execute a due scan.

Only positive overlaps must enter the query. The nearer rejected candidates must not hide the
farther eligible item.
That item must win, then its removal must leave the output absent.

Repeat at squared feet distance 1024 and just below it; equality must fail.
Change F without other changes; the query and selected item must stay the same.
Repeat with pickup delay and ownership set on an otherwise desired item; no independent sensor
filter must reject it.
Equal-distance eligible items must follow encounter order.

### E-B15: Bed candidate cap, retry and retained output

Status: proposal. Rule: BRAIN-VAN-017.

1. Configure a baby at a known feet block with four inaccessible home POIs in a recorded encounter order.
2. Use jitter 0 for the first scan at t.
3. Capture accepted targets and retry deadlines.
4. Execute failed scans at t+20 with jitter 0 and t+40 with jitter 1.
5. Make the first target reachable before t+60.
6. Execute the t+60 scan.
7. Destroy the selected bed after success.
8. Execute the next scan.

The first scan must accept four targets with deadline t+40.
The t+20 scan must exclude them. The t+40 scan must still exclude them, then remove their records
because t+40<t+41.
The t+60 scan can accept them again and must write the reachable target.
The destroyed bed must not erase that output.

Repeat with jitter 0 at t+40. Equality must retain the records; cleanup at t+60 permits the earliest
retry at t+80.
Repeat with five new inaccessible positions and one old retry record whose deadline is strictly less than U.
The scan must count all five new positions, accept only the first four, and retain the expired old record.
An adult scan must leave output and retry facts unchanged, without a POI query.

### E-B16: Activity requests and exit effects

Status: proposal. Rules: BRAIN-VAN-021, BRAIN-VAN-022, BRAIN-VAN-023, BRAIN-VAN-024, BRAIN-VAN-027, BRAIN-VAN-039.

1. Configure core, idle and fight activities with a present-memory requirement on fight.
2. Give idle an unmet entry requirement.
3. Give fight a running behaviour and an attack-target exit memory.
4. Request fight with that memory absent.
5. Capture the active set and memory effects.
6. Write the required memory.
7. Request the ordered list `[minecraft:fight,minecraft:idle]`.
8. Request idle.
9. Capture exit erasure and the old behaviour's next action.

The first request must select default idle without its entry test.
The ordered list must select fight. The idle request must erase the attack target, but must not
generically cancel the old behaviour.

Repeat an explicit request for an already-active fight after its entry memory becomes absent.
It must fall back to default. An already-active eligible target must cause no transition effects.
An empty candidate list or a list without eligible activities must leave both memories and active
ids unchanged.
A core-id request must retain the other active id.

Use two observable memory writes at equal priority in each activity-order tie of BRAIN-VAN-027.
Record first declaration order for N=12, 13, 24, 25 and 26.
The table order must determine the last write. Repeat with a transition during a start turn and
during the running pass.
Later attempts in the current start turn must remain eligible; the running pass's initial
participants must retain their turns.

For every BRAIN-VAN-039 row, enter its activity with the listed memory present, then leave it.
That memory must become absent. Repeat with each omitted built-in activity/body pair and an
unrelated present memory.
No exit erasure must occur for an empty list. Entry with the required memory absent must fail.

### E-B17: Behaviour lifecycle, durations and weighted groups

Status: proposal. Rules: BRAIN-VAN-028, BRAIN-VAN-029, BRAIN-VAN-030, BRAIN-VAN-031, BRAIN-VAN-032, BRAIN-VAN-033.

1. Configure a timed behaviour with observable start, tick and stop actions.
2. Set its duration bounds to 0..0 and permit continuation.
3. Start it at T.
4. Capture actions through T+2.
5. Repeat with 60..60 and a failed default continuation.
6. Repeat with a failed entry requirement.

For D=0, expect start and tick at T, stop at T+1, then the next start at T+2.
For failed default continuation, expect start and stop together without a tick action; the next
start can occur at T+1.
A failed entry must produce no action.
Fixed duration bounds must give that duration.
Pause AI past a retained deadline; the first resumed running step must stop without intermediate
tick actions.

Configure a group with a child that fails, a successful one-shot and a successful timed child, in
that order.
In single-success mode, only the first two must receive attempts.
In try-all mode, all stopped children must receive attempts. A running child must receive no new
start attempt.
A successful one-shot must act only in the start pass and stop in the running pass.
A group with no successful children must stop in that update and can restart in the next.

For weighted mode, use two eligible one-shots with weights 1 and 2 over 100,000 fresh activations.
Record the selected orders. Their distribution must satisfy BRAIN-VAN-031.
With prescribed equal keys and equal weights, ties must retain the previous order.
Repeat with an ineligible child and a running child; eligible stopped children must retain the stated relative-order distribution.
Ordered mode must preserve its supplied order.

Force a group stop with two running children and an exit memory.
The children must stop in retained order before the exit memory becomes absent.
Repeat after the activity becomes inactive; ordinary child steps and forced stops must still occur.
AI suspension alone must produce no stop action. An empty activation must still apply its exit erasure.

### E-B18: Intermediate destinations and home-bias distribution

Status: proposal. Rule: BRAIN-VAN-036.

1. Configure no direct route, a fixed target direction and equal walk values for all legal destinations.
2. Record all ten sample attempts and the final route request.
3. Repeat with a home on both positive horizontal axes.
4. Repeat with a home on both negative horizontal axes.
5. Repeat with equality between the body and home on each horizontal axis in separate trials.
6. Repeat with a direct partial route.

Compare the candidate distribution with the formula in BRAIN-VAN-036.
A sample outside the horizontal limits must fail.
Other attempts must have a uniform vertical offset in [-7,7] and independent x and z biases when a home exists.
Each home bias must be uniform on [0,5) blocks, with distribution fidelity.
The coordinate comparison in BRAIN-VAN-036 must determine each bias sign.

Use body feet `(0,-60,0)` for these additional trials.
Prescribe x and z bias amounts of 2.5 blocks before the final floor.

| Home block | Expected change to sampled x/z offsets |
|------------|---------------------------------------|
| `(0,-60,10)` | `(2.5,2.5)`; equality on X must add the x bias. |
| `(-10,-60,0)` | `(-2.5,2.5)`; equality on Z must add the z bias. |

All ten attempts must occur before the first equal-value legal sample wins.
There must be at most one fallback route request. A direct partial route must trigger no fallback search.
Repeat with one invalid build height and one forbidden home destination; neither candidate can win.

### E-B19: Look lifetime and flotation probability

Status: proposal. Rules: BRAIN-VAN-029, BRAIN-VAN-037, BRAIN-VAN-038.

1. Supply a fixed look target and prescribed durations 45 and 90 in separate trials.
2. Capture look requests through each inclusive deadline and stop.
3. Replace the target immediately before stop.
4. Repeat with a living visible target, a dead target and a nonliving target.

Expect requests through T+D and erasure of the current target at T+D+1, including the replacement.
A living target must satisfy the retained visibility result; a nonliving or fixed target must pass
that test.
Record default head yaw, pitch and navigation state. Compare the limits and residual request with
BRAIN-VAN-037 and its linked control rules.

For flotation, test depth below, at and above the threshold in otherwise identical trials.
Include eye height 0.4 blocks, a smaller eye height, breeze and positive lava depth.
Only the stated strict depth cases must start. Keep eligible depth constant from T through T+62.

Record the duration and per-tick jump requests over repeated trials.
Their probability must equal the probability that a uniform binary32 fraction is strictly below binary32 0.8.
Jump eligibility must include T..T+60, exclude T+61 and resume at T+62.
The float behaviour must retain unrelated walk and look memories.

### 12.3 Capture conditions

The captures above ran on 2026-11-11 against vanilla server `26.4-snapshot-7`, in a flat world with seed 1.
The server listened only on loopback. Each independent probe removed old subjects and restored its test area.
The captures used one force-loaded chunk, with its surface at y=-60.
Nether and End probes used a stone platform at y=63 in their force-loaded chunk.
No player was present for the expiry, load, death, first water-scan or sleep probes.

Runtime and save metadata agreed:

```text
> version
Server version info:
id = 26.4-snapshot-7
name = 26.4 Snapshot 7
data = 5127
protocol = 1073742169 (0x40000159)
```

The save reported `DataVersion:5127` and `Version:{Name:"26.4 Snapshot 7",Id:5127,Snapshot:1b}`.
The server archive had SHA-1 `0870c02432ef2391ba7c586c3e113e98406aea20`.
The expiry and sleep records each count completed game ticks.
Each step waited until `time query gametime` reached its previous value plus the requested count.

The working clock commands are:

```text
> time of minecraft:overworld pause
Paused clock minecraft:overworld
> time of minecraft:overworld set 1000
Set minecraft:overworld to 1000 tick(s)
> time of minecraft:overworld query time
Clock minecraft:overworld is at 1000 tick(s)
> time query minecraft:villager_schedule
Timeline minecraft:villager_schedule is at 1000 tick(s)
> time query daytime
Can't find element 'minecraft:daytime' of type 'minecraft:timeline'...ry daytime<--[HERE]
```

Quoted replies below retain the game's spelling. Field excerpts omit unrelated memories.
An empty reply means the conditional command did not run.
A capture confirms only the scope stated in its conclusion.
