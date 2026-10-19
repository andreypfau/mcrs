# Block simulation: behavioral specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |
| World version (`DataVersion`) | `5124` |
| Data pack format | `124.0` |

The source of these values is `../../assets/minecraft/version.json`.

Changes from 26.4-snapshot-2: 26.4-snapshot-3 and 26.4-snapshot-4 did not change
the code of the machinery in this document. This machinery is:

- the server tick and the level tick
- the tick-rate gate
- the scheduled-tick queues
- neighbor updates and shape updates
- sets
- block events
- random ticks
- section properties.

Only one change in the game data affects this machinery: the fluid registry. From
26.4-snapshot-4, a source fluid and a flowing fluid are one fluid type (0.1). Each
rule that this change affects has a "Changed in" note in its Provenance line. These
rules are SIM-SCH-001, SIM-SCH-004, SIM-SCH-007, SIM-SCH-012 and SIM-SCH-013, and
also the constants of section 9. The terms (0.1) and the cited artifacts (0.5)
are updated to agree with this change.

This document gives the block simulation machinery of one dimension on the server.
It gives only what a server, a client or a save file can see. It contains these
subjects:

- where block work runs in a tick
- the flags of a block set
- neighbor updates and shape updates
- scheduled block ticks and scheduled fluid ticks
- block events
- random ticks.

This document does not tell an implementation which architecture to use. It does
not include what each block does when it receives an update, a tick or an event.
It gives only the machinery that delivers these to the block.

The document has two parts.

- **Part A, the abstract model** (section A). Part A gives:
  - the primitives that this document uses for all implementations
  - the contract for all implementations that conform
  - the freedoms that an implementation has
  - the parameters that make one implementation different from another.

  The rules in Part A have `SIM-MODEL` identifiers.
- **Part B, instances** (sections B and 1 to 10). An instance is an
  implementation that has a value for each parameter of Part A. Part B gives two
  instances:
  - The **vanilla instance** keeps the exact rules (sections 1 to 9, identifiers
    `SIM-TICK`, `SIM-SET`, `SIM-NBR`, `SIM-SHP`, `SIM-SCH`, `SIM-BEV`, `SIM-RND`,
    `SIM-RNG`). An exact-parity mode gives the same results as this instance,
    except at the intentional deviations of section B.4.
  - The **threaded instance** (section B.3) is a server that divides a dimension
    into partitions. Each partition has its own tick. Section B.3 gives a summary
    that compares its rules with vanilla, rule by rule. This instance is a
    comparison only. It does not conform to Part A (A.0).

  Section B.4 lists the intentional deviations: the points where all instances of
  this project keep the contract and do not copy vanilla. Section 11 gives the
  design space that a third architecture can use.

Each vanilla rule has two different parts:

- **Observable result**: what must be true after the work, for all internal
  orders. A semantic ordering policy must give this result. It does not have to
  do more.
- **Vanilla order**: the exact order that the game uses. A vanilla ordering policy
  must use this same order. If the two parts are the same, the rule shows this.

Each rule uses sections or positions as its unit. Sometimes the observable
behavior of the game depends on how vanilla puts sections into columns. Then the rule has a
**Vanilla grouping** paragraph. This paragraph shows how an exact-parity mode gets
that behavior from sections (section 0.2).

## 0. Conventions

### 0.1 Terms

- **Position**: an integer block position `(x, y, z)`.
- **Block state**: one state of one block. Two states are *the same* when they are one state. Equal property values of two different blocks do not make two states the same.
- **Block type**: the block of a state, for example `minecraft:repeater`.
- **Fluid type**: one entry of the `minecraft:fluid` registry: `minecraft:empty`, `minecraft:water` or `minecraft:lava`. A water state or a lava state has two properties that the type does not have:
  - `source`, a boolean. It shows if the fluid is a source or a flowing fluid.
  - `level`, 1 to 8. It is the amount of fluid.

  The single state of `minecraft:empty` has no properties. The default state of water and of lava is a source with `level` 8. A water source and flowing water are both of type `minecraft:water`. A flowing state with `level` 8 is *falling*. Changed in 26.4-snapshot-4: the game removed the types `minecraft:flowing_water` and `minecraft:flowing_lava`.
- **Section**, **section position**, **column**: refer to 0.2.
- **Game time**: the tick counter of the dimension (a 64-bit integer). Its value never decreases. The level data contains it. All dimensions of one server read the same game time (SIM-TICK-003).
- **Level tick**: one iteration of the tick of one dimension (SIM-TICK-002).
- **Server tick**: one iteration of the server loop. It runs one level tick for each dimension (SIM-TICK-001).
- **Runs normally**: the tick-rate gate (SIM-TICK-004) is open for this server tick.
- **Chain**: a group of neighbor updates and shape updates. One update request causes all of them, directly or through other updates. That request is made when no chain is active (SIM-NBR-003).
- **Load level** and **simulation level**: two integers that the ticket system keeps for each column in vanilla. A lower value is "more loaded". These thresholds are important here: 33 = full, 32 = block-ticking, 31 = entity-ticking. They are inputs to the section properties of SIM-TICK-005.
- **Section properties**: *loaded*, *accepts ticks*, *runs scheduled ticks*, *delivers block events*, *random-ticked*, *broadcasts changes*. SIM-TICK-005 explains them. A position has a property when its section has it.
- **Full key** of a scheduled tick: the three values (due tick, priority, sub-tick order). To compare two full keys, compare the due ticks first. If they are equal, compare the priorities. If these are also equal, compare the sub-tick orders. The smaller key comes first (SIM-SCH-001).
- **Set**: one write of a block state to one position, with a flag word and an update limit (SIM-SET-002).
- **Pending**: a scheduled tick is pending from the time that its queue accepts it until a phase collects it (SIM-SCH-004). A block event is pending while it is in the queue of block events (SIM-BEV-001).
- **Shape function**: the function of a block state that calculates a new state for its position from one changed neighbor (SIM-SHP-001).
- **Removal side effect**: what a block does when a set removes its state from a position (SIM-SET-002 step 2.5).
- **Placement side effect**: what a block does when a set puts its state at a position (SIM-SET-002 step 2.7). This document does not include what each block does in these side effects.
- **Pre-removal side effects**: what a block entity does before a set removes it. For example, a container releases its contents (SIM-SET-002 step 2.4).
- **Redstone conductor**, **analog output signal**: two properties of a block state that its block definition sets. Comparators read the analog output signal, for example of a container.
- **Randomly ticking**: a property of a block state or of a fluid state that its definition sets. Only states with this property receive random ticks (SIM-RND-002).
- **Level random**: the random source of one dimension that handlers draw from (SIM-RNG-001).
- **Position generator**: the generator of random positions of one dimension (SIM-RND-003).
- **Full status**, **block-ticking status**: two stages of the load of a column in vanilla. A column at full status has all its blocks in memory. A column gets to block-ticking status when its load level is 32 or less and its eight neighbor columns are at full status (SIM-TICK-005).
- **Generation view**: the view of the world that world generation writes through, while a column is not at full status (SIM-NBR-005).
- **Post-processing mark**: a position that generation marks for a later shape check (SIM-SCH-017).
- **Debug world**: the world type that shows all block states. It runs no block simulation.
- **Probe block**: a block type that only the conformance tests of this document use. It has these handlers: neighbor changed, shape update, scheduled tick, block event, random tick, placed, removed. Each handler adds `(handler, position, arguments)` to the end of a test log. Then it does only what the test script tells it to do. Kernel conformance tests use probe blocks. Black-box experiments against the game use vanilla blocks.
- **Probe fluid**: a fluid type that only the conformance tests use. Its tick handler adds an entry to the test log, as the handlers of a probe block do.
- **Inert probe**: a probe block whose handlers only add entries to the test log.
- **Failing twin**: an incorrect result of a conformance test. A known incorrect implementation gives it. An implementation that conforms must not give it.

### 0.2 Sections and columns

The world is a grid of cubic sections with no limit. A column is a vanilla group of
sections in that grid. A column is never the unit of simulation.

- **Section**: a 16 × 16 × 16 cube of positions. It is the unit that contains block
  states, scheduled-tick queues, client-sync marks and random-tick eligibility.
- **Section position**: `(sx, sy, sz) = (x >> 4, y >> 4, z >> 4)`, with an
  arithmetic shift (floor division by 16). There is no limit on `sy`. The number of
  a section comes from its absolute height, not from the lowest section of a
  dimension. The offset of a position in its section is `(x & 15, y & 15, z & 15)`.
- **Column**: the group of all sections that have the same `(sx, sz)`. Vanilla
  calls it a chunk. In vanilla, a column is:
  - the unit that the server uses to send sections to the client
    (`minecraft:level_chunk_with_light`, play clientbound id 46)
  - the unit that selects the players that receive changes to a section. These
    are the players that track its column.
  - the storage of heightmaps. Precipitation and other surface logic read the
    heightmaps.
  - the unit of the save record. The region file has one compound for each column,
    with `block_ticks` and `fluid_ticks`.
  - the unit of load and of activation
  - the unit that gets a load level and a simulation level from the ticket system
  - an xz index of sections.
- **From position to column**: position `(x, y, z)` → section
  `(x >> 4, y >> 4, z >> 4)` → column `(x >> 4, z >> 4)`.
- **Vertical extent**: no rule in this document uses a minimum or maximum build
  height as a condition. If vanilla data is the observable input, the rule shows
  this. In vanilla, the `min_y` and `height` of the dimension type control:
  - the positions that vanilla does not set, and reads as `minecraft:void_air`
    (SIM-SET-002, SIM-SET-003)
  - the sections that a column can contain.
- **Vanilla grouping paragraphs**: a vanilla order, a vanilla eligibility input or
  a save layout can depend on columns. In that case, the rule gives:
  - (a) the section-native rule. A semantic policy gives this result.
  - (b) what an exact-parity mode also does to give the vanilla result. It puts
    sections into groups by `(sx, sz)`.

  An exact-parity mode uses this condition: all sections of one column have the
  same section properties (SIM-TICK-005), as in vanilla.

This document uses each of these words for one thing only:

- *section*: the cube and its contents.
- *column*: only the vanilla group above.
- *load level*, *simulation level* and *ticket*: the machinery that loads columns.
  Vanilla calculates these values for each column.

### 0.3 Direction names

`D` down (−y), `U` up (+y), `N` north (−z), `S` south (+z), `W` west (−x), `E` east (+x).

This document uses three direction orders. They are different. Do not make them into one order.

| Name | Order | Used by |
|------|-------|---------|
| Neighbor order | `W, E, D, U, N, S` | neighbor updates (SIM-NBR-001) |
| Shape order | `W, E, N, S, D, U` | shape updates (SIM-SHP-001) |
| Horizontal order | `N, E, S, W` | comparator output updates (SIM-NBR-006), diagonal shape updates of redstone wire (SIM-SHP-003) |

### 0.4 Provenance and confidence

- `black_box_confirmed`: an artifact in this repository shows it. The game made the artifact. The text gives the path of the artifact.
- `source_suggested_black_box_confirmed`: taken from the reference implementation. A cited artifact agrees with it.
- `source_suggested`: taken from the reference implementation. There is no artifact for it at this time. The rule gives a concrete experiment, or a reference to one.
- `derived`: it comes from other rules of this document, through the argument in the rule.
- `unresolved`: not decided. Section 10 contains these items.

A Provenance line gives only the status and the confidence. No experiment in this
document was run. An open question marked *Answered* has its answer from the
reference implementation, not from an experiment.

### 0.5 Artifacts cited

- `../../assets/minecraft/version.json`: the version values above.
- `../../assets/mcrs/reports/registries.json`: the entries and ids of the block registry and of the fluid registry, and the `minecraft:game_rule` registry. Generated with 26.4-snapshot-4.
- `../../assets/mcrs/reports/packets.json`: packet ids (`minecraft:block_event` = 7, `minecraft:block_update` = 8, `minecraft:level_chunk_with_light` = 46, `minecraft:section_blocks_update` = 86 in the play clientbound table). Generated with 26.4-snapshot-4.
- `data/minecraft/datapacks/redstone_experiments/pack.mcmeta` in the client jar: the feature pack that enables `minecraft:redstone_experiments`.

### 0.6 Rule groups

| Prefix | Subject |
|--------|---------|
| `SIM-TICK` | where block work runs in the server tick and in the level tick, and section properties |
| `SIM-SET` | a block set: flags, side effects, client sync |
| `SIM-NBR` | neighbor updates and the chain |
| `SIM-SHP` | shape updates |
| `SIM-SCH` | scheduled block ticks and scheduled fluid ticks |
| `SIM-BEV` | block events |
| `SIM-RND` | random ticks and precipitation picks |
| `SIM-RNG` | random sources |
| `SIM-MODEL` | the abstract model of Part A |
| `DEV` | intentional deviations from vanilla (B.4) |

### 0.7 Model terms

Part A explains these terms. This list is here for quick reference.

- **Partition**: a group of sections that step together, one step at a time (SIM-MODEL-002).
- **Partition step**: one advance of the work of one partition (SIM-MODEL-002). In vanilla, a partition step is one level tick. In Part A, *step* alone means a partition step.
- **Halo**: the positions that a step can read or write synchronously. The step holds them exclusively until it ends (SIM-MODEL-002).
- **Lock-step**, **free-running**: the two clock modes of SIM-MODEL-004.
- **Guarantee level**: exact, semantic or boundary. Each instance states the level that it gives (A.0).
- **Sequence**: the key of a scheduled tick that orders the calls that schedule ticks (SIM-MODEL-005). In vanilla, the sequence is the sub-tick order (SIM-SCH-001).
- **Owned**: the partition of a section owns that section. A position is owned when its section is owned.
- **Partition time** and **world time**: the clocks of SIM-MODEL-004. There is one partition time for each partition.
- **Work item**: a scheduled tick, a neighbor request, a shape update, a block event, a random pick or a set (SIM-MODEL-005).
- **Cross-partition effect**: a work item that one partition makes, with a target that another partition owns or that no partition owns (SIM-MODEL-007).
- **Ordering policy**: the rule that puts the work items of one queue in a partition into an order. It is also the rule that merges queues when partitions merge (SIM-MODEL-008).
- **Instance**: an implementation, given by its value for each parameter of section A.5.

---

# Part A — Abstract model

## A. The contract

Part A is normative for all implementations. A rule in Part A gives these items:

- what must be true (**invariants**)
- what can change, and the limits of that change (**freedoms**)
- what an implementation selects (**parameters**, collected in A.5).

The detailed rules of sections 1 to 9 are the vanilla instance. A.6 shows which
of these rules are true for all instances with no change.

### A.0 Guarantee levels

Each instance states the guarantee level that it gives:

- **Exact.** The result is the result of vanilla, tick for tick, except at the
  deviations of B.4. Only the vanilla policy over one partition for each
  dimension gives this level (B.2).
- **Semantic.** Inside each partition, the Observable result of each rule of
  sections 1 to 9 holds. Items that depend on each other (SIM-MODEL-006) run in
  the order that the rules give. Items in conflict whose order the rules leave
  free run in an order that a stable rule selects. Independent items can run in
  any order and at the same time.
- **Boundary.** The semantic level holds inside each partition. An item that
  crosses a partition boundary is a message (SIM-MODEL-007). The message runs
  later than vanilla runs the item, so other work can see an intermediate state.

In this document, an item *crosses a partition boundary* when its footprint
(SIM-MODEL-006) is not inside the halo of its partition (SIM-MODEL-007). Such an
item is a message to the partition that owns its target. That partition can be
the same partition. An item with its
footprint in the halo runs synchronously, also when another partition owns a
position of the footprint.

An instance at the boundary level states which item kinds can cross a boundary.
It also states when the receiving partition applies them. The instance of this
project gives the exact level with one partition for each
dimension. With more partitions, it gives the boundary level (OQ-12).

An instance that discards a cross-partition effect gives none of these levels.
It does not conform to Part A. The threaded instance of B.3 is such an instance.
This document gives it as a comparison only.

### A.1 Primitives

#### SIM-MODEL-001: Space

- The world of one dimension is the grid of cubic sections of 0.2. This grid has
  no limit. A section is the smallest unit of state ownership. These items are held
  for each section:
  - block states
  - pending scheduled ticks
  - client-sync marks
  - post-processing marks
  - random-tick eligibility.
- Any group of sections (a vanilla column, a partition of the threaded server,
  a shard) is an instance parameter. A group of sections is never part of the
  contract.
- The vertical extent is dimension data (0.2). It is not a property of the model.

Provenance: `derived` from 0.2. Confidence: high.

#### SIM-MODEL-002: Partitions

- **Definition.** A *partition* is a group of sections. One step function
  advances the work of these sections, one step at a time. At any time, each
  section that holds simulated state belongs to not more than one partition of
  its dimension. A section that is in no partition is not simulated.
- **Step.** A *partition step* runs the phases of A.4 that run for each
  partition. It runs them over the owned sections of the partition. In one step,
  the work of the partition runs in the order that its ordering policy gives
  (SIM-MODEL-008). A step reads and writes only these data:
  - the sections of its halo
  - read-only snapshots of state outside all partitions (world time, weather,
    game rules, registries).
- **Lifecycle.** Partitions can be created, merged and split. A merge or a split
  occurs only between the steps of all partitions that it involves. It never
  occurs while one of these partitions is inside a step.
- **Halo.** The *halo* of a partition is a group of positions. A step can read
  or write these positions synchronously from a work item at an owned position.
  - The step holds each section of its halo exclusively until the step ends. No
    other step that runs at the same time owns that section or has it in its
    halo.
  - The halo contains owned positions. It can also contain positions in
    partitions that never step at the same time as this partition.
  - An instance gives its halo (A.5, P-HALO). In vanilla, the halo is all
    positions of all dimensions, because the dimensions step one after another.
  - The halo of a partition does not change during a step. A step that needs a
    position outside its halo uses a cross-partition effect (SIM-MODEL-007).

Invariants:
- Two partitions never write the same section in steps that overlap.
- Each pending work item belongs to one partition only, or to an explicit
  in-flight message (SIM-MODEL-007). That partition is the partition that owns
  the target section of the item.
- A merge or a split loses no pending work item. It changes no key of an item,
  except the change that SIM-MODEL-004 allows. That change is a rebase of the due
  tick by a clock offset.

Freedoms:
- the shape, the size and the formation rule of partitions
- when to merge and split partitions
- if steps of different partitions run at the same time.

Provenance: `derived`. Confidence: high.

#### SIM-MODEL-003: Section properties are inputs

The six section properties of SIM-TICK-005 (*loaded*, *accepts ticks*, *runs
scheduled ticks*, *delivers block events*, *random-ticked*, *broadcasts changes*)
are inputs. The host gets them from its load machinery. The model has only these
requirements:

- A section that *runs scheduled ticks* also *accepts ticks* and is *loaded*.
- A section that is *random-ticked* is also *loaded*.
- A section that *accepts ticks* is not necessarily *loaded*. Also, a *loaded*
  section does not necessarily have the property *accepts ticks* (SIM-TICK-005).
- Each section that a partition steps over is owned by that partition.

How the host calculates the properties (tickets, distances, column groups) is an
instance parameter (P-PROPS). A section property can be different for two
sections of one vanilla column only in a non-vanilla instance.

Provenance: `derived` from SIM-TICK-005. Confidence: high.

#### SIM-MODEL-004: Clocks

- **World time** `W`: one counter for each dimension (vanilla: shared by all
  dimensions, SIM-TICK-003). It advances one time in each server tick that runs
  normally. Players, commands and saved level data see it as game time.
- **Partition time** `τ(p)`: one counter for each partition. It advances by one
  at each step of `p` that runs normally. Each scheduled tick of `p` uses `τ(p)`
  as its clock. When the tick is scheduled, its due tick is `τ(p) + d`. The tick
  is collected only when `τ(p) ≥ due`.
- An instance does one of these two things:
  - It ties each `τ(p)` to `W` (*lock-step*). Each step of each partition
    advances together with `W`. As a result, `τ(p) = W` at all times.
  - It lets the partition times drift (*free-running*).
- **Rebase on merge.** When partition `a` is merged into `b`, each due tick of
  `a` is shifted by `τ(b) − τ(a)`. As a result, the delay that remains
  (`due − τ`) does not change. On a split, each part starts from the `τ` of the
  parent.
- **Persistence.** A saved tick stores the delay that remains, relative to the
  clock of its owner at the time of the save. On load, the tick is due at the
  clock of the loader plus that delay (SIM-SCH-012, SIM-SCH-013).

Invariants (all instances):
- **Delay.** A tick that is scheduled with delay `d ≥ 1` runs only after at
  least `d` steps that run normally. These are steps of the partition that owns
  the position of the tick, and they occur after the tick was scheduled. The
  count continues across merges and splits. Time spent unloaded does not count.
  Time spent loaded counts, also when ticks do not run (SIM-SCH-005, SIM-SCH-013).
  Vanilla breaks this invariant in one case (B.4, DEV-02).
- **No loss.** A pending tick stays pending until a phase collects it, also
  across an unload and a load. A tick that ran does not run again after a load.
  Vanilla breaks this invariant (B.4, DEV-01).
- **Promptness.** When a tick is due, it runs at the first step in which its
  section *runs scheduled ticks*, unless the budget (SIM-MODEL-104) defers it.
- If two ticks of one partition are scheduled `k` steps apart with delays `d1`
  and `d2`, they keep their relative due ticks: `due2 − due1 = k + d2 − d1`.

Freedoms: if partition time is equal to world time or not. In a free-running
instance, the relative time of two contraptions in different partitions is not
defined. A contraption that spans partitions of different speeds sees each part
at its own speed.

Provenance: `derived` from SIM-TICK-003 and SIM-SCH-003. The free-running case is
taken from the threaded instance. Confidence: high.

#### SIM-MODEL-005: Work items and their keys

| Item | Created by | Target | Identity (de-duplication) | Order key inside a partition (vanilla policy) | Rule |
|------|------------|--------|---------------------------|-----------------------------------------------|------|
| set | any caller | one position | none | program order of the caller | SIM-SET-002 |
| neighbor request (4 kinds) | a write, a handler | 1 to 6 positions next to a source | none | chain order: depth first, request order (SIM-NBR-003) | SIM-NBR-002 |
| shape update | a write | one neighbor of the written position, or a wire diagonal | none | chain order | SIM-SHP-001 |
| scheduled tick | a handler or a write | one position | (position, type) for each kind | (due tick, priority, sequence), collected for each vanilla column (SIM-SCH-006) | SIM-SCH-001 |
| block event | a handler | one position | (position, block, a, b) while pending | FIFO by insertion | SIM-BEV-001 |
| random pick | the phase | one section | none | the order in which sections are visited, then pick index | SIM-RND-002 |

- The **sequence** of a scheduled tick is a number that orders the calls that
  schedule ticks. In vanilla, the sequence is the sub-tick counter of the
  dimension (SIM-SCH-003).
- The item itself carries each key that an ordering policy can use: the due
  tick, the priority, the sequence and the insertion order. No key is calculated
  from the identity of the partition, the thread or the column that holds the
  item. The only exception is the vanilla collection order, which groups by
  column (SIM-SCH-006).

