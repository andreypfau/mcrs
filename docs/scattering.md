# Scattering: the footprint scheduler and the feature pipeline

A specification for steps 5 and 6 of `worldgen.md` §19: the scheduler that
plans work by footprints, and the scattered-object half of step 6 — ore on the
modern path and trees — as its first two consumers. Structure materialisation
and the structure placement index (§10, G1) are outside this document; §7 says
what is deferred and why.

`worldgen.md` is the authority on invariants and vocabulary; this document
cites its sections and labels rather than restating them. `heightmap.md` owns
the maps the placement modifiers read. Reference paths are relative to
`~/src/gitlab.com/andreypfau/minecraft/src/main/java/net/minecraft`, version
`26.3-snapshot-10`; they are cited as data, never as a shape to copy.

Every count below is a count; the measurements that settled the cost claims
are in `PERF.md`.

One correction to the brief this document was written against: 26.3 has no
`configured_feature` registry. A `Feature` carries its own configuration
(`world/level/levelgen/feature/Feature.java:20-25`), a `PlacedFeature` refers to
it directly (`placement/PlacedFeature.java:17-24`), and the corpus folder is
`assets/minecraft/worldgen/feature/` (235 files) beside
`worldgen/placed_feature/` (273 files). This document uses the 26.3 names.

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
(`ChunkGenerator.java:384-391`): reseed a Xoroshiro source with the world seed,
draw two longs and OR each with 1, and take `(originX·a + originZ·b) ^ seed`
(`levelgen/WorldgenRandom.java:44-51`). An object's seed is
`decorationSeed + index + 10000·step` (`WorldgenRandom.java:53-56`), and every
reseed is a fresh 128-bit upgrade of the long
(`levelgen/XoroshiroRandomSource.java:45-46`). The decoration source is always
Xoroshiro, regardless of the `legacy_random_source` flag of the noise settings
(`ChunkGenerator.java:390`). `XoroshiroRandom::new` performs the same upgrade
(`crates/mcrs_minecraft_random/src/xoroshiro.rs:20-23`), and nothing else of
the chain exists in the workspace yet.

Consequences of S2 that the scheduler must not break: the RNG state does not
carry across objects, so skipping an object that does not belong to the window's
biomes shifts nothing (`ChunkGenerator.java:456-467`: absent features are
skipped, present ones keep their global index); the chain and the generator
share one source, so the draw order inside an object is part of the definition
(§4.2).

**S3. What a program reads is part of the definition too.** An object reads
blocks, heightmaps and biomes, and its draws depend on what it read (a tree
clips its height to the free space above it,
`feature/TreeFeature.java:141-158`; an ore vein draws a float per candidate
block when its air-exposure chance is fractional,
`feature/AbstractOreFeature.java:94-100`). So the state an object sees must be
pinned as precisely as the seed. This document pins it in Wn2 (§3.4): a unit
sees the *filled* window plus its own earlier writes, and never another unit's
scattering writes. That is a deliberate divergence from the reference, whose
units see whatever neighbours happened to be decorated first, i.e. the
player's route (§15 of `worldgen.md`, last paragraph). D2 in §9 decides it.

**S4. The write footprint is a hard bound, not a hint.** Every object's origin
lies in its own column (`placement/InSquarePlacement.java:23-25` jitters by
0..15 from the column origin; `count_on_every_layer` and `fixed_placement` do
the same, `CountOnEveryLayerPlacement.java:42-43`, `FixedPlacement.java:39-40`),
and the reference refuses any write beyond one column from it
(`server/level/WorldGenRegion.java:321-330, 333-350`, with the radius set at
`chunk/status/ChunkPyramid.java:34`). A write outside the footprint is a data
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
(`levelgen/GenerationStep.java:8-18`, or more if a datapack has longer lists,
`ChunkGenerator.java:407`); in each step it first places the structure starts
of that step with a counter of their own, then the step's features with the
sorted global index (`ChunkGenerator.java:410-434, 436-478`). Feature seeds do
not depend on the structure counter, so a pipeline that runs the feature half
alone produces the same features it will produce once structures exist.

---

## 2. Footprints

**What radius a tree needs.** Foliage radius is bounded by the codec at 16
(`feature/foliageplacers/FoliagePlacer.java:30`), trunk height at 32 plus two
random terms (`trunkplacers/TrunkPlacer.java:34-35, 62`), and the branching
placers add horizontal reach on top. The reference does not bound the tree by
its data; it bounds it by the write radius of the stage, one column
(`ChunkPyramid.java:30-36`), and drops the rest (S4). A tree reads free space
over a `minimum_size` footprint around the trunk (`TreeFeature.java:141-158`)
and rewrites leaf distances within its own bounding box
(`TreeFeature.java:213-216, 225-291`), so its read radius is the same one
column. `r_w = r_r = 1`.

**What radius an ore vein needs.** A modern vein spreads `size/8 ≤ 8` blocks
from the origin along a random direction and probes a box a further
`ceil((size/16·2 + 1)/2) ≤ 5` blocks wide (`feature/OreFeature.java:45-59`),
so it reaches at most thirteen blocks past an origin that lies inside the
column: `r_w = 1`. Before placing it scans the `OCEAN_FLOOR_WG` heightmap over
that box (`OreFeature.java:61-66`), and with a fractional air-exposure chance it
reads six neighbours per candidate (`AbstractOreFeature.java:75-92`): `r_r = 1`.

