# Scattering: the footprint scheduler and the feature pipeline

A specification of the scheduler that plans work by footprints, and of the
scattered-object pipeline — ore on the modern path and trees — as its first two
consumers. Structure materialisation and the structure placement index are
outside this document; `structures.md` specifies them.

`worldgen.md` is the authority on invariants and vocabulary; this document
cites its sections and labels rather than restating them. `heightmap.md` owns
the maps the placement modifiers read.

The version named by `assets/minecraft/version.json` has no
`configured_feature` registry. A `Feature` carries its own configuration
(`Feature`), a `PlacedFeature` refers to
it directly, and the corpus folder is
`data/minecraft/worldgen/feature/` of the client jar (235 files) beside
`worldgen/placed_feature/` (273 files). This document uses the names of that version.

---

## 1. The scattering stage as an object

`worldgen.md` §1 defines the class: a stage whose write footprint leaves the
unit, so that the order in which units are processed is observable (W1) and
therefore part of the definition of the world. §11 defines the object inside
the stage: a placer chain and a generator, seeded from the triple of unit seed,
step number and global object index (F1, F2).

What this section adds is the precise statement of *what* is defined, so that
the scheduler in §3 can be checked against it rather than against intuition.

**S1. A scattering stage is a family of per-unit programs plus a rank.** For
each unit `U` there is a program `run(U)` — the sequence of objects, in step
and index order, each drawing from the seed chain of S2 — and there is a total
order `rank` on units that is a pure function of unit coordinates. The world
after the stage is the world before it, with every write of every `run(U)`
applied, and where two writes land on one position the write from the unit of
higher rank is the one that stands. Nothing else is part of the definition: not
the thread that ran a unit, not the moment it ran, not the order in which two
units of unrelated positions ran.

**S2. The seed chain is data from the reference.** The unit seed is
`setDecorationSeed(worldSeed, originX, originZ)` with the column's block origin
: reseed a Xoroshiro source with the world seed,
draw two longs and OR each with 1, and take
`(originX·a + originZ·b) ^ seed`. The
wrapper matters: over a Xoroshiro source a 32-bit draw is the top
32 bits of one Xoroshiro long, and
the long draw composes each long from two such draws, the first
shifted up 32 and the second added sign-extended,
so each of the two longs costs two Xoroshiro longs and is not either of them.
`decoration_seed` in `crates/mcrs_minecraft_worldgen_generator/src/stages.rs`
does exactly that, pinned against the per-chunk stream seeds the structure
geometry fixture records. An object's seed is
`decorationSeed + index + 10000·step`, and every
reseed is a fresh 128-bit upgrade of the long
. The decoration source is always
Xoroshiro, regardless of the `legacy_random_source` flag of the noise settings
. `XoroshiroRandom::new` performs the same upgrade
(`crates/mcrs_minecraft_random/src/xoroshiro.rs`).

Consequences of S2 that the scheduler must not break: the RNG state does not
carry across objects, so skipping an object that does not belong to the window's
biomes shifts nothing (absent features are
skipped, present ones keep their global index); the chain and the generator
share one source, so the draw order inside an object is part of the definition
(§4.2).

**S3. What a program reads is part of the definition too.** An object reads
blocks, heightmaps and biomes, and its draws depend on what it read (a tree
clips its height to the free space above it;
an ore vein draws a float per candidate
block when its air-exposure chance is fractional). So the state an object sees must be
pinned as precisely as the seed. This document pins it in Wn2 (§3.4): a unit
sees the *filled* window plus its own earlier writes, and never another unit's
scattering writes. That is a deliberate divergence from the reference, whose
units see whatever neighbours happened to be decorated first, i.e. the
player's route (§15 of `worldgen.md`, last paragraph).

**S4. The write footprint is a hard bound, not a hint.** Every object's origin
lies in its own column (`in_square` jitters by
0..15 from the column origin; `count_on_every_layer` and `fixed_placement` do
the same),
and the reference refuses any write beyond one column from it
(with the radius set at). A write outside the footprint is a data
error: the corpus never does it, because the reference would have logged it. A
write that lands outside the window is dropped and asserted in debug builds,
never deferred and never applied later.

**S5. What is observable and what is not.** Observable: every block after the
stage; hence the rank (S1), the seed chain (S2), the read rule (S3), the draw
order inside an object (§4.2), and the object order inside a unit — the
topological sort of F2. Not observable, and therefore free for the scheduler:
which worker runs a unit, when, in what order relative to units it shares no
position with, whether a unit's program ran once or was re-run after eviction
(St1 of `worldgen.md`: a re-run is bit-identical).

**S6. Structures occupy a slot in the same loop and consume none of the
feature numbering.** The reference walks eleven steps
(or more if a datapack has longer lists); in each step it first places the structure starts
of that step with a counter of their own, then the step's features with the
sorted global index. Feature seeds do
not depend on the structure counter, so a pipeline that runs the feature half
alone produces the same features it will produce once structures exist.

---

## 2. Footprints

