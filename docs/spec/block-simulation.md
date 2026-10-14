# Block simulation: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |
| World version (`DataVersion`) | `5124` |
| Data pack format | `124.0` |

Source of these values: the `version.json` of the 26.4-snapshot-4 game jar, which
`assets/minecraft/version.json` mirrors once the corpus is updated to this snapshot
(until then the repository file still reads 26.4-snapshot-2).

Changes from 26.4-snapshot-2: the machinery this document specifies (server and
level tick, tick-rate gate, scheduled-tick queues, neighbour and shape updates,
block writes, block events, random ticks, section properties) did not change in
26.4-snapshot-3 or 26.4-snapshot-4. The one change that reaches it is the fluid
registry: from 26.4-snapshot-4 a source and a flowing fluid are one fluid type
(0.1). The rules it touches carry a "Changed in" note in their Provenance line:
SIM-SCH-001, SIM-SCH-004, SIM-SCH-007, SIM-SCH-012, SIM-SCH-013, and the constants
of section 9. Terms (0.1) and cited artefacts (0.5) are updated to match.

This document states what a server, a client or a save file can observe of the
block simulation machinery of one dimension on the server: where block work runs
inside a tick, block set flags, neighbour updates, shape updates, scheduled block
and fluid ticks, block events and random ticks. It does not prescribe an
architecture. What individual blocks do when they receive an update, a tick or an
event is out of scope; only the machinery that delivers it is specified.

The document has two parts.

- **Part A, the abstract model** (section A): the primitives every implementation
  is described in, the contract any conforming implementation meets, the freedoms
  it has, and the parameters by which one implementation differs from another.
  Rules there carry `SIM-MODEL` identifiers.
- **Part B, instances** (sections B and 1 to 10): implementations described as a
  choice of every parameter of Part A. The **vanilla instance** keeps the exact
  rules (sections 1 to 9, identifiers `SIM-TICK`, `SIM-SET`, `SIM-NBR`, `SIM-SHP`,
  `SIM-SCH`, `SIM-BEV`, `SIM-RND`, `SIM-RNG`); it is the profile an exact-parity
  mode reproduces. The **region-threaded instance** (section B.3) is a server that
  partitions a dimension into independently ticked regions; its rule-by-rule
  comparison with vanilla is summarised in section B.3. Section 11 states
  the design space a third architecture may use.

Each vanilla rule separates two things:

- **Observable result**: what must hold after the work, whatever the internal
  order. A semantic ordering policy must satisfy this and nothing more.
- **Vanilla order**: the exact order the game uses. A vanilla ordering policy
  must reproduce it. Where the two coincide, the rule says so.

Every rule is stated per section or per position. Where the game's observable
behaviour depends on grouping sections into columns, the rule has a **Vanilla
grouping** paragraph that states how an exact-parity mode derives that behaviour
from sections (section 0.2).

## 0. Conventions

### 0.1 Terms

