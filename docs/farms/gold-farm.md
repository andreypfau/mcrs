# The gold farm of MumboJumbo: benchmark scene

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Save | Hermitcraft Season 10, `~/mc-worlds/upgraded/hermitcraft10` (snapshot 7) and `~/mc-worlds/hermitcraft10` (1.21.8) |
| Video | Hermitcraft 10, episodes 11 and 12, "GOLD FARMING" |

This document describes a real mob farm of a real save. It states what the farm does in a vanilla
server, which mechanics it uses, and what this server must add to run it. It also states how to
benchmark the farm. Item drops, item entities and item transport are out of scope. The farm
collects gold items with hoppers, but this document does not use them.

This document is not a specification. Each mechanic points to the specification that must state
it. The numbers of section 2 come from a vanilla server that ran the save. The behaviour of
section 3 comes from the reference implementation and from that run.

## 1. The farm

### 1.1 Location

The farm is on the roof of the nether, above the bedrock at `y` 127. The centre of the farm is the
block column `x -317, z -609`. The biome of the column is `minecraft:nether_wastes` at all heights.
The farm occupies the block columns from `x -333` to `-301` and from `z -627` to `-566`. The chunks
are `(-22..-19, -40..-37)`. The inhabited time of these chunks is about 4,800,000 ticks.

### 1.2 Layout

The farm has these parts, from the top down:

| Part | Position | Content |
|------|----------|---------|
| Spawn platforms | 19 layers, `y` 198 to 252, one layer each 3 blocks | 210 `magma_block` in each layer, a disc of diameter 15 around the centre. Two blocks of air are above each layer |
| Edge of the top layer | `y` 252 | a ring of `tripwire` and a ring of `glass` to diameter 33 |
| Lure | the centre column `x -317, z -609` | one `turtle_egg` (`eggs=1`) each 6 blocks, from `y` 197 to 251, on a `glass` block. 10 eggs |
| Drop shafts | `x -317, z -610` and `x -317, z -608`, `y` 198 to 254 | an open bottom `mangrove_trapdoor` in each layer. Below `y` 198 the shafts are air |
| Shaft walls | the cells around the centre | `glass`, and `grindstone` at `x -316` |
| Kill chamber | `x -317, z -610..-608, y` 175 to 176 | three `honey_block` at `y` 174 are the floor. Closed top trapdoors at `x -318` make a gap at the feet and at the head |
| Player position | `x -319.5, y 175, z -609.5`, on `glass` | the player looks east into the kill chamber |
| Item collection | `y` 147 to 174 | hoppers, droppers, crafters, chests, redstone. Out of scope |

### 1.3 How the farm works

1. Zombified piglins spawn naturally on the magma platforms. Magma accepts only fire-immune mobs as
   a spawn floor, so ghasts, piglins and endermen cannot spawn there.
2. A zombified piglin searches for a turtle egg near it. Each egg is in range of the layer under it
   and the layer above it.
3. The piglin makes a path to the egg. The path finder sees an open trapdoor as a floor, so the
   path goes across a drop shaft.
4. The piglin walks onto the trapdoor, falls down the shaft, and lands on honey at `y` 175. It falls
   from 24 to 78 blocks.
5. The honey multiplies the fall damage by `0.2`. A fall of 78 blocks gives 15 damage, and the
   piglin lives with 5 health.
6. The player kills the piglins with a sword through the gap. A sweep attack hits more than one
   piglin. A kill by a player drops experience orbs.
7. The three cells of the chamber keep the crowd in each cell under 24 most of the time. When the
   crowd grows, crowd damage also kills piglins.

The farm needs a player. Natural spawning needs a player near the chunks, and the mob cap fills when
nobody kills the piglins. Without kills, the farm stops (section 2.3).

### 1.4 Carpet and vanilla

The save ran Carpet. `carpet.conf` sets only `netherPortalEntityInteractionCheckUseFullBlockShape`.
The farm has no portal, so this rule has no effect on it. `carpet-fixes.conf` sets only
`inhabitedTimeFix`. The farm works in a vanilla server. The run of section 2 used Carpet only for a
stand-in player and for counters.