**What radius a tree needs.** Foliage radius is bounded by the codec at 16
, trunk height at 32 plus two
random terms, and the branching
placers add horizontal reach on top. The reference does not bound the tree by
its data; it bounds it by the write radius of the stage, one column
, and drops the rest (S4). A tree reads free space
over a `minimum_size` footprint around the trunk
and rewrites leaf distances within its own bounding box
, so its read radius is the same one
column. `r_w = r_r = 1`.

**What radius an ore vein needs.** A modern vein spreads `size/8 ≤ 8` blocks
from the origin along a random direction and probes a box a further
`ceil((size/16·2 + 1)/2) ≤ 5` blocks wide,
so it reaches at most thirteen blocks past an origin that lies inside the
column: `r_w = 1`. Before placing it scans the `OCEAN_FLOOR_WG` heightmap over
that box, and with a fractional air-exposure chance it
reads six neighbours per candidate: `r_r = 1`.

**Where the reference's radii come from.** Features require terrain at radius
1 and write at radius 1; light requires initialised light at radius 1
. Terrain at radius 1 around features at radius 1
is the twenty-five-column window that §3.3 derives independently. The
structure-starts requirement at radius 8 on the same lines is the dependency
§10 of `worldgen.md` rejects, and it is absent here.

**The two heightmap generations.** The reference keeps two sets of maps
. `WORLD_SURFACE_WG` and
`OCEAN_FLOOR_WG` are maintained by the fill and by the surface stage
(surface writes go through
the proto-chunk while the status is still `biomes`), and no
carver touches them. The four final maps are primed after carving
 and updated on every decoration write
(the status selects them for the `terrain` and `features`
statuses). So a placement that names a `_WG` map reads the column **before
carving**, and one that names a final map reads it **after carving and after
the column's own earlier objects**. `heightmap.md`'s appendix calls the pairs a
split by lifetime with identical predicates; the predicates are identical, the
contents are not, and 27 shipped placements read the pre-carve pair
(`grep -l _WG worldgen/placed_feature/*.json` in the unpacked data pack: the grass
patches, the disks, both lava lakes, glow lichen, bamboo). `heightmap.md` §4
now states the two generations; Wn3 below carries the maps.

---

## 3. The scheduler

### 3.1 What is truth and what is derived

Classifying every value before storing it, per the project's ECS rule:

| Value | Category | Owner | Note |
| --- | --- | --- | --- |
| Seed, compiled feature tables (§4.3) | truth | the freeze build | cloned into the sub-app once, like `DimensionRouters` (`crates/mcrs_minecraft_vanilla_worldgen/src/routers.rs`, `DimensionRouters`) |
| Which columns players want, at which target | derived from players | the generation actor, at each request | `worldgen.md` §16: never stored |
| A column's stage | derived, but only recoverable by redoing the work | the staging store | an enum field, not a marker (X10); each section entity holds one `SectionStage` that moves `Loading → Generating → Loaded` |
| A filled column, its palettes, its six maps | materialised projection of the seed | the staging store | read by up to nine `run`s; recomputing is a whole fill |
| A unit's out-of-column writes (its deltas) | materialised, temporary | the staging store | consumed by one merge each, then dropped |
| The rank of a unit | pure function | nobody | computed where compared |

There is one owner for every materialised value: the generation actor of the
dimension (`crates/mcrs_minecraft_vanilla_worldgen/src/generation_actor.rs`, `GenerationActor`). The
actor runs on a thread of its own, one thread for each dimension. It holds the
wanted columns (`pending`), the stages on the pool (`in_flight`) and the
staging store (`crates/mcrs_minecraft_worldgen_generator/src/staging.rs`,
`StagingStore`). Its completion handler (`GenerationActor::finish`) has a
branch for each stage. The store is outside the ECS and outside the dimension
sub-app (X1).

The dimension and the actor share no data. They exchange messages only
(`crates/mcrs_minecraft_vanilla_worldgen/src/generation_link.rs`, `GenerationRequest`,
`GenerationReport`). The dimension sends the fill context, the wanted columns
and the views of each tick. The actor sends back each delivered column, the
sections of each abandoned column, and a report when it owes nothing more.
The dimension takes the reports in at the start of each tick, in the order of
the column positions
(`crates/mcrs_minecraft_vanilla_worldgen/src/world_generation.rs`, `deliver_world_columns`).
Nothing takes them in between ticks.

### 3.2 The stage ladder

Every column passes through four stages. Each is a separate task on the
generation pool. The actor dispatches each task
(`crates/mcrs_minecraft_vanilla_worldgen/src/generation_dispatch.rs`, `GenerationActor::dispatch`).

```
Filled     fill, surface, carving, the six heightmaps, packed palettes
Run        the column's own program against its 3×3 window; own writes into
           a private buffer, out-of-column writes into eight deltas
Merged     own buffer + the eight incoming deltas, applied in rank order;
           the four final maps rebuilt; packed
Delivered  the sections enter the ECS as SectionStage::Loaded
```

