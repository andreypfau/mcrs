# Structures: placement, layout, materialisation and search

A specification for §10 of `worldgen.md` and for the structure slot that
`scattering.md` leaves open (S6, §7): where a structure stands, what shape it
takes, how the terrain bends to it, how its blocks reach a column under the
ladder of `scattering.md` §3.2, and how a structure is *found* — by `/locate`,
by an explorer map, by an eye of ender, and by a searcher enumerating seeds.
Jigsaw assembly and the template system are the largest part of it because 28
of the 52 shipped structures are jigsaw structures and every one of them is
data.

The system is described once and instantiated per edition. Java 26.3 is the
primary reference and the only one with sources; Beta 1.7.3 is read from two
reimplementations; Bedrock is read from the wiki and from Microsoft's
behaviour-pack reference, which publish formats and behaviours but not the
random generator. §1.2 says what the three share and §10 says what each adds.

`worldgen.md` is the authority on invariants and vocabulary; `scattering.md`
owns the ladder, the window and the read rule (Wn1–Wn5) that materialisation
runs under; `heightmap.md` owns the maps. Reference paths are relative to
`~/src/gitlab.com/andreypfau/minecraft/src/main/java/net/minecraft`, version
`26.3-snapshot-10`, and are cited as data, never as a shape to copy. Beta paths
are relative to
`~/src/github.com/Project-Poseidon/src/main/java/net/minecraft/server` (Java)
and `~/src/github.com/BetrockPlusPlus/src/bpp_shared` (C++); SteelMC paths to
`~/src/github.com/SteelMC`. Where this document disagrees with a sentence of
`scattering.md` §7, this document is right and §16 below records the
amendment.

Every count below is a count; nothing in this document has been measured.

---

## 1. The system

### 1.1 Layers

A structure raises six questions the rest of the pipeline never asks together,
and the first move is to keep them apart, because each has a different answer
to the classification test of the ECS rule (truth, derived, materialised,
temporal) and a different cost.

| Layer | Question | Function of | Category | Section |
| --- | --- | --- | --- | --- |
| Placement | Is chunk `C` a start chunk of set `S`? | the low 48 bits of the seed, `C`, `S` | pure; three draws | §2 |
| Selection | Which structure of `S` is tried, in what order? | the same, plus the sites' verdicts | pure | P6 |
| Site | Where does it stand, and is the biome right? | the seed, `C`, the climate field, the *unbearded* density column | pure; one biome sample, at most one column | §3 |
| Layout | What are its pieces? | the seed, `C`, the registries, the same two fields | pure; expensive; memoised | §3, §4 |
| Adaptation | How does the terrain bend? | the pieces of nearby starts | a term of the density field | §6 |
| Materialisation | Which blocks does column `U` get? | the pieces, the filled window of `U` | a program of the scattering stage | §7 |
| Search | Where is the nearest start; which seeds admit one here? | the first four layers | derived; never stored | §9 |

The first four never read a block. That is the whole content of G1 and it is
worth restating in the reference's own terms: `createStructures` runs at the
`structure_starts` status, which has no requirement on any other chunk
(`world/level/chunk/status/ChunkPyramid.java:13`), and everything a layout
consults is either a registry, the climate field
(`structure/Structure.java:289-331`), or the density column evaluated **without**
a terrain adaptation term (`levelgen/NoiseBasedChunkGenerator.java:157-166`,
which passes `null` for it, against `:105-121`, which passes the real one for a
fill). So a start depends on no other start, the layouts of two cells are
independent, and the index of §8 is a pure function of the seed that can be
computed in any order on any thread. Search (§9) reads the same four layers
and nothing else, which is why it needs no world.

Only materialisation touches the world, and it is the one `scattering.md`
already answers: a structure's blocks are placed by the column's own program,
in the step slot S6 reserves, under the read rule Wn2.

### 1.2 Editions

**E1. One pipeline; an edition is data plus two functions.** The layers of
§1.1 are shared. An edition supplies (a) the *placement function* — the
lattice, its random generator and its seeding (§2 for Java; §10.2 for what is
known of Bedrock; nothing for Beta); (b) the *decoders* — the serde dialect of
the four registries and the template file format (§5, §10.2, Appendix D); and
(c) the *set of layout generators* it ships (L4). Nothing else in §3–§9 is
edition-specific, and no shared layer branches on an edition: the index takes
a placement function, the template store takes a decoded template, the
materialiser takes pieces. A world preset names its edition profile the way it
names its noise settings; there is no runtime switch.

**E2. Parity is a property of a profile and is stated per profile.**

| Aspect | Java 26.3 | Beta 1.7.3 | Bedrock |
| --- | --- | --- | --- |
| Placement | `random_spread`, `concentric_rings`, `dimension_origin` (P1) | none: no structure sets exist (§10.1) | `random_spread` with the same lattice fields; `concentric_rings` declared without documented fields (§10.2) |
| Placement random | `LegacyRandom`, 48-bit, additive salt (P2) | — | unpublished; positions differ from Java for the same seed by the wiki's own statement |
| Selection | weighted draw with removal (P6) | — | weighted list documented; retry semantics unknown |
| Layout | 28 jigsaw + 15 hardcoded types (L4) | dungeons are a decoration feature, caves a carver (§10.1) | jigsaw, data-driven for trail ruins only; the rest "legacy" and closed |
| Templates | `structure/*.nbt`, pinned at data version 5011 (T1) | none | `.mcstructure` (§10.2) |
| Processors | eleven types (T5) | none | four types |
| Adaptation | four shapes (§6) | none | the same four names |
| Reproducibility here | starts and layouts bit for bit (§13) | dungeons and caves bit for bit | formats round-trip; positions **not** reproducible (SD14) |
| Search | `/locate structure`, maps, eyes, dolphins (§9) | nothing to find | `/locate structure [useNewChunksOnly]` |

**E3. The seed's width is a profile fact.** Java's placement and layout draws
see the low 48 bits of the seed and its climate and density fields see all 64
(R1). Beta's *entire* generator is `java.util.Random` — the terrain octaves
(`ChunkProviderGenerate.java:33-41`), the biome octaves seeded by
`seed · 9871`, `seed · 39811`, `seed · 543321` (`WorldChunkManager.java:18-20`),
the populate and cave seeds (§10.1) — so a Beta world is a function of the low
48 bits of its seed and the top 16 are inert. Bedrock's width is unknown. A
seed searcher (R3) is built on this fact and reads it from the profile.

### 1.3 What is truth

Nothing a structure produces is truth except one counter. The registries are
truth (assets), the seed is truth, and every position, piece and block is a
projection of them until a player edits it (`worldgen.md` §16). The one
exception is a start's `references` — how many explorer maps have claimed it
— which gameplay writes and the seed cannot recover (L6, I1). I1 classifies
every stored value.

---

## 2. Placement

**P1. A structure set is a lattice rule plus a weighted list.** The registry
`worldgen/structure_set/<id>.json` (21 shipped files) is a list of
`{structure, weight ≥ 1}` and a `placement` dispatched on `type`
(`structure/StructureSet.java:12-47`; `placement/StructurePlacements.java:10-12`
registers `random_spread`, `concentric_rings`, `dimension_origin`). The two
spreading types share `salt` (required, non-negative), `frequency` (0..1,
default 1), `frequency_reduction_method` (default `default`), `locate_offset`
(each axis within ±16, default zero) and an optional `exclusion_zone` of
`{other_set, chunk_count 1..16}`, which the reference marks deprecated
(`placement/AbstractSpreadingStructurePlacement.java:29-49, 148-167`).
`random_spread` adds `spacing` and `separation`, both 0..4096 with
`spacing > separation` enforced by the codec, and `spread_type` `linear` or
`triangular` (`placement/RandomSpreadStructurePlacement.java:15-39`).
`concentric_rings` adds `distance` 0..1023, `spread` 0..1023, `count` 1..4095
and a biome set `preferred_biomes` (`placement/ConcentricRingsStructurePlacement.java:36-43`).
`dimension_origin` has no fields — it is not a spreading placement and carries
no salt, frequency, offset or exclusion — and matches the *dimension origin*
only (`placement/DimensionOriginStructurePlacement.java:15-18`). The origin is
chunk `(0, 0)` (`world/level/chunk/ChunkGenerator.java:117-119`) unless the
noise settings carry a non-empty `spawn_target`, in which case it is the chunk
of the spawn-target search over the climate field
(`levelgen/NoiseBasedChunkGenerator.java:130-141`), a function of the full
seed. No shipped set uses it. Every range is a freeze check (Fe1).

**P2. The cell function.** For `random_spread`, with chunk coordinates `(x, z)`:

```
gx = floorDiv(x, spacing);  gz = floorDiv(z, spacing)
r  = LegacyRandom(gx·341873128712 + gz·132897987541 + seed + salt)
ox = spread(r, spacing − separation);  oz = spread(r, spacing − separation)
cell(gx, gz) = (gx·spacing + ox, gz·spacing + oz)
starts_at(x, z) ⟺ cell(gx, gz) = (x, z)
```

`floorDiv` rounds toward negative infinity; the seeding is
`setLargeFeatureWithSalt` (`levelgen/WorldgenRandom.java:66-69`) over a
`LegacyRandomSource`, the 48-bit congruential generator
(`levelgen/LegacyRandomSource.java:31-49`), never Xoroshiro; `x` is drawn before
`z`; `linear` is one `nextInt(limit)`, `triangular` is the integer mean of two
(`RandomSpreadType.java:23-28`; `RandomSpreadStructurePlacement.java:84-93`).
Three draws and one hash per cell: the cheapest thing in this document, and
the thing R2 takes apart bit by bit.

**P3. Three gates, in this order, each a pure function of the seed.**
`isStructureChunk` is the conjunction of the cell test, the frequency gate and
the exclusion gate, short-circuited in that order
(`AbstractSpreadingStructurePlacement.java:89-94`). The frequency gate runs only
when `frequency < 1` and has four spellings, which are data because the corpus
uses three of them (`:113-146`):

| Method | Seed | Test | Corpus user |
| --- | --- | --- | --- |
| `default` | `setLargeFeatureWithSalt(seed, salt, x, z)` — the salt takes the `x` slot and `z` becomes the blend | `nextFloat() < f` | none |
| `legacy_type_1` | `setSeed((x ≫ 4) ^ ((z ≫ 4) ≪ 4) ^ seed)`, then one `nextInt()` discarded | `nextInt(⌊1/f⌋) == 0` | pillager outposts |
| `legacy_type_2` | `setLargeFeatureWithSalt(seed, x, z, 10387320)` | `nextFloat() < f` | buried treasure |
| `legacy_type_3` | `setLargeFeatureSeed(seed, x, z)` | `nextDouble() < f` | mineshafts |

The argument order of `default` is not a typo to correct: it is the function.
The exclusion gate asks whether the other set has a start anywhere in the
`(2n+1)²` square around `(x, z)`, by running the other set's full three-gate
test on every chunk of that square (`ChunkGeneratorStructureState.java:224-238`).
A set may exclude a set that excludes it; the reference would recurse forever
and SteelMC panics on the cycle. A cycle is a freeze error (§11). One more
fact about the gates that §9 needs: the reference's *cheap* presence test runs
the cell test and the frequency gate but not the exclusion gate
(`structure/StructureCheck.java:112`); the exclusion gate is re-applied when
the chunk is generated. Here the three gates are one function and every
consumer sees all three.

**P4. Rings are a per-dimension constant.** For `concentric_rings` the set of
start chunks is computed once per dimension from the seed alone
(`ChunkGeneratorStructureState.java:131-196`): a `LegacyRandom` seeded with the
level seed (zero for flat worlds, `:57, :70`) draws an angle, then for each of
`count` positions a distance `4·d + 6·d·ring + (nextDouble() − 0.5)·2.5·d` in
chunks along the current angle, forks a child source, and snaps the position
to the nearest chunk whose biome is in `preferred_biomes` within 112 blocks of
the candidate's centre by the reference's horizontal biome search with
reservoir sampling on the fork (`world/level/biome/BiomeSource.java:47-57,
103-166`); the angle advances by `2π/spread`, and when a ring fills, `spread`
grows by `2·spread/(ring+1)`, is clamped to what remains, and the angle jumps by
a fresh draw. Strongholds are 128 positions on rings of 3, 6, 10, 15, … with
`distance 32`. The fork per position is what makes the biome searches
independent; the reference fans them out to a pool, and so may we, after the
forks are drawn in sequence. The angles and distances are 48-bit facts; the
snap reads the climate field and is a 64-bit one (R1).