The game rules of the save that the farm uses are `max_entity_cramming` 24, `mob_griefing` true,
`fall_damage` true, `mob_drops` true, `spawn_monsters` true and `universal_anger` false.
`fire_spread_radius_around_player` is 0. The 1.21.8 save has the difficulty hard. The upgraded save
has the difficulty easy. The run used hard.

## 2. Measured numbers

### 2.1 Method

1. Copy the upgraded save to a temporary directory. Remove the data packs of the save, because
   snapshot 7 does not load them. The farm uses none of them.
2. Run the vanilla server of snapshot 7 with Fabric loader 0.20.1 and Carpet 26.4-beta-5. Set
   `difficulty=hard`, `view-distance=10` and `simulation-distance=10`.
3. Spawn a Carpet player at `-319.5 175 -609.5`, yaw `-90`, pitch 0, in the nether, in survival.
   Give it a netherite sword with `sharpness` 5, `sweeping_edge` 3 and `looting` 3, and
   `resistance` 4.
4. Load the Scarpet app of section 8. It writes one line for each nether tick.
5. Run phase A: the player does not attack, for 670 ticks.
6. Run phase B: the player attacks each 13 ticks (`/player Gold attack interval 13`), for 12,110
   ticks.
7. Run `/profile health 400` and `/profile entities 400` at the end of phase B.

The hardware was an Apple M4 Max. The tick times include the overhead of Carpet and of the app.

### 2.2 Phase B: the farm runs

The counts are for each tick, over 12,110 ticks (10.1 minutes).

| Quantity | Mean | p50 | p90 | Max |
|----------|------|-----|-----|-----|
| Zombified piglins in the nether | 69.2 | 70 | — | 77 |
| Piglins on the platforms (`x -326..-308, y 197..255, z -620..-598`) | 13.3 | 13 | 18 | 28 |
| Piglins in the drop shafts | 1.0 | 1 | — | 5 |
| Piglins in the kill chamber | 26.0 | 26 | 29 | 50 |
| Experience orbs within 6 blocks of the chamber | 80.6 | 85 | 96 | 105 |
| Magma cubes in the nether | 2.5 | 1 | — | 9 |

| Event | Total | For each tick | Note |
|-------|-------|---------------|------|
| Zombified piglin spawns | 1,192 | 0.098 | max 5 in one tick; 576 ticks have a spawn |
| Spawns on the 19 platforms | 944 | 0.078 | 29 to 72 on each layer |
| Spawns below the roof | 236 | 0.019 | `y` 57 to 120 |
| Spawns at `y` 160 | 12 | 0.001 | on the floor of the item area |
| Deaths from a player attack | 785 | 0.065 | |
| Deaths from crowd damage (`cramming`) | 129 | 0.011 | |
| Deaths from a fall | 8 | 0.001 | |
| Other spawns | — | — | 35 piglins, 32 magma cubes, 28 ghasts, all below the roof |
| Experience orb entities added | 1,549 | 0.128 | the player reached level 46 |

Carpet spawn tracking for the same run reports 6,355 zombified piglins for each hour. The monster
category is at its cap of 70 in most ticks. Thus the kill rate limits the spawn rate.

The nether tick took about 7.8 ms. These are the parts that Carpet reports:

| Part of the nether tick | ms |
|-------------------------|----|
| Entities | 4.48 |
| — zombified piglins (73 on average) | 1.86 |
| — experience orbs (92 on average) | 0.99 |
| — items (50 on average, out of scope) | 0.30 |
| — striders, piglins and other mobs below the roof | about 0.5 |
| Natural spawning | 1.86 |
| Random ticks | 0.56 |
| Scheduled ticks | 0.50 |
| Block entities (243 hoppers, out of scope) | 0.35 |

The save also keeps chunks of the overworld loaded. That overworld load (1,733 hoppers) is not part
of the farm.

### 2.3 Phase A: no player attack