`Filled` is the fill task (`crates/mcrs_minecraft_worldgen_generator/src/stages.rs`,
`fill_column`) plus the heightmap descent that follows it, run twice: once
before carving for the pre-carve pair and once after for the final four (Wn3).
`Delivered` is the delivery of the actor (`GenerationActor::deliver`). The
dimension integrates the delivery in its column drain.

**Q1. Dependencies, derived from C2.**

```
Run(U)       requires  Filled(V)  for every V with |V − U|∞ ≤ 1
Merged(U)    requires  Run(V)     for every V with |V − U|∞ ≤ 1
Delivered(U) requires  Merged(U)
```

Hence `Delivered(U)` requires `Filled` over the 5×5 around `U`: twenty-five
columns, the same accumulated window the reference pays to light a chunk
(the `structure_starts` requirement excluded). It is
not the same as what the pipeline pays today, which fills only the columns a
view asks for. The halo cost is `(2V+5)²/(2V+1)²` in fills per wanted column —
1.42 at a view radius of ten, counted, not measured — and the reference pays
it too.

**Q2. No stage waits on a stage of its own kind.** `Run(U)` reads only
`Filled` snapshots, which are immutable once produced, and writes only into
memory nobody else holds. `Merged(U)` reads only deltas, which are immutable
once produced, and writes only `U`. So every stage is a field stage in the
sense of §1: two `Run`s never conflict, two `Merge`s never conflict, and C3
holds without a colouring, a barrier or a lock. This is form 1 of
`worldgen.md` §14 — compute locally, push cross-border output into a delta
queue, apply in a separate phase — chosen over the in-place colouring
for the reason in §3.3.

**Q3. The rank is the colour index, fixed as data.** With
`r_w = r_r = 1` the colouring formula gives `d = 2`, `k = 3` and nine colours;
`rank(U) = (x mod 3)·3 + (z mod 3)` with Euclidean remainders, so that negative
coordinates colour like positive ones. Merging applies the nine writers of a
position in ascending rank, last write standing. Any pure function of the
coordinates would define *a* world; this one keeps the colouring vocabulary and makes
the in-place executor of §6.4 comparable. Changing it is a different world
(§15). No configuration hash exists in the workspace yet; St3 owes one, and
its inputs are fixed here so the first version is complete: the world version
(5124), the data set's content hash, the precision profile, the unit size (one
column), the rank function (Q3), and the read rule (Wn2). A mismatch on any of
them means an untouched unit may not be regenerated from the seed.

**Q4. The write and the merge are the whole of N1.** In the delta form the
conflict predicate is never evaluated, so the bug N1 describes — dropping one
radius from `d` — cannot occur. What replaces it is the merge-order bug: a
writer applied out of rank, or a delta lost between eviction and merge. §6.3
is written against that.

### 3.3 Why not the colouring

An in-place scattering stage needs a colouring, and its barrier is paid at the
maximum unit cost of the batch (N3). There is a second cost, and it is the one
that decides the design here.

In-place execution with the colour order means: `U` may run when every
conflicting unit of lower colour has run. With nine colours and `d = 2`, a unit
of colour 8 waits for its lower-coloured neighbours within two columns; each
of those waits for its own; the chain is strictly decreasing in colour, so it
has at most eight steps, each of at most two columns. Counting the lexicographic
order on `(x mod 3, z mod 3)`: the `x` residue can fall twice, the `z` residue
twice per `x` level, and a fall of a residue is a displacement of `+2` or `−1`;
the reach of a chain is four columns in `x` and sixteen in `z`. A streaming
dispatcher — one that pops the nearest wanted column
(`crates/mcrs_minecraft_vanilla_worldgen/src/generation_dispatch.rs`, `walk_fresh`) rather than decorating a closed batch — must therefore
fill and decorate columns up to sixteen away from the view before it can
deliver a column at the edge, and those columns are wanted by nobody. The wave
barrier the reference's batch model would use does not exist here: there is no
batch, only a frontier that moves with the player.

The delta form has no chain. `Run(U)` needs nothing that has run, `Merged(U)`
needs exactly the eight neighbouring runs, and the reach is the two columns of
Q1. The price is S3: a unit cannot see its neighbours' scattering writes, which
the reference's units can see when the route happens to have decorated the
neighbour first. Since that visibility is route-dependent in the reference, it
was never part of any reproducible world, and giving it up loses no parity that
was attainable (§15). What it changes in the picture — trees at a border can
interleave with the neighbour's trees, a lake can cut a neighbour's trunk — the
reference also does, on some routes. §6.4 measures how far the two definitions
sit apart.

### 3.4 The window