- **Position**: an integer block position `(x, y, z)`.
- **Block state**: one state of one block. Two states are *the same* when they are the identical state, not merely equal property values of different blocks.
- **Block type**: the block a state belongs to, for example `minecraft:repeater`.
- **Fluid type**: one entry of the `minecraft:fluid` registry: `minecraft:empty`, `minecraft:water`, `minecraft:lava`. Whether a fluid is a source or flowing, and its amount, are properties of the fluid *state* (`source`, a boolean; `level`, 1 to 8), not of the type: a water source and flowing water are both of type `minecraft:water`. A flowing state with `level` 8 is *falling*. Changed in 26.4-snapshot-4: the types `minecraft:flowing_water` and `minecraft:flowing_lava` no longer exist (the repository's `assets/mcrs/reports/registries.json` still lists them until it is regenerated).
- **Section**, **section position**, **column**: see 0.2.
- **Game time**: the dimension's monotonically increasing tick counter (a 64-bit integer), stored in the level data. All dimensions of one server read the same game time (SIM-TICK-003).
- **Level tick**: one execution of the per-dimension tick (SIM-TICK-002).
- **Server tick**: one execution of the server loop, which runs one level tick per dimension (SIM-TICK-001).
- **Runs normally**: the tick-rate gate (SIM-TICK-004) is open for this server tick.
- **Chain**: the set of neighbour and shape updates caused, directly or transitively, by one update request made while no chain is running (SIM-NBR-003).
- **Load level** and **simulation level**: two integers the ticket system maintains per column in vanilla. Lower is "more loaded". The thresholds that matter here: 33 = full, 32 = block-ticking, 31 = entity-ticking. They are inputs to the section properties of SIM-TICK-005.
- **Section properties**: *loaded*, *accepts ticks*, *runs scheduled ticks*, *delivers block events*, *random-ticked*, *broadcasts changes*, defined in SIM-TICK-005. A position has a property when its section has it.
- **Full key** of a scheduled tick: the triple (due tick, priority, sub-tick order), compared lexicographically, smaller first (SIM-SCH-001).
- **Probe block**: a block type used only by the conformance tests of this document. Each of its handlers (neighbour changed, shape update, scheduled tick, block event, random tick, placed, removed) appends `(handler, position, arguments)` to a test log and then does exactly what the test script says. Kernel conformance tests use probe blocks; black-box experiments against the game use vanilla blocks.

### 0.2 Sections and columns

The world is an unbounded grid of cubic sections. A column is a vanilla grouping
over that grid, never the unit of simulation.

- **Section**: a 16 × 16 × 16 cube of positions. It is the unit that holds block
  states, scheduled-tick queues, client-sync marks and random-tick eligibility.
- **Section position**: `(sx, sy, sz) = (x >> 4, y >> 4, z >> 4)`, with arithmetic
  shift (floor division by 16). There is no bound on `sy`: sections are numbered
  by absolute height, not from the lowest section of a dimension. The position's
  offset inside its section is `(x & 15, y & 15, z & 15)`.
- **Column**: the set of all sections with the same `(sx, sz)`; vanilla calls it a
  chunk. In vanilla it is:
  - the unit in which sections are transported to the client
    (`minecraft:level_chunk_with_light`, play clientbound id 46) and the set of
    players that receive changes to a section (the players that track its column);
  - the storage of heightmaps, which precipitation and other surface logic read;
  - the unit of the save record (one compound per column in the region file,
    holding `block_ticks` and `fluid_ticks`) and of load and activation;
  - the unit the ticket system assigns load and simulation levels to;
  - an xz index over sections.
- **Mapping**: position `(x, y, z)` → section `(x >> 4, y >> 4, z >> 4)` → column
  `(x >> 4, z >> 4)`.
- **Vertical extent**: no requirement here assumes a minimum or maximum build
  height. Where vanilla data is the observable input, the rule says so: the
  dimension type's `min_y` and `height` decide which positions vanilla refuses to
  set and reads as `minecraft:void_air` (SIM-SET-002, SIM-SET-003), and which
  sections vanilla holds in a column at all.
- **Vanilla grouping paragraphs**: where a vanilla order, a vanilla eligibility
  input or a save layout depends on columns, the rule states (a) the
  section-native rule, which a semantic policy satisfies, and (b) what an
  exact-parity mode reproduces on top by grouping sections by `(sx, sz)`. An
  exact-parity mode assumes every section of one column has the same section
  properties (SIM-TICK-005), as vanilla gives them.

Vocabulary used consistently below: *section* for the cube and its contents;
*column* only for the vanilla grouping above; *load level*, *simulation level*
and *ticket* for the loading machinery, which vanilla computes per column.

### 0.3 Direction names

`D` down (−y), `U` up (+y), `N` north (−z), `S` south (+z), `W` west (−x), `E` east (+x).

Three direction orders are used. They are different and must not be unified.

| Name | Order | Used by |
|------|-------|---------|
| Neighbour order | `W, E, D, U, N, S` | neighbour updates (SIM-NBR-001) |
| Shape order | `W, E, N, S, D, U` | shape updates (SIM-SHP-001) |
| Horizontal order | `N, E, S, W` | comparator output updates (SIM-NBR-006), redstone wire diagonal shape updates (SIM-SHP-003) |

### 0.4 Provenance and confidence

- `black_box_confirmed`: shown by a game-produced artefact in this repository, cited by path.
- `source_suggested_black_box_confirmed`: established by reading the reference implementation and consistent with a cited artefact.
- `source_suggested`: established by reading the reference implementation, with no artefact yet. A concrete experiment is attached or referenced.
- `derived`: follows from other rules of this document by the argument given in the rule.
- `unresolved`: not decided; collected in section 10.

A Provenance line states the status and the confidence only. None of the
experiments in this document has been run.

### 0.5 Artefacts cited

- `assets/minecraft/version.json`: the version identity above.
- `assets/mcrs/reports/registries.json`: block and fluid registry entries and ids; the `minecraft:game_rule` registry. Generated at 26.4-snapshot-2; its `minecraft:fluid` entries are superseded by 0.1, and block ids shift with the blocks added in 26.4-snapshot-3 and -4. The game rules cited here are unchanged.
- `assets/mcrs/reports/packets.json`: packet ids (`minecraft:block_event` = 7, `minecraft:block_update` = 8, `minecraft:level_chunk_with_light` = 46, `minecraft:section_blocks_update` = 86 in the play clientbound table). Generated at 26.4-snapshot-2; the play packet tables did not change up to 26.4-snapshot-4, so these ids still hold (`source_suggested` until the report is regenerated).
- `assets/minecraft/datapacks/redstone_experiments/pack.mcmeta`: the feature pack that enables `minecraft:redstone_experiments`.

### 0.6 Rule groups

| Prefix | Subject |
|--------|---------|
| `SIM-TICK` | placement of block work inside the server and level tick; section properties |
| `SIM-SET` | setting a block: flags, side effects, client sync |
| `SIM-NBR` | neighbour updates and the chain |
| `SIM-SHP` | shape updates |
| `SIM-SCH` | scheduled block and fluid ticks |
| `SIM-BEV` | block events |
| `SIM-RND` | random ticks and precipitation picks |
| `SIM-RNG` | random sources |
| `SIM-MODEL` | the abstract model of Part A |

### 0.7 Model terms

Defined in Part A; listed here for lookup.

- **Partition**: a set of sections advanced together by one step at a time (SIM-MODEL-002).
- **Owned**: a section is owned by the partition it belongs to; a position is owned when its section is.
- **Partition time** and **world time**: the two kinds of clock of SIM-MODEL-004.
- **Work item**: a scheduled tick, a neighbour request, a shape update, a block event, a random pick or a block write (SIM-MODEL-005).
- **Cross-partition effect**: a work item made in one partition whose target is owned by another or by none (SIM-MODEL-007).
- **Ordering policy**: the rule that orders the work items of one queue inside a partition, and the rule that merges queues when partitions merge (SIM-MODEL-008).
- **Instance**: an implementation described by its choice of every parameter of section A.5.

---

# Part A — Abstract model

## A. The contract

Part A is normative for every implementation. A rule here says what must hold
(**invariants**), what may vary and within which bounds (**freedoms**), and what
an implementation chooses (**parameters**, collected in A.5). The detailed rules
of sections 1 to 9 are the vanilla instance; A.6 says which of them hold for every
instance unchanged.

### A.1 Primitives

#### SIM-MODEL-001: Space

- The world of one dimension is the unbounded grid of cubic sections of 0.2. A
  section is the smallest unit of state ownership: block states, pending scheduled
  ticks, client-sync marks, post-processing marks and random-tick eligibility are
  held per section.
- Any grouping of sections (a vanilla column, a region of a region-threaded
  server, a shard) is an instance parameter, never part of the contract.
- Vertical extent is dimension data (0.2), not a property of the model.

Provenance: `derived` from 0.2. Confidence: high.

#### SIM-MODEL-002: Partitions

- **Definition.** A *partition* is a set of sections whose work is advanced by one
  step function, one step at a time. At any instant every section that holds
  simulated state belongs to at most one partition of its dimension; a section in
  no partition is not simulated.
- **Step.** A *partition step* runs the per-partition phases of A.4 over the
  partition's owned sections. Within one step the partition's work runs in the
  order its ordering policy gives (SIM-MODEL-008). A step reads and writes only
  owned sections, plus read-only snapshots of state outside every partition
  (world time, weather, game rules, registries).
- **Lifecycle.** Partitions may be created, merged and split. A merge or split
  happens only between steps of every partition it involves; never while one of
  them is inside a step.
- **Halo.** The *halo* of a partition is the set of positions a step may reach
  synchronously, as a read or a write, from a work item at an owned position. An
  instance states its halo (A.5, P-HALO); for vanilla it is the whole dimension.

Invariants:
- No two partitions write the same section in overlapping steps.
- Every pending work item belongs to exactly one partition, the one owning its
  target section, or to an explicit in-flight message (SIM-MODEL-007).
- A merge or split loses no pending work item and changes none of its keys
  except as SIM-MODEL-004 allows (rebasing the due tick by a clock offset).

Freedoms: the shape, size and formation rule of partitions; when to merge and
split; whether steps of different partitions run in parallel.

Provenance: `derived`. Confidence: high.

#### SIM-MODEL-003: Section properties are inputs

The six section properties of SIM-TICK-005 (*loaded*, *accepts ticks*, *runs
scheduled ticks*, *delivers block events*, *random-ticked*, *broadcasts changes*)
are inputs the host derives from its loading machinery. The model requires only:

- *runs scheduled ticks* implies *accepts ticks* implies *loaded*;
- *random-ticked* implies *loaded*;
- every section a partition steps over is owned by it.

How properties are derived (tickets, distances, column grouping) is an instance
parameter (P-PROPS). A section property may differ between two sections of one
vanilla column only in a non-vanilla instance.

Provenance: `derived` from SIM-TICK-005. Confidence: high.

#### SIM-MODEL-004: Clocks

- **World time** `W`: one counter per dimension (vanilla: shared by all
  dimensions, SIM-TICK-003), advanced once per server tick that runs normally.
  It is what players, commands and saved level data see as game time.
- **Partition time** `τ(p)`: a counter per partition, advanced by one at each
  step of `p` that runs normally. Every scheduled tick of `p` is keyed by `τ(p)`:
  its due tick is `τ(p) + d` at scheduling, and it is collected only when
  `τ(p) ≥ due`.
- An instance either ties every `τ(p)` to `W` (*lock-step*: each step of every
  partition advances together with `W`, so `τ(p) = W` always) or lets them drift
  (*free-running*).
- **Rebasing on merge.** When partition `a` is merged into `b`, every due tick of
  `a` is shifted by `τ(b) − τ(a)`, so its *remaining delay* `due − τ` is unchanged.
  On split, each part starts from the parent's `τ`.
- **Persistence.** A saved tick stores its remaining delay relative to the clock
  of its owner at save; on load it is due at the loader's clock plus that delay
  (SIM-SCH-012, SIM-SCH-013).

Invariants (every instance):
- **Delay.** A tick scheduled with delay `d ≥ 1` runs only after at least `d`
  steps that run normally of the partition owning its position have elapsed since
  it was scheduled, counting across merges and splits. Time spent unloaded does
  not count; time spent loaded but not running ticks does (SIM-SCH-005, SIM-SCH-013).
- **Promptness.** Once due, a tick runs at the first step at which its section
  *runs scheduled ticks*, unless the budget (SIM-MODEL-104) defers it.
- Two ticks of one partition scheduled `k` steps apart with delays `d1` and `d2`
  keep their relative due ticks: `due2 − due1 = k + d2 − d1`.

Freedoms: whether partition time equals world time. In a free-running instance
the relative timing of two contraptions in different partitions is not defined,
and a contraption spanning partitions of different speeds sees each part at its
own speed.

Provenance: `derived` from SIM-TICK-003 and SIM-SCH-003; the free-running case is
taken from the region-threaded instance. Confidence: high.

#### SIM-MODEL-005: Work items and their keys

| Item | Created by | Target | Identity (de-duplication) | Order key inside a partition (vanilla policy) | Rule |
|------|------------|--------|---------------------------|-----------------------------------------------|------|
| block write | anything | one position | none | program order of the caller | SIM-SET-002 |
| neighbour request (4 kinds) | a write, a handler | 1 to 6 positions next to a source | none | chain order: depth first, emission order (SIM-NBR-003) | SIM-NBR-002 |
| shape update | a write | one neighbour of the written position, or a wire diagonal | none | chain order | SIM-SHP-001 |
| scheduled tick | a handler or a write | one position | (position, type) per kind | (due tick, priority, sequence), collected per vanilla column (SIM-SCH-006) | SIM-SCH-001 |
| block event | a handler | one position | (position, block, a, b) while pending | FIFO by insertion | SIM-BEV-001 |
| random pick | the phase | one section | none | section visiting order, then pick index | SIM-RND-002 |

- The **sequence** of a scheduled tick is a number that orders scheduling calls:
  in vanilla, the per-dimension sub-tick counter (SIM-SCH-003).
- Every key an ordering policy may use is carried by the item itself: due tick,
  priority, sequence, insertion order. No key is derived from the identity of the
  partition, thread or column holding the item, except in the vanilla collection
  order, which groups by column (SIM-SCH-006).

Provenance: `derived`. Confidence: high.

#### SIM-MODEL-006: Causal dependency

- Work item `y` *depends on* item `x` when `x` created `y` (emission), or when `y`
  reads a position that `x` writes and `x` precedes `y` in the order the instance
  defines (data).
- A set of items is *causally connected* when the dependency relation joins them.
- The **footprint** of an item is the set of positions it may read or write when
  it runs. For the machinery here the footprint is bounded (A.2); what a block
  handler reads is bounded by the handler (for example a piston reads up to 13
  positions along its axis and a tripwire hook up to 41 positions along its line).
- Two causally unconnected sets of items whose footprints are disjoint may run in
  any order and in parallel with the same result, except through the shared
  counters of A.2 (sequence, budget, chain count, random state).

Provenance: `derived`. Confidence: high for the definition; medium for the
handler radii, which belong to block behaviour (out of scope) and are given as
examples only.

#### SIM-MODEL-007: Cross-partition effects

- A work item made while stepping partition `p` whose footprint leaves `p`'s halo
  is a *cross-partition effect*. Its target is owned by another partition `q`,
  or by none (not loaded).
- An instance chooses, per item kind, one of: **synchronous** (only possible when
  the halo covers the target: the whole dimension in vanilla); **message** (the
  item is delivered to `q` and applied at a defined point of `q`'s step, in a
  stable order); **load** (the target section is loaded into `p` first, as
  vanilla does, SIM-SET-003); **drop** (the item has no effect). Parameter
  P-FOREIGN.
- A message keeps every key the item carries (SIM-MODEL-005). Its due tick, if
  any, is expressed as a remaining delay and rebased into `q`'s clock.
- The application point and the order of messages arriving at `q` in one step are
  part of `q`'s ordering policy; it must not depend on which thread delivered them.

Invariant: whatever the choice, an item is applied at most once. An instance that
claims gameplay parity with vanilla across partition boundaries must not drop
(see A.6 for what dropping breaks).

Provenance: `derived`; the drop and refuse choices are taken from the
region-threaded instance (B.3). Confidence: high.

#### SIM-MODEL-008: Ordering policy

An *ordering policy* is, per queue and per partition: the total order in which
the step runs that queue's items, and the rule by which queues of two partitions
combine on merge and divide on split. The contract fixes only the invariants of
A.3; the vanilla policy is the exact one of sections 1 to 9. A policy is
*stable* when the order depends only on item keys and the stored state, never on
thread timing, hash iteration or memory layout.

Requirement: every policy of an instance used for an exact-parity mode is the
vanilla policy over one partition per dimension. Every other policy must be
stable: the same inputs give the same order on any thread; the region-threaded instance's is not (B.3).

Provenance: `derived`. Confidence: high.

### A.2 Locality

A property is *local* when it can be decided from one section and a bounded
neighbourhood of it; it is *global* when it depends on state of the whole
partition (or dimension). Local properties partition freely; global ones are
either kept per partition (and then differ from vanilla when there are several
partitions) or need a cross-partition protocol.

| Mechanism | One step reads/writes | What is global | Rule |
|-----------|----------------------|----------------|------|
| block write | the position, its 6 neighbours for updates | nothing | SIM-SET-002 |
| neighbour request | 1 to 6 positions at distance 1 | the chain: its stack, its order across sections, its request count | SIM-NBR-001, SIM-NBR-003, SIM-NBR-004 |
| comparator output update | up to 2 positions along each horizontal | nothing | SIM-NBR-006 |
| shape update | 1 position at distance 1 (wire diagonal: distance 2) | the update limit travels with the cascade, not with a section | SIM-SHP-001..003 |
| scheduled tick: schedule | its position | the sequence counter; de-duplication is per position, so local | SIM-SCH-003, SIM-SCH-004 |
| scheduled tick: collect | the queue heads of all sections | the budget, the merge order across columns | SIM-SCH-006, SIM-SCH-008 |
| block event | one position | FIFO order of the single queue; the drain runs to exhaustion | SIM-BEV-001, SIM-BEV-002 |
| random pick | one section | the position generator and the level random streams; section visiting order | SIM-RND-001..003 |
| precipitation pick | one column surface | as random picks, plus the heightmap | SIM-RND-004 |
| client sync | marks per section | nothing (packets per section) | SIM-SET-005 |
| save | per vanilla column record | restored sequence numbering per column record | SIM-SCH-012, SIM-SCH-013 |

Consequences:

- Every **single step** of the machinery is local with radius at most 2 blocks;
  block handlers add their own bounded radii (A.1, SIM-MODEL-006).
- What is global is a short list of **counters and orders**: the scheduled-tick
  sequence, the scheduled-tick budget, the chain request count, the chain's
  depth-first order, the block event FIFO, the random streams, and the order in
  which sections are visited. These are exactly what a partitioned instance must
  either keep per partition or reproduce across partitions.

Provenance: `derived` from the cited rules. Confidence: high.

### A.3 Behaviour contracts

Each contract lists (a) invariants, (b) freedoms, (c) parameters (A.5).

#### SIM-MODEL-101: Block writes

- (a) The flag semantics of SIM-SET-001, the sequence of SIM-SET-002, destroy of
  SIM-SET-004: unchanged in every instance. A write whose result is false emits
  nothing. Neighbour updates carry the old block type.
- (b) None inside one write.
- (c) P-FOREIGN for writes outside the halo; P-LOAD for writes to positions not
  *loaded*.

#### SIM-MODEL-102: Neighbour and shape updates; the chain

- (a) Every accepted request runs exactly once, each step at most once; the
  handler that made a request completes before the request runs; a request reads
  the target state at the moment it runs, except the captured fields of
  SIM-NBR-002; a write outside any chain returns only after its whole chain; the
  traversal does not recurse (SIM-NBR-003).
- (a) A chain that stays inside one partition runs exactly as SIM-NBR-003 within
  that partition.
- (b) The order between requests of one chain that are causally unconnected and
  have disjoint footprints. The vanilla order is depth first in emission order;
  an implementation may use another order only for such requests.
- (b) A chain that crosses a partition boundary: the part beyond the boundary is
  a cross-partition effect (SIM-MODEL-007) and runs later, in the receiving
  partition, as the start of a new chain there.
- (c) P-CHAIN (scope of the chain and its count), P-FOREIGN, P-LOAD.

#### SIM-MODEL-103: Limits

- (a) **Update limit**: a shape cascade has at most `L` generations after the
  original write, `L` = 512 by default, counted per cascade, across sections and
  across partitions (a message carries the remaining limit) (SIM-SHP-002).
- (a) **Chain limit**: a chain accepts at most `M` requests (SIM-NBR-004); the
  server never fails on a long chain.
- (b) Where the chain is divided into per-partition pieces, the count may be kept
  per piece; then a long chain crossing `n` partitions may run up to `n·M`
  requests. An instance states this (P-CHAIN).

#### SIM-MODEL-104: Scheduled ticks

- (a) Fields and identity (SIM-SCH-001); priority values and decoding
  (SIM-SCH-002); de-duplication by (position, type) per kind, ended by collection
  (SIM-SCH-004); the delay and promptness invariants of SIM-MODEL-004; the
  type check at run (SIM-SCH-007); block ticks of a step before its fluid ticks
  (SIM-SCH-011); a tick is collected at most once and ticks scheduled during a
  phase are not collected by it (SIM-SCH-009); the queries of SIM-SCH-010.
- (a) Inside one section, collected ticks run in full-key order.
- (a) A merge, split or message keeps a tick's priority and sequence; only its due
  tick may be rebased (SIM-MODEL-004).
- (b) The order across sections (vanilla: column-grouped merge, SIM-SCH-006). The
  scope and value of the budget: vanilla 65 536 per kind per dimension per step;
  with several partitions it is per partition (P-BUDGET).
- (b) The scope of the sequence counter (P-SEQ). With one counter shared by
  partitions that run in parallel, two ticks' relative sequence depends on
  thread timing; a stable instance derives the sequence from partition-local
  state (for example (partition time, local counter)) and orders across merged
  partitions by a stable rule.
- (c) P-CLOCK, P-SEQ, P-BUDGET, P-COLLECT, P-FOREIGN.

#### SIM-MODEL-105: Block events

- (a) De-duplication of equal pending events; delivery checks the block type at
  delivery; the packet and its range (SIM-BEV-004); events are not saved;
  deferral while the position does not *deliver block events* keeps relative
  order (SIM-BEV-003); events queued during a drain are delivered in the same
  drain (SIM-BEV-002).
- (a) FIFO among the events of one partition.
- (b) The order between events of different partitions: none is defined once
  there are several partitions. On merge the two queues must be combined by a
  stable rule (P-EVENTS).
- (c) P-EVENTS, P-FOREIGN.

#### SIM-MODEL-106: Random ticks and precipitation

- (a) Distributions only (SIM-RND-001, SIM-RNG-001): each randomly ticking block
  state of a *random-ticked* section receives random ticks at the rate of `k`
  uniform picks per step over the section's 4096 positions, with replacement;
  precipitation at `k` picks per column surface per step, each with probability
  1/48.
- (b) The section visiting order, the random streams and the pick algorithm, as
  long as the per-block rate and independence hold.
- (c) P-RNG, P-RANDOM.

#### SIM-MODEL-107: Client sync

- (a) SIM-SET-005: at most one packet per section per broadcast, carrying current
  states; the broadcast of a step follows its scheduled and random ticks and
  precedes its block events.
- (b) The order across sections.

#### SIM-MODEL-108: Persistence

- (a) The save layout of SIM-SCH-012 (it is the save format, which is exact): one record per vanilla column, delays relative to the
  owner's clock, restored sequences `k − m` per record (SIM-SCH-013).
- (b) None in the format; when a column record is written is free.

#### SIM-MODEL-109: Time gates

- (a) Freeze and step (SIM-TICK-004) stop and release every gated phase; pending
  ticks wait and do not run early after a resume.
- (b) Whether the gate is per world or per partition (P-GATE). With per-partition
  gates, partition time stops while world time may advance.

### A.4 Phases of a partition step

Every instance runs, per partition step that runs normally, in this order: the
scheduled block ticks, the scheduled fluid ticks, the random and precipitation
picks with the client broadcast after them, the block events, then entities and
block entities (SIM-TICK-002). Neighbour and shape updates run inside whichever
phase causes them. What lies before and after (world border, weather, world
time) may run outside partitions, once per dimension.

Provenance: `source_suggested` for both instances (B.2, B.3). Confidence: high.

### A.5 Parameters an instance chooses

| Id | Parameter | Values |
|----|-----------|--------|
| P-SHAPE | partition shape and granularity | e.g. whole dimension; groups of whole columns; sets of cubic sections |
| P-FORM | formation, merge and split rule, and when they apply | e.g. none; proximity of loaded columns, merged on contact and split when emptied; causal connectivity |
| P-HALO | positions a step may touch synchronously | e.g. whole dimension; owned area minus a margin |
| P-CLOCK | lock-step or free-running partition time; rebasing rule | SIM-MODEL-004 |
| P-SCHED | how steps are scheduled; catch-up when late | e.g. one loop; independent deadlines per partition |
| P-SEQ | scope of the scheduled-tick sequence counter | per dimension; per partition; derived |
| P-COLLECT | collection order across sections | vanilla column merge; global full key; per section |
| P-BUDGET | budget scope and value | per dimension; per partition |
| P-CHAIN | scope of a chain and its count | per dimension; per partition piece |
| P-EVENTS | block event queue scope and merge rule | per dimension; per partition, appended on merge |
| P-FOREIGN | what happens to each item kind whose target is outside the halo | synchronous; message; load; drop; refuse with an error |
| P-LOAD | whether a read or write at a valid unloaded position loads it | yes (vanilla); no |
| P-PROPS | derivation of section properties | vanilla tickets per column; other |
| P-RNG | random streams and their scope | one per dimension; per partition; per section |
| P-RANDOM | random pick algorithm | vanilla per-section picks; others preserving rates |
| P-GATE | scope of freeze, step and pause-when-empty | per server; per partition |
| P-WIRE | redstone wire evaluation algorithm | vanilla; alternatives with other update orders |

### A.6 Which detailed rules hold for every instance

- **Universal as written** (every instance keeps them, whatever its parameters):
  SIM-SET-001, SIM-SET-002 (without the synchronous load of step 2),
  SIM-SET-004, SIM-NBR-002, SIM-NBR-005, SIM-SHP-002, SIM-SHP-004, SIM-SCH-001,
  SIM-SCH-002, SIM-SCH-004, SIM-SCH-007, SIM-SCH-010, SIM-SCH-011, SIM-SCH-012
  (layout), SIM-SCH-013 (numbering), SIM-SCH-014, SIM-BEV-001, SIM-BEV-004,
  SIM-RND-003 (as the vanilla generator; other instances may replace it, P-RNG).
- **Universal observable result, instance order**: SIM-NBR-001, SIM-NBR-003,
  SIM-NBR-006, SIM-SHP-001, SIM-SHP-003, SIM-SCH-006, SIM-SET-005, SIM-RND-001,
  SIM-RND-002, SIM-RND-004, SIM-BEV-002.
- **Instance-specific**: SIM-TICK-001 to SIM-TICK-005 (loop, clocks, gates,
  section properties), SIM-SET-003 (loading), SIM-NBR-004 (chain scope),
  SIM-NBR-007 (draws from the level random), SIM-SCH-003 (sequence scope),
  SIM-SCH-005 (queue grouping), SIM-SCH-008 (budget scope), SIM-SCH-009 (phase
  boundaries are per step), SIM-SCH-015, SIM-SCH-016, SIM-SCH-017, SIM-BEV-003
  (eligibility input), SIM-RNG-001.

Provenance: `derived`, checked against B.3. Confidence: medium; each entry
follows from the instance comparison, which is itself of mixed confidence.

---

# Part B — Instances

## B. Instances

### B.1 Parameter map

| Parameter | Vanilla (B.2, sections 1–9) | Region-threaded server (B.3) |
|-----------|-----------------------------|------------------------------|
| P-SHAPE | one partition per dimension | regions: unions of square groups of columns (default 16 × 16 columns, full height) |
| P-FORM | none | groups containing any column with a load record, padded by one group on every side, merged when padded groups touch; split after a step when enough groups emptied (B.3) |
| P-HALO | whole dimension | owned groups; most update paths also require a 25-block margin to be owned |
| P-CLOCK | lock-step: partition time = world time, shared by all dimensions | free-running per region; rebased on merge |
| P-SCHED | one loop, dimensions in sequence | each region on a thread pool with its own deadline; a late region skips periods instead of catching up |
| P-SEQ | one counter per dimension | one atomic counter per dimension shared by all regions |
| P-COLLECT | vanilla column merge (SIM-SCH-006) | the same algorithm per region |
| P-BUDGET | 65 536 per kind per dimension | per kind per region, configurable, default 65 536 |
| P-CHAIN | one chain per dimension | one chain per region |
| P-EVENTS | one FIFO per dimension | one FIFO per region; absorbed region's events appended on merge |
| P-FOREIGN | synchronous (whole dimension is the halo) | neighbour and shape updates: drop; a block write, scheduling a tick or queueing an event: refused with an error; a read of a loaded foreign position: unsynchronised read |
| P-LOAD | yes, synchronously, per column | updates do not load; other reads and writes load an owned column synchronously and refuse a foreign one |
| P-PROPS | tickets per column (SIM-TICK-005) | the chunk system's ticking state per column |
| P-RNG | one level random and one position generator per dimension | one thread-local level random (shared by whatever runs on the thread), one fast random per region for random ticks; the position generator draws from the thread-local random |
| P-RANDOM | vanilla picks | picks over a per-section list of ticking positions, same per-block rate |
| P-GATE | per server; pause when empty | per region and for the global tick separately; no step; pause when empty disabled |
| P-WIRE | vanilla | vanilla by default; two alternatives selectable |

### B.2 The vanilla instance

Sections 1 to 9 below are the vanilla instance. Their rules are exact; a
**Vanilla grouping** paragraph states where vanilla groups sections into columns.
An exact-parity mode is this instance: one partition per dimension, lock-step
clock, vanilla ordering policy for every queue.

### B.3 The region-threaded instance

A region-threaded server (a fork of a widely used server that splits each
dimension into regions ticked in parallel) is the instance in the right-hand
column of B.1. The behaviour that matters for the model:

- **Regions are formed by proximity, not causality.** Columns are grouped into
  square groups (default 16 × 16 columns); a group belongs to a region when any
  column in it has a load record, and every such group pads itself with its eight
  neighbour groups. Regions whose padded groups touch merge; owned areas of two
  regions are therefore at least one group apart. Merges apply between steps
  (deferred while a participant is mid-step); splits are considered when a region
  finishes a step and enough of its groups have emptied.
- **Partition time is free-running.** Each region has its own clock, starting at
  1 for a new region, advanced once per region step. Scheduled ticks, saved tick
  delays and redstone torch burnout use it. World time is advanced separately,
  by the global tick, with catch-up. On merge the absorbed region's due ticks are
  shifted by the clock difference; on split each part copies the parent clock.
- **Cross-region effects are dropped or refused.** A neighbour or shape update
  whose target is not owned with a 25-block margin, or not loaded, is skipped.
  Scheduling a tick or queueing a block event at a position the region does not
  own is an error. Nothing is delivered as a message.
- **Global counters stay global and become nondeterministic.** The sub-tick
  counter is one atomic counter per dimension shared by all regions.
- **Known divergences:** pending ticks of the absorbed region lose their priority
  when the clocks differ at a merge; random ticks and precipitation use per-region
  and per-thread streams; lava receives one fluid random tick per pick instead of
  two; client-sync marks depend on whether the column was sent to a player; the
  gates have no step and apply per region.

---

# The vanilla instance in detail

---

## 1. Where block work runs

### SIM-TICK-001: Server tick and the gates before it

- **Inputs:** one iteration of the server loop.
- **Relevant state:** the online player count; the server property `pause-when-empty-seconds` (default 60); the tick-rate state (SIM-TICK-004).
- **Preconditions:** none.
- **Behavior:**
  1. If `pause-when-empty-seconds` × 20 is greater than 0, the server counts consecutive server ticks with zero players while not sprinting. Once that count reaches the threshold, the server tick does nothing except network handling: no level ticks run and game time does not advance. The first paused tick saves the world.
  2. Otherwise the tick-rate state advances (SIM-TICK-004), then:
     1. command functions tick;
     2. if runs normally: world clocks advance;
     3. every 20 server ticks: time synchronisation is sent;
     4. each dimension runs one level tick (SIM-TICK-002), one dimension after another, in dimension creation order: the overworld first, then the other dimensions in the order they were created at startup;
     5. connections, the player list and the rest of the server tick.
- **Ordering constraints:** observable result and vanilla order coincide: dimensions tick sequentially, never interleaved, in the order above.
- **Numeric semantics:** threshold = `pause-when-empty-seconds × 20` server ticks.
- **Postconditions:** after a server tick that runs normally, game time is one greater (SIM-TICK-003).
- **Edge cases:** a paused server does not count game time, so pending scheduled ticks stay due at the same game time (SIM-SCH-003).
- **Invariants:** block work of two dimensions never interleaves within one server tick.
- **Conformance tests:**
  - With no player online and `pause-when-empty-seconds=1`, after 20 server ticks game time stops advancing and a repeater tick scheduled for game time `G+4` does not run while paused. Failing twin: with one player online, it runs at `G+4`.

Provenance: `source_suggested`. Confidence: high.

### SIM-TICK-002: Phases of one level tick

- **Inputs:** one level tick of one dimension.
- **Relevant state:** runs normally (SIM-TICK-004); whether the world is a debug world; the game rule `random_tick_speed`; whether the dimension has active tickets.
- **Preconditions:** SIM-TICK-001 reached this dimension.
- **Behavior:** the level tick runs these steps in this order. A step marked *gated* runs only when the tick runs normally.

| Step | Work | Gated | Block-simulation content |
|---|-------|-------|--------------------------|
| 1 | world border | yes | none |
| 2 | weather | yes | none |
| 3 | sleep check (may jump the default clock and clear weather) | no | none |
| 4 | sky brightness | no | none |
| 5 | game time advance (overworld only, SIM-TICK-003) | yes | none |
| 6 | scheduled block ticks (SIM-SCH-006) | yes, and skipped in a debug world | block ticks |
| 7 | scheduled fluid ticks (SIM-SCH-006) | yes, and skipped in a debug world | fluid ticks |
| 8 | raids | yes | none |
| 9 | loading and per-area work: ticket purge and load-level updates; then, if runs normally and not a debug world, spawning work (thunder, natural spawning) over the spawning area, followed by random ticks and precipitation picks (SIM-RND-001), then the special spawners; then, if not a debug world, the broadcast of changed blocks to clients (SIM-SET-005), whether or not the tick runs normally; then unloading and saving | partly | random ticks; precipitation; client sync of block changes; saving of pending ticks (SIM-SCH-012) |
| 10 | block events (SIM-BEV-002) | yes | block events |
| 11 | entities (only while the dimension has had active tickets within the last 300 level ticks) | per entity | none here |
| 12 | block entities (same 300-tick condition; each ticker runs only if runs normally and its position delivers block events, SIM-TICK-005) | yes | none here |
| 13 | entity management, debug | no | none |

  Neighbour and shape updates are not a phase. They run synchronously inside whatever phase causes a block change (SIM-NBR-003).
- **Ordering constraints:** observable result and vanilla order coincide for the phase order. In particular: all scheduled block ticks of a level tick run before all its scheduled fluid ticks; both run before random ticks; random ticks run before block events; block events run before entities and block entities.
- **Numeric semantics:** the empty-dimension threshold is 300 level ticks without an active ticket (counted only on ticks that run normally).
- **Postconditions:** none beyond the phases.
- **Edge cases:**
  - The client broadcast of block changes (step 9) runs after scheduled ticks and random ticks of this tick but before block events, entities and block entities. Block changes made in steps 10–12 reach clients in step 9 of the next level tick.
  - Thunder and natural spawning for the whole spawning area run before any random tick of the level tick.
- **Invariants:** a scheduled tick never runs after a block event of the same level tick.
- **Conformance tests:**
  - At game time G, schedule a block tick with delay 1 for probe block P, a fluid tick with delay 1 for a probe fluid at Q, and queue a block event for P. During the level tick of G+1 the log reads: block tick, fluid tick, block event. Failing twin: the fluid tick before the block tick.
  - A probe block random tick at P queues a block event; the event is delivered in the same level tick (step 10 follows step 9).

Provenance: `source_suggested`. Confidence: high.

### SIM-TICK-003: Game time and the due tick

- **Inputs:** a level tick that runs normally.
- **Relevant state:** game time `G` before the tick.
- **Preconditions:** none.
- **Behavior:** in step 5 of the overworld's level tick, game time becomes `G + 1` (the scheduled server function queue for `G + 1` also runs there). Other dimensions do not advance game time; they read the shared value. Scheduled ticks of every dimension in this server tick therefore run with *current tick* = `G + 1`.
- **Ordering constraints:** game time advances before any scheduled tick of the same server tick runs, in every dimension.
- **Numeric semantics:** 64-bit integer; +1 per server tick that runs normally.
- **Postconditions:** a tick scheduled with delay `d` at game time `T` is due at `T + d` (SIM-SCH-003).
- **Edge cases:** during a tick that does not run normally, game time does not advance and no scheduled tick runs, so due ticks keep waiting. A non-overworld dimension's own level data ignores writes to game time.
- **Invariants:** game time is the same in every dimension at any instant.
- **Conformance tests:** a tick scheduled with delay 2 while game time reads 100 runs in the level tick in which game time becomes 102, in any dimension.

Provenance: `source_suggested`. Confidence: high.

### SIM-TICK-004: The tick-rate gate (freeze and step)

- **Inputs:** `/tick freeze`, `/tick unfreeze`, `/tick step N`.
- **Relevant state:** `frozen` (boolean), `steps_left` (integer ≥ 0).
- **Preconditions:** none.
- **Behavior:** at the start of each server tick, *runs normally* := `!frozen || steps_left > 0`; then, if `steps_left > 0`, it decreases by one. When not running normally, these do not run: world border, weather, game time, world clocks, scheduled block and fluid ticks, raids, random ticks and spawning, block events, block entity ticks, non-player entities (players and entities carrying a player passenger still tick). These still run: loading and unloading, the client broadcast of block changes, and every neighbour and shape update caused by something that still runs (for example a player placing a block).
- **Ordering constraints:** none beyond SIM-TICK-002.
- **Numeric semantics:** `/tick step N` lets exactly N server ticks run normally.
- **Postconditions:** block events queued while frozen stay queued; scheduled ticks stay queued and become due by game time only.
- **Edge cases:** a block placed while frozen still performs its whole neighbour/shape chain immediately, and any tick it schedules is due relative to the frozen game time.
- **Invariants:** game time advances exactly on the ticks that run normally.
- **Conformance tests:**
  - Frozen; a player powers a repeater with delay 1. No change after 10 server ticks; after `/tick step 2` the repeater output turns on (repeater delay 1 is 2 game ticks). Failing twin: turns on while frozen.

Provenance: `source_suggested`. Confidence: high.

### SIM-TICK-005: Section properties for each kind of work

- **Inputs:** a section.
- **Relevant state:** the inputs the host derives the properties from. In vanilla: the load level and simulation level of the section's column, whether the column's entities are loaded, and whether the column has reached block-ticking status.
- **Preconditions:** none.
- **Behavior:** each kind of work tests one property of the section holding the position concerned. The properties are section-native; vanilla derives every one of them from the column, so all sections of one column share them.

| Property | Gates | Vanilla derivation (column-grouped) |
|----------|-------|-------------------------------------|
| *loaded* | reads and sets without a synchronous load (SIM-SET-003) | the column is loaded at full status (load level ≤ 33) |
| *accepts ticks* | scheduled ticks are stored rather than dropped (SIM-SCH-005) | the column is loaded at full status; equal to *loaded* |
| *runs scheduled ticks* | stored ticks may be collected (SIM-SCH-006) | simulation level ≤ 32, and the column has reached block-ticking status (load level ≤ 32, which requires the eight neighbouring columns to be full), and the column's entities are loaded |
| *delivers block events* | block events are delivered, not deferred (SIM-BEV-003); block entities tick | simulation level ≤ 32; nothing else is checked |
| *random-ticked* | random ticks (SIM-RND-001) | simulation level ≤ 31 and the column has reached block-ticking status |
| *broadcasts changes* | sets are marked for client sync (SIM-SET-005) | the column has reached block-ticking status |

  Two transitions also matter: saved ticks activate (SIM-SCH-013) and post-processing runs (SIM-SCH-017) when the section starts to *broadcast changes* (vanilla: when its column reaches block-ticking status).
- **Ordering constraints:** none.
- **Numeric semantics:** full = 33, block-ticking = 32, entity-ticking = 31. In vanilla the simulation level comes from the simulation distance (server property `simulation-distance`, default 10) and the load level from the view distance and other tickets.
- **Postconditions:** none.
- **Edge cases:**
  - *random-ticked* requires the entity-ticking threshold although *runs scheduled ticks* requires only block-ticking. A section at simulation level 32 runs its scheduled ticks but receives no random ticks.
  - *delivers block events* does not require the section to be loaded.
- **Invariants:** *runs scheduled ticks* implies *accepts ticks*.
- **Vanilla grouping:** load and simulation levels are computed per column by the ticket system, with no vertical component: a section's properties depend only on `(sx, sz)`. A host may compute properties per section by any rule; an exact-parity mode assigns every section of a column the properties vanilla gives the column.
- **Conformance tests:** with simulation distance 3, a section at simulation level 32 (one ring outside the entity-ticking area) runs a pending repeater tick but a sapling there never grows by random ticks. Failing twin: the sapling grows.

Provenance: `source_suggested`. Confidence: high.

---

## 2. Setting a block

### SIM-SET-001: Update flags

- **Inputs:** an integer bit set passed with every block set.
- **Relevant state:** none.
- **Preconditions:** none.
- **Behavior:** each bit has exactly this effect on the server. Bits not listed are ignored. The names are this document's own.

| Bit | Value | Name | Effect |
|-----|-------|------|--------|
| 0 | 1 | *notify-neighbours* | the set emits neighbour updates (SIM-NBR-001) and comparator output updates (SIM-NBR-006); it also lets the old block's removal side effect run (SIM-SET-002 step 2.5) |
| 1 | 2 | *sync-clients* | the change is marked for client sync (SIM-SET-005) |
| 2 | 4 | *no-rerender* | client-side only: on a client level, suppresses the re-render. No effect on the server |
| 3 | 8 | *rerender-now* | client-side only: re-render synchronously. No effect on the server |
| 4 | 16 | *keep-shape* | the set emits no shape updates, direct or indirect (SIM-SHP-001) |
| 5 | 32 | *no-drops* | read only when a shape update turns a block into air: the block is removed without drops (SIM-SHP-002) |
| 6 | 64 | *piston-move* | the old block's removal side effect and the new block's placement side effect are told "moved by piston"; the removal side effect runs even without *notify-neighbours* |
| 7 | 128 | *wire-skips-shape* | a shape update carrying this bit is skipped when the block receiving it is `minecraft:redstone_wire` |
| 8 | 256 | *skip-block-entity-effects* | when the old block entity is removed, its pre-removal side effects (for example dropping a container's contents) are skipped |
| 9 | 512 | *skip-placement* | the new state's placement side effect does not run |

  Combinations used below:

| Value | Bits |
|-------|------|
| 3 | *notify-neighbours*, *sync-clients* (the ordinary set) |
| 11 | *notify-neighbours*, *sync-clients*, *rerender-now* |
| 260 | *no-rerender*, *skip-block-entity-effects* |
| 816 | *keep-shape*, *no-drops*, *skip-block-entity-effects*, *skip-placement* |

- **Ordering constraints:** none.
- **Numeric semantics:** the flag word is a 32-bit integer.
- **Postconditions:** none.
- **Edge cases:** 260 sends nothing to clients only because it lacks *sync-clients*; *no-rerender* does not suppress client sync on the server.
- **Invariants:** the flags a shape update carries are derived from the flags of the set that emitted it (SIM-SHP-001).
- **Conformance tests:** table-driven, one row per flag combination, over a probe block at P with probe neighbours on all six sides:

| Flags | Neighbour updates logged | Shape updates logged | Marked for clients | Placement logged |
|-------|--------------------------|----------------------|--------------------|------------------|
| 3 | 6 | 6 | yes | yes |
| 2 | 0 | 6 | yes | yes |
| 18 | 0 | 0 | yes | yes |
| 1 | 6 | 6 | no | yes |
| 515 | 6 | 6 | yes | no |
| 816 | 0 | 0 | no | no |

Provenance: `source_suggested`. Confidence: high.

### SIM-SET-002: The sequence of one block set

- **Inputs:** position P, new state S, flags F, update limit L (default 512).
- **Relevant state:** the state O at P; the section holding P.
- **Preconditions:** none.
- **Behavior:**
  1. If P is outside the valid bounds (SIM-SET-003), the set is refused: result *false*, nothing happens. In a debug world, every server-side set is refused.
  2. P's section is made *loaded* (loading it synchronously if needed, SIM-SET-003). Then:
     1. If P's section contains only air and S is air: result *false*, nothing happens.
     2. S is written. If O is the same state as S: result *false*, nothing else happens.
     3. Section emptiness, light bookkeeping and, in vanilla, the heightmaps of P's column are updated.
     4. If the block type changed, O had a block entity and S does not keep it: unless *skip-block-entity-effects*, the old block entity's pre-removal side effects run; the block entity is removed.
     5. If (the block type changed, or S is any rail) and (*notify-neighbours* or *piston-move*): O's removal side effect runs with "moved by piston" = *piston-move*.
     6. If the state now at P is no longer of S's block type (step 5 replaced it): result *false*, nothing else happens.
     7. Unless *skip-placement*: S's placement side effect runs with the old state O and "moved by piston".
     8. If S has a block entity, it is created or updated.
  3. If the state at P is now exactly S (steps 2.5 and 2.7 may have changed it):
     1. If *sync-clients* and P's section *broadcasts changes* (SIM-TICK-005): P is marked for client sync (SIM-SET-005), and mob paths crossing a changed collision shape are recomputed.
     2. If *notify-neighbours*: neighbour updates around P with source block = **O's** block type (SIM-NBR-001); then, if S has an analog output signal, comparator output updates with source = S's block type (SIM-NBR-006).
     3. If not *keep-shape* and L > 0, with flags F' = F without *notify-neighbours* and without *no-drops*, and limit L − 1: O's indirect shape updates (SIM-SHP-003), then S's direct shape updates (SIM-SHP-001), then S's indirect shape updates.
     4. Points of interest are updated.
  4. The result is *true* whenever step 2 got past 2.6, including when step 3 was skipped.
- **Ordering constraints:** observable result and vanilla order coincide within one set: removal side effect, then placement side effect, then client marking, then neighbour updates, then comparator updates, then old-indirect, direct and new-indirect shape updates. When no chain is running these run to completion inside the set; inside a chain they are queued in this order (SIM-NBR-003).
- **Numeric semantics:** L is decremented once per generation of shape updates (SIM-SHP-002).
- **Postconditions:** if the result is true, P held S at least momentarily.
- **Edge cases:**
  - Setting a state equal to the current one is a no-op with result false: no updates, no client sync.
  - The neighbour updates carry the **old** block type as their source, not the new one.
  - A placement side effect that sets P again makes step 3 compare against the newer state; if it differs from S, step 3 is skipped entirely but the result is still true.
  - A rail replaced by a rail of another shape (same block type) still runs the removal side effect.
  - A set whose updates cross into other sections behaves exactly as within one section; no step of this rule depends on section or column boundaries.
- **Invariants:** no update of any kind is emitted by a set whose result is false.
- **Conformance tests:**
  - Probe block A at P replaced by probe block B with flags 3: log order is A.removed, B.placed, then six neighbour updates whose source is A, then six shape updates. Failing twin: neighbour updates with source B.
  - Same state set twice with flags 3: the second logs nothing and returns false.
  - B's placement handler sets P to probe C: step 3 logs no neighbour or shape update for the B set; the C set logs its own.

Provenance: `source_suggested`. Confidence: high.

### SIM-SET-003: Sets and reads at unloaded or invalid positions

- **Inputs:** a set, or a read (for example by a neighbour or shape update), at position P.
- **Relevant state:** whether P's section is *loaded*.
- **Preconditions:** none.
- **Behavior:**
  - **Valid bounds.** P is valid iff its y lies inside the dimension's vertical extent (dimension type `min_y` ≤ y < `min_y + height`, vanilla data) and `max(|sx|, |sz|)` ≤ the valid horizontal bound (section coordinates; block coordinate limit 33 554 431 → 2 097 151, minus a generation safety margin; see section 9). A read at an invalid position returns `minecraft:void_air` (fluid: empty) and loads nothing; a set there is refused.
  - **Synchronous load.** On the server main thread, a set or a read at a valid position whose section is not *loaded* **loads (or generates) it synchronously** before continuing. The section is then *loaded* and *accepts ticks* for at least the rest of the level tick, but does not *run scheduled ticks* unless something else makes it, so ticks scheduled into it wait (SIM-SCH-015).
- **Ordering constraints:** the load completes before the set or read continues.
- **Numeric semantics:** see Vanilla grouping.
- **Postconditions:** P's section is *loaded* for at least the rest of the tick.
- **Edge cases:** many vanilla blocks guard their own reads with an "is loaded" check; comparator output updates (SIM-NBR-006) skip some unloaded neighbours. The neighbour updater itself does not check.
- **Invariants:** a set never silently writes into an unloaded section; it loads it first.
- **Vanilla grouping:** the unit vanilla loads is P's whole column, generated to full status, and it holds it there with a temporary ticket on the column: level 33, lifetime 1 level tick. An exact-parity mode loads every section of that column inside the dimension's vertical extent and gives them all the same properties.
- **Conformance tests:** see OQ-03.

Provenance: `source_suggested`. Confidence: medium (OQ-03).

### SIM-SET-004: Destroying a block

- **Inputs:** destroy at P, drop flag, update limit L (default 512).
- **Relevant state:** state O at P; its fluid.
- **Preconditions:** none.
- **Behavior:** if O is air, nothing happens (result false). Otherwise destroy particles are emitted, drops are spawned if the drop flag is set, then P is set to the block form of the fluid at P (air, or the fluid's source/flowing block) with flags 3 and limit L (SIM-SET-002). If that set succeeds a `block_destroy` game event is emitted. "Remove block" (without drops) sets the fluid's block form with flags 3, plus *piston-move* when requested.
- **Ordering constraints:** drops before the set; the set's chain before the game event.
- **Numeric semantics:** none.
- **Postconditions:** a waterlogged block destroyed leaves water.
- **Edge cases:** a destroy always emits neighbour updates (flags 3), even when it was caused by a shape update whose flags had no *notify-neighbours*.
- **Invariants:** none.
- **Conformance tests:** a probe block that a shape update turns into air logs neighbour updates around it. Failing twin: none logged.

Provenance: `source_suggested`. Confidence: high.

### SIM-SET-005: Client sync of block changes

- **Inputs:** sets with *sync-clients*.
- **Relevant state:** per section, the set of marked positions.
- **Preconditions:** P's section *broadcasts changes* when the set happens.
- **Behavior:** marking records only the position, in its section. In step 9 of each level tick (SIM-TICK-002), every section with marks that still *broadcasts changes* sends to the players that see it: one `block_update` packet with the state **current at broadcast time** if exactly one position is marked, or one `section_blocks_update` packet with the current states of all marked positions otherwise; block entity data follows where needed. Marks are then cleared.
- **Ordering constraints:** observable result: one packet per marked section per broadcast, carrying current states. Vanilla order: see Vanilla grouping.
- **Numeric semantics:** packet ids as in 0.5.
- **Postconditions:** a client never sees an intermediate state of a position that changed twice between broadcasts.
- **Edge cases:**
  - A set with *sync-clients* in a section that is *loaded* but does not *broadcast changes* is not marked at all; a client learns the state only when the section is (re)sent (OQ-06).
  - A position set and then set back to its original state before the broadcast still produces a packet carrying the original state.
  - With no player seeing the section, its marks are cleared and nothing is sent.
- **Invariants:** at most one packet per section per broadcast.
- **Vanilla grouping:** marks live per section inside a per-column record. "The players that see a section" are the players tracking its column. The broadcast visits columns in the iteration order of an identity-keyed hash set, which is neither insertion order nor reproducible across runs; inside one column it visits sections from the bottom up. An exact-parity mode need only keep "bottom up within a column"; the order across columns is free.
- **Conformance tests:** set P to stone then to dirt within one scheduled-tick phase: the client receives one `block_update` with dirt. Failing twin: two packets.

Provenance: `source_suggested`. Confidence: high for content and for the order inside a column; high that the order across columns is not reproducible.

---

## 3. Neighbour updates

### SIM-NBR-001: Neighbour updates around a position

- **Inputs:** a source position P, a source block type B, optionally a skipped direction X, optionally an orientation (SIM-NBR-007).
- **Relevant state:** the states of the six neighbours of P at the moment each is updated.
- **Preconditions:** none.
- **Behavior:** each neighbour Q = P + d, for d in neighbour order `W, E, D, U, N, S` except X, receives "neighbour changed" with (Q's state **read at the moment of this step**, Q, B, orientation, moved-by-piston = false).
- **Ordering constraints:**
  - Observable result: every neighbour except X receives exactly one update carrying B.
  - Vanilla order: `W, E, D, U, N, S`, with the cascade rule of SIM-NBR-003 between steps.
- **Numeric semantics:** none.
- **Postconditions:** none.
- **Edge cases:** the state read for a later direction reflects every change made by the chains of earlier directions. Neighbours in other sections are updated identically; a neighbour in an unloaded section is loaded by the read (SIM-SET-003).
- **Invariants:** at most six updates per request.
- **Conformance tests:** probe neighbours on all six sides of P; a set at P with flags 1 logs updates at P+W, P+E, P+D, P+U, P+N, P+S in that order. Failing twin: shape order.

Provenance: `source_suggested`. Confidence: high.

### SIM-NBR-002: Kinds of queued update

- **Inputs:** an update request.
- **Relevant state:** none.
- **Preconditions:** none.
- **Behavior:** a request is one of four kinds. They differ in what is captured when the request is made and what is read when it runs.

| Kind | Captured at request | Read at run | Steps |
|------|---------------------|-------------|-------|
| neighbours-around (SIM-NBR-001) | P, B, skipped direction, orientation | each neighbour's state, per step | up to 6 |
| single, by position | target Q, B, orientation | Q's state | 1 |
| single, with state | target Q, **Q's state**, B, orientation, moved-by-piston | nothing; the captured state is used | 1 |
| shape update (SIM-SHP-001) | target Q, direction, **the changed neighbour's state**, its position, flags, limit | Q's state | 1 |

- **Ordering constraints:** none beyond SIM-NBR-003.
- **Numeric semantics:** each request counts as one towards the chain limit regardless of its step count (SIM-NBR-004).
- **Postconditions:** none.
- **Edge cases:** a "single, with state" request runs the handler of the captured state even if Q changed meanwhile; comparator output updates use this kind. A shape update passes the neighbour state captured at request time even if the neighbour changed again before it ran.
- **Invariants:** none.
- **Conformance tests:** inside a chain, request "single, with state" for Q holding probe A, then change Q to probe C before it runs: A's handler is logged, not C's.

Provenance: `source_suggested`. Confidence: high.

### SIM-NBR-003: The chain and its traversal order

- **Inputs:** update requests (SIM-NBR-002) made from anywhere: a set, a block handler, a tick.
- **Relevant state:** whether a chain is running; the chain's stack of requests; the list of requests added during the current step; the chain's request count.
- **Preconditions:** none.
- **Behavior:**
  - **No chain running.** The request starts a chain and the whole chain runs to completion before the request call returns. A set made outside any chain therefore returns only after every update it caused, transitively, has run.
  - **Chain running.** The request is appended to the *added-this-step* list and the call returns immediately. The requesting handler finishes before any of its requests run.
  - **Traversal.** The chain keeps a stack. Repeat until the stack and the added-this-step list are empty: push the added-this-step list onto the stack in reverse, so its first element is on top; clear the list; take the top request and run its steps one at a time; stop running it as soon as a step added requests (they are pushed next round), or when it has no step left, in which case it is popped (a request whose last step added requests is popped once it is next found with no step left).
  - When the chain ends, its count resets to 0.
- **Ordering constraints:**
  - Observable result: every accepted request runs exactly once, each step at most once; the handler that made a request always completes first.
  - Vanilla order: depth first. After each single step, every request that step added runs, in the order added, each completely and depth first, before the next step of the same request. Requests made during one step keep their emission order.
- **Numeric semantics:** none here; the limit is SIM-NBR-004.
- **Postconditions:** when a chain ends, the stack and list are empty.
- **Edge cases:**
  - Within SIM-SET-002 step 3, the requests are emitted in this order and therefore run in this order: neighbours-around P, comparator updates, old-indirect shapes, the six direct shape updates in shape order, new-indirect shapes. Requests emitted by the removal and placement side effects (steps 2.5 and 2.7) come before all of them.
  - Outside a chain, each of those emissions starts and completes its own chain in turn, so the effective order is the same but each emission has its own count.
  - A handler that reads the world after making a request inside a chain does not see that request's effects.
  - A chain is per dimension and spans any number of sections; its order and its count ignore section and column boundaries.
- **Invariants:** the traversal is iterative; the depth of handler nesting does not grow with the chain length. Every live dimension, server and client, delivers neighbour and shape updates through this traversal.
- **Conformance tests:**
  - Probe A at P; probes at all six neighbours. P+W's handler, when updated, sets P+W+W (probe Z with probe neighbours) with flags 1. A set at P with flags 1 logs: P+W, then Z's six neighbours in neighbour order, then P+E, P+D, P+U, P+N, P+S. Failing twin: P+E before Z's neighbours (breadth first).
  - The same, with P+W's handler logging "after-set" after its set call: "after-set" is logged before Z's neighbours. Failing twin: after them (recursive execution).
  - A set at P made from a scheduled tick (no chain running) and followed by a log line "returned": "returned" is logged after every neighbour update of the chain.

Provenance: `source_suggested`. Confidence: high.

### SIM-NBR-004: The chain limit

- **Inputs:** requests within one chain.
- **Relevant state:** the chain's request count `c` (requests offered so far in this chain, accepted or not); the limit `M`.
- **Preconditions:** none.
- **Behavior:** when a request is offered with `M ≥ 0` and `c ≥ M`, it is dropped; otherwise it is accepted. Either way `c` increases by one. When the first request is dropped (`c` was exactly `M`), one error is logged naming its position: "Too many chained neighbor updates. Skipping the rest. First skipped position: x, y, z". The chain continues with the requests already accepted; every later request of the same chain is dropped, because `c` only grows. `M < 0` means unlimited.
- **Ordering constraints:** the requests dropped are exactly the ones offered after the first `M`, in offering order.
- **Numeric semantics:** `M` = server property `max-chained-neighbor-updates`, default 1 000 000; the integrated (single-player) server uses 1 000 000. The first request of the chain counts.
- **Postconditions:** at most `M` requests run per chain.
- **Edge cases:** `M = 0`: nothing ever runs and the first request of every chain logs the error. A neighbours-around request counts once although it updates up to six neighbours. The count resets only when the outermost chain ends, so two sets made outside any chain have independent budgets. The limit is per chain, not per section.
- **Invariants:** the server never crashes on a long chain.
- **Conformance tests:** with `M = 3`, a set at P (flags 1) whose six probe neighbours each request one single update targeting an inert position: requests offered are 1 (around P) + 6 = 7; accepted are around-P and the first two singles, from P+W and P+E. Exactly one error naming the third single's target. Failing twin: the chain aborts and P+D, … are not updated (they are updated; only their requests are dropped).

Provenance: `source_suggested`. Confidence: high.

### SIM-NBR-005: World generation path

- **Inputs:** shape updates made while placing features during world generation.
- **Relevant state:** none.
- **Preconditions:** the change is made through the generation path, outside the server tick.
- **Behavior:** during world generation, shape updates run immediately and recursively, each hop decrementing the update limit by one, with no chain and no chain count. No live dimension uses this path.
- **Ordering constraints:** none.
- **Numeric semantics:** none.
- **Postconditions:** none.
- **Edge cases:** it matters only for feature placement.
- **Invariants:** none.
- **Conformance tests:** none beyond SIM-NBR-003.

Provenance: `source_suggested`. Confidence: high.

### SIM-NBR-006: Comparator output updates

- **Inputs:** position P, source block type B (the new block of SIM-SET-002 step 3.2, or a block calling it directly).
- **Relevant state:** the four horizontal neighbours of P and the blocks one further.
- **Preconditions:** none.
- **Behavior:** for d in horizontal order `N, E, S, W`: let Q = P + d. If Q's section is not *loaded*, skip d. If Q holds a comparator, request "single, with state" for Q with source B. Otherwise, if Q is a redstone conductor and Q + d holds a comparator, request "single, with state" for Q + d with source B.
- **Ordering constraints:** vanilla order `N, E, S, W`; observable result: each such comparator receives exactly one update.
- **Numeric semantics:** none.
- **Postconditions:** none.
- **Edge cases:**
  - Only Q is tested for being loaded. Q + d is read without a test, so if it lies in a section that is not *loaded*, the read loads it (SIM-SET-003).
  - This is the one place in the updater that tests loading.
- **Invariants:** at most four requests.
- **Vanilla grouping:** vanilla tests whether Q's column is loaded at full status, ignoring y. For a Q outside the dimension's vertical extent the test passes and the read returns `minecraft:void_air`. An exact-parity mode tests the column; a section-native host tests Q's section, which differs only when sections of one column differ in *loaded*.
- **Conformance tests:** a container at P with a comparator at P+N and a solid block at P+E with a comparator at P+E+E: changing the container's contents updates both comparators, P+N first.

Provenance: `source_suggested`. Confidence: high.

### SIM-NBR-007: Orientation under the redstone experiments feature

- **Inputs:** neighbour updates in a dimension whose enabled features include `minecraft:redstone_experiments`.
- **Relevant state:** the level random (SIM-RNG-001).
- **Preconditions:** the feature is enabled (`assets/minecraft/datapacks/redstone_experiments/pack.mcmeta`).
- **Behavior:** without the feature, every neighbour update carries no orientation. With it:
  - An orientation is a triple (up, front, side bias ∈ {left, right}) with up and front on different axes: 6 × 4 × 2 = 48 orientations, indexed `((up_ordinal << 2) + front_key) << 1 | side_bias_ordinal`, where direction ordinals are `D=0, U=1, N=2, S=3, W=4, E=5`, `front_key = (front_axis_key << 1) | front_axis_direction` (axis direction positive = 0, negative = 1), and `front_axis_key` is 1 when (up is vertical and front is on X) or (up is horizontal and front is vertical), else 0.
  - A *fresh orientation* is one of the 48 drawn uniformly from the level random, with its side bias then forced to left.
  - A set with *notify-neighbours* (SIM-SET-002 step 3.2) draws a fresh orientation when it makes its neighbours-around request.
  - A neighbours-around request made by a block with no orientation draws a fresh one when its first step runs; if the request skips a direction X, that orientation's front is set to the opposite of X.
  - Each step towards direction d carries the request's orientation with its front set to d.
- **Ordering constraints:** each draw consumes one value of the level random at the moment stated above.
- **Numeric semantics:** uniform integer in [0, 48).
- **Postconditions:** none.
- **Edge cases:** the neighbour order stays `W, E, D, U, N, S`; the orientation only informs the receiving block (the experimental redstone wire uses it). The wire evaluator itself is out of scope.
- **Invariants:** with the feature off, the level random is not consumed by neighbour updates.
- **Conformance tests:** a set at P with flags 1, surrounded by inert probes: with the feature off the level random is not advanced; with it on it is advanced by exactly one draw, and the six updates carry orientations whose fronts are W, E, D, U, N, S.

Provenance: `source_suggested`. Confidence: medium.

---

## 4. Shape updates

### SIM-SHP-001: Direct shape updates

- **Inputs:** a changed position P, its new state S, flags F', limit L' (from SIM-SET-002 step 3.3).
- **Relevant state:** the six neighbours of P.
- **Preconditions:** the emitting set had no *keep-shape* and its limit L > 0 (so L' = L − 1 ≥ 0).
- **Behavior:** for d in shape order `W, E, N, S, D, U`, one shape-update request (SIM-NBR-002) targets Q = P + d with direction = opposite of d (the direction from Q towards P), neighbour position P, neighbour state S, flags F', limit L'. When it runs:
  1. Q's current state C is read.
  2. If F' has *wire-skips-shape* and C is redstone wire, nothing happens.
  3. Otherwise C's shape function computes C' from (direction, P, S), with the level random available.
  4. If C' is the same state as C, nothing happens. If C' is air, Q is destroyed (SIM-SET-004) with drops unless F' has *no-drops*, with limit L'. Otherwise Q is set to C' with flags F' without *no-drops* and limit L'.
- **Ordering constraints:**
  - Observable result: every neighbour receives one shape update carrying S.
  - Vanilla order: `W, E, N, S, D, U`, cascading as in SIM-NBR-003.
- **Numeric semantics:** F' = F with bits 1 and 32 cleared (F & −34). L' = L − 1.
- **Postconditions:** none.
- **Edge cases:**
  - The shape function may schedule ticks (for example a falling block schedules its fall); scheduling is immediate (SIM-SCH-003), not queued.
  - Because F' has no *notify-neighbours*, a shape-driven change emits no neighbour updates, unless it is a destruction (SIM-SET-004 always uses flags 3).
  - *piston-move*, *wire-skips-shape*, *skip-block-entity-effects*, *skip-placement* and *sync-clients* propagate down the shape cascade.
  - A set reached through the shape cascade drops *no-drops* before it emits its own shape updates, so in a cascade started by an ordinary set no destruction suppresses drops.
- **Invariants:** a shape update never runs for a set with *keep-shape*.
- **Conformance tests:**
  - Probes on all six sides; a set at P with flags 2: six shape updates logged at P+W, P+E, P+N, P+S, P+D, P+U with directions E, W, S, N, U, D. Failing twin: neighbour order.
  - Torch on a block; the block is set to air with flags 3: the torch is destroyed with its item dropped. With flags 18 (*sync-clients*, *keep-shape*): the torch stays floating.

Provenance: `source_suggested`. Confidence: high.

### SIM-SHP-002: The update limit

- **Inputs:** the limit L of a set (512 unless the caller passes another).
- **Relevant state:** none.
- **Preconditions:** none.
- **Behavior:** a set with limit L emits shape updates only if L > 0, and they carry L − 1. A set made by a shape update (or a destroy made by one) uses the limit the shape update carried. Therefore, counting the original set as generation 0, the change at generation k is made with limit 512 − k, and the change at generation 512 emits no shape updates.
- **Ordering constraints:** none.
- **Numeric semantics:** default L = 512. The limit counts shape generations only; neighbour updates do not consume it, and a set made by a neighbour or tick handler starts again from its own default (normally 512).
- **Postconditions:** a shape cascade has at most 512 generations after the original set.
- **Edge cases:** the chain limit (SIM-NBR-004) applies to the same requests independently. The limit travels with the cascade across section boundaries; it is not reset or counted per section.
- **Invariants:** none.
- **Conformance tests:** a vertical stack of 600 probe blocks (spanning at least 38 sections), each turning into probe-changed when its below neighbour's shape changes, triggered by a set at the bottom: exactly 512 blocks above the bottom change. Failing twin: 513, the whole stack, or a stop at a section boundary.

Provenance: `source_suggested`. Confidence: high.

### SIM-SHP-003: Indirect shape updates

- **Inputs:** a set at P (SIM-SET-002 step 3.3).
- **Relevant state:** the old and new states at P.
- **Preconditions:** as SIM-SHP-001.
- **Behavior:** before the direct shape updates the **old** state's indirect shape updates run, and after them the **new** state's. Only `minecraft:redstone_wire` has any at this version; every other block has none. For a wire at P, for d in horizontal order `N, E, S, W` where the wire connects towards d (side not `none`) and P + d is not wire: if P + d + D is wire, a shape update targets it with direction opposite of d, neighbour position P + d + D + opposite(d) and that position's current state; then the same for P + d + U. Flags and limit as in SIM-SHP-001.
- **Ordering constraints:** old-indirect, then direct, then new-indirect.
- **Numeric semantics:** as SIM-SHP-001.
- **Postconditions:** none.
- **Edge cases:** the neighbour state passed is read when the request is made, not the wire's own state.
- **Invariants:** none.
- **Conformance tests:** placing a wire on a block next to a lower wire diagonally updates the lower wire's connection in the same chain.

Provenance: `source_suggested`. Confidence: medium.

### SIM-SHP-004: Shape update when the target changed meanwhile

- **Inputs:** a shape update queued during a chain whose target was modified before it ran.
- **Relevant state:** target's current state.
- **Preconditions:** a chain is running.
- **Behavior:** the shape function of the target's **current** state runs, with the neighbour state captured at request time (SIM-NBR-002).
- **Ordering constraints:** none.
- **Numeric semantics:** none.
- **Postconditions:** none.
- **Edge cases:** if the neighbour has changed again, the target's computation uses a stale neighbour state; vanilla does not correct it.
- **Invariants:** none.
- **Conformance tests:** inside a chain, set P to probe B (emitting shape updates carrying B), then before they run set P to probe C with *keep-shape*: the shape update at P+W is logged with neighbour state B.

Provenance: `source_suggested`. Confidence: high.

---

## 5. Scheduled ticks

### SIM-SCH-001: Fields of a scheduled tick

- **Inputs:** a scheduling request.
- **Relevant state:** none.
- **Preconditions:** none.
- **Behavior:** a scheduled tick holds:

| Field | Type | Meaning |
|-------|------|---------|
| type | block type (block ticks) or fluid type (fluid ticks: `minecraft:water` or `minecraft:lava`, the same for a source and a flowing state, 0.1) | what must be at the position when it runs |
| position | block position | where it runs; fixes its section |
| due tick | signed 64-bit game time | the game time at or after which it may run |
| priority | one of seven values (SIM-SCH-002) | order among ticks due together |
| sub-tick order | signed 64-bit | insertion sequence |

  The full key is (due tick, priority, sub-tick order). Block ticks and fluid ticks are two separate kinds with identical rules.
- **Ordering constraints:** none.
- **Numeric semantics:** see SIM-SCH-003.
- **Postconditions:** none.
- **Edge cases:** identity for de-duplication is (position, type) only (SIM-SCH-004). The position does not enter the full key; no order in this section depends on x, y, z, the section or the column except where a Vanilla grouping paragraph says so.
- **Invariants:** none.
- **Conformance tests:** covered by SIM-SCH-006.

Provenance: `source_suggested`. Confidence: high. Changed in 26.4-snapshot-4: a fluid tick's type no longer distinguishes a source from a flowing fluid.

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

  A lower value runs earlier. Decoding an integer outside −3..3 clamps: below −3 gives −3, above 3 gives 3.
- **Ordering constraints:** see SIM-SCH-006.
- **Numeric semantics:** stored as a 32-bit integer.
- **Postconditions:** none.
- **Edge cases:** at this version only redstone diodes (`minecraft:repeater`, `minecraft:comparator`) schedule ticks with a priority other than normal: −3, −2, −1 and 0 are used.
- **Invariants:** none.
- **Conformance tests:** decoding `p: -7` yields −3; `p: 9` yields 3; `p: 2` yields 2.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-003: Scheduling and the due tick

- **Inputs:** position P, type T, delay d (32-bit integer), optional priority (default normal).
- **Relevant state:** game time G at the moment of the call; the dimension's sub-tick counter `n`.
- **Preconditions:** none.
- **Behavior:** the tick is created with due tick `G + d`, the priority, and sub-tick order `n`; then `n` increases by one. It is then offered to the queue of P's section (SIM-SCH-005), which may reject it as a duplicate (SIM-SCH-004).
- **Ordering constraints:** sub-tick order is the order of scheduling calls in the dimension, shared by block and fluid ticks and by every section.
- **Numeric semantics:** `n` is a per-dimension 64-bit counter starting at 0 when the dimension is created at server start; it is not saved. It advances for every call, including calls whose tick is then rejected or dropped.
- **Postconditions:** an accepted tick is due at `G + d`.
- **Edge cases:** d ≤ 0 makes the tick due immediately; it still runs no earlier than the next scheduled-tick phase that collects it (SIM-SCH-009).
- **Invariants:** runtime ticks have sub-tick order ≥ 0 and no two runtime ticks of a dimension share one; ticks restored from a save have negative sub-tick order (SIM-SCH-013).
- **Conformance tests:** at G = 100, schedule (P, A, 3) then (Q, A, 3): both due at 103; P runs before Q (same priority, smaller sub-tick order), whether or not P and Q share a section.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-004: De-duplication

- **Inputs:** a tick offered to the queue of its section.
- **Relevant state:** the set of (position, type) pairs pending in that queue.
- **Preconditions:** none.
- **Behavior:** if a tick with the same position and the same type is pending, the new tick is dropped silently, whatever its due tick or priority. A tick stops being pending at the moment it is **collected** for running (SIM-SCH-006), not when it runs.
- **Ordering constraints:** none.
- **Numeric semantics:** none.
- **Postconditions:** at most one pending tick per (position, type) per kind.
- **Edge cases:**
  - An earlier due tick or higher priority does not replace the pending one.
  - Between collection and execution in the same phase, the same (position, type) can be scheduled again and is accepted; it is queued for a later collection. The query of SIM-SCH-010 lets blocks avoid this.
  - Block and fluid ticks are separate: a block tick and a fluid tick at the same position never collide. A fluid tick for a water source and one for flowing water at the same position have the same type `minecraft:water` and collide: the second is dropped. Water and lava ticks at one position are distinct.
  - Ticks restored from a save count as pending for de-duplication even before they are activated (SIM-SCH-013).
- **Invariants:** the pending pair set equals the set of pairs in the queue (plus not-yet-activated saved ticks).
- **Vanilla grouping:** vanilla de-duplicates per column. Because the position is part of the identity and fixes the section, de-duplicating per section is the same rule.
- **Conformance tests:** schedule (P, A, delay 10, normal) then (P, A, delay 2, extremely high): one tick, due at G + 10. Failing twin: it runs at G + 2. Schedule (P, A, 5) and (P, B, 5): both run. With flowing water at P, schedule a `minecraft:water` fluid tick with delay 5; set P to a water source; schedule a `minecraft:water` fluid tick with delay 1: one tick, due at G + 5. Failing twin: two ticks (source and flowing treated as two types, as before 26.4-snapshot-4).

Provenance: `source_suggested`. Confidence: high. Changed in 26.4-snapshot-4: a source and a flowing state of one fluid share one type, so their fluid ticks at one position de-duplicate against each other; before, water and flowing water were separate types and both could be pending.

### SIM-SCH-005: Per-section queues and eligibility

- **Inputs:** a tick offered at position P.
- **Relevant state:** P's section properties (SIM-TICK-005).
- **Preconditions:** none.
- **Behavior:** each section that *accepts ticks* owns one block-tick queue and one fluid-tick queue, ordered by full key. A tick offered for a position whose section does not *accept ticks* (not loaded, or still generating) is dropped with an error log "Trying to schedule tick in not loaded position". A queue's ticks may be collected only while its section *runs scheduled ticks*; while it does not, they stay and become overdue.
- **Ordering constraints:** see SIM-SCH-006.
- **Numeric semantics:** none.
- **Postconditions:** none.
- **Edge cases:** a section loaded by a set at an unloaded position (SIM-SET-003) accepts ticks but does not run them.
- **Invariants:** a tick belongs to the section containing its position.
- **Vanilla grouping:** vanilla keeps one queue per kind per column, ordered by full key. SIM-SCH-006 states how the column queue is recovered from section queues.
- **Conformance tests:** a tick scheduled into a section that accepts but does not run ticks at G with delay 1, where the section starts running ticks at G + 50: it runs in the scheduled phase of G + 50 (overdue), not at G + 51.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-006: Selection and run order within one phase

- **Inputs:** current tick `t` (game time after this tick's advance), budget `B = 65536`, the queues of one kind (block or fluid) of every section.
- **Relevant state:** the head (smallest tick by full key) of each section queue.
- **Preconditions:** the tick runs normally and the world is not a debug world.
- **Behavior:**
  - **Observable result (section-native).** The ticks that run are drawn only from ticks with due ≤ `t` in sections that *run scheduled ticks*; at most `B` run; within one section they run in full-key order; every tick runs at most once per phase. When more than `B` are due, the ticks left over run in a later phase.
  - **Recommended semantic policy.** Run the collected ticks in global full-key order across all sections. It satisfies the observable result, keeps full-key order inside every column as well, and coincides with the vanilla order whenever no collected tick is overdue (see Invariants).
  - **Vanilla order (exact).** Collection, then execution, over *column queues*:
    1. *Column queue.* For each column `(sx, sz)`, its column queue is the merge of the queues of all its sections by full key. Vanilla keeps one queue per column ordered by full key alone; the position, its height and its section do not enter the order, so this merge reproduces vanilla's column queue tick for tick. It need not be materialised: the column's head is the smallest of its sections' heads, and taking the head takes it from the section that holds it.
    2. *Candidates.* Every column whose sections *run scheduled ticks* (vanilla gives all sections of a column the same answer) and whose column-queue head has due ≤ `t` is a candidate. A column with no ticks, or whose head is not yet due, is not.
    3. *Merge.* Keep the candidates in a priority set keyed by their current head's (priority, sub-tick order) only — **not** its due tick. Repeat while fewer than `B` ticks are collected and a candidate remains: remove the candidate column with the smallest key; collect its head; let K be the key of the best remaining candidate, read once now (none if the set is empty); then keep collecting from the same column while fewer than `B` are collected, its new head is due (≤ `t`), and its new head's (priority, sub-tick order) is not greater than K. If the column then still has a due head and the budget is not reached, it re-enters the candidate set keyed by that head.
    4. *Run.* Run the collected ticks in collection order (SIM-SCH-007).
  - Within one column, collection follows the column-queue order (due first), so an overdue tick precedes a tick due now even if the latter has higher priority; across columns, the comparison ignores the due tick.
- **Ordering constraints:** as above. Collection completes before any collected tick runs.
- **Numeric semantics:** `B = 65536` per kind per dimension per level tick.
- **Postconditions:** collected ticks are no longer pending (SIM-SCH-004).
- **Edge cases:**
  - Two ticks with equal (priority, sub-tick order) are possible only for ticks restored from saves (SIM-SCH-013) and for copies (SIM-SCH-016). Between candidate columns, vanilla's choice is unspecified (OQ-01). Inside one column, restored ticks cannot tie with each other or with runtime ticks; a copy can tie with a later runtime tick, and vanilla's order between them is then unspecified.
  - Ticks scheduled while the collected ticks run are not collected in this phase (SIM-SCH-009).
  - Grouping sections into a candidate per section instead of per column is not vanilla: it lets a later section's head overtake an earlier overdue head of the same column (see the second table).
- **Invariants:**
  - A tick is collected at most once.
  - (`derived`) If every collected tick has due = `t`, the vanilla order is the global ascending (priority, sub-tick order) order of those ticks across all sections, because each column queue is then sorted by (priority, sub-tick order) and step 3 is an exact k-way merge on that key.
- **Conformance tests:** (vanilla policy; `t` = 11; all priority normal)

| Column | Section | Ticks (due, sub-tick order) |
|--------|---------|-----------------------------|
| A = (0, 0) | (0, 0, 0) | a2 (11, 1) |
| A = (0, 0) | (0, 5, 0) | a1 (10, 5) |
| B = (1, 0) | (1, 0, 0) | b1 (11, 3) |

  Expected order: b1, a1, a2. Failing twins: a1, a2, b1 (global full-key sort); a2, b1, a1 (global sort by priority and sub-tick order, or one candidate per section). Under a semantic policy any order in which a1 precedes a2 is acceptable only if a1 and a2 are in one section; here they are in two sections, so every order of the three is acceptable, and the recommended policy gives a1, a2, b1.
  - Single section, `t` = 11: x (due 10, normal, sub 9) and y (due 11, extremely high, sub 10): x runs before y.
  - 70 000 ticks due at `t` in one section: 65 536 run in this phase, the remaining 4 464 in the next, in sub-tick order.

Provenance: `source_suggested` for the vanilla algorithm and the column-queue claim (the column queue's order is the full key and nothing else); `derived` for the invariant. Confidence: high.

### SIM-SCH-007: Running one tick

- **Inputs:** a collected tick (P, T).
- **Relevant state:** the state at P when it runs.
- **Preconditions:** collected in this phase.
- **Behavior:** block tick: if the block state at P is of block type T, its scheduled-tick handler runs with the level random (SIM-RNG-001); otherwise nothing happens. Fluid tick: if the fluid state of the block at P is of fluid type T, whether a source or flowing state, the fluid's tick handler runs; otherwise nothing happens.
- **Ordering constraints:** collection order.
- **Numeric semantics:** none.
- **Postconditions:** the tick is consumed in both cases.
- **Edge cases:** a tick whose block was replaced by another block type is silently consumed; a later replacement back does not revive it. A state change within the same block type (for example a property change) does not prevent the tick. Likewise a fluid tick survives its position turning from flowing into a source of the same fluid, or back, and runs the handler of the current fluid state.
- **Invariants:** none.
- **Conformance tests:** schedule (P, probe A, 2); at G + 1 replace P with probe B: at G + 2 neither A's nor B's tick handler is logged. Schedule a `minecraft:water` fluid tick at P holding flowing water with delay 2; at G + 1 set P to a water source with flags 530 (no neighbour or shape updates, no placement side effect, so nothing schedules a second tick): at G + 2 the source's fluid tick handler runs. Failing twin: the tick is consumed without running (the behaviour before 26.4-snapshot-4, when it was keyed to the flowing type).

Provenance: `source_suggested`. Confidence: high. Changed in 26.4-snapshot-4: the type check matches any state of the fluid, so a source/flowing change at P no longer consumes a pending fluid tick silently.

### SIM-SCH-008: Budget and fairness

- **Inputs:** more than `B` due ticks of one kind.
- **Relevant state:** as SIM-SCH-006.
- **Preconditions:** none.
- **Behavior:** the excess remains pending and is due; it is considered first in the next phase by the same rule. Block and fluid budgets are independent. The budget is per dimension, not per section.
- **Ordering constraints:** as SIM-SCH-006.
- **Numeric semantics:** `B = 65536`.
- **Postconditions:** none.
- **Edge cases:** a tick deferred by the budget keeps its due tick, so in its own section (and, in vanilla, its own column) it stays ahead of ticks due later.
- **Invariants:** none.
- **Conformance tests:** see SIM-SCH-006, last case.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-009: Ticks scheduled during a phase

- **Inputs:** a tick scheduled while collected ticks run, or in a later phase of the same level tick.
- **Relevant state:** game time `t`.
- **Preconditions:** none.
- **Behavior:** a block tick scheduled during the block-tick phase with delay ≤ 0 is due at ≤ `t` but runs in the next level tick's block phase. A **fluid** tick scheduled during the block-tick phase with delay 0 is collected by the fluid phase of the **same** level tick, because fluid collection happens after all block ticks ran. A tick scheduled during the fluid phase, random ticks, block events, entities or block entities with delay ≤ 0 runs in the next level tick. A tick scheduled with delay 1 at any point of the level tick of `t` runs in the level tick of `t + 1`.
- **Ordering constraints:** collection happens once per phase, before execution.
- **Numeric semantics:** none.
- **Postconditions:** none.
- **Edge cases:** a block tick handler may reschedule its own (P, T) with any delay; it is accepted because the running tick is no longer pending.
- **Invariants:** no tick runs twice in one phase.
- **Conformance tests:**
  - A probe block tick at `t` schedules a block tick at P with delay 0 and a fluid tick with delay 0 for a probe fluid at Q: the fluid tick runs at `t`, the block tick at `t + 1`. Failing twin: the block tick also at `t`.
  - A probe block tick at `t` reschedules itself with delay 0: its handler runs once per level tick, not repeatedly within one.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-010: Queries

- **Inputs:** (P, T).
- **Relevant state:** the queue of P's section; the collected list of the running phase.
- **Preconditions:** none.
- **Behavior:**
  - *Has scheduled tick*: true iff a (P, T) tick is pending in the queue of P's section (including not-yet-activated saved ticks). False if P's section does not *accept ticks*. A collected tick is not pending.
  - *Will tick this tick*: true iff, in the phase currently running for that kind, a (P, T) tick was collected and has not yet been run. False outside that kind's phase.
- **Ordering constraints:** none.
- **Numeric semantics:** none.
- **Postconditions:** queries have no side effects.
- **Edge cases:** during the fluid phase, "will tick this tick" for a block tick is false. Repeaters and comparators use "will tick this tick" so as not to schedule a second tick for themselves while one is already about to run.
- **Invariants:** for one (P, T) at any instant, "has scheduled tick" and "will tick this tick" can both be true only if the pair was scheduled again after collection.
- **Conformance tests:** two probe ticks at P and Q due at `t`, with P's sub-tick order smaller. During P's handler: "will tick this tick (Q)" is true and "has scheduled tick (Q)" is false; during Q's handler, "will tick this tick (P)" is false.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-011: Block and fluid interleaving

- **Inputs:** due block and fluid ticks in one level tick.
- **Relevant state:** none.
- **Preconditions:** none.
- **Behavior:** all collected block ticks run, then fluid ticks are collected and all of them run. They never interleave. The sub-tick counter is shared, but the two kinds are ordered independently.
- **Ordering constraints:** observable and vanilla: block phase strictly before fluid phase.
- **Numeric semantics:** separate budgets of 65 536.
- **Postconditions:** none.
- **Edge cases:** see SIM-SCH-009 for fluid ticks scheduled by block ticks.
- **Invariants:** none.
- **Conformance tests:** see SIM-TICK-002.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-012: Save format

- **Inputs:** the ticks of the sections of one column being saved at game time `G`.
- **Relevant state:** those sections' queues and any saved ticks not yet activated.
- **Preconditions:** none.
- **Behavior:** the save record of a column (its compound in the region file) holds two lists, `block_ticks` and `fluid_ticks`, covering every section of the column. Each element is a compound:

| Key | NBT type | Content |
|-----|----------|---------|
| `i` | string | the block id (in `block_ticks`) or fluid id (in `fluid_ticks`: `minecraft:water` or `minecraft:lava`), for example `minecraft:repeater`, `minecraft:water` |
| `x` | int | position x |
| `y` | int | position y |
| `z` | int | position z |
| `t` | int | delay: due tick − `G` |
| `p` | int | priority value (SIM-SCH-002) |

  Element order: first the not-yet-activated saved ticks of the column in the order they were loaded, with their stored delays unchanged; then the active ticks of all sections of the column merged by sub-tick order ascending. Sub-tick order itself is not stored. For a column still generating (not full), a tick scheduled during generation is stored with `t = 0`, a tick loaded into it from an earlier save keeps its stored `t`, and the list keeps insertion order (SIM-SCH-014).
- **Ordering constraints:** the list order encodes the relative insertion order (SIM-SCH-013).
- **Numeric semantics:** `t` = due − `G`, truncated to 32 bits. It is negative for an overdue tick.
- **Postconditions:** loading the column and activating it at game time `G'` restores due ticks `G' + t`.
- **Edge cases:** a column saved while its sections did not run ticks carries their overdue ticks with negative `t`. Both lists are written even when empty (OQ-07).
- **Invariants:** every element's position lies in the column.
- **Vanilla grouping:** the save record is per column; that is the layout of the save format, not of the simulation. A section-native store produces it by concatenating the column's unactivated saved list with the merge, by sub-tick order, of the active ticks of the column's sections. Where two ticks of the column share a sub-tick order (copies, SIM-SCH-016), vanilla's relative order is unspecified.
- **Conformance tests:** with ticks frozen at game time G, a player places a redstone block next to an unpowered repeater (delay setting 1, plain line): after `/save-all flush`, the column's `block_ticks` contains `{i: "minecraft:repeater", x, y, z, t: 2, p: -1}`. Failing twins: `t: G+2` (absolute), or `p: 0`.

Provenance: `source_suggested`. Confidence: high for layout; medium for the repeater example (OQ-07). Changed in 26.4-snapshot-4: `fluid_ticks` ids are only `minecraft:water` and `minecraft:lava`; a flowing fluid's tick is written under its fluid's id. The layout is otherwise unchanged.

### SIM-SCH-013: Load and activation

- **Inputs:** a column save record with `block_ticks` / `fluid_ticks`.
- **Relevant state:** game time when the column's sections start to *broadcast changes* (vanilla: the column reaches block-ticking status).
- **Preconditions:** none.
- **Behavior:**
  1. *Decode.* Each element is decoded independently. An element with an unknown id, a missing field or a wrongly typed field is dropped with an error log; the others are kept. An element whose position is outside this column is dropped silently. A missing list is an empty list.
  2. *Pending.* When the column's sections become *loaded*, the decoded list becomes their pending saved ticks, each in the section holding its position. They count for de-duplication (SIM-SCH-004) and for "has scheduled tick", but do not run.
  3. *Activation.* When the sections start to *broadcast changes*, at game time `G'`, after post-processing (SIM-SCH-017), the k-th of the `m` saved ticks of the column's list (k = 0..m−1, list order) becomes a tick with due `G' + t`, its priority, and sub-tick order `k − m` (so −m … −1). They are added without de-duplication. The saved list is then discarded.
- **Ordering constraints:** restored ticks precede every runtime tick with the same due tick and priority in the same column, and keep their saved relative order.
- **Numeric semantics:** sub-tick orders −m … −1, numbered over one column's list, not per section.
- **Postconditions:** time spent unloaded does not count towards a tick's delay; time spent loaded but not running ticks does (SIM-SCH-005).
- **Edge cases:**
  - Two identical (position, type) elements in one list both activate and both run (OQ-08).
  - A tick scheduled at runtime between steps 2 and 3 for a pair not in the saved list is accepted and kept with its runtime sub-tick order.
  - Restored ticks of two different columns can have equal sub-tick orders (OQ-01).
  - A column saved while still generating carries `t = 0`; its ticks are due at the first scheduled phase after activation.
  - In a record of the current world version, a `fluid_ticks` element naming `minecraft:flowing_water` or `minecraft:flowing_lava` is an unknown id and is dropped with an error log by step 1. Vanilla never sees one there: it rewrites those names to `minecraft:water` and `minecraft:lava` while upgrading a record written before world version 5123 (including the `UpgradeData` neighbour tick lists). This project upgrades nothing; a record of another world version is rejected before decoding.
- **Invariants:** the activated ticks are exactly the decoded, in-column elements.
- **Vanilla grouping:** load and activation happen per column, and the restored sub-tick numbering runs over the column's whole list. A section-native store keeps, for each saved tick, its index in the column list (or keeps the list whole until activation) and assigns `k − m` from it, then places each tick in its section's queue. Numbering per section instead would change the order of restored ticks across sections of one column and is not vanilla.
- **Conformance tests:**
  - A column saved with `t: 3` for a repeater, loaded later and activated at `G'`: the repeater ticks in the level tick of `G' + 3`. Failing twin: immediately, or at the stored absolute time.
  - `block_ticks: [{i: "minecraft:no_such_block", …}, {i: "minecraft:repeater", …, t: 1, p: 0}]`: the repeater tick runs; one error is logged. Failing twin: both dropped (OQ-02).
  - A column list `[u (section sy=4, t: 0), v (section sy=0, t: 0)]`, both priority normal, activated at `G'`: in the phase of `G'`, u runs before v (u gets −2, v −1). Failing twin: v first (numbering per section, or by height).

Provenance: `source_suggested`. Confidence: medium (OQ-02). Changed in 26.4-snapshot-4: the flowing fluid ids are gone, so they decode as unknown ids; vanilla renames them when upgrading saves from before world version 5123. Decoding and activation are otherwise unchanged.

### SIM-SCH-014: Ticks during generation

- **Inputs:** ticks scheduled while a column is generated (feature placement and other generation steps before full status).
- **Relevant state:** the generating column's tick lists.
- **Preconditions:** the column is not yet full.
- **Behavior:** the tick is stored as (type, position, delay 0, priority), de-duplicated by (position, type); due tick and sub-tick order are discarded. "Will tick this tick" is always false during generation. When the column becomes full the list becomes pending saved ticks (SIM-SCH-013).
- **Ordering constraints:** insertion order is kept as list order.
- **Numeric semantics:** delay 0.
- **Postconditions:** a block scheduled during generation ticks at the first scheduled phase after its section starts to run ticks.
- **Edge cases:** the requested delay is lost.
- **Invariants:** none.
- **Vanilla grouping:** generation runs per column, so the list and its insertion order are per column, and that order becomes the activation numbering of SIM-SCH-013.
- **Conformance tests:** world-generated water with a pending fluid tick saved before full status carries `t: 0`.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-015: Unload and sections that do not run ticks

- **Inputs:** a section stopping being *loaded*, or staying *loaded* without *running scheduled ticks*.
- **Relevant state:** its queues.
- **Preconditions:** none.
- **Behavior:** on unload, the section's queues stop being consulted and are saved (SIM-SCH-012), relative to game time at save. A section that is *loaded* but does not run ticks keeps its queues in place; game time keeps advancing, so their ticks become overdue and run, up to the budget, at the first phase in which the section runs ticks again.
- **Ordering constraints:** unloading happens in step 9 of SIM-TICK-002, after this tick's scheduled ticks.
- **Numeric semantics:** none.
- **Postconditions:** none.
- **Edge cases:** overdue ticks that run late run in queue order (due first), not compressed or dropped.
- **Invariants:** none.
- **Vanilla grouping:** vanilla unloads and saves whole columns.
- **Conformance tests:** see SIM-SCH-005 and SIM-SCH-013.

Provenance: `source_suggested`. Confidence: high.

### SIM-SCH-016: Area clear and copy (short form)

Structure placement by structure blocks and the test framework can clear all ticks
whose position lies in a box (from the queues of the sections the box touches and
from the current phase's collected and already-run lists) and copy them with an
offset. A copied tick keeps due tick and priority; its sub-tick order becomes
`s − min + max + 1`, where `min` and `max` are the smallest and largest sub-tick
orders among the copied ticks, so copies sort after the originals. Copies go
through de-duplication (SIM-SCH-004). The copied sub-tick orders do not advance
the dimension's counter, so a later runtime tick can receive the same sub-tick
order as a copy. Not used by the ordinary tick.

Provenance: `source_suggested`. Confidence: medium.

### SIM-SCH-017: Post-processing at the start of broadcasting

- **Inputs:** a section that starts to *broadcast changes* (vanilla: its column reaches block-ticking status) and carries post-processing marks left by generation.
- **Relevant state:** the section's post-processing marks: a list of positions inside the section.
- **Preconditions:** none.
- **Behavior:** for each marked position, in list order: if its block state has a non-empty fluid, the fluid's tick handler runs at once (not scheduled); then, if the block is a fluid block, the block's scheduled-tick handler runs at once with the level random; otherwise the state is recomputed from its six neighbours' shapes and, if it changed, set with flags 276 (*no-rerender*, *keep-shape*, *skip-block-entity-effects*: no neighbour updates, no shape updates, no client sync). The marks are then cleared. Ticks scheduled here take the runtime path (SIM-SCH-003) and are de-duplicated against the pending saved ticks.
- **Ordering constraints:** post-processing completes before saved ticks activate (SIM-SCH-013 step 3).
- **Numeric semantics:** flags 276.
- **Postconditions:** the section has no post-processing marks.
- **Edge cases:** a runtime tick created here has a non-negative sub-tick order, so it runs after any restored tick with the same due tick and priority in its column.
- **Invariants:** none.
- **Vanilla grouping:** vanilla stores the marks per section within the column and processes the column's sections from the bottom up when the column reaches block-ticking status.
- **Conformance tests:** none yet; see OQ-09.

Provenance: `source_suggested`. Confidence: medium.

---

## 6. Block events

### SIM-BEV-001: Fields and queueing

- **Inputs:** a block event request: position P, block type B, two 32-bit integers `a` and `b`.
- **Relevant state:** the dimension's pending block events, an insertion-ordered set.
- **Preconditions:** none.
- **Behavior:** the event (P, B, a, b) is appended to the pending set unless an equal event (all four fields equal) is already pending, in which case it is dropped and the pending one keeps its place.
- **Ordering constraints:** first in, first out.
- **Numeric semantics:** `a` and `b` are 32-bit integers on the server; only their low 8 bits reach the client (SIM-BEV-004).
- **Postconditions:** none.
- **Edge cases:** events differing only in `a` or `b` are distinct and both delivered. There is one queue per dimension; there is no queue per section or per column, and the FIFO order spans all sections.
- **Invariants:** no two equal events are pending at once.
- **Conformance tests:** queue (P, A, 1, 0), (Q, A, 0, 0), (P, A, 1, 0), (P, A, 2, 0): delivered (P,1,0), (Q,0,0), (P,2,0). Failing twin: (P,1,0) twice.

Provenance: `source_suggested`. Confidence: high.

### SIM-BEV-002: The drain

- **Inputs:** step 10 of a level tick that runs normally (SIM-TICK-002).
- **Relevant state:** the pending set.
- **Preconditions:** none.
- **Behavior:** repeat until the pending set is empty: remove its first event; if its position *delivers block events* (SIM-BEV-003), deliver it (SIM-BEV-004); otherwise put it on a deferred list. Events queued while draining are appended to the pending set and drained in the same phase. After the loop, the deferred events become the pending set for the next tick, in deferral order (removing duplicates).
- **Ordering constraints:** observable and vanilla: strict FIFO over the pending set, including events added during the drain.
- **Numeric semantics:** there is no bound on the number of events per drain (OQ-05).
- **Postconditions:** after the phase, the pending set holds only deferred events, followed later in the tick by events queued by entities and block entities.
- **Edge cases:**
  - An event equal to one already removed from the set in this drain can be queued again and delivered again in the same drain.
  - Events queued in steps 11–12 or in the next tick's earlier phases are delivered at the next drain, after the deferred ones.
  - Pistons: a piston's event moves blocks, whose neighbour updates make other pistons queue events, which run in the same drain.
- **Invariants:** at the end of a drain no pending event's position *delivers block events*.
- **Conformance tests:** probe A's event handler at P queues (Q, A, 0, 0) once: both P's and Q's events are delivered in the same level tick, P first. Failing twin: Q's next tick.

Provenance: `source_suggested`. Confidence: high.

### SIM-BEV-003: Eligibility and deferral

- **Inputs:** an event at P.
- **Relevant state:** whether P's section *delivers block events* (SIM-TICK-005).
- **Preconditions:** none.
- **Behavior:** an event is delivered iff P's section *delivers block events*. Other events are deferred in the dimension queue, without limit, until it does.
- **Ordering constraints:** deferred events keep their relative order.
- **Numeric semantics:** vanilla threshold: simulation level ≤ 32.
- **Postconditions:** none.
- **Edge cases:** deferral does not check that the block is still there; that is checked at delivery. An event deferred while its section is unloaded survives in memory and is delivered (if the block still matches) when the section returns within simulation range; it is not saved.
- **Invariants:** block events are never written to the save.
- **Vanilla grouping:** vanilla tests the simulation level of P's column; all sections of a column agree.
- **Conformance tests:** queue an event in a section outside simulation distance; it is not delivered; bring a player close: delivered in the first drain after the section reaches simulation level ≤ 32.

Provenance: `source_suggested`. Confidence: high for eligibility; medium for "not saved" (no save path for block events was found).

### SIM-BEV-004: Delivery and the packet

- **Inputs:** a deliverable event (P, B, a, b).
- **Relevant state:** the state at P; players of the dimension.
- **Preconditions:** none.
- **Behavior:** if the state at P is of block type B, its event handler runs with (a, b) and returns delivered or not. If delivered, a `block_event` packet (P, a, b, B) is sent to every player in the same dimension whose squared distance from the point (P.x, P.y, P.z) — the block's minimum corner, not its centre — to the player's position is **less than** 64² = 4096. If the block at P is no longer of type B, the event is dropped silently.
- **Ordering constraints:** the packet is sent immediately, during the drain, so it precedes the client sync of any block change the event caused (SIM-SET-005).
- **Numeric semantics:** packet layout: position (packed block position), `a` as one unsigned byte, `b` as one unsigned byte, block as its registry id (VarInt). Range 64.0, strict inequality. The recipients are chosen by distance, not by section or column tracking.
- **Postconditions:** none.
- **Edge cases:** a handler returning "not delivered" sends nothing. A property change within the same block type does not drop the event.
- **Invariants:** none.
- **Conformance tests:** a note block played with a player at distance exactly 64.0 from the block's corner receives no packet; at 63.99 it does. A chest opened then broken in the same tick before the drain: no `block_event` for it.

Provenance: `source_suggested`; packet id `black_box_confirmed` by `assets/mcrs/reports/packets.json`. Confidence: high.

---

## 7. Random ticks

### SIM-RND-001: Section selection

- **Inputs:** step 9 of a level tick that runs normally.
- **Relevant state:** the game rule `random_tick_speed` (integer ≥ 0, default 3); section properties (SIM-TICK-005).
- **Preconditions:** not a debug world.
- **Behavior:** every *loaded* section that is *random-ticked* receives its picks (SIM-RND-002) once per level tick. Separately, precipitation picks run per column (SIM-RND-004).
- **Ordering constraints:** observable: each random-ticked section is processed exactly once per level tick, and each section's picks run without interleaving with another section's. Vanilla: see Vanilla grouping.
- **Numeric semantics:** `random_tick_speed` default 3, minimum 0.
- **Postconditions:** none.
- **Edge cases:** with `random_tick_speed = 0` there are no precipitation picks and no random ticks.
- **Invariants:** none.
- **Vanilla grouping:** vanilla visits columns, not sections. For each column that is *random-ticked*, completely before the next: its precipitation picks (SIM-RND-004), then each of its sections from the lowest to the highest within the dimension's vertical extent. The column order is the iteration order of an internal hash table of simulation levels, deterministic only given the table's full history (OQ-04). Because the position generator and the level random are unseeded (SIM-RNG-001), no exact-parity mode can reproduce vanilla's picks; parity is by distribution only, and the grouping need not be reproduced.
- **Conformance tests:** with `random_tick_speed = 0`, a field of saplings never grows; with the default it does.

Provenance: `source_suggested`; the game rule's existence is `black_box_confirmed` by `assets/mcrs/reports/registries.json` (`minecraft:game_rule`, entry `minecraft:random_tick_speed`). Confidence: high.

### SIM-RND-002: Picks within one section

- **Inputs:** a section at `(sx, sy, sz)`, `k = random_tick_speed`.
- **Relevant state:** the level random; the position generator (SIM-RND-003); the section's counts of randomly ticking block and fluid states.
- **Preconditions:** SIM-RND-001.
- **Behavior:** if k > 0 and the section contains at least one randomly ticking block state or randomly ticking fluid state (checked once, before its picks), k times: draw a position from the generator with base `(16·sx, 16·sy, 16·sz)` and y mask 15; read the block state there from the section; if that block state is randomly ticking, run its random tick handler with the level random; then, if the fluid state **of the block state read before the handler ran** is randomly ticking, run the fluid's random tick handler with the level random.
- **Ordering constraints:** vanilla: picks in generator order. Observable: k independent uniform picks per eligible section per tick; distributions only.
- **Numeric semantics:** each pick is uniform over the 4096 positions of the section (SIM-RND-003). The same position may be picked twice in one tick.
- **Postconditions:** none.
- **Edge cases:**
  - A section's eligibility is evaluated before its first pick and not re-evaluated during its picks. "Randomly ticking" is a per-state property fixed by the block definition. The rule is the same for every section, whatever its height.
  - A fluid block whose fluid ticks randomly (lava) is a randomly ticking block whose random tick handler runs its fluid's random tick handler; the second step of the pick then runs the fluid's handler again. Such a position receives two fluid random ticks per pick (`derived` from the two steps; `source_suggested`, confidence high).
- **Invariants:** a section with no randomly ticking content consumes no generator values.
- **Conformance tests:** over 10 000 ticks with k = 3, a section holding one randomly ticking probe receives a mean of 3 × 10 000 / 4096 ≈ 7.32 random ticks (binomial tolerance). Failing twin: ≈ 117 (3 × 10 000 / 256, the y offset not randomised).

Provenance: `source_suggested`. Confidence: high.

### SIM-RND-003: The position generator

- **Inputs:** base (x0, y0, z0), y mask m.
- **Relevant state:** a per-dimension signed 32-bit integer `r`, initialised from an unseeded random at dimension creation.
- **Preconditions:** none.
- **Behavior:** `r := r × 3 + 1013904223` (32-bit wrapping); `v := r >> 2` (arithmetic shift); result `(x0 + (v & 15), y0 + ((v >> 16) & m), z0 + ((v >> 8) & 15))`.
- **Ordering constraints:** one advance per draw, shared by precipitation picks, random-tick picks and thunder target picks of the dimension.
- **Numeric semantics:** as stated; all arithmetic on 32-bit two's complement.
- **Postconditions:** none.
- **Edge cases:** the initial value is not saved and not derived from the world seed, so vanilla random tick positions are not reproducible across runs; only distributions are.
- **Invariants:** none.
- **Conformance tests:** from `r = 0`, with base (0, 0, 0) and mask 15:

| Draw | `r` after | `v` | offset (x, y, z) |
|------|-----------|-----|------------------|
| 1 | 1013904223 | 253476055 | (7, 11, 12) |
| 2 | −239350404 | −59837601 | (15, 14, 3) |

  Failing twin: a logical (unsigned) shift on draw 2 gives a different `v` and different offsets.

Provenance: `source_suggested`; table recomputed from the formula. Confidence: high.

### SIM-RND-004: Precipitation picks (column-grouped)

- **Inputs:** a column `(sx, sz)` processed by SIM-RND-001, `k = random_tick_speed`.
- **Relevant state:** the level random; the position generator; the column's motion-blocking heightmap; weather; the biome at the surface.
- **Preconditions:** the column is *random-ticked*.
- **Behavior:** k times: draw a uniform integer in [0, 48) from the level random; if it is 0, draw a position from the generator with base `(16·sx, 0, 16·sz)` and y mask 15, and run precipitation at its (x, z): the y is discarded and replaced by the column's motion-blocking surface. Precipitation may turn water below the surface to ice, and while raining may add snow (bounded by the game rule `max_snow_accumulation_height`) and let the block below the surface react to rain or snow. What precipitation does to individual blocks is out of scope.
- **Ordering constraints:** vanilla: a column's precipitation picks run before its sections' random-tick picks.
- **Numeric semantics:** gate 1 in 48 per pick; k gates per column per level tick.
- **Postconditions:** none.
- **Edge cases:** the gate draws consume the level random even when no pick follows.
- **Invariants:** none.
- **Vanilla grouping:** inherently per column, because it reads the column's heightmap and its rate is per `(x, z)`, not per section. A section-native host runs it once per random-ticked column (the xz index over its sections) at that rate; a host without heightmaps must still find the same surface y for the observable result to match.
- **Conformance tests:** with `random_tick_speed = 0`, no snow accumulates during rain in a snowy biome; with the default it does.

Provenance: `source_suggested`. Confidence: high.

---

## 8. Random sources

### SIM-RNG-001: Which random each mechanism uses

- **Inputs:** none.
- **Relevant state:** per dimension: the level random (a 48-bit linear congruential generator seeded from a unique, unsaved seed at dimension creation) and the position generator (SIM-RND-003).
- **Preconditions:** none.
- **Behavior:**

| Consumer | Source |
|----------|--------|
| scheduled block tick handlers | level random |
| scheduled fluid tick handlers | level random (through the level, when they draw) |
| shape functions (SIM-SHP-001) | level random |
| random tick handlers (block and fluid) | level random |
| precipitation gate, one draw in [0, 48) per pick | level random |
| random tick and precipitation positions | position generator |
| orientation draws under redstone experiments (SIM-NBR-007) | level random |
| neighbour-changed and block event handlers | whatever the handler draws from: the level random |
| shuffle of the spawning area (before random ticks) | level random |

  Neither source is derived from the world seed or saved, so vanilla's random sequence is not reproducible; parity is required only for distributions.
- **Ordering constraints:** every consumer of one dimension shares one level random, so its sequence depends on the exact order of all draws in the dimension.
- **Numeric semantics:** none beyond the generators.
- **Postconditions:** none.
- **Edge cases:** none.
- **Invariants:** no block-simulation consumer uses a per-position or per-section random at this version.
- **Conformance tests:** none against vanilla (not reproducible). A kernel that carries its own random state must give the same result from the same state and inputs.

Provenance: `source_suggested`. Confidence: high.

---

## 9. Numeric constants

| Constant | Value | Where | Provenance |
|----------|-------|-------|------------|
| Section edge | 16 blocks; section position = position >> 4 on each axis | 0.2 | `source_suggested` |
| Scheduled tick budget per kind per dimension per level tick | 65 536 | SIM-SCH-006 | `source_suggested` |
| Default update limit (shape generations) | 512 | SIM-SHP-002 | `source_suggested` |
| Default chain limit `max-chained-neighbor-updates` | 1 000 000 (negative = unlimited) | SIM-NBR-004 | `source_suggested` |
| Update flag bits | 1, 2, 4, 8, 16, 32, 64, 128, 256, 512 | SIM-SET-001 | `source_suggested` |
| Flag combinations | 3, 11, 260, 816 | SIM-SET-001 | `source_suggested` |
| Shape-update flag mask | F & −34 (clears 1 and 32) | SIM-SHP-001 | `source_suggested` |
| Non-air shape-result flag mask | F & −33 (clears 32) | SIM-SHP-001 | `source_suggested` |
| Post-processing set flags | 276 | SIM-SCH-017 | `source_suggested` |
| Priority values | −3 … 3 | SIM-SCH-002 | `source_suggested` |
| Initial sub-tick counter | 0, per dimension, not saved | SIM-SCH-003 | `source_suggested` |
| Restored sub-tick orders | −m … −1 per column save record | SIM-SCH-013 | `source_suggested` |
| Block event broadcast range | 64.0 blocks, squared distance < 4096 | SIM-BEV-004 | `source_suggested` |
| `block_event` / `block_update` / `level_chunk_with_light` / `section_blocks_update` packet ids | 7 / 8 / 46 / 86 | SIM-BEV-004, SIM-SET-005, 0.2 | `black_box_confirmed` at 26.4-snapshot-2 (`assets/mcrs/reports/packets.json`); unchanged up to 26.4-snapshot-4 (`source_suggested`) |
| Load levels: full, block-ticking, entity-ticking | 33, 32, 31 | SIM-TICK-005 | `source_suggested` |
| Default simulation distance | 10 | SIM-TICK-005 | `source_suggested` |
| Temporary ticket for synchronous loads | level 33, 1 level tick, per column | SIM-SET-003 | `source_suggested` |
| Block coordinate packing limit | 33 554 431 (2²⁵ − 1); section/column coordinate 2 097 151 before the generation safety margin | SIM-SET-003 | `source_suggested` |
| `random_tick_speed` default / minimum | 3 / 0 | SIM-RND-001 | `source_suggested`; rule id `black_box_confirmed` (`registries.json`) |
| Precipitation gate | 1 in 48 per pick | SIM-RND-004 | `source_suggested` |
| Position generator | `r = 3r + 1013904223`, `v = r >> 2`, x = `v & 15`, z = `(v >> 8) & 15`, y = `(v >> 16) & 15` | SIM-RND-003 | `source_suggested` |
| Orientations | 48 | SIM-NBR-007 | `source_suggested` |
| Empty-dimension threshold for entity and block entity ticking | 300 level ticks | SIM-TICK-002 | `source_suggested` |
| Pause when empty | `pause-when-empty-seconds` × 20, default 60 s | SIM-TICK-001 | `source_suggested` |
| Time sync interval | 20 server ticks | SIM-TICK-001 | `source_suggested` |
| Fluid types | `empty`, `water`, `lava`; source or flowing is the fluid state's `source` property, the amount its `level` (1 to 8) | 0.1, SIM-SCH-001 | `source_suggested`; changed in 26.4-snapshot-4 (was five types with separate flowing ones, `black_box_confirmed` at 26.4-snapshot-2 by `registries.json`) |

---

## 10. Open questions

Each question lists the rules it affects and an experiment that decides it.

- **OQ-01: Ties between restored ticks of different columns** (SIM-SCH-006, SIM-SCH-013).
  - **Question:** two columns activated together each restore a tick with the same priority and the same sub-tick order (for example both the last of their list, −1). Vanilla picks between them by the internal layout of a priority heap filled in hash-table order, so the choice is deterministic only given the table's history. Which one runs first?
  - **Experiment:** two columns, each with one saved `minecraft:observer` tick `t: 0`, wired so that whichever pulses first locks the other (a repeater lock). Load both in one tick across 20 restarts with different load orders. If the winner varies, the vanilla policy may break the tie by any stable key; propose ascending `(sx, sz)` of the column.
- **OQ-02: One bad element in a tick list** (SIM-SCH-013).
  - **Question:** does the game drop only the bad element, as reading the reference suggests, or the whole list?
  - **Experiment:** edit a region file so `block_ticks` holds an unknown block id followed by a valid repeater tick with `t: 1`. Load the column and observe the repeater toggle; read the log for "Failed to read field".
- **OQ-03: Updates reaching unloaded sections** (SIM-SET-003).
  - **Question:** a neighbour update whose target is in a section that is not loaded loads it (in vanilla, its whole column) synchronously. Confirm, and confirm the column unloads after the 1-tick ticket expires with any ticks scheduled into it saved.
  - **Experiment:** on a server with view distance 2 and one player, an observer chain running across the edge of the loaded area; watch load events (debug screen loaded count, or region file timestamps) and the observer's pending tick in the saved column record.
- **OQ-04: Random tick visiting order** (SIM-RND-001).
  - **Question:** vanilla's column order is a hash-table iteration order. Is any tick-for-tick parity wanted here, given that the position generator and the level random are unseeded anyway?
  - **Proposal:** no; the vanilla policy fixes a stable order (ascending section position) and declares this an intentional deviation. No experiment can reproduce vanilla's order without its RNG state.
- **OQ-05: Unbounded block event drain** (SIM-BEV-002).
  - **Question:** the drain has no bound; a handler that queues a new distinct event each time never terminates the tick. Is a bound an intentional deviation?
  - **Experiment:** none in vanilla (no vanilla block queues an event from its own event unconditionally). Decide by policy; if a bound is added, it must be larger than any vanilla piston contraption needs, measured by counting events per drain in a large flying-machine test world.
- **OQ-06: Client sync in sections that are loaded but do not broadcast** (SIM-SET-005).
  - **Question:** a set with *sync-clients* in a section that is *loaded* but does not *broadcast changes* is not marked. Does a client that already has that section ever see the change before it is resent?
  - **Experiment:** with view distance larger than simulation distance + 2, `/setblock` in a column at load level 33 that the client already displays; observe whether the client shows the new block without relogging.
- **OQ-07: Empty tick lists and the repeater save example** (SIM-SCH-012).
  - **Question:** are `block_ticks` and `fluid_ticks` written as empty lists when there are no ticks, and is the repeater example's `p: -1` what the game writes?
  - **Experiment:** save a column with no pending ticks and read it with an NBT viewer; then run the SIM-SCH-012 conformance test.
- **OQ-08: Duplicate elements in a saved list** (SIM-SCH-013).
  - **Question:** activation adds saved ticks without de-duplication. Do two identical elements both run?
  - **Experiment:** edit a column record so `block_ticks` lists the same `minecraft:observer` position twice with `t: 0`. An observer ticked twice in one phase powers and unpowers in the same tick; a redstone lamp behind it, observed through a comparator-based pulse detector, distinguishes one tick from two.
- **OQ-09: Post-processing order and its ticks** (SIM-SCH-017).
  - **Question:** confirm that post-processing of a freshly generated column runs before its saved ticks activate, and that fluids it touches are ticked at once rather than scheduled.
  - **Experiment:** generate new terrain next to an ocean with a player flying in; on the first tick the new column becomes block-ticking, record whether water at a generated cave opening has already flowed one step (ticked at once) or flows one delay later (scheduled).
- **OQ-10: Exact mode and partitioned mode on one code path** (SIM-MODEL-008, section 11).
  - **Question:** the vanilla collection order (SIM-SCH-006) compares heads of different columns by (priority, sequence) while ignoring the due tick. In a partitioned instance the sequence must be stable (P-SEQ); a sequence derived from partition-local state is not the vanilla counter. Can one ordering policy reproduce the vanilla order exactly when there is one partition and still be stable with several?
  - **Proposal:** the sequence is (partition time at scheduling, partition id, local counter) in a partitioned mode and the vanilla counter in exact mode; the two modes differ only in that policy. Decide when the scheduled-tick kernel is designed.
- **OQ-11: Chain limit across partitions** (SIM-MODEL-103).
  - **Question:** when a chain continues in another partition as a message, does the remaining request budget travel with it (one budget per chain, as vanilla) or restart (one budget per piece)?
  - **Experiment:** none against vanilla (one partition). Decide by policy; travelling keeps the vanilla bound of `M` requests per chain.

---

## 11. Design space

What a third architecture (for example causally connected shards of cubic
sections, merged and split dynamically, with cross-shard effects as messages
applied in a stable order) may and may not do, derived from Part A.

### 11.1 Freedoms it can use

- **Partition shape** (P-SHAPE, P-FORM). Nothing in the contract mentions columns
  or squares. A partition may be any set of cubic sections, of any height, formed
  from causal connectivity (SIM-MODEL-006) rather than proximity, because every
  single step of the machinery has a footprint of radius at most 2 blocks plus the
  handler's own bounded radius (A.2).
- **Parallel steps.** Two partitions whose pending work is causally unconnected
  and whose footprints are disjoint may step in parallel; the result equals any
  sequential order (SIM-MODEL-006), provided the shared counters of A.2 are made
  partition-local and stable.
- **Order across sections.** Scheduled-tick collection across sections, the
  section visiting order of random picks and of the client broadcast, and the
  order between causally unconnected chain requests are free (SIM-MODEL-102,
  SIM-MODEL-104, SIM-MODEL-106, SIM-MODEL-107).
- **Random streams** (P-RNG, P-RANDOM): only distributions are required, so a
  stream per partition or per section, seeded from saved state, is allowed and
  makes the result reproducible, which vanilla is not.
- **Clock** (P-CLOCK): free-running partition time is allowed by the contract, at
  the cost stated in SIM-MODEL-004 (contraptions spanning partitions of different
  speeds). Lock-step costs nothing in a design whose partitions all advance once
  per server tick, and keeps world time and partition time equal.

### 11.2 Hard constraints

- **Exact mode.** With one partition per dimension, the vanilla ordering policy
  for every queue, lock-step clock and the vanilla counters, every rule of
  sections 1 to 9 holds, and parity fixtures recorded from vanilla pass. The
  exact mode runs on the same code path as the partitioned one; only the
  partitioning and the ordering policy differ.
- **No lost work.** Merges, splits and messages keep every pending item and every
  key except a rebased due tick (SIM-MODEL-002, SIM-MODEL-004). The
  region-threaded instance's loss of priority on merge is the failure to avoid.
- **No dropped cross-partition effects.** A neighbour, shape, tick or event item
  aimed outside the partition is a message, never a silent skip or an error, or
  contraptions on a partition boundary behave differently from the same
  contraption inside one partition.
- **Stable order.** Every order is a function of item keys and stored state
  (SIM-MODEL-008). A shared atomic counter, hash iteration or "first region found"
  is not.
- **Delay and promptness** (SIM-MODEL-004), **de-duplication** (SIM-SCH-004),
  **budget semantics** per partition (SIM-SCH-006, SIM-SCH-008), **update limit**
  travelling with the cascade (SIM-SHP-002), **chain limit** (SIM-NBR-004,
  OQ-11), **phase order** (A.4).
- **Save format** (SIM-MODEL-108): the per-column record with relative delays and
  per-record restored sequence numbering, whatever the partitioning.
- **Unbounded height.** No rule may assume a column of fixed height; the
  vanilla-only column groupings stay in the vanilla instance (0.2).