**P5. Sets are filtered by the dimension, structures by the biome at the
site.** A set is live in a dimension when at least one of its structures names
a biome the dimension's source can produce
(`ChunkGeneratorStructureState.java:67-79`); within a live set a structure is a
candidate when it names such a biome (`:101-129`, which also builds the
placement list per structure — a structure in two sets has two placements,
and search groups by placement for that reason). The biome test proper happens
after the site is chosen, at the site (§3, L2). The `possibleBiomes` of a
source is the set D4 of `scattering.md` already fixes.

**P6. Selection within a set is a draw with removal.** In a start chunk of a
set with one entry, that entry is tried. With several, a `LegacyRandom` seeded
by `setLargeFeatureSeed(seed, x, z)` (`WorldgenRandom.java:58-64`: reseed, two
`nextLong`s, then `x·a ^ z·b ^ seed`) draws `nextInt(total weight)`, walks the
list in JSON order subtracting weights, tries the entry it lands on, and on
failure removes it, lowers the total and draws again from the same stream
until an entry yields a valid start or the list is empty
(`ChunkGenerator.java:592-635`). So the JSON order of the entries is part of the
definition, and the number of failed tries is too: it shifts the stream. The
structure tried gets its own fresh `LegacyRandom` seeded the same way
(`Structure.java:283-287`), so a layout never sees the selection draws. One
chunk holds at most one start per *structure*, and a set whose structure
already has a valid start in the chunk is skipped (`:568-575`), which is
unobservable under P7 because the index computes each chunk exactly once. The
one set with many entries is `abandoned_camp` (18 camps, weight 1 each): a
cell of that set almost always places *some* camp, whichever biome-valid
variant survives the loop.

**P7. The index is the placement.** `worldgen.md` §10 rejected placement as a
stage; nothing in P1–P6 reads a column, so nothing needs one. The reference's
`structure_starts` status, its radius-8 requirement on four later statuses
(`ChunkPyramid.java:14-32`) and its `structure_references` scan
(`ChunkGenerator.java:689-733`) are how a per-chunk store answers "which starts
reach me"; §8 answers the same question from the seed.

---

## 3. Layout: from a placed cell to a start

**L1. A start is a chunk, a structure and a non-empty list of pieces.** The
reference's `StructureStart` is exactly that plus a counter
(`structure/StructureStart.java:28-31`); it is valid iff it has a piece
(`:127-129`), and its bounding box is the union of the piece boxes, inflated by
12 on every axis when the structure has a terrain adaptation
(`:75-83`; `Structure.java:86-88`). A layout that produces no piece is a
rejection, and a rejection returns to P6's loop.

**L2. The site is chosen first, tested for biome second, and the pieces are
built third.** `findGenerationPoint` yields a position and a deferred piece
builder; the biome of the *chosen position*, sampled at quart resolution
including its `y`, must be in the structure's `biomes`; only then is the
builder run (`Structure.java:118-128, 235-237, 289-300, 334-348`). The `y` is
not decorative: a stronghold is tested at `y = 0`, a fortress at 64, a
mineshaft at `50 + offset`, a surface structure at the first occupied height
of its heightmap. One structure builds its pieces *before* the test: the
mineshaft carries a filled builder in its stub (`structures/MineshaftStructure.java:36-76`),
so its draws happen whether or not the site passes. For a jigsaw structure the
site step is exactly: the `start_height` draw, the centre rotation, the centre
element, the optional named-jigsaw shuffle, the centre's box from the
template's *size*, the optional column-biome test and free-height query, and
the padding test (`pools/JigsawPlacement.java:51-141`); every piece after the
centre is built inside the deferred closure (`:141-186`). The site is a
necessary condition for a start and is what search pays for (R3, R6); the
layout is what fill and run pay for.

**L3. Heights come from the density column, not from blocks.** Every height a
layout reads is `getBaseHeight`: the density tape evaluated over one strip,
substance applied by the aquifer, walked top-down until the heightmap
predicate holds, with no beardifier and no blending
(`NoiseBasedChunkGenerator.java:157-166, 204-252`). `WORLD_SURFACE_WG` stops at
the first non-air cell, which includes water; `OCEAN_FLOOR_WG` at the first
motion-blocking one (`levelgen/Heightmap.java:28-31, 153-156`). The helpers are
few and all corner-shaped: the chunk-centre column
(`Structure.java:138-159`, at block 8,8), the four corners of a box for a mean
or a minimum (`:172-213`), and the four corners of a 5×5 box offset by the
rotation (`:215-231`, used by end cities and mansions). The consequence is the
one that matters: **a layout's heights ignore carving, surface rules and every
other structure**, so the layout is a function of the seed and of nothing that
any stage of the ladder writes. Under §7 the *materialised* piece will then sit
on carved, surfaced ground that the layout never saw; that is the reference's
behaviour too, and the beardifier of §6 exists to close the gap.

**L4. A jigsaw structure is data; the other fifteen types are algorithms whose
draws are the data.** `structure/StructureType.java:24-41` registers sixteen
`type`s. `jigsaw` reads its whole layout from the registries (§4). The others —
`buried_treasure`, `desert_pyramid`, `end_city`, `fortress`, `igloo`,
`jungle_temple`, `mineshaft`, `nether_fossil`, `ocean_monument`, `ocean_ruin`,
`ruined_portal`, `shipwreck`, `stronghold`, `swamp_hut`, `woodland_mansion` —
are Java methods whose observable content is the sequence of draws they make
and the boxes they emit. For those the prescription of F2 applies: the draw
order is copied verbatim, because a draw moved by one changes every piece
after it. What is *not* copied is the object graph: a piece is data (L6), a
generator is a function from a seeded source to a piece list, and the static
mutable tables the reference keeps between calls
(`structures/StrongholdPieces.java:68-82`, `NetherFortressPieces.java:33-49`
with their `placeCount`s) are locals of that function. Their per-type
parameters are the `type`-specific JSON fields: `mineshaft_type`, `is_beached`,
`biome_temp`/`large_probability`/`cluster_probability`, the ruined portal's
`setups`, the fossil's `height`; everything else is `StructureSettings`:
`biomes`, `spawn_overrides`, `step`, `terrain_adaptation`
(`Structure.java:359-377`). The complete list of what a layout may read is the
reference's generation context (`Structure.java:241-254`): the registries, the
height and biome queries, the template manager, the 48-bit random of P6, the
raw seed, the chunk, the dimension's height range and the biome predicate —
and nothing that is not a function of those.

**L5. Every structure has a reach, and it is at most eight chunks.** A piece
farther than eight chunks from its start chunk is not part of the world: the
reference's reference scan stops there (`ChunkGenerator.java:689-694`;
`ChunkStatus.MAX_STRUCTURE_DISTANCE`), and a jigsaw structure's
`max_distance_from_center.horizontal` plus its 12-block adaptation margin may
not exceed 128 (`structures/JigsawStructure.java:33, 74-82`). Fortresses and
strongholds bound their pieces to 112 blocks from the start
(`NetherFortressPieces.java:3059-3091`, `StrongholdPieces.java:216-242`),
mineshafts to 80 (`MineshaftPieces.java:85-112`), and end cities and mansions
are bounded only by the cap. The index of §8 needs the bound *before* the
layout exists, and takes the reference's: every structure is looked for
within eight chunks. A tighter reach derived from `horizontal + margin` is not
a bound, because the pieces are held within that distance of the centre
piece's box centre, not of the start chunk, and a rotated centre piece moves
that centre outside the chunk; such a reach can fall a chunk short and drop
the start from the columns at the edge of its box. The actual bounding box,
once known, is what the query filters on. The constant is older than structures: Beta's cave map-gen
already visited a radius of 8 (§10.1, B3).

**L6. A piece is its serialised form.** The reference writes each piece as
`id`, `BB`, `O`, `GD` plus type-specific fields, and a jigsaw piece as its
element, position, rotation, `ground_level_delta`, junctions and liquid
settings (`structure/StructurePiece.java:90-99`;
`structure/PoolElementStructurePiece.java:73-92`); the start as `id`,
`ChunkX`, `ChunkZ`, `references`, `Children`
(`StructureStart.java:110-125`). Under the serde rule that layout *is* the Rust
type — one enum over piece kinds, each variant a typed struct, deriving the
codec that writes the save and the one that would cross the wire — and there
is no second in-memory shape. Two fields carry more than their name: `GD` is a
collision tag for end cities and a room flag for monuments
(`structures/EndCityPieces.java:507-515`) and is stored verbatim; `references`
is gameplay truth (how many explorer maps have claimed the start, at most one,
`StructureStart.java:135-149`) and is the one field a layout never sets.

---

## 4. Jigsaw assembly

The corpus: 245 template pools, 40 processor lists, 28 jigsaw structures,
1511 templates. This section is the reference's `JigsawPlacement` as a
definition rather than a class; the algorithm is copied draw for draw (L4),
the data structures are not.

**J1. The structure's fields.** `start_pool`, optional `start_jigsaw_name`,
`size` 0..20 (the depth), `start_height`, `use_expansion_hack`, optional
`project_start_to_heightmap`, `max_distance_from_center` as an int 1..128 or
`{horizontal 1..128, vertical 1..4064}`, `pool_aliases` (default empty),
`dimension_padding` as an int or `{bottom, top}` (default 0), `liquid_settings`
(default `apply_waterlogging`) (`JigsawStructure.java:36-62, 191-213`;
`pools/DimensionPadding.java:9-22`). A pool is `fallback` plus `elements` of
`{element, weight 1..150}`, and the weights are *expanded*: the pool's working
list holds each element `weight` times in JSON order, so a weight changes both
the odds and the number of draws a shuffle spends
(`pools/StructureTemplatePool.java:29-65`). An element is dispatched on
`element_type`: `single_pool_element` and `legacy_single_pool_element`
(`location`, `processors`, `projection`, optional `override_liquid_settings`),
`list_pool_element`, `feature_pool_element`, `empty_pool_element`
(`pools/StructurePoolElementType.java:8-14`). The legacy form differs from the
single form in one thing: its ignore processor drops air as well as structure
blocks and sits *last* in the chain, so a legacy template never carves the
terrain (`pools/LegacySinglePoolElement.java:32-43`). Aliases are `direct`,
`random`, `random_group`, resolved in list order into a map by a
`LegacyRandom` positioned at the pre-projection start position
(`pools/alias/PoolAliasLookup.java:19-32`); a duplicate alias is a freeze error.

**J2. One stream, in one order.** The layout random is the start's
`LegacyRandom` of P6. Its first draw is `start_height.sample`
(`JigsawStructure.java:158`); then, in `addPieces`
(`pools/JigsawPlacement.java:51-186`): the centre rotation (`nextInt(4)` over
`NONE, CW90, CW180, CCW90`); the centre element (`nextInt(expanded size)`);
if `start_jigsaw_name` is set, the centre's jigsaw blocks are *shuffled* with
this stream before the named one is found (J4). Then the queue of J3 runs and
each `tryPlacingChildren` spends, per source jigsaw: a shuffle of the target
pool's expanded list **only if `depth ≠ size`**, a shuffle of the fallback's
expanded list **always**, and per candidate element a shuffle of the four
rotations (three draws) and a shuffle of the candidate's jigsaw blocks
(`:404-420`; `util/Util.java:1161-1168` is the Fisher–Yates from the end,
`n−1` draws). Skipping the fallback shuffle at maximum depth because "it cannot
matter" desynchronises every draw after it; SteelMC records the same lesson
for duplicate pool entries (`steel-worldgen/src/structure/jigsaw.rs:387-392`).