**Wn1. A window is nine owning snapshots and one private centre.** A `Run`
task receives `Arc` handles to the nine `Filled` snapshots of its 3×3 (X7: an
owning snapshot crosses the boundary, never a key) and a dense buffer for its
own column, unpacked from the centre snapshot. The dense buffer is the existing
per-worker `ColumnBlocks` (`crates/mcrs_minecraft_worldgen_generator/src/column_blocks.rs`, `ColumnBlocks`);
F7 is honoured for the centre because it already is. For the ring, reads go
through the snapshots' paletted sections with a last-section cache, the shape
of the reference's `BulkSectionAccess`.
Whether the ring should be flat too is unmeasured; the ring is read by the
tree's free-space scan and the vein's probe and adjacency tests, and the count
of those reads per column is what decides it (measured in). Unpacking
eight neighbours densely would be eight more buffers of 98,304 cells per
worker (`crates/mcrs_minecraft_worldgen_generator/src/column_blocks.rs`, `new`), which T2 argues against without a measurement
for it.

**Wn2. The read rule.** A read at position `p` answers, in this order: the
unit's own earlier write to `p` if there is one, anywhere in the 3×3; else the
`Filled` block of the snapshot that holds `p`; else, outside the 3×3, air, with
a debug assertion. The reference logs a read outside the write zone as unsafe
, so the corpus does not do it; the fixed
answer keeps the rule a pure function should a datapack do it anyway. Own
writes are found through a written-mask of one bit per cell over the 3×3
(nine times 12 KB per worker, cleared per unit) and, for the ring, the delta
entry the mask points at.

**Wn3. The window carries six maps per column.** Two pre-carve maps
(`SURFACE`, `SOLID` in the vocabulary of `heightmap.md` §2, answering
`WORLD_SURFACE_WG` and `OCEAN_FLOOR_WG`) frozen at `Filled`; and the four
final maps (`heightmap.md` §2, answering the four final names of), frozen at `Filled` for the ring and **live for the
centre**: every write into the centre updates them by the rule of
`heightmap.md` §5. A tree write is the cheap "rise" branch; an ore write
replaces stone with ore and moves nothing, which the §7 cut-off of
`heightmap.md` recognises in one predicate comparison; a lake removes blocks
and pays the descent, and lakes are few. After `Merged` the four final maps are
rebuilt once by the existing descent (`crates/mcrs_minecraft_engine/src/heightmap.rs`, `update_column_heightmaps`, `block_at`), because a
neighbour's delta can raise them too. The pre-carve pair needs one extra
descent per column before carving, which by the fast path of
`heightmap.md` §4 is one read per strip for most strips: unmeasured.

**Wn4. The biome set is computed once per `Run`.** F6 asks for once per batch;
here the batch is the unit, and the set is the union of the distinct entries of
every section palette of the nine snapshots
(`VoxelPalette::for_each_distinct`,
`crates/mcrs_minecraft_chunk/src/voxel_palette.rs`, `set`), intersected with the
biome source's possible biomes as the reference does. The
block-resolution biome query the `biome` filter needs (F3, Cl4) reads the
window's stored palettes through the zoom
(`crates/mcrs_minecraft_biome/src/zoom.rs`, `obfuscate_seed`); unlike the surface stage,
whose read radius is zero and which must re-evaluate climate, this stage has
the ring in hand.

**Wn5. The writer.** `set(p, state)`: in the centre, store into the dense
buffer, set the mask bit, update the live maps; in the ring, append
`(target column, cell index, state)` to that column's delta and set the mask
bit; outside the 3×3, drop and assert (S4). A delta is a `Vec` of packed
entries per target column; a tree at a border contributes tens, an ore vein
tens, so the eight deltas of a column are small and their exact structure is
not a design decision. The mask over the centre is kept into `Merged`: a
neighbour's delta of lower rank must not overwrite a cell the centre wrote
itself (Q3), and the mask is what says which cells those are.

### 3.5 The dispatcher

The queue has a key (`ColumnKey`, distance then position,
`crates/mcrs_minecraft_vanilla_worldgen/src/generation_actor.rs`) and a shape (a `BTreeMap` popped from
the front, `GenerationActor::pending`). An entry asks for a target stage, and
it is ready by Q6.

**Q5. A request names a target stage, and requests propagate by Q1.** A view
that wants column `U` (`crates/mcrs_minecraft_vanilla_worldgen/src/world_generation.rs`, `ask_world_columns`;
`GenerationActor::want`) asks for
`Delivered(U)`; the actor derives `Run` over the 3×3 and `Filled` over the
5×5, each at the priority of the column that needs it, never stored beyond the
tick that computed it (`worldgen.md` §16). A derived request that is already
satisfied or already in flight is a no-op.

**Q6. A task is dispatched only when its inputs are in the store.** Readiness
is checked by scanning the 3×3 of the store on each attempt: nine lookups in a
hash map, cheap enough not to warrant a per-column dependency counter until a
profile says otherwise. The check runs when a stage completes, over the
neighbours that may have been waiting on it, and when a request arrives.