Over 670 ticks the cap stays full. The kill chamber holds 47 to 52 piglins. There are 32 spawns
(0.048 for each tick), most of them in the first ticks, and 6 deaths from crowd damage. The farm
stops when nobody kills the piglins.

### 2.4 Scene data of the save

The area of the farm holds 4,835 saved scheduled ticks of `minecraft:fire` below the roof. A fire
schedules its next tick 30 to 39 ticks later, also when it cannot spread. Thus these fires give about
140 scheduled ticks in each tick when their chunks tick. The save holds no zombified piglin near the
farm, because the server removes the mobs that can despawn when it unloads a chunk.

## 3. Mechanics

The table lists each mechanic in the order of the life of a piglin. The cost column is the share of
the nether tick of section 2.2. The status column uses these words:

- **Has**: this server has the mechanic, with a specification.
- **Part**: this server has a part. The column states the missing part.
- **None**: this server does not have the mechanic.

The size is the estimate of the work for the specification and the code: S (days), M (one to two
weeks), L (more).

| # | Mechanic | Vanilla behaviour | Cost | mcrs status | Specification | Size |
|---|----------|-------------------|------|-------------|---------------|------|
| 1 | Save and dimension | The nether loads from `dimensions/minecraft/the_nether/region`. The farm needs the columns within the simulation distance of the player | — | Has: the dimension worlds load a save (`mcrs_minecraft_engine_server`, `dimension_worlds_plugin.rs`) | — | — |
| 2 | Mob entities | A mob is a server-simulated entity with a type, a box, attributes, equipment and an AI | — | Part: the entity rows and the walker category exist (`mcrs_minecraft_engine_physics`, `entity_rows.rs`; `mcrs_minecraft_physics`, `category.rs`). The engine world has only players. No mob reaches a client: track N1 of `physics-architecture.md` is open | `replication.md` §7.11 (N1) | M |
| 3 | Natural spawning | Each tick, each spawnable chunk near a player tries packs of the monster category. A pack position is random in the chunk, `y` from the bottom to the `WORLD_SURFACE` height plus 1. The global cap is 70 × chunks / 289. A position must be more than 24 blocks from the player. The floor must accept the type: `magma_block` accepts only fire-immune types. The zombified piglin rule rejects a floor of `nether_wart_block` and needs no light check. The box must be free of blocks and liquids. `nether_wastes` gives zombified piglin weight 100, packs of 4 | 24 % | None. The biome spawn tables exist (`mcrs_minecraft_worldgen_builtin`, `biome.rs`). The mob category exists (`mcrs_minecraft_entity`, `spawn.rs`) | new `natural-spawning.md` | L |
| 4 | Despawn | A mob that can despawn and is more than 128 blocks from each player goes at once. After 600 ticks with no player within 32 blocks, it goes with the chance 1/800 in each tick. Mobs that can despawn count to the cap; others do not | part of 3 | None | `natural-spawning.md` | S |
| 5 | Mob AI: goals | Goal selection each 2 ticks (AIT-VAN-002). A zombified piglin has these goals: break a turtle egg, melee attack, random stroll, look at a player, look around. Its targets: the attacker (and it alerts others), a player only when it is angry | 24 % with 6 to 9 | None: `entity-ai-timing.md` states the cadence; track A is open | `entity-ai-timing.md` §1; new `mob-goals.md` | L |
| 6 | Turtle egg lure | The egg goal searches 24 blocks to the side and 3 blocks up and down for a `turtle_egg` with two air blocks above it. It searches again after 200 to 399 ticks, or 20 ticks after a find. It walks to the block above the egg. When it reaches the egg, it breaks it. In this farm no piglin reaches an egg | in 5 | None | `mob-goals.md` | M |
| 7 | Path search | The path search gives each block a path type. The tag `pathfinding/drop_down` holds `#trapdoors`, so the block above an open trapdoor is a floor with no malus. A zombified piglin has the malus 8 on lava. The node budget is 16 × the follow range (35), so 560 nodes | in 5 | None | new `pathfinding.md` | L |
| 8 | Path follower and movement | The mob walks along the path at its speed (0.23, plus 0.05 when angry). Movement, collision and step-up are the walker tick | in 5 | Has: walker tick (`mcrs_minecraft_physics`; PHY-ORD-002, PHY-MOV-*). The follower is None | `entity-physics.md`; `pathfinding.md` | M |
| 9 | Anger | A hurt piglin gets angry for 20 to 39 seconds. Each 4 to 6 seconds it gives its target to each zombified piglin within the follow range to the side and 10 blocks up and down that has no target. An angry piglin chases the player and makes paths again each 4 to 11 ticks, more when far | in 5 | None | `mob-goals.md` | M |
| 10 | Fall and fall damage | The fall distance grows while the mob falls. On landing, the damage is `floor((distance − safe_fall_distance) × multiplier × fall_damage_multiplier)`. The honey multiplier is 0.2. Damage type `fall` | small | Part: the fall distance and the fall action exist (PHY-FALL-001, `BlockAction::Fall`). The engine drops the actions, and there is no damage | new `entity-damage.md` | S |
| 11 | Honey floor | Honey slows the entity and stops its jump | small | Has: PHY-GRV-013, PHY-GRV-015, the honey motion data | — | — |
| 12 | Magma floor | A step on magma deals 1 `hot_floor` damage to a living entity that does not step carefully. A fire-immune piglin takes no damage | small | Part: the step action exists (PHY-RCH-002). No damage | `entity-damage.md` | S |
| 13 | Health, attributes, damage | Max health 20, armor 2. For 10 ticks after a hit, a new hit deals only the amount above the last hit, unless its damage type has the tag `bypasses_cooldown`. The death animation lasts 20 ticks | small | Part: the attribute table exists (`mcrs_minecraft_entity`, `attribute.rs`). No health and no damage | `entity-damage.md` | M |
| 14 | Crowd damage | In the push of a living entity: when more than 23 pushable entities touch it, it takes 6 `cramming` damage with the chance 1/4 | small | Part: the crowd count exists (PHY-CMP-014, PHY-DEV-006, `entity_pass.rs`). No damage | `entity-damage.md` | S |
| 15 | Push between entities | The piglins in the chamber push each other | in 8 | Has: PHY-PSH-001..007 | — | — |
| 16 | Player attack | The player attacks with the interact packet. The damage uses the attack strength of the cooldown, the weapon attributes and `sharpness`. A sword with full strength sweeps: it hits other mobs near the target. A hit marks the mob as hurt by a player | small | None: the packet decodes (`mcrs_minecraft_protocol_game`, `serverbound.rs`); no handler | new `player-attack.md` | M |
| 17 | Death and experience | A mob that a player hurt in the last 100 ticks drops experience. A zombified piglin gives 5, plus 1 to 3 for each equipment item with a drop chance of 1 or less. A baby gives 2.5 times. The orbs split by value | in 18 | None | `entity-damage.md` | S |
| 18 | Experience orbs | An orb moves to the nearest player within 8 blocks. Orbs of near values merge. The player picks up an orb and gets its points | 13 % | Part: the orb motion exists (`Category::ExperienceOrb`, PHY-ORD-005, the lifetime in `entity_pass.rs`). No follow, merge or pickup | new `experience-orbs.md` | M |
| 19 | Spawn equipment | A spawned zombified piglin holds a golden sword, or a golden spear with the chance 1/20. The equipment changes its experience and its attack | — | None | `natural-spawning.md` | S |
| 20 | Scheduled ticks of the scene | 4,835 saved fire ticks, rescheduled each 30 to 39 ticks | 6 % | Part: the scheduled ticks exist (`mcrs_minecraft_simulation`, SIM-SCH-*). The save loader keeps only the fluid ticks (`mcrs_minecraft_anvil`, `chunk.rs`), and fire has no handler | `block-simulation.md` SIM-SCH-* | S |
| 21 | Random ticks of the scene | The nether columns near the player tick at speed 3 | 7 % | Has: SIM-RND-*, D1 | — | — |