Provenance: `derived`. Confidence: high.

#### SIM-MODEL-006: Causal dependency

- Two items are *in conflict* when they touch one position, and at least one of
  them writes that position. A conflict is a read after a write, a write after a
  read, or a write after a write.
- Work item `y` *depends on* item `x` in these two cases:
  - `x` created `y` (creation).
  - `x` and `y` are in conflict, and the rules of sections 1 to 9 put `x`
    before `y` (order).
- The rules leave the order of some items in conflict free. An example is two
  overdue scheduled ticks in different sections (SIM-SCH-006). The instance
  selects the order of such items with a stable rule (SIM-MODEL-008). The result
  is the same on each run, but it can be different from vanilla. Items in
  conflict never run at the same time.
- A group of items is *causally connected* when creation or conflict joins
  them.
- The **footprint** of an item is the group of positions that it can read or
  write when it runs. For the machinery of this document, the footprint has a
  limit (A.2). What a block handler reads has a limit that the handler controls.

  For example, a piston reads positions up to 14 blocks away along its axis. It
  reads more positions to the side when it moves slime or honey blocks. The
  piston does this in its neighbor-changed handler and in its block event, not
  in a scheduled tick. A tripwire hook
  reads up to 41 positions along its line.
- The footprint of one item has a limit, but the footprint of a group has none.
  A handler can create items, and these items can create more items, farther
  away. An instance knows the footprint of an item only when the item exists.
  An instance that runs groups at the same time must handle this growth through
  its halo (P-HALO) and its choice for cross-partition effects (P-FOREIGN).
- Two groups of items can run in any order and at the same time, with the same
  result, if these conditions are true:
  - No item of one group created an item of the other group.
  - No item of one group writes a position that an item of the other group
    reads or writes. Two items can read one position.

  These conditions apply also to the items that the groups create while they
  run. The shared counters of A.2 (sequence, budget, chain count, random state)
  are the only exception.

Provenance: `derived`. Confidence: high for the definition, medium for the
handler radii. The handler radii are part of block behavior, which this document
does not include. They are given as examples only.

#### SIM-MODEL-007: Cross-partition effects

- A work item that a step of partition `p` makes is a *cross-partition effect*
  if its footprint is not inside the halo of `p`. The *receiver* is the
  partition that owns the target of the item. The receiver can be another
  partition, or `p` itself when only a part of the footprint leaves the halo.
  An example is a neighbor update or a block event at a piston near the edge of
  the halo: the piston is in the halo, but the blocks that it moves are not. If
  no partition owns the target, the target is not loaded.
- For each item kind, an instance selects one of these choices (parameter
  P-FOREIGN):
  - **synchronous**: this choice is possible only when the halo covers the
    footprint. In vanilla, the halo is all positions of all dimensions.
  - **message**: the item is delivered to the receiver. It is applied at a
    defined point of a step of the receiver, in a stable order.
  - **load**: the target section is first loaded into `p`, as vanilla does
    (SIM-SET-003).
  - **discard**: the item has no effect.
  - **refuse**: the call that makes the item fails with an error. The item has
    no effect.
- A message keeps all keys that the item carries (SIM-MODEL-005). If the item
  has a due tick, the message carries it as the delay that remains. This due tick is
  rebased into the clock of the receiver.
- The application point and the order of the messages that arrive at the
  receiver in one step are part of the ordering policy of the receiver. It must
  not depend on the thread that delivered the messages.
- **Progress.** Before the receiver applies a message, the instance makes the
  halo of the receiver cover the footprint of the message. It merges partitions,
  or it gives the receiver more sections, between steps (SIM-MODEL-002). The
  footprint of one item has a limit (SIM-MODEL-006), so this growth ends, and
  each message runs.

Invariant: for all choices, an item is applied not more than one time. An
instance that claims gameplay parity with vanilla across partition boundaries
must not discard items. B.3 shows what breaks when items are discarded.

The choice *message* gives the boundary level of A.0 for its item kind. The set
that made the item returns before the item runs. Other work can see the state
between the two.

Provenance: `derived`. The discard and refuse choices are taken from the
threaded instance (B.3). Confidence: high.

#### SIM-MODEL-008: Ordering policy

An *ordering policy* has two parts, for each queue and each partition:

- the total order in which the step runs the items of that queue
- the rule that combines the queues of two partitions on a merge, and divides
  them on a split.

The contract fixes only the invariants of A.3. The vanilla policy is the exact
policy of sections 1 to 9. A policy is *stable* when the order depends only on
the item keys and the stored state. The order of a stable policy never depends
on when threads run, on hash iteration or on memory layout.

Requirement: each policy of an instance that is used for an exact-parity mode is
the vanilla policy over one partition for each dimension. All other policies
must be stable: the same inputs give the same order on any thread. The policy of
the threaded instance is not stable (B.3).

Provenance: `derived`. Confidence: high.

### A.2 Locality

A property is *local* when it can be found from one section and a limited
neighborhood of that section. A property is *global* when it depends on the
state of the whole partition (or dimension). Local properties can be divided
between partitions freely. Global properties are either kept for each partition,
or they must have a cross-partition protocol. If they are kept for each
partition, they are different from vanilla when there are several partitions.

| Mechanism | One step reads/writes | What is global | Rule |
|-----------|----------------------|----------------|------|
| set | the position, its 6 neighbors for updates | nothing | SIM-SET-002 |
| neighbor request | 1 to 6 positions at distance 1 | the chain: its stack, its order across sections, its request count | SIM-NBR-001, SIM-NBR-003, SIM-NBR-004 |
| comparator output update | up to 2 positions along each horizontal direction | nothing | SIM-NBR-006 |
| shape update | 1 position at distance 1 (wire diagonal: distance 2) | the update limit travels with the cascade, not with a section | SIM-SHP-001..003 |
| scheduled tick: schedule | its position | the sequence counter. De-duplication is for each position, so it is local | SIM-SCH-003, SIM-SCH-004 |
| scheduled tick: collect | the heads of the queues of all sections | the budget, the merge order across columns | SIM-SCH-006, SIM-SCH-008 |
| block event | one position | FIFO order of the single queue. The drain runs until the queue is empty | SIM-BEV-001, SIM-BEV-002 |
| random pick | one section | the position generator and the level random streams. The order in which sections are visited | SIM-RND-001..003 |
| precipitation pick | one column surface | the same as random picks, and also the heightmap | SIM-RND-004 |
| client sync | marks for each section | nothing (packets for each section) | SIM-SET-005 |
| save | one record for each vanilla column | the sequences that a load restores, for each column record | SIM-SCH-012, SIM-SCH-013 |

Consequences:

- Each **single step** of the machinery is local, with a radius of not more than
  2 blocks. Block handlers add their own radii, and each of these radii has a
  limit (A.1, SIM-MODEL-006).
- The global items are a short list of **counters and orders**:
  - the scheduled-tick sequence
  - the scheduled-tick budget
  - the chain request count
  - the depth-first order of the chain
  - the block event FIFO
  - the random streams
  - the order in which sections are visited.

  A partitioned instance must keep each of these items for each partition. If
  not, it must give the same result across partitions as one global counter or
  order.

Provenance: `derived` from the cited rules. Confidence: high.

### A.3 Behavior contracts

Each contract gives (a) invariants, (b) freedoms and (c) parameters (A.5).

#### SIM-MODEL-101: Sets

- (a) These items do not change in any instance:
  - the flag semantics of SIM-SET-001
  - the steps of SIM-SET-002 and their order
  - the destroy of SIM-SET-004.

  A set that the server refuses (SIM-SET-002 steps 1, 2.1 and 2.2) makes no
  requests or updates, and sends nothing. A set that fails at step 2.6 already
  wrote its state. It also ran the removal side effect, with the requests of that
  side effect. Neighbor updates carry the old block type.
- (b) There is no freedom inside one set.
- (c) P-FOREIGN applies to sets with a footprint outside the halo. P-LOAD applies to sets at
  positions that are not *loaded*.

#### SIM-MODEL-102: Neighbor updates, shape updates and the chain

- (a) The invariants are:
  - Each accepted request runs one time only. Each update of a request runs not
    more than one time.
  - The handler that made a request completes before the request runs.
  - A request reads the target state at the time when it runs. The captured
    fields of SIM-NBR-002 are the exception.
  - At the exact and the semantic levels (A.0), a set outside any chain returns
    only after its whole chain. At the boundary level, it returns after the part
    of its chain inside the halo of its partition.
  - The traversal does not recurse (SIM-NBR-003).
- (a) A chain that stays inside one partition runs in that partition as
  SIM-NBR-003 gives.
- (b) The order between requests of one chain that do not depend on each other
  and are not in conflict (SIM-MODEL-006). The vanilla order is depth first, in
  request order.
  An implementation can use another order only for such requests.
- (b) At the boundary level, a chain can cross a partition boundary. The part of
  the chain on the other side of the boundary is a cross-partition effect
  (SIM-MODEL-007). It runs later in the partition that receives it, as the start
  of a new chain in that partition. At the exact and the semantic levels, each
  chain runs in one partition.
- (c) P-CHAIN (scope of the chain and its count), P-FOREIGN, P-LOAD.

#### SIM-MODEL-103: Limits

- (a) **Update limit** (SIM-SHP-002): a shape cascade has not more than `L`
  generations after the original set. When a block makes shape updates
  directly with limit `L`, the cascade has not more than `L` + 1 generations
  (SIM-SHP-001). By default, `L` = 512. The generations are counted for each
  cascade, across sections and across partitions. A message carries the limit
  that is left.
- (a) **Chain limit**: a chain accepts not more than `M` requests (SIM-NBR-004).
  If `M ≥ 0`, the server never fails on a long chain.
- (b) If the chain is divided into pieces, one for each partition, the count can
  be kept for each piece. Then a long chain that crosses `n` partitions can run
  up to `n·M` requests. An instance gives this information (P-CHAIN).

#### SIM-MODEL-104: Scheduled ticks

- (a) The invariants are:
  - the fields and the identity (SIM-SCH-001)
  - the priority values and how they are decoded (SIM-SCH-002)
  - de-duplication by (position, type) for each kind, until the tick is
    collected (SIM-SCH-004)
  - the delay, no-loss and promptness invariants of SIM-MODEL-004
  - the type check when the tick runs (SIM-SCH-007)
  - the block ticks of a step run before its fluid ticks (SIM-SCH-011)
  - a tick is collected not more than one time, and a phase does not collect the
    ticks that are scheduled during that phase (SIM-SCH-009)
  - the queries of SIM-SCH-010.
- (a) Inside one section, collected ticks run in full-key order.
- (a) Inside one partition, if no collected tick is overdue, the collected ticks
  run in (priority, sequence) order across all sections (SIM-SCH-006). This
  order keeps the priorities of repeaters and comparators across section
  boundaries.
- (a) A merge, a split or a message keeps the priority and the sequence of a
  tick. Only its due tick can be rebased (SIM-MODEL-004).
- (b) The order across sections when some collected tick is overdue (vanilla: a
  merge grouped by column, SIM-SCH-006). The scope and the value of the
  budget: vanilla uses 65 536 for
  each kind, for each dimension and for each step. With several partitions, the
  budget is for each partition (P-BUDGET).
- (b) The scope of the sequence counter (P-SEQ). If partitions that run at the
  same time share one counter, the relative sequence of two ticks depends on
  when threads run. A stable instance calculates the sequence from
  partition-local state (for example (partition time, local counter)). It orders
  ticks across merged partitions by a stable rule.
- (c) P-CLOCK, P-SEQ, P-BUDGET, P-COLLECT, P-FOREIGN.

#### SIM-MODEL-105: Block events

- (a) The invariants are:
  - de-duplication of equal pending events
  - delivery checks the block type at the time of delivery
  - the packet and its range (SIM-BEV-004)
  - events are not saved
  - the deferral while the position does not *deliver block events* keeps the
    relative order of the events (SIM-BEV-003)
  - events queued during a drain are delivered in the same drain (SIM-BEV-002).
- (a) FIFO order among the events of one partition.
- (b) The order between events of different partitions. When there are several
  partitions, this order is not defined. On a merge, the two queues must be
  combined by a stable rule (P-EVENTS).
- (c) P-EVENTS, P-FOREIGN.

#### SIM-MODEL-106: Random ticks and precipitation

- (a) Only the distributions are invariants (SIM-RND-001, SIM-RNG-001):
  - Each randomly ticking block state of a *random-ticked* section receives
    random ticks at the rate of `k` uniform picks in each step. The picks are
    over the 4096 positions of the section, with replacement.
  - Precipitation has `k` picks for each column surface in each step. Each pick
    has the probability 1/48.
- (b) The order in which sections are visited, the random streams and the pick
  algorithm are free if these two conditions are true:
  - Each pick is uniform over the positions of the section.
  - The rate for each block is kept.

  It is not necessary that the picks are independent. The picks of vanilla are
  correlated (SIM-RND-002).
- (c) P-RNG, P-RANDOM.

#### SIM-MODEL-107: Client sync

- (a) SIM-SET-005: not more than one block-state packet for each section in each
  broadcast. The packet carries the states that the section has at that time. The broadcast
  of a step comes after the scheduled ticks and random ticks of the step, and
  before its block events.
- (b) The order across sections.

#### SIM-MODEL-108: Persistence

- (a) The save layout of SIM-SCH-012. This layout is the save format, which is
  exact. It contains these items:
  - one record for each vanilla column
  - delays relative to the clock of the owner
  - restored sequences `k − m` for each record and each kind (SIM-SCH-013).
- (b) There is no freedom in the format. The time when a column record is written
  is free.

#### SIM-MODEL-109: Time gates

- (a) Freeze and step (SIM-TICK-004) stop and release all gated phases. Pending
  ticks wait. They do not run early after a resume.
- (b) The scope of the gate: one gate for the world, or one gate for each
  partition (P-GATE). With a gate for each partition, partition time stops while
  world time can advance.

### A.4 Phases of a partition step

Each instance runs these phases in this order, in each partition step that runs
normally (SIM-TICK-002):

1. the scheduled block ticks
2. the scheduled fluid ticks
3. the random picks and the precipitation picks, with the client broadcast after
   them
4. the block events
5. entities and block entities.

Neighbor and shape updates run inside the phase that causes them. Other work can
run between these phases. In vanilla:

- Raids run after the scheduled fluid ticks.
- Ticket work, load work and spawning work run before the random picks.
  Spawning work draws from the level random.
- Special spawners run after the random picks and before the broadcast.
- Entities and block entities run only while the dimension is not idle
  (P-IDLE).

Work that does not belong to a partition (world border, weather) can run outside
partitions, one time for each dimension. World time advances one time for each
server tick (SIM-TICK-003).

Provenance: `source_suggested` for both instances (B.2, B.3). Confidence: high.

### A.5 Parameters an instance chooses

| Id | Parameter | Values |
|----|-----------|--------|
| P-SHAPE | partition shape and granularity | for example: whole dimension; groups of whole columns; groups of cubic sections |
| P-FORM | formation, merge and split rule, and when they apply | for example: none; proximity of loaded columns, merged on contact and split when emptied; causal connectivity |
| P-HALO | positions that a step can touch synchronously | for example: whole dimension; owned area minus a margin |
| P-CLOCK | lock-step or free-running partition time; rebase rule | SIM-MODEL-004 |
| P-SCHED | how steps are scheduled; catch-up when late | for example: one loop; independent deadlines for each partition |
| P-SEQ | scope of the scheduled-tick sequence counter | for each dimension; for each partition; calculated |
| P-COLLECT | collection order across sections | vanilla column merge; global full key; for each section |
| P-BUDGET | budget scope and value | for each dimension; for each partition |
| P-CHAIN | scope of a chain and its count | for each dimension; for each partition piece |
| P-EVENTS | scope of the block event queue, and merge rule | for each dimension; for each partition, appended on merge |
| P-FOREIGN | what occurs to each item kind with a footprint outside the halo | synchronous; message; load; discard; refuse with an error |
| P-LOAD | if a read or a write at a valid unloaded position loads it, and how long the section stays loaded after that | yes, held by a temporary ticket (vanilla); no |
| P-PROPS | how section properties are calculated | vanilla tickets for each column; other |
| P-RNG | random streams and their scope | one for each dimension; for each partition; for each section |
| P-RANDOM | random pick algorithm | vanilla picks in each section; others that keep the rates |
| P-GATE | scope of freeze, step and pause-when-empty | for each server; for each partition |
| P-WIRE | algorithm that evaluates redstone wire | vanilla; alternatives with other update orders |
| P-IDLE | when a dimension stops the ticks of its entities and block entities | vanilla: 300 level ticks after the last keep-active ticket (SIM-TICK-002); never; other rules |
| P-CAPTURE | the neighbor state that a queued shape update gives to the shape function | the state captured at request time (vanilla); the state when the update runs |

### A.6 Which detailed rules hold for all instances

- **Universal as written**: all instances keep these rules, for all parameter
  values:
  - SIM-SET-001, SIM-SET-002 (without the synchronous load of step 2),
    SIM-SET-004. At the boundary level, the updates of a set complete in the set
    only inside the halo (SIM-MODEL-102).
  - SIM-NBR-002, SIM-NBR-005
  - SIM-SHP-002
  - SIM-SCH-001, SIM-SCH-002, SIM-SCH-004, SIM-SCH-007, SIM-SCH-010,
    SIM-SCH-011, SIM-SCH-012 (layout), SIM-SCH-013 (restored sequences),
    SIM-SCH-014
  - SIM-BEV-001, SIM-BEV-004
  - SIM-RND-003, as the vanilla generator. Other instances can replace it
    (P-RNG).
- **Universal observable result, instance order**: SIM-NBR-001, SIM-NBR-003,
  SIM-NBR-006, SIM-SHP-001, SIM-SHP-003, SIM-SCH-006, SIM-SET-005, SIM-RND-001,
  SIM-RND-002, SIM-RND-004, SIM-BEV-002. At the boundary level, SIM-NBR-003
  holds without its guarantee that the whole chain completes before the call
  returns. Only the part of the chain inside the halo completes (SIM-MODEL-102).
- **Instance-specific**:
  - SIM-TICK-001 to SIM-TICK-005 (loop, clocks, gates, section properties)
  - SIM-SET-003 (load)
  - SIM-NBR-004 (chain scope), SIM-NBR-007 (draws from the level random)
  - SIM-SHP-004 (the neighbor state that a queued shape update gives, P-CAPTURE)
  - SIM-SCH-003 (sequence scope), SIM-SCH-005 (how queues are grouped),
    SIM-SCH-008 (budget scope), SIM-SCH-009 (phase boundaries are for each step),
    SIM-SCH-015, SIM-SCH-016, SIM-SCH-017
  - SIM-BEV-003 (eligibility input)
  - SIM-RNG-001.

All instances of this project also keep the deviations of B.4.

Provenance: `derived`, checked against B.3. Confidence: medium. Each entry
follows from the instance comparison. The confidence of that comparison is
itself mixed.

---

# Part B — Instances

## B. Instances

### B.1 Parameter map

| Parameter | Vanilla (B.2, sections 1–9) | Threaded server (B.3) |
|-----------|-----------------------------|-----------------------|
| P-SHAPE | one partition for each dimension | unions of square groups of columns (default 16 × 16 columns, full height) |
| P-FORM | none | groups that have any column with a load record. Each group is padded by one group on all sides. Merged when padded groups touch. Split after a step when enough groups emptied (B.3) |
| P-HALO | all positions of all dimensions. A step can set blocks in another dimension synchronously (SIM-TICK-001) | owned groups; for most update paths, a 25-block margin must also be owned |
| P-CLOCK | lock-step: partition time = world time, shared by all dimensions | free-running for each partition; rebased on merge |
| P-SCHED | one loop, one dimension after another | each partition on a thread pool with its own deadline; a late partition skips periods and does not catch up |
| P-SEQ | one counter for each dimension | one atomic counter for each dimension, shared by all partitions |
| P-COLLECT | vanilla column merge (SIM-SCH-006) | the same algorithm in each partition |
| P-BUDGET | 65 536 for each kind in each dimension | for each kind in each partition; configurable, default 65 536 |
| P-CHAIN | one chain for each dimension | one chain for each partition |
| P-EVENTS | one FIFO for each dimension | one FIFO for each partition; on merge, the events of the absorbed partition are appended |
| P-FOREIGN | synchronous (the halo covers all targets) | neighbor and shape updates: discard. A set, a call that schedules a tick or a call that queues an event: refuse. A read of a loaded foreign position: unsynchronized read |
| P-LOAD | yes, synchronously, for each column; a temporary ticket holds the column (SIM-SET-003) | updates do not load; other reads and writes load an owned column synchronously and refuse a foreign column |
| P-PROPS | tickets for each column (SIM-TICK-005) | the tick state of each column in its load system |
| P-RNG | one level random and one position generator for each dimension | one thread-local level random, shared by all work that runs on the thread. One fast random for each partition, for random ticks. The position generator draws from the thread-local random |
| P-RANDOM | vanilla picks | picks over a list of the positions that tick, one list for each section; the same rate for each block |
| P-GATE | for each server; pause when empty | for each partition, and separately for the global tick; no step; pause when empty is disabled |
| P-WIRE | the default evaluator; the experimental evaluator when `minecraft:redstone_experiments` is enabled | the default evaluator by default; two alternatives can be selected |
| P-IDLE | 300 level ticks after the last keep-active ticket (SIM-TICK-002) | not examined |
| P-CAPTURE | the state captured at request time (SIM-SHP-004) | not examined |

### B.2 The vanilla instance

Sections 1 to 9 below are the vanilla instance. Their rules are exact. A
**Vanilla grouping** paragraph shows where vanilla groups sections into columns.
An exact-parity mode is this instance, with:

- one partition for each dimension
- a lock-step clock
- the vanilla ordering policy for each queue
- the deviations of B.4.

### B.3 The threaded instance

A threaded server is the instance in the right-hand column of B.1. This server is
a fork of a widely used server. It splits each dimension into partitions and ticks
these partitions at the same time. These items of its behavior are important for
the model:

- **Partitions are formed by proximity, not by causality.** Columns are grouped
  into square groups (default 16 × 16 columns). A group belongs to a partition
  when any column in it has a load record. Each such group pads itself with its
  eight neighbor groups.
  - Partitions merge when their padded groups touch. As a result, the owned areas
    of two partitions are at least one group apart.
  - Merges apply between steps. A merge is deferred while a partition that takes
    part in it is inside a step.
  - Splits are examined when a partition finishes a step and enough of its groups
    emptied.
- **Partition time is free-running.** Each partition has its own clock. The clock
  of a new partition starts at 1. The clock advances one time in each step of the
  partition. Scheduled ticks, saved tick delays and redstone torch burnout use
  this clock. The global tick advances world time separately, with catch-up.
  - On merge, the due ticks of the absorbed partition are shifted by the clock
    difference.
  - On split, each part copies the clock of the parent.