**Where the reference's radii come from.** Features require terrain at radius
1 and write at radius 1; light requires initialised light at radius 1
(`ChunkPyramid.java:30-40`). Terrain at radius 1 around features at radius 1
is the twenty-five-column window that §3.3 derives independently. The
structure-starts requirement at radius 8 on the same lines is the dependency
§10 of `worldgen.md` rejects, and it is absent here.

**The two heightmap generations.** The reference keeps two sets of maps
(`chunk/status/ChunkStatus.java:17-25`). `WORLD_SURFACE_WG` and
`OCEAN_FLOOR_WG` are maintained by the fill and by the surface stage
(`levelgen/NoiseBasedChunkGenerator.java:479-480`; surface writes go through
`ProtoChunk.setBlockState` while the status is still `biomes`,
`material/MaterialSystem.java:113-115`, `ProtoChunk.java:159-173`), and no
carver touches them. The four final maps are primed after carving
(`ChunkStatusTasks.java:153-161`) and updated on every decoration write
(`ChunkStatus.java:36-37` selects them for the `terrain` and `features`
statuses). So a placement that names a `_WG` map reads the column **before
carving**, and one that names a final map reads it **after carving and after
the column's own earlier objects**. `heightmap.md`'s appendix calls the pairs a
split by lifetime with identical predicates; the predicates are identical, the
contents are not, and 27 shipped placements read the pre-carve pair
(`grep -l _WG assets/minecraft/worldgen/placed_feature/*.json`: the grass
patches, the disks, both lava lakes, glow lichen, bamboo). `heightmap.md` §4
now states the two generations; Wn3 below carries the maps.

---

## 3. The scheduler

### 3.1 What is truth and what is derived

Classifying every value before storing it, per the project's ECS rule:

| Value | Category | Owner | Note |
| --- | --- | --- | --- |
| Seed, compiled feature tables (§4.3) | truth | `WorldgenFreeze` build | cloned into the sub-app once, like `DimensionRouters` (`generate/routers.rs:36`) |
| Which columns players want, at which target | derived from players | the dispatcher, each drain | `worldgen.md` §16: never stored |
| A column's stage | derived, but only recoverable by redoing the work | the staging store | an enum field, not a marker (X10); each section entity holds one `SectionStage` that moves `Loading → Generating → Loaded` |
| A filled column, its palettes, its six maps | materialised projection of the seed | the staging store | read by up to nine `run`s; recomputing is a whole fill |
| A unit's out-of-column writes (its deltas) | materialised, temporary | the staging store | consumed by one merge each, then dropped |
| The rank of a unit | pure function | nobody | computed where compared |

There is one owner for every materialised value: the scheduler resource that
already exists (`ColumnScheduler`, `chunk.rs:259-273`). It gains a third state
beside `pending` and `in_flight`, and the completion handler
(`process_completed_columns`, `chunk.rs:451-500`) gains a branch per stage. The
store lives where the scheduler lives — in the dimension sub-app
(`world/sub_app_builder.rs:354`), outside the ECS's entities (X1).

### 3.2 The stage ladder

Every column passes through four stages. Each is a separate task on the
generation pool, dispatched by the same drain that dispatches today
(`sub_app_builder.rs:271-306`, `chunk.rs:770-1157`).

```
Filled     fill, surface, carving, the six heightmaps, packed palettes
Run        the column's own program against its 3×3 window; own writes into
           a private buffer, out-of-column writes into eight deltas
Merged     own buffer + the eight incoming deltas, applied in rank order;
           the four final maps rebuilt; packed
Delivered  the sections enter the ECS as SectionStage::Loaded
```

`Filled` is today's task up to and including `into_sections`
(`chunk.rs:1007-1128`) plus the heightmap descent that follows it
(`chunk.rs:1130-1132`), run twice: once before carving for the pre-carve pair
and once after for the final four (Wn3). `Run` and `Merged` are new. `Delivered`
is today's completion handler.

**Q1. Dependencies, derived from C2.**

```
Run(U)       requires  Filled(V)  for every V with |V − U|∞ ≤ 1
Merged(U)    requires  Run(V)     for every V with |V − U|∞ ≤ 1
Delivered(U) requires  Merged(U)
```

Hence `Delivered(U)` requires `Filled` over the 5×5 around `U`: twenty-five
columns, the same accumulated window the reference pays to light a chunk
(`ChunkPyramid.java:30-40`; the `structure_starts` requirement excluded). It is
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
queue, apply in a separate phase — chosen over the in-place colouring of §13
for the reason in §3.3.