The mechanics 3 and 5 to 9 are about half of the nether tick. Natural spawning is the largest single
part. A zombified piglin costs about 25 µs in each tick, most of it AI and path search.

### 3.1 Dependencies

- Damage (10, 12, 14, 16) needs health and attributes (13).
- Death and experience (17) need damage (13) and the mark of a player hit (16).
- The lure (6) and anger (9) need the goal selection (5) and the path search (7).
- The path search (7) needs the block tags of the corpus and the collision shapes. Both exist.
- Natural spawning (3) needs the mob entities (2) and the mob count of each category for the cap.
- A client sees the farm only after the mobs replicate (2).

## 4. Implementation plan

Each step ships alone and leaves the server in a working state. The order follows the dependencies
of section 3.1. Each step has an exit criterion that a test or the benchmark of section 5 checks.

| Step | Scope | Mechanics | Size | Exit |
|------|-------|-----------|------|------|
| G1 | Mob rows in the engine world: spawn and remove a zombified piglin by command, with its type, box, attributes and walker tick. Replicate it to a client (N1) | 2, 8, 15 | M | a summoned piglin falls into the chamber and a vanilla client shows it |
| G2 | `entity-damage.md`: health, invulnerability time, armor, death animation, and the damage types `fall`, `hot_floor` and `cramming`. Consume the fall and step actions and the crowd count | 10, 12, 13, 14 | M | a piglin that falls 78 blocks on honey has 5 health; crowds of 25 die at the vanilla rate |
| G3 | `natural-spawning.md`: the spawn state, the cap, the pack loop, the spawn rules of the floor and the type, the box check, despawn and the spawn equipment | 3, 4, 19 | L | the spawn rate on each platform layer matches section 2.2 within the tolerance of section 5.3 with an empty chamber |
| G4 | `pathfinding.md`: node evaluator, path types with the tags, node budget, path follower | 7, 8 | L | a piglin at the edge of a platform walks to the centre and falls into a shaft |
| G5 | `mob-goals.md`: goal selection at the AIT cadence; the egg goal, stroll and look goals; target goals and anger with alerts | 5, 6, 9 | L | without a player attack, the chamber fills to the counts of section 2.3 |
| G6 | `player-attack.md`: the attack packet, cooldown, damage, sweep, the player hit mark | 16 | M | a scripted attack each 13 ticks gives the death counts of section 2.2 |
| G7 | Experience: the drop amount, `experience-orbs.md` with follow, merge and pickup | 17, 18 | M | the orb counts near the chamber match section 2.2 |
| G8 | Saved block ticks of all kinds, and the fire tick that only reschedules when it cannot spread | 20 | S | the scheduled tick count of the scene matches section 2.4 |