- **Cross-partition effects are discarded or refused.** A neighbor or shape update
  is skipped if its target is not owned with a 25-block margin, or if its target
  is not loaded. It is an error to schedule a tick or to queue a block event at a
  position that the partition does not own. Nothing is delivered as a message.
- **Global counters stay global and are not always the same for the same
  inputs.** The sub-tick counter is one atomic counter for each dimension, and
  all partitions share it.
- **Known divergences:**
  - Pending ticks of the absorbed partition lose their priority when the clocks
    are different at a merge.
  - Random ticks and precipitation use streams for each partition and for each
    thread.
  - Lava receives one fluid random tick for each pick, not two.
  - Client-sync marks depend on if the column was sent to a player or not.
  - The gates have no step, and they apply for each partition.

The reference sources of this document do not include that fork. Its rules are
not checked against 26.4-snapshot-4.

### B.4 Intentional deviations

All instances of this project keep the contract of Part A at these points. They
do not copy vanilla, also in exact-parity mode. A differential test reads this
list before it reports a difference from vanilla as a defect.

| Id | Vanilla behavior | Behavior of this project | Rules |
|----|------------------|--------------------------|-------|
| DEV-01 | When the server removes a column from memory, it writes the column only if the column is marked unsaved. Scheduled ticks do not mark it. Ticks scheduled after its last save are lost. Ticks that ran after that save, and changed no block, run again after the next load. | Each removed column keeps its pending ticks in the save. A tick that ran does not run again. | SIM-MODEL-004, SIM-SCH-013, SIM-SCH-015 |
| DEV-02 | In the overworld, steps 1 to 4 of the level tick run before game time advances. A tick that these steps schedule with delay 1 runs in step 6 of the same level tick. An example: the sleep check empties beds, and the set of a bed can schedule a tick. | Game time advances before any work of the level tick that can schedule a tick. A tick with delay `d ≥ 1` runs only after `d` partition steps. | SIM-MODEL-004, SIM-TICK-003, SIM-SCH-009 |
| DEV-03 | Two saved elements with the same (position, type) both activate, and both run. | Activation keeps only the first element of each (position, type). Not more than one tick is pending for each pair. | SIM-SCH-004, SIM-SCH-013 |
| DEV-04 | An area clear does not update the group behind "will tick this tick". If a query came before the clear, a cleared pair continues to answer true. | An area clear also removes the pair from that group. | SIM-SCH-010, SIM-SCH-016 |
| DEV-05 | A column record can carry a non-empty `UpgradeData` compound from an old upgrade. When the column gets to block-ticking status, vanilla schedules the neighbor ticks of that compound. It does this again at each new transition while the column stays in memory. | This project refuses such a record at load with an error, as it refuses a record of another world version. | SIM-SCH-013, SIM-SCH-017 |

Provenance of the vanilla column: `source_suggested`. No experiment was run.

Experiment for DEV-01:
1. Put a repeater at the edge of column C. Put its input in the next column D.
2. Load C, then run `/save-all flush`.
3. Freeze the ticks.
4. Power the input from D. The repeater schedules a tick, and no block of C changes.
5. Move all players away, so that C unloads while saves are enabled.
6. Return, and unfreeze the ticks.

Result expected from the sources: the repeater never turns on. If it turns on,
remove DEV-01 from this list.

---

# The vanilla instance in detail

---

## 1. Where block work runs

### SIM-TICK-001: Server tick and the gates before it

- **Inputs:** one iteration of the server loop.
- **Relevant state:**
  - the count of online players
  - the server property `pause-when-empty-seconds` (default 60)
  - the tick-rate state (SIM-TICK-004).
- **Preconditions:** none.
- **Behavior:**
  1. If `pause-when-empty-seconds` × 20 is greater than 0, the server counts consecutive server ticks with zero players and no active sprint. The server increments the counter first, then compares it. From the tick on which the count gets to the threshold, the server tick handles only connections. In this paused state:
     - On a dedicated server, the connection work includes queued console commands. The block changes of these commands run with their full chains.
     - No level ticks run.
     - The tick-rate state, the time-sync counter and game time do not advance.
     - The server saves the world on the tick on which the count equals the threshold.
  2. If not, the tick-rate state advances (SIM-TICK-004). Then these steps occur:
     1. If the tick runs normally, the `#minecraft:tick` function tag runs. On the first such tick after the server starts or after a reload, `#minecraft:load` runs before it.
     2. If the tick runs normally and the game rule `advance_time` is true, each world clock that is not paused advances by its rate. The fractional part of the rate carries over to the next tick.
     3. At intervals of 20 server ticks, the server sends time synchronization. It carries the game time from before the advance in this tick.
     4. Each dimension runs one level tick (SIM-TICK-002), one dimension after another. The overworld runs first. Then the other dimensions run, in an order that vanilla does not fix. This order comes from the iteration of a hash map over identity-hashed keys.

        The order is not the registration order or the name order. Vanilla does not make sure that this order is stable across runs.
     5. These items run: the connections, the player list and, if the tick runs normally, the game tests. The connections include the base tick of each player and queued console commands.
     6. The autosave countdown decreases by one. At 0, the server saves the world. The next interval is 300 seconds at the current tick rate, and not less than 100 ticks.

  The server handles the queued packets of players at the start of each loop iteration, before step 1. Chat commands and remote-console commands are not in this queue. They run as tasks while the server waits for the next tick. Load-level updates and light work also run during this wait. All this work runs between two server ticks, paused or not. As a result, the block action of a player runs at the game time of the previous tick.
- **Ordering constraints:** level ticks run one after another, in the order above. Any fixed order of the dimensions after the overworld conforms. An exact-parity mode fixes one order.
- **Numeric semantics:** threshold = `pause-when-empty-seconds × 20` server ticks. Only a dedicated server reads this property (default 60 s). Other servers use 0. The threshold counts server ticks at any tick rate. If the property is lowered below the count at runtime, the server pauses without the save of step 1.

  An integrated server has a separate pause. It handles only connections while the game menu pauses it, or while it has no player.
- **Postconditions:** after a server tick that runs normally, game time is one greater (SIM-TICK-003).
- **Edge cases:** a paused server does not count game time. As a result, pending scheduled ticks stay due at the same game time (SIM-SCH-003).
- **Invariants:** level ticks of two dimensions never overlap. But a phase of one dimension can set blocks in another dimension, synchronously:
  - Commands that run in another dimension do this.
  - A non-player entity that changes dimension in its entity phase makes the exit portal or the End platform in this way. A player changes dimension in the connection work, after all level ticks.

  Such a set and its whole neighbor and shape chain run inside the phase of the writer, on the chain of the target dimension. The ticks that it schedules and the block events that it queues join the queues of the target dimension.
- **Conformance tests:**
  - With no player online, set `pause-when-empty-seconds=1`. Force-load the column of the repeater. Schedule a repeater tick for a game time past the pause. Result: game time advances on the first 19 consecutive empty server ticks and stops from the 20th. The repeater tick does not run while the server is paused. Failing twin: with one player online, the tick runs on time.

    Without the forced column, no column runs scheduled ticks when no player is online, after the timed `portal` and `ender_pearl` tickets expire. In that case, the test passes also without the pause.

Provenance: `source_suggested`. Confidence: high.

### SIM-TICK-002: Phases of one level tick

- **Inputs:** one level tick of one dimension.
- **Relevant state:**
  - if the tick runs normally (SIM-TICK-004)
  - if the world is a debug world
  - the game rule `random_tick_speed`
  - if the dimension has active tickets.
- **Preconditions:** SIM-TICK-001 got to this dimension.
- **Behavior:** the level tick runs these steps in this order. A step that is marked *gated* runs only when the tick runs normally.

| Step | Work | Gated | Block-simulation content |
|---|-------|-------|--------------------------|
| 1 | world border | yes | none |
| 2 | weather | yes | none |
| 3 | sleep check (can jump the default clock and clear the weather) | no | none |
| 4 | sky brightness | no | none |
| 5 | game time advance (overworld only, SIM-TICK-003) | yes | none |
| 6 | scheduled block ticks (SIM-SCH-006) | yes, and skipped in a debug world | block ticks |
| 7 | scheduled fluid ticks (SIM-SCH-006) | yes, and skipped in a debug world | fluid ticks |
| 8 | raids | yes | none |
| 9 | load work, spawning work, random ticks, client broadcast, unload and save work. The list below gives the order | partly | random ticks; precipitation; client sync of block changes; the save of pending ticks (SIM-SCH-012) |
| 10 | block events (SIM-BEV-002) | yes | block events |
| 11 | the dragon fight, if the dimension has one and the tick runs normally; then entities. Both run only while the empty-dimension counter is less than 300. Refer to Numeric semantics | for each entity | the dragon fight can set blocks |
| 12 | block entities (same counter condition). Each ticker runs only if these five conditions are true. The tick runs normally. Its position *delivers block events*. Its column has load level 32 or less, and its entities are loaded. It is inside the world border. Its type is valid for the block state that its position has at that time | yes | none here |
| 13 | entity management, debug | no | none |

  Step 9 runs these items in this order:
  1. The ticket purge: timed tickets count down and expire. This item runs only if the tick runs normally.
  2. Load-level updates. This item always runs.
  3. If the tick runs normally and the world is not a debug world: spawning work over the spawning area. This work is the shuffle of the spawning area, thunder and natural spawning.
  4. Under the same condition: random ticks and precipitation picks (SIM-RND-001).
  5. Under the same condition, if the game rule `spawn_mobs` is true: the special spawners.
  6. If the world is not a debug world: the broadcast of changed blocks to clients (SIM-SET-005). This item runs also when the tick does not run normally.
  7. Entity tracking and point-of-interest work. These items always run.
  8. Only while saves are enabled: unload work (SIM-SCH-015) and the save of changed columns.

  Neighbor and shape updates are not a phase. They run synchronously inside any phase that causes a block change (SIM-NBR-003).
- **Ordering constraints:** the observable result and the vanilla order are the same for the phase order. In particular:
  - All scheduled block ticks of a level tick run before all its scheduled fluid ticks.
  - Both run before random ticks.
  - Random ticks run before block events.
  - Block events run before entities and block entities.
- **Numeric semantics:** each dimension has its own empty-dimension counter. The counter starts at 0, and it is not saved. This counter is the vanilla value of P-IDLE.
  - After step 10, the counter is reset to 0 if the dimension holds a ticket of a type that keeps it active. This reset occurs also when the tick does not run normally. In vanilla, these types keep it active: `player_simulation`, `forced`, `portal`, `ender_pearl`. These types do not keep it active: `player_loading`, `player_spawn`, `spawn_search`, `dragon`, `unknown`. Then, if the tick runs normally, the counter is incremented.
  - Steps 11 and 12 run if, and only if, the counter is less than 300.
  - The counter is also reset when a non-player entity arrives from another dimension, or when an ender pearl registers its ticket. A player that arrives does not reset it.
  - If all ticks run normally, entities and block entities run for 298 level ticks after the last level tick with such a ticket. They stop on the 299th.
- **Postconditions:** none beyond the phases.
- **Edge cases:**
  - The client broadcast of block changes (step 9) runs after the scheduled ticks and the random ticks of this tick. But it runs before block events, entities and block entities. Block changes that steps 10–12 make get to clients in step 9 of the next level tick.
  - Thunder and natural spawning for the whole spawning area run before any random tick of the level tick.
- **Invariants:** a scheduled tick never runs after a block event of the same level tick.
- **Conformance tests:**
  - At game time G, schedule a block tick with delay 1 for a probe block at P. At the same game time, schedule a fluid tick with delay 1 for a probe fluid at Q. At the same game time, queue a block event for the probe block at P. Result: during the level tick of G+1, the log shows this order: block tick, fluid tick, block event. Failing twin: the log shows the fluid tick before the block tick.
  - Let the probe block at P queue a block event in its random tick. Result: the event is delivered in the same level tick, because step 10 comes after step 9.

Provenance: `source_suggested`. Confidence: high.

### SIM-TICK-003: Game time and the due tick

- **Inputs:** a level tick that runs normally.
- **Relevant state:** game time `G` before the tick.
- **Preconditions:** none.
- **Behavior:** in step 5 of the level tick of the overworld, game time changes to `G + 1`. The queue of scheduled server functions for `G + 1` also runs in this step. Other dimensions do not advance game time. They read the shared value. As a result, the scheduled ticks of all dimensions in this server tick run with *current tick* = `G + 1`. Steps 1 to 4 of the overworld read `G` (B.4, DEV-02).
- **Ordering constraints:** game time advances before any scheduled tick of the same server tick runs, in each dimension.
- **Numeric semantics:** a 64-bit integer. It increases by 1 in each server tick that runs normally.
- **Postconditions:** a tick scheduled with delay `d` at game time `T` is due at `T + d` (SIM-SCH-003).
- **Edge cases:** during a tick that does not run normally, game time does not advance and no scheduled tick runs. As a result, due ticks continue to wait. If a dimension is not the overworld, its own level data ignores writes to game time.
- **Invariants:** game time is the same in all dimensions at any instant.
- **Conformance tests:** in any dimension, schedule a tick with delay 2 while game time is 100. Result: the tick runs in the level tick in which game time changes to 102.

Provenance: `source_suggested`. Confidence: high.

### SIM-TICK-004: The tick-rate gate (freeze and step)

- **Inputs:** `/tick freeze`, `/tick unfreeze`, `/tick step N`, `/tick step stop`, `/tick sprint`.
- **Relevant state:** `frozen` (boolean), `steps_left` (integer ≥ 0).
- **Preconditions:** none.
- **Behavior:** at the start of each server tick, *runs normally* := `!frozen || steps_left > 0`. Then, if `steps_left > 0`, `steps_left` decreases by one.
  - When the tick does not run normally, these items do not run:
    - world border
    - weather
    - game time and the scheduled function queue
    - world clocks
    - `#minecraft:tick` and `#minecraft:load` functions
    - scheduled block and fluid ticks
    - raids
    - ticket expiry
    - random ticks and spawning work
    - block events
    - the increment of the empty-dimension counter. A keep-active ticket still resets the counter.
    - the dragon fight
    - game tests
    - block entity ticks
    - non-player entities. But players, entities with a player passenger at any depth, and all entities that ride such an entity continue to tick. A player does not push other entities while the tick does not run normally.
  - These items continue to run:
    - load-level updates and light updates
    - load work and unload work. But columns that only timed tickets hold stay loaded, because those tickets do not count down.
    - the sleep check and sky brightness
    - the client broadcast of block changes
    - entity management and entity tracking
    - time synchronization and the autosave countdown
    - each neighbor and shape update that is caused by an item that continues to run (for example, when a player places a block).
- **Ordering constraints:** none beyond SIM-TICK-002.
- **Numeric semantics:** `/tick step N` lets N server ticks run normally, not more and not fewer.
- **Postconditions:** block events that are queued while the server is frozen stay queued. Scheduled ticks stay queued, and only game time makes them due.
- **Edge cases:**
  - If a block is placed while the server is frozen, the freeze does not stop its chain. Its whole neighbor/shape chain runs immediately. Any tick that it schedules is due relative to the frozen game time.
  - `/tick step N` replaces the number of steps that are left, and does not add to it. `/tick step N` fails if the server is not frozen. `/tick step stop` sets the steps that are left to 0. `/tick freeze` cancels an active step and an active sprint.
  - A *sprint* (`/tick sprint`) runs ticks as fast as possible. It unfreezes the server for the sprint. When the sprint ends, the server returns to the freeze state that it had before the sprint.
- **Invariants:** game time advances on the ticks that run normally, and only on these ticks.
- **Conformance tests:**
  - Freeze the server. Let a player power a repeater with delay 1. Result: no change after 10 server ticks. After `/tick step 2`, the repeater output turns on. Repeater delay 1 is 2 game ticks. Failing twin: the output turns on while the server is frozen.

Provenance: `source_suggested`. Confidence: high.

### SIM-TICK-005: Section properties for each kind of work

- **Inputs:** a section.
- **Relevant state:** the inputs from which the host calculates the properties. In vanilla, these inputs are:
  - the load level and the simulation level of the column of the section
  - if the entities of the column are loaded
  - if the column got to block-ticking status.
- **Preconditions:** none.
- **Behavior:** each kind of work tests one property of the section that holds the position in question. The properties are section-native. Vanilla calculates each of them from the column. As a result, all sections of one column share them.

| Property | Gates | Vanilla derivation (column-grouped) |
|----------|-------|-------------------------------------|
| *loaded* | the "is loaded" test (SIM-NBR-006). A read or a set at a position in a section that is not *loaded* loads it synchronously (SIM-SET-003) | load level ≤ 33. Vanilla checks nothing else. As a result, a column with a load in progress counts as loaded, and a read there waits until the load finishes |
| *accepts ticks* | scheduled ticks are stored, not discarded (SIM-SCH-005) | the column is in memory at full status: from the moment it finishes its load to full status until it is unloaded. The unload condition is a load level above 44. The server processes the unload in step 9 of SIM-TICK-002, possibly many ticks later (SIM-SCH-015). Not equal to *loaded*. A column at load level ≤ 33 with a load in progress does not accept ticks. A column demoted to 34–44 accepts ticks |
| *runs scheduled ticks* | stored ticks can be collected (SIM-SCH-006) | simulation level ≤ 32, and the column got to block-ticking status, and the entities of the column are loaded. Block-ticking status is load level ≤ 32, and for it the eight neighbor columns must be full |
| *delivers block events* | block events are delivered, not deferred (SIM-BEV-003); a precondition of block entity ticks (SIM-TICK-002 step 12) | simulation level ≤ 32; vanilla checks nothing else |
| *random-ticked* | random ticks (SIM-RND-001) | simulation level ≤ 31, and the column got to block-ticking status |
| *broadcasts changes* | sets are marked for client sync (SIM-SET-005) | the column got to block-ticking status, and its load level is 32 or less |

  "The column got to block-ticking status" means that the column completed its transition to block-ticking (SIM-SCH-013 step 3). Post-processing (SIM-SCH-017) and the activation of saved ticks (SIM-SCH-013) occur in this transition. The properties that need block-ticking status turn on directly after them. A load level of 32 or less is not sufficient.
- **Ordering constraints:** none.
- **Numeric semantics:** full = 33, block-ticking = 32, entity-ticking = 31. In vanilla, these values apply to a column at Chebyshev distance d (in columns) from the column of a player:
  - simulation level = max(0, 31 − S) + d. S is the simulation distance (server property `simulation-distance`, default 10).
  - load level from player tickets = 31 + max(0, d − V). V is the view distance, clamped to 2–32.
  - Each ticket type changes the load level, the simulation level, or both:
    - only the simulation level: `player_simulation`
    - only the load level: `player_loading`, `player_spawn`, `spawn_search`, `unknown`
    - both: `forced`, `portal`, `ender_pearl`, `dragon`.
  - A radius ticket of radius r is at 33 − r.

  A column stays in memory while its load level is ≤ 44. This value is 33 plus the 11-column radius of full-status generation dependencies at this version.
- **Postconditions:** none.
- **Edge cases:**
  - For *random-ticked*, the entity-ticking threshold is necessary. For *runs scheduled ticks*, only the block-ticking threshold is necessary. A section at simulation level 32 runs its scheduled ticks but receives no random ticks.
  - For *delivers block events*, it is not necessary that the section is loaded.
  - A column demoted to load level 34–44 keeps its queues in memory. It accepts ticks but runs none. Its ticks keep their absolute due ticks. But a save and reload re-keys them (SIM-SCH-013).
- **Invariants:** *runs scheduled ticks* implies *accepts ticks*.
- **Vanilla grouping:** the ticket system calculates load and simulation levels for each column. These levels have no vertical component: the properties of a section depend only on `(sx, sz)`. A host can calculate properties for each section by any rule. An exact-parity mode gives each section of a column the properties that vanilla gives the column.
- **Conformance tests:** use simulation distance 3 and a view distance of at least 3 (default 10). Put a repeater with a pending tick in a section at simulation level 32. This section is one ring outside the entity-ticking area (d = 4). Put a sapling in the same section. Result: the pending repeater tick runs, but the sapling never grows by random ticks. Failing twin: the sapling grows.

Provenance: `source_suggested`. Confidence: high.

---

## 2. Block sets

### SIM-SET-001: Update flags

- **Inputs:** an integer flag word that is passed with each set.
- **Relevant state:** none.
- **Preconditions:** none.
- **Behavior:** on the server, each bit has the effect that this table shows. The server ignores bits that are not in the table. The names come from this document.

| Bit | Value | Name | Effect |
|-----|-------|------|--------|
| 0 | 1 | *notify-neighbors* | the set makes neighbor updates (SIM-NBR-001) and comparator output updates (SIM-NBR-006). It also lets the removal side effect of the old block run (SIM-SET-002 step 2.5) |
| 1 | 2 | *sync-clients* | the set marks the change for client sync (SIM-SET-005) |
| 2 | 4 | *no-rerender* | client-side only: on a client level, it stops the re-render. No effect on the server |
| 3 | 8 | *rerender-now* | client-side only: re-render synchronously. No effect on the server |
| 4 | 16 | *keep-shape* | the set makes no shape updates, direct or indirect (SIM-SHP-001). During generation, a set **without** this flag marks a position for post-processing (SIM-NBR-005) |
| 5 | 32 | *no-drops* | the server reads it only when a shape update changes a block to air: the block is removed without drops (SIM-SHP-001). At this version, no shape update carries it (SIM-SHP-001). As a result, a set with 35 has the same behavior as a set with 3 |
| 6 | 64 | *piston-move* | the removal side effect of the old block and the placement side effect of the new block receive "moved by piston". The removal side effect runs also when the flags do not have *notify-neighbors* |
| 7 | 128 | *wire-skips-shape* | if a shape update carries this bit and the block that receives the update is `minecraft:redstone_wire`, the update is skipped. At this version, only the experimental wire evaluator sets it, as 2 \| 128 |
| 8 | 256 | *skip-block-entity-effects* | when the old block entity is removed, its pre-removal side effects are skipped. An example of such a side effect: a container releases its contents |
| 9 | 512 | *skip-placement* | the placement side effect of the new state does not run |

  This document refers to these combinations:

| Value | Bits | Used by |
|-------|------|---------|
| 3 | *notify-neighbors*, *sync-clients* | the usual set, destroy (SIM-SET-004), and remove (as 3 \| 64) |
| 11 | *notify-neighbors*, *sync-clients*, *rerender-now* | player placement and item use, and a burning projectile that lights a block. On the server, the same as 3 |
| 258 | *sync-clients*, *skip-block-entity-effects* | non-strict `/setblock` and `/fill` (the note after SIM-SET-005) |
| 260 | *no-rerender*, *skip-block-entity-effects* | state changes that the server does not send to clients, for example growth ages |
| 276 | *no-rerender*, *keep-shape*, *skip-block-entity-effects* | post-processing (SIM-SCH-017), and a piston when it starts a move |
| 324 | *no-rerender*, *piston-move*, *skip-block-entity-effects* | a piston when it puts blocks in motion |
| 818 | *sync-clients*, *keep-shape*, *no-drops*, *skip-block-entity-effects*, *skip-placement* | strict commands, the barriers of `/clone`, the placement of a test instance, and the removal of a sapling when a tree grows |
| 820 | *no-rerender*, *keep-shape*, *no-drops*, *skip-block-entity-effects*, *skip-placement* | template placement: a barrier before a block that has block entity data |