**Q3. The rank is the colour index of §13, fixed as data.** With
`r_w = r_r = 1` the §13 formula gives `d = 2`, `k = 3` and nine colours;
`rank(U) = (x mod 3)·3 + (z mod 3)` with Euclidean remainders, so that negative
coordinates colour like positive ones. Merging applies the nine writers of a
position in ascending rank, last write standing. Any pure function of the
coordinates would define *a* world; this one keeps §13's vocabulary and makes
the in-place executor of §6.4 comparable. Changing it is a different world
(§15). No configuration hash exists in the workspace yet; St3 owes one, and
its inputs are fixed here so the first version is complete: the world version
(5015), the data set's content hash, the precision profile, the unit size (one
column), the rank function (Q3), and the read rule (Wn2). A mismatch on any of
them means an untouched unit may not be regenerated from the seed.

**Q4. The write and the merge are the whole of N1.** In the delta form the
conflict predicate is never evaluated, so the bug N1 describes — dropping one
radius from `d` — cannot occur. What replaces it is the merge-order bug: a
writer applied out of rank, or a delta lost between eviction and merge. §6.3
is written against that.

### 3.3 Why not the colouring of §13

`worldgen.md` §13 derives the colouring for an in-place scattering stage and
notes that its barrier is paid at the maximum unit cost of the batch (N3).
There is a second cost it does not derive, and it is the one that decides the
design here.

In-place execution with the colour order means: `U` may run when every
conflicting unit of lower colour has run. With nine colours and `d = 2`, a unit
of colour 8 waits for its lower-coloured neighbours within two columns; each
of those waits for its own; the chain is strictly decreasing in colour, so it
has at most eight steps, each of at most two columns. Counting the lexicographic
order on `(x mod 3, z mod 3)`: the `x` residue can fall twice, the `z` residue
twice per `x` level, and a fall of a residue is a displacement of `+2` or `−1`;
the reach of a chain is four columns in `x` and sixteen in `z`. A streaming
dispatcher — one that pops the nearest wanted column
(`chunk.rs:901-907`) rather than decorating a closed batch — must therefore
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
per-worker `ColumnBlocks` (`chunk.rs:1001-1003`, `column_blocks.rs:16-48`);
F7 is honoured for the centre because it already is. For the ring, reads go
through the snapshots' paletted sections with a last-section cache, the shape
of the reference's `BulkSectionAccess` (`chunk/BulkSectionAccess.java:12-15`).
Whether the ring should be flat too is unmeasured; the ring is read by the
tree's free-space scan and the vein's probe and adjacency tests, and the count
of those reads per column is what decides it (measured in `PERF.md`). Unpacking
eight neighbours densely would be eight more buffers of 98,304 cells per
worker (`column_blocks.rs:29`), which T2 argues against without a measurement
for it.

**Wn2. The read rule.** A read at position `p` answers, in this order: the
unit's own earlier write to `p` if there is one, anywhere in the 3×3; else the
`Filled` block of the snapshot that holds `p`; else, outside the 3×3, air, with
a debug assertion. The reference logs a read outside the write zone as unsafe
(`WorldGenRegion.java:277-303`), so the corpus does not do it; the fixed
answer keeps the rule a pure function should a datapack do it anyway. Own
writes are found through a written-mask of one bit per cell over the 3×3
(nine times 12 KB per worker, cleared per unit) and, for the ring, the delta
entry the mask points at.

**Wn3. The window carries six maps per column.** Two pre-carve maps
(`SURFACE`, `SOLID` in the vocabulary of `heightmap.md` §2, answering
`WORLD_SURFACE_WG` and `OCEAN_FLOOR_WG`) frozen at `Filled`; and the four
final maps (`heightmap.md` §2, answering the four final names of
`Heightmap.java:153-168`), frozen at `Filled` for the ring and **live for the
centre**: every write into the centre updates them by the rule of
`heightmap.md` §5. A tree write is the cheap "rise" branch; an ore write
replaces stone with ore and moves nothing, which the §7 cut-off of
`heightmap.md` recognises in one predicate comparison; a lake removes blocks
and pays the descent, and lakes are few. After `Merged` the four final maps are
rebuilt once by the existing descent (`world/heightmap.rs:167-221`), because a
neighbour's delta can raise them too. The pre-carve pair needs one extra
descent per column before carving, which by the fast path of
`heightmap.md` §4 is one read per strip for most strips: unmeasured.

**Wn4. The biome set is computed once per `Run`.** F6 asks for once per batch;
here the batch is the unit, and the set is the union of the distinct entries of
every section palette of the nine snapshots
(`ChunkGenerator.java:392-400`; `VoxelPalette::for_each_distinct`,
`crates/mcrs_minecraft_chunk/src/voxel_palette.rs:79`), intersected with the
biome source's possible biomes as the reference does at line 400. The
block-resolution biome query the `biome` filter needs (F3, Cl4) reads the
window's stored palettes through the zoom
(`crates/mcrs_minecraft_biome/src/zoom.rs:10`); unlike the surface stage,
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

The queue keeps its key (`ColumnKey`, distance then position,
`chunk.rs:142-147`) and its shape (a `BTreeMap` popped from the front,
`chunk.rs:901-907`). What changes is what an entry asks for and when it is
ready.