G1 and G2 can start now. G3 needs G1. G4 can run in parallel with G2 and G3. G5 needs G4. G6 needs
G2. G7 needs G6. G8 is independent.

A shorter path gives a benchmark before the AI is complete. After G1, G2 and G3, the farm spawns
and the piglins stand on the platforms. That scene measures spawning and the walker tick, but not
the AI.

## 5. Benchmark

### 5.1 Scene

- Save: `~/mc-worlds/upgraded/hermitcraft10`, the nether only. Do not commit world data.
- Area: `MCRS_BENCH_AREA="minecraft:the_nether -317 -609 10"`. Radius 10 columns covers the
  simulation distance of 10 of the vanilla run.
- Game rules: the values of the save (section 1.4). Difficulty hard.
- One player in survival at `-319.5 175 -609.5`, yaw `-90`, pitch 0. The player attacks each 13
  ticks with the sword of section 2.1. The player does not move.
- Warm up for 1,200 ticks, so that the mob cap fills. Then measure for at least 12,000 ticks.

The player is a scripted player of the bench, not a network client, so that the bench can run
without a human (section 7).

### 5.2 What to measure

- The time of each tick of the nether, and its split: spawning, entity pass, AI stage, block
  domain.
- The counts of section 2.2 for each tick: zombified piglins in the dimension, on the platforms, in
  the shafts and in the chamber; experience orbs; spawns by type and height; deaths by damage type.

