# Mob brain: memories, sensors, activities and behaviours

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition) |
| Protocol version | `1073742169` |
| World version | `5127` |

This document states the shared brain rules for
[issue #230](https://github.com/andreypfau/mcrs/issues/230). It contains no complete mob definition.
[The sensor rules](mob-brain-sensors.md) form part of this specification. Section 7 reserves the
mob-specific rules. The registries contain 115 memory ids, 24 sensor ids and 26 activity ids.

## 0. Conventions

### 0.1 Terms

- **Brain**, **memory**, **sensor**, **behaviour**, **AI step**, **guidance**, **path** and
  **line of sight**: as [entity-ai-timing.md](entity-ai-timing.md), section 0.1, defines them.
- **Body**: the mob whose brain acts.
- **Brain update**: one eligible execution of AIT-VAN-017 for the body.
- **Registered**: a memory id belongs to this brain's fixed set of permitted ids.
- **Present**, **absent**: a registered memory has a value, or has no value.
- **Expiry**: the remaining number of brain updates associated with a value.
- **Permanent**: a memory value with no finite expiry.
- **Activity**: a named set of behaviours, entry requirements and an optional exit-erasure list.
- **Core activity**: an activity that accompanies the selected activity.
- **Default activity**: the fallback for a failed explicit activity request.
- **Priority**: an integer for a behaviour. Smaller integers run first across all activities.
- **Entry requirement**: a memory id paired with a required status.
- **Running**, **stopped**: the two lifecycle states of a behaviour.
- **One-shot**: a behaviour whose action occurs in its start attempt.
- **Group**: a behaviour that selects and runs child behaviours.
- **Feet point**, **eye point**, **feet distance**, **home**, **walk value**: as
  [mob-goals.md](mob-goals.md), section 0.1, defines them.
- **Speed multiplier**: the factor that scales the movement speed for a walk target.
- **Tracked target**: a fixed point or an entity-relative point that supplies its current position.
- **Visibility result**: an ordered entity set with per-candidate visibility answers for one scan.
- **Game time**: the dimension's tick count, independent of a schedule clock's phase.
- **Schedule phase**: the Overworld clock tick modulo 24000.
- **World random source**: the random source shared by the mobs of one dimension.
- **Own**: the body's memories, behaviour state, activity state and guidance.

### 0.2 Evidence and scope

Provenance labels follow [registry.md](registry.md), section 0.3.
Captured observations and remaining proposals have separate statuses in section 12.
The captures used the target server and matching save metadata on 2026-11-11.

A rule can name a limited confirmation scope. Its confirmed metadata applies only to that scope.
Other clauses retain the provenance and confidence stated beside that scope.
A saved NBT observation does not confirm a transient value or activity id.

The `source_suggested` rules and clauses come from version `26.3`, world version `5023`.
A comparison with the target found no differences in the behaviour stated by the BRAIN-VAN and BRAIN-DEV rules.
The same applies to AIT-VAN-015, AIT-VAN-016 and AIT-VAN-017.
This comparison does not raise provenance or confidence.
Captured results confirm their stated rule scopes on the target version.

Per-mob memory inventories are provisional. BRAIN-VAN-001 defines registration; section 7 does
not certify complete per-mob inventories. Species actions, path search and physics remain in
their respective specifications. [mob-goals.md](mob-goals.md), section 2, defines the shared controls.

The implementation must expose this vanilla behaviour as a removable vanilla plugin outside the
server crate.
Implementation review must check that location and plugin registration, with no server-owned copy of
the rules.

### 0.3 Kind, confidence and fidelity

Each rule states provenance, kind, confidence and fidelity. Kinds follow [README.md](README.md).

- `exact`: observable values, boundaries and order must match the stated rule.
- `distribution`: support, probabilities and independence must match; the random generator is free.
- `envelope`: the result must stay within the stated set of allowed outcomes.

An exact rule can contain random choices. Such choices have distribution fidelity. No rule
requires a particular generator or process seed. A world seed need not give an identical random trace.
Rule ids use `BRAIN-VAN`, `BRAIN-DES` or `BRAIN-DEV`; assigned ids must not be reused.
Unless a table says otherwise, table facts inherit the metadata of their containing rule.

## 1. Memories

**BRAIN-VAN-001** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
The brain must register the union of explicit memory ids, sensor-declared ids and behaviour entry
requirements. Requirements of children and inactive activities also contribute. Activity entry
conditions and exit-erasure lists alone must not register an id. A mere read or write must not
register it. Ordinary writes, erasure, activity changes and saved values must not enlarge the set.
A replacement brain must determine its set again.

BRAIN-VAN-011 in [the sensor rules](mob-brain-sensors.md#1-cadence-and-world-queries)
lists the sensor-declared ids for each common sensor.

Each present memory must have the value kind below. Unqualified ids in this table use `minecraft:`.
Integer payload counters and timestamps must not acquire automatic countdown semantics from their
kind. Only expiry has the shared countdown of BRAIN-VAN-004.

| Id | Value kind | Persisted |
| - | - | - |
| `dummy` | no value | no |
| `home` | global position | yes |
| `job_site` | global position | yes |
| `potential_job_site` | global position | yes |
| `meeting_point` | global position | yes |
| `secondary_job_site` | list of global positions | no |
| `mobs` | list of living entities | no |
| `visible_mobs` | visibility result | no |
| `visible_villager_babies` | list of living entities | no |
| `nearest_players` | list of players | no |
| `nearest_visible_player` | player reference | no |
| `nearest_visible_targetable_player` | player reference | no |
| `nearest_visible_targetable_players` | list of players | no |
| `walk_target` | tracked position or entity, binary32 speed multiplier, integer arrival distance in blocks | no |
| `look_target` | tracked position or entity | no |
| `attack_target` | living entity reference | no |
| `attack_cooling_down` | boolean | no |
| `interaction_target` | living entity reference | no |
| `breed_target` | ageable mob reference | no |
| `ride_target` | entity reference | no |
| `path` | computed path | no |
| `doors_to_close` | set of global positions | no |
| `nearest_bed` | block position | no |
| `hurt_by` | damage source | no |
| `hurt_by_entity` | living entity reference | no |
| `avoid_target` | living entity reference | no |
| `nearest_hostile` | living entity reference | no |
| `nearest_attackable` | living entity reference | no |
| `hiding_place` | global position | no |
| `heard_bell_time` | signed 64-bit integer | no |
| `cant_reach_walk_target_since` | signed 64-bit integer | no |
| `golem_detected_recently` | boolean | yes |
| `danger_detected_recently` | boolean | yes |
| `last_slept` | signed 64-bit integer | yes |
| `last_woken` | signed 64-bit integer | yes |
| `last_worked_at_poi` | signed 64-bit integer | yes |
| `nearest_visible_adult` | living entity reference | no |
| `nearest_visible_wanted_item` | item entity reference | no |
| `nearest_visible_nemesis` | mob reference | no |
| `play_dead_ticks` | integer | yes |
| `tempting_player` | player reference | no |
| `temptation_cooldown_ticks` | integer | yes |
| `gaze_cooldown_ticks` | integer | yes |
| `long_jump_cooling_down` | integer | yes |
| `long_jump_mid_jump` | boolean | no |
| `has_hunting_cooldown` | boolean | yes |
| `ram_cooldown_ticks` | integer | yes |
| `ram_target` | 3D vector (doubles) | no |
| `is_in_water` | unit | yes |
| `is_pregnant` | unit | yes |
| `is_panicking` | boolean | yes |
| `unreachable_tongue_targets` | list of UUIDs | no |
| `visited_block_positions` | set of global positions | yes |
| `unreachable_transport_block_positions` | set of global positions | yes |
| `transport_items_cooldown_ticks` | integer | no |
| `charge_cooldown_ticks` | integer | yes |
| `attack_target_cooldown` | integer | yes |
| `spear_fleeing_time` | integer | no |
| `spear_fleeing_position` | 3D vector (doubles) | no |
| `spear_charge_position` | 3D vector (doubles) | no |
| `spear_engage_time` | integer | no |
| `spear_status` | spear state: approach, charging, retreat | no |
| `angry_at` | UUID | yes |
| `universal_anger` | boolean | yes |
| `admiring_item` | boolean | yes |
| `time_trying_to_reach_admire_item` | integer | no |
| `disable_walk_to_admire_item` | boolean | no |
| `admiring_disabled` | boolean | yes |
| `hunted_recently` | boolean | yes |
| `celebrate_location` | block position | no |
| `dancing` | boolean | no |
| `nearest_visible_huntable_hoglin` | hoglin reference | no |
| `nearest_visible_baby_hoglin` | hoglin reference | no |
| `nearest_targetable_player_not_wearing_gold` | player reference | no |
| `nearby_adult_piglins` | list of piglin-family mobs | no |
| `nearest_visible_adult_piglins` | list of piglin-family mobs | no |
| `nearest_visible_adult_hoglins` | list of hoglins | no |
| `nearest_visible_adult_piglin` | piglin-family mob reference | no |
| `nearest_visible_zombified` | living entity reference | no |
| `visible_adult_piglin_count` | integer | no |
| `visible_adult_hoglin_count` | integer | no |
| `nearest_player_holding_wanted_item` | player reference | no |
| `ate_recently` | boolean | no |
| `nearest_repellent` | block position | no |
| `pacified` | boolean | no |
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
| `sniffer_sniffing_target` | block position | no |
| `sniffer_digging` | boolean | no |
| `sniffer_happy` | boolean | no |
| `breeze_jump_cooldown` | unit | yes |
| `breeze_shoot` | unit | yes |
| `breeze_shoot_charging` | unit | yes |
| `breeze_shoot_recover` | unit | yes |
| `breeze_shoot_cooldown` | unit | yes |
| `breeze_jump_inhaling` | unit | yes |
| `breeze_jump_target` | block position | yes |
| `breeze_leaving_water` | unit | yes |

The table has 52 persisted kinds and 63 transient kinds. The dummy kind has no normal value.

**BRAIN-VAN-002** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
Status tests must give these results. An unregistered id must fail every entry requirement.

| Test | Registered, present | Registered, absent | Unregistered |
|------|---------------------|--------------------|--------------|
| Present | true | false | false |
| Absent | false | true | false |
| Registered | true | true | false |

An ordinary value or expiry request for an unregistered id must report an invalid request.
A tolerant request can distinguish unregistered from registered-but-absent. Presence and equality
tests on an unregistered id must return false. An absent registered value equals an absent value.
A write or erasure for an unregistered id must have no effect. An expiry request for an
absent registered value must return the permanent sentinel.

**BRAIN-VAN-003** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
A write must replace both value and expiry, even when the value does not change. A write without
expiry must make the value permanent. Erasure or a write of no value must remove the value and
its finite expiry. An empty list or set must erase the memory, including after load.

False, zero and a unit flag must count as present values. An empty visibility result must also
count as present.

Later operations in the same update must see each write immediately. The last
operation must determine the value and expiry. A write after the expiry phase must retain its
full new expiry until the next update. Storage alone must not validate a referenced entity's
life, dimension or loaded state. Only an explicit later action can reject that stale reference.

**BRAIN-VAN-004** (`source_suggested_black_box_confirmed`, semantic, confidence: high, fidelity: exact).
Confirmation: [E-B2](mob-brain-experiments.md#e-b2-injected-expiry-zero-boundary-freeze-and-ai-suspension) and [E-B5](mob-brain-experiments.md#e-b5-sensor-written-expiry-and-first-appearance) cover the countdown, zero boundary, sentinel and write-phase distinction.

Expiry must be a signed 64-bit count. The value `9223372036854775807` must mean permanent.
Every other representable value must be finite, including zero and negative values.

At the expiry phase, the brain must erase a finite value whose expiry is already at most zero.
Otherwise, it must subtract exactly one. Subtraction to zero must leave the value present for this
update.
Reads and saves must not decrease expiry or hide a zero-duration value.

For an inside-update write at tick T with duration N >= 0, the value must survive through T+N.
It must first be absent at T+N+1, if every subsequent game tick updates the brain.
For a write before tick T's expiry phase with N >= 1, absence must begin at T+N.
A duration at most zero must survive only until the next expiry phase. Skipped updates must
stretch these game-time intervals; no catch-up subtraction is permitted.

| Write of duration 2 | Immediately after write | After next update | After second update | After third update |
|---------------------|-------------------------|-------------------|---------------------|--------------------|
| After the expiry phase | 2 | 1, present | 0, present | absent |
| Before the expiry phase | 2 | 1, present | 0, present | absent |

The rows share the update-count result. Their game-tick labels differ because of the write phase.
[E-B2](mob-brain-experiments.md#e-b2-injected-expiry-zero-boundary-freeze-and-ai-suspension) and [E-B5](mob-brain-experiments.md#e-b5-sensor-written-expiry-and-first-appearance) test these boundaries.

**BRAIN-VAN-005** (`source_suggested_black_box_confirmed`, semantic, confidence: high, fidelity: exact).
Confirmation scope: [E-B2](mob-brain-experiments.md#e-b2-injected-expiry-zero-boundary-freeze-and-ai-suspension) covers `NoAI` and ordinary freeze.
[E-B8](mob-brain-experiments.md#e-b8-player-passenger-freeze-and-the-dying-camel-exception) covers a direct player passenger, saddle control and the three tested dying bodies.
Other clauses retain `source_suggested`, confidence: medium.

Without a brain update, expiry and sensor countdowns must not advance.
Each condition below must prevent a brain update:

- `NoAI:1b`;
- an unloaded chunk or a chunk without entity ticks;
- client control of a mount;
- ordinary death, except the camel-family case below.

`/tick freeze` must pause ordinary mobs; `/tick step N` must permit N eligible updates.
A direct or indirect player passenger can exempt entity ticking from freeze. Client control
can still prevent that entity's server AI. [E-B2](mob-brain-experiments.md#e-b2-injected-expiry-zero-boundary-freeze-and-ai-suspension) and [E-B8](mob-brain-experiments.md#e-b8-player-passenger-freeze-and-the-dying-camel-exception) separate these conditions.

Only a baby happy ghast must update its brain. Maturity must clear its memories.
A watched creaking, a sitting camel and a playing-dead axolotl must not pause solely for that state.

Camel grazing or rearing must prevent updates. A fresh natural grazing start after tick T must
skip T+1 through T+50 and permit T+51, if uninterrupted. An injected `EatingHaystack:1b` starts
with 51 skipped steps. The dying camel-family exception has the captured comparison in [E-B8](mob-brain-experiments.md#e-b8-player-passenger-freeze-and-the-dying-camel-exception).

A dying camel or camel husk must still update its brain when no other condition suppresses that update.
A dying goat must not update it. [E-B8](mob-brain-experiments.md#e-b8-player-passenger-freeze-and-the-dying-camel-exception) compares their first corpse update.

External events can still write a memory without a brain update.
A schedule clock change alone must not change expiry.

**BRAIN-VAN-006** (`source_suggested_black_box_confirmed`, semantic, confidence: high, fidelity: exact).
Confirmation scope: [E-B2](mob-brain-experiments.md#e-b2-injected-expiry-zero-boundary-freeze-and-ai-suspension) and [E-B4](mob-brain-experiments.md#e-b4-damaged-entries-ignored-registration-and-complete-replacement) cover the tested persisted entries, finite `ttl`, permanent omission and rejected transient input.
Other clauses retain `source_suggested`, confidence: medium.

A living entity's save must contain `Brain:{memories:{...}}`, including the empty form.
Only registered, present memories marked persisted in BRAIN-VAN-001 must appear in `memories`.
Each entry's key must be the namespaced memory id. Its compound must contain `value` and,
for finite expiry, `ttl` as an NBT long. `ttl` must mean remaining duration, not an absolute tick.
Zero and negative finite durations must be saved.

Permanent values must omit `ttl`.

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

Entry and set order are unspecified. Lists must retain order and duplicates. Global positions
must retain their dimension when the body changes dimension. Live entity references must not
become saved UUID memories.

Active activities, behaviour states, sensor phases and retry records
must not appear in `Brain`. An axolotl bucket's `HuntingCooldown` is a separate NBT long carrier
for the remaining hunting cooldown. Release must restore true with that duration, or erase it
when that bucket key is absent; the bucket must not represent a complete brain save.

**BRAIN-VAN-007** (`source_suggested_black_box_confirmed`, semantic, confidence: high, fidelity: exact).
Confirmation scope: [E-B4](mob-brain-experiments.md#e-b4-damaged-entries-ignored-registration-and-complete-replacement) covers complete command replacement, independent entry rejection and the listed `ttl` conversions.
Other clauses retain `source_suggested`, confidence: medium.

A usable `Brain.memories` compound must replace the previous memory state. Omitted values must
not survive that replacement. The loader must ignore valid persisted ids that this body does not
register. It must diagnose unknown ids, non-persisted kinds and undecodable entries individually.
Other valid entries must survive. An empty valid compound must load as empty memories.

The loader must keep recoverable parts. A UUID array longer than four must retain its first four
words. A block-position array longer than three must retain its first three coordinates.
Short arrays must fail. Position collections must drop invalid elements, retain recoverable
ones and collapse set duplicates. Such recovery must produce a diagnostic.

Boolean values must accept numeric zero as false and other numeric values as true.
A unit must accept any compound and ignore its contents. A noncompound unit must fail.

A global position must not require an existing world or destination block at load.

Absent or nonnumeric `ttl` must mean permanent. Numeric NBT types must convert to a signed 64-bit
duration. In-range finite fractions must truncate toward zero. No positive-duration clamp is
permitted. Out-of-range and nonfinite numeric conversion remains U7. A zero or negative loaded
duration must remain present until the next expiry phase.

A missing `Brain` must leave the existing brain unchanged. An unusable outer compound must
produce a diagnostic without replacing the current state. This includes absent or noncompound
`memories`. Villager-specific refresh can subsequently replace transient state.

An entity data edit reloads its saved data; a valid resulting `Brain` must therefore replace it.
A merge combines compounds. [E-B4](mob-brain-experiments.md#e-b4-damaged-entries-ignored-registration-and-complete-replacement) uses complete replacement to test omission.

**BRAIN-VAN-008** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
Unload/reload and travel to another dimension must retain only saved memories and their remaining
expiry. They must lose transient memories. A teleport within one dimension must keep the brain.

A villager refresh on an age boundary, profession assignment/reset or load must stop old
behaviours first. It must retain the persisted values that survive stop effects, then create
fresh activity, behaviour and sensor state. A goat's age change alone must keep its brain.
Happy ghast maturity must instead stop behaviours and clear all memories.

A tadpole-to-frog conversion must create the destination's initial brain. It must not transfer
the whole old memory set. Natural applicability of anger transfer is unresolved in U4.
A replacement brain must choose fresh sensor phases and discard bed retry records.

## 2. Tick placement and sensors

**BRAIN-VAN-009** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
The body must place its brain update as AIT-VAN-001 specifies. AIT-VAN-017 defines the four
brain phases; AIT-VAN-016 defines sensor periods. These rules apply once per eligible AI step,
without the goal selection's two-tick alternation. BRAIN-VAN-004 defines expiry boundaries.
[Sensor rules BRAIN-VAN-010 through BRAIN-VAN-020](mob-brain-sensors.md) define scans and queries.

Navigation precedes the brain. Move, look and jump controls follow it, in that order.
A new route must first receive path-follow progress in the next AI step. A new look or jump
request can reach its control in the current AI step. Species selection outside the brain
must not cause a second start pass in that update.

## 3. Activities

**BRAIN-VAN-021** (`source_suggested`, semantic, confidence: low, fidelity: exact).
An activity must have ordered behaviours at integer priorities, entry requirements and an
exit-erasure list. Entry requirements must form a conjunction of BRAIN-VAN-002 status tests.
An unavailable activity must fail eligibility; an available activity without requirements must
pass. Core activities must accompany the selected id without an entry test. Duplicate ids must
count only once. A request for an already-active core id must not remove the other active id.

The default initially must be `minecraft:idle`; breeze uses `minecraft:fight`.
Before species-specific initial requests, the active set must contain the core ids plus default.
The built-in brain cohort must use only `minecraft:core` as core.

A change of the default alone must not change active activities. Default selection must not test its
conditions.
Entry conditions must gate requests, not continued membership. Activities must not reserve the
control flags of goals or preempt behaviours merely because of priority.

**BRAIN-VAN-022** (`source_suggested`, semantic, confidence: low, fidelity: exact).
A transition to an already-active target must do nothing. Otherwise, it must erase the listed
exit memories of every old active id other than the target. This includes core ids that remain
core. It must then select the core ids plus the target, without another eligibility test.
It must not stop or restart running behaviours. Only the configured exit lists must erase
memories; no generic erasure of walk, look or path memories is permitted.

**BRAIN-VAN-023** (`source_suggested`, semantic, confidence: low, fidelity: exact).
An explicit request must test the requested activity, even when it is already active.
It must choose that id on success. On failure, it must choose the current default without
a test of the default's availability or conditions. It must then apply BRAIN-VAN-022.

**BRAIN-VAN-024** (`source_suggested`, semantic, confidence: low, fidelity: exact).
An ordered-list request must test candidates from first to last. It must select the first
eligible id and apply BRAIN-VAN-022. An already-active eligible candidate must end the search.
An empty list or no eligible candidate must leave the active set and memories unchanged.
It must not fall back implicitly. A post-brain request can use this update's new memories;
new starts at already-passed priorities must wait for the next update.

**BRAIN-VAN-027** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
Both start and running passes must order behaviours by numeric priority first.
At each priority, the activity order below must apply. Within one activity and priority,
behaviours must retain declaration order. N is the number of activities with behaviours at
that priority, including inactive activities.

Equal order numbers must retain the order in
which those activities first have a behaviour at that priority. These numbers are ordering
data, not activity-selection preferences. [E-B6](mob-brain-experiments.md#e-b6-partial-observation-of-equal-priority-activity-order) is a partial observation; U2 limits confirmation.

| Activity id | Order, N=1..12 | Order, N=13..24 | Order, N=25..26 |
| - | - | - | - |
| `minecraft:play` | 0 | 0 | 0 |
| `minecraft:dig` | 3 | 3 | 3 |
| `minecraft:tongue` | 5 | 5 | 5 |
| `minecraft:lay_spawn` | 8 | 8 | 8 |
| `minecraft:investigate` | 11 | 11 | 11 |
| `minecraft:ride` | 13 | 13 | 13 |
| `minecraft:long_jump` | 15 | 15 | 15 |
| `minecraft:roar` | 11 | 27 | 27 |
| `minecraft:fight` | 14 | 30 | 30 |
| `minecraft:play_dead` | 15 | 31 | 31 |
| `minecraft:ram` | 15 | 31 | 31 |
| `minecraft:rest` | 1 | 1 | 33 |
| `minecraft:admire_item` | 2 | 2 | 34 |
| `minecraft:idle` | 5 | 5 | 37 |
| `minecraft:celebrate` | 5 | 5 | 37 |
| `minecraft:work` | 6 | 6 | 38 |
| `minecraft:sniff` | 6 | 6 | 38 |
| `minecraft:panic` | 15 | 15 | 47 |
| `minecraft:core` | 1 | 17 | 49 |
| `minecraft:hide` | 2 | 18 | 50 |
| `minecraft:meet` | 4 | 20 | 52 |
| `minecraft:pre_raid` | 5 | 21 | 53 |
| `minecraft:emerge` | 10 | 26 | 58 |
| `minecraft:avoid` | 11 | 27 | 59 |
| `minecraft:swim` | 14 | 30 | 62 |
| `minecraft:raid` | 15 | 31 | 63 |

For starts, active membership must be tested at the beginning of each activity's turn at that
priority. A transition inside that turn must not cancel its later attempts. A newly enabled
activity can receive later turns in this pass; passed turns must wait. The running pass must
include the participants that are running when that pass begins, even in inactive activities.
A transition during that pass must not remove later participants or repeat the start pass.

**BRAIN-VAN-039** (`source_suggested`, semantic, confidence: low, fidelity: exact).
The built-in nonempty exit-erasure lists must be the data below. Each row also requires the
listed memory to be present at entry. Other built-in activity/body pairs must have empty lists,
including all core and villager activities. These data do not define complete species activities.

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

**BRAIN-VAN-025** (`source_suggested_black_box_confirmed`, semantic, confidence: high, fidelity: exact).
Confirmation scope: [E-B3](mob-brain-experiments.md#e-b3-schedule-change-with-sleep-as-the-observable) covers the 21-tick request interval and the wake consequence for both ages.
Other clauses retain `source_suggested`, confidence: low.

A schedule request at game time G must be ignored when G-L <= 20, where L is the last accepted
request time. An accepted request must set L=G before it samples the attribute. Initially L must be
-9999.
The earliest request after one at T must be T+21. Clock changes must not reset this interval.
The interval limits requests; it does not guarantee a request every 21 ticks.

The request must sample the chosen environment attribute at the body's exact feet point.
Without a chosen attribute, it must use `minecraft:idle`. If the sampled id is already active,
it must do nothing without an entry test. Otherwise it must use BRAIN-VAN-023.
Failed entry or an unchanged result must still consume the accepted interval.

A villager must choose `minecraft:gameplay/baby_villager_activity` on baby brain creation,
and `minecraft:gameplay/villager_activity` otherwise. A refresh must select again and request
an immediate sample. Ordinary readers must act at priority 99 in idle, work, play, rest and meet.
Panic/hide recovery and raid recovery can also request samples. Those species actions are hooks.
A villager must not use an unconditional post-brain schedule request.

**BRAIN-VAN-026** (`source_suggested_black_box_confirmed`, semantic, confidence: high, fidelity: exact).
Confirmation scope: [E-B9](mob-brain-experiments.md#e-b9-schedule-frame-boundaries-by-age-and-dimension) covers both interval tables and the Overworld clock effect on sleep in three dimensions.
Other clauses retain `source_suggested`, confidence: low.

Both schedule attributes must default to `minecraft:idle` (ENVA-TYPE-006).
The built-in `minecraft:villager_schedule` must repeat on `minecraft:overworld` every 24000
clock ticks. Each interval below includes its start and excludes its end (ENVA-TL-003).

| Schedule phase | Adult | Baby |
|----------------|-------|------|
| `[0,10)` | `minecraft:rest` | `minecraft:rest` |
| `[10,2000)` | `minecraft:idle` | `minecraft:idle` |
| `[2000,3000)` | `minecraft:work` | `minecraft:idle` |
| `[3000,6000)` | `minecraft:work` | `minecraft:play` |
| `[6000,9000)` | `minecraft:work` | `minecraft:idle` |
| `[9000,10000)` | `minecraft:meet` | `minecraft:idle` |
| `[10000,11000)` | `minecraft:meet` | `minecraft:play` |
| `[11000,12000)` | `minecraft:idle` | `minecraft:play` |
| `[12000,24000)` | `minecraft:rest` | `minecraft:rest` |

The universal timeline must apply in overworld, overworld caves, Nether and End dimension types.
It must follow the Overworld clock in each. A dimension's default clock must not replace that
clock. A pause of that clock must hold schedule phase while the game-time request interval continues.
Data-pack environment layers can replace these sampled values; the brain must use those values.

Work without a job-site memory must fail entry. An already-active scheduled work id must skip
that entry test. Rest entry alone must not require a home memory. [E-B3](mob-brain-experiments.md#e-b3-schedule-change-with-sleep-as-the-observable) tests a visible consequence.

## 5. Behaviours

**BRAIN-VAN-028** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
A new behaviour must be stopped. A stopped behaviour in an active activity must test all entry
requirements before its extra start condition. Failure must prevent the start action.
A successful timed start must become running and choose its duration before its start action.
Requirements must not become generic continuation conditions. Concurrent behaviours
can write the same memory or guidance value; their ordered last write must prevail.

At a running step, timeout must be tested first. Without timeout, the continuation condition
must be tested. Failure of either must stop the behaviour and perform its stop action.
Only success must perform the tick action. The default continuation must fail, so such a
behaviour must start and stop in the same update without a tick action.

**BRAIN-VAN-029** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
An ordinary timed start at game time T must choose an integer D uniformly from inclusive bounds.
The default bounds must be 60..60. The duration choice has distribution fidelity (section 9).

The timeout test must be current game time > T+D. With continuous allowed continuation,
tick actions must occur at T through T+D inclusive. Stop must occur at T+D+1.
The next ordinary start opportunity must be T+D+2 because the start pass precedes the stop.
D=0 must permit a tick action at T. A default failed continuation instead permits restart at T+1.

An AI pause must not execute intermediate actions. The deadline must still use game time;
an overdue behaviour must stop when it next receives a running step. A behaviour with its own
timeout condition can define a different deadline. Groups must have no independent deadline.

**BRAIN-VAN-030** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
A one-shot must perform its action in the start attempt. Success must make it running until
the running pass stops it in the same update, without another action. Failure must leave it
stopped. It must have no generic duration. A successful one-shot must count as a successful
child for single-success selection. Each scheduled one-shot can succeed at most once per update;
several distinct one-shots can succeed in that update.

**BRAIN-VAN-031** (`source_suggested`, semantic, confidence: medium, fidelity: distribution).
A group must test its own entry requirements, then become running even when every child fails.
Ordered selection must keep child order. This rule covers weighted groups with positive child weights w.
For each weighted selection, the child order must have the distribution below.
Let u=k/2^24 for each child, with fresh independent uniform integers k in 0<=k<2^24.
The order must equal ascending keys `-(u^(1/w))`, with `1/w` rounded to binary32.

Equal keys must retain the pre-selection order.
On later selections, equal keys must retain the previous selection's order.

Single-success mode must try stopped children until the first successful start. Try-all mode
must try every stopped child in selected order. Both must skip running children. Selection
must occur only at group start.

**BRAIN-VAN-032** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
A group running step must visit its running children in retained order. Each child must use
its own timeout and continuation rules. The group must stop after the last child stops.
It must not retry stopped children during the same activation. A group with only one-shots,
or without successful children, must stop in its starting update and can restart next update.

A group stop must first stop the group, then stop each running child in retained order,
then erase exactly its configured exit memories. An empty activation must also erase them.
The exit set must not implicitly include entry requirements or child requirements.

**BRAIN-VAN-033** (`source_suggested_black_box_confirmed`, semantic, confidence: high, fidelity: exact).
Confirmation scope: [E-B3](mob-brain-experiments.md#e-b3-schedule-change-with-sleep-as-the-observable) covers same-update wake when rest ends.
Other clauses retain `source_suggested`, confidence: medium.

Inactive activities must prevent new starts but must not cancel running behaviours.
A forced stop-all must include running behaviours of inactive activities and running group
children. It must apply ordinary stop effects and must not stop an already-stopped behaviour.
AI suspension alone must not perform a forced stop. A sleeping villager's own continuation
requires active rest, so a schedule switch can wake it in that same running pass.

## 6. Walk, look and flotation

**BRAIN-VAN-034** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
Walk guidance must require all these memory statuses:

- present `minecraft:walk_target`;
- absent `minecraft:path`;
- registered `minecraft:cant_reach_walk_target_since`.

A walk target must contain a tracked point,
a binary32 speed multiplier s and an integer arrival distance C. Arrival must mean Manhattan
block distance <= C between floored feet and target coordinates. Negative coordinates must floor.
Entity targets normally use feet; explicit eye targets must use eyes.

After any retry pause, start must test arrival, then ask for a direct route. Any returned route
can start movement, including a partial route. Only no route must trigger BRAIN-VAN-036.
Arrival or failure of both route requests must fail start and erase the walk target.
Arrival at start must also erase the failure timestamp. A failed start must not create a retry
pause.

A successful start must write path memory and request navigation with multiplier s.

Ordinary duration must be 150..250 inclusive. Sniffer core uses 500..700; villager play uses
80..120; breeze sliding uses 20..40. These are duration data, not complete species behaviours.

**BRAIN-VAN-035** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
Walk continuation must require all these conditions:

- a retained route and target anchor;
- unfinished navigation;
- a present walk target;
- non-arrival;
- a target that is not a spectator entity.

Each tick must synchronize `minecraft:path` with navigation's route, including absence.
Target-block displacement squared > 4 from the last successful route target must request a
new route. Equality at 4 or a speed-only change must not trigger it.
A successful recomputation must update the route, target anchor and speed multiplier.
A failed recomputation must not immediately replace navigation's route; no retained replacement
must fail the next continuation check.

A direct computation that reaches or can reach the target must clear
`minecraft:cant_reach_walk_target_since`. Otherwise, it must write current game time only when
that memory is absent. Intermediate success must not clear an older failure timestamp.

Every stop must cancel navigation and erase the current walk and path memories, including
replacement targets. It must not normally erase the failure timestamp. Arrival at continuation
alone must not clear an older failure timestamp.

A stop with a present, unreached walk target and stuck navigation must choose K uniformly in
0..39 before cleanup. Other stops and failed initial route searches
must not create a retry pause. Eligible later start attempts must spend one pause unit and fail while K>0.
An inactive activity, absent walk target or failed entry condition must not spend the pause.

For K=3 at S, attempts S+1..S+3 must fail and S+4 can compute a route, if all are eligible.
For K=0, the next eligible attempt can compute. [E-B7](mob-brain-experiments.md#e-b7-stuck-pursuit-pause-and-target-displacement) tests the visible bound; U5 states its limit.

The move request must use s times effective `minecraft:movement_speed` (GOAL-CTL-002).
It must not mean blocks per tick. Airborne flight can use `minecraft:flying_speed`.
A cancellation after navigation must not undo a move-control request already issued this step.

**BRAIN-VAN-036** (`source_suggested`, semantic, confidence: medium, fidelity: distribution).
After no direct route, the intermediate destination search must try exactly 10 samples.
Without home bias, candidates must stay in horizontal offsets [-10,10] and vertical [-7,7].
The distribution must be a forward angular sample toward the target, not a uniform box sample.

For horizontal target direction `(dx,dz)`, define binary32 `a = pi/2` and `q = sqrt(2)`.
Let u=k/2^24 and v=l/2^53, where k and l are independent uniform integers with 0<=k<2^24 and 0<=l<2^53.
Define
`theta = atan2(dz,dx)-a+(2u-1)*a`, `r = 10*sqrt(v)*q`, `x = -r*sin(theta)`, `z = r*cos(theta)`.
Samples outside the horizontal limits must fail.
Otherwise y must be a uniform integer in [-7,7], and the sample must use floored x and z.

A home must supply independent x and z biases, each uniform on [0,5) blocks.
The sign must follow GOAL-POS-002, step 2, with horizontal distance 10.
Each axis must compare the body's feet coordinate with the home's integer block coordinate.
If the body coordinate is greater, the search must subtract that axis's bias; otherwise it must add it.

Equality on either axis must add the bias.
These choices have distribution fidelity.
The search must then add the body's feet position and floor the final position.

This bias can extend the unbiased box. Build limits, home restrictions and legal navigation
destinations must filter candidates. GOAL-POS-001 defines the home restriction.

The highest walk value must win, with the first sample on equal value. All ten samples must
occur before selection. At most one route request must target the selected bottom-centre point.

A direct partial route must not trigger this search.
Destination scoring outside GOAL-POS-004 remains a species hook.

**BRAIN-VAN-037** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
Look guidance must require present `minecraft:look_target` and choose duration 45..90 inclusive.
Continuation must require a present visible target. Fixed positions and nonliving entity targets
must pass this visibility test. A living target must be alive and visible in `minecraft:visible_mobs`.

Every tick must request the target's precise position. Every stop must erase the current target,
including a replacement. Entity points can mean feet or eyes; block points normally mean centres.

GOAL-CTL-003, GOAL-CTL-004 and GOAL-CTL-008 define look hold and control timing.
The default yaw limit must be 10 degrees per tick and pitch limit 40 degrees.
Default pitch must reset to zero before the control aims. During navigation, the head/body limit must
be +/-75 degrees. One residual aiming update can follow the last request; idle return starts
two control updates after that request. Species controls and swimming variants remain hooks.

**BRAIN-VAN-038** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
Ordinary flotation must have no memory entry requirements. Start and continuation must require
floatable-fluid depth strictly above the threshold, or positive detected lava depth.
The default tag must be `#minecraft:entity_floatable`, which contains `#minecraft:water`.
The threshold must be zero for eye height < 0.4 blocks, otherwise 0.4 blocks.
Breeze must use its eye height instead. Equality with the threshold must not pass.

Depth must be maximum matching surface height above the hitbox bottom, not eye depth or average
depth. Ordinary unmounted measurement uses a hitbox contracted by 0.001 blocks per face.
Vehicle-dependent fluid measurement remains a physics hook. Duration must be 60..60.

Each allowed running tick must request a jump with the probability that a uniform binary32 fraction is strictly below binary32 0.8.
A timed-out step must not request a jump.
With uninterrupted fluid, jump eligibility must include T..T+60, exclude T+61, and resume
at T+62. It must not erase walk or look memories. Jump control must consume each request
in the same step (GOAL-CTL-008). Creaking's movement restriction remains a species hook.

## 7. Hooks

Each row reserves later species rules. Its scope is one line. Shared rules above still apply.
These hooks do not certify a complete sensor, activity or behaviour list for a mob.

### 7.1 Other sensors

| Sensor id | Scope |
|-----------|-------|
| `minecraft:armadillo_scare_detected` | Armadillo threats and the danger memory. |
| `minecraft:axolotl_attackables` | Aquatic targets, hunting eligibility and cooldown. |
| `minecraft:breeze_attack_entity_sensor` | Breeze nearby entities, visibility and attack targets. |
| `minecraft:dummy` | No-op sensor, unused by the built-in brain cohort. |
| `minecraft:frog_attackables` | Edible targets and unreachable-target exclusions. |
| `minecraft:golem_detected` | Iron-golem detection and recent-detection expiry. |
| `minecraft:hoglin_specific_sensor` | Repellents, nearby adults and piglin/hoglin counts. |
| `minecraft:piglin_brute_specific_sensor` | Nemeses and nearby adult piglins. |
| `minecraft:piglin_specific_sensor` | Repellents, equipment, wanted items, threats and group counts. |
| `minecraft:secondary_pois` | Profession-specific secondary block positions. |
| `minecraft:villager_babies` | Visible baby villagers. |
| `minecraft:villager_hostiles` | Villager-specific threats and their ranges. |
| `minecraft:warden_entity_sensor` | Warden nearby entities and player-first attack eligibility. |

### 7.2 Mob activities and behaviours

| Mob id | Scope |
|--------|-------|
| `minecraft:allay` | Its core/idle activities, item delivery and vibration responses. |
| `minecraft:armadillo` | Its core/idle/panic activities, threat response and rolling. |
| `minecraft:axolotl` | Its core/idle/fight/play-dead activities, hunting and play-dead recovery. |
| `minecraft:breeze` | Its core/idle/fight activities, default fight, sliding, jumps and shooting. |
| `minecraft:camel` | Its core/idle activities, temptation, riders, grazing and control limits. |
| `minecraft:camel_husk` | Its core/idle activities, riders, grazing and species restrictions. |
| `minecraft:copper_golem` | Its core/idle activities, container visits and item transport. |
| `minecraft:creaking` | Its core/idle/fight activities, observation, player reaction and movement limits. |
| `minecraft:frog` | Its core/idle/tongue/lay-spawn/long-jump/swim activities and controls. |
| `minecraft:goat` | Its core/idle/ram/long-jump activities, temptation and cooldown requirements. |
| `minecraft:happy_ghast` | Its baby core/idle/panic activities and the transition to adult goals. |
| `minecraft:hoglin` | Its core/idle/fight/avoid activities, repulsion and aggression. |
| `minecraft:nautilus` | Its core/idle/fight activities, temptation, charges, riders and swimming controls. |
| `minecraft:piglin` | Its core/idle/admire-item/fight/avoid/celebrate/ride activities and item pursuit. |
| `minecraft:piglin_brute` | Its core/idle/fight activities, anger and activity sound. |
| `minecraft:sniffer` | Its core/idle/sniff/dig activities, explored positions and digging output. |
| `minecraft:tadpole` | Its core/idle activities, concurrent goals, swimming and conversion. |
| `minecraft:villager` | Its activities by age/profession, work, play, sleep, panic, bells and raids. |
| `minecraft:warden` | Its core/idle/emerge/dig/roar/fight/investigate/sniff activities and vibrations. |
| `minecraft:zoglin` | Its core/idle/fight activities and aggression. |
| `minecraft:zombie_nautilus` | Its core/idle/fight activities, charges, riders and swimming controls. |

Entry conditions beyond BRAIN-VAN-039, sensor order and complete behaviour lists for those mobs
belong to these hooks. No row is a complete reference mob. The common registration rule still
requires all nested behaviour entry requirements, including inactive activities.

## 8. Cost and parallelism

The target is 50,000 mobs with a small per-tick cost. This table states dependencies and costs,
without a choice of index, container or execution algorithm. Own writes must retain the
order of one brain. Different brains can evaluate own-only work in parallel.

| Step | Reads | Writes | Parallel across mobs | Required order or boundary |
|------|-------|--------|----------------------|----------------------------|
| Eligibility | own age, AI/control state; entity-tick eligibility | none | yes | before brain work |
| Expiry | own present finite durations | own values and durations | yes | before this body's sensors |
| Sensor countdown | own sensor phase | own phase | yes | declared sensor order per body |
| Due sensor scan | own attributes/state; player/entity/POI/block query; damage history | own sensor results and retry facts | yes with world reads fixed | preceding sensor writes visible to later sensors |
| Visibility answers | own scan candidates and previous answers; candidate state and block rays | own answers for that scan | yes | first answer retained until the next scan |
| Start pass | own memories, activity membership, stopped state; action-specific world reads | own state/guidance; action effects | own-only actions: yes | numeric priority, activity order, declaration order |
| Running pass | own running participants, deadlines and continuation inputs; action-specific reads | own state/guidance; action effects | own-only actions: yes | after all starts; preserve participant order |
| Activity transition | own requirements, active ids, default and erase lists | own activities and listed memories | yes | immediate at its ordered request point |
| Schedule request | own last-read time, age-selected attribute; environment at feet | own last-read time and transition | yes | strict 21-tick minimum; same-body request order |
| Walk/look/float | own memories, route, geometry, fluid; target and path inputs | own guidance and memories | yes | navigation before brain; controls after brain |
| Cross-entity/world action | other entities, POI occupancy or world state | other entities or shared world | evaluation can be parallel | resolved shared effects follow AIT-CPL-001 and AIT-CPL-002 |
| Save/load/refresh | own registered set and saved values | own replacement state | yes for independent mobs | stop effects precede state preservation on villager refresh |

**BRAIN-DES-001** (`design`, semantic, confidence: medium, fidelity: envelope).
Sensor world queries must use the entity and spatial-query facilities of
[issue #98](https://github.com/andreypfau/mcrs/issues/98) and
[issue #99](https://github.com/andreypfau/mcrs/issues/99).
The facility must return results within the query contracts in BRAIN-VAN-011 through BRAIN-VAN-020.
This rule must not prescribe an index layout. Batch execution can share read-only world data.
The own memory changes that result must still appear at the specified point in each brain update.

Expiry cost depends on present finite values, not the size of the memory registry.
A sensor not due needs no world query. Due entity scans are proportional to query candidates;
there is no fixed candidate cap except the bed scan's four accepted positions.

Visibility needs at most one fresh answer per requested candidate per scan, plus block-ray work.
Behaviour work is proportional to the considered start attempts and running participants.
The ordinary schedule reader can request every update, but accepted attribute reads are at least
21 game ticks apart. Exact rules do not permit a skipped update that hides a required reaction.

## 9. Parallelism notes

The world random source is shared by the mobs of one dimension in the reference game.
The table lists choices that share this source; the two species actions remain hook boundaries.
These source facts inform parallel tick design. Every random choice has distribution fidelity.
The number and order of calls to a random source are outside this specification.
The support and conditional probability remain requirements.

| Event | Result and eligibility | Reference random source | Parallel option |
|-------|------------------------|-------------------------|-----------------|
| Successful ordinary timed start | inclusive uniform duration | world | independent stream for body, behaviour and start |
| Qualifying stuck stop | uniform integer 0..39 | world | independent stream for body and stop |
| Due baby bed scan | uniform jitter 0..19 for retry deadlines and cleanup | world | independent stream for body and scan |
| Villager raid entry/reset hook | proceed if a uniform integer in 0..19 is zero | world | independent stream for body and action |
| Piglin brute activity-sound hook, including idle checks | sound if a uniform fraction in [0,1) is <0.0125 | world | independent stream for body and sound check |

Distribution fidelity permits independent keyed streams (AIT-LAG-008).
Same-body action order and conditional results still matter for their semantic effects.

Sensor initialization, intermediate destinations and flotation tick chances use mob randomness in the reference game.
Weighted group selection has independent group randomness, with seeds that depend on the process.
Exact process seed reproduction is not required.
Species actions can have additional random choices. Their future specifications must state the result distributions.

A sensor reads other entities and blocks but writes only its body. Own sensor scans can run in
parallel over suitable world snapshots. Immediate changes within one brain must not wait for a
cross-mob merge. Actions that damage, breed, claim a POI, change another mob or change a block
require the AIT pair-action boundary. This specification does not authorize new shared writes.

## 10. Intentional deviations

### BRAIN-DEV-001: Visibility range belongs to the observer

Provenance: `design`. Kind: semantic. Confidence: medium. Fidelity: exact.

A fresh visibility or attack-range test must use the observer's current follow range.
It must not depend on another mob's sensor phase, follow range or dimension.

- **Rule:** BRAIN-VAN-012 and BRAIN-VAN-014 use BRAIN-DEV-001 for fresh visibility answers.
- **Reference:** between scans, a newly requested visibility answer can depend on the follow
  range of the last body whose sensor ran, including a body in another dimension.
- **Visible to the client:** yes; a head turn toward an already-listed entity can differ.
- **Test:** do not compare this cross-mob range influence. Compare the observer-range rule instead.

The effect has no captured observation. It is excluded because unrelated mobs must not alter an
observer's range. No experiment here attempts to confirm that shared-range accident.

### BRAIN-DEV-002: Random generators can be independent

Provenance: `design`. Kind: semantic. Confidence: medium. Fidelity: distribution.

- **Rule:** section 9, BRAIN-VAN-029, BRAIN-VAN-031 and BRAIN-VAN-035 through BRAIN-VAN-038.
- **Reference:** choices can depend on mob order and process seeds, as section 9 describes.
- **Visible to the client:** yes; individual choices differ; their conditional distributions must agree.
- **Test:** compare distributions and tick boundaries, not a world-seed-identical random trace.

## 11. Unresolved

- **U1:** work/play/meet activity membership and their actions remain proposal under [E-B3](mob-brain-experiments.md#e-b3-schedule-change-with-sleep-as-the-observable) and [E-B9](mob-brain-experiments.md#e-b9-schedule-frame-boundaries-by-age-and-dimension).
  Environment values do not expose active activity ids.
- **U2:** the equal-priority table is medium-confidence ordering data. [E-B6](mob-brain-experiments.md#e-b6-partial-observation-of-equal-priority-activity-order) cannot certify every
  entry. The captured display/sleep trial did not identify activity or behaviour-stop order.
  Ordinary commands cannot configure arbitrary activity sets or expose all transient states.
- **U3:** [E-B8](mob-brain-experiments.md#e-b8-player-passenger-freeze-and-the-dying-camel-exception) confirms the dying camel-family exception, direct-passenger freeze and saddle control.
  Indirect passengers and the separate grazing/rearing timing remain uncaptured.
- **U4:** a generic conversion can carry `minecraft:angry_at` as permanent, but no natural source/
  destination pair was established. Applicability remains low; no universal transfer rule is stated.
- **U5:** [E-B7](mob-brain-experiments.md#e-b7-stuck-pursuit-pause-and-target-displacement) can observe a pursuit pause but cannot identify every eligible attempt or exact stuck
  stop from saved NBT. The 20-subject corridor trial did not establish a qualified stuck stop.
  Exact pause consumption and the >2-block route-change boundary need a trace.
- **U6:** entity/player encounter ties after reload, bed-query encounter order and equal-quality
  path-target choice are unmeasured. They must not be described as a uniform random selection.
- **U7:** nonfinite or out-of-range numeric `ttl` conversion is not specified here.
- **U8:** generic commands cannot certify complete per-mob registration of transient values.
  The provisional census must not replace the registration contract or the species hooks.
- **U9:** living-reference visibility after a referenced entity unloads needs a consumer-specific
  probe. Memory storage alone retains the reference; it does not promise the consumer accepts it.
- **U10:** [E-B1](mob-brain-experiments.md#e-b1-nearest-player-scan-and-an-observable-player-reaction) connected a survival player, but captured motion in both survival and spectator controls.
  No unique activation signal identified the scan. A direct memory capture or a qualified activation witness is still required.
- **U11:** [E-B2](mob-brain-experiments.md#e-b2-injected-expiry-zero-boundary-freeze-and-ai-suspension) confirms injected expiry, suspension and the zero boundary.
  Generic same-update write visibility in BRAIN-VAN-003 remains proposal under [E-B10](mob-brain-experiments.md#e-b10-status-requests-memory-writes-and-brain-replacement).
- **U12:** [E-B4](mob-brain-experiments.md#e-b4-damaged-entries-ignored-registration-and-complete-replacement) confirms the listed replacement and rejection cases.
  Array recovery, unusable outer compounds and the complete registration inventory remain uncaptured extensions of that proposal.
- **U13:** [E-B5](mob-brain-experiments.md#e-b5-sensor-written-expiry-and-first-appearance) confirms first water writes within 19 updates, 20-update resampling and first finite writes with full expiry.
  Exact phase probabilities and independence remain source-suggested. The sample counts alone do not prove them.
- **U14:** BRAIN-DEV-001 has no measured client-visible head-turn effect. Its reference-effect classification remains provisional.
  Compare the observer-range contract; do not experiment to confirm the excluded shared-range accident.
- **U15:** the target restarts the path's stuck window at each node advance or on receipt of a different path.
  An identical route request does not restart it. A mob that keeps reaching nodes is not declared stuck.
  More than 100 follower ticks without node progress can cause a stuck stop despite repeated requests for the same route.
  This differs from AIT-VAN-012 in [entity-ai-timing.md](entity-ai-timing.md) and determines when BRAIN-VAN-035 applies its pause.
  Correction of AIT-VAN-012 is outside this change.

These additional proposals need a brain capture facility that does not yet exist:

| Proposal | Unresolved observation |
|----------|------------------------|
| [E-B10](mob-brain-experiments.md#e-b10-status-requests-memory-writes-and-brain-replacement) | Registration/status requests, same-update writes and refresh/travel retention in BRAIN-VAN-001, BRAIN-VAN-002, BRAIN-VAN-003 and BRAIN-VAN-008. |
| [E-B11](mob-brain-experiments.md#e-b11-nearest-player-memory-and-its-exact-erasure-scan) | Permanent nearest-player values and exact next-scan erasure in BRAIN-VAN-003, BRAIN-VAN-010, BRAIN-VAN-011 and BRAIN-VAN-013. |
| [E-B12](mob-brain-experiments.md#e-b12-entity-query-visibility-and-dependent-adulttemptation-scans) | Query boundaries, retained visibility and adult/temptation outputs in BRAIN-VAN-011, BRAIN-VAN-012, BRAIN-VAN-014, BRAIN-VAN-019 and BRAIN-VAN-020. |
| [E-B13](mob-brain-experiments.md#e-b13-damage-source-age-and-independent-attacker-retention) | Damage-source age and attacker retention in BRAIN-VAN-015. |
| [E-B14](mob-brain-experiments.md#e-b14-item-query-and-first-eligible-result) | Item query, visibility and first-match selection in BRAIN-VAN-011 and BRAIN-VAN-016. |
| [E-B15](mob-brain-experiments.md#e-b15-bed-candidate-cap-retry-and-retained-output) | Bed candidates, retry cleanup and output retention in BRAIN-VAN-017. |
| [E-B16](mob-brain-experiments.md#e-b16-activity-requests-and-exit-effects) | Activity requests, ordered effects and built-in exit lists in BRAIN-VAN-021 through BRAIN-VAN-024, BRAIN-VAN-027 and BRAIN-VAN-039. |
| [E-B17](mob-brain-experiments.md#e-b17-behaviour-lifecycle-durations-and-weighted-groups) | Lifecycle, weighted ordering and group stop effects in BRAIN-VAN-028 through BRAIN-VAN-033. |
| [E-B18](mob-brain-experiments.md#e-b18-intermediate-destinations-and-home-bias-distribution) | Fallback destination and home-bias distributions in BRAIN-VAN-036. |
| [E-B19](mob-brain-experiments.md#e-b19-look-lifetime-and-flotation-probability) | Look duration and flotation boundaries in BRAIN-VAN-029, BRAIN-VAN-037 and BRAIN-VAN-038. |

## 12. Experiments

[Experiment procedures and captured results](mob-brain-experiments.md).

| Experiment | Rules | Status |
|------------|-------|--------|
| [E-B1](mob-brain-experiments.md#e-b1-nearest-player-scan-and-an-observable-player-reaction) | BRAIN-VAN-010, BRAIN-VAN-013, BRAIN-VAN-009 | proposal; inconclusive trial |
| [E-B2](mob-brain-experiments.md#e-b2-injected-expiry-zero-boundary-freeze-and-ai-suspension) | BRAIN-VAN-003 through BRAIN-VAN-007 | captured |
| [E-B3](mob-brain-experiments.md#e-b3-schedule-change-with-sleep-as-the-observable) | BRAIN-VAN-025, BRAIN-VAN-026, BRAIN-VAN-033 | captured |
| [E-B4](mob-brain-experiments.md#e-b4-damaged-entries-ignored-registration-and-complete-replacement) | BRAIN-VAN-001, BRAIN-VAN-006, BRAIN-VAN-007 | captured (listed cases) |
| [E-B5](mob-brain-experiments.md#e-b5-sensor-written-expiry-and-first-appearance) | BRAIN-VAN-004, BRAIN-VAN-010, BRAIN-VAN-018 | captured (listed probes) |
| [E-B6](mob-brain-experiments.md#e-b6-partial-observation-of-equal-priority-activity-order) | BRAIN-VAN-027 | proposal; inconclusive trial |
| [E-B7](mob-brain-experiments.md#e-b7-stuck-pursuit-pause-and-target-displacement) | BRAIN-VAN-034, BRAIN-VAN-035 | proposal; inconclusive trial |
| [E-B8](mob-brain-experiments.md#e-b8-player-passenger-freeze-and-the-dying-camel-exception) | BRAIN-VAN-005, BRAIN-VAN-008 | captured (direct passenger, control and death) |
| [E-B9](mob-brain-experiments.md#e-b9-schedule-frame-boundaries-by-age-and-dimension) | BRAIN-VAN-025 and BRAIN-VAN-026 | partial capture; activity consequences remain proposal |
| [E-B10](mob-brain-experiments.md#e-b10-status-requests-memory-writes-and-brain-replacement) | BRAIN-VAN-001, BRAIN-VAN-002, BRAIN-VAN-003, BRAIN-VAN-008 | proposal |
| [E-B11](mob-brain-experiments.md#e-b11-nearest-player-memory-and-its-exact-erasure-scan) | BRAIN-VAN-003, BRAIN-VAN-010, BRAIN-VAN-011, BRAIN-VAN-013 | proposal |
| [E-B12](mob-brain-experiments.md#e-b12-entity-query-visibility-and-dependent-adulttemptation-scans) | BRAIN-VAN-011, BRAIN-VAN-012, BRAIN-VAN-014, BRAIN-VAN-019, BRAIN-VAN-020 | proposal |
| [E-B13](mob-brain-experiments.md#e-b13-damage-source-age-and-independent-attacker-retention) | BRAIN-VAN-015 | proposal |
| [E-B14](mob-brain-experiments.md#e-b14-item-query-and-first-eligible-result) | BRAIN-VAN-011, BRAIN-VAN-016 | proposal |
| [E-B15](mob-brain-experiments.md#e-b15-bed-candidate-cap-retry-and-retained-output) | BRAIN-VAN-017 | proposal |
| [E-B16](mob-brain-experiments.md#e-b16-activity-requests-and-exit-effects) | BRAIN-VAN-021, BRAIN-VAN-022, BRAIN-VAN-023, BRAIN-VAN-024, BRAIN-VAN-027, BRAIN-VAN-039 | proposal |
| [E-B17](mob-brain-experiments.md#e-b17-behaviour-lifecycle-durations-and-weighted-groups) | BRAIN-VAN-028, BRAIN-VAN-029, BRAIN-VAN-030, BRAIN-VAN-031, BRAIN-VAN-032, BRAIN-VAN-033 | proposal |
| [E-B18](mob-brain-experiments.md#e-b18-intermediate-destinations-and-home-bias-distribution) | BRAIN-VAN-036 | proposal |
| [E-B19](mob-brain-experiments.md#e-b19-look-lifetime-and-flotation-probability) | BRAIN-VAN-029, BRAIN-VAN-037, BRAIN-VAN-038 | proposal |

## 13. Coverage of issue #230

| What | Rules |
|------|-------|
| Typed memories and optional expiry | BRAIN-VAN-001 through BRAIN-VAN-008 |
| Independent sensors and player/entity/damage/item/block results | BRAIN-VAN-010 through BRAIN-VAN-020 |
| Behaviour sets, entry conditions, memory and schedule transitions | BRAIN-VAN-021 through BRAIN-VAN-033, BRAIN-VAN-039 |
| Entity/spatial indexes #98 and #99 | BRAIN-DES-001, BRAIN-VAN-011 |
| Vanilla plugin outside the server crate | Section 0.2 scope; implementation review of placement and plugin registration |
| Nearest-player write within its sensor interval and exact erasure scan | BRAIN-VAN-010, BRAIN-VAN-013; [E-B11](mob-brain-experiments.md#e-b11-nearest-player-memory-and-its-exact-erasure-scan) and its mcrs behaviour fixture |
| Finite expiry boundary | BRAIN-VAN-004; [E-B2](mob-brain-experiments.md#e-b2-injected-expiry-zero-boundary-freeze-and-ai-suspension) and [E-B5](mob-brain-experiments.md#e-b5-sensor-written-expiry-and-first-appearance) |
| Attribute-schedule activity change | BRAIN-VAN-025, BRAIN-VAN-026, BRAIN-VAN-033; [E-B3](mob-brain-experiments.md#e-b3-schedule-change-with-sleep-as-the-observable) and [E-B9](mob-brain-experiments.md#e-b9-schedule-frame-boundaries-by-age-and-dimension) |
| Production behaviour test | [Capture proposals](mob-brain-experiments.md#122-proposals-that-need-a-brain-capture-facility); nearest-player fixture [E-B11](mob-brain-experiments.md#e-b11-nearest-player-memory-and-its-exact-erasure-scan) and schedule fixture [E-B3](mob-brain-experiments.md#e-b3-schedule-change-with-sleep-as-the-observable) |