**Q5. A request names a target stage, and requests propagate by Q1.** A view
that wants column `U` (`enqueue_pending_columns`, `chunk.rs:514-597`) asks for
`Delivered(U)`; the scheduler derives `Run` over the 3×3 and `Filled` over the
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
Cancellation keeps its current form (`cancel_stale_columns`, `chunk.rs:614-682`)
with "wanted" replaced by "wanted or within two of a wanted column".

**Q8. Delivery is the only point where a column becomes visible, and nothing
writes into it afterwards.** By Q1, when `U` is delivered every unit that can
write into `U` has run and its delta has been merged. This closes the hole that
§13 of `worldgen.md` describes: light reads its neighbours at radius one
(`epoch.rs:84-91`), and every neighbour it can see is either delivered — and
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
that type (`Feature.java:20-25`; the 58 registered types are listed at
`feature/FeatureTypes.java:8-65`; the corpus uses 47 of them, `tree` 45 times,
`simple_block` 37, `ore` 30, `random_selector` 21). In Rust that is one enum
with `#[serde(tag = "type")]`, the shape `ProtoChunkGenerator` already uses
(`crates/mcrs_minecraft_world/src/worldgen/chunk_generator.rs:50-59`). An
unknown type is a load error, as a 26.3 mismatch always is.

`worldgen/placed_feature/<id>.json` is a `PlacedFeature`: `feature` and
`placement` (`PlacedFeature.java:17-24`). `feature` is either an id or an
inline `Feature` — the reference's `Holder` codec — and so is every
`PlacedFeature` reference inside a selector feature
(`feature/RandomSelectorFeature.java:15-23`,
`WeightedPlacedFeature.java:13-20`; `trees_plains` in the corpus inlines two
and references one). That is the either-form of `worldgen.md`'s serde section:
a hand-written `Deserialize` with a symmetric `Serialize`, so the shape
round-trips unchanged; `BiomeSet` is the in-repo model
(`crates/mcrs_minecraft_worldgen_surface/src/proto.rs:130-160`). `placement`
is a list of `PlacementModifier`, a tagged enum over the eighteen registered
types (`placement/PlacementModifierTypes.java:11-28`; the corpus uses fifteen,
`biome` 211 times, `count` 201, `in_square` 198, `block_predicate_filter` 155,
`heightmap` 111).

Nested shapes the two registries pull in, each a serde type under
`crates/mcrs_minecraft_worldgen_feature/src/`:

| Shape | Reference |
| --- | --- |
| `IntProvider`, `HeightProvider`, `VerticalAnchor` | `util/valueproviders`, `levelgen/heightproviders` |
| `BlockState` (name or name plus properties) | `BlockState.CODEC` |
| `RuleTest` (`predicate_type`-tagged, ten types) | `structure/templatesystem/RuleTestType.java:8-17` |
| `BlockPredicate` (`type`-tagged, sixteen types) | `levelgen/blockpredicates/BlockPredicateType.java:8-35` |
| `BlockStateProvider` (ten concrete), `TrunkPlacer` (ten), `FoliagePlacer` (twelve), `TreeDecorator` (eleven), `FeatureSize` (two), `RootPlacer` (one) | `feature/stateproviders`, `trunkplacers`, `foliageplacers`, `treedecorators`, `featuresize`, `rootplacers` |

