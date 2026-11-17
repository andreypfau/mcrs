# Mob brain: memories, sensors, activities and behaviours

Version: see `assets/minecraft/version.json`.

This document states the shared brain rules. It contains no complete mob definition.
[The sensor rules](mob-brain-sensors.md) form part of this specification. The vanilla registries contain 115 memory ids, 24 sensor ids and 26 activity ids.

## 0. Conventions

### 0.1 Terms

- **Brain**, **memory**, **sensor**, **behaviour**, **AI step**, **guidance**, **path** and
  **line of sight**: as [entity-ai-timing.md](entity-ai-timing.md), section 0.1, defines them.
- **Decision**, **reflex**, **allowed delay**, `max_lag`: as
  [entity-ai-timing.md](entity-ai-timing.md), sections 2 and 3, define them.
- **Body**: the mob whose brain acts.
- **Brain update**: one eligible execution of AIT-VAN-017 for the body.
- **Memory kind**: an entry of the registry `memory_module_type`, with its value kind and its
  persistence.
- **Registered**: a memory kind belongs to the set of permitted kinds of this brain.
- **Present**, **absent**: a registered memory has a value, or has no value.
- **Expiry**: the remaining number of brain updates associated with a value.
- **Permanent**: a memory value with no finite expiry.
- **Activity**: a named set of behaviours, entry requirements and an exit-erasure list.
- **Core activity**: an activity that accompanies the selected activity.
- **Default activity**: the fallback for a failed explicit activity request.
- **Priority**: an integer for a behaviour. Smaller integers run first across all activities.
- **Entry requirement**: a memory kind paired with a required status.
- **Species start test**: the start test of a behaviour after its entry requirements.
- **Running**, **stopped**: the two lifecycle states of a behaviour.
- **Instant behaviour**: a behaviour whose action occurs in its start attempt.
- **Group**: a behaviour that chooses and runs child behaviours.
- **Species data**: a value that a shared rule takes as a parameter, with a default. Section 7
  gives the vanilla values.
- **Species refresh**: a species event that rebuilds the brain of a live body (BRAIN-VAN-008).
- **Feet point**, **eye point**, **feet distance**, **home**, **walk value**: as
  [mob-goals.md](mob-goals.md), section 0.1, defines them.
- **Speed multiplier**: the factor that scales the movement speed for a walk target.
- **Tracked target**: a fixed point or an entity-relative point that supplies its current position.
- **Visibility result**: an ordered entity set with per-candidate visibility answers for one scan.
- **Game time**: the dimension's tick count, independent of a schedule clock's phase.
- **Schedule phase**: the phase of the clock that the sampled timeline follows.
- **Skipped update**: a game tick in which the body exists but has no brain update (BRAIN-VAN-005).
- **World random source**: the random source shared by the mobs of one dimension.
- **Body random source**: the random source that one body owns.
- **Own**: the body's memories, behaviour state, activity state and guidance.

### 0.2 Evidence and scope

Provenance labels follow [registry.md](registry.md), section 0.3.


The `source_suggested` rules and clauses come from version `26.3`, world version `5023`.
A comparison with the target found one difference that these rules depend on: the stuck window of
AIT-VAN-012. BRAIN-VAN-035 uses the target window through AIT-VAN-012 (U15).
The comparison found no other difference in the BRAIN rules or in AIT-VAN-015 through AIT-VAN-017.
This comparison does not raise provenance or confidence.

Species actions, path search and physics stay in their own specifications. [mob-goals.md](mob-goals.md), section 2, defines the
shared controls.

### 0.3 Kind, confidence and fidelity

Each rule states provenance, kind, confidence and fidelity. Kinds follow [README.md](README.md).

- `exact`: observable values, boundaries and order must match the stated rule.
- `distribution`: support, probabilities and independence must match; the random generator is free.
- `envelope`: the result must stay within the stated set of allowed outcomes.

An exact rule can contain random choices. Such choices have distribution fidelity. No rule
requires a particular generator or process seed.
Rule ids use `BRAIN-VAN`, `BRAIN-DES` or `BRAIN-DEV`; assigned ids must not be reused.
Unless a table says otherwise, table facts inherit the metadata of their containing rule.

### 0.4 Library contract

A proxy, a bot, a save editor, a world viewer and a minigame server with custom mobs use these
rules. The vanilla mobs are one user of them.

**BRAIN-DES-002** (`design`, semantic, confidence: medium, fidelity: exact).
The brain contract has these layers. A user must be able to take each layer without the layers
after it.

1. The codec of `Brain.memories` (BRAIN-VAN-006, BRAIN-VAN-007) is game data. It must work without
   the engine, without Bevy and without a body. A save tool must be able to decode and encode it.
   Decode followed by encode must keep each valid entry, its value and its `ttl`.
2. The memory store with expiry (BRAIN-VAN-001 through BRAIN-VAN-004) is a rules item without Bevy.
   The filter of a body's registered set belongs to this layer, not to the codec.
3. Activity selection and the behaviour lifecycle (sections 3 and 5) form a kernel without Bevy.
4. Species data is vanilla data that the generic rules take as parameters. It includes the default
   activity, the exit-erasure lists, the duration ranges and the update-eligibility exceptions.

The engine drives layers 2 and 3 once in each brain update. The vanilla behaviour is a removable
vanilla plugin outside the server crate. Implementation review must check that location and
that the server has no copy of the rules.