### 5.3 Expected values

The vanilla values of section 2.2 are the reference. Spawns and deaths are random, so compare means
over at least 12,000 ticks.

| Quantity | Vanilla | Tolerance |
|----------|---------|-----------|
| Zombified piglins in the nether | 69.2 | ±3 |
| On the platforms | 13.3 | ±3 |
| In the chamber | 26.0 | ±4 |
| Spawns for each tick | 0.098 | ±15 % |
| Spawns on the platforms for each tick | 0.078 | ±15 % |
| Player kills for each tick | 0.065 | ±15 % |
| Crowd deaths for each tick | 0.011 | ±30 % |
| Orbs near the chamber | 80.6 | ±15 |
| Vanilla nether tick on an M4 Max | 7.8 ms | — |

The tolerances are first estimates from one run. A second vanilla run with a different seed must
set them before a test uses them.

## 6. Other farms

The save has a second gold farm by Impulse at the nether position `x -1054, z -296`, with 38 saved
piglins. This document does not analyse it.

## 7. Decisions

1. The bench uses a scripted player of the bench, not a network client.
2. The bench uses the values of the run: one attack each 13 ticks with the netherite sword of
   section 2.1.
3. The farm scene excludes the overworld.
4. The fire ticks go to a separate benchmark of the block domain, not to the farm benchmark.
5. The bench uses the difficulty hard, the value of the server.
6. Each piglin has its golden sword as a flag of the bench, without item entities.
7. A second vanilla run with a different seed sets the tolerances of section 5.3 before a test
   uses them.

## 8. The measurement app

Put this file at `<world>/scripts/goldfarm.sc`. Run `/script load goldfarm`, then
`/script in goldfarm run start()` and `/script in goldfarm run stop()`. The app writes
`<world>/scripts/shared/log.txt`, one line for each nether tick.

```text
__config() -> {'scope' -> 'global', 'stay_loaded' -> true};
global_on = false; global_t = 0; global_sp = {}; global_de = {}; global_spy = {};
__track(e, new) -> (
    if (new && global_on,
        k = str(e~'type');
        global_sp:k = global_sp:k + 1;
        if (k == 'zombified_piglin', global_spy:(floor(e~'y')) = global_spy:(floor(e~'y')) + 1);
        entity_event(e, 'on_death', _(e, reason) -> (
            r = str(e~'type') + ':' + reason; global_de:r = global_de:r + 1))));
for (['zombified_piglin', 'magma_cube', 'ghast', 'enderman', 'piglin', 'strider', 'skeleton',
      'wither_skeleton', 'hoglin', 'blaze', 'experience_orb'],
    entity_load_handler(_, _(e, new) -> __track(e, new)));
start() -> (global_on = true; global_t = 0; global_sp = {}; global_de = {}; global_spy = {};
    write_file('log', 'shared_text', 'start ' + tick_time()));
stop() -> (global_on = false; write_file('log', 'shared_text', 'spawn_y ' + global_spy));
__fmt(m) -> join(';', map(keys(m), _ + '=' + m:_));
__on_tick_nether() -> (
    if (global_on,
        global_t += 1;
        line = str('%d all=%d farm=%d shaft=%d kill=%d mc=%d orbs=%d items=%d sp{%s} de{%s}',
            global_t,
            length(entity_list('zombified_piglin')),
            length(entity_area('zombified_piglin', [-317, 226, -609], [9, 29, 11])),
            length(entity_area('zombified_piglin', [-317, 187, -609], [0.5, 10, 1.6])),
            length(entity_area('zombified_piglin', [-317, 176, -609], [1.5, 1.5, 1.6])),
            length(entity_list('magma_cube')),
            length(entity_area('experience_orb', [-317, 176, -609], [6, 6, 6])),
            length(entity_area('item', [-317, 168, -609], [8, 10, 8])),
            __fmt(global_sp), __fmt(global_de));
        write_file('log', 'shared_text', line);
        global_sp = {}; global_de = {}));
```