**Q7. The staging store is bounded by the halo, and eviction is safe.** A
`Filled` snapshot is kept while any column within two of it is wanted; a delta
is kept until its target is merged or its target stops being wanted. Evicting
a `Filled` column and filling it again later is bit-identical by St1 and costs
one fill. The store holds at most the halo of the views: at a view radius of
ten, 625 paletted columns per player, tens of kilobytes each; unmeasured.
Cancellation (`crates/mcrs_minecraft_vanilla_worldgen/src/generation_actor.rs`, `GenerationActor::screen`,
`GenerationActor::evict_outside_halo`) uses "wanted or within two of a wanted
column" for "wanted".

**Q8. Delivery is the only point where a column becomes visible, and nothing
writes into it afterwards.** By Q1, when `U` is delivered every unit that can
write into `U` has run and its delta has been merged. This closes the hole of a delivered column that a later write changes: light reads its neighbours at radius one
(`crates/mcrs_minecraft_light_engine/src/epoch.rs`, `of`), and every neighbour it can see is either delivered — and
so final — or absent, which the light code already treats as "not yet"
(`light.rs` seeds on a section landing in `Loaded` and re-seeds seams as
neighbours arrive). No radius-two dependency on light is declared, and the
light code does not change.

### 3.6 The single-threaded oracle

The executor that stays in the tests forever (`worldgen.md` §19, list item 2)
is the same three stage functions driven by one thread in a fixed order: fill
the region, run every column, merge every column. It shares every line with the
parallel path except the dispatch. A divergence between the two is a race —
a worker-local cache that leaked across columns, a snapshot handed out before
it was complete, a delta lost between eviction and merge — and never rounding,
because no floating-point path differs between them.

The oracle is not an in-place sequential decorator. An executor that lets later
units read earlier units' writes implements a different definition (S3), and it
exists only as the measurement instrument of §6.4.

---

## 4. The feature pipeline

### 4.1 Assets

Two registries, both type-dispatched, both ordinary serde.

`worldgen/feature/<id>.json` is a `Feature`: a `type` field and the fields of
that type (`Feature`; the 58 registered types are listed in the registry; the corpus uses 47 of them, `tree` 45 times,
`simple_block` 37, `ore` 30, `random_selector` 21). In Rust that is one enum
with `#[serde(tag = "type")]`, the shape the noise chunk generator already uses
(`crates/mcrs_minecraft_game_registries/src/worldgen/chunk_generator.rs`). An
unknown type is a load error, as a mismatch with the targeted version always is.

`worldgen/placed_feature/<id>.json` is a `PlacedFeature`: `feature` and
`placement`. `feature` is either an id or an
inline `Feature` — the reference's `Holder` codec — and so is every
`PlacedFeature` reference inside a selector feature
(`trees_plains` in the corpus inlines two
and references one). That is the either-form of `worldgen.md`'s serde section:
a hand-written `Deserialize` with a symmetric `Serialize`, so the shape
round-trips unchanged; `TimeMarker` is the in-repo model
(`crates/mcrs_minecraft_environment/src/timeline/marker.rs`). `placement`
is a list of `PlacementModifier`, a tagged enum over the eighteen registered
types (the corpus uses fifteen,
`biome` 211 times, `count` 201, `in_square` 198, `block_predicate_filter` 155,
`heightmap` 111).

Nested shapes the two registries pull in, each a serde type under
`crates/mcrs_minecraft_worldgen_feature/src/`:

| Shape |
| --- |
| `IntProvider`, `HeightProvider`, `VerticalAnchor` |
| `BlockState` (name or name plus properties) |
| `RuleTest` (`predicate_type`-tagged, ten types) |
| `BlockPredicate` (`type`-tagged, sixteen types) |
| `BlockStateProvider` (ten concrete), `TrunkPlacer` (ten), `FoliagePlacer` (twelve), `TreeDecorator` (eleven), `FeatureSize` (two), `RootPlacer` (one) | `feature/stateproviders`, `trunkplacers`, `foliageplacers`, `treedecorators`, `featuresize`, `rootplacers` |