**J3. The queue is priority, then FIFO, and higher priority pre-empts.** The
centre piece's children are tried first; every accepted piece with
`depth + 1 ≤ size` is enqueued under its source jigsaw's `placement_priority`;
the next piece processed is the first of the highest non-empty priority
(`JigsawPlacement.java:221-263`; `util/SequencedPriorityIterator.java:18-67`).
A piece at depth `size` is still processed but sees only fallbacks (J2). The
piece list is the centre followed by acceptance order, and the first piece is
what materialisation's reference position is taken from (§7).

**J4. A jigsaw block list is sorted by selection priority, ties in shuffle
order.** The blocks of a template are the palette's `minecraft:jigsaw` entries
in template order (T2), transformed to the piece's rotation, shuffled with the
stream, then stably sorted by `selection_priority` descending
(`pools/SinglePoolElement.java:39-42, 124-141`). The palette used to enumerate
them is chosen by a positional random of the piece position, and for a
*candidate* the position is the origin, so the palette is index
`hash(0,0,0) mod n` — which can differ from the palette later placed at the
real position (`StructurePlaceSettings.java:110-116, 138-147`; `util/Mth.java:367-371`).
Twenty of the 1511 templates carry several palettes. The quirk is data.

**J5. Attachment.** For each source jigsaw, with target position
`source + front`, a candidate `(element, rotation, jigsaw)` attaches iff
`canAttach`: the target's front is the source's opposite, the tops agree or
the source's joint is `rollable`, and the source's `target` equals the target's
`name` (`world/level/block/JigsawBlock.java:79-91`); the first candidate that
also fits (J6) wins and the loop moves to the next source jigsaw
(`JigsawPlacement.java:463-582`). An `empty_pool_element` met in the candidate
list *breaks* the list: nothing behind it is tried (`:412-415`). The vertical
placement is where projection enters: with both sides `rigid`, the target box
is placed at `source box minY + Δy` where `Δy = sourceJigsawLocalY −
targetJigsawLocalY + front.stepY`; with either side `terrain_matching`, the
source jigsaw's column is queried once (L3, `WORLD_SURFACE_WG`, the *free*
height, i.e. one above the occupied) and the target box's `minY` is that height
minus the target jigsaw's local `y` (`:474-498`). The `ground_level_delta` of a
rigid target is the source's minus `Δy`; of a terrain-matching one, the
element's own (always 1) (`:520-526`). Every accepted attachment records two
junctions, one on each piece, holding the *other* jigsaw's `x`, `z`, a ground
`y` and `Δy` (`:537-574`); they are the only junctions there are and §6 reads
them.

**J6. Occupancy is inclusive integer box arithmetic.** The reference tests a
candidate box, deflated by a quarter block, against a voxel shape that starts
as the reach box minus the centre piece and loses every accepted box
(`:143-182, 508-519`). For integer boxes the deflation makes the test exact:
a candidate fits iff its box lies within the context's bounds and intersects no
box already subtracted from that context, with `BoundingBox.intersects`
semantics (both ends inclusive). There are two kinds of context: the lineage's
shared one, whose bounds are the reach box `[c − h, c + h]` horizontally and
`[max(c − v, minY + bottomPad), min(c + v, maxY − topPad)]` vertically
(`:143-158`), and a per-piece interior one, created the first time a target
position falls *inside* its source piece's box, bounded by that box
(`:393-402`). A subtracted box is subtracted from the context the attachment
used, and children of a piece inherit the context their attachment used. The
expansion hack grows a candidate's *occupancy* box upward to
`max(expandTo + 1, height)` when the flag is on and the candidate is at most 16
tall, where `expandTo` is the tallest element of any pool a jigsaw of the
candidate would attach inside it (`:424-461, 499-506`). The grown box is not
only occupancy: it is the box the accepted piece is constructed with and keeps
(`PoolElementStructurePiece.java:36-52`), so it is what the start's box unions
(L1) and what the beardifier reads (§6); only a piece reloaded from the save
recomputes its box from the element (`:56-66`). SteelMC's `DeflatedQuarters` octree
(`steel-worldgen/src/structure/box_octree.rs`) is a port of a Fabric mod's
optimisation, not of the reference; a list of boxes per context is the
definition, and whether a search structure pays is measured on a village.