Where the type of a value depends on a sibling — every one of the shapes above
— the internally tagged enum is the answer, and no `DeserializeSeed` is needed:
the tag is a field of the same object. The seed case of `worldgen.md` (a map
key choosing the value's type) does not occur in these registries.

**Fe1. Validate at freeze, never at run.** The proto keeps ids; resolution to
indices, `VoxelId`s and tag masks happens once at `OnEnter(WorldgenFreeze)`,
the way carvers are resolved (`modern_carvers.rs:428-439, 447-470`) and routers
compiled (`routers.rs:45-91`), and the result is a resource cloned into each
dimension's sub-app. Every range the reference's codecs enforce is enforced
there: `count` 0..4096 (`CountPlacement.java:11`), ore `size` 0..64 and the
air-exposure chance 0..1 (`AbstractOreFeature.java:32-41`), foliage radius
0..16, offsets −16..16 (`OffsetPlacement.java:17-19`), environment-scan steps
1..32 (`EnvironmentScanPlacement.java:25`). An id that resolves to nothing — a
feature a biome names, a block a provider names, a tag a rule test names — is
a freeze error naming the asset. The freeze build resolves the biomes' ids the
way the carver build resolves carver ids (`modern_carvers.rs:461-470`).

A biome's step list is a `HolderSet`: either a `#tag` of placed features or a
list whose entries are ids or inline placed features
(`core/registries/codec/HolderSetCodec.java:45-46`,
`RegistryCodecs.java:14-18` with `allowInline` true; `alwaysUseList` only fixes
the serialised form). The shipped corpus uses ids only and ships no
`tags/worldgen/placed_feature/`, but 26.3 admits all three, so the proto is a
`Vec<FeatureStepList>` where a step list is the either-form of a tag or a list
of holders, and the current `Vec<Vec<ResourceLocation>>`
(`crates/mcrs_minecraft_biome/src/lib.rs:36`) changes. At freeze a tag
expands to its entries in the tag file's order, and an inline entry is a vertex
of its own in the sort (§4.3, D13). The biome asset declares no dependencies
(`biome/mod.rs:83-85`) and keeps not doing so; the feature registries load
through Fe2 and are joined by id at freeze.

**Fe2. Loading goes through the worldgen registries macro.** The four
registries a worldgen asset can name are spelled once
(`crates/mcrs_minecraft_worldgen/src/bevy.rs:108-200`) and grow the ids, the
handles, the dependency walk and the loader for each. `feature` and
`placed_feature` are two more rows: `placed_feature` is a nested registry
(it names features and, through selectors, other placed features), `feature`
is nested for the same reason. The protos live beside the existing ore port in
`mcrs_minecraft_worldgen_feature::place`; the compiled forms live next to them, in
the pattern of `material::proto` and `material::compile`
(`crates/mcrs_minecraft_worldgen_surface/src/compile.rs:42-47, 259`).

**Fe3. Beta's populate step is one feature.** Beta draws every object of a
chunk's populate step from one `LegacyRandom` seeded per chunk
(`beta_chunk_seed`), in the order `ChunkProviderGenerate.getChunkAt` fixes, and
where that stream stands when an object starts depends on what the objects
before it read: clay spends its vein draws only where it finds water. A placed
feature is seeded from the decoration seed, its step and its index alone, so no
split into one placed feature per object reproduces that stream. The step is
therefore one feature type, `mcrs:beta_populate`, which every `beta_*` biome
lists and the `Run` of §3.2 runs like any other feature; it seeds its own
stream and ignores the source its step hands it. Its table stays Beta code, and
its veins cross column borders as Beta's did.

### 4.2 The modifier chain

The reference's placer is an explicit stack machine (`FeaturePlacer.java:38-78`):
a stack of positions with the index of the modifier that should consume each;
the origin is pushed with index 0; the loop pops, runs one modifier, and pushes
that modifier's outputs **in reverse** so they pop in emission order
(lines 63-66); outputs of the last modifier go to the generator (lines 68-73).
The random source is shared by every modifier and by the generator (lines 60,
69), so the traversal order fixes the draw order and is part of the definition
(S5). An empty chain runs the generator at the origin directly (lines 45-50).

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
| `count`, `noise_based_count`, `noise_threshold_count`, `count_on_every_layer` | the count provider; the two noise variants sample a simplex noise with the fixed seed 2345 that no world seed enters (`biome/Biome.java:84-86`, `NoiseBasedCountPlacement.java:28-32`); the layer variant reads `MOTION_BLOCKING` and descends through blocks per layer (`CountOnEveryLayerPlacement.java:44-45, 61-83`) | count, then per-layer jitter |
| `in_square` | nothing | two |
| `height_range` | the height provider against the dimension extent (`HeightRangePlacement.java:32-39`) | the provider's |
| `heightmap` | one map at the origin's strip (`HeightmapPlacement.java:27-32`); which generation, per Wn3 | none |
| `surface_water_depth_filter` | `OCEAN_FLOOR` and `WORLD_SURFACE` at the strip (`SurfaceWaterDepthFilter.java:21-25`) | none |
| `surface_relative_threshold_filter` | one map at the strip | none |
| `block_predicate_filter` | blocks around the origin through a `BlockPredicate` (`BlockPredicateFilter.java:20-22`); `would_survive` is four tag rules by block family, §5.2 | a `random_*` rule test draws |
| `environment_scan` | a vertical run of blocks, at most 32 (`EnvironmentScanPlacement.java:52-76`) | none |
| `biome` | the block-resolution biome at the origin, then the biome's feature set (`BiomeFilter.java:21-30`) | none |
| `rarity_filter`, `random_chance` | nothing | one float |
| `offset` | nothing | three |
| `randomly_selected` | one of its sub-modifiers | one, then the sub-modifier's |
| `fixed_placement` | the origin's column | none |
| `cuboid` | nothing | the two size providers |

Every read above is inside the origin's column or within one column of it,
which is the `r_r = 1` of §2.

**Fe5. The step loop is the reference's, minus the structure slot.** For each
step from 0 to `max(11, longest biome list)`: the structure slot (empty until
§7); then the features of this step present in the window's biomes, ascending
by global index, each reseeded by S2 and placed with the biome check
(`ChunkGenerator.java:409, 436-478`).

### 4.3 The topological sort

F2 prescribes copying the algorithm, and this section says which lines. Input:
the biome source's possible biomes in the source's own order
(`ChunkGenerator.java:106-108` passes `List.copyOf(biomeSource.possibleBiomes())`),
and per biome its list of step lists. `FeatureSorter.buildFeaturesPerStep`
(`biome/FeatureSorter.java:28-125`):

1. Assign each distinct placed feature an index in order of first encounter,
   walking biomes in input order and steps in order (lines 33-34, 44-58).
   Identity is object identity (a `Reference2IntMap`); two biomes naming the
   same registry entry share one vertex, two inline copies of the same shape
   are two vertices.
2. Vertices are `(index, step, feature)` ordered by step then index (lines
   39-41); the edge map is a `TreeMap` in that order, and each biome's list
   contributes the edges `feature[i] → feature[i+1]` within its own list, with
   every vertex present even when it has no successor (lines 60-67).
3. Depth-first search over the edge map's key order, with the three-set
   colouring of `util/Graph.java:12-37`: a vertex is emitted after its
   successors, in post-order; a back edge is a cycle. The emitted list is
   reversed (line 112).
4. The result is split by step, keeping the sorted order within each step
   (lines 115-122); a feature's position in its step's list is the
   `index` of S2 (lines 127-130, used at `ChunkGenerator.java:450, 467`).

A cycle is a freeze error naming the biomes involved
(`FeatureSorter.java:83-108`: the reference's reduction loop is diagnostics,
not recovery).

**Fe6. The sort output is a per-dimension resource.** `possibleBiomes` differs
per biome source, so the tables are keyed by dimension exactly as routers and
carver tables are (`routers.rs:36`, `modern_carvers.rs:424`). Per step: the
sorted list of compiled placed features. Per biome: a bit set over each
step's list saying which of the step's features the biome carries — the union
across the window's biomes is `possibleFeaturesThisStep`
(`ChunkGenerator.java:437-458`), and the per-biome test is `hasFeature` for
the `biome` filter. The set is derived from the assets at freeze, so a
datapack that adds a feature to a biome is picked up without a code change.

**Fe7. The order of `possibleBiomes` is part of the definition for every
biome source.** The reference takes the distinct biomes in order of first
appearance in the parameter list (`biome/BiomeSource.java:32-33`,
`MultiNoiseBiomeSource.java:52-54`); the in-repo overworld and nether lists
keep that order and say so (`biome/overworld_preset.rs:7`). The other sources
in the workspace (`biome/source.rs:108-134`): `Fixed` is one biome;
`Checkerboard` is its list in file order; `TheEnd` is the reference's five end
biomes in the order `TheEndBiomeSource` lists them; `Beta` is the eleven land
biomes in `BetaLandBiome` discriminant order (`source.rs:18-30`); Beta has no
ocean biome. The Beta order is ours to fix,
since the reference has no such source; it is fixed here and covered by the
configuration hash (Q3).

### 4.4 Block access from an object

Objects see the window through one reader and one writer (Wn2, Wn5), with
world-absolute coordinates, the same shape the ore port already takes
(`mcrs_minecraft_worldgen_feature/src/place/mod.rs:18-29`). Beyond blocks a
generator needs: the six maps of Wn3, the block-resolution biome (Wn4), the
dimension extent (`value_provider.rs:11`), the block tag registry
(`DynTagRegistry`, taken at `chunk.rs:779`) resolved to masks at freeze, and
the property-transition tables a tree needs to rewrite a leaf's `distance`
(`TreeFeature.java:262`) — a state-to-state map per block, built at freeze
from the block definitions, since `try_resolve_state` already moves a stated
property (`chunk.rs:1218` tests it).

---

## 5. The first two consumers

### 5.1 Ore on the modern path

`ore` is 30 features and `scattered_ore` two; their placements are `count`,
`in_square`, `height_range` (uniform or trapezoid over anchors), `biome`, and
sometimes `rarity_filter`, as in `ore_coal_upper` and `ore_diamond` in the
corpus. From the assets a vein needs:

- `targets`: a list of `{target: RuleTest, state: BlockState}`
  (`feature/BlockReplacement.java:8-15`). The corpus's rule tests are
  `tag_match` over `stone_ore_replaceables`, `deepslate_ore_replaceables` and
  `height_specific_ore_replaceables`, `height_match` on `y`, and `any_of`,
  `all_of`, `not` over them — resolved to tag masks and ranges at freeze.
- `size` 0..64 and `discard_chance_on_air_exposure` 0..1.

From the window it reads the pre-carve `OCEAN_FLOOR_WG` map over its probe box
(`OreFeature.java:61-66`; the box reaches the ring), the block at every
candidate, and six neighbours per candidate when the chance is fractional.

The algorithm is `OreFeature.doPlace` (`OreFeature.java:72-198`): `size`
sphere centres interpolated along the segment with one double drawn each
(lines 92-103), the pairwise cull of a smaller sphere inside a larger one
(lines 105-123), and a per-box `tested` bit set that visits every position once
across spheres (lines 88, 148-153). The bit set is not an optimisation: with a
fractional air-exposure chance every visited candidate draws a float
(`AbstractOreFeature.java:94-100`), so visiting a position twice draws twice.

The existing `OreFeature` (`feature/mod.rs:64-148`) is the Beta algorithm —
no cull, no bit set, no probe, `+2` on the vertical endpoints — and stays what
it is. The modern vein is a second function beside it, not a flag on the
first: the loop structure differs, and `OreYOffset::ModernMinus2`
(`feature/config.rs:8-11`) is the only shared line.

### 5.2 Trees

`tree` is 45 features; `trees_plains` in the corpus is a `random_selector`
over `oak_bees_005`, `fancy_oak_bees_005` and `fallen_oak_tree`, placed with
`count` over a weighted list, `in_square`, `surface_water_depth_filter`,
`heightmap: OCEAN_FLOOR`, `block_predicate_filter: would_survive oak_sapling`,
`biome`. From the assets a tree needs (`TreeFeature.java:42-66`):

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
  `nextInt(599)` each (`treedecorators/BeehiveDecorator.java:58-68`). The draws
  sit inside that `ifPresent`, so a level without block entities would also
  skip them and shift every later draw; D6 gives the decorator a channel to
  write into so the draws always happen.
- `ignore_vines`, `root_placer` (mangrove only).

From the window it reads free space (`isFree`: air or the
`replaceable_by_trees` tag, `TrunkPlacer.java:116-120`, `TreeFeature.java:86-88`)
over the `minimum_size` footprint up to the tree's height
(`TreeFeature.java:141-158`), vines when not ignored, the block under a sapling
for `would_survive` (`blockpredicates/WouldSurvivePredicate.java:26-28`), and
its own bounding box during the leaf-distance pass
(`TreeFeature.java:225-291`). It writes logs, leaves with `distance` and
`persistent`, roots, and the decorators' blocks.

**`would_survive` is four rules over tags.** The corpus tests thirteen states
through it; in 26.3 each one's `canSurvive` is a tag test plus a shape, and the
tags are in the corpus (`assets/minecraft/tags/block/supports_*.json`):

| States | Rule | Reference |
| --- | --- | --- |
| the ten saplings, `firefly_bush` | the block below is in `supports_vegetation` | `block/VegetationBlock.java:19-21, 40-43`; `FireflyBushBlock` extends it (`FireflyBushBlock.java:16`) |
| `sugar_cane` | below is sugar cane; or below is in `supports_sugar_cane` and one of its four horizontal neighbours is in `supports_sugar_cane_adjacently` as a block or as a fluid | `SugarCaneBlock.java:86-104` |
| `cactus` | no horizontal neighbour is solid or lava; below is cactus or in `supports_cactus`; above is not liquid | `CactusBlock.java:112-123` |
| `mangrove_propagule` | below is in `supports_mangrove_propagule`; the hanging form tests `supports_hanging_mangrove_propagule` above | `MangrovePropaguleBlock.java:52-54, 72-76` |

The shapes are code, keyed by block family at freeze; the tags are data. A
`would_survive` naming a state outside these families is a freeze error with
the state's name (Fe1), and a fifth shape is added when a datapack needs it.
"Solid" and "liquid" come from the block definitions.

The reference records every write of a tree in four sets and runs the
leaf-distance pass over their bounding box afterwards
(`TreeFeature.java:172-219`). That pass reads the leaves of other trees inside
the box and lowers their `distance`; under Wn2 it reads the `Filled` ring,
where there are no leaves, and the own-column leaves it wrote itself. Then it
asks the level to update shapes on every face of the written shape (line 216;
`structure/templatesystem/StructureTemplate.java:477-500` calls `updateShape`
on each face cell and its outward neighbour). For leaves that is one more
distance recomputation from the neighbour (`block/LeavesBlock.java:108`); D7
says what is kept of it.

---

## 6. Verification

Bit for bit against the reference, where the reference computes without a
world — each fixture is a dump from `tools/vanilla-oracle` and carries
`WORLD_VERSION` 5015:

- the sort, per shipped biome source
  (`mcrs_minecraft_worldgen_feature/tests/feature_sort.rs`);
- ore geometry, `OreFeature.doPlace` over a flat all-stone array
  (`mcrs_minecraft_worldgen_feature/tests/ore_vein_parity.rs`);
- tree geometry, through the oracle's stub level
  (`mcrs_minecraft_worldgen_feature/tests/tree_geometry_parity.rs`).

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

---

## 7. Deferred: structures and the placement index

Structure materialisation and the placement index of §10 (G1) are not in this
document. The reasons:

- The index needs the `structure_set`, `structure`, `template_pool` and
  `processor_list` registries, of which only a stub exists
  (`crates/mcrs_minecraft_world/src/worldgen/structure_set.rs:8-13`), and
  jigsaw assembly on top.
- Terrain adaptation needs the density-graph leaf that B4 of `worldgen.md`
  owes at the same time as the first structure.
- Its load profile is the one §17 says must be measured apart from objects.

What this document leaves ready for it: the step slot of S6 and Fe5; and the
fact that piece materialisation runs inside `Run` under the ladder of §3.2 and
needs no fourth stage. Its write radius is one, not zero: the reference clips
template pieces to the chunk's own box (`ChunkGenerator.java:420-425`,
`getWritableArea`), but three hardcoded pieces write one column beyond it,
which Wn5 already routes into a delta. `structures.md` (M1, SD2) is the
specification.