**BRAIN-DES-003** (`design`, semantic, confidence: medium, fidelity: exact).
The sets of memory kinds, sensor types and activities must be open to a library user.
Their ids come from the registries `memory_module_type`, `sensor_type` and `activity`, not from a
closed list. A user can add a memory kind in its own namespace, with its own value codec and
persistence. A user can add a sensor type and an activity in the same way. The tables of this
specification give the vanilla entries.

## 1. Memories

**BRAIN-VAN-001** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
The brain must register the union of explicit memory kinds, sensor-declared kinds and behaviour
entry requirements. Requirements of children and inactive activities also contribute. Activity
entry conditions and exit-erasure lists alone must not register a kind. A read or a write alone
must not register it. Writes, erasure, activity changes and saved values must not enlarge the set.
A replacement brain must determine its set again. The registered set of a body is species data,
and its default is this union.

BRAIN-VAN-011 in [the sensor rules](mob-brain-sensors.md#1-cadence-and-world-queries)
lists the sensor-declared kinds for each common sensor.

Each present memory must have the value kind below. Unqualified ids in this table use `minecraft:`.
Integer payload counters and timestamps must not get automatic countdown semantics from their
kind. Only expiry has the shared countdown of BRAIN-VAN-004.

| Id | Value kind | Persisted |
| - | - | - |
| `home` | global position | yes |
| `job_site` | global position | yes |
| `potential_job_site` | global position | yes |
| `meeting_point` | global position | yes |
| `mobs` | list of living entities | no |
| `visible_mobs` | visibility result | no |
| `nearest_players` | list of players | no |
| `nearest_visible_player` | player reference | no |
| `nearest_visible_targetable_player` | player reference | no |
| `nearest_visible_targetable_players` | list of players | no |
| `walk_target` | tracked position or entity, binary32 speed multiplier, integer arrival distance in blocks | no |
| `look_target` | tracked position or entity | no |
| `attack_target` | living entity reference | no |
| `ride_target` | entity reference | no |
| `path` | computed path | no |
| `nearest_bed` | block position | no |
| `hurt_by` | damage source | no |
| `hurt_by_entity` | living entity reference | no |
| `avoid_target` | living entity reference | no |
| `cant_reach_walk_target_since` | signed 64-bit integer | no |
| `golem_detected_recently` | boolean | yes |
| `danger_detected_recently` | boolean | yes |
| `last_slept` | signed 64-bit integer | yes |
| `last_woken` | signed 64-bit integer | yes |
| `last_worked_at_poi` | signed 64-bit integer | yes |
| `nearest_visible_adult` | living entity reference | no |
| `nearest_visible_wanted_item` | item entity reference | no |
| `play_dead_ticks` | integer | yes |
| `tempting_player` | player reference | no |
| `temptation_cooldown_ticks` | integer | yes |
| `gaze_cooldown_ticks` | integer | yes |
| `long_jump_cooling_down` | integer | yes |
| `has_hunting_cooldown` | boolean | yes |
| `ram_cooldown_ticks` | integer | yes |
| `is_in_water` | unit | yes |
| `is_pregnant` | unit | yes |
| `is_panicking` | boolean | yes |
| `visited_block_positions` | set of global positions | yes |
| `unreachable_transport_block_positions` | set of global positions | yes |
| `charge_cooldown_ticks` | integer | yes |
| `attack_target_cooldown` | integer | yes |
| `angry_at` | UUID | yes |
| `universal_anger` | boolean | yes |
| `admiring_item` | boolean | yes |
| `admiring_disabled` | boolean | yes |
| `hunted_recently` | boolean | yes |
| `celebrate_location` | block position | no |
| `roar_target` | living entity reference | no |
| `disturbance_location` | block position | no |
| `recent_projectile` | unit | yes |
| `is_sniffing` | unit | yes |
| `is_emerging` | unit | yes |
| `roar_sound_delay` | unit | yes |
| `dig_cooldown` | unit | yes |
| `roar_sound_cooldown` | unit | yes |
| `sniff_cooldown` | unit | yes |
| `touch_cooldown` | unit | yes |
| `vibration_cooldown` | unit | yes |
| `sonic_boom_cooldown` | unit | yes |
| `sonic_boom_sound_cooldown` | unit | yes |
| `sonic_boom_sound_delay` | unit | yes |
| `liked_player` | UUID | yes |
| `liked_noteblock` | global position | yes |
| `liked_noteblock_cooldown_ticks` | integer | yes |
| `item_pickup_cooldown_ticks` | integer | yes |
| `sniffer_explored_positions` | list of global positions | yes |
| `breeze_jump_cooldown` | unit | yes |
| `breeze_shoot` | unit | yes |
| `breeze_shoot_charging` | unit | yes |
| `breeze_shoot_recover` | unit | yes |
| `breeze_shoot_cooldown` | unit | yes |
| `breeze_jump_inhaling` | unit | yes |
| `breeze_jump_target` | block position | yes |
| `breeze_leaving_water` | unit | yes |

The table lists all 52 persisted vanilla kinds, and the 22 transient kinds that a rule of this
specification or of the sensor rules uses.

**BRAIN-VAN-002** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
Status tests must give these results. An entry requirement on an unregistered kind must fail.

| Test | Registered, present | Registered, absent | Unregistered |
|------|---------------------|--------------------|--------------|
| Present | true | false | false |
| Absent | false | true | false |
| Registered | true | true | false |

A write or an erasure for an unregistered kind must have no effect.

**BRAIN-VAN-003** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
A write must replace both value and expiry, even when the value does not change. A write without
expiry must make the value permanent. Erasure or a write of no value must remove the value and
its finite expiry. An empty list or set must erase the memory, also after load.

False, zero and a unit flag must count as present values. An empty visibility result must also
count as present.

Later operations in the same update must see each write immediately. The last
operation must determine the value and expiry. A write after the expiry phase must keep its
full new expiry until the next update. Storage alone must not validate a referenced entity's
life, dimension or loaded state. Only an explicit later action can reject that stale reference.

**BRAIN-VAN-004** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
Expiry must be a signed 64-bit count. The value `9223372036854775807` must mean permanent.
Every other representable value must be finite, including zero and negative values.

At the expiry phase, the brain must erase a finite value whose expiry is already at most zero.
Otherwise, it must subtract exactly one. Subtraction to zero must leave the value present for this
update. Reads and saves must not decrease expiry or hide a zero-duration value.

For an inside-update write at tick T with duration N >= 0, the value must survive through T+N.
It must first be absent at T+N+1, if every later game tick updates the brain.
For a write before tick T's expiry phase with N >= 1, absence must begin at T+N.
A duration at most zero must survive only until the next expiry phase. Skipped updates must
stretch these game-time intervals; no catch-up subtraction is permitted.

| Write of duration 2 at tick T | Immediately after write | After next update | After second update | After third update | First absent at game tick |
|-------------------------------|-------------------------|-------------------|---------------------|--------------------|---------------------------|
| After the expiry phase | 2 | 1, present | 0, present | absent | T+3 |
| Before the expiry phase | 2 | 1, present | 0, present | absent | T+2 |

The rows share the update-count result. The next update is at T+1 in the first row and at T in
the second row, so the first absent tick differs.

**BRAIN-VAN-005** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
Without a brain update, expiry and sensor countdowns must not advance.
Each condition below must prevent a brain update:

- `NoAI:1b`;
- an unloaded chunk or a chunk without entity ticks;
- client control of a mount;
- death, unless the species data "update while dying" permits it;
- a species pause state.

The species data defaults are: no update while dying, and no pause state. Section 7 gives the
vanilla values.

`/tick freeze` must give skipped updates to ordinary mobs; `/tick step N` must permit N eligible
updates. A direct or indirect player passenger can exempt entity ticking from `/tick freeze`. Client control
can still prevent that entity's server AI. External events can still write a memory without a
brain update. A schedule clock change alone must not change expiry.

**BRAIN-VAN-006** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
A living entity's save must contain `Brain:{memories:{...}}`, including the empty form.
Only registered, present memories of a persisted kind must appear in `memories`.
Each entry's key must be the namespaced memory id. Its compound must contain `value` and,
for finite expiry, `ttl` as an NBT long. `ttl` must mean remaining duration, not an absolute tick.
Zero and negative finite durations must be saved. Permanent values must omit `ttl`.

```snbt
{Brain:{memories:{"minecraft:has_hunting_cooldown":{value:1b,ttl:2L}}}}
```

| Persisted kind | NBT shape of `value` |
|----------------|----------------------|
| Boolean | byte `0b` or `1b` |
| Integer | signed 32-bit int |
| Long integer | signed 64-bit long |
| Unit | empty compound `{}` |
| UUID | `[I;a,b,c,d]`, four signed words, most significant word first |
| Block position | `[I;x,y,z]`, three signed block coordinates |
| Global position | `{dimension:"minecraft:overworld",pos:[I;x,y,z]}` |
| List of global positions | ordered list of global-position compounds, with duplicates |
| Set of global positions | list of global-position compounds, without duplicate values |

Entry and set order are unspecified. Lists must keep order and duplicates. Global positions
must keep their dimension when the body changes dimension. Live entity references must not
become saved UUID memories.

Active activities, behaviour states, sensor phases and retry records must not appear in `Brain`.
A species can carry one memory outside `Brain` (section 7).

**BRAIN-VAN-007** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
A usable `Brain.memories` compound must create a fresh brain (BRAIN-VAN-008). Omitted values must
not survive. The loader must ignore valid persisted kinds that this body does not register.
It must diagnose individually each id that no registered memory kind declares (BRAIN-DES-003),
each transient kind and each undecodable entry. Other valid entries must survive. An empty valid
compound must load as empty memories.

The loader must keep recoverable parts. A UUID array longer than four must keep its first four
words. A block-position array longer than three must keep its first three coordinates.
Short arrays must fail. Position collections must drop invalid elements, keep recoverable
ones and collapse set duplicates. Such recovery must produce a diagnostic.

Boolean values must accept numeric zero as false and other numeric values as true.
A unit must accept any compound and ignore its contents. A noncompound unit must fail.
A global position must not require an existing world or destination block at load.

Absent or nonnumeric `ttl` must mean permanent. Numeric NBT types must convert to a signed 64-bit
duration. In-range finite fractions must truncate toward zero. No positive-duration clamp is
permitted. U7 covers out-of-range and nonfinite conversion. A zero or negative loaded
duration must stay present until the next expiry phase.

When `Brain` is absent, the brain must not change. An unusable outer compound, which
includes absent or noncompound `memories`, must produce a diagnostic and keep the current state.
An entity data edit reloads its saved data, so a usable `Brain` in the result replaces the brain.
A merge combines compounds.

**BRAIN-VAN-008** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
Each load with a usable `Brain` must create a fresh brain. This includes unload and reload,
travel to another dimension and an entity data edit. The fresh brain has these properties:

- memories: only the loaded memories with the expiry that they have left; transient memories are lost;
- active activities: the core ids plus the default activity, then the species initial requests;
- every behaviour stopped, with no stop action for the old running behaviours;
- fresh sensor phases (BRAIN-VAN-010), and no bed retry records.

After an entity data edit, navigation must keep a route that a dropped walk behaviour started.
After an unload and reload or a travel to another dimension, the body has no route.
A teleport within one dimension must keep the brain.

A species refresh (section 7) must first stop the running behaviours with their stop effects.
It must then keep the persisted values that remain and create fresh activity, behaviour and sensor
state. On a load, the fresh brain replaces the old brain before the species refresh. A conversion
must create the initial brain of the destination. It must not transfer the whole old memory set.
U4 covers anger transfer.

## 2. Tick placement and sensors

**BRAIN-VAN-009** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
The body must place its brain update as AIT-VAN-001 specifies. AIT-VAN-017 defines the four
brain phases; AIT-VAN-016 defines sensor periods. These rules apply once per eligible AI step,
without the two-tick alternation of goal selection. BRAIN-VAN-004 defines expiry boundaries.
[Sensor rules BRAIN-VAN-010 through BRAIN-VAN-020](mob-brain-sensors.md) define scans and queries.

Navigation precedes the brain. Move, look and jump controls follow it, in that order.
A new route must first receive path-follow progress in the next AI step. A new look or jump
request can reach its control in the current AI step. Species selection outside the brain
must not cause a second start pass in that update.

**BRAIN-DES-005** (`design`, semantic, confidence: medium, fidelity: envelope).
The tick boundaries of this specification are exact in the exact-parity mode (AIT-LAG-011).
In the other modes, sensor scans, behaviour starts, behaviour ticks and path searches are
decisions (AIT-DEC-001). Their allowed delay (AIT-DEC-003, AIT-LAG-002) can move these boundaries:

- each run of a sensor scan (BRAIN-VAN-010), by up to its period;
- the update in which a behaviour starts, ticks or stops (BRAIN-VAN-028 through BRAIN-VAN-037);
- the update in which an activity request applies (BRAIN-VAN-022 through BRAIN-VAN-025). The
  request has the allowed delay of the brain start check or brain tick that issues it
  (AIT-DEC-003). A schedule request has the allowed delay of the behaviour that reads the schedule;
- the route of a walk start or a recompute (AIT-DEC-005).

A duration counts from the application (AIT-LAG-010). A delay must not move these parts:

- the eligibility of a brain update (BRAIN-VAN-005);
- the expiry countdown and erasure (BRAIN-VAN-004);
- the 21-tick minimum between accepted schedule requests (BRAIN-VAN-025), in game time between
  the applications of those requests;
- the save and load layout (BRAIN-VAN-006 through BRAIN-VAN-008);
- the reflexes: path follow, look and jump controls (AIT-RFX-001), and flotation (BRAIN-VAN-038,
  AIT-RFX-005).

## 3. Activities

**BRAIN-VAN-021** (`source_suggested`, semantic, confidence: low, fidelity: exact).
An activity must have ordered behaviours at integer priorities, entry requirements and an
exit-erasure list. Entry requirements must form a conjunction of BRAIN-VAN-002 status tests.
An unavailable activity must fail eligibility; an available activity without requirements must
pass. Core activities must accompany the selected id without an entry test. Duplicate ids must
count only once. A request for an already-active core id must not remove the other active id.

The default activity and the core set are species data. Their defaults are `minecraft:idle` and
`minecraft:core`. Before the species initial requests, the active set must contain the core ids
plus the default. A change of the default alone must not change active activities. Default
selection must not test its conditions.

Entry conditions must gate requests, not continued membership. An activity must not reserve the
control flags of goals, and its priority alone must not preempt a behaviour.

**BRAIN-VAN-022** (`source_suggested`, semantic, confidence: low, fidelity: exact).
A transition to an already-active target must do nothing. Otherwise, it must erase the listed
exit memories of every old active id other than the target. This includes core ids that remain
core. It must then select the core ids plus the target, without another eligibility test.
It must not stop or restart running behaviours. Only the exit lists must erase memories;
no generic erasure of walk, look or path memories is permitted.

**BRAIN-VAN-023** (`source_suggested`, semantic, confidence: low, fidelity: exact).
An explicit request must test the requested activity, even when it is already active.
It must choose that id on success. On failure, it must choose the current default without
a test of the default's availability or conditions. It must then apply BRAIN-VAN-022.

**BRAIN-VAN-024** (`source_suggested`, semantic, confidence: low, fidelity: exact).
An ordered-list request must test candidates from first to last. It must select the first
eligible id and apply BRAIN-VAN-022. An already-active eligible candidate must end the search.
An empty list or no eligible candidate must leave the active set and memories unchanged.
It must not fall back implicitly. A post-brain request can use this update's new memories;
new starts at priorities that already passed must wait for the next update.

**BRAIN-VAN-027** (`design`, semantic, confidence: medium, fidelity: exact).
Both start and running passes must order behaviours by numeric priority first.
At each priority, activities must take turns in the order in which they first have a behaviour at
that priority. Within one activity and priority, behaviours must keep declaration order.
BRAIN-DEV-003 records the reference order.

For starts, active membership must be tested at the beginning of each activity's turn at that
priority. A transition inside that turn must not cancel its later attempts. A newly enabled
activity can receive later turns in this pass; passed turns must wait. The running pass must
include the participants that are running when that pass begins, even in inactive activities.
A transition during that pass must not remove later participants or repeat the start pass.

**BRAIN-VAN-039** (`source_suggested`, semantic, confidence: low, fidelity: exact).
The exit-erasure list of an activity is species data, with an empty default. The vanilla nonempty
lists are the data below. Each row also requires the listed memory to be present at entry. All
other vanilla activity and body pairs have empty lists, including all core and villager activities.

| Activity id | Bodies | Memory erased |
|-------------|--------|---------------|
| `minecraft:fight` | axolotl, hoglin, piglin, piglin brute, warden, zoglin | `minecraft:attack_target` |
| `minecraft:celebrate` | piglin | `minecraft:celebrate_location` |
| `minecraft:admire_item` | piglin | `minecraft:admiring_item` |
| `minecraft:avoid` | hoglin, piglin | `minecraft:avoid_target` |
| `minecraft:ride` | piglin | `minecraft:ride_target` |
| `minecraft:play_dead` | axolotl | `minecraft:play_dead_ticks` |
| `minecraft:tongue` | frog | `minecraft:attack_target` |
| `minecraft:sniff` | warden | `minecraft:is_sniffing` |
| `minecraft:investigate` | warden | `minecraft:disturbance_location` |
| `minecraft:roar` | warden | `minecraft:roar_target` |
| `minecraft:emerge` | warden | `minecraft:is_emerging` |

## 4. Schedules

**BRAIN-VAN-025** (`source_suggested`, semantic, confidence: low, fidelity: exact).
A schedule request at game time G must be ignored when G-L <= 20, where L is the last accepted
request time. An accepted request must set L=G before it samples the attribute. Initially L must be
-9999. The earliest request after one at T must be T+21. Clock changes must not reset this
interval. The interval limits requests; it does not guarantee a request every 21 ticks.

The request must sample the chosen environment attribute at the body's exact feet point.
Without a chosen attribute, it must use `minecraft:idle`. If the sampled id is already active,
it must do nothing without an entry test. Otherwise it must use BRAIN-VAN-023.
Failed entry or an unchanged result must still consume the accepted interval.

These parts are species data:

- the attribute that a body chooses, and when it chooses again; the default is no attribute;
- the behaviours and species actions that issue schedule requests, with their activities and
  priorities; the default is none.

A source outside this species data must not issue a schedule request. Section 7 gives the
vanilla values.

**BRAIN-VAN-026** (`source_suggested`, semantic, confidence: low, fidelity: exact).
Both schedule attributes must default to `minecraft:idle` (ENVA-TYPE-006). The sampled values
come from the data-pack timeline `minecraft:villager_schedule`, not from a list in the code.
For the pinned data pack, the timeline repeats on the clock `minecraft:overworld` every 24000
clock ticks and gives the table below. Each interval includes its start and excludes its end
(ENVA-TL-003).

| Schedule phase | `minecraft:gameplay/villager_activity` | `minecraft:gameplay/baby_villager_activity` |
|----------------|----------------------------------------|---------------------------------------------|
| `[0,10)` | `minecraft:rest` | `minecraft:rest` |
| `[10,2000)` | `minecraft:idle` | `minecraft:idle` |
| `[2000,3000)` | `minecraft:work` | `minecraft:idle` |
| `[3000,6000)` | `minecraft:work` | `minecraft:play` |
| `[6000,9000)` | `minecraft:work` | `minecraft:idle` |
| `[9000,10000)` | `minecraft:meet` | `minecraft:idle` |
| `[10000,11000)` | `minecraft:meet` | `minecraft:play` |
| `[11000,12000)` | `minecraft:idle` | `minecraft:play` |
| `[12000,24000)` | `minecraft:rest` | `minecraft:rest` |

The dimension types of the pinned data pack apply this timeline in the overworld, overworld caves,
Nether and End. It follows the Overworld clock in each; a dimension's default clock must not
replace that clock. A pause of that clock must hold the schedule phase while the game-time request
interval continues. Data-pack environment layers can replace the sampled values; the brain must
use those values.

The activities of a body and their entry requirements are species data (BRAIN-VAN-021).
A sampled id that the body does not have must fail as unavailable and fall back to the default
(BRAIN-VAN-023). An already-active scheduled id must skip the entry test. Section 7 gives the
villager activities and requirements.

## 5. Behaviours

**BRAIN-VAN-028** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
A new behaviour must be stopped. A stopped behaviour in an active activity must test all entry
requirements before its species start test. Failure must prevent the start action.
A successful timed start must become running and choose its duration before its start action.
Requirements must not become generic continuation conditions. Concurrent behaviours
can write the same memory or guidance value; their ordered last write must prevail.

At a running step, timeout must be tested first. Without timeout, the continuation condition
must be tested. Failure of either must stop the behaviour and perform its stop action.
Only success must perform the tick action. A behaviour without a continuation condition must
start and stop in the same update, without a tick action.

**BRAIN-VAN-029** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
An ordinary timed start at game time T must choose an integer D uniformly from inclusive bounds.
The default bounds must be 60..60. The duration choice has distribution fidelity (section 9).

The timeout test must be current game time > T+D. With continuous allowed continuation,
tick actions must occur at T through T+D inclusive. Stop must occur at T+D+1.
The next ordinary start opportunity must be T+D+2 because the start pass precedes the stop.
D=0 must permit a tick action at T. A behaviour without a continuation condition can start again
at T+1.

Skipped updates must not execute intermediate actions. The deadline must still use game time;
an overdue behaviour must stop when it next receives a running step. A behaviour with its own
timeout condition can define a different deadline. Groups must have no independent deadline.

**BRAIN-VAN-030** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
An instant behaviour must act in its start attempt. It has no tick action, no stop action and no
duration. It can act at most once in each brain update, and again in the next update. Several
instant behaviours can act in the same update. A failed attempt has no effect.

**BRAIN-VAN-031** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
A group must start when its own entry requirements pass, even when no child starts.
Its children can start only in the update in which the group starts. A stopped child must not
start again before the group stops and starts again.

**BRAIN-VAN-032** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
Each running child must use its own timeout and continuation rules. The group must stop in the
update in which its last running child stops. A group with no running child after its start
pass must stop in that update and can start again in the next update.

A group stop must perform the stop action of each running child and erase exactly the group's
exit memories, in that update. An empty activation must also erase them. The exit set must not
implicitly include entry requirements or child requirements.

**BRAIN-DES-004** (`design`, semantic, confidence: medium, fidelity: distribution).
Species data composes behaviours into groups. The kernel can represent a group in any form.
It must give these results:

- Ordered choice tries the children in declaration order.
- Weighted choice draws a fresh order at each group start. Each next child comes from the
  children that remain, with probability proportional to its positive weight.
- First-success mode tries stopped children until the first child that starts.
- Every-child mode tries each stopped child.
- Both modes skip running children.
- An instant child that acts counts as a child that starts.

**BRAIN-VAN-033** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
Inactive activities must prevent new starts but must not cancel running behaviours.
A forced stop-all must include running behaviours of inactive activities and running group
children. It must apply ordinary stop effects and must not stop an already-stopped behaviour.
Skipped updates alone must not perform a forced stop. A behaviour whose own continuation requires
an active activity must stop at its first running step after that activity becomes inactive.
A transition in the start pass thus stops it in the same update. Section 7 gives the vanilla
sleep case.

## 6. Walk, look and flotation

**BRAIN-VAN-034** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
Walk guidance must require all these memory statuses:

- present `minecraft:walk_target`;
- absent `minecraft:path`.

A walk target must contain a tracked point, a binary32 speed multiplier s and an integer arrival
distance C. Arrival must mean Manhattan block distance <= C between floored feet and target
coordinates. Negative coordinates must floor. An entity target must use the feet point of that
entity, unless the target explicitly names its eye point.

After any retry pause, start must test arrival, then ask for a direct route. Any returned route
can start movement, including a partial route. Only no route must trigger BRAIN-VAN-036.
Arrival or failure of both route requests must fail start and erase the walk target.
Arrival at start must also erase the failure timestamp. A failed start must not create a retry
pause.

A successful start must write path memory and request navigation with multiplier s.
The duration is species data, with the default 150..250 inclusive (section 7).

**BRAIN-VAN-035** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
Walk continuation must require all these conditions:

- a kept route and target anchor;
- unfinished navigation;
- a present walk target;
- non-arrival;
- a target that is not a spectator entity.

Each tick must synchronize `minecraft:path` with navigation's route, including absence.
Target-block displacement squared > 4 from the last successful route target must request a
new route. Equality at 4 or a speed-only change must not trigger it.
A successful recomputation must update the route, target anchor and speed multiplier.
A failed recomputation must not immediately replace navigation's route. When no replacement
remains, the next continuation check fails.

A direct computation that reaches or can reach the target must clear
`minecraft:cant_reach_walk_target_since`. Otherwise, it must write current game time only when
that memory is absent. Intermediate success must not clear an older failure timestamp.

Every stop must cancel navigation and erase the current walk and path memories, including
replacement targets. It must not erase the failure timestamp. Arrival at continuation
alone must not clear an older failure timestamp.

A stop with a present, unreached walk target and stuck navigation (AIT-VAN-012) must choose K
uniformly in 0..39 before cleanup. Other stops and failed initial route searches must not create a
retry pause. Eligible later start attempts must spend one pause unit and fail while K>0.
An inactive activity, absent walk target or failed entry condition must not spend the pause.

For K=3 at S, attempts S+1..S+3 must fail and S+4 can compute a route, if all are eligible.
For K=0, the next eligible attempt can compute. U5 and U15 state its limits.

The move request must use s times effective `minecraft:movement_speed` (GOAL-CTL-002).
It must not mean blocks per tick. Airborne flight can use `minecraft:flying_speed`.
A cancellation after navigation must not undo a move-control request already issued this step.

**BRAIN-VAN-036** (`source_suggested`, semantic, confidence: medium, fidelity: distribution).
After no direct route, the intermediate destination search must try exactly 10 samples.
Without home bias, candidates must stay in horizontal offsets [-10,10] and vertical [-7,7].
The distribution must be a forward angular sample toward the target, not a uniform box sample.

The horizontal offset `(x,z)` must be uniform by area over the half disk of radius `10*sqrt(2)`
blocks whose central direction is the horizontal target direction `(dx,dz)`.
Samples outside the horizontal limits must fail.
Otherwise y must be a uniform integer in [-7,7], and the sample must use floored x and z.

A body that has a home (section 0.1) must supply independent x and z biases, each uniform on
[0,5) blocks. The bias does not depend on the home restriction of GOAL-POS-001 or on the memory
`minecraft:home`.
The sign must follow GOAL-POS-002, step 2, with horizontal distance 10.
Each axis must compare the body's feet coordinate with the home's integer block coordinate.
If the body coordinate is greater, the search must subtract that axis's bias; otherwise, including
equality, it must add it. The search must then add the body's feet position and floor the final
position.

This bias can extend the unbiased box. Build limits, home restrictions and legal navigation
destinations must filter candidates. GOAL-POS-001 defines the home restriction.

The highest walk value must win, with the first sample on equal value. All ten samples must
occur before selection. At most one route request must target the selected bottom-centre point.
A direct partial route must not trigger this search.
Destination scoring outside GOAL-POS-004 stays a species hook.

**BRAIN-VAN-037** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
Look guidance must require present `minecraft:look_target` and choose duration 45..90 inclusive.
Continuation must require a present visible target. Fixed positions and nonliving entity targets
must pass this visibility test. A living target must be alive and visible in `minecraft:visible_mobs`.

Every tick must request the target's precise position. Every stop must erase the current target,
including a replacement. An entity point must be the feet point, unless the target explicitly
names the eye point. A fixed point from a block position must be the block centre. A fixed point
from an exact position must be that position.

GOAL-CTL-003, GOAL-CTL-004 and GOAL-CTL-008 define look hold and control timing.
The default yaw limit must be 10 degrees per tick and pitch limit 40 degrees.
Default pitch must reset to zero before the control aims. During navigation, the head/body limit must
be +/-75 degrees. One residual aiming update can follow the last request; idle return starts
two control updates after that request. Species controls and swimming variants stay hooks.

**BRAIN-VAN-038** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
Ordinary flotation must have no memory entry requirements. Start and continuation must require
floatable-fluid depth strictly above the threshold, or positive detected lava depth.
The default tag must be `#minecraft:entity_floatable`, which contains `#minecraft:water`.
The threshold is species data. Its default is zero for eye height < 0.4 blocks, otherwise 0.4
blocks. Equality with the threshold must not pass.

Depth must be maximum matching surface height above the hitbox bottom, not eye depth or average
depth. Ordinary unmounted measurement uses a hitbox contracted by 0.001 blocks per face.
Vehicle-dependent fluid measurement stays a physics hook. Duration must be 60..60.

Each allowed running tick must request a jump with probability 0.8.
A timed-out step must not request a jump.
With uninterrupted fluid, jump eligibility must include T..T+60, exclude T+61, and resume
at T+62. It must not erase walk or look memories. Jump control must consume each request
in the same step (GOAL-CTL-008).

## 7. Species data

This table gives the vanilla values of the species data that the shared rules take. A body or a
parameter that the table does not name uses the default of its rule. BRAIN-VAN-039 gives the
exit-erasure lists. The table facts have the provenance and confidence of the rule that they
name.

| Body | Parameter | Vanilla value | Rule |
|------|-----------|---------------|------|
| camel, camel husk | update while dying | yes, unless the body has a saddle and any passenger | BRAIN-VAN-005 |
| camel | pause state | grazing or rearing | BRAIN-VAN-005 |
| camel | grazing length | a natural grazing start after update T gives skipped updates at T+1 through T+50, and T+51 is a brain update, if no interruption occurs; an injected `EatingHaystack:1b` starts with 51 skipped updates | BRAIN-VAN-005 |
| happy ghast | pause state | adult | BRAIN-VAN-005 |
| villager | species refresh | age boundary, profession assignment, profession reset; each refresh chooses the schedule attribute again and requests an immediate schedule sample | BRAIN-VAN-008, BRAIN-VAN-025 |
| villager | schedule attribute | `minecraft:gameplay/baby_villager_activity` for a brain created as a baby, `minecraft:gameplay/villager_activity` otherwise | BRAIN-VAN-025 |
| villager | schedule requests | a reader behaviour at priority 99 in idle, work, play, rest and meet; the species actions of the villager | BRAIN-VAN-025 |
| villager | activities | a baby brain has no `minecraft:work`; an adult brain has no `minecraft:play` | BRAIN-VAN-021, BRAIN-VAN-026 |
| villager | entry requirements | `minecraft:work` requires present `minecraft:job_site`; `minecraft:meet` requires present `minecraft:meeting_point`; `minecraft:rest` has no entry requirement on `minecraft:home` | BRAIN-VAN-021, BRAIN-VAN-026 |
| villager | sleep continuation | requires active `minecraft:rest`, so a schedule change can wake the villager in the same update | BRAIN-VAN-033 |
| happy ghast | species refresh | maturity; it also clears all memories | BRAIN-VAN-008 |
| tadpole | conversion | the frog gets its initial brain | BRAIN-VAN-008 |
| breeze | default activity | `minecraft:fight` | BRAIN-VAN-021 |
| sniffer | walk duration, core | 500..700 | BRAIN-VAN-034 |
| villager | walk duration, play | 80..120 | BRAIN-VAN-034 |
| breeze | walk duration, slide | 20..40 | BRAIN-VAN-034 |
| breeze | flotation threshold | its eye height | BRAIN-VAN-038 |
| axolotl | bucket carrier | the bucket long `HuntingCooldown` restores `minecraft:has_hunting_cooldown` true with that duration; an absent key erases it | BRAIN-VAN-006 |

U3 leaves the camel grazing length without a capture.

## 8. Sensor queries

**BRAIN-DES-001** (`design`, semantic, confidence: medium, fidelity: envelope).
Sensor world queries must use the entity and spatial-query facilities of
[issue #98](https://github.com/andreypfau/mcrs/issues/98) and
[issue #99](https://github.com/andreypfau/mcrs/issues/99).
The facility must return results within the query contracts in BRAIN-VAN-011 through BRAIN-VAN-020.
This rule must not prescribe an index layout. Batch execution can share read-only world data.
The own memory changes that result must still appear at the specified point in each brain update.

## 9. Random sources

The table lists the random choices of the shared rules. Every random choice has distribution
fidelity. The column "Reference source" records the source that the reference game uses. The
number and order of calls to a random source are outside this specification.

| Event | Result and eligibility | Rule | Reference source |
|-------|------------------------|------|------------------|
| Brain creation | for each sensor, a uniform first-run offset in 0..P-1 | AIT-VAN-016, BRAIN-VAN-010 | body random source |
| Successful ordinary timed start | inclusive uniform duration | BRAIN-VAN-029 | world random source |
| Qualifying stuck stop | uniform integer 0..39 | BRAIN-VAN-035 | world random source |
| Intermediate destination search | for each of the 10 samples, a horizontal offset uniform by area over the half disk, then a uniform integer y in [-7,7] | BRAIN-VAN-036 | body random source |
| Intermediate destination sample of a body with a home | independent x and z biases, each uniform on [0,5) | BRAIN-VAN-036 | body random source |
| Allowed flotation running tick | jump request with probability 0.8 | BRAIN-VAN-038 | body random source |
| Weighted group start | a fresh child order, by BRAIN-DES-004 | BRAIN-DES-004 | a source that each group owns |
| Piglin brute sound check | the sound plays when a uniform fraction in [0,1) is below 0.0125 | species action | body random source |
| Villager raid entry or reset check | the check proceeds when a uniform integer in 0..19 is zero | species action | body random source |
| Due baby bed scan | uniform jitter 0..19 for retry deadlines and cleanup | BRAIN-VAN-017 | world random source |

Distribution fidelity permits an independent keyed stream for each choice (AIT-LAG-008).

Species actions can have more random choices. The specifications of species actions must state the result distributions.

## 10. Intentional deviations

### BRAIN-DEV-001: Visibility range belongs to the observer

Provenance: `design`. Kind: semantic. Confidence: medium. Fidelity: exact.

A fresh visibility or attack-range test that uses follow range must use the observer's current
follow range. It must not depend on another mob's sensor phase, follow range or dimension.

- **Rule:** BRAIN-VAN-012 and BRAIN-VAN-014 use BRAIN-DEV-001 for fresh visibility answers.
- **Reference:** between scans, a newly requested visibility answer can depend on the follow
  range of the last body whose sensor ran, including a body in another dimension.
- **Visible to the client:** yes; a head turn toward an already-listed entity can differ.

### BRAIN-DEV-002: Random generators can be independent

Provenance: `design`. Kind: semantic. Confidence: medium. Fidelity: distribution.

- **Rule:** section 9, BRAIN-VAN-029, BRAIN-DES-004 and BRAIN-VAN-035 through BRAIN-VAN-038.
- **Reference:** choices can depend on mob order and process seeds, as section 9 describes.
- **Visible to the client:** no, as a distribution. Each chance and each range stays the same.
  One choice can differ from a vanilla run with the same seed.

### BRAIN-DEV-003: Equal-priority activity order is first-appearance order

Provenance: `design`. Kind: semantic. Confidence: medium. Fidelity: exact.

- **Rule:** BRAIN-VAN-027 orders the activities at one priority by first appearance at that priority.
- **Reference:** the order depends on the number of activities that have behaviours at that
  priority, inactive activities included. It is not an activity preference.
- **Visible to the client:** only when two behaviours of different activities at one priority have conflicting effects in the same update.

## 11. Unresolved

- **U1:** work/play/meet activity membership and their actions stay proposal.
  Environment values do not expose active activity ids.
- **U2:** the transition and cancellation clauses of BRAIN-VAN-022 and BRAIN-VAN-033 have no complete observation.
  Ordinary commands cannot configure arbitrary activity sets or expose all transient states.
- **U3:** the dying camel-family exception is consistent with observation.
  The direct-passenger exemption from `/tick freeze`, saddle control, indirect passengers, the saddled dying camel with a passenger and the grazing and rearing ticks have no capture.
- **U4:** a generic conversion can carry `minecraft:angry_at` as permanent, but no natural source
  and destination pair was established. Applicability stays low; no universal transfer rule is stated.
- **U5:** a pursuit pause cannot identify every eligible attempt or exact stuck
  stop from saved NBT. Exact pause consumption and the >2-block route-change boundary need a trace.
- **U6:** entity/player encounter ties after reload, bed-query encounter order and equal-quality
  path-target choice are not measured. They must not be described as a uniform random selection.
- **U7:** nonfinite or out-of-range numeric `ttl` conversion is not specified here.
- **U8:** generic commands cannot certify complete per-mob registration of transient values.
  The provisional census must not replace the registration contract or the species hooks.
- **U9:** living-reference visibility after a referenced entity unloads needs a consumer-specific
  probe. Memory storage alone keeps the reference; it does not promise the consumer accepts it.
- **U10:** no unique activation signal identified the nearest-player scan.
  A direct memory capture or a qualified activation witness is still required.
- **U11:** generic same-update write visibility in BRAIN-VAN-003 stays proposal.
- **U12:** array recovery, unusable outer compounds and the complete registration inventory are not captured.
- **U13:** exact phase probabilities and independence of the sensor writes stay source-suggested.
- **U14:** BRAIN-DEV-001 has no measured client-visible head-turn effect. Its reference-effect classification stays provisional.
- **U15:** the stuck window of AIT-VAN-012 changed after version `26.3`. In the target, an advance
  of the next node and the acceptance of a changed path restart it; a request for the same path
  does not. The window sets the stop tick that chooses the pause of BRAIN-VAN-035.
  No capture confirms that stop tick.