- **Ordering constraints:** none.
- **Numeric semantics:** the flag word is a 32-bit integer.
- **Postconditions:** none.
- **Edge cases:** with 260, the server sends nothing to clients only because 260 does not have *sync-clients*. On the server, *no-rerender* does not stop client sync.
- **Invariants:** a shape update that a set makes carries the flags of that set, masked (SIM-SHP-001). A block that makes shape updates directly gives its own flags.
- **Conformance tests:** the table gives one test for each flag combination. In each test, use a probe block at P, with probe neighbors on all six sides:

| Flags | Neighbor updates logged | Shape updates logged | Marked for clients | Placement logged |
|-------|--------------------------|----------------------|--------------------|------------------|
| 3 | 6 | 6 | yes | yes |
| 2 | 0 | 6 | yes | yes |
| 18 | 0 | 0 | yes | yes |
| 1 | 6 | 6 | no | yes |
| 515 | 6 | 6 | yes | no |
| 816 | 0 | 0 | no | no |

Provenance: `source_suggested`. Confidence: high.

### SIM-SET-002: The steps of one set

- **Inputs:** position P, new state S, flags F, update limit L (default 512).
- **Relevant state:** the state O at P, and the section that holds P.
- **Preconditions:** none.
- **Behavior:**
  1. If P is outside the valid bounds (SIM-SET-003), the server refuses the set. The result is *false*, and nothing occurs. In a debug world, the server refuses all server-side sets.
  2. The server makes the section of P *loaded*. If necessary, it loads the section synchronously (SIM-SET-003). Then:
     1. If the section of P contains only air and S is air, the result is *false* and nothing occurs.
     2. The server writes S. If O is the same state as S, the result is *false* and nothing else occurs.
     3. The server updates these items:
        - the section emptiness
        - the light records
        - in vanilla, the heightmaps of the column of P.
     4. These actions occur if all of these conditions are true:
        - The block type changed.
        - O had a block entity.
        - S does not keep that block entity.

        If F does not have *skip-block-entity-effects*, the pre-removal side effects of the old block entity run. The server removes the block entity.
     5. The removal side effect of O runs if both of these conditions are true:
        - The block type changed, or S is of one of the rail block types: `minecraft:rail`, `minecraft:powered_rail`, `minecraft:detector_rail` or `minecraft:activator_rail`. The test uses the block type, not a tag.
        - F has *notify-neighbors* or *piston-move*.

        The removal side effect gets "moved by piston" = *piston-move*.
     6. If the state at P is now no longer of the block type of S, the result is *false*. Nothing else occurs. This occurs when step 5 replaced the state at P.
     7. If F does not have *skip-placement*, the placement side effect of S runs. It gets the old state O and "moved by piston".
     8. These actions occur if both of these conditions are true:
        - S has a block entity.
        - After 2.7, the state at P continues to be of the block type of S.

        First, if there is a block entity at P that is not valid for S, the server removes it (with a warning). Then, if there is no block entity, the server creates one. If not, the server sets the state of the block entity to S.
  3. Steps 2.5 and 2.7 can change the state at P. If the state at P is now the same state as S, these steps occur:
     1. If F has *sync-clients* and the section of P *broadcasts changes* (SIM-TICK-005), the server marks P for client sync (SIM-SET-005). If F has *sync-clients* and the collision shape at P changed, mobs with a path near P calculate their paths again. Mob paths are outside this document.
     2. If F has *notify-neighbors*, the set makes neighbor updates around P with source block = **O's** block type (SIM-NBR-001). Then, if F has *notify-neighbors* and S has an analog output signal, the set makes comparator output updates (SIM-NBR-006). These updates have source = the block type of S.
     3. If F does not have *keep-shape* and L > 0, the set makes shape updates in this order:
        - the indirect shape updates of O (SIM-SHP-003)
        - then the direct shape updates of S (SIM-SHP-001)
        - then the indirect shape updates of S.

        These shape updates have flags F' = F without *notify-neighbors* and without *no-drops*. They have limit L − 1.
     4. The server updates the points of interest.
  4. If step 2 continued after 2.6, the result is *true*, also when step 3 was skipped.
- **Ordering constraints:** in one set, the observable result and the vanilla order are the same. The order is:
  - the removal side effect
  - then the placement side effect
  - then the mark for client sync
  - then the neighbor updates
  - then the comparator updates
  - then the old-indirect, direct and new-indirect shape updates.

  The removal side effect, the placement side effect and the mark for client sync always run synchronously in the set. Steps 2.6 and 3 depend on this. When no chain is active, the neighbor, comparator and shape updates also run to completion in the set. Each request completes before the set calculates the next request (SIM-NBR-003). In a chain, these updates and any requests that the side effects make are queued in request order.
- **Numeric semantics:** L is decremented one time for each generation of shape updates (SIM-SHP-002).
- **Postconditions:** if the result is true, P held S, at least for a short time.
- **Edge cases:**
  - If the new state is equal to the state that P has at that time, the set does nothing. The result is false. There are no updates and there is no client sync.
  - The neighbor updates carry the **old** block type as their source, not the new block type.
  - A placement side effect can set P again. In this case, step 3 compares S with the newer state. If the newer state is different from S, the server skips all of step 3, but the result is true.
  - A state change inside one rail block type runs the removal side effect, for example a change of shape or of power.
  - If the updates of a set go into other sections, the set has the same behavior as in one section. No step of this rule depends on section or column boundaries.
  - Commands do not follow these steps. Refer to the note after SIM-SET-005.
- **Invariants:** a set that is refused at step 1, 2.1 or 2.2 makes no requests and sends nothing. If a set fails at step 2.6, these actions occurred before the failure:
  - The set wrote S.
  - The set updated the heightmaps and the light.
  - The pre-removal side effects of the old block entity ran.
  - The removal side effect of O ran, together with any requests that this side effect made.
- **Conformance tests:**
  - Put probe block A at P. Replace it with probe block B, with flags 3. Result: the log order is A.removed, B.placed, then six neighbor updates with source A, then six shape updates. Failing twin: neighbor updates with source B.
  - Set the same state two times with flags 3. Result: the second set logs nothing and returns false.
  - Give B a placement handler that sets P to probe C. Then set P to B. Result: step 3 logs no neighbor update and no shape update for the B set. The C set logs its own neighbor and shape updates.

Provenance: `source_suggested`. Confidence: high.

### SIM-SET-003: Sets and reads at unloaded or invalid positions

- **Inputs:** a set, or a read (for example, by a neighbor update or a shape update), at position P.
- **Relevant state:** the *loaded* property of the section of P.
- **Preconditions:** none.
- **Behavior:**
  - **Valid bounds.** P is valid if, and only if, these two conditions are true:
    - The y of P is in the vertical extent of the dimension (dimension type `min_y` ≤ y < `min_y + height`, vanilla data).
    - `max(|sx|, |sz|)` ≤ 2 097 061 = (33 554 431 >> 4) − 90.

    In this formula, the margin is 90 = (32 + 12 + 1) × 2. The value 12 is the number of dependency rings of full-status generation at this version. In blocks, −33 552 976 ≤ x, z ≤ 33 552 991. The ±30 000 000 world limit of commands and spawns is a separate bound, and it is smaller.

    A read at an invalid position returns `minecraft:void_air` (fluid: empty) and loads nothing. The server refuses a set at an invalid position.
  - **Synchronous load.** This rule applies on the server main thread to a set or a read at a valid position. It applies if one of these two conditions is true:
    - The section of the position is not *loaded*.
    - The load of the column of the position is not complete.

    In this case, the set or the read **loads (or generates) it synchronously**. Then the set or the read continues.

    While the set or the read waits, the server runs the queued load work of the same dimension: load-level updates, light scheduling and load tasks. These tasks include the completion of block-ticking status of other columns. Other server tasks do not run. As a result, the post-processing and the saved-tick activation of these columns (SIM-SCH-017, SIM-SCH-013) can occur in the middle of a chain.

    Then the section is *loaded* and *accepts ticks*, for at least the rest of the level tick. But it does not *run scheduled ticks*, if no other cause makes it do so. As a result, ticks that are scheduled into it wait (SIM-SCH-015).

    It *delivers block events* only if it was in simulation range before the load. Also:
    - It is not *random-ticked*.
    - It does not *broadcast changes*.
    - Its block entities do not tick.
- **Ordering constraints:** the load completes before the set or the read continues.
- **Numeric semantics:** refer to Vanilla grouping.
- **Postconditions:** the section of P is *loaded* for at least the rest of the tick.
- **Edge cases:** in the machinery of this document, only comparator output updates (SIM-NBR-006) test if a position is loaded. The neighbor updater itself does not do this test. Some block behavior also does not load:
  - the fire spread of lava
  - the block-pattern check
  - game events, sculk vibrations and sculk spread.

  All other block-state reads load. For commands, refer to the note after SIM-SET-005.
- **Invariants:** a set never writes into a section while the section is unloaded. It loads the section first.
- **Vanilla grouping:** the unit that vanilla loads is the complete column of P, generated to full status. Vanilla holds the column at full status with a temporary ticket on the column. This ticket is `minecraft:unknown`, level 33, for load only, with timeout 1. It does not change the simulation level.

  Each main-thread access that loads, and that misses a cache of four entries, adds that ticket or resets it. Accesses that do not load also fill this cache. The server clears the cache at the end of step 9 and after each change of load levels. As a result, an access that hits the cache does not reset the ticket.

  The purge at the start of step 9 of SIM-TICK-002 counts the ticket down, but only on ticks that run normally. The second purge after the access removes the ticket. This gives these results:
  - If the ticket is added in steps 1–8 of tick N, it holds until step 9 of tick N + 1.
  - If it is added later in N, or between ticks, it holds until step 9 of N + 2.
  - While the server is frozen, the ticket never expires.

  When the ticket expires, the column gets the load level that other tickets give it (SIM-SCH-015). An exact-parity mode loads all sections of that column in the vertical extent of the dimension. It gives the same properties to all of these sections.
- **Conformance tests:** make an immediate chain (redstone dust, not scheduled observers) that runs out of the loaded area. Result: the chain loads each column that it gets to. After the chain, these columns go back to the levels that other tickets give them. With save on, only those farther than Chebyshev distance V + 13 from all players are unloaded (OQ-03, answered). The unload can come many ticks later (SIM-SCH-015).

Provenance: `source_suggested`. Confidence: high.

### SIM-SET-004: Block destruction

- **Inputs:** a destroy at P, a drop flag, update limit L (default 512), and an optional breaker entity.
- **Relevant state:** the state O at P, and its fluid.
- **Preconditions:** none.
- **Behavior:** if O is air, nothing occurs (result false). If not, these steps occur:
  - The server sends level event 2001 with the id of O to the players within 64 blocks. Some blocks send another event or no event (fire sends none). This is block behavior.
  - If the drop flag is set, the server spawns the drops of O. Then O runs its after-break effects with no tool, for example an ore drops experience. The drops and the game event use the breaker.
  - Then the server sets P to the block form of the fluid at P, with flags 3 and limit L (SIM-SET-002). This block form is air, or the source/flowing block of the fluid.

  If that set succeeds, the server sends a `block_destroy` game event.

  "Remove block" (without drops) sets the block form of the fluid with flags 3. If the caller requests *piston-move*, the flags also have *piston-move*. "Remove block" has no air check, no particles and no game event. It uses limit 512.
- **Ordering constraints:** drops come before the set. When no chain is active, the updates of the set complete before the game event. In a chain, the server sends the game event after the side effects of the set and its mark for client sync. But the server sends the game event before the queued neighbor, comparator and shape updates of the set run. Each destroy that a shape update makes is in a chain.
- **Numeric semantics:** none.
- **Postconditions:** a waterlogged block that is destroyed leaves water.
- **Edge cases:** a destroy always makes neighbor updates (flags 3). This is also true when a shape update caused the destroy, and the flags of that shape update did not have *notify-neighbors*.
- **Invariants:** none.
- **Conformance tests:** make a shape update change a probe block to air. Result: neighbor updates around the probe block are logged. Failing twin: none are logged.

Provenance: `source_suggested`. Confidence: high.

### SIM-SET-005: Client sync of block changes

- **Inputs:** sets with *sync-clients*, and positions that are marked directly without a set. These are examples of a direct mark:
  - a block entity with changed data
  - the `/data` command, or a command with block entity data
  - a spawn egg used on a spawner
  - a refused break of an operator-only block.

  For a direct mark, only the eligibility below applies. The flags that the caller passes have no effect on this.
- **Relevant state:** for each section, the group of marked positions.
- **Preconditions:** the section of P *broadcasts changes* when the set occurs. Vanilla tests this two times:
  - The set tests the load level of the column. It must be 32 or less.
  - The mark itself needs the column to have completed its transition to block-ticking.
- **Behavior:** the mark records only the position, in its section.

  In step 9 of each level tick (SIM-TICK-002), each section that has marks sends packets, if its column continues to *broadcast changes*. The section sends to the players that see it. These are the players that meet these two conditions:
  - The view of columns that they track contains the column.
  - The column does not wait to be sent to them.

  The section sends one of these packets:
  - If only one position is marked: one `block_update` packet with the state that the position has **at broadcast time**.
  - If not: one `section_blocks_update` packet with the states that all marked positions have at that time.

  For each sent position whose state has a block entity, the server then sends the data packet of that block entity, if it has one. Then the server clears the marks.

  A column that does not broadcast changes at broadcast time keeps its marks, but it leaves the broadcast queue. The server queues it again when the column is ready to send again. A light change in a section without light marks also queues it again. A new block change does not queue it again.
- **Ordering constraints:** observable result: one packet for each marked section in each broadcast, with the states at broadcast time. Vanilla order: refer to Vanilla grouping.
- **Numeric semantics:** for the packet ids, refer to 0.5.
- **Postconditions:** through the broadcast, a client never sees an intermediate state of a position that changed two times between broadcasts. Some paths send a `block_update` immediately, outside the broadcast, with the state at that time:
  - A falling block that lands. It also sends `add_transient_block`. It sends the two packets to the players that track the falling block.
  - The answers of the server to the dig and use actions of a player. Only that player receives them.
  - The server opens a sign editor for a player.
  - A player claims ground contact without support. The server then sends the blocks under the player, not more than one time in 200 ticks.
- **Edge cases:**
  - A set with *sync-clients* in a section that is *loaded* but does not *broadcast changes* is not marked at all. The server sends only block-ticking columns to all players. A player gives tickets, unless it is a spectator and the game rule `spectators_generate_chunks` is false. Each column that such a player tracks has load level 32 or less from those tickets. As a result, this case occurs only for a column that is demoted while a player tracks it.

    An example: another player loaded the column, and a spectator watches it while `spectators_generate_chunks` is false. When the column is block-ticking again, the server sends the complete column again to each player that tracks it. This second send tells the client about the change (OQ-06, answered).
  - If a position is set and then set back to its original state before the broadcast, the broadcast sends a packet for it. This packet carries the original state.
  - If no player sees the section, the server clears its marks and sends nothing.
- **Invariants:** not more than one block-state packet for each section in each broadcast. Block entity data packets can follow it.
- **Vanilla grouping:** vanilla keeps the marks for each section in a record for each column. "The players that see a section" are the players that track its column. The broadcast visits columns in the iteration order of an identity-keyed hash set. This order is not the insertion order, and it is not always the same from one run to the next.

  In one column, the broadcast visits sections from the bottom up. The only order that an exact-parity mode must keep is "bottom up within a column". The order across columns is free.
- **Conformance tests:** set P to stone. Then set P to dirt in the same scheduled-tick phase. Result: the client receives one `block_update` with dirt. Failing twin: two packets.

Provenance: `source_suggested`. Confidence: high for content and for the order inside a column. Confidence is also high that the order across columns is not always the same from one run to the next.

### Note: block changes by commands

This note is informative. Commands are not part of the block simulation
machinery. A black-box experiment that changes blocks by command must account for
these differences from SIM-SET-002.

- **Target test.** A command tests only the positions or the columns that it
  names. `/setblock` tests its target. `/fill` tests only its two corners. A column
  between the corners that is not loaded loads synchronously (SIM-SET-003).
  `/clone` tests all columns of its two areas.
- **`/setblock` and `/fill`, not strict.** For each position, these steps occur:
  1. The command adapts the new state to the six neighbors with their shape
     functions. If the result is air, the command keeps the original state. Then
     it puts back each property that the command names.
  2. The command sets the state with flags 258. There are no neighbor updates,
     but shape updates run.
  3. A follow-up runs. If the block type changed, the follow-up runs the removal
     side effect of the old state. The rail case of SIM-SET-002 does not apply.
     Then the follow-up makes neighbor updates around P with the **new** block
     type as source. Then, if the new state has an analog output signal, it makes
     comparator output updates.

  `/setblock` runs the follow-up directly after its set. `/fill` sets all
  positions first. Then it runs all follow-ups, in the same position order.
- **Strict mode.** `/setblock` and `/fill` set with 818 and send nothing else.
  There is no follow-up.
- **Destroy mode.** The command first destroys the old block with drops
  (SIM-SET-004). `/setblock` compares the new block with the state after the
  destroy. `/fill` compares it with the state before the destroy. As a result,
  `/fill` runs the removal side effect of the original block two times.
- **Block entity data.** If the data of the command changes the block entity, the
  command marks P for client sync directly. The command then succeeds, also when
  the set returned false.
- **`/clone`, not strict.** The command sets each target to a barrier with 818.
  Then it sets each block with flags 2. Then it runs the follow-up of `/setblock`
  for each position, in reverse order. A move clears the source with barriers
  (818), then with air (flags 3). After all sets, the command copies the block
  ticks of the source (SIM-SCH-016).
- **`/clone`, strict.** The command uses 818 for each set. There is no follow-up.

---

## 3. Neighbor updates

### SIM-NBR-001: Neighbor updates around a position