Where the type of a value depends on a sibling — every one of the shapes above
— the internally tagged enum is the answer, and no `DeserializeSeed` is needed:
the tag is a field of the same object. The seed case of `worldgen.md` (a map
key choosing the value's type) does not occur in these registries.

**Fe1. Validate at freeze, never at run.** The proto keeps ids; resolution to
indices, `VoxelId`s and tag masks happens once at `OnEnter(WorldgenFreeze)`,
the way carvers are resolved (`crates/mcrs_minecraft_vanilla_worldgen/src/modern_carvers.rs`, `build_modern_carver_biomes`) and routers
compiled (`crates/mcrs_minecraft_vanilla_worldgen/src/routers.rs`, `DimensionRouters`, `build_dimension_routers`), and the result is a resource cloned into each
dimension's sub-app. Every range the reference's codecs enforce is enforced
there: `count` 0..4096, ore `size` 0..64 and the
air-exposure chance 0..1, foliage radius
0..16, offsets −16..16, environment-scan steps
1..32. An id that resolves to nothing — a
feature a biome names, a block a provider names, a tag a rule test names — is
a freeze error naming the asset. The freeze build resolves the biomes' ids the
way the carver build resolves carver ids (`crates/mcrs_minecraft_vanilla_worldgen/src/modern_carvers.rs`, `build_modern_carver_biomes`).

A biome's step list is a `HolderSet`: either a `#tag` of placed features or a
list whose entries are ids or inline placed features
(inline entries allowed; a flag only fixes
the serialised form). The shipped corpus uses ids only and ships no
`tags/worldgen/placed_feature/`, but the targeted version admits all three, so the proto is a
`Vec<FeatureStepList>` where a step list is the either-form of a tag or a list
of holders, and the current `Vec<Vec<ResourceLocation>>`
(`crates/mcrs_minecraft_biome/src/lib.rs`, `Biome`) changes. At freeze a tag
expands to its entries in the tag file's order, and an inline entry is a vertex
of its own in the sort (§4.3). The biome asset declares no dependencies
and keeps not doing so; the feature registries load
through Fe2 and are joined by id at freeze.

**Fe2. Loading goes through the worldgen registries macro.** The four
registries a worldgen asset can name are spelled once
(`crates/mcrs_minecraft_worldgen/src/bevy.rs`) and grow the ids, the
handles, the dependency walk and the loader for each. `feature` and
`placed_feature` are two more rows: `placed_feature` is a nested registry
(it names features and, through selectors, other placed features), `feature`
is nested for the same reason. The protos live beside the existing ore port in
`mcrs_minecraft_worldgen_feature::place`; the compiled forms live next to them, in
the pattern of `material::proto` and `material::compile`
(`crates/mcrs_minecraft_worldgen_surface/src/compile.rs`, `SURFACE_NOISE_NAMES`, `Op`, `SurfaceNoise`).

**Fe3. Beta's populate step is one feature.** Beta draws every object of a
chunk's populate step from one `LegacyRandom` seeded per chunk
(`beta_chunk_seed`), in the order the step walk fixes, and
where that stream stands when an object starts depends on what the objects
before it read: clay spends its vein draws only where it finds water. A placed
feature is seeded from the decoration seed, its step and its index alone, so no
split into one placed feature per object reproduces that stream. The step is
therefore one feature type, `mcrs:beta_populate`, which every `beta_*` biome
lists and the `Run` of §3.2 runs like any other feature; it seeds its own
stream and ignores the source its step hands it. Its table stays Beta code, and
its veins cross column borders as Beta's did.

### 4.2 The modifier chain

The reference's placer is an explicit stack machine:
a stack of positions with the index of the modifier that should consume each;
the origin is pushed with index 0; the loop pops, runs one modifier, and pushes
that modifier's outputs **in reverse** so they pop in emission order
; outputs of the last modifier go to the generator.
The random source is shared by every modifier and by the generator, so the traversal order fixes the draw order and is part of the definition
(S5). An empty chain runs the generator at the origin directly.

**Fe4. The chain is compiled to an enum and driven by reused stacks.** One
`Vec<Modifier>` per placed feature, no boxing, no recursion; one
`PlacerScratch` per worker with the position stack, the index stack and the
output buffer, cleared between objects and never reallocated in steady state
(F5). The traversal is the reference's, reverse push included; a placer that
processed outputs in emission order without reversing would draw in a
different order and place a different world.

What each modifier reads, so that its footprint is known before it runs:

| Modifier | Reads | Draws |
| --- | --- | --- |
| `count`, `noise_based_count`, `noise_threshold_count`, `count_on_every_layer` | the count provider; the two noise variants sample a simplex noise with the fixed seed 2345 that no world seed enters; the layer variant reads `MOTION_BLOCKING` and descends through blocks per layer | count, then per-layer jitter |
| `in_square` | nothing | two |
| `height_range` | the height provider against the dimension extent | the provider's |
| `heightmap` | one map at the origin's strip; which generation, per Wn3 | none |
| `surface_water_depth_filter` | `OCEAN_FLOOR` and `WORLD_SURFACE` at the strip | none |
| `surface_relative_threshold_filter` | one map at the strip | none |
| `block_predicate_filter` | blocks around the origin through a `BlockPredicate`; `would_survive` is four tag rules by block family, §5.2 | a `random_*` rule test draws |
| `environment_scan` | a vertical run of blocks, at most 32 | none |
| `biome` | the block-resolution biome at the origin, then the biome's feature set | none |
| `rarity_filter`, `random_chance` | nothing | one float |
| `offset` | nothing | three |
| `randomly_selected` | one of its sub-modifiers | one, then the sub-modifier's |
| `fixed_placement` | the origin's column | none |
| `cuboid` | nothing | the two size providers |

Every read above is inside the origin's column or within one column of it,
which is the `r_r = 1` of §2.

**Fe5. The step loop is the reference's, minus the structure slot.** For each
step from 0 to `max(11, longest biome list)`: the structure slot (empty until
structures exist); then the features of this step present in the window's biomes, ascending
by global index, each reseeded by S2 and placed with the biome check.

### 4.3 The topological sort

F2 prescribes copying the algorithm, and this section says which lines. Input:
the biome source's possible biomes in the source's own order,
and per biome its list of step lists:

1. Assign each distinct placed feature an index in order of first encounter,
   walking biomes in input order and steps in order.
   Identity is object identity (a `Reference2IntMap`); two biomes naming the
   same registry entry share one vertex, two inline copies of the same shape
   are two vertices.
2. Vertices are `(index, step, feature)` ordered by step then index; the edge map is a `TreeMap` in that order, and each biome's list
   contributes the edges `feature[i] → feature[i+1]` within its own list, with
   every vertex present even when it has no successor.
3. Depth-first search over the edge map's key order, with the three-set
   colouring: a vertex is emitted after its
   successors, in post-order; a back edge is a cycle. The emitted list is
   reversed.
4. The result is split by step, keeping the sorted order within each step;
   a feature's position in its step's list is the
   `index` of S2.

A cycle is a freeze error naming the biomes involved
(the reference's reduction loop is diagnostics,
not recovery).

**Fe6. The sort output is a per-dimension resource.** possible biomes differs
per biome source, so the tables are keyed by dimension exactly as routers and
carver tables are (`crates/mcrs_minecraft_vanilla_worldgen/src/routers.rs`, `DimensionRouters`; `crates/mcrs_minecraft_vanilla_worldgen/src/modern_carvers.rs`, `build_modern_carver_biomes`). Per step: the
sorted list of compiled placed features. Per biome: a bit set over each
step's list saying which of the step's features the biome carries — the union
across the window's biomes is `possibleFeaturesThisStep`
, and the per-biome test is `hasFeature` for
the `biome` filter. The set is derived from the assets at freeze, so a
datapack that adds a feature to a biome is picked up without a code change.

**Fe7. The order of possible biomes is part of the definition for every
biome source.** The reference takes the distinct biomes in order of first
appearance in the parameter list; the in-repo overworld and nether lists
keep that order and say so (`crates/mcrs_minecraft_biome/src/overworld_preset.rs`). The other sources
in the workspace (`crates/mcrs_minecraft_biome/src/source.rs`, `BiomeSource`): `Fixed` is one biome;
`Checkerboard` is its list in file order; `TheEnd` is the reference's five end
biomes in the order `TheEndBiomeSource` lists them; `Beta` is the eleven land
biomes in `BetaLandBiome` discriminant order (`crates/mcrs_minecraft_biome/src/source.rs`, `BetaLandBiome`); Beta has no
ocean biome. The Beta order is ours to fix,
since the reference has no such source; it is fixed here and covered by the
configuration hash (Q3).

### 4.4 Block access from an object

Objects see the window through one reader and one writer (Wn2, Wn5), with
world-absolute coordinates, the same shape the ore port already takes
(`crates/mcrs_minecraft_worldgen_feature/src/placer/mod.rs`, `WorldGenVolume`). Beyond blocks a
generator needs: the six maps of Wn3, the block-resolution biome (Wn4), the
dimension extent, the block tag registry
(`crates/mcrs_minecraft_worldgen_generator/src/ids.rs`, `block_tags`, which reaches the actor in the fill context) resolved to masks at freeze, and
the property-transition tables a tree needs to rewrite a leaf's `distance`
 — a state-to-state map per block, built at freeze
from the block definitions, since `try_resolve_state` already moves a stated
property (`crates/mcrs_minecraft_worldgen_generator/src/block_state.rs`, `try_resolve_state`).

---

## 5. The first two consumers

### 5.1 Ore on the modern path

`ore` is 30 features and `scattered_ore` two; their placements are `count`,
`in_square`, `height_range` (uniform or trapezoid over anchors), `biome`, and
sometimes `rarity_filter`, as in `ore_coal_upper` and `ore_diamond` in the
corpus. From the assets a vein needs:

- `targets`: a list of `{target: RuleTest, state: BlockState}`
. The corpus's rule tests are
  `tag_match` over `stone_ore_replaceables`, `deepslate_ore_replaceables` and
  `height_specific_ore_replaceables`, `height_match` on `y`, and `any_of`,
  `all_of`, `not` over them — resolved to tag masks and ranges at freeze.
- `size` 0..64 and `discard_chance_on_air_exposure` 0..1.

From the window it reads the pre-carve `OCEAN_FLOOR_WG` map over its probe box
(the box reaches the ring), the block at every
candidate, and six neighbours per candidate when the chance is fractional.

The algorithm: `size`
sphere centres interpolated along the segment with one double drawn each
, the pairwise cull of a smaller sphere inside a larger one
, and a per-box `tested` bit set that visits every position once
across spheres. The bit set is not an optimisation: with a
fractional air-exposure chance every visited candidate draws a float
, so visiting a position twice draws twice.

The existing Beta ore port (`crates/mcrs_minecraft_worldgen_feature_place/src/ore_beta.rs`, `OreConfig`) is the Beta algorithm —
no cull, no bit set, no probe, `+2` on the vertical endpoints — and stays what
it is. The modern vein is a second function beside it, not a flag on the
first: the loop structure differs, and little more than the vertical offset convention is shared.

### 5.2 Trees

`tree` is 45 features; `trees_plains` in the corpus is a `random_selector`
over `oak_bees_005`, `fancy_oak_bees_005` and `fallen_oak_tree`, placed with
`count` over a weighted list, `in_square`, `surface_water_depth_filter`,
`heightmap: OCEAN_FLOOR`, `block_predicate_filter: would_survive oak_sapling`,
`biome`. From the assets a tree needs (`TreeFeature`):

- `trunk_placer`: all ten types in the corpus (`straight` 19, `fancy` 6,
  `poplar` 6, `dark_oak` 5, `giant`, `cherry`, `upwards_branching` 2 each,
  `bending`, `forking`, `mega_jungle` 1 each).
- `foliage_placer`: twelve types in the corpus, `blob` 16 and `fancy`,
  `poplar` 6 each at the top.
- `trunk_provider`, `foliage_provider`, `below_trunk_provider`: state
  providers, `simple_state_provider` mostly, `rule_based_state_provider` with
  a block predicate for the dirt under a trunk (`oak` in the corpus).
- `minimum_size`: `two_layers_feature_size` or `three_layers_feature_size`.
- `decorators`: `place_on_ground` 20, `beehive` 16, `shelf_mushroom` 6,
  `leave_vine` 5, and six rarer ones. `beehive` places a bee nest and then, if
  the level hands back a block entity, draws `2 + nextInt(2)` occupants with
  `nextInt(599)` each. The draws
  sit inside that block-entity branch, so a level without block entities would also
  skip them and shift every later draw; the decorator writes into a channel so
  the draws always happen.
- `ignore_vines`, `root_placer` (mangrove only).

From the window it reads free space (`isFree`: air or the
`replaceable_by_trees` tag)
over the `minimum_size` footprint up to the tree's height
, vines when not ignored, the block under a sapling
for `would_survive`, and
its own bounding box during the leaf-distance pass
. It writes logs, leaves with `distance` and
`persistent`, roots, and the decorators' blocks.

**`would_survive` is four rules over tags.** The corpus tests thirteen states
through it; at the targeted version each one's `canSurvive` is a tag test plus a shape, and the
tags are in the data pack (`data/minecraft/tags/block/supports_*.json`):

| States | Rule |
| --- | --- |
| the ten saplings, `firefly_bush` | the block below is in `supports_vegetation`  |
| `sugar_cane` | below is sugar cane; or below is in `supports_sugar_cane` and one of its four horizontal neighbours is in `supports_sugar_cane_adjacently` as a block or as a fluid |
| `cactus` | no horizontal neighbour is solid or lava; below is cactus or in `supports_cactus`; above is not liquid |
| `mangrove_propagule` | below is in `supports_mangrove_propagule`; the hanging form tests `supports_hanging_mangrove_propagule` above |

The shapes are code, keyed by block family at freeze; the tags are data. A
`would_survive` naming a state outside these families is a freeze error with
the state's name (Fe1), and a fifth shape is added when a datapack needs it.
"Solid" and "liquid" come from the block definitions.

The reference records every write of a tree in four sets and runs the
leaf-distance pass over their bounding box afterwards
. That pass reads the leaves of other trees inside
the box and lowers their `distance`; under Wn2 it reads the `Filled` ring,
where there are no leaves, and the own-column leaves it wrote itself. Then it
asks the level to update shapes on every face of the written shape (it calls the shape update
on each face cell and its outward neighbour). For leaves that is one more
distance recomputation from the neighbour. Live-world block updates inside a
feature are not reproduced: the tree's own pass already recomputes every leaf it wrote.

---

## 6. Verification

Bit for bit against the reference, where the reference computes without a
world — each fixture is a dump from `tools/vanilla-oracle` and carries
`WORLD_VERSION` 5124:

- the sort, per shipped biome source
  (`crates/mcrs_minecraft_worldgen_feature/tests/it/feature_sort.rs`);
- ore geometry over a flat all-stone array
  (`crates/mcrs_minecraft_worldgen_feature_place/tests/it/ore_vein_parity.rs`);
- tree geometry, through the oracle's stub level
  (`crates/mcrs_minecraft_worldgen_feature_place/tests/it/tree_geometry_parity.rs`).

Decorated chunks of a real world are not attainable (§15 of `worldgen.md`,
S3): a dump taken from a running server is a dump of one route.

Structurally: every shipped `feature` and `placed_feature` round-trips to an
equal JSON value, every id every biome names resolves at freeze, and the set
of feature types with no generator is a census over the assets pinned by name
(`generate/tests/corpus_generators.rs`).

The race and the merge: the oracle of §3.6 against the parallel scheduler over
a region, decoded blocks compared bit for bit, through every order the
definition says is unobservable (S5) — worker counts, request orders, batch
sizes, forced evictions (`generate/tests/ladder.rs`).

§6.4's number — how far Wn2 sits from an in-place sequential executor over the
same region — is not yet measured.