---

## 8. Decisions

Every question the first draft left open is decided here. A decision that
depends on a measurement names the checkpoint that takes it; nothing else
waits.

**D1. The delta form is in force; §13 of `worldgen.md` is amended.** §3.3 is
the argument. `worldgen.md` §13 keeps its derivation as the derivation for an
in-place stage and says which form is in force, §14 names this document at
form 1, §18 qualifies its two colouring entries, and the appendix row for
scattering stages says "private writes merged in a fixed rank". Done in the
same change as this section.

**D2. The read rule Wn2 is the definition.** A unit sees the `Filled` window
and its own writes, never another unit's scattering writes. The reference's
alternative is route-dependent and was never reproducible; §6.4's number is
recorded when it exists and does not reopen the decision.

**D3. Two generations of two maps; `heightmap.md` is amended.** The pre-carve
`SURFACE` and `SOLID` maps are built by one descent after the surface stage and
before carving, live only in the staging snapshot, and are never stored: one
descent recovers them, and nothing after `Run` reads them. The four final maps
keep their lifecycle. `heightmap.md` §4 and its appendix say so now.

**D4. `possibleBiomes` order per source is fixed in Fe7.** Multi-noise as the
reference; Beta as `BetaLandBiome` discriminant order; the
choice is an input of the configuration hash (Q3).