- **Inputs:** a source position P and a source block type B. The optional inputs are a skipped direction X and an orientation (SIM-NBR-007).
- **Relevant state:** the state of each of the six neighbors of P at the time when that neighbor gets its update.
- **Preconditions:** none.
- **Behavior:** for each direction d in neighbor order `W, E, D, U, N, S`, except X, the neighbor Q = P + d receives "neighbor changed". The arguments are (Q's state **read at the time of this update**, Q, B, orientation, moved-by-piston = false).
- **Ordering constraints:**
  - Observable result: each neighbor, except X, receives one update only, and that update carries B.
  - Vanilla order: `W, E, D, U, N, S`, with the cascade rule of SIM-NBR-003 between the updates.
- **Numeric semantics:** none.
- **Postconditions:** none.
- **Edge cases:** the state that the update reads for a later direction shows all changes that the chains of earlier directions made. Neighbors in other sections get their updates in the same way. If a neighbor is in an unloaded section, the read loads it (SIM-SET-003).
- **Invariants:** not more than six updates for each request.
- **Conformance tests:** put probe blocks on all six sides of P. Do a set at P with flags 1. Result: the log shows updates at P+W, P+E, P+D, P+U, P+N, P+S, in that order. Failing twin: the updates come in shape order.

Provenance: `source_suggested`. Confidence: high.

### SIM-NBR-002: Kinds of queued update

- **Inputs:** an update request.
- **Relevant state:** none.
- **Preconditions:** none.
- **Behavior:** a request is one of four kinds. The kinds are different in what the request captures when it is made, and in what the request reads when it runs.

| Kind | Captured at request | Read at run | Updates |
|------|---------------------|-------------|-------|
| neighbors-around (SIM-NBR-001) | P, B, skipped direction, orientation | the state of each neighbor, in each update | up to 6 |
| single, by position | target Q, B, orientation. The value of moved-by-piston is always false | Q's state | 1 |
| single, with state | target Q, **Q's state**, B, orientation, moved-by-piston | nothing. The request uses the captured state | 1 |
| shape update (SIM-SHP-001) | target Q, direction, **the state of the changed neighbor** (vanilla, P-CAPTURE), the position of that neighbor, flags, limit | Q's state | 1 |

- **Ordering constraints:** only the constraints of SIM-NBR-003.
- **Numeric semantics:** each request counts as one toward the chain limit (SIM-NBR-004). The number of updates of the request has no effect on this.
- **Postconditions:** none.
- **Edge cases:** a "single, with state" request runs the handler of the captured state, even if Q changed between the request and its run. Comparator output updates use this kind. In vanilla, a shape update carries the neighbor state that it captured at request time. It carries this state also when the neighbor changed again (SIM-SHP-004).
- **Invariants:** none.
- **Conformance tests:** inside a chain, request "single, with state" for Q while Q holds probe A. Before the request runs, change Q to probe C. Result: the log shows the handler of A, not the handler of C.

Provenance: `source_suggested`. Confidence: high.

### SIM-NBR-003: The chain and its traversal order

- **Inputs:** update requests (SIM-NBR-002) made from any place: a set, a block handler, a tick.
- **Relevant state:**
  - if a chain is active
  - the stack of requests of the chain
  - the list of requests added during the update that runs now (the *added* list)
  - the request count of the chain.
- **Preconditions:** none.
- **Behavior:**
  - **No chain is active.** The request starts a chain. All of the chain runs to completion before the request call returns. As a result, a set made outside any chain returns only after all the updates that it caused, directly or through other updates, are complete. At the boundary level, this rule applies only to the part of the chain inside the halo (SIM-MODEL-102).
  - **A chain is active.** The chain adds the request to the end of the *added* list, and the call returns immediately. The handler that made the request finishes before any of its requests run.
  - **Traversal.** The chain keeps a stack. The chain repeats these steps until the stack and the *added* list are both empty:
    1. Push the *added* list onto the stack in reverse order. The first element of the list is then on top.
    2. Clear the list.
    3. Take the top request and run its updates one at a time:
       - After the last update of the request, pop the request immediately. Do this even if that update added requests.
       - If an update that is not the last update added accepted requests, stop the request immediately after that update. The request stays on the stack, under the new requests.
  - When the chain ends, its count resets to 0. This is true if the chain ended normally, and also if an exception in a handler ended it.
- **Ordering constraints:**
  - Observable result: each accepted request runs one time only, and each update of a request runs not more than one time. A handler that runs inside a chain completes before any request that it made runs. A request made when no chain is active completes, together with all of its chain, before the call returns. At the boundary level, only the part of the chain inside the halo completes before the call returns.
  - Vanilla order: depth first. After each update, all the requests that the update added run before the next update of the same request. They run in the order in which the step added them. Each of them runs completely and depth first. Requests made during one update keep their request order.
- **Numeric semantics:** none in this rule. For the limit, refer to SIM-NBR-004.
- **Postconditions:** when a chain ends, the stack and the list are empty.
- **Edge cases:**
  - In SIM-SET-002 step 3, the set makes its requests in this order. As a result, they run in this order:
    1. neighbors-around P
    2. comparator updates
    3. old-indirect shapes
    4. the six direct shape updates, in shape order
    5. new-indirect shapes.

    Requests that the removal side effect and the placement side effect make (steps 2.5 and 2.7) come before all of them.
  - Outside a chain, each request starts its own chain. That chain completes before the next request is calculated. As a result, the comparator scan (SIM-NBR-006), the indirect-shape scans (SIM-SHP-003) and the states that they capture see the effects of the earlier chains. Inside a chain, all requests are calculated during the set. The request order is the same in the two cases. The content can be different, and each request has its own count.
  - If a handler makes a request inside a chain and then reads the world, it does not see the effects of that request.
  - Each dimension has its own chain, and a chain can extend across any number of sections. Section and column boundaries have no effect on the order and the count of a chain.
- **Invariants:** the traversal is iterative. The depth to which handlers are nested does not increase with the chain length. Each server dimension delivers neighbor updates and shape updates through this traversal. A client dimension has its own chain, with a fixed limit of 1 000 000, and that chain carries only shape updates. On a client, neighbors-around, single and comparator requests do nothing. The client applies the block changes that it receives from the server with *keep-shape*.
- **Conformance tests:**
  - Put probe A at P and probes at all six neighbors of P. Put probes at all neighbors of P+W+W. When P+W gets an update, its handler sets P+W+W to probe Z with flags 17. Do a set at P with flags 17. Result: the log shows P+W, then the six neighbors of Z in neighbor order, then P+E, P+D, P+U, P+N, P+S. Failing twin: P+E comes before the neighbors of Z (breadth first).

    Flags 17 are *notify-neighbors* and *keep-shape*, so no shape update gets into the log. With flags 1, the six shape updates of Z run between the neighbors of Z and P+E.
  - Do the same test, but make the handler of P+W log "after-set" after its set call. Result: the log shows "after-set" before the neighbors of Z. Failing twin: the log shows it after them (recursive execution).
  - Do a set at P from a scheduled tick. At that time, no chain is active. After the set, log the line "returned". Result: "returned" is in the log after all neighbor updates of the chain.

Provenance: `source_suggested`. Confidence: high.

### SIM-NBR-004: The chain limit

- **Inputs:** requests in one chain.
- **Relevant state:** the request count `c` of the chain, and the limit `M`. The count `c` is the number of requests offered until now in this chain, accepted or not.
- **Preconditions:** none.
- **Behavior:**
  - When a request is offered with `M ≥ 0` and `c ≥ M`, the chain discards it. If not, the chain accepts it. In both cases, `c` increases by one.
  - When the chain discards the first request (`c` was `M`), one error is logged. The error names the position of that request: for a neighbors-around request, the source P, and for a single or shape request, its target. The text of the error is: "Too many chained neighbor updates. Skipping the rest. First skipped position: x, y, z".
  - The chain continues with the requests that it accepted before. Because `c` only increases, the chain discards all later requests of the same chain.
  - `M < 0` means that there is no limit.
- **Ordering constraints:** the discarded requests are all the requests offered after the first `M`, and only these. "First" refers to the order in which the requests are offered.
- **Numeric semantics:** `M` = server property `max-chained-neighbor-updates`, default 1 000 000. The integrated (single-player) server uses 1 000 000. The first request of the chain counts.
- **Postconditions:** not more than `M` requests run in each chain.
- **Edge cases:** `M = 0`: nothing ever runs, and the first request of each chain logs the error. A neighbors-around request counts one time, but it updates up to six neighbors. The count resets only when the outermost chain ends. As a result, two sets made outside any chain have independent request counts. The limit is for each chain, not for each section.
- **Invariants:** if `M ≥ 0`, the server never crashes because of a long chain.
- **Conformance tests:** use `M = 3`, with probes at the six neighbors of P. Each probe requests one single update for an inert position when it gets an update. Do a set at P with flags 1. Result: the requests offered are 1 (around P) + 6 = 7. The chain accepts the around-P request and the first two single requests, from P+W and P+E. Only one error is logged, and it names the target of the third single request.

  Failing twin: the chain stops, and P+D, … get no updates. In the correct result, they get updates, and the chain discards only their requests.

Provenance: `source_suggested`. Confidence: high.

### SIM-NBR-005: World generation path

- **Inputs:** block sets made through the generation view while a column is generated (features, carvers, structure pieces).
- **Relevant state:** none.
- **Preconditions:** the change is made through the generation path, outside the server tick.
- **Behavior:**
  - A set through the generation view writes the block, and also its block entity and its point of interest. The set makes **no neighbor updates and no shape updates**. There is no chain, no count and no limit.
  - If the target column is already at full status, the set writes nothing, but its result is true. A tick for that column is discarded, and a post-processing mark for it is ignored.
  - If the target is outside the write radius of the generation step, the server refuses the set and logs an error.
  - If the set does not have *keep-shape*, it marks the position that the new block names for post-processing (SIM-SCH-017):
    - Brown and red mushrooms name themselves.
    - Soul sand and magma blocks name the block above.
    - All other blocks name no position.
  - Neighbor-update requests made on the generation view do nothing. A shape-update request made on the generation view runs at once, and the view writes its result. No vanilla generation code makes such a request at this version.
  - Shapes are made consistent in three other ways:
    - Post-processing, when the column completes its transition to block-ticking (SIM-SCH-017).
    - Template placement, if the template shapes are not known. Template placement then runs a *face pass* and a recalculation. All these writes go through the generation view, which propagates nothing.
    - Tree placement. A tree sets its blocks with *keep-shape* (flags 19). Then it sets the distance value of each leaf that it reaches from its logs. Then it runs the face pass over the tree volume, with flags 3. The tree volume is the logs, the roots, the decorations and the leaves that the tree reached.

  The *face pass* visits each face between a placed position and a position that was not placed. Faces inside the bounding box also count. The pass visits the north and south faces first, then the down and up faces, then the west and east faces. At each face, the shape function updates the placed block first. Then it updates the outside block from the new state of the placed block. These writes do not have *notify-neighbors*.

  The *recalculation* of template placement visits the placed blocks in placement order. It calculates each block again from its six neighbors, in shape order. These writes also have *keep-shape*. The order of positions inside each group of faces belongs to the specification of structure placement.
- **Ordering constraints:** none.
- **Numeric semantics:** none.
- **Postconditions:** none.
- **Edge cases:** the placement of a feature into a live dimension (for example by command) goes through the dimension itself, not through the generation view. This placement is an ordinary set (SIM-SET-002).
- **Invariants:** generation makes no neighbor updates. It runs shape functions only in the passes above.
- **Conformance tests:** generate a column with features that place blocks next to each other. Result: before the column gets to block-ticking status, it has no state change that a shape update makes. The exceptions are the places where template placement, tree placement or a post-processing mark applies.

Provenance: `source_suggested`. Confidence: high.

### SIM-NBR-006: Comparator output updates

- **Inputs:** position P and source block type B. B is the new block of SIM-SET-002 step 3.2, or a block that calls these updates directly.
- **Relevant state:** the four horizontal neighbors of P, and the blocks one position farther.
- **Preconditions:** none.
- **Behavior:** for each d in horizontal order `N, E, S, W`, do these steps:
  1. Let Q = P + d.
  2. If the section of Q is not *loaded*, skip d.
  3. If Q holds a comparator, request "single, with state" for Q with source B.
  4. If all of these conditions are true, request "single, with state" for Q + d with source B:
     - Q does not hold a comparator.
     - Q is a redstone conductor.
     - Q + d holds a comparator.

  Each of these requests carries no orientation (also under SIM-NBR-007) and moved-by-piston = false.
- **Ordering constraints:** vanilla order `N, E, S, W`. Observable result: each such comparator receives one update only.
- **Numeric semantics:** none.
- **Postconditions:** none.
- **Edge cases:**
  - Only Q is tested to find if it is loaded. Q + d is read with no test. As a result, if Q + d is in a section that is not *loaded*, the read loads it (SIM-SET-003).
  - This is the only place in the updater that tests if a position is loaded.
- **Invariants:** not more than four requests.
- **Vanilla grouping:** vanilla tests if the load level of the column of Q is 33 or less (SIM-TICK-005, *loaded*). The column can still be in its load. This test ignores y. If Q is outside the vertical extent of the dimension, the test passes and the read returns `minecraft:void_air`.

  An exact-parity mode tests the column. A section-native host tests the section of Q. The two tests give different results only when the sections of one column are different in *loaded*.
- **Conformance tests:** put a container at P and a comparator at P+N. Put a solid block at P+E and a comparator at P+E+E. Change the contents of the container. Result: both comparators get an update, P+N first.

Provenance: `source_suggested`. Confidence: high.

### SIM-NBR-007: Orientation under the redstone experiments feature

- **Inputs:** neighbor updates in a dimension that has `minecraft:redstone_experiments` in its enabled features.
- **Relevant state:** the level random (SIM-RNG-001).
- **Preconditions:** the feature is enabled (`data/minecraft/datapacks/redstone_experiments/pack.mcmeta` of the client jar). Each world has its own enabled features, so all dimensions agree.
- **Behavior:** if the feature is not enabled, no neighbor update carries an orientation. If the feature is enabled, these rules apply:
  - An orientation is a triple (up, front, side bias ∈ {left, right}), with up and front on different axes. There are 6 × 4 × 2 = 48 orientations. The index of an orientation is `((up_ordinal << 2) + front_key) << 1 | side_bias_ordinal`. The terms of this formula are:
    - The direction ordinals are `D=0, U=1, N=2, S=3, W=4, E=5`.
    - The side bias ordinals are left = 0, right = 1.
    - `front_key = (front_axis_key << 1) | front_axis_direction` (axis direction positive = 0, negative = 1).
    - `front_axis_key` is 1 if up is vertical and front is on X. It is also 1 if up is horizontal and front is vertical. In all other cases, it is 0.
  - A *fresh orientation* is the orientation with the index `nextInt(48)`, drawn from the level random, after its side bias is changed to left. This change clears the low bit of its index. Then a caller can change its up, and then its front.
  - A *change of front* to f keeps the same up and side bias, with two exceptions. If f is the old up, up changes to the opposite of the old front. If f is the opposite of the old up, up changes to the old front.
  - A neighbors-around request made without an orientation argument draws a fresh orientation when the request is made. These requests come from step 3.2 of a set, and from all vanilla blocks that request in that way. The draw occurs before the chain limit is checked. As a result, even a discarded request draws.
  - A request made with an orientation argument that is absent draws a fresh orientation when its first update runs. If the request skips a direction X, the front of that orientation changes to the opposite of X. No vanilla caller makes such a request.
  - Each update toward direction d carries the stored orientation of the request, with its front changed to d. This orientation is calculated from the stored orientation, not from the orientation of the previous update.
  - The blocks that make orientations themselves draw them from the same random. These blocks are buttons, levers, redstone torches, observers, diodes, pistons, lightning rods, lecterns, trapped chests, tripwire hooks, blocks in motion and redstone wire. Redstone wire draws in its evaluator, and when a player changes it between dot and cross.
- **Ordering constraints:** each draw takes one value from the level random, at the time given above.
- **Numeric semantics:** a uniform integer in [0, 48).
- **Postconditions:** none.
- **Edge cases:** the neighbor order stays `W, E, D, U, N, S`. The orientation only gives information to the block that receives the update. The experimental redstone wire uses this information. This document does not include the wire evaluator itself.
- **Invariants:** if the feature is off, neighbor updates do not take values from the level random.
- **Conformance tests:** put inert probes around P. Do a set at P with flags 1. Result with the feature off: the level random does not advance. Result with the feature on: the level random advances by one draw only. With the feature on, the six updates also carry orientations with the fronts W, E, D, U, N, S.

Provenance: `source_suggested`. Confidence: medium.

---

## 4. Shape updates

### SIM-SHP-001: Direct shape updates

- **Inputs:** a changed position P, its new state S, flags F' and limit L'. These come from SIM-SET-002 step 3.3, or from a block handler that makes the update directly (see Edge cases).
- **Relevant state:** the six neighbors of P.
- **Preconditions:** for an update that a set makes (SIM-SET-002 step 3.3), the set did not have *keep-shape*, and its limit L > 0. As a result, L' = L − 1 ≥ 0. An update that a block makes directly carries the flags and the limit that the block passes (Edge cases).
- **Behavior:** for each d in shape order `W, E, N, S, D, U`, there is one shape-update request (SIM-NBR-002) with the target Q = P + d. Its direction is the opposite of d (the direction from Q towards P). Its neighbor position is P, and its neighbor state is S. Its flags are F', and its limit is L'. When the request runs, these steps occur:
  1. The shape update reads the state C that Q has at that time.
  2. If F' has *wire-skips-shape* and C is redstone wire, nothing occurs.
  3. If not, the shape function of C calculates C' from (direction, P, S). The level random is available to the shape function.
  4. If C' is the same state as C, nothing occurs. If C' is air, the shape update destroys Q (SIM-SET-004) with limit L'. The destroy releases drops, unless F' has *no-drops*. If C' is a different state and not air, the shape update sets Q to C' with limit L'. The flags of this set are F' without *no-drops*.
- **Ordering constraints:**
  - Observable result: each neighbor receives one shape update that carries S.
  - Vanilla order: `W, E, N, S, D, U`, with cascades as in SIM-NBR-003.
- **Numeric semantics:** F' = F without bits 1 and 32 (F & −34). L' = L − 1.
- **Postconditions:** none.
- **Edge cases:**
  - The shape function receives the level as a reader and as a tick scheduler. It can read any position, and it can schedule ticks. For example, a falling block schedules its fall. A call that schedules a tick takes effect immediately (SIM-SCH-003). The chain does not queue the call as a request. The shape function is not a pure function of (C, direction, P, S).
  - F' does not have *notify-neighbors*. As a result, a set made by a shape update makes no neighbor updates and no comparator output updates (SIM-SET-002 step 3.2). Its removal side effect runs only if F' has *piston-move*.

    But its placement side effect runs unless the set has *skip-placement*. The placement side effect can make requests. For example, when a shape update changes the `locked` property of a repeater, the repeater updates the block in front of it.
  - When a shape update sets its target, the shape updates of this set carry the same F' ((F' & −33) & −34 = F'). As a result, all bits of the original F, except *notify-neighbors* and *no-drops*, get to all generations. This includes the client-only bits 4 and 8. When a shape update destroys its target, the destroy makes a set with flags 3. As a result, the shape updates below that destroy in the cascade carry only *sync-clients*.
  - No shape update that a set makes carries *no-drops*. SIM-SET-002 step 3.3 removes this flag, also when the set had it. As a result, each destroy that a shape update causes releases drops. Only a shape update that a block makes directly with bit 32 gets to the *no-drops* branch of step 4. No vanilla block makes such a shape update at this version.
  - Blocks also make shape updates directly, outside of a set. These shape updates have limit 512, not L − 1. A cascade that starts in this way can change 513 generations. These vanilla blocks do this:
    - A piston, with flags 2. When it starts to retract, it puts a block in motion at its own position with *keep-shape*. Then it makes direct shape updates around that position. It makes the same updates around each position that it leaves empty. It puts the other blocks in motion with ordinary sets with flags 324.
    - A piston also makes indirect shape updates directly (SIM-SHP-003), with flags 2. It makes them for each block that it destroys. It also makes them for the old block and for air at each position that it leaves empty.
    - A shulker box, with flags 3, four times in each open-close cycle. It makes them on the first tick of opening and when it is fully open. It also makes them on the first tick of closing and when it is fully closed. The client makes the same updates.

    With flags 3, only the sets that the first shape updates make have *notify-neighbors*. Only these sets make neighbor updates. Their own shape updates carry flags 2.
  - The set captures O and S when step 3.3 makes its requests. Outside of a chain, the neighbor updates of step 3.2 run to completion before that time. These neighbor updates can change P. But each direct shape update carries S, also when P changed (SIM-SHP-004).
  - On a client level, the client ignores a shape result of air (no destroy). The client sets all other results.
- **Invariants:** a shape update never runs for a set with *keep-shape*.
- **Conformance tests:**
  - Put probe blocks on all six sides of P. Do a set at P with flags 2. Result: the log shows six shape updates, at P+W, P+E, P+N, P+S, P+D, P+U. Their directions are E, W, S, N, U, D. Failing twin: the updates are in neighbor order.
  - Put a torch on a block. Set the block to air with flags 3. Result: the torch is destroyed and releases its item. Do the same test with flags 18 (*sync-clients*, *keep-shape*). Result: the torch stays in its position and floats.

Provenance: `source_suggested`. Confidence: high.

### SIM-SHP-002: The update limit

- **Inputs:** the limit L of a set. L is 512, unless the caller passes a different value.
- **Relevant state:** none.
- **Preconditions:** none.
- **Behavior:** a set with limit L makes shape updates only if L > 0. These shape updates carry L − 1. A set or a destroy that a shape update makes uses the limit that the shape update carried. As a result, the change at generation k is made with limit 512 − k. In this count, the original set is generation 0. The change at generation 512 makes no shape updates.
- **Ordering constraints:** none.
- **Numeric semantics:** the default value of L is 512. The limit counts only shape generations. Neighbor updates do not decrease it. A set that a neighbor handler or a tick handler makes starts again from its own default (normally 512).
- **Postconditions:** a shape cascade has not more than 512 generations after the original set.
- **Edge cases:** the chain limit (SIM-NBR-004) also applies to the same requests. The two limits are independent. The update limit goes with the cascade across section boundaries. It is not reset or counted for each section. A cascade that starts with shape updates that a block makes directly is longer by one (SIM-SHP-001).
- **Invariants:** none.
- **Conformance tests:** use a dimension type with a height of at least 601. No vanilla dimension type is that tall. Put 600 probe blocks in a vertical stack. The stack goes through at least 38 sections. Each probe block changes to probe-changed when the shape of the neighbor below it changes. Start the cascade with a set at the bottom.

  Result: 512 blocks above the bottom change, not more and not less. Failing twin: 513 blocks, all of the stack, or a stop at a section boundary.

Provenance: `source_suggested`. Confidence: high.

### SIM-SHP-003: Indirect shape updates

- **Inputs:** a set at P (SIM-SET-002 step 3.3), or a piston that makes them directly (SIM-SHP-001).
- **Relevant state:** the old state and the new state at P.
- **Preconditions:** as in SIM-SHP-001.
- **Behavior:** the indirect shape updates of the **old** state run before the direct shape updates, and those of the **new** state run after them. At this version, only `minecraft:redstone_wire` has indirect shape updates. All other blocks have none. The flags and the limit of these shape updates are as in SIM-SHP-001. For a wire at P, each d in horizontal order `N, E, S, W` is examined. For each d, if the wire connects towards d (side not `none`) and P + d is not wire, these steps occur:
  1. If P + d + D is wire, a shape update has the target P + d + D. Its direction is the opposite of d. Its neighbor position is P + D (the block under the wire). Its neighbor state is the state that this neighbor position has at that time.
  2. Then, if P + d + U is wire, a shape update has the target P + d + U. It is the same as in step 1, but its neighbor position is P + U.
- **Ordering constraints:** the order is old-indirect, then direct, then new-indirect.
- **Numeric semantics:** as in SIM-SHP-001.
- **Postconditions:** none.
- **Edge cases:** the update passes a neighbor state that is read when the request is made. This state is not the state of the wire itself.
- **Invariants:** none.
- **Conformance tests:** with a set, put a wire state on a block, diagonally next to a lower wire. The side of this state towards the lower wire must not be `none`. A wire in the default state has no connected side, and it makes no indirect shape updates. Result: the connection of the lower wire is updated before this set returns. Failing twin: the lower wire keeps its old connection.

Provenance: `source_suggested`. Confidence: medium.

### SIM-SHP-004: Shape update when the target changed before it ran

- **Inputs:** a shape update that was queued during a chain, with a target that was changed before the update ran.
- **Relevant state:** the state that the target has when the update runs.
- **Preconditions:** a chain is active.
- **Behavior:** the shape update runs the shape function of the state that the target has **when the update runs**. The neighbor state that it gives to the shape function is an instance parameter (P-CAPTURE):
  - the state that was captured at request time (SIM-NBR-002). Vanilla uses this choice.
  - the state that the neighbor has when the update runs.
- **Ordering constraints:** none.
- **Numeric semantics:** none.
- **Postconditions:** none.
- **Edge cases:** with the captured state, if the neighbor changed again, the calculation for the target uses an out-of-date neighbor state. Vanilla does not correct it. Only shape functions that use the passed state see the out-of-date value. The downward case of the wire uses it. But the horizontal case of the wire and a torch read the world, not the passed state.
- **Invariants:** none.
- **Conformance tests:** in a chain, set P to probe B. This set makes shape updates that carry B. Before they run, set P to probe C with *keep-shape*. Result with the vanilla choice: the log shows the shape update at P+W with neighbor state B. Result with the other choice: the neighbor state is C.

Provenance: `source_suggested`. Confidence: high.

---

## 5. Scheduled ticks

### SIM-SCH-001: Fields of a scheduled tick

- **Inputs:** a call that schedules a tick.
- **Relevant state:** none.
- **Preconditions:** none.
- **Behavior:** a scheduled tick has these fields:

| Field | Type | Description |
|-------|------|---------|
| type | block type (block ticks) or fluid type (fluid ticks). For fluid ticks, the type is `minecraft:water` or `minecraft:lava`. It is the same for a source and a flowing state (0.1). Vanilla blocks and fluids schedule only water and lava. A `minecraft:empty` tick can still occur: from a saved element, or from a spring feature of a data pack. It does nothing when it runs. | what must be at the position when the tick runs |
| position | block position | where the tick runs. The position selects the section of the tick. |
| due tick | signed 64-bit game time | the tick can run at this game time or later |
| priority | one of seven values (SIM-SCH-002) | the order of ticks that are due together |
| sub-tick order | signed 64-bit | the order of the call that scheduled the tick: the vanilla sequence (SIM-MODEL-005) |

  The full key is (due tick, priority, sub-tick order). Block ticks and fluid ticks are two separate kinds. The two kinds have the same rules, except the handler check (SIM-SCH-007) and area clear and copy (SIM-SCH-016, block ticks only).
- **Ordering constraints:** none.
- **Numeric semantics:** see SIM-SCH-003.
- **Postconditions:** none.
- **Edge cases:** the identity of a tick for de-duplication is (position, type) only (SIM-SCH-004). The position is not part of the full key. No order in this section depends on x, y, z, the section or the column, except where a Vanilla grouping paragraph says so.
- **Invariants:** none.
- **Conformance tests:** the conformance tests of SIM-SCH-006 cover this rule.

Provenance: `source_suggested`. Confidence: high. Changed in 26.4-snapshot-4: the type of a fluid tick is now the same for a source and for a flowing fluid.

### SIM-SCH-002: Priority

- **Inputs:** a priority value.
- **Relevant state:** none.
- **Preconditions:** none.
- **Behavior:**