**J7. The centre is projected, then padded, then checked.** With
`project_start_to_heightmap` the whole column at the centre's `(x, z)` must
admit a valid biome (L2's column form), and the centre's ground row
(`minY + ground_level_delta`) is moved to `start_height + freeHeight(x, z)`
(`:112-129`); `dimension_padding` then rejects a centre whose box crosses the
padded floor or ceiling (`:130-137`); the biome test of L2 runs at
`(centreX, bottomY + anchorY, centreZ)` (`:139-142`).

**J8. Nothing in J1–J7 reads a block.** The only world queries are L3's height
and the biome field. Hence a layout is reproducible against the reference bit
for bit given the same seed and registries, and §13 tests it so.

---

## 5. Templates and processors

**T1. A template is an NBT file and its Rust type is that file.**
`structure/<path>.nbt`, gzip-compressed, root keys `size`, `blocks`
(`pos`, `state`, optional `nbt`), `palette` or `palettes` (20 files), `entities`
(172 files), `DataVersion` (`templatesystem/StructureTemplate.java:726-843`).
The type derives `Deserialize`/`Serialize` through `mcrs_nbt`; the palette
entries resolve to `VoxelId` at freeze. Every shipped template carries
`DataVersion` 5011 against a world version of 5015. The reference runs the
structure datafixer over that gap (`templatesystem/loader/TemplateSource.java:81-88`);
the fixers registered between 5011 and 5015 rename explorer-map items and
strip one data component (`util/datafix/DataFixers.java:2014-2038`), and a scan
of all 1511 files finds neither. So the template version is pinned at 5011 as
a named constant and checked at load; a file at any other version is a loud
error, as the project rule demands, and the pin moves when the corpus does.
The Rust type is the template, not the file: a second decoder reads Bedrock's
`.mcstructure` into the same type (§10.2, K3), and nothing downstream knows
which file it came from.

**T2. Block order is part of the definition.** After load the blocks are split
into three lists — with `nbt`; else "full" (collision shape is a full block
and not dynamic); else "other" — each sorted by `(y, x, z)` and concatenated
full, other, block-entity (`StructureTemplate.java:152-186, 845-867`). This
order is the jigsaw enumeration order of J4 and the placement order of T4; it
is computed once at freeze and is why the block predicate "is a full block"
must resolve at freeze from the block schema.

**T3. Transform.** Mirror first (`LEFT_RIGHT` negates `z`, `FRONT_BACK`
negates `x`), then rotation about the pivot `(px, pz)`:
`CCW90 → (px − pz + z, y, px + pz − x)`, `CW90 → (px + pz − z, y, pz − px + x)`,
`180 → (2px − x, y, 2pz − z)` (`:630-657`); entities use the continuous form
with `+1` terms (`:659-684`). A piece's bounding box is the transformed box of
`(0,0,0)`–`(size − 1)` moved to the piece position (`:706-724`), so for a
rotated piece the box minimum is not the position, which is why J5 tracks both.
Jigsaw pieces never mirror and always pivot at zero (`SinglePoolElement.java:184-205`).

**T4. Placement is one pass in one order with one clip.** `placeInWorld`
(`:285-466`): choose the palette (J4's positional draw at the piece position);
for each block in T2 order compute its world position and **skip it before any
processor runs** if it lies outside the placement box — unless a `capped`
processor is in the chain, which sees the whole piece (`:510-532`); run the
processors in chain order until one drops the block; run every processor's
`finalizeProcessing` in chain order; then set the survivors, block entities
with a barrier placed first, loot seeds drawn from the *placement* random, and
waterlogging by the liquid rules (`:323-407`). The chain for a jigsaw piece is
`[block_ignore(structure_block), jigsaw_replacement, …processor_list…,
gravity if terrain_matching]`, update flags 18, shape updates disabled by
`knownShape` (`SinglePoolElement.java:167, 184-205`). D7 of `scattering.md`
already declines the live-world shape updates; jigsaw pieces do not request
them.

**T5. Processors are a `type`-dispatched enum; their randomness is
positional.** Eleven types (`templatesystem/StructureProcessorTypes.java:10-20`);
the corpus uses `rule` (39 lists), `protected_blocks` (7), `block_rot` (6) and
`capped` (4), and code injects `block_ignore`, `jigsaw_replacement` and
`gravity`. `rule` draws from `LegacyRandom(hash(worldPos))` shared across a
block's rules, in rule order, `input → location → position` short-circuited
(`templatesystem/ProcessorRule.java:63-74`; `RuleProcessor.java:14-48`);
`block_rot` and `block_age` from the settings' positional random; `capped`
from a positional fork of the level seed at the piece position
(`CappedProcessor.java:61-62`). Ten rule tests and three position tests are
`predicate_type`-dispatched enums, of which `RuleTest` already exists as a
serde type (`crates/mcrs_minecraft_worldgen_feature/src/rule_test.rs:6-38`), as
does the processor list itself, reached today only through the fossil and
template features (`feature/proto.rs:664-725`). A `location_predicate`,
`protected_blocks` and `lava_submerged_block` read the *world* block at the
target position: under Wn2 that is the filled window plus the column's own
writes, and §7 says what that means for a piece placed from several columns.

**T6. Templates load eagerly, once.** 1511 files, 7.7 MB compressed, 43 MiB of
raw NBT. Fe1 says validate at load: every template a pool names must exist,
parse at the pinned version, and resolve its palette, or the freeze fails
naming the file. The reference substitutes an *empty* template for a missing
or unreadable one and carries on (`StructureTemplateManager.java:90-121`), so
that a pool with a typo places nothing silently; that is the failure mode Fe1
forbids. The packed size after freeze (a `u16` position and a `VoxelId` per
block, one list per template) is a measurement, not a guess; if it is too large
to hold, the fallback is a per-pool lazy load behind the same validated
manifest, never an unvalidated one. Whatever the outcome, the *manifest* —
each template's id, size, palette count and jigsaw blocks — is a separate,
small, frozen table: it is all the site step (L2) and the search (R3) ever
read, and a searcher links it without the block lists.

---

## 6. Terrain adaptation

`worldgen.md` §10 states B1–B4; this section supplies the data and settles the
node B4 owes.

**A1. The term.** For a column `U`, collect every start within reach whose
structure has `terrain_adaptation ≠ none`, and from each start every piece
whose box comes within 12 blocks of `U`'s footprint
(`levelgen/Beardifier.java:46-104`; `StructurePiece.java:130-134`). A jigsaw
piece contributes as a *rigid* only if its projection is `rigid`, with its
`ground_level_delta`; any other piece contributes as a rigid with delta 0; and
every junction of a jigsaw piece whose `(x, z)` lies strictly inside the
footprint inflated by 12 contributes as a point (`:62-94`). The union of the
collected boxes and points, inflated by 24, is the affected box; outside it the
term is exactly zero (`:100, 154-159`).

**A2. The formulas.** With `g = box.minY + delta`, `dx`, `dz` the horizontal
distances to the box (zero inside), `dyg = y − g`:

```
bury:         dy = dyg;                                 bury(dx, dy/2, dz)
beard_thin:   dy = dyg;                                 beard(dx, dy, dz, dyg) · 0.8
beard_box:    dy = max(0, g − y, y − box.maxY);         beard(dx, dy, dz, dyg) · 0.8
encapsulate:  dy = max(0, box.minY − y, y − box.maxY);  bury(dx/2, dy/2, dz/2) · 0.8
junction:     beard(x − jx, y − jy, z − jz, y − jy) · 0.4

bury(dx, dy, dz)  = d² ≥ 36 ? 0 : 1 − √d²/6
beard(dx, dy, dz, yg) = |dx|,|dy|,|dz| < 12 ? −(yg + 0.5) · invsqrt((dx² + (yg+0.5)² + dz²)/2) / 2 · K[dx][dy][dz] : 0
K[i][j][k] = exp(−(i² + (j + 0.5)² + k²) / 16),  i, j, k ∈ [−12, 12)
```

(`Beardifier.java:161-227`). `K` is 13 824 floats built once per process; the
`invsqrt` is the reference's fast inverse square root, part of the strict
profile's function. The corpus assigns `beard_thin` to villages, outposts,
camps and fossils, `beard_box` to the ancient city, `bury` to strongholds and
trail ruins, `encapsulate` to trial chambers; twelve structures adapt, the
rest do not.

**A3. The term is added per block at the root of `final_density`.** In the
shipped overworld graph the leaf sits as
`add(min(squeeze(interpolated(…)), noodle), beardifier)`
(`assets/minecraft/worldgen/density_function/overworld/final_density.json`;
`NoiseRouterData.java:683-715` for the other dimensions) — outside the
interpolation, which is B3. SteelMC learned it the hard way: adding at cell
corners puts the term inside the squeeze and trilerps it
(`steel-worldgen/src/noise/noise_chunk.rs:474-482`).

**A4. The term is added after the graph, not bound inside it.** The compiler
folds `minecraft:beardifier` to a declared constant zero with the infinite
interval (`crates/mcrs_minecraft_worldgen_density/src/compile.rs:316`), and the fold
stays. Because the beardifier is the outermost `add` of `final_density` in
every shipped router (A3), a fill evaluates the graph over its volume and then
adds A2 into the density of every block of the volume the affected box
reaches, before the substance loop reads it — the aquifer decides on bearded
density, as in `levelgen/NoiseBasedChunkGenerator.java:478-498`. That is the
graph's value bit for bit, up to the sign of a zero density, which no block
choice reads. A cell whose block box meets the affected box is filled block by
block without asking its interval bound; outside the box the term is exactly
zero and the bound holds unchanged. The term is a per-fill temporary written
into the fill's own density, not a stored projection; its owner is the fill
task. The shape is the precondition, so it is checked where the router is
built: a dimension with a live structure refuses a `final_density` that names
the beardifier anywhere but as an operand of its root `add`, and one with a
live adapted structure refuses a root that is not `add(_, beardifier)` in
either order. Beta names no beardifier and adapts nothing, and passes.

**A5. Layout heights ignore the term (L3), so starts stay independent.** The
reference passes no beardifier to the height query. Were the query bearded, a
village's site would depend on the outpost next to it, the index of §8 would
stop being per-cell, and P7 would fall. This is the invariant that makes the
whole of §2–§4 parallel.

---

## 7. Materialisation

Piece placement is not a stage. `scattering.md` §7 said it is part of `Run`
with `r_w = 0`; the radius is wrong and the rest stands.

**M1. The write footprint is one column, like every scattered object.** The
reference clips each piece to the chunk's writable box — its own 16×16, from
one above the floor to the ceiling (`ChunkGenerator.java:496-504`;
`StructureStart.java:85-108`) — and that clip holds for template pieces and
for the grid generators' `placeBlock`. Three hardcoded pieces widen it: the
ruined portal encapsulates its whole box and spreads netherrack within 14
blocks of its centre from the one column that holds the centre
(`structures/RuinedPortalPiece.java:180-198, 258-296`), the nether fossil
encapsulates its box from every column it touches
(`NetherFossilPieces.java:107-121`), and buried treasure and the igloo write a
neighbour or two unclipped (`BuriedTreasurePieces.java:44-77`;
`IglooPieces.java:161-178`). Every such write lands within one column of the
placing column, which is the radius the reference's region permits
(`server/level/WorldGenRegion.java:321-350`; `ChunkPyramid.java:30-36`), and
which Wn5 already routes into a delta. So `r_w = 1`, the ladder of §3.2 needs
no fourth stage, and a write beyond the ring is the data error S4 already
names.

**M2. The read rule is Wn2, and it changes what a piece may compute.** The
reference reads the *live* world at placement — heightmaps for the ground of a
pyramid, blocks for a mineshaft's supports — and freezes some of what it finds
into the piece for every later chunk (`ScatteredFeaturePiece.java:50-76` stores
the mean height of the columns *inside the first decorating chunk*;
`ShipwreckPieces.java:170-192` likewise; `BuriedTreasurePieces.java:79` rewrites
the box). That value depends on which chunk decorated first, which is the
player's route: it was never part of any reproducible world (S3, D2). Under
Wn2 a column sees the filled window and its own writes, so two columns placing
one piece can disagree on anything derived from *own* writes, and cannot see a
column two away at all. Hence:

**M3. A piece parameter is fixed at layout or derived from `Filled` alone.**
Anything the reference computes at placement and stores on the piece — the
scattered pieces' height, the shipwreck's adjusted height, the treasure's
resting block — is either (a) computed at layout by the density heights of
L3, in the form the reference itself uses when the piece is too large for its
region (`ShipwreckPieces.java:199-211`: the four-corner minimum or mean), or
(b) computed by every placing column from the `Filled` snapshots only, never
from own writes, and only when the reads lie within the 3×3 of *every* column
that can place the piece, which is a check on the piece's extent at freeze.
(a) is the default; (b) is allowed where the extent admits it (the treasure's
one column, the igloo's entrance column). Per-column decisions the reference
makes per chunk — the mineshaft's flooded-shell test on its own clipped box
(`MineshaftPieces.java:1196-1247`), a mansion's cobblestone footing under its
own columns (`WoodlandMansionStructure.java:62-92`) — stay per column and read
the window as any object does.

**M4. Placement flags are derivable and are dropped.** `hasPlacedChest`,
`placedTrap`, `Witch`, `Mob` and their kin exist so that a re-run of a chunk's
decoration does not place a chest twice. Every block belongs to one column,
and a column's run is bit-identical on re-run (St1), so the flag equals "the
column holding that block has run" and is not stored. The reference's
unseeded draws — a pyramid's cellar (`DesertPyramidPiece.java:741, 832`), a
mansion's allay count (`WoodlandMansionPieces.java:1509`) — draw from the
placement stream instead, the same substitution St1 makes for spawning.

**M5. The step slot, and the stream inside it.** In step `s` a column first
places the starts of every structure whose `step` is `s`, in the registry
order of structure ids (path, then namespace: `resources/Identifier.java:152-159`),
each structure reseeding the decoration source with
`decorationSeed + index + 10000·s` where `index` counts structures of that step
in that order whether or not they have a start here; then the step's features
run with their own indices (`ChunkGenerator.java:386-434`; S6). All starts of
one structure that reach the column share one stream in sequence, in the order
of their source chunks; the reference's order is a hash table's
(`world/level/chunk/ChunkAccess.java:75`; SteelMC reproduces it,
`steel-worldgen/src/structure/start.rs:88-98`). Decoration parity is
unattainable regardless (`worldgen.md` §15), so the order here is ascending
`(x, z)` of the start chunk, an input of the configuration hash. Every piece of
a start whose box meets the column is placed, in piece order, with the start's
reference position — the first piece's box centre at its floor — for the
position tests of T5 (`StructureStart.java:93-101`); then the structure's
after-place hook, which two structures define (`DesertPyramidStructure.java:32-66`,
`WoodlandMansionStructure.java:62-92`).

**M6. Rungs.** The current ladder cuts the steps into rungs at
`vegetal_decoration` and `top_layer_modification`
(`crates/mcrs_minecraft_worldgen_generator/src/feature_program.rs:275-296`).
Every structure step — `underground_structures` 3, `surface_structures` 4,
`strongholds` 5, `underground_decoration` 7 — lies in the first rung, so
materialisation never straddles a cut and a structure's blocks are merged
before any plant asks whether it can stand on them.

**M7. Block entities are data until delivery.** Chests, spawners, jigsaw
blocks left as `final_state`, brushable blocks: D6 of `scattering.md` gives the
column a typed list, and a piece appends to it. A loot table with its seed from
the placement stream (T4) is such an entry; an entity a template spawns
(172 templates carry some) is a second list of the same shape, delivered the
same way, and not a component until then.

---

## 8. The index

**I1. Classification.**

| Value | Category | Owner | Note |
| --- | --- | --- | --- |
| Structure sets, structures, pools, processor lists, templates, the frozen tables | truth | the freeze build | cloned into the sub-app once, like the routers (`generate/routers.rs:45-91`) |
| Ring positions per dimension | projection of the seed | the index, at construction | P4; a few hundred kilobytes of biome sampling, once |
| `site(C, S)` for a chunk `C` and structure `S` | projection of the seed | the index, memoised per chunk | L2; cheap; what search asks (R6) |
| `starts_at(C)` for a chunk `C` | projection of the seed | the index, memoised per chunk | P2–P6 then §3–§4; expensive (a village layout), reused by fill, run and gameplay |
| The beard term of a fill | derived, temporary | the fill task | A4; added into the fill's density, never stored |
| `starts.<id>` and `References` in the chunk NBT | projection, written for the format | the save writer | I5 |
| A start's `references` counter | **truth** | gameplay, the map that claims it | L6 |
| A structure's blocks in a column | projection of the seed, until edited | the column's run | §7; `worldgen.md` §16 |

**I2. Shape and owner.** One index per dimension: the frozen tables, the ring
positions, and a concurrent memo from chunk position to an owning slice of
starts (usually empty), each slot filled once by whichever task asks first.
Generation lives outside the ECS (X1), and so does the index: it sits beside
the staging store and the scheduler (`generate/staging.rs:178`;
`world/chunk.rs:208`) and is handed to fill and run tasks as an owning handle
(X7). The sub-app receives the same handle as a read-only resource for the
consumers of I4. Two tasks racing on one cell compute the same bits (St1); a
once-cell per slot removes the duplicate work, and nothing else needs a lock.
A slot has two levels, filled independently: the *site* level, which records
per structure whether the site step passed and where (L2), and the *layout*
level, which holds the pieces. Search fills the first and never the second
(R6); fill and run fill the second, computing the first on the way if it is
empty.

**I3. The query.** *Starts reaching column `U`*: for each live set, for each
chunk `C` within eight chunks of `U` (L5), the starts of `C` whose box
intersects `U`'s footprint, the box already inflated by 12 where the
structure adapts (L1). For a `random_spread` set the chunks to
visit are the cells whose potential chunk lies in the square, which for
`spacing ≥ 20` is a handful; for the three spacing-1 or spacing-2 sets it is
the square itself. The reference visits the 289 chunks of radius 8 for every
one of its four dependent statuses; the index visits the same square once per
query, and memo hits are the norm because neighbouring columns share it.

**I4. Consumers.** Fill (A1); the run of each step slot (M5); the spawner's
overrides, which ask for every start whose box or whose *pieces* contain a
position (`ChunkGenerator.java:518-546`; `spawn_overrides.bounding_box` is
`piece` or `full`, serialised as `"full"` for the enum's `STRUCTURE`,
`world/level/StructureSpawnOverride.java:24-41`; the first structure in
iteration order with a matching override wins, and the nether fortress is
special-cased before the generic path, `world/level/NaturalSpawner.java:359-369`);
the piece test of the cat spawner and the black-cat variant
(`world/entity/npc/CatSpawner.java:40-42`; `world/entity/animal/feline/CatVariants.java:57`);
every search of §9; and the save writer (I5). Each reads the same memo through
the same query; none writes it, except the map's claim (R5).

**I5. The save.** Vanilla tools and vanilla servers reading our world expect
`structures.starts` keyed by structure id in the start chunk and
`structures.References` keyed by structure id in every chunk a start reaches
(`world/level/chunk/storage/SerializableChunkData.java:555-582`), both as
`mcrs_nbt` writes the types of L6. `starts` is written for the chunks the memo
holds; `References` is recomputed at write time by I3, because it is a
projection and the memo answers it. On load, a stored start *primes* the memo
for its chunk and the seed is not consulted for it; absent, the seed derives
it. The two agree whenever the configuration hash matches (St3) — its inputs
now include the structure registries' content hash, the template version pin
(T1) and the ordering rules of M5 — and the hash is checked at load, so a
world whose datapack changed is refused rather than seamed. A stored
`references` counter is truth (L6) and never regenerated.

**I6. Eviction.** The memo is bounded the way the staging store is (Q7): a
chunk's slot is kept while any column within its reach is wanted, and a slot
dropped and recomputed is bit-identical. The reference keeps starts for the
life of the chunk and re-reads region files to answer `/locate` outside the
loaded area (`structure/StructureCheck.java:88-142`); with I5's priming that
path becomes "load the chunk's `starts` tag if the chunk is on disk, else
derive", and the negative cache the reference keeps for chunks without starts
on disk (`:59-61, 105-107`, capped at 131 072 entries and cleared wholesale) is
a detail of that path.

---

## 9. Search: `/locate`, maps, eyes and seeds

Everything in this section is a consumer of the first four layers of §1.1 and
adds no state. It has two readers with opposite shapes: `/locate` and its kin
hold the seed fixed and walk cells; a seed searcher holds cells fixed and
walks seeds. The invariants below are what make both cheap, and R2 is the one
the reference never states because it never needed it.

**R1. Two seeds.** The world seed is 64 bits. Every draw of §2 and §3 goes
through `LegacyRandomSource.setSeed`, which is `(s ^ 0x5DEECE66D) & (2⁴⁸ − 1)`
(`levelgen/LegacyRandomSource.java:32-38`); `setLargeFeatureWithSalt` adds the
seed to the cell hash before it (`WorldgenRandom.java:66-69`), and
`setLargeFeatureSeed` reseeds with the seed, draws two longs and XORs, then
reseeds again (`:58-64`) — in both the top 16 bits of the seed are discarded
before any draw. So the cell function (P2), all four frequency gates (P3), the
selection stream (P6) and the layout random (L2, `Structure.java:283-287`) are
functions of the low 48 bits, the *structure seed*. The climate field and the
density column are not: `RandomState` seeds a Xoroshiro source through
`upgradeSeedTo128bit`, a full 64-bit mix (`levelgen/RandomState.java:66-118`;
`levelgen/RandomSupport.java:17-31`; `levelgen/XoroshiroRandomSource.java:16-18,
44-48`), so the biome test of L2, every height of L3 and the ring snap of P4
depend on the whole seed. Consequences: (a) a positional constraint on
structures is decided at the 48-bit tier and the 2¹⁶ seeds above it are
either all lifted or all not; (b) constraints on different sets intersect at
that tier, because each set is a function of the same 48 bits; (c) nothing at
the 64-bit tier is shared between two seeds — the mix has no structure to
exploit — so the lift is 2¹⁶ climate samples per surviving structure seed and
must be ordered by cost.

**R2. The congruential generator is bit-monotone, and a bounded draw with a
non-power-of-two limit leaks its low bits.** The state after `setSeed(s)` is
`(s ^ M) mod 2⁴⁸` and a step is `state·M + 11 mod 2⁴⁸`
(`LegacyRandomSource.java:41-49`). XOR with a constant, addition and
multiplication modulo a power of two carry only upward, so **bit `j` of the
state after any number of steps is a function of bits `0..j` of `s`**, and
since `s = gx·A + gz·B + seed + salt` is a sum, of bits `0..j` of the seed.
A bounded draw is `nextInt(limit)` (`levelgen/BitRandomSource.java:17-32`):

```
power of two:  (limit · next(31)) >> 31              — the top bits of the state
otherwise:     sample = next(31) = state >> 17
               offset = sample mod limit,
               redrawn while sample − offset + (limit − 1) overflows
```

If `2ᵏ` divides `limit`, then `offset mod 2ᵏ = sample mod 2ᵏ` = bits `17..17+k−1`
of the state, a function of the low `17 + k` bits of the seed; the second
draw (`oz`) is one step later and, by monotonicity, a function of the same
bits. The redraw branch is taken on the high bits, which the partial key does
not know — but the residue *under each branch count* is still a function of
the low `17 + k` bits, so a sieve computes the residues for zero and for one
redraw and accepts a partial key that matches either. The sieve is then
exact: every seed that passes the full test passes the sieve, and the false
positives it admits cost one full 48-bit test each. For `triangular` the mean
`(a + b)/2 mod 2ᵏ⁻¹` is a function of `a mod 2ᵏ` and `b mod 2ᵏ`. For a
power-of-two limit there is no reduction, and for the `nextFloat`/`nextDouble`
gates neither, because both are top bits. What the corpus admits:

| Set | `spacing − separation` | `2ᵏ` | Partial key | Notes |
| --- | --- | --- | --- | --- |
| desert_pyramids, igloos, jungle_temples, swamp_huts, pillager_outposts | 24 | 8 | 20 bits | the outposts' frequency gate is `nextInt(5)`, odd, and adds nothing |
| ocean_ruins, shipwrecks | 12, 20 | 4 | 19 bits | |
| villages, trail_ruins, trial_chambers | 26, 26, 22 | 2 | 18 bits | |
| woodland_mansions | 60, triangular | 4 per draw, 2 after the mean | 19 bits | |
| ancient_cities | 16 | power of two | none | top-bits path |
| abandoned_camp, end_cities, nether_complexes, ocean_monuments, ruined_portals | 29, 9, 23, 27, 25 | 1 | none | odd limits |
| buried_treasures, mineshafts, nether_fossils | 1, 1, 1 | — | none | every chunk is a cell; the gates are `nextFloat`/`nextDouble` |
| strongholds | rings | — | none | P4 |

The sieve is what makes a "four huts around one point" search feasible: four
20-bit constraints intersect before a single 48-bit test runs. It is a
property of P2 and R1 together, and a change to either — a different
generator, a salt that enters after the mask — voids it, which is why the
profile (E1) owns the placement function and the sieve is derived from the
profile's limits at freeze rather than written per set.

**R3. The search ladder.** A seed search is the same ladder as generation,
entered at the cheap end and left at the first failing tier:

1. the partial-key sieve of R2, over `17 + k` bits, for every positional
   constraint that admits one;
2. the cell function and the frequency gate, over 48 bits (P2, P3) — three
   draws per set per constraint;
3. the exclusion gate (P3) and, where the constraint names them, the 48-bit
   parts of selection and layout: which entry of a set, the centre rotation
   and element of a jigsaw, a mineshaft's type, a portal's setup (P6, J2, L4);
4. the lift: for each of the 2¹⁶ seeds above a surviving structure seed, the
   biome test at the site (L2), which is one climate sample at quart
   resolution — the first 64-bit read;
5. what depends on heights: the projected `y` of the site, the ring snap of
   strongholds, any layout that queries a column (L3, P4).

Every tier is a pure function of the seed and the cell; no tier shares state
across seeds; the batch axis is therefore *seeds*, not cells: lanes of 48-bit
states advanced together through the non-redraw path, with the rare redraw
lane resolved scalar. Tiers 1–3 never touch a template, a biome or a density
node; tier 4 needs the template manifest of T6 for the centre's box and the
climate sampler; only tier 5 needs a density tape. A searcher therefore links
the frozen registries and the manifest, not the block lists (T6), and not the
materialiser.

**R4. `/locate` is a walk over cells in the reference's answer order.** The
command asks the generator for the nearest start of a structure set within
100 lattice cells, never creating references
(`server/commands/LocateCommand.java:50, 126-128`;
`ChunkGenerator.java:177-266`). Structures are grouped by placement; every
`concentric_rings` placement is answered from its whole ring list by true
minimum distance (`:268-305`, at the chunk centre and `y = 32`); then, for
`radius = 0..100`, every `random_spread` placement scans the *perimeter* of
the `(2r+1)²` cell square, `x` ascending then `z` ascending, computes the
potential chunk of each cell and returns the **first** cell whose start is
present (`:307-340`); the hits of all placements at that radius are compared
by squared 3-D distance from the player to the locate position (the chunk's
minimum corner plus `locate_offset`, `y = 0`;
`placement/StructurePlacement.java:22-28`), and the walk stops at the first
radius with any hit (`:227-261`). The answer is thus the nearest *among the
first hit per placement on the first non-empty ring*, not the nearest start:
observable to a player comparing with a vanilla server, and reproduced as the
default. A `nearest` mode continues the walk while a ring can still hold a
closer chunk, i.e. while `(r − 1) · spacing · 16 < best`, and finishes the ring
it is on.

The presence test behind each cell is where the two designs part. The
reference's `StructureCheck` answers from its memo, else from the region
file's `structures.starts` tag by a field scan, else by the frequency gate and
a fresh site step with the pieces discarded — and if that passes it *loads the
chunk* to `structure_starts` to learn whether the start is valid
(`StructureCheck.java:88-141`; `ChunkGenerator.java:342-368`). Here the index
answers `site(C, S)` from its site level (I2) — the three gates and the site
step, no chunk — and needs the layout only when a site can pass with no
piece, which is a per-type fact fixed at freeze: a jigsaw start always has
its centre piece, and the table of hardcoded types says which of them can
return empty. With that table a locate never runs a layout.

**R5. The other readers of the walk.** An explorer map calls the same walk
with `search_radius` (default 50), `skip_existing_chunks` (default on) and
*creates a reference*: with the flag the walk demands an unreferenced start
and claims the one it returns, having first claimed the start the player is
standing in so the map does not point home
(`world/level/storage/loot/functions/ExplorationMapFunction.java:37-44, 81-118`;
`StructureCheck.java:222-229`; `StructureManager.java:211-214`). That claim is
the one write any consumer makes and lands on the `references` counter L6
calls truth. The eye of ender walks `#eye_of_ender_located` at radius 100,
dolphins `#dolphin_located` at 50 (`world/item/EnderEyeItem.java:85-94`;
`world/entity/animal/dolphin/Dolphin.java:437-439`). The tags are data
(`tags/StructureTags.java:8-40`, contents in
`data/tags/StructureTagsProvider.java:18-86`) and resolve at freeze the way
biome tags do; two ids are traps for a hand-written table — `cats_spawn_in`
is plural, and the eight abandoned-camp map tags end in the biome, not in
`_maps` (`StructureTags.java:33-40`). Every reader receives the `locate`
position of P1, never a piece position, and a map is drawn from it.

**R6. Cost, and what is memoised.** Per cell: one 48-bit seed, two or three
steps of the generator, one memo probe. Per site: one climate sample and, for
a projected structure, one density strip (L3). A `/locate` at radius 100 is at
most `201²` cells per placement, of which the memo answers every cell already
asked by fill; a stronghold locate is 128 entries. The reference pays a chunk
allocation and a `createStructures` per candidate past its memo. The site
level of the memo is what search writes (I2); it is bounded with the memo
(I6) and a dropped site is recomputed bit-identical. Search never allocates a
column, a window or a template block list, and never blocks a fill: the index
is shared by handle and its slots are once-cells.

---

## 10. Editions

### 10.1 Beta

**B1. Beta has no structures; it has two ancestors.** Neither reimplementation
contains a structure start, a piece, a template or anything named stronghold,
village, mineshaft or fortress (a search over both trees finds nothing).
What Beta 1.7.3 has is `WorldGenDungeons`, a feature of the populate step, and
`MapGenCaves`, a carver with the placement pattern that structures inherited
(`ChunkProviderGenerate.java:21, 207, 358-363`). The Nether has neither
(`ChunkProviderHell.java:315-372`). So the Beta profile (E1) has **zero
structure sets**, an index that answers nothing, and no template store;
nothing of §2–§8 is instantiated for it and nothing is stubbed. `/locate`
under Beta finds nothing, as it should.

**B2. The dungeon is a scattered object, and its definition is its draws.**
Populate seeds a `java.util.Random` with the world seed, draws two longs
forced odd, and reseeds with `x·a + z·b ^ seed` for the chunk
(`ChunkProviderGenerate.java:329-334`; mcrs already has it,
`crates/mcrs_minecraft_worldgen_generator/src/lib.rs:760-768`). Dungeons
are the third decorator, after a water lake gated by `nextInt(4) == 0` and a
lava lake gated by `nextInt(8) == 0` (`:340-353`), so the draws before the
first attempt are the two gates, the lakes' positions and, when a lake
places, its `nextInt(4) + 4` blobs of six doubles each
(`WorldGenLakes.java:13-31`). Then eight attempts (`:358-363`), each at
`x = 16·cx + 8 + nextInt(16)`, `y = nextInt(128)`, `z = 16·cz + 8 + nextInt(16)`,
and each spending its two size draws `nextInt(2) + 2` per axis *before* any
test (`WorldGenDungeons.java:10-13`), so a rejected attempt still shifts the
stream. Appendix E gives the rest: the validity scan, the open-side count
`1..5` (`:39`), the floor draw `nextInt(4)` taken only on the floor layer and
only for buildable shell blocks in `x` ascending, `y` *descending*, `z`
ascending order (`:40-56`), two chests of three tries each against exactly
one solid cardinal neighbour with eight loot rolls whose slot draw is spent
only for a non-null item (`:58-111`), the eleven-way loot table (`:123-127`),
and the spawner's `nextInt(4)` (`:129-133`). BetrockPlusPlus reproduces every
draw (`world/generator/shared/feature_gen.cpp:101-241`). Under the ladder the
dungeon is an object of `mcrs:beta_populate` (`scattering.md` D11): its
origin is shifted by 8 from the column and its shell reaches 4 further, so
its writes lie within `[x + 4, x + 27]` and `r_w = 1` holds; it reads air and
buildability, which under Wn2 is the filled window plus the two lakes that
precede it in the same program. Bit-exact parity is attainable and §13 asks
for it.

**B3. The cave map-gen is the placement pattern, fifteen years early.**
`MapGenBase` visits the `17 × 17` chunks around the target, reseeds per
*source* chunk with the same odd-forced hash as populate, and lets a source
chunk's tunnels write into the target array (`MapGenBase.java:7-25`;
`MapGenCaves.java:173-198` for the per-source draws, `:13-171` for a tunnel
with its own forked random, `:18`). That is "a source cell decides, a target
receives, within a reach" — the shape of I3 with a reach of 8 and a lattice of
spacing 1 — and it is what `MapGenStructure` later put a start map on. It is
also already the carver stage here (`generate/beta_caves.rs`;
`mcrs_minecraft_worldgen_carver/src/beta.rs`) with its parity test, so it
is not a structure and is not made one; it is cited because it explains why
L5's reach is 8 and why the index needs no new concept for Beta. Two facts
Betrock records and mcrs must not inherit: its Nether tunnels omit the `0.5`
vertical scale (`MapGenCavesHell.java:182` against `cave_gen.cpp:60-61`), and
it seeds the Nether's populate, which vanilla never does
(`ChunkProviderHell.java:315`; `nether/chunk_gen.cpp:355-368`) — a Beta
Nether, if ever built, is order-dependent in the reference and reproducible
only under our own rule.

**B4. A Beta seed is 48 bits wide.** Terrain, biomes, populate and caves all
draw from `java.util.Random` (E3), so the low-bit sieve of R2 applies to the
whole Beta world, and a Beta seed searcher has no 64-bit tier at all: the
ladder of R3 ends at tier 3.

### 10.2 Bedrock

What follows is read from the wiki and Microsoft's behaviour-pack reference,
which are the only sources; the wiki states outright that structure placement
"differs between these editions" for equal seeds and that a seed generates
"the same terrain and biomes" in both, and it documents no random generator.
Claims of a Mersenne-Twister generator circulate on third-party sites and are
not adopted. Bedrock has accepted 64-bit seeds since 1.18.30.

**K1. The formats are a dialect of Java's.** A behaviour pack ships
`worldgen/{structures,structure_sets,template_pools,processors}/*.json` and
`structures/<namespace>/*.mcstructure`. A `minecraft:jigsaw` structure has
`start_pool`, `start_jigsaw_name`, `max_depth` 0..20, `start_height` as a
constant or uniform over `absolute`, `above_bottom`, `below_top` or `from_sea`
anchors, `heightmap_projection` ∈ `world_surface`, `ocean_floor`, `none`,
`dimension_padding`, `max_distance_from_center` (default horizontal 80),
`pool_aliases` (`direct`, `random`, `random_group`), `liquid_settings`,
`terrain_adaptation` with Java's four names, the same eleven `step`s, and
`biome_filters` in place of a biome list. A `structure_set` has `placement.type`
`random_spread` or `concentric_rings`, `salt`, `spacing` (default 34),
`separation` (default 8, "must be less than spacing"), `spread_type` and a
weighted `structures` list; it has no `frequency`, no
`frequency_reduction_method`, no `exclusion_zone`, no `locate_offset`, and no
documented ring fields. A `template_pool` has `elements` of weight 1..200 and
a `fallback`; the five element types and the two projections are Java's. A
`processor_list` has four types: `block_ignore`, `protected_blocks`, `capped`
(with `limit` where Java writes `value`) and `rule`, six block predicates and
two position predicates (`always_true`, `axis_aligned_linear_pos`), and block
entity modifiers `passthrough`, `append_loot`, `clear`. Only trail ruins are
data-driven in vanilla Bedrock; villages, bastions and the rest use a closed
legacy path. Appendix D is the field-by-field table.

**K2. What the wiki says a Bedrock world does.** Recorded as data for a
profile that may never be built, not as commitments: strongholds are
unlimited, at least 160 blocks from the origin, with three more at least 453
blocks out placed under village meeting points; the Nether region is 480 × 480
blocks with a fortress at 1/3 and a bastion at 2/3 and a fortress forced in
basalt deltas; the ocean monument's roof is always at `y = 56`; buried
treasure sits at chunk-relative `(8, 8)` where Java uses `(9, 9)`; trial
chambers use the same 34-chunk grid as Java; outposts may generate inside
villages; the only vanilla salt published is trail ruins' `83469867` with
`34 / 8`, equal to Java's. `/locate structure` takes `useNewChunksOnly` and
prints `x ~ z`. Nether fossils are a feature, not a structure.

**K3. Decisions for the shared architecture.** Bedrock enters at the two
seams E1 names and nowhere else. (a) *Decoders*: the four registries gain a
Bedrock `Deserialize` for the same Rust types — a rename map (`max_depth` for
`size`, `heightmap_projection` for `project_start_to_heightmap`,
`biome_filters` for `biomes`, `limit` for `value`, `target_pool` for `pool`)
plus the wider weight range and the `from_sea` anchor as a variant; a
`.mcstructure` decoder yields the template type of T1 from `size`,
`structure.block_indices` in `z, y, x` order with the second layer as the
waterlogging layer and `−1` as structure void, `palette.default.block_palette`
through the block schema, and `block_position_data` as block entities. Every
field either maps or is a load error naming the file; a Bedrock pack with a
field this document does not list is refused, not guessed. (b) *Placement*:
until Bedrock's generator is known, a Bedrock profile places its sets with the
Java lattice function of P2 and Bedrock's `spacing`, `separation`, `salt` and
`spread_type` — the same lattice model both editions document — and states
plainly that its positions match no Bedrock server. Layout, adaptation,
materialisation and search are unchanged. Whether Bedrock's assembly is J1–J7
draw for draw is unknown (§15) and is not claimed.

**K4. What cross-platform means here, exactly.** The same registries, pools,
processors and templates load from either edition's files into one set of
types; one index, one materialiser and one search serve every profile; the
Java profile reproduces Java's positions and layouts, the Beta profile
reproduces Beta's dungeons and caves, and the Bedrock profile reproduces
Bedrock's *content* with positions of our own. Nothing more is promised
because nothing more is knowable from the sources.

---

## 11. Assets and freeze

Fe1 and Fe2 of `scattering.md` apply unchanged. Four registries and one
binary asset join the worldgen macro (`crates/mcrs_minecraft_worldgen/src/bevy.rs:110-256`):
`structure_set` (nested: names structures and, through an exclusion zone,
other sets), `structure` (nested: names pools, a `placed_feature` through a
feature element is reachable only through a pool), `template_pool` (nested:
elements name templates, processor lists, placed features, and pools name
their fallback and each other), `processor_list`, and `structure/<path>.nbt`
through a loader that gunzips and checks the pin of T1. The stub at
`crates/mcrs_minecraft_world/src/worldgen/structure_set.rs:13` (a unit struct
with no loader) is replaced, not extended. The profile (E1) is a field of the
world preset naming the placement function, the decoders and the generator
census; the Beta preset names the empty profile.

The freeze resolves and checks, naming the asset on failure:

- every id every set, structure, pool and element names;
- every codec range of P1, J1, T5; `spacing > separation`; the jigsaw
  distance-plus-margin bound; a non-empty pool that names no fallback that
  exists;
- exclusion-zone cycles (P3);
- duplicate aliases (J1);
- the per-structure `(step, index)` of M5, from the sorted structure ids;
- for each hardcoded type, that a generator exists in this build, as a census
  pinned by name, the way feature types with no generator are
  (`generate/tests/corpus_generators.rs`); a type without one is logged and
  places nothing, never a silent empty template;
- the "site implies a piece" table of R4 for the hardcoded types;
- the sieve widths of R2, derived per set from `spacing − separation` and
  `spread_type`, so a datapack set gets its sieve without a code change;
- every structure tag of R5 resolves to structures that exist.

The pools' expanded lists (J1), the per-pool maximum height the expansion
hack reads (`StructureTemplatePool.java:87-98`), and the template manifest of
T6 are frozen tables; the biome holder sets of P5 and the `has_structure/*`
tags resolve to biome masks the way `BiomeMask` already does. A Bedrock pack
loads through the dialect of K3 into the same tables and passes the same
checks.

---

## 12. Cost

Counts, to be replaced by `PERF.md` numbers.

- **Placement** (P2, P3): three draws and a hash per cell; an exclusion zone
  multiplies by `(2n+1)²` (pillager outposts: 441 village-cell tests, each a
  memo hit after the first).
- **Rings**: 128 biome searches over a 57×57 quart square each, once per
  dimension, on a pool if it shows in startup.
- **A site** (L2): one climate sample; for a projected structure one density
  strip; for a jigsaw, three to five draws and a manifest lookup.
- **A village layout**: `size 6` with the expansion hack; per source jigsaw two
  shuffles of expanded lists (tens of entries) and per candidate four rotation
  draws and a jigsaw shuffle; a few hundred candidates, a few hundred box
  tests per candidate in the shared context, on the order of 10⁵ box
  comparisons; plus one density-strip evaluation per terrain-matching
  attachment, which is the unknown that decides whether the strip evaluator
  needs a path of its own (a tape built for tiles, asked for a 1×H×1 volume).
- **The beard**: per column of a dimension with a live jigsaw structure, one
  walk of the starts reaching it besides the one the run makes — the 289
  chunks within eight, gated, until the memo of I2 answers them — then A2 over
  the affected box clipped to each block-filled cell, a handful of sections
  for a village. Every cell meeting the box gives up the interval verdict.
- **Materialisation**: the largest and least predictable stage
  (`worldgen.md` §11), which is why `worldgen.md` §17 asks for it measured
  apart from objects; a village is thousands of block writes and a processor
  chain per block.
- **`/locate`**: at most `201²` sites per placement at radius 100, the memo
  answering every cell fill has already asked; a stronghold locate is 128
  distances (R6).
- **A seed search**: per constraint and seed, tier 1 is a handful of
  integer operations on a `17 + k`-bit key; tier 2 is three generator steps;
  tier 4 is one climate sample per lifted seed, the first cost that is not a
  few nanoseconds; the ratio of seeds reaching tier 4 to seeds tested is the
  number that decides everything and is measured, not estimated (R3).
- **Memory**: the memo holds a village's pieces (a few hundred records) per
  cell within reach of a wanted column and a site record per probed cell; the
  frozen templates are the one number to measure before deciding T6.

---

## 13. Verification

Bit for bit against the reference, where the reference computes without a
world — each fixture a dump from `tools/vanilla-oracle` carrying
`WORLD_VERSION` 5015, and a template-version pin of 5011 that fails on
mismatch (R5 of `worldgen.md`):

- the cell function and the three gates, per shipped set, over a grid of
  cells around the origin and far from it, negative coordinates included;
- the 128 stronghold positions of a seed set;
- the layout of every shipped structure type: piece list with boxes,
  rotations, element ids, ground deltas and junctions, for seeds where the
  reference places one — attainable because J8 and L3 hold;
- the beard term: A2 against the reference's sampler over a piece list, per
  block of a chunk volume;
- template geometry: one piece placed into an all-air region through the
  oracle's stub level, the analogue of tree geometry parity, with the
  processor chains of the corpus;
- `/locate`: the reference's answer for a seed set and a grid of player
  positions, including the cases where the first-ring answer is not the
  nearest start (R4);
- the 48-bit invariant: for every fixture above that R1 places at the 48-bit
  tier, flipping any of the top 16 seed bits changes nothing, and for a site
  fixture it changes the biome verdict and nothing before it;
- the sieve: a property test that every seed passing the full cell test
  passes the sieve of R2 for its cell, for every shipped set with a sieve,
  including the redraw branch, found by searching for seeds whose first
  sample lands in the redraw interval.

Beta, against Project-Poseidon through the existing parity harness
(`generate/tests/beta_cave_parity.rs` is the model): the populate stream up to
and including the eight dungeon attempts (B2, Appendix E) for a seed set,
with room boxes, floor blocks, chest positions, loot and spawner ids; and,
once dungeons exist in `mcrs:beta_populate`, the ore positions that D11 of
`scattering.md` says currently diverge.

Bedrock: Microsoft's published trail-ruins pack round-trips through the
dialect of K3 into the same registries and passes the freeze; a
`.mcstructure` built from a Java template by the reverse encoder decodes to an
equal template. No positional parity is tested because none is claimed.

Structurally: every shipped `structure_set`, `structure`, `template_pool` and
`processor_list` round-trips to an equal JSON value; every one of the 1511
templates round-trips to equal NBT; every id resolves at freeze; the census of
§11.

The ladder: `generate/tests/ladder.rs` gains a region with a village, a
mansion and a mineshaft, and the oracle of `scattering.md` §3.6 against the
parallel scheduler through every unobservable order (S5).

Decorated parity with a reference world is not attainable (`worldgen.md`
§15), and this document adds nothing that makes it so; what it adds is that
the *starts* of a reference world are, which is the half worth having.

---

## 14. What not to do

- **Make placement or references a stage.** `worldgen.md` §18 already says it;
  §2 and §8 are the alternative in full.
- **Give a start a mutable life.** SteelMC decorates under the *source*
  chunk's write lock and mutates pieces as it places them
  (`steel-core/src/worldgen/feature/runner.rs:308-338`), then pins its parity test
  to one chunk order. M3 and M4 are the alternative: a start is written once by
  the index and read by everyone.
- **Transliterate the generator's object graph.** A `Structure` trait object
  per type keyed by a string, a thirteen-field mutable generation context, a
  piece with four "family override" enums standing in for Java subclass
  overrides — SteelMC's `Box<dyn Structure>`, `GenerationContext` and
  `TemplatePieceData` (`steel-worldgen/src/structure/generator.rs:1012-1036`;
  `generation.rs:33-80`; `piece.rs:84-111`). The shape here is one enum over
  structure configs with a `match`, one enum over piece kinds (L6), and
  functions of plain inputs: the seed, the chunk, the frozen tables, the
  height and biome queries.
- **Substitute an empty template, a warning, or a hardcoded size for a
  missing asset.** T6 and §11. SteelMC hardcodes mansion template sizes by name
  with a fallback (`steel-worldgen/src/structure/mansion/template.rs:8-46`)
  and panics on a missing template at placement; both are load-time errors
  here.
- **Optimise the occupancy test before measuring it.** J6.
- **Read the live world for a piece parameter.** M2, M3.
- **Add the beard inside the graph, or accept a router whose root is not
  `add(_, beardifier)`.** A3, A4: at cell corners it is trilerped, and under
  any other root the add after the fill is no longer the graph's value.
- **Test the biome before the cell.** R3: the climate sample is the first
  64-bit cost and the cell test rejects almost every candidate for free; a
  locate or a search that samples biomes first pays for every cell.
- **Link the block lists into a searcher, or load a chunk to answer a
  locate.** T6, R4, R6: the manifest and the index answer both.
- **Branch on the edition inside a shared layer.** E1: the index, the
  materialiser and the search take functions and tables; an `if bedrock` in
  any of them is the Java object graph's cousin.
- **Claim Bedrock positional parity, or derive a generator from a forum.**
  K3, §15.
- **Write the structure tags by hand.** R5: they are data with two ids that
  a hand copy gets wrong.

---

## 15. Uncertainties and open questions

Facts this document could not establish from its sources, and the decision
each is waiting on. None blocks the order of work in §17; each names the step
that would be affected.

- **Bedrock's placement generator and seeding.** Unpublished; the wiki says
  only that positions differ from Java. Affects K3(b) alone; the profile
  ships with the Java lattice function until a primary source exists.
- **Whether Bedrock's jigsaw assembly is J1–J7 draw for draw**, and which
  heightmap generation (pre- or post-carve) `heightmap_projection` names.
  Affects nothing until a Bedrock parity claim is wanted, which is never in
  this document.
- **Bedrock's stronghold selection** ("under village meeting points"): whether
  it is a function of the seed or of generation order. Same scope as above.
- **`dimension_origin` with a `spawn_target`.** The origin is the spawn-target
  search over the climate field (P1); mcrs has no spawn finder yet, and no
  shipped set uses the placement, so the profile treats a non-empty
  `spawn_target` with a `dimension_origin` set as a freeze error until the
  finder exists. Affects §11.
- **The reference's `/locate` skips the exclusion gate in its cheap test and
  re-applies it on chunk load** (P3). The observable answer is the same as
  running all three gates, so no divergence is recorded; if a measurement
  ever shows a vanilla server answering an excluded outpost, this is where to
  look.
- **The 3-D distance in the locate comparison** uses the player's `y` against
  `y = 0` or `y = 32` locate positions (R4). Reproduced as is; the bias is the
  reference's and is not corrected.
- **The packed size of the templates** (T6) and **the cost of a 1×H×1 strip
  through the tile tape** (L3): measurements that decide a lazy load and a
  strip path respectively.
- **The "site implies a piece" table** for the fifteen hardcoded types (R4):
  written from the generators as they are ported, one row per type, and a
  type without a row runs its layout on locate.
- **Whether the low-bit sieve pays for constraints with `k ≤ 1`** (villages,
  trail ruins, trial chambers): an 18-bit key halves the work of tier 2 at
  best; the sieve is derived regardless and the searcher measures whether to
  apply it.
- **The Beta Nether's unseeded populate** (B3): the reference is
  order-dependent; a Beta Nether profile, if wanted, defines its own seeding
  and says so.

---

## 16. Decisions

**SD1. Placement and layout live in an index, not a stage.** §2, §8;
`worldgen.md` §10 and its divergence table already say so, and this document
is its specification. The `structure_references` scan is a query (I3).

**SD2. Materialisation is part of `Run` with `r_w = 1`.** M1 corrects
`scattering.md` §7, whose `r_w = 0` held for template pieces only; §7 of that
document is amended to cite this section.

**SD3. Piece parameters are fixed at layout from the density heights; Wn2 is
the read rule of everything else.** M3, with the reference's own large-piece
form as the canonical one. The divergence is listed in `worldgen.md`'s table
with the reason: the reference's value was route-dependent.

**SD4. The order of starts of one structure within a column is ascending
`(x, z)` of the start chunk.** M5. An input of the configuration hash. Not the
reference's hash order, which SteelMC reproduces; if a measurement of §6.4 of
`scattering.md` ever shows decoration parity within reach, this is the one
line to revisit.

**SD5. Placement flags are dropped; unseeded draws come from the placement
stream.** M4.

**SD6. Templates are pinned at data version 5011 and loaded eagerly at
freeze; the manifest is a table of its own.** T1, T6; the size after packing
is measured and a per-pool lazy load behind the same manifest is the fallback.

**SD7. The beard is added after the fill of `final_density`, behind a root
check, not bound as a node kind.** A4; the fold at `compile.rs:316` stays, and
a router whose root is not `add(_, beardifier)` is refused where a live
structure adapts.

**SD8. The save carries `starts` and `References` for the format; on load a
stored start primes the memo; the configuration hash gains the structure
inputs.** I5.

**SD9. The hardcoded generators are ported verbatim in draw order, as data,
after the jigsaw path.** L4, §17.

**SD10. One pipeline; an edition is a profile of data plus a placement
function and decoders, named by the world preset.** E1, K3. No shared layer
branches on it.

**SD11. The index memoises sites and layouts as two levels; search fills
sites only and never runs a layout that a freeze table says cannot be empty.**
I2, R4, R6.

**SD12. `/locate` reproduces the reference's answer order by default; a
`nearest` mode is a flag.** R4.

**SD13. The seed searcher is the ladder of R3 over the 48/64 split of R1,
with the exact sieve of R2 derived per set at freeze, batched over seeds, and
linked against the manifest and the frozen registries only.** §9.

**SD14. Beta is the empty profile: zero sets, dungeons as an object of
`mcrs:beta_populate`, caves as the carver they already are.** §10.1. The Beta
seed is 48 bits wide and the searcher reads that from the profile.

**SD15. Bedrock is supported as formats, not as positions: a serde dialect
and an `.mcstructure` decoder into the shared types, placement by the Java
lattice with Bedrock's parameters, and no parity claim until a primary
source for its generator exists.** §10.2.

---

## 17. Order of work

1. Registries and the freeze (§11), the template type, its pin and its
   manifest (T1, T2, T6), round-trip tests. Nothing places a block yet.
2. The index with P1–P6 and rings, the site level (I2), the cell and ring
   fixtures of §13, the 48-bit invariant test. `/locate` (R4) and the eye of
   ender can ship here, answering from sites alone for jigsaw structures.
3. Jigsaw layout (§4) and its fixture; the strip height query (L3) measured.
4. Template materialisation into the step slot (T3–T5, M5–M7) and the
   ladder test; villages and outposts appear.
5. The beard term (§6, A4) and its fixture; villages sit on ground.
6. The hardcoded types in the order of their share of the corpus and their
   cost: the template-backed ones (igloo, shipwreck, ocean ruin, ruined
   portal, fossil, end city, mansion), then the scattered three, then the grid
   generators (mineshaft, fortress, stronghold, monument, treasure); each
   adds its row to the "site implies a piece" table.
7. The save (I5), explorer maps with their claim (R5), spawn overrides and the
   cat spawner (I4).
8. The seed searcher (R2, R3) as a tool over the frozen registries, with the
   sieve property test; the Beta dungeon inside `mcrs:beta_populate` (B2) and
   its Poseidon parity fixture.
9. The Bedrock dialect and the `.mcstructure` decoder (K3), against
   Microsoft's trail-ruins example.

---

## Appendix A. The shipped sets

| Set | Placement | Salt | Spacing / separation | Spread | Frequency | Exclusion | Entries (weight) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| abandoned_camp | random_spread | 91231127 | 37 / 8 | linear | — | — | 18 camps (1) |
| ancient_cities | random_spread | 20083232 | 24 / 8 | linear | — | — | ancient_city |
| buried_treasures | random_spread | 0 | 1 / 0 | linear | 0.01, legacy_type_2 | — | buried_treasure; `locate_offset [9,0,9]` |
| desert_pyramids | random_spread | 14357617 | 32 / 8 | linear | — | — | desert_pyramid |
| end_cities | random_spread | 10387313 | 20 / 11 | triangular | — | — | end_city |
| igloos | random_spread | 14357618 | 32 / 8 | linear | — | — | igloo |
| jungle_temples | random_spread | 14357619 | 32 / 8 | linear | — | — | jungle_pyramid |
| mineshafts | random_spread | 0 | 1 / 0 | linear | 0.004, legacy_type_3 | — | mineshaft (1), mineshaft_mesa (1) |
| nether_complexes | random_spread | 30084232 | 27 / 4 | linear | — | — | fortress (2), bastion_remnant (3) |
| nether_fossils | random_spread | 14357921 | 2 / 1 | linear | — | — | nether_fossil |
| ocean_monuments | random_spread | 10387313 | 32 / 5 | triangular | — | — | monument |
| ocean_ruins | random_spread | 14357621 | 20 / 8 | linear | — | — | ocean_ruin_cold (1), ocean_ruin_warm (1) |
| pillager_outposts | random_spread | 165745296 | 32 / 8 | linear | 0.2, legacy_type_1 | villages, 10 | pillager_outpost |
| ruined_portals | random_spread | 34222645 | 40 / 15 | linear | — | — | 7 portals (1) |
| shipwrecks | random_spread | 165745295 | 24 / 4 | linear | — | — | shipwreck (1), shipwreck_beached (1) |
| strongholds | concentric_rings | 0 | distance 32, spread 3, count 128 | — | — | — | stronghold; `#stronghold_biased_to` |
| swamp_huts | random_spread | 14357620 | 32 / 8 | linear | — | — | swamp_hut |
| trail_ruins | random_spread | 83469867 | 34 / 8 | linear | — | — | trail_ruins |
| trial_chambers | random_spread | 94251327 | 34 / 12 | linear | — | — | trial_chambers |
| villages | random_spread | 10387312 | 34 / 8 | linear | — | — | 5 villages (1) |
| woodland_mansions | random_spread | 10387319 | 80 / 20 | triangular | — | — | mansion |

## Appendix B. The shipped structures

| Structure | Type | Step | Adaptation | Jigsaw: pool / size / distance / hack / heightmap / start |
| --- | --- | --- | --- | --- |
| abandoned_camp_* (18) | jigsaw | surface_structures | beard_thin | `abandoned_camp/tent/<biome>` / 2 / 80 / yes / WORLD_SURFACE_WG / 0 |
| ancient_city | jigsaw | underground_decoration | beard_box | `ancient_city/city_center`, anchor `city_anchor` / 7 / 116 / no / — / −27 |
| bastion_remnant | jigsaw | surface_structures | none | `bastion/starts` / 6 / 80 / no / — / 33 |
| pillager_outpost | jigsaw | surface_structures | beard_thin | `pillager_outpost/base_plates` / 7 / 80 / yes / WORLD_SURFACE_WG / 0 |
| trail_ruins | jigsaw | underground_structures | bury | `trail_ruins/tower` / 7 / 80 / no / WORLD_SURFACE_WG / −15 |
| trial_chambers | jigsaw | underground_structures | encapsulate | `trial_chambers/chamber/end` / 20 / 116 / no / — / uniform −40..−20; padding 10, `ignore_waterlogging`, 9 aliases |
| village_* (5) | jigsaw | surface_structures | beard_thin | `village/<kind>/town_centers` / 6 / 80 / yes / WORLD_SURFACE_WG / 0 |
| buried_treasure, mineshaft, mineshaft_mesa | hardcoded | underground_structures | none | |
| fortress | hardcoded | underground_decoration | none | |
| nether_fossil | hardcoded | underground_decoration | beard_thin | `height` uniform 32 .. below_top 2 |
| stronghold | hardcoded | surface_structures | bury | |
| desert_pyramid, end_city, igloo, jungle_pyramid, mansion, monument, ocean_ruin_cold, ocean_ruin_warm, ruined_portal (7), shipwreck, shipwreck_beached, swamp_hut | hardcoded | surface_structures | none | |

## Appendix C. Correspondences in the reference

| What | Where |
| --- | --- |
| The three gates and the cell function | `structure/placement/AbstractSpreadingStructurePlacement.java:89-146`, `RandomSpreadStructurePlacement.java:84-99` |
| The 48-bit mask and the bounded draw | `levelgen/LegacyRandomSource.java:32-49`; `levelgen/BitRandomSource.java:17-32`; `levelgen/WorldgenRandom.java:58-69` |
| The 64-bit climate seed | `levelgen/RandomState.java:66-118`; `levelgen/RandomSupport.java:17-31`; `levelgen/XoroshiroRandomSource.java:16-18, 44-48, 114-139` |
| Rings | `world/level/chunk/ChunkGeneratorStructureState.java:131-196` |
| Set filtering, selection with removal, the start's random | `ChunkGenerator.java:548-687`, `ChunkGeneratorStructureState.java:60-129`, `Structure.java:283-287` |
| Site, biome test, height helpers, the generation context | `structure/Structure.java:138-237, 241-331` |
| The unbearded height query | `levelgen/NoiseBasedChunkGenerator.java:157-166, 204-252` |
| Reference scan and its radius | `ChunkGenerator.java:689-733`, `ChunkPyramid.java:14-32` |
| Jigsaw assembly | `structure/pools/JigsawPlacement.java:51-186, 221-263, 348-593`; `util/SequencedPriorityIterator.java` |
| Pool, elements, aliases | `pools/StructureTemplatePool.java`, `pools/*PoolElement.java`, `pools/alias/PoolAliasLookup.java:19-32` |
| Template NBT, order, transform, placement | `templatesystem/StructureTemplate.java:152-186, 285-466, 510-557, 630-724, 726-867` |
| Processors and rule tests | `templatesystem/StructureProcessorTypes.java:10-20`, `ProcessorRule.java:63-74`, `CappedProcessor.java` |
| Terrain adaptation | `levelgen/Beardifier.java:29-104, 161-227`; `densityfunction/generator/SimpleDensityFunction.java:26-31` |
| The decoration loop's structure slot and clip | `ChunkGenerator.java:386-434, 496-504`; `StructureStart.java:85-108` |
| `/locate`, the ring walk, the presence test | `server/commands/LocateCommand.java:50, 126-128`; `ChunkGenerator.java:177-377`; `structure/StructureCheck.java:59-61, 88-141, 222-259`; `placement/StructurePlacement.java:22-28` |
| Maps, eyes, dolphins, cats, spawn overrides | `loot/functions/ExplorationMapFunction.java:37-44, 81-118`; `item/EnderEyeItem.java:85-94`; `Dolphin.java:437-439`; `npc/CatSpawner.java:40-42`; `ChunkGenerator.java:518-546`; `StructureSpawnOverride.java:12-41` |
| Structure tags | `tags/StructureTags.java:8-40`; `data/tags/StructureTagsProvider.java:18-86` |
| Chunk NBT for structures | `world/level/chunk/storage/SerializableChunkData.java:555-582`; `StructureStart.java:110-125` |
| Template data version and its fixers | `templatesystem/loader/TemplateSource.java:81-88`; `util/datafix/DataFixers.java:2014-2038` |
| Beta populate, dungeons, caves | Poseidon `ChunkProviderGenerate.java:324-363`, `WorldGenDungeons.java`, `MapGenBase.java:7-25`, `MapGenCaves.java:13-198`; Betrock `world/generator/overworld/chunk_gen.cpp:424-433, 528-755`, `shared/feature_gen.cpp:101-241`, `shared/cave_gen.cpp:17-207` |

### Deliberate divergences from the reference

| Topic | Reference | Here | Reason |
| --- | --- | --- | --- |
| Placement, references | statuses with stored results, radius 8 | an index queried by reach | G1, P7, I3 |
| Piece parameters read at placement | live world, first decorating chunk wins | fixed at layout from density heights | M3: the reference's value was route-dependent |
| Order of starts of one structure in a column | hash-table order | ascending start chunk | M5, SD4 |
| Placement flags on pieces | stored, mutated at placement | derived from which column ran | M4 |
| Unseeded draws in pyramid cellar and mansion allays | level random | placement stream | St1 |
| Missing or unreadable template | empty template, generation continues | freeze error | Fe1, T6 |
| Template data version | datafixer from 5011 | pinned, checked | T1 |
| Exclusion-zone cycle | unbounded recursion | freeze error | P3 |
| Presence test for `/locate` | site with pieces discarded, then a chunk load | the index's site level, no chunk | R4, SD11 |
| `/locate` answer | first hit per placement on the first non-empty ring | the same by default; `nearest` as a mode | R4, SD12 |
| `dimension_origin` with a `spawn_target` | spawn-target search | freeze error until a spawn finder exists | §15 |

## Appendix D. The Bedrock dialect

Field by field, from Microsoft's behaviour-pack reference and the wiki
(§10.2). "Same" means the Java name and range; a Bedrock-only value is listed
as the variant it becomes.

| Java | Bedrock | Note |
| --- | --- | --- |
| `structure.type: minecraft:jigsaw` | file under `worldgen/structures`, `minecraft:jigsaw` | `description.identifier` names it; used by `/locate` and `/place` |
| `biomes` (tag or list) | `biome_filters` (entity-filter objects, typically `has_biome_tag`) | resolves to a biome mask like a tag |
| `step` | same, eleven values | |
| `terrain_adaptation` | same four names, default `none` | |
| `start_pool`, `start_jigsaw_name` | same | |
| `size` 0..20 | `max_depth` 0..20 | |
| `start_height` (anchors `absolute`, `above_bottom`, `below_top`) | `constant` or `uniform` with the same anchors plus `from_sea` | `from_sea` is a new anchor variant |
| `project_start_to_heightmap` (six maps) | `heightmap_projection` ∈ `world_surface`, `ocean_floor`, `none` | which generation of the map is unknown (§15) |
| `max_distance_from_center` (required) | same shape; default horizontal 80 | |
| `use_expansion_hack` | absent | treated as `false` |
| `pool_aliases` | same three kinds | |
| `dimension_padding`, `liquid_settings` | same | |
| `spawn_overrides` | absent | none |
| `structure_set.placement.type` | `random_spread`, `concentric_rings` | ring fields undocumented; a Bedrock ring set is a load error |
| `salt`, `spacing`, `separation`, `spread_type` | same; defaults 34 / 8 / `linear` | |
| `frequency`, `frequency_reduction_method`, `exclusion_zone`, `locate_offset` | absent | defaults 1, `default`, none, zero |
| `template_pool.elements[].weight` 1..150 | 1..200 | the type's range is the wider one; Java's freeze check keeps 150 |
| `element_type` (five), `projection` (two) | same, namespaced | |
| `override_liquid_settings` | absent | |
| `processor_list` types (eleven) | `block_ignore`, `protected_blocks`, `capped`, `rule` | the other seven are Java-only |
| `capped.value` | `capped.limit` | |
| rule tests (six `predicate_type`s) | same six, namespaced | |
| position tests: `always_true`, `linear_pos`, `axis_aligned_linear_pos` | `always_true`, `axis_aligned_linear_pos` | |
| block entity modifiers: `passthrough`, `append_static`, `append_loot`, `clear` | `passthrough`, `append_loot`, `clear` | |
| jigsaw block `pool` | `target_pool` | no priorities documented for Bedrock |
| template `.nbt`: `size`, `blocks{pos,state,nbt}`, `palette(s)`, `entities`, `DataVersion` | `.mcstructure`: `format_version` 1, `size`, `structure_world_origin`, `structure.block_indices` (two layers, `z,y,x` order, `−1` void), `structure.palette.default.block_palette{name,states,version}`, `block_position_data{block_entity_data,tick_queue_data}`, `structure.entities` | one template type, two decoders |

## Appendix E. The Beta dungeon, in draws

From `WorldGenDungeons.java` (Poseidon) and `feature_gen.cpp:101-241`
(Betrock), which agree. Per attempt, after the three position draws of B2:

| Step | Draws | Rule |
| --- | --- | --- |
| Size | `nextInt(2) + 2` for `x`, then for `z` | half-extents 2 or 3; interior height 3; always drawn |
| Scan | none | over `[x − l − 1, x + l + 1] × [y − 1, y + 4] × [z − i − 1, z + i + 1]`; any non-buildable block in the floor or ceiling plane rejects; count wall-ring blocks at `y` with air at `y` and `y + 1` |
| Gate | none | accept iff the count is in `1..5` |
| Shell | `nextInt(4)` per buildable floor-layer shell block, in `x` ↑, `y` ↓, `z` ↑ order | interior → air; a shell block over non-buildable → air; floor → mossy cobblestone unless the draw is 0; walls and ceiling → cobblestone |
| Chests | up to 2 chests × 3 tries: `nextInt(2l + 1) − l`, `nextInt(2i + 1) − i` | placed iff the spot is air and exactly one cardinal neighbour at `y` is buildable |
| Loot | 8 rolls of `nextInt(11)`, then per item as below; `nextInt(27)` for the slot only when the item is non-null | later items overwrite earlier slots |
| Spawner | `nextInt(4)` | 0 skeleton, 1–2 zombie, 3 spider; at the attempt's origin |

Loot by roll: 0 saddle; 1 iron ×`nextInt(4)+1`; 2 bread; 3 wheat ×`nextInt(4)+1`;
4 gunpowder ×`nextInt(4)+1`; 5 string ×`nextInt(4)+1`; 6 bucket; 7 golden apple
iff `nextInt(100) == 0`; 8 redstone ×`nextInt(4)+1` iff `nextInt(2) == 0`;
9 a record, `13` or `cat` by `nextInt(2)`, iff `nextInt(10) == 0`; 10 cocoa
beans (`WorldGenDungeons.java:123-127`). The order of the conditional draws is
the source's and is copied, not the table's.