**D5. `would_survive` is four tag rules keyed by block family** (§5.2). The
tags are data; the four shapes are code; a state outside the families fails at
freeze with its name. Expressing the shapes as data is not pursued: the
reference keeps them as code, and there is no datapack shape to be compatible
with.

**D6. Block entities are entities of their own, parented to the section; the
generated column carries them as data until delivery spawns them.** The anvil
reader already carries block entities per chunk
(`crates/mcrs_minecraft_anvil/src/chunk.rs:176`), the chunk packet already has
the entry (`mcrs_minecraft_protocol/src/chunk.rs:140-145`), and the block
schema knows which blocks have one
(`mcrs_minecraft_world/src/block/definition/schema.rs:464`); what is missing
is the runtime form. That form is an entity per block entity with a back-link
to its section, in the shape the column-to-section link already has: a
custom component naming the parent
(`mcrs_minecraft_level/src/world/storage/column.rs:20-25`, `InColumn`), a forward
index on the parent maintained by the same reconcile step that attaches the
back-link, and not the built-in hierarchy (X8 of `worldgen.md`). The section's
despawn (`lifecycle/ticket.rs:208-228`) despawns its block entities through
that index, since a custom link does not cascade. Position, kind and the
kind's state — for a bee nest, its occupants — are the entity's components,
typed per kind. NBT is a serialisation format and appears nowhere at run time:
the same typed state derives `Serialize`/`Deserialize` and is what
`mcrs_nbt` writes into the save and the chunk packet, and what the anvil reader
parses into, so one type serves the three encoders and no compound is held or
inspected in memory (the project's serde rule).

Generation never touches the ECS (X1), so the generated column gains
`block_entities: Vec<GeneratedBlockEntity>` beside its sections — position plus
a typed enum over the kinds a generator can produce — written by `beehive`
from step 6 so its draws happen (§5.2). The `Delivered` branch spawns one
entity per entry under the section entity it inserts, the anvil path spawns the
same entities from the saved list through the same types, and the packet
assembler serialises them back through the section's index. Until step 8 does
that, delivery drops the list with a counter.

**D7. Live-world block updates inside a feature are not reproduced.** The
face update after a tree (`TreeFeature.java:216`) and the post-processing
marks of disks, lakes and fallen trees (`Feature.java:50-61`;
`DiskFeature.java:77`, `LakeFeature.java:120, 148`,
`FallenTreeFeature.java:156`) are neighbour-shape semantics of the running
world: the leaf case is a `distance` recomputation the tree's own pass already
performed for every leaf it wrote (`TreeFeature.java:225-291`), and the
post-processing marks are resolved by the live world when the column becomes
full. What they change on a generated column is the `distance` of pre-existing
leaves at the face of a new tree, which under D2 are none. Listed under the
reference divergences of `worldgen.md`; not measured, because the set of
affected blocks is empty by construction under D2.

**D8. Unit is the column; rank is the colour index (Q3).** Both are inputs of
the configuration hash, whose inputs Q3 now lists. A larger unit is a different
world and is not revisited after the first shipped world.

**D9. Paletted snapshots in the store, a dense centre, a paletted ring**
(Wn1). Settled by measurement: `PERF.md` has the numbers, `DECISIONS.md` the
entry.

**D10. The 5×5 halo is accepted.** Its outer ring is read by the `Run`s of
the inner ring — free-space scans, vein probes, adjacency tests, maps — and a
cheaper `Filled` for it would be a second definition of a column. The
reference pays the same window (`ChunkPyramid.java:30-40`).

**D11. Beta populate beyond ore is later work inside `mcrs:beta_populate`.**
Lakes, dungeons, trees and flowers of Beta's populate step run in that feature
in Beta's order; they are outside this document's consumers, and until they
exist Beta's vein positions keep the divergence `apply_beta_ores_in` records.

**D12. Tree parity goes through a stub level in the oracle tool** (§6.1).
Ore parity goes through the verbatim harness. The method subset the stub needs
is discovered by running, not designed.

**D13. Inline and tag entries in biome step lists are supported** (§4.1).
An inline entry is a vertex per occurrence, as the reference's identity map
makes it (`FeatureSorter.java:33-34, 54`); a tag expands in tag-file order.
The corpus exercises neither, so both are covered by synthetic assets in the
round-trip and sort tests.