| Name | Value |
|------|-------|
| extremely high | −3 |
| very high | −2 |
| high | −1 |
| normal | 0 (default) |
| low | 1 |
| very low | 2 |
| extremely low | 3 |

  A tick with a lower value runs earlier. An integer outside −3..3 is clamped when it is decoded. A value below −3 gives −3, and a value above 3 gives 3.
- **Ordering constraints:** see SIM-SCH-006.
- **Numeric semantics:** the priority is stored as a 32-bit integer.
- **Postconditions:** none.
- **Edge cases:** at this version, only redstone diodes (`minecraft:repeater`, `minecraft:comparator`) schedule ticks with a priority that is not normal. The values that are used are −3, −2, −1 and 0.
- **Invariants:** none.
- **Conformance tests:** decode `p: -7`: the result is −3. Decode `p: 9`: the result is 3. Decode `p: 2`: the result is 2.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-003: Schedule calls and the due tick

- **Inputs:** position P, type T, delay d (32-bit integer), an optional priority (default normal).
- **Relevant state:** game time G at the time of the call, and the sub-tick counter `n` of the dimension.
- **Preconditions:** none.
- **Behavior:** the call makes a tick with due tick `G + d`, the priority, and sub-tick order `n`. Then `n` increases by one. Then the tick is offered to the queue of the section of P (SIM-SCH-005). The queue can reject the tick as a duplicate (SIM-SCH-004).
- **Ordering constraints:** sub-tick order is the order of the calls that schedule ticks in the dimension. Block ticks, fluid ticks and all sections share this order.
- **Numeric semantics:** `n` is a 64-bit counter for each dimension. It starts at 0 when the dimension is created at server start. `n` is not saved. It advances for each call. This is also true when the tick of the call is then rejected or discarded.
- **Postconditions:** an accepted tick is due at `G + d`.
- **Edge cases:** if d ≤ 0, the tick is due immediately. But it does not run before the next scheduled-tick phase that collects it (SIM-SCH-009).
- **Invariants:** runtime ticks have a sub-tick order ≥ 0. No two runtime ticks of a dimension have the same sub-tick order. Ticks restored from a save have a negative sub-tick order (SIM-SCH-013).
- **Conformance tests:** at G = 100, schedule (P, A, 3). Then schedule (Q, A, 3). Result: both ticks are due at 103. P runs before Q (same priority, smaller sub-tick order). This result is the same if P and Q are in one section or in two sections.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-004: De-duplication

- **Inputs:** a tick offered to the queue of its section.
- **Relevant state:** the group of (position, type) pairs that are pending in that queue.
- **Preconditions:** none.
- **Behavior:** if a tick with the same position and the same type is pending, the queue discards the new tick with no log message. The due tick and the priority of the new tick have no effect on this. A tick is pending until the phase **collects** it (SIM-SCH-006), not until it runs.
- **Ordering constraints:** none.
- **Numeric semantics:** none.
- **Postconditions:** not more than one pending tick for each (position, type) in each kind. Vanilla breaks this postcondition only for duplicate saved elements (B.4, DEV-03).
- **Edge cases:**
  - A new tick with an earlier due tick or a higher priority does not replace the pending tick.
  - After the phase collects a tick and before that tick runs in the same phase, a call can schedule the same (position, type) again. The queue accepts the new tick. The new tick is queued for a later collection. The query of SIM-SCH-010 lets blocks avoid this.
  - Block ticks and fluid ticks are separate. A block tick and a fluid tick at the same position never collide. A fluid tick for a water source and a fluid tick for flowing water at the same position have the same type `minecraft:water`. As a result, they collide, and the queue discards the second tick. Water ticks and lava ticks at one position are different.
  - Ticks restored from a save count as pending for de-duplication, also before they are activated (SIM-SCH-013).
- **Invariants:** the group of pending pairs is equal to the pairs in the queue, plus the pairs of saved ticks that are not activated.
- **Vanilla grouping:** vanilla de-duplicates for each column. The position is part of the identity, and the position selects the section. As a result, de-duplication for each section is the same rule.
- **Conformance tests:**
  - Schedule (P, A, delay 10, normal). Then schedule (P, A, delay 2, extremely high). Result: one tick, due at G + 10. Failing twin: it runs at G + 2.
  - Start with A at P. The handler of A sets P to B. Schedule (P, A, 5) and (P, B, 5). Result: at G + 5, the handler of A is logged, then the handler of B. Failing twin: only the handler of A is logged, because the queue discards the tick of B as a duplicate.
  - Put flowing water at P with flags 530, so that no fluid tick is pending at P. Schedule a `minecraft:water` fluid tick with delay 5. Set P to a water source. Then schedule a `minecraft:water` fluid tick with delay 1.

    Result: one tick, due at G + 5. Failing twin: two ticks, because the source state and the flowing state have different types (as before 26.4-snapshot-4).

Provenance: `source_suggested`. Confidence: high. Changed in 26.4-snapshot-4: a source and a flowing state of one fluid have the same type. As a result, their fluid ticks at one position de-duplicate against each other. Before this change, water and flowing water were separate types, and it was possible for both ticks to be pending.

### SIM-SCH-005: Section queues and eligibility

- **Inputs:** a tick offered at position P.
- **Relevant state:** the section properties of P (SIM-TICK-005).
- **Preconditions:** none.
- **Behavior:** each section that *accepts ticks* owns one block-tick queue and one fluid-tick queue, ordered by full key. If a tick is offered for a position in a section that does not *accept ticks*, the tick is discarded with an error log. The log message is "Trying to schedule tick in not loaded position", followed by the position. A section does not *accept ticks* when the load of its column to full status is not complete, or when its column was unloaded. The ticks of a queue can be collected only while its section *runs scheduled ticks*. While the section does not *run scheduled ticks*, the ticks stay in the queue and get overdue.
- **Ordering constraints:** see SIM-SCH-006.
- **Numeric semantics:** none.
- **Postconditions:** none.
- **Edge cases:** if a set at an unloaded position loads a section (SIM-SET-003), that section accepts ticks but does not run them. A tick is accepted at any y of a column that accepts ticks, also outside the vertical extent of the dimension. Such a tick is saved like all other ticks. When it runs, it reads `minecraft:void_air` and is consumed with no effect. A section-native store must also accept and save such ticks.
- **Invariants:** a tick belongs to the section that contains its position.
- **Vanilla grouping:** vanilla keeps one queue for each kind in each column, ordered by full key. SIM-SCH-006 shows how to get the column queue from the section queues.
- **Conformance tests:** use a section that accepts ticks but does not run them. At G, schedule a tick in this section with delay 1. Make the section start to run ticks at G + 50. Result: the tick runs in the scheduled phase of G + 50 (overdue), not at G + 51.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-006: Selection and run order within one phase

- **Inputs:** the current tick `t`, the budget `B = 65536`, and the queues of one kind (block or fluid) of all sections. The current tick `t` is the game time after the advance of this tick.
- **Relevant state:** the head (smallest tick by full key) of each section queue.
- **Preconditions:** the tick runs normally and the world is not a debug world.
- **Behavior:**
  - **Observable result (section-native).**
    - The ticks that run come only from ticks with due ≤ `t` in sections that *run scheduled ticks*.
    - Not more than `B` ticks run.
    - In one section, the ticks run in full-key order.
    - If no collected tick is overdue, the ticks run in (priority, sequence) order across all sections. Vanilla gives this order (Invariants). It keeps the priorities of repeaters and comparators across section boundaries.
    - If some collected tick is overdue, the order across sections is free. Vanilla gives an order that depends on the column layout in this case.
    - Each tick runs not more than one time in each phase.
    - When more than `B` ticks are due, the ticks that are left run in a later phase.
  - **Recommended semantic policy.** Run the collected ticks in global full-key order across all sections. This policy satisfies the observable result. It also keeps full-key order inside each column. When no collected tick is overdue, the order of this policy is the same as the vanilla order (see Invariants).
  - **Vanilla order (exact).** The phase first collects ticks over *column queues*, then runs them:
    1. *Column queue.* For each column `(sx, sz)`, the column queue is the merge of the queues of all its sections by full key. Vanilla keeps one queue for each column and orders it only by full key, not by position, height or section. As a result, this merge gives the same column queue as vanilla, tick for tick. The column queue does not have to be stored separately: its head is the smallest head of its sections. When the phase takes the head, it takes it from the section that holds it.
    2. *Candidates.* A column is a candidate if its sections *run scheduled ticks* and its column-queue head has due ≤ `t`. Vanilla gives all sections of a column the same answer. A column with no ticks, or with a head that is not due, is not a candidate.
    3. *Merge.* Keep the candidates in a candidate queue. The key of a candidate is only the (priority, sub-tick order) of the head that it has at that time. The due tick of the head is **not** part of the key. Repeat these steps while fewer than `B` ticks are collected and the candidate queue is not empty:
       1. Remove the candidate column with the smallest key from the candidate queue.
       2. Collect the head of this column.
       3. Let K be the key of the best candidate that is left in the candidate queue. Read K one time only, at this point. If the candidate queue is empty, K is none.
       4. Then continue to collect from the same column while all these conditions are true:
          - Fewer than `B` ticks are collected.
          - The new head of the column is due (≤ `t`).
          - The (priority, sub-tick order) of the new head is not greater than K.
       5. If the column then has a due head and fewer than `B` ticks are collected, it re-enters the candidate queue. Its key is the (priority, sub-tick order) of that head.
    4. *Run.* Run the collected ticks in collection order (SIM-SCH-007).
  - In one column, collection follows the column-queue order (due tick first). As a result, an overdue tick comes before a tick that is due now. This is also true when the tick that is due now has a higher priority. Between columns, the comparison ignores the due tick.
- **Ordering constraints:** as above. Collection is complete before any collected tick runs.
- **Numeric semantics:** `B = 65536` for each kind, in each dimension, in each level tick.
- **Postconditions:** collected ticks are no longer pending (SIM-SCH-004).
- **Edge cases:**
  - Two ticks can have the same (priority, sub-tick order) only in two cases: ticks restored from saves (SIM-SCH-013) and copies (SIM-SCH-016). Between candidate columns, this document does not give the choice that vanilla makes (OQ-01). In one column, restored ticks cannot tie with each other or with runtime ticks. But a copy can tie with any tick of the destination, a pending tick or a tick scheduled later. In that case, this document does not give the order that vanilla uses between them.
  - Ticks scheduled while the collected ticks run are not collected in this phase (SIM-SCH-009).
  - A candidate for each section, in place of a candidate for each column, is not vanilla. It lets the head of a later section overtake an earlier overdue head of the same column. The second failing twin of the conformance table shows this.
- **Invariants:**
  - A tick is collected not more than one time.
  - (`derived`) If each collected tick has due = `t`, the vanilla order is a global order across all sections. It sorts those ticks by (priority, sub-tick order), smallest first. This is because each column queue is then sorted by (priority, sub-tick order), and step 3 is an exact k-way merge on that key.
- **Conformance tests:** (vanilla policy, `t` = 11, all priority normal)

| Column | Section | Ticks (due, sub-tick order) |
|--------|---------|-----------------------------|
| A = (0, 0) | (0, 0, 0) | a2 (11, 1) |
| A = (0, 0) | (0, 5, 0) | a1 (10, 5) |
| B = (1, 0) | (1, 0, 0) | b1 (11, 3) |

  Result: the order is b1, a1, a2. First failing twin: a1, a2, b1 (global full-key sort). Second failing twin: a2, b1, a1 (global sort by priority and sub-tick order, or one candidate for each section). Under a semantic policy, a1 must come before a2 only if a1 and a2 are in one section. Here they are in two sections, so all orders of the three ticks are acceptable. The recommended policy gives a1, a2, b1.
  - Use one section with `t` = 11. The section has x (due 10, normal, sub 9) and y (due 11, extremely high, sub 10). Result: x runs before y.
  - Put 70 000 ticks that are due at `t` in one section. Result: 65 536 run in this phase, and the other 4 464 run in the next phase, in sub-tick order.

Provenance: `source_suggested` for the vanilla algorithm and for the column-queue claim, and `derived` for the invariant. Confidence: high. The column-queue claim is that the order of the column queue is the full key and nothing else.

### SIM-SCH-007: How one tick runs

- **Inputs:** a collected tick (P, T).
- **Relevant state:** the state at P when the tick runs.
- **Preconditions:** the tick was collected in this phase.
- **Behavior:** for a block tick, if the block state at P is of block type T, its scheduled-tick handler runs with the level random (SIM-RNG-001). If not, nothing occurs. For a fluid tick, if the fluid state of the block at P is of fluid type T, the tick handler of the fluid runs. If not, nothing occurs. This fluid check is the same for a source state and for a flowing state.
- **Ordering constraints:** collection order.
- **Numeric semantics:** none.
- **Postconditions:** the tick is consumed in both cases.
- **Edge cases:** if the block at P was replaced by a different block type, the tick is consumed with no effect. If a later set changes P back to the first block type, the tick does not come back. A state change in the same block type (for example a property change) does not prevent the tick. In the same way, a fluid tick is not lost when P changes between a flowing state and a source state of the same fluid. The tick runs the handler of the fluid state that P has at that time.
- **Invariants:** none.
- **Conformance tests:**
  - Schedule (P, probe A, 2). At G + 1, replace P with probe B. Result: at G + 2, no tick handler of A or of B is logged.
  - Put flowing water at P with flags 530, so that no fluid tick is pending at P. Schedule a `minecraft:water` fluid tick at P with delay 2. At G + 1, set P to a water source with flags 530. Flags 530 cause no neighbor or shape updates and no placement side effect, so nothing schedules a second tick.

    Result: at G + 2, the fluid tick handler of the source runs. Failing twin: the tick is consumed and does not run. This was the behavior before 26.4-snapshot-4, when the tick was keyed to the flowing type.

Provenance: `source_suggested`. Confidence: high. Changed in 26.4-snapshot-4: the type check matches any state of the fluid. As a result, a source/flowing change at P no longer consumes a pending fluid tick with no effect.

### SIM-SCH-008: Budget and fairness

- **Inputs:** more than `B` due ticks of one kind.
- **Relevant state:** the same as SIM-SCH-006.
- **Preconditions:** none.
- **Behavior:** the ticks over the budget stay pending and due, and SIM-SCH-006 collects them in a later phase, with no carry-over priority. In its own section (in vanilla, its own column), such a tick comes before ticks that are due later. But between columns, it competes only by (priority, sub-tick order). As a result, a later-due head of another column with a smaller key runs first. Block budgets and fluid budgets are independent. The budget is for each dimension, not for each section.
- **Ordering constraints:** the same as SIM-SCH-006.
- **Numeric semantics:** `B = 65536`.
- **Postconditions:** none.
- **Edge cases:** a tick that the budget defers keeps its due tick. As a result, in its own section (and, in vanilla, its own column), it stays before ticks that are due later.
- **Invariants:** none.
- **Conformance tests:** see the last case of SIM-SCH-006.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-009: Ticks scheduled during a phase

- **Inputs:** a tick scheduled while collected ticks run, or in a later phase of the same level tick.
- **Relevant state:** game time `t`.
- **Preconditions:** none.
- **Behavior:**
  - A block tick scheduled during the block-tick phase with delay ≤ 0 is due at ≤ `t`. But it runs in the block phase of the next level tick.
  - If a **fluid** tick is scheduled during the block-tick phase with delay 0, the fluid phase of the **same** level tick collects it. The reason is that fluid collection occurs after all block ticks ran.
  - If a tick with delay ≤ 0 is scheduled during one of these, it runs in the next level tick:
    - the fluid phase
    - random ticks
    - block events
    - entities
    - block entities
  - A tick scheduled with delay 1 at any point of the level tick of `t` runs in the level tick of `t + 1`. Vanilla breaks this rule in the overworld before the game time advance (B.4, DEV-02).
- **Ordering constraints:** collection occurs one time in each phase, before the ticks run.
- **Numeric semantics:** none.
- **Postconditions:** none.
- **Edge cases:** a block tick handler can schedule its own (P, T) again with any delay. The queue accepts this tick, because the tick that runs is no longer pending.
- **Invariants:** no tick runs two times in one phase.
- **Conformance tests:**
  - Run a probe block tick at `t`. Its handler schedules a block tick at P with delay 0. It also schedules a fluid tick for a probe fluid at Q with delay 0. Result: the fluid tick runs at `t`, and the block tick runs at `t + 1`. Failing twin: the block tick also runs at `t`.
  - Run a probe block tick at `t`. Its handler schedules its own tick again with delay 0. Result: the handler runs one time in each level tick, not again and again in one level tick.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-010: Queries

- **Inputs:** (P, T).
- **Relevant state:** the queue of the section of P, and the collected list of the phase that runs at that time.
- **Preconditions:** none.
- **Behavior:**
  - *Has scheduled tick*: true if, and only if, a (P, T) tick is pending in the queue of the section of P. Saved ticks that are not activated also count. The result is false if the section of P does not *accept ticks*. A collected tick is not pending.
  - *Will tick this tick*: this query is about the phase of that kind that runs at that time. It is true if, and only if, that phase collected a (P, T) tick, and the tick did not run before the query. The result is false outside the phase of that kind.
- **Ordering constraints:** none.
- **Numeric semantics:** none.
- **Postconditions:** "has scheduled tick" has no side effect. In vanilla, the first "will tick this tick" query of a phase fixes the group that the later queries of that phase read.
- **Edge cases:**
  - During the fluid phase, "will tick this tick" for a block tick is false.
  - Repeaters, comparators and redstone torches use "will tick this tick". They use it to not schedule a second tick for themselves when one tick is about to run. A test block in start mode also makes this query, and ignores the result.
  - Vanilla builds the group behind "will tick this tick" at the first query of the phase, not before. An area clear (SIM-SCH-016) does not update this group. For this reason, after a clear during the phase, a cleared pair continues to answer true if any query came before the clear. This project removes the pair (B.4, DEV-04).
- **Invariants:** for one (P, T) at any time, the two queries can both be true only if the pair was scheduled again after its collection. The two queries are "has scheduled tick" and "will tick this tick".
- **Conformance tests:** schedule two probe ticks, at P and at Q, both due at `t`. Make sure that the tick at P has the smaller sub-tick order. Result: during the handler of P, "will tick this tick (Q)" is true and "has scheduled tick (Q)" is false. During the handler of Q, "will tick this tick (P)" is false.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-011: Order of block and fluid phases

- **Inputs:** the due block ticks and fluid ticks in one level tick.
- **Relevant state:** none.
- **Preconditions:** none.
- **Behavior:** all collected block ticks run first. Then the fluid phase collects the fluid ticks, and all of them run. Block ticks and fluid ticks never interleave. The two kinds share the sub-tick counter, but the order of each kind is independent of the other kind.
- **Ordering constraints:** observable and vanilla: the block phase comes strictly before the fluid phase.
- **Numeric semantics:** each kind has a separate budget of 65 536.
- **Postconditions:** none.
- **Edge cases:** for fluid ticks that block ticks schedule, see SIM-SCH-009.
- **Invariants:** none.
- **Conformance tests:** see SIM-TICK-002.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-012: Save format

- **Inputs:** the ticks of the sections of one column that the server saves at game time `G`.
- **Relevant state:** the queues of those sections, and any saved ticks that are not activated.
- **Preconditions:** none.
- **Behavior:** the save record of a column (its compound in the region file) holds two lists, `block_ticks` and `fluid_ticks`. These two lists cover all sections of the column. Each element is a compound:

| Key | NBT type | Content |
|-----|----------|---------|
| `i` | string | the block id (in `block_ticks`) or the fluid id (in `fluid_ticks`: normally `minecraft:water` or `minecraft:lava`), for example `minecraft:repeater`, `minecraft:water` |
| `x` | int | position x |
| `y` | int | position y |
| `z` | int | position z |
| `t` | int | delay: due tick − `G` |
| `p` | int | priority value (SIM-SCH-002) |

  Element order: first come the saved ticks of the column that are not activated. They are in the order in which the server loaded them, and their stored delays do not change. Then come the active ticks of all sections of the column, merged by sub-tick order, smallest first. The record does not store the sub-tick order.

  For a column in generation (not full), this applies (SIM-SCH-014):

  - The server stores a tick that is scheduled during generation with `t = 0`.
  - A tick that the server loaded into the column from an earlier save keeps its stored `t`.
  - The list keeps the insertion order.
- **Ordering constraints:** the list order encodes the relative insertion order (SIM-SCH-013).
- **Numeric semantics:** `t` = due − `G`, truncated to 32 bits. It is negative for an overdue tick.
- **Postconditions:** when the server loads the column and activates it at game time `G'`, it restores the ticks with the due ticks `G' + t`.
- **Edge cases:** a column that is saved while its sections do not run ticks carries their overdue ticks with a negative `t`. The server writes both lists, also when they are empty (OQ-07, answered).
- **Invariants:** the position of each element is in the column.
- **Vanilla grouping:** there is one save record for each column. This is the layout of the save format, not of the simulation. A section-native store makes this record from two parts, one after the other. The first part is the list of saved ticks of the column that are not activated. The second part is the active ticks of the sections of the column, merged by sub-tick order. If two ticks of the column share a sub-tick order (copies, SIM-SCH-016), the relative order of these ticks in vanilla is unspecified.
- **Conformance tests:** freeze the ticks at game time G. Use an unpowered repeater with a repeater delay of 1, in a plain line. As a player, place a redstone block on the input side of the repeater, directly behind it. Make sure that the redstone block is in the column of the repeater. Do `/save-all flush`.

  Result: the `block_ticks` of the column contains `{i: "minecraft:repeater", x, y, z, t: 2, p: -1}`. Failing twins: `t: G+2` (absolute), or `p: 0`.

  Do not put a repeater or a comparator at the output side of the repeater. A diode there that does not face back into the repeater changes the priority to −3.

  A redstone block on a lateral side schedules nothing. A repeater reads its input from behind, and only diodes lock it from the side. The placement in the column of the repeater is important. `/save-all flush` writes only the columns that are marked unsaved. When the server schedules a tick, it does not mark the column (SIM-SCH-015).

Provenance: `source_suggested`. Confidence: high. Changed in 26.4-snapshot-4: the only `fluid_ticks` ids are `minecraft:water` and `minecraft:lava`. The server writes the tick of a flowing fluid under the id of its fluid. There is no other change to the layout.

### SIM-SCH-013: Load and activation

