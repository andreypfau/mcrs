# Mob brain: common sensors

This file is part of [mob-brain.md](mob-brain.md). Its terms, target version, fidelity classes,
provenance conventions, hooks and experiments apply here. All sensor ids use `minecraft:`.
F means current effective `minecraft:follow_range`; Q means `minecraft:tempt_range` rounded
to binary32. Attribute defaults are F=32 and Q=10 blocks, each with range [0,2048].
The ordinary mob base for F is 16; species overrides remain hooks.

## 1. Cadence and world queries

**BRAIN-VAN-010** (`source_suggested_black_box_confirmed`, semantic, confidence: high, fidelity: exact).
Confirmation scope: [E-B5](mob-brain-experiments.md#e-b5-sensor-written-expiry-and-first-appearance) covers first water scans in 1..19 and a subsequent scan after exactly 20 updates.
The distribution estimate is consistent with the stated probabilities; it does not prove their exact values.
Other clauses retain `source_suggested`, confidence: medium.

Sensors must use the periods and initial phase distribution of AIT-VAN-016.
For P=20, first tick 1 must have probability 2/20, and ticks 2..19 each 1/20.
No first scan can occur at update 0 or 20. These random choices have distribution fidelity.

A sensor not due must only advance its own countdown. It must not query or write memories.
A due sensor must act in the body's declared sensor order. Earlier sensor writes must be visible
to later sensors and behaviour starts in that update.

A saved-brain load must choose fresh phases;
no saved countdown must resume. Entity-data edits can therefore disturb cadence measurements.

**BRAIN-VAN-011** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
A query must use feet distance in three dimensions unless it explicitly names block or eye
coordinates. An expanded box must include the body's original width and height.
A candidate box must positively overlap each axis; face, edge or corner contact alone must fail.
A distance sort must use ascending squared feet distance and retain encounter order for equal distances.

Input encounter order is not a UUID preference.
Exact encounter ties after reload remain U6 of the main file.

Each sensor below defines its query set, filters, selection and cadence. BRAIN-DES-001
requires the facilities of #98 and #99 without a requirement on their index layout.
Except for the bed candidate limit, the common queries must not impose a result-count cap.
All common sensors below have period 20 brain updates.

Line-of-sight work can add block reads to an entity query.
GOAL-QRY-004's deliberate order-key tie rule for goals must not silently replace sensor ties.

Each common sensor must declare exactly the memory ids in this table for BRAIN-VAN-001.
A declaration can include an input memory as well as an output memory.

| Sensor | Sensor-declared memory ids |
|--------|----------------------------|
| `minecraft:nearest_players` | `minecraft:nearest_players`, `minecraft:nearest_visible_player`, `minecraft:nearest_visible_targetable_players`, `minecraft:nearest_visible_targetable_player` |
| `minecraft:nearest_living_entities` | `minecraft:mobs`, `minecraft:visible_mobs` |
| `minecraft:hurt_by` | `minecraft:hurt_by`, `minecraft:hurt_by_entity` |
| `minecraft:nearest_items` | `minecraft:nearest_visible_wanted_item` |
| `minecraft:nearest_bed` | `minecraft:nearest_bed` |
| `minecraft:is_in_water` | `minecraft:is_in_water` |
| `minecraft:nearest_adult` | `minecraft:nearest_visible_adult`, `minecraft:visible_mobs` |
| `minecraft:nearest_adult_any_type` | `minecraft:nearest_visible_adult`, `minecraft:visible_mobs` |
| `minecraft:food_temptations` | `minecraft:tempting_player` |
| `minecraft:frog_temptations` | `minecraft:tempting_player` |
| `minecraft:nautilus_temptations` | `minecraft:tempting_player` |

## 2. Shared visibility and attack checks

**BRAIN-VAN-012** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
A shared visibility check must reject the body, dead entities and spectators.
An attack check must also reject an entity the body cannot attack and an ally.
Ordinary attack restrictions include invulnerability, player invulnerable ability and players
on peaceful difficulty. Peaceful must not prohibit all attacks on nonplayers.
Ordinary mobs must exclude ghasts; additional species restrictions remain hooks.

Noncombat visibility must allow allies, creative players and invulnerable candidates.

For range R>0, a candidate must have squared feet distance <= `max(R*V,2)^2`.
For R<=0, the shared check must skip the distance test. Equality must pass.
An earlier sensor-specific strict filter must still apply. Follow-range checks must use this
observer's F as BRAIN-DEV-001 requires. Temptation must independently use Q.

The visibility factor V must be the product of these inputs, clamped to [0,10]:

- 0.8 when the candidate's sneak input is active;
- `0.7*max(0.1,A)` when invisible, where A is the fraction of four occupied armour slots;
- the `minecraft:mob_visibility` multiplier of each applicable item in its `minecraft:equippable` slot.

Any nonempty item must count toward A, including non-armour items. The floor 0.1 must use
binary32. Default head disguises that match must contribute 0.5 for these observer types:

- `minecraft:skeleton_skull` against `minecraft:skeleton`;
- `minecraft:zombie_head` against `minecraft:zombie`;
- `minecraft:creeper_head` against `minecraft:creeper`;
- `minecraft:piglin_head` against `minecraft:piglin` or `minecraft:piglin_brute`.

Other default heads must not contribute that factor. Data-pack component values can change it.
For the body's current `minecraft:attack_target`, shared checks must use V=1 instead of these
factors. All other range, life, spectator, attack and sight requirements must still apply.

A sight-required check must satisfy GOAL-QRY-003: same dimension, eye distance <=128 blocks,
and no block collision shape between the eyes. Fluids and entities must not block that segment.
Within one AI step, the first sight answer for a candidate must be reused (AIT-VAN-004).
An explicitly sight-free attack variant must skip only the sight requirement.
The item sensor instead must make the fresh geometric check of BRAIN-VAN-016.

## 3. Common scans

**BRAIN-VAN-013** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
`minecraft:nearest_players` must query every player in the current dimension.
It must reject spectators, require squared feet distance < F*F, then sort survivors by distance.
This strict sphere must include the same vertical range and must accept no player for F=0.
It must not require life, sight or vulnerability for the initial player list.
Each due scan must refresh all these outputs:

| Memory | Result |
|--------|--------|
| `minecraft:nearest_players` | all sorted survivors |
| `minecraft:nearest_visible_player` | first survivor passing noncombat visibility |
| `minecraft:nearest_visible_targetable_players` | all visible survivors also passing attack checks |
| `minecraft:nearest_visible_targetable_player` | first entry in that attackable subset |

An empty list or absent first match must erase its output. Player ties must retain dimension
encounter order. The scan must have no player-count cap or random selection.

These player memories are transient. A scan must write them without finite expiry.
They must retain their values between scans. After a positive scan at S, player departure alone must
not erase them before the next due scan.
With uninterrupted updates and no other writer, that scan at S+20 must erase the outputs that have
no match.
[E-B1](mob-brain-experiments.md#e-b1-nearest-player-scan-and-an-observable-player-reaction) uses a consequent player reaction; [E-B11](mob-brain-experiments.md#e-b11-nearest-player-memory-and-its-exact-erasure-scan) defines the direct memory capture and mcrs behaviour
fixture.

**BRAIN-VAN-014** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
`minecraft:nearest_living_entities` must query living-entity hitboxes against the body's box
expanded by F on all axes. For width w and height h, this is `(w+2F,h+2F,w+2F)`.
It must reject the body and entities that are not alive, then sort by feet distance.
The raw set must not exclude alive spectators, creative entities, allies or invulnerable entities.
It must write the whole list to `minecraft:mobs`, or erase that memory when empty.
It must write a present `minecraft:visible_mobs` visibility result even when the set is empty.

Membership and distance order must remain those of this scan until the next scan.
The first requested visibility answer for a candidate must use current positions, the current
attack-target exemption, and BRAIN-VAN-012 with the BRAIN-DEV-001 range.
The answer must remain until the next scan, including across AI steps.
A consumer's candidate filter must precede the visibility request. A rejected candidate need
not obtain an answer. Box corners and hitboxes with positive overlap can include feet beyond F;
only the later visibility check can make those candidates visible.

**BRAIN-VAN-015** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
`minecraft:hurt_by` must read the body's damage history, without a spatial query.
A source recorded at game time T must remain available through T+40 inclusive.
At a due scan with an available source, the sensor must write it to `minecraft:hurt_by`.
For a living responsible entity, the sensor must also write `minecraft:hurt_by_entity`.
An environmental or nonliving source must leave the old attacker memory unchanged.
Without an available source, the sensor must erase only the source memory.

Every due scan must erase the attacker if it is dead or in another dimension.
It must not erase it for distance, lost sight, source age, creative status or alliance.
The attacker must have no sensor-imposed timeout. Without another hit or writer, source erasure
must occur on the first scan in T+41..T+60 with uninterrupted updates.
A scan at T+40 must still retain the source. There must be no sorting.

**BRAIN-VAN-016** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
`minecraft:nearest_items` must query item hitboxes against the body's box expanded by
32 blocks on X/Z and 16 on Y. For width w and height h, this is `(w+64,h+32,w+64)`.
It must sort all candidates by feet distance. In order, it must test these conditions:

1. The body's item-desire predicate must pass.
2. Squared feet distance must be <1024.
3. Fresh eye-to-item line of sight must pass.

It must write the first eligible item to `minecraft:nearest_visible_wanted_item`, or erase it
when no candidate passes. Follow range must not change this query.
No independent pickup-delay or ownership filter must apply. Item desire remains a species hook.
There must be no candidate cap.

**BRAIN-VAN-017** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
`minecraft:nearest_bed` must scan only for a baby. An adult due scan must make no POI query or output change.
For a baby scan at game time t, retry deadlines and cleanup must use U=t+j, where j is uniform in 0..19.

The sensor must query `minecraft:home` POIs, with any occupancy.
Their squared block-coordinate distance from the body's floored feet block must be <=2304. This is a
radius-48 sphere without a separate vertical cap.
POI records denote bed heads. The query must not sort by distance.

The new-position count must start at zero for each baby scan.
The sensor must visit every home POI position in the query region before it requests a route.
It must count each encountered position absent from the body's retry records as new.

It must accept only its first four new positions.
Each accepted position must acquire deadline U+40
before path success is known. Already-recorded positions must not increase that count and must
remain excluded even after their deadline, until cleanup actually removes their record.
The count must include the fifth and all later new positions until the query is exhausted.

The sensor must request a route to the accepted targets. Reach tolerance must be the greater of 1 and the
largest accepted POI valid range. For default home POIs, Manhattan final-node distance <=1
must count as reachable. If a reachable route exists and its chosen target still has a POI,
the sensor must write that target block to `minecraft:nearest_bed`. It need not be the
Euclidean-nearest bed.
Path-search details remain in [pathfinding.md](pathfinding.md).

When no reachable path exists and the count is below five, the sensor must clean retry records.
Cleanup must remove records with deadline strictly < U. Equality must keep the record.
Success, or failure after at least five new positions, must not clean records.
An absent target POI after path success must not trigger cleanup.

Five new unreachable POIs must yield count five and only four accepted targets.
Such a failure must retain every old retry record, including an expired record.

This sensor must never erase its bed output, including for a miss, destroyed beds or adulthood.
A recreated brain must discard these retry facts.

With fewer than five beds and uninterrupted failed scans, a record inserted at t can first
be removed at t+40. This requires that scan's jitter to exceed the original jitter.
Otherwise, first removal must be at t+60.
The sensor must filter candidates before cleanup. Thus the earliest retry must be t+60 or t+80
respectively.

Success or scans with at least five new positions can postpone retry indefinitely.
Equal-quality target choice and POI encounter order remain U6.
Accepted targets must retain POI encounter order.

**BRAIN-VAN-018** (`source_suggested_black_box_confirmed`, semantic, confidence: high, fidelity: exact).
Confirmation scope: [E-B5](mob-brain-experiments.md#e-b5-sensor-written-expiry-and-first-appearance) covers deep-water unit writes without expiry and erasure after water removal.
Other clauses retain `source_suggested`, confidence: medium.

`minecraft:is_in_water` must read current water-contact state without a fresh block-area query.
Water contact must write a unit value to `minecraft:is_in_water`; no contact must erase it.
Submerged eyes must not be required. There must be no sorting or candidate limit.
Fluid-contact calculation remains in the physics rules. [E-B5](mob-brain-experiments.md#e-b5-sensor-written-expiry-and-first-appearance) reads this persisted unit.

**BRAIN-VAN-019** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
`minecraft:nearest_adult` and `minecraft:nearest_adult_any_type` must read the latest
`minecraft:visible_mobs`, without a new entity query.
With absent input, each must leave its previous `minecraft:nearest_visible_adult` unchanged.
With present input, each must visit candidates in the input's distance order.
The first sensor must require the body's entity type and nonbaby age.
The second must require `#minecraft:followable_friendly_mobs` and nonbaby age.

After that filter, the sensor must test the input's visibility answer.
It must write the first match to `minecraft:nearest_visible_adult`, or erase it if none passes.
Adult bodies can run this scan.

There must be no extra radius or count limit. Search membership must remain the
earlier expanded-F box; visibility supplies the effective distance and sight limits.

**BRAIN-VAN-020** (`source_suggested`, semantic, confidence: medium, fidelity: exact).
`minecraft:food_temptations`, `minecraft:frog_temptations` and `minecraft:nautilus_temptations`
must query all players in the current dimension and reject spectators.
They must use noncombat visibility at range Q without sight or the attack-target exemption.
For Q>0, squared feet distance <= `max(Q*V,2)^2` must pass. Q=0 must remove the distance limit.

Alive must be required. Creative status, invulnerability, allies and peaceful must not exclude.
Main-hand food must be tested before off-hand food; either passing hand must suffice.

A direct passenger of the body must fail. Another vehicle or indirect nested passenger must
not fail merely for that relation. Walls must not exclude a tempting player.

The sensor must sort eligible players by feet distance and keep encounter order on ties.
It must select exactly the first. It must write that player to `minecraft:tempting_player`, or erase
it when no player passes.
There must be no count cap. Food predicates must be as follows:

| Sensor | Food predicate |
|--------|----------------|
| `minecraft:food_temptations` | the body's feeding/breeding-food predicate; false for a nonanimal |
| `minecraft:frog_temptations` | item tag `#minecraft:frog_food`, default `minecraft:slime_ball` |
| `minecraft:nautilus_temptations` | item tag `#minecraft:nautilus_food`, default fishes and nautilus bucket food tags |

Item tags must not be confused with identically named entity-type tags. No fourth temptation
sensor exists in the target registry. Per-species food predicates remain hooks.