- **Inputs:** a save record of a column, with `block_ticks` / `fluid_ticks`.
- **Relevant state:** the game time when the sections of the column start to *broadcast changes* (vanilla: the column gets to block-ticking status).
- **Preconditions:** none.
- **Behavior:**
  1. *Decode.* The server decodes each element independently. It discards an element if one or more of these conditions are true:
     - The element is not a compound.
     - The element does not have all of `i`, `x`, `y`, `z`, `t` and `p`.
     - The element has an `i` that is not a string, or an `i` that names no registered entry.
     - The element has a non-numeric `x`, `y`, `z`, `t` or `p`.

     The server keeps the other elements. A numeric field can have an NBT numeric type that is not int. If so, the server narrows it to int before it clamps the value. For example, `p: 4294967295L` narrows to −1 and reads as high. A `p` below −3 reads as −3, and a `p` above 3 reads as 3. An `i` without a namespace means `minecraft:`.

     If a list has one or more discarded elements, the server logs one error for the list: `Failed to read field (block_ticks=…): …`. This error joins the element messages.

     If the x or the z of an element is outside this column, the server discards the element with no log message. The server does not check the y. An element with a y outside the vertical extent activates, and its tick runs with no effect (SIM-SCH-005).

     If there is no list, the server uses an empty list. An array tag reads as a list of numbers, and the server discards each number with the error above. An empty array gives an empty list with no log. If the tag is another type, the server logs this and uses an empty list.
  2. *Pending.* When the column gets to full status, the decoded list changes to the pending saved ticks of the column. Each saved tick is in the section that holds its position. These ticks count for de-duplication (SIM-SCH-004) and for "has scheduled tick", but they do not run.
  3. *Activation.* This step occurs when the column completes its transition to block-ticking, at game time `G'`, after post-processing (SIM-SCH-017). The list of one kind of the column has `m` saved ticks. The k-th saved tick (k = 0..m−1, list order) changes to a tick with due `G' + t` and its priority. Its sub-tick order is `k − m` (so −m … −1).

     The server numbers `block_ticks` and `fluid_ticks` separately, each with its own m. Vanilla adds these ticks without de-duplication. This project keeps only the first element of each (position, type) (B.4, DEV-03). Then the server discards the saved list.

     Vanilla runs this step in the task that completes the block-ticking status of the column. This task waits until the column and its eight neighbors are full. It runs on the main thread between level ticks (or in a synchronous load). *Broadcasts changes*, *runs scheduled ticks* and *random-ticked* turn on directly after this task (SIM-TICK-005).
- **Ordering constraints:** restored ticks come before all runtime ticks in the same column that have the same due tick and priority. Restored ticks keep their saved relative order.
- **Numeric semantics:** the sub-tick orders are −m … −1. They are numbered over the list of one kind of one column, not for each section.
- **Postconditions:**
  - The time when the column is unloaded does not count toward the delay of a tick.
  - The time when the column is loaded but does not run ticks counts (SIM-SCH-005).
  - In vanilla, the time between the last save of the column and its removal from memory also does not count. This applies if the server did not write the column at removal (SIM-SCH-015, B.4 DEV-01).
- **Edge cases:**
  - In vanilla, two elements with the same (position, type) in the list of a full column both activate, and both run. After the first one runs, the pair does not count as scheduled, but the second copy continues to be in the queue. This project activates only the first element (B.4, DEV-03). In the list of a column in generation, the server discarded the later duplicate at load (OQ-08, answered).
  - A tick can be scheduled at runtime between steps 2 and 3, for a pair that is not in the saved list. The queue accepts this tick and keeps it with its runtime sub-tick order.
  - Restored ticks of two different columns can have equal sub-tick orders (OQ-01).
  - A column that was saved while in generation carries `t = 0`. Its ticks are due at the first scheduled phase after activation.
  - In a record of the world version that this game version uses, a `fluid_ticks` element that names `minecraft:flowing_water` or `minecraft:flowing_lava` has an unknown id. Step 1 discards it and logs an error. Vanilla never finds such an element there. It rewrites those names to `minecraft:water` and `minecraft:lava` when it upgrades a record that was written before world version 5123. This also applies to the `UpgradeData` neighbor tick lists.

    This project upgrades nothing, and it rejects a record of another world version before it decodes the record. It also rejects a record that carries a non-empty `UpgradeData` compound (B.4, DEV-05).
- **Invariants:** the activated ticks are all the decoded elements in the column, and only these.
- **Vanilla grouping:** vanilla does load and activation for each column. It numbers the restored sub-tick orders over all of the list of the column for each kind. A section-native store keeps the index of each saved tick in the column list, or it keeps the list complete until activation. It assigns `k − m` from the index, and then puts each tick in the queue of its section. If the store numbers the ticks for each section separately, the order of restored ticks across the sections of one column changes. This is not vanilla.
- **Conformance tests:**
  - Save a column with `t: 3` for a repeater. Later, load the column. It activates at `G'`. Result: the repeater ticks in the level tick of `G' + 3`. Failing twin: the repeater ticks immediately, or at the stored absolute time.
  - Load a column with `block_ticks: [{i: "minecraft:no_such_block", …}, {i: "minecraft:repeater", …, t: 1, p: 0}]`. Result: the repeater tick runs. The server logs one error line for the list. Failing twin: the server discards both elements (OQ-02, answered).
  - Use a column list `[u (section sy=4, t: 0), v (section sy=0, t: 0)]`, with both ticks at priority normal. The column activates at `G'`. Result: in the first scheduled phase after activation, u runs before v (u gets −2, v gets −1). In vanilla, this phase is normally the level tick of `G' + 1`, and both ticks are then overdue. Failing twin: v runs first (numbered for each section, or by height).

Provenance: `source_suggested`. Confidence: high. Changed in 26.4-snapshot-4: the flowing fluid ids are removed. As a result, they decode as unknown ids. Vanilla renames them when it upgrades saves from before world version 5123. There is no other change to the decode step and the activation step.

### SIM-SCH-014: Ticks during generation

- **Inputs:** ticks that are scheduled while the server generates a column (feature placement and other generation steps before full status).
- **Relevant state:** the tick lists of the column in generation.
- **Preconditions:** the column is not full.
- **Behavior:** the server stores the tick as (type, position, delay 0, priority). It de-duplicates the ticks by (position, type). It discards the due tick and the sub-tick order. During generation, "will tick this tick" is always false. When the column gets to full status, the list changes to pending saved ticks (SIM-SCH-013).
- **Ordering constraints:** the list order keeps the insertion order.
- **Numeric semantics:** delay 0.
- **Postconditions:** a block that is scheduled during generation ticks at the first scheduled phase after its section starts to run ticks.
- **Edge cases:** the requested delay is lost.
- **Invariants:** none.
- **Vanilla grouping:** vanilla runs generation for each column. As a result, there is one list, with its insertion order, for each column. SIM-SCH-013 uses that order to number the ticks at activation.
- **Conformance tests:** let the world generate water with a pending fluid tick. Save the column before it gets to full status. Result: the saved fluid tick carries `t: 0`.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-015: Unload and sections that do not run ticks

- **Inputs:** a section that is not *loaded* any more, or a section that stays *loaded* but does not *run scheduled ticks*.
- **Relevant state:** the queues of the section, and if its column is marked unsaved or not.
- **Preconditions:** none.
- **Behavior:** the queues of a column are present from its first full status until the server removes the column from memory. In vanilla, the column is removed when its load level is above the loaded maximum, `33 + the full-status dependency radius`. The removal occurs when its unload task runs in step 9 of SIM-TICK-002.

  The removal can occur many ticks later. The server runs unload tasks only while the tick has time left, or when more than 2000 are queued. A column also waits until the status work on it completes.

  The queues of the column stay in place in these two periods:
  - while the column is loaded but does not run ticks
  - after the column is not *loaded* any more, but before the server removes it.

  In these periods, the queues accept ticks and count for de-duplication. Game time continues to advance. As a result, the ticks in the queues get overdue. They run, up to the budget, at the first phase in which the section runs ticks again.

  At removal, vanilla writes the column (SIM-SCH-012, relative to the game time at that save) only if the column is marked unsaved. These events mark the column unsaved:
  - its load from disk, or its generation
  - a change of a block state, of structure data, of biomes by command, or of the light data of a section
  - a change of a block entity while the column is loaded.

  While the column stays in memory, vanilla also writes unsaved columns in step 9. It writes up to 20 columns in each tick, and each column not more than one time in 10 seconds.

  When the server schedules, runs or removes ticks, vanilla does not mark the column. If the column is not marked unsaved, the record of the last save stays unchanged. For this record:
  - its `t` is relative to the game time of that save
  - ticks that were scheduled after that save are lost
  - ticks that ran after that save and did not change a block run again after the next load.

  This loss is an intentional deviation (B.4, DEV-01). This project writes the pending ticks of each column that it removes.
- **Ordering constraints:** the unload occurs in step 9 of SIM-TICK-002, after the scheduled ticks of this tick.
- **Numeric semantics:** none.
- **Postconditions:** none.
- **Edge cases:** overdue ticks that run late run in queue order (due first). The server does not compress or discard them.
- **Invariants:** none.
- **Vanilla grouping:** vanilla unloads and saves each column as one unit.
- **Conformance tests:** see SIM-SCH-005 and SIM-SCH-013.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-016: Area clear and copy (short form)

There are two operations. They apply only to block ticks. The server never clears
or copies fluid ticks. Neither operation is part of the ordinary tick, but a command
block can run them in the block-tick phase.

- *Clear* (the test framework: a test instance block that places its structure, the
  test runner, `/test`) removes all block ticks in a box from:
  - the active queues of the sections that the box touches
  - the collected list of the phase that runs at that time
  - the run list of that phase (the ticks that ran in the phase before the clear).

  Clear also removes the pending block events of the box. Saved ticks that are not
  activated stay pending.
- *Copy* (`/clone`, with a source that can be in another dimension) gathers all block
  ticks in the source box. The mask, the filter and the mode of the clone have no effect on this.
  A move leaves the source ticks in place. Copy gathers the ticks in this order:
  1. from the run list of the source
  2. from the collected list of the source
  3. from the active queues of the source (not from saved ticks that are not activated).

  Copy offers each tick, in that order, to the destination at position + offset
  (SIM-SCH-005). The first offered tick wins (SIM-SCH-004). A copy keeps its due tick
  and priority. Its sub-tick order changes to `s − min + max + 1`. Here, `min` and
  `max` are the smallest and largest sub-tick orders among the gathered ticks,
  numbered in the source dimension.

  The counter of the destination does not advance. As a result, a copy can share its
  sub-tick order with any destination tick in `max + 1 … 2·max − min + 1`. That
  destination tick can be pending, or scheduled later.

  The copy runs after the clone sets all its blocks. Ticks that these sets schedule
  at the destination are pending first, and they win over the copies. Ticks that a
  move schedules in the source box are copied. The copy also runs when the clone
  changes no block. The command then fails, but the copied ticks stay.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-017: Post-processing when broadcast starts

- **Inputs:** a section that carries post-processing marks that generation left, in a column that completes its transition to block-ticking (as SIM-SCH-013 step 3).
- **Relevant state:** the post-processing marks of the section. They are a list of positions in the section. The server saves the marks with the column (`PostProcessing`: one list for each section, from the lowest section up, of shorts `x | y << 4 | z << 8`). It restores them on load. The marks are not de-duplicated.

  A mark can be at a position that was not set. A set through the generation view marks the position that the new block names (SIM-NBR-005). Also, generation steps mark positions directly.
- **Preconditions:** none.
- **Behavior:** the server does these steps for each marked position, in list order:
  1. If the block state has a non-empty fluid, the tick handler of the fluid runs immediately (not scheduled).
  2. Then the server uses the block state that it read before step 1. If the block is a fluid block, the scheduled-tick handler of the block runs immediately with the level random. If not, the server calculates the state again from the shapes of its six neighbors, in shape order. If the state changed, the server sets it with flags 276 (*no-rerender*, *keep-shape*, *skip-block-entity-effects*).

     This set makes no neighbor updates, no shape updates and no client sync. But the placement side effect of the new state runs, because 276 does not have *skip-placement*.

  The fluid tick of step 1 makes ordinary sets with flags 3.

  Then the server clears the marks. Ticks scheduled here take the runtime path (SIM-SCH-003). They are de-duplicated against the pending saved ticks.
- **Ordering constraints:** post-processing is complete before the saved ticks activate (SIM-SCH-013 step 3). Between the two, vanilla promotes the pending block entities of the column. Then vanilla applies the upgrade data of the record. This project refuses a record with upgrade data (B.4, DEV-05).
- **Numeric semantics:** flags 276.
- **Postconditions:** the section has no post-processing marks.
- **Edge cases:** a runtime tick that is created here has a non-negative sub-tick order. As a result, it runs after any restored tick in its column that has the same due tick and priority.
- **Invariants:** none.
- **Vanilla grouping:** vanilla stores the marks for each section in the column. When the column gets to block-ticking status, vanilla processes the sections of the column from the bottom up.
- **Conformance tests:** use a generated column with a marked water position next to a cave entrance. Result: in the task that activates the column, the water flows one step before any saved tick activates. The fluid is ticked immediately, not scheduled. Failing twin: the water flows one delay later (OQ-09, answered).

Provenance: `source_suggested`. Confidence: medium.

---

## 6. Block events

### SIM-BEV-001: Fields and the queue

- **Inputs:** a block event request with a position P, a block type B, and two 32-bit integers `a` and `b`.
- **Relevant state:** the pending block events of the dimension. They are in the pending queue, in insertion order.
- **Preconditions:** none.
- **Behavior:** if no equal event is pending, the pending queue appends the event (P, B, a, b) to its end. Two events are equal when all four fields are equal. If an equal event is pending, the queue discards the new event, and the pending event keeps its place.
- **Ordering constraints:** first in, first out.
- **Numeric semantics:** on the server, `a` and `b` are 32-bit integers. Only their low 8 bits get to the client (SIM-BEV-004).
- **Postconditions:** none.
- **Edge cases:** two events that are different only in `a` or `b` are two different events, and both are delivered. There is one queue for each dimension. There is no queue for each section or for each column. The FIFO order applies across all sections.
- **Invariants:** no two equal events are pending at the same time.
- **Conformance tests:** queue four events in this order. The events are (P, A, 1, 0), (Q, A, 0, 0), (P, A, 1, 0), (P, A, 2, 0). Result: the delivered events are (P,1,0), (Q,0,0), (P,2,0). Failing twin: (P,1,0) is delivered two times.

Provenance: `source_suggested`. Confidence: high.

### SIM-BEV-002: The drain

- **Inputs:** step 10 of a level tick that runs normally (SIM-TICK-002).
- **Relevant state:** the pending queue.
- **Preconditions:** none.
- **Behavior:** the drain does these steps again and again, until the pending queue is empty:
  1. Remove the first event from the pending queue.
  2. If the position of the event *delivers block events* (SIM-BEV-003), deliver the event (SIM-BEV-004).
  3. If not, put the event on a deferred list.

  Events that are queued during the drain are appended to the pending queue. The same phase drains them. After the loop, the pending queue for the next tick holds the deferred events. They are in the order of deferral, and duplicates are removed.
- **Ordering constraints:** observable and vanilla: strict FIFO over the pending queue. This also applies to events that are added during the drain.
- **Numeric semantics:** there is no limit on the number of events in each drain (OQ-05).
- **Postconditions:** after the phase, the pending queue holds only deferred events. Later in the tick, the events that entities and block entities queue come after them.
- **Edge cases:**
  - An event can be equal to an event that this drain removed from the queue before. Such an event can be queued again and delivered again in the same drain.
  - Events that are queued in steps 11–12, or in the earlier phases of the next tick, are delivered at the next drain. They are delivered after the deferred events.
  - Pistons: the event of a piston moves blocks. The neighbor updates of these blocks make other pistons queue events. These events run in the same drain.
- **Invariants:** at the end of a drain, no pending event has a position that *delivers block events*.
- **Conformance tests:** make the event handler of probe A at P queue (Q, A, 0, 0) one time. Result: the events of P and of Q are both delivered in the same level tick. The event of P is first. Failing twin: the event of Q is delivered in the next tick.

Provenance: `source_suggested`. Confidence: high.

### SIM-BEV-003: Eligibility and deferral

- **Inputs:** an event at P.
- **Relevant state:** the *delivers block events* property of the section of P (SIM-TICK-005).
- **Preconditions:** none.
- **Behavior:** an event is delivered if, and only if, the section of P *delivers block events*. Other events are deferred in the queue of the dimension, with no limit, until the section *delivers block events*.
- **Ordering constraints:** deferred events keep their relative order.
- **Numeric semantics:** vanilla threshold: simulation level ≤ 32.
- **Postconditions:** none.
- **Edge cases:** deferral does not check that the block continues to be there. This check occurs at delivery. If an event is deferred while its section is unloaded, the event stays in memory. When the section comes back into simulation range, the event is delivered if the block continues to match. The event is not saved.
- **Invariants:** block events are never written to the save.
- **Vanilla grouping:** vanilla tests the simulation level of the column of P. All sections of a column give the same result.
- **Conformance tests:** queue an event in a section outside the simulation distance. Result: the event is not delivered. Move a player near the section. Result: the event is delivered in the first drain after the section gets to simulation level ≤ 32.

Provenance: `source_suggested`. Confidence: high. A block event is a plain record with no codec, and only the queue of the dimension holds it. As a result, it cannot be saved.

### SIM-BEV-004: Delivery and the packet

- **Inputs:** an event (P, B, a, b) that can be delivered.
- **Relevant state:** the state at P, and the players of the dimension.
- **Preconditions:** none.
- **Behavior:** if the state at P is of block type B, the event handler of that state runs with (a, b). The handler returns "delivered" or "not delivered".

  If the handler returns "delivered", the server sends a `block_event` packet (P, a, b, B) to each player in range in the same dimension. A player is in range when the squared distance from the point (P.x, P.y, P.z) to the player's position is **less than** 64² = 4096. The point (P.x, P.y, P.z) is the minimum corner of the block, not its center. If the block at P is not of type B at that time, the server discards the event with no log message.
- **Ordering constraints:** the server sends the packet immediately, during the drain. As a result, the packet comes before the client sync of any block change that the event caused (SIM-SET-005).
- **Numeric semantics:** the packet layout has these fields:
  - the position (packed block position)
  - `a`, as one unsigned byte
  - `b`, as one unsigned byte
  - the block, as its registry id (VarInt).

  The range is 64.0, with a strict inequality. The server selects the recipients by distance, not by the sections or columns that the players track.
- **Postconditions:** none.
- **Edge cases:** if a handler returns "not delivered", nothing is sent. A property change inside the same block type does not discard the event.
- **Invariants:** none.
- **Conformance tests:**
  - Put a player at a distance of 64.0 from the corner of a note block. Play the note block. Result: the player receives no packet. At a distance of 63.99, the player receives the packet.
  - Open a chest. In the same tick, before the drain, break the chest. Result: there is no `block_event` for the chest.

Provenance: `source_suggested`, and the packet id is `black_box_confirmed` by `../../assets/mcrs/reports/packets.json`. Confidence: high.

---

## 7. Random ticks

### SIM-RND-001: Section selection

- **Inputs:** step 9 of a level tick that runs normally.
- **Relevant state:** the game rule `random_tick_speed` (integer ≥ 0, default 3), and the section properties (SIM-TICK-005).
- **Preconditions:** the world is not a debug world.
- **Behavior:** each *loaded* section that is *random-ticked* receives its picks (SIM-RND-002) one time in each level tick. Precipitation picks are separate. They run for each column (SIM-RND-004).
- **Ordering constraints:** observable: each random-ticked section is processed one time only in each level tick. The picks of a section do not interleave with the picks of another section. Vanilla: see Vanilla grouping.
- **Numeric semantics:** `random_tick_speed`: default 3, minimum 0.
- **Postconditions:** none.
- **Edge cases:** when `random_tick_speed = 0`, there are no precipitation picks and no random ticks.
- **Invariants:** none.
- **Vanilla grouping:** vanilla visits columns, not sections. Vanilla completes all the work of a *random-ticked* column before it starts the next column. For each such column, vanilla first runs its precipitation picks (SIM-RND-004). Then it visits each of its sections, from the lowest to the highest in the vertical extent of the dimension.

  The column order is the iteration order of an internal hash table of simulation levels. This order is always the same for the same inputs only when the inputs include the full history of the table (OQ-04). The position generator and the level random are unseeded (SIM-RNG-001). As a result, no exact-parity mode can give the same picks as vanilla. Parity is only by distribution, and it is not necessary to keep the vanilla grouping.
- **Conformance tests:** make a field of saplings. Use `random_tick_speed = 0`. Result: the saplings never grow. With the default value, the saplings grow.

Provenance: `source_suggested`, and the presence of the game rule is `black_box_confirmed` by `../../assets/mcrs/reports/registries.json` (`minecraft:game_rule`, entry `minecraft:random_tick_speed`). Confidence: high.

### SIM-RND-002: Picks within one section

- **Inputs:** a section at `(sx, sy, sz)`, and `k = random_tick_speed`.
- **Relevant state:** the level random, the position generator (SIM-RND-003), and the counts of randomly ticking block states and fluid states in the section.
- **Preconditions:** SIM-RND-001.
- **Behavior:** if these two conditions are true, the server does k picks in the section:
  - k > 0.
  - The section contains at least one randomly ticking block state or randomly ticking fluid state. This condition is checked one time, before the picks of the section.

  Each pick has these steps:
  1. Draw a position from the generator with base `(16·sx, 16·sy, 16·sz)` and y mask 15.
  2. Read the block state at that position from the section.
  3. If that block state is randomly ticking, run its random tick handler with the level random.
  4. Then get the fluid state **of the block state read before the handler ran**. If that fluid state is randomly ticking, run the random tick handler of the fluid with the level random.
- **Ordering constraints:**
  - Vanilla: the picks are in the order of the generator.
  - Observable: k picks for each eligible section in each tick. Each pick is uniform over the 4096 positions of the section. The observable result is about distributions only.
  - The picks of vanilla are not independent. They are consecutive outputs of the multiplier-3 generator (SIM-RND-003). As a result, on average 61.5 of the 4096 offsets can come after a given offset. For one offset, this number is 48 to 64. Two picks never give the same position when the number of draws between them is even and is not a multiple of 32.
  - If an implementation draws the picks independently, it is different from vanilla only in the joint distribution of the picks. The contract does not constrain this joint distribution (SIM-MODEL-106).
- **Numeric semantics:** each pick is uniform over the 4096 positions of the section (SIM-RND-003). The same position can be picked two times in one tick.
- **Postconditions:** none.
- **Edge cases:**
  - The eligibility of a section is evaluated before its first pick. It is not evaluated again during the picks of the section. "Randomly ticking" is a property of each state, and the block definition fixes it. The rule is the same for all sections, at all heights.
  - A fluid block with a fluid that ticks randomly (lava) is a randomly ticking block. Its random tick handler runs the random tick handler of its fluid. Then step 4 of the pick runs the handler of the fluid again. As a result, such a position receives two fluid random ticks in each pick (`derived` from steps 3 and 4, and `source_suggested`, confidence high).
- **Invariants:** a section with no randomly ticking content draws no values from the generator.
- **Conformance tests:** put one randomly ticking probe in a section. Use k = 3. Run 10 000 ticks. Result: the section receives a mean of 3 × 10 000 / 4096 ≈ 7.32 random ticks (binomial tolerance). Failing twin: ≈ 117 (3 × 10 000 / 256, when the y offset is not randomized).

Provenance: `source_suggested`. Confidence: high.

### SIM-RND-003: The position generator

- **Inputs:** a base (x0, y0, z0) and a y mask m.
- **Relevant state:** a signed 32-bit integer `r` for each dimension. When the server creates the dimension, it initializes `r` from a thread-local random. This random is not saved, and it does not come from the world seed.
- **Preconditions:** none.
- **Behavior:** each draw does these steps:
  1. `r := r × 3 + 1013904223`. The value of `r` wraps at 32 bits.
  2. `v := r >> 2` (arithmetic shift).
  3. The result is `(x0 + (v & 15), y0 + ((v >> 16) & m), z0 + ((v >> 8) & 15))`.
- **Ordering constraints:** the generator advances one time for each draw. The precipitation picks, the random-tick picks and the thunder target picks of the dimension share this generator.
- **Numeric semantics:** as in Behavior. All arithmetic is in 32-bit two's complement.
- **Postconditions:** none.
- **Edge cases:** the initial value is not saved, and it does not come from the world seed. As a result, the random tick positions of vanilla are not always the same from one run to the next. Only the distributions are the same.
- **Invariants:** none.
- **Conformance tests:** start from `r = 0`. Use the base (0, 0, 0) and the mask 15. Result:

| Draw | `r` after | `v` | offset (x, y, z) |
|------|-----------|-----|------------------|
| 1 | 1013904223 | 253476055 | (7, 11, 12) |
| 2 | −239350404 | −59837601 | (15, 14, 3) |

  Failing twin: if an implementation swaps the y and z fields, draw 1 gives (7, 12, 11). The kind of shift is not observable at mask 15. A logical shift changes only bits 30–31 of `v`. As a result, on draw 2 it gives `v` = 1013904223 and the same offsets (15, 14, 3).

Provenance: `source_suggested`, with the table recalculated from the formula. Confidence: high.

### SIM-RND-004: Precipitation picks (column-grouped)

- **Inputs:** a column `(sx, sz)` that SIM-RND-001 processes, and `k = random_tick_speed`.
- **Relevant state:** the level random, the position generator, the motion-blocking heightmap of the column, the weather, and the biome at the surface.
- **Preconditions:** the column is *random-ticked*.
- **Behavior:** the server does these steps k times:
  1. Draw a uniform integer in [0, 48) from the level random.
  2. If the integer is 0, draw a position from the generator with base `(16·sx, 0, 16·sz)` and y mask 15. Then run precipitation at the (x, z) of that position. The y of the position is discarded, and the motion-blocking surface of the column replaces it.

  Precipitation can change water below the surface to ice, with or without rain. While there is rain, precipitation can also do these two things:
  - Add snow, up to `min(max_snow_accumulation_height, 8)` layers. The game rule `max_snow_accumulation_height` has the default 1 and the range 0–8. The value 0 disables snow.
  - Let the block below the surface react to rain or snow.

  There is *rain* when these two conditions are true:
  - The dimension can have weather. That is, it has skylight, it has no ceiling, and its dimension key is not `minecraft:the_end`.
  - The rain level of the dimension is more than 0.2.

  The rain level is a 32-bit float. It increases by 0.01 in each tick, also in the tick of the change. From a rain level of 0, rain starts on the 20th tick after the weather changes to rain. The tick of the change counts as the first. When a world loads during rain, the rain level starts at 1.

  A dimension without weather also makes the gate draws, and it also changes water to ice. This document does not include what precipitation does to individual blocks.
- **Ordering constraints:** vanilla: the precipitation picks of a column run before the random-tick picks of its sections.
- **Numeric semantics:** the gate is 1 in 48 for each pick. There are k gates for each column in each level tick.
- **Postconditions:** none.
- **Edge cases:** the gate draws take values from the level random, also when no pick comes after them.
- **Invariants:** none.
- **Vanilla grouping:** this rule is by its nature a rule for each column, for two reasons. It reads the heightmap of the column, and its rate is for each `(x, z)`, not for each section. A section-native host runs it one time for each random-ticked column (the xz index over its sections), at that rate. To get the same observable result, a host without heightmaps must find the same surface y.
- **Conformance tests:** in a snowy biome, use `random_tick_speed = 0` during rain. Result: no snow is added. With the default value, snow is added.

Provenance: `source_suggested`. Confidence: high.

---

## 8. Random sources

### SIM-RNG-001: Which random each mechanism uses

- **Inputs:** none.
- **Relevant state:** for each dimension, the level random and the position generator (SIM-RND-003). The level random is a 48-bit linear congruential generator. When the server creates the dimension, it seeds the level random from a unique seed that is not saved.
- **Preconditions:** none.
- **Behavior:**

| Consumer | Source |
|----------|--------|
| scheduled block tick handlers | level random |
| scheduled fluid tick handlers | level random (through the level, when they draw) |
| shape functions (SIM-SHP-001) | level random |
| random tick handlers (block and fluid) | level random |
| precipitation gate, one draw in [0, 48) for each pick | level random |
| random tick and precipitation positions | position generator |
| orientation draws under the redstone experiments feature (SIM-NBR-007) | level random |
| neighbor-changed handlers and block event handlers | the source that the handler draws from: the level random |
| shuffle of the spawning area (before random ticks; n − 1 draws, made also when mob spawns are disabled) | level random |
| thunder gate, one draw in [0, 100 000) for each column of the spawning area in entity-ticking range. The draw occurs only while there is rain and the thunder level times the rain level is more than 0.9. After a hit: one position draw, then draws to select a target entity and to roll for a skeleton trap | level random, position generator |
| natural mob spawning, for each column of the spawning area after its thunder gate and before all random ticks; the special spawners; the weather cycle | level random |
| loot rolls for the drops of a block that any handler destroys | the random source of the loot table. There is one source for each table, server-wide. It is saved with the world, and seeded from the world seed and the id of the source (each vanilla block loot table names one). The spawn offset of each drop and the amount of experience use the level random |
| seed of a sound that a handler plays through the level | a second random for each dimension (thread-safe 48-bit LCG, unique seed that is not saved). The note block passes its own seed, drawn from the level random |

  The level random is a 48-bit linear congruential generator (multiplier 25214903917, increment 11). When the server creates the dimension, it seeds the level random from a process-wide seed uniquifier, mixed with the clock. Nothing seeds it again. The level random and the position generator do not come from the world seed, and they are not saved. As a result, the random sequence of vanilla is not always the same from one run to the next. Parity is necessary only for distributions.
- **Ordering constraints:** each consumer that draws from the level random of one dimension shares it. As a result, its output depends on the exact order of all such draws in the dimension.
- **Numeric semantics:** none, other than the generators.
- **Postconditions:** none.
- **Edge cases:** none.
- **Invariants:** at this version, no consumer of the machinery in this document uses a random for each position or for each section. Block behavior can use one. For example, the block entity of the potent sulfur geyser draws from a positional random before it sets blocks. This positional random comes from the world seed.
- **Conformance tests:** none against vanilla, because its random output is not always the same from one run to the next. A kernel that carries its own random state must give the same result from the same state and inputs.

Provenance: `source_suggested`. Confidence: high.

---

## 9. Numeric constants

| Constant | Value | Where | Provenance |
|----------|-------|-------|------------|
| Section edge | 16 blocks; section position = position >> 4 on each axis | 0.2 | `source_suggested` |
| Scheduled tick budget, for each kind, each dimension and each level tick | 65 536 | SIM-SCH-006 | `source_suggested` |
| Default update limit (shape generations) | 512 | SIM-SHP-002 | `source_suggested` |
| Default chain limit `max-chained-neighbor-updates` | 1 000 000 (negative = no limit) | SIM-NBR-004 | `source_suggested` |
| Update flag bits | 1, 2, 4, 8, 16, 32, 64, 128, 256, 512 | SIM-SET-001 | `source_suggested` |
| Flag combinations | 3, 11, 258, 260, 276, 324, 818, 820 | SIM-SET-001 | `source_suggested` |
| Flag mask for shape updates | F & −34 (clears 1 and 32) | SIM-SHP-001 | `source_suggested` |
| Flag mask for a shape result that is not air | F & −33 (clears 32) | SIM-SHP-001 | `source_suggested` |
| Flags of a post-processing set | 276 | SIM-SCH-017 | `source_suggested` |
| Priority values | −3 … 3 | SIM-SCH-002 | `source_suggested` |
| Start value of the sub-tick counter | 0, for each dimension, not saved | SIM-SCH-003 | `source_suggested` |
| Restored sub-tick orders | −m … −1, for each column save record and for each kind | SIM-SCH-013 | `source_suggested` |
| Broadcast range of block events | 64.0 blocks, squared distance < 4096 | SIM-BEV-004 | `source_suggested` |
| `block_event` / `block_update` / `level_chunk_with_light` / `section_blocks_update` packet ids | 7 / 8 / 46 / 86 | SIM-BEV-004, SIM-SET-005, 0.2 | `black_box_confirmed` at 26.4-snapshot-4 (`../../assets/mcrs/reports/packets.json`) |
| Load levels: full, block-ticking, entity-ticking | 33, 32, 31 | SIM-TICK-005 | `source_suggested` |
| Default simulation distance | 10 | SIM-TICK-005 | `source_suggested` |
| Temporary ticket for synchronous loads | `minecraft:unknown`, level 33, load only, timeout 1 (removed by the second purge after the last access that loads and misses the cache), for each column | SIM-SET-003 | `source_suggested` |
| Valid horizontal bound | `max(|sx|, |sz|)` ≤ 2 097 061 (2 097 151 − 90) | SIM-SET-003 | `source_suggested` |
| Largest load level kept in memory | 44 (33 + 11) | SIM-TICK-005, SIM-SCH-015 | `source_suggested` |
| Limit of a block coordinate in packed form | 33 554 431 (2²⁵ − 1); section or column coordinate: 2 097 151, before the safety margin for generation | SIM-SET-003 | `source_suggested` |
| `random_tick_speed` default / minimum | 3 / 0 | SIM-RND-001 | `source_suggested`; rule id `black_box_confirmed` (`registries.json`) |
| Precipitation gate | 1 in 48 for each pick | SIM-RND-004 | `source_suggested` |
| Thunder gate | 1 in 100 000 for each column of the spawning area in entity-ticking range, for each level tick, while there is rain and the thunder level times the rain level is more than 0.9 | SIM-RNG-001 | `source_suggested` |
| `max_snow_accumulation_height` default / range | 1 / 0–8 (not more than 8 layers; 0 disables snow) | SIM-RND-004 | `source_suggested` |
| Client chain limit | 1 000 000, fixed | SIM-NBR-003 | `source_suggested` |
| Position generator | `r = 3r + 1013904223`, `v = r >> 2`, x = `v & 15`, z = `(v >> 8) & 15`, y = `(v >> 16) & 15` | SIM-RND-003 | `source_suggested` |
| Orientations | 48 | SIM-NBR-007 | `source_suggested` |
| Empty-dimension threshold for the ticks of entities and block entities (vanilla value of P-IDLE) | counter < 300. If all level ticks run normally, the ticks of entities and block entities stop on the 299th level tick after the last keep-active ticket. Ticks that do not run normally do not count | SIM-TICK-002 | `source_suggested` |
| Pause when empty | `pause-when-empty-seconds` × 20, default 60 s. Only a dedicated server reads the property; other servers use 0 | SIM-TICK-001 | `source_suggested` |
| Time sync interval | 20 server ticks | SIM-TICK-001 | `source_suggested` |
| Fluid types | `empty`, `water`, `lava`; source/flowing: the `source` property of the fluid state; amount: its `level` (1 to 8) | 0.1, SIM-SCH-001 | `black_box_confirmed` at 26.4-snapshot-4 by `registries.json`; changed in 26.4-snapshot-4 (before: five types, with separate flowing types) |

---

## 10. Open questions

Each question lists the rules that it affects and an experiment that answers it.

- **OQ-01: Ties between restored ticks of different columns** (SIM-SCH-006, SIM-SCH-013).
  - **Question:** two columns that activate together each restore a tick with equal priority and equal sub-tick order (for example, −1, the last of each list). Vanilla selects between them by the internal layout of a priority heap that it fills in hash-table order. As a result, the choice is always the same for the same inputs only when the inputs include the history of the table. Which one runs first? The code does not answer this question, because the layout of the heap, not the positions of the columns, settles a tie. On a tie, the phase continues to collect from the column that it collects from at that time.
  - **Experiment:** use two columns, each with one saved `minecraft:observer` tick with `t: 0`. Connect them so that the one that pulses first locks the other (a repeater lock). Load the two columns in one tick. Do this in 20 restarts, with different load orders. If the winner is not always the same, the vanilla policy can break the tie by any stable key. For this key, propose the `(sx, sz)` of the column, smallest first.
- **OQ-02: One bad element in a tick list** (SIM-SCH-013).
  - **Question:** does the game discard only the bad element, as the reference implementation suggests, or the full list?
  - **Experiment:** edit a region file. In `block_ticks`, put an element with an unknown block id first, then a valid repeater tick with `t: 1`. Load the column. Monitor the repeater for a toggle. Look for "Failed to read field" in the log.
  - **Answered:** the game discards only the bad element (SIM-SCH-013 step 1).
- **OQ-03: Updates that get to unloaded sections** (SIM-SET-003).
  - **Question:** a neighbor update with a target in a section that is not loaded loads that section synchronously (in vanilla, its whole column). Confirm this. Also confirm that the column unloads after the 1-tick ticket expires, and that any ticks scheduled into it are saved.
  - **Experiment:** use a server with view distance 2 and one player. Make a line of observers that crosses the edge of the loaded area. Monitor the load events (the loaded count on the debug screen, or the timestamps of the region files). Monitor the pending tick of the observer in the saved column record.
  - **Answered:** yes. For what occurs after the ticket expires, and for an experiment that can show it, refer to SIM-SET-003.
- **OQ-04: Order of random tick visits** (SIM-RND-001).
  - **Question:** the column order of vanilla is a hash-table iteration order. The position generator and the level random are unseeded anyway. Is any tick-for-tick parity wanted here?
  - **Proposal:** no. The vanilla policy fixes a stable order (section position, smallest first) and declares this an intentional deviation. The column order of vanilla is the iteration order of a hash map of simulation levels. This order is not shuffled and not random, but only the capacity and the insertion history of the map fix it. No experiment can give the same picks as vanilla, because vanilla seeds the two random sources from the clock or from a thread-local random.
- **OQ-05: Block event drain with no limit** (SIM-BEV-002).
  - **Question:** the drain has no bound. With a handler that queues a new distinct event each time, the tick never ends. Is a bound an intentional deviation?
  - **Experiment:** none in vanilla. No vanilla block queues an event from its own event with no condition. Make the decision by policy. A bound, if added, must be larger than the number of events in one drain of any vanilla piston contraption. To measure this number, count the events in each drain in a large test world with flying machines.
  - **Premise confirmed (source):** the drain has no counter. Only the watchdog of the dedicated server (`max-tick-time`, default 60 000 ms) stops a drain that never ends. The watchdog writes a crash report and exits the process with status 1. If the exit does not finish in 10 seconds, the watchdog halts the process. A value of 0 or less turns off the watchdog. At this version, only these blocks and block entities queue events:
    - pistons
    - note blocks
    - potent sulfur
    - some block entities: chest (also the trapped chest and the copper chests), ender chest, shulker box, bell, decorated pot, spawner, end gateway.

    The policy question stays open.
- **OQ-06: Client sync in sections that are loaded but do not broadcast** (SIM-SET-005).
  - **Question:** a set with *sync-clients* in a section that is *loaded* but does not *broadcast changes* is not marked. Does a client that has that section at the time of the set ever see the change before it is resent?
  - **Experiment:** use a view distance that is larger than the simulation distance + 2. Use `/setblock` in a column at load level 33 that the client shows at that time. Do not log out and log in again. See if the client shows the new block.
  - **Answered:** no. Refer to SIM-SET-005.
- **OQ-07: Empty tick lists and the repeater save example** (SIM-SCH-012).
  - **Question:** are `block_ticks` and `fluid_ticks` written as empty lists when there are no ticks? Is the `p: -1` of the repeater example the value that the game writes?
  - **Experiment:** save a column with no pending ticks. Read the column with an NBT viewer. Then run the SIM-SCH-012 conformance test.
  - **Answered:** the game always writes both lists, and the corrected example is correct (SIM-SCH-012).
- **OQ-08: Duplicate elements in a saved list** (SIM-SCH-013).
  - **Question:** activation adds saved ticks without de-duplication. Do two elements that are the same both run?
  - **Experiment:** edit a column record so that `block_ticks` lists the same `minecraft:observer` position two times, with `t: 0`. An observer that ticks two times in one phase powers and unpowers in the same tick. Put a redstone lamp behind the observer. Monitor the lamp through a pulse detector that uses a comparator. The lamp shows the difference between one tick and two ticks.
  - **Answered:** refer to SIM-SCH-013, edge cases.
- **OQ-09: Post-processing order and its ticks** (SIM-SCH-017).
  - **Question:** confirm that post-processing of a newly generated column runs before its saved ticks activate. Also confirm that the fluids that it touches are ticked immediately, not scheduled.
  - **Experiment:** generate new terrain next to an ocean while a player flies in. Find the first tick on which the new column gets block-ticking status. On that tick, examine the water at a generated cave entrance. Record if the water flowed one step on that tick (ticked immediately) or flows one delay later (scheduled).
  - **Answered:** refer to SIM-SCH-017.
- **OQ-10: Exact mode and partitioned mode on one code path** (SIM-MODEL-008, section 11).
  - **Question:** the vanilla collection order (SIM-SCH-006) compares the heads of different columns by (priority, sequence) and ignores the due tick. In a partitioned instance, the sequence must be stable (P-SEQ). A sequence that is calculated from partition-local state is not the vanilla counter. Can one ordering policy give the same order as vanilla when there is one partition, and also be stable with several partitions?
  - **Proposal:** in a partitioned mode, the sequence is (partition time when the tick is scheduled, partition id, local counter). In exact mode, the sequence is the vanilla counter. The two modes are different only in that policy. Make this decision when the scheduled-tick kernel is designed.
- **OQ-11: Chain limit across partitions** (SIM-MODEL-103).
  - **Question:** when a chain continues in another partition as a message, what occurs to its request count? Does the count travel with the chain (one limit for each chain, as in vanilla)? Or does it restart (one limit for each piece)?
  - **Experiment:** none against vanilla (one partition). Make the decision by policy. A request count that travels with the chain keeps the vanilla bound of `M` requests for each chain.
- **OQ-12: Work that crosses a partition boundary** (A.0, SIM-MODEL-007, SIM-MODEL-102).
  - **Question:** which guarantee level does the instance of this project give when it has more than one partition?
  - **Answered:** the boundary level. A cross-partition effect is a message. The receiving partition applies its messages in a stable order, at a point that the design of the partitioned kernel fixes. That point must not depend on when threads run.

    For example, in a lock-step clock, a partition applies in the step of world time `W + 1` the messages that the steps of `W` sent. This example needs all messages of `W` before any step of `W + 1` starts: a barrier between the two, or an equal guarantee. Performance tests must measure the cost of this barrier together with the cost of the messages.
  - **Reason:** the semantic level across partitions needs a hand-over of sections during a step. A hand-over needs safe points to stop a step and a protocol to add a section to a halo. It also needs a rule against two steps that wait for each other. This project does not build it, because each such protocol adds defects.
  - **Consequence:** the formation rule keeps causally connected work in one partition, so crossings are rare. A cascade has no limit (SIM-MODEL-006), so formation cannot remove all crossings. A contraption that needs the exact order across a boundary runs correctly with one partition for each dimension (the exact level).

---

## 11. Design space

This section shows what a third architecture can do and must not do. These
statements follow from Part A. An example is an architecture with causally
connected shards of cubic sections. It merges and splits the shards
dynamically. Its cross-shard effects are messages that it applies in a stable
order.

### 11.1 Freedoms it can use

- **Partition shape** (P-SHAPE, P-FORM). The contract does not mention columns
  or squares. A partition can be any group of cubic sections, of any height. It
  can be formed from causal connectivity (SIM-MODEL-006), not from proximity.
  This is possible because of the footprint of each single step of the
  machinery. That footprint has a radius of at most 2 blocks, plus the bounded
  radius of the handler itself (A.2).
- **Parallel steps.** Two partitions can step at the same time if their work
  meets the conditions of SIM-MODEL-006. These conditions also apply to the
  work that the steps create. If the shared
  counters of A.2 are made partition-local and stable, the result is the same as
  the result of any sequential order (SIM-MODEL-006).
- **Order across sections.** These items are free (SIM-MODEL-102,
  SIM-MODEL-104, SIM-MODEL-106, SIM-MODEL-107):
  - the order of scheduled ticks across sections when some collected tick is
    overdue (SIM-SCH-006)
  - the order of section visits for random picks and for the client broadcast
  - the order between chain requests that do not depend on each other and are
    not in conflict.
- **Random streams** (P-RNG, P-RANDOM): only distributions are necessary. For
  this reason, a stream for each partition or for each section, seeded from saved
  state, is allowed. Such a stream makes the result always the same for the same
  inputs. This is not true for vanilla.
- **Clock** (P-CLOCK): the contract allows free-running partition time. This has
  the cost given in SIM-MODEL-004 (contraptions that span partitions of different
  speeds). Lock-step costs nothing in a design where all partitions advance one
  time for each server tick. Lock-step also keeps world time and partition time
  equal.

### 11.2 Hard constraints

- **Exact mode.** All rules of sections 1 to 9 hold, and parity fixtures
  recorded from vanilla pass, with these settings:
  - one partition for each dimension
  - the vanilla ordering policy for each queue
  - a lock-step clock
  - the vanilla counters.

  The exact mode runs on the same code path as the partitioned mode. Only the
  division into partitions and the ordering policy are different.
- **No lost work.** Merges, splits and messages keep all pending items and all
  keys. The only exception is a rebased due tick (SIM-MODEL-002, SIM-MODEL-004).
  The loss of priority on merge in the threaded instance is the failure to
  avoid.
- **No discarded cross-partition effects.** A neighbor, shape, tick or event item
  that is aimed outside the partition is never a skip with no log message, and
  never an error. The instance runs it synchronously (semantic level) or sends
  it as a message (boundary level, OQ-12). If the instance discards it,
  contraptions on a partition boundary behave differently from the same
  contraption inside one partition (B.3).
- **Stable order.** Each order is a function of item keys and stored state
  (SIM-MODEL-008). A shared atomic counter, a hash iteration or "first partition
  found" is not such a function.
- These constraints also apply:
  - **Delay and promptness** (SIM-MODEL-004).
  - **De-duplication** (SIM-SCH-004).
  - **Budget semantics** for each partition (SIM-SCH-006, SIM-SCH-008).
  - **Update limit** that travels with the cascade (SIM-SHP-002).
  - **Chain limit** (SIM-NBR-004, OQ-11).
  - **Phase order** (A.4).
  - **Intentional deviations** (B.4).
- **Save format** (SIM-MODEL-108): the record for each column, with relative
  delays and with restored sequences that are calculated separately for
  each record. This applies to any division into partitions.
- **Height with no limit.** A rule must not use a column of fixed height as a
  condition. The column groupings that apply only to vanilla stay in the vanilla
  instance (0.2).
