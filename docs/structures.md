# Structures: placement, layout, materialisation and search

A specification for the structure part of `worldgen.md` and for the structure slot that
`scattering.md` leaves open: where a structure stands, what shape it
takes, how the terrain bends to it, how its blocks reach a column under the
ladder of `scattering.md` §3.2, and how a structure is found, by `/locate`,
by an explorer map and by an eye of ender.
Jigsaw assembly and the template system are the largest part of it because 28
of the 52 shipped structures are jigsaw structures and every one of them is
data.

`worldgen.md` is the authority on invariants and vocabulary; `scattering.md`
owns the ladder, the window and the read rule (Wn1–Wn5) that materialisation
runs under; `heightmap.md` owns the maps.

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
worth restating in the reference's own terms: the layout step runs at the
`structure_starts` status, which has no requirement on any other chunk, and everything a layout
consults is either a registry, the climate field, or the density column evaluated **without**
a terrain adaptation term, unlike a fill, which passes the chunk's sampler fields. So a start depends on no other start, the layouts of two cells are
independent, and the index of §8 is a pure function of the seed that can be
computed in any order on any thread. Search (§9) reads the same four layers
and nothing else, which is why it needs no world.

Only materialisation touches the world, and it is the one `scattering.md`
already answers: a structure's blocks are placed by the column's own program,
in the step slot S6 reserves, under the read rule Wn2.

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
(structure set;
registers `random_spread`, `concentric_rings`, `dimension_origin`). The two
spreading types share `salt` (required, non-negative), `frequency` (0..1,
default 1), `frequency_reduction_method` (default `default`), `locate_offset`
(each axis within ±16, default zero) and an optional `exclusion_zone` of
`{other_set, chunk_count 1..16}`, which the reference marks deprecated.
`random_spread` adds `spacing` and `separation`, both 0..4096 with
`spacing > separation` enforced by the codec, and `spread_type` `linear` or
`triangular`.
`concentric_rings` adds `distance` 0..1023, `spread` 0..1023, `count` 1..4095
and a biome set `preferred_biomes`.
`dimension_origin` has no fields — it is not a spreading placement and carries
no salt, frequency, offset or exclusion — and matches the *dimension origin*
only. The origin is
chunk `(0, 0)` unless the
noise settings carry a non-empty `spawn_target`, in which case it is the chunk
of the spawn-target search over the climate field, a function of the full
seed. No shipped set uses it. Every range is a freeze check (Fe1).

**P2. The cell function.** For `random_spread`, with chunk coordinates `(x, z)`:

```
gx = floorDiv(x, spacing);  gz = floorDiv(z, spacing)
r  = LegacyRandom(gx·341873128712 + gz·132897987541 + seed + salt)
ox = spread(r, spacing − separation);  oz = spread(r, spacing − separation)
cell(gx, gz) = (gx·spacing + ox, gz·spacing + oz)
starts_at(x, z) ⟺ cell(gx, gz) = (x, z)
```

floor division rounds toward negative infinity; the seeding is
the salted seeding over a
`LegacyRandomSource`, the 48-bit congruential generator, never Xoroshiro; `x` is drawn before
`z`; `linear` is one `nextInt(limit)`, `triangular` is the integer mean of two.
Three draws and one hash per cell: the cheapest thing in this document, and

**P3. Three gates, in this order, each a pure function of the seed.**
the chunk test is the conjunction of the cell test, the frequency gate and
the exclusion gate, short-circuited in that order. The frequency gate runs only
when `frequency < 1` and has four spellings, which are data because the corpus
uses three of them (`AbstractSpreadingStructurePlacement`):

| Method | Seed | Test | Corpus user |
| --- | --- | --- | --- |
| `default` | salted seeding (seed, salt, x, z) — the salt takes the `x` slot and `z` becomes the blend | `nextFloat() < f` | none |
| `legacy_type_1` | `setSeed((x ≫ 4) ^ ((z ≫ 4) ≪ 4) ^ seed)`, then one `nextInt()` discarded | `nextInt(⌊1/f⌋) == 0` | pillager outposts |
| `legacy_type_2` | salted seeding (seed, x, z, 10387320) | `nextFloat() < f` | buried treasure |
| `legacy_type_3` | large-feature seeding (seed, x, z) | `nextDouble() < f` | mineshafts |

The argument order of `default` is not a typo to correct: it is the function.
The exclusion gate asks whether the other set has a start anywhere in the
`(2n+1)²` square around `(x, z)`, by running the other set's full three-gate
test on every chunk of that square.
A set may exclude a set that excludes it; the reference would recurse forever. A cycle is a freeze error (§11). One more
fact about the gates that §9 needs: the reference's *cheap* presence test runs
the cell test and the frequency gate but not the exclusion gate; the exclusion gate is re-applied when
the chunk is generated. Here the three gates are one function and every
consumer sees all three.

**P4. Rings are a per-dimension constant.** For `concentric_rings` the set of
start chunks is computed once per dimension from the seed alone: a `LegacyRandom` seeded with the
world seed (zero for flat worlds) draws an angle, then for each of
`count` positions a distance `4·d + 6·d·ring + (nextDouble() − 0.5)·2.5·d` in
chunks along the current angle, forks a child source, and snaps the position
to the nearest chunk whose biome is in `preferred_biomes` within 112 blocks of
the candidate's centre by the reference's horizontal biome search with
reservoir sampling on the fork; the angle advances by `2π/spread`, and when a ring fills, `spread`
grows by `2·spread/(ring+1)`, is clamped to what remains, and the angle jumps by
a fresh draw. Strongholds are 128 positions on rings of 3, 6, 10, 15, … with
`distance 32`. The fork per position is what makes the biome searches
independent; the reference fans them out to a pool, and so may we, after the
forks are drawn in sequence. The angles and distances are 48-bit facts; the
snap reads the climate field and is a 64-bit one (R1).

**P5. Sets are filtered by the dimension, structures by the biome at the
site.** A set is live in a dimension when at least one of its structures names
a biome the dimension's source can produce; within a live set a structure is a
candidate when it names such a biome (the placement list is built per structure — a structure in two sets has two placements,
and search groups by placement for that reason). The biome test proper happens
after the site is chosen, at the site (§3, L2). The possible biomes of a
source is the set that `scattering.md` §4.3 fixes.

**P6. Selection within a set is a draw with removal.** In a start chunk of a
set with one entry, that entry is tried. With several, a `LegacyRandom` seeded
by large-feature seeding (seed, x, z) (reseed, two
`nextLong`s, then `x·a ^ z·b ^ seed`) draws `nextInt(total weight)`, walks the
list in JSON order subtracting weights, tries the entry it lands on, and on
failure removes it, lowers the total and draws again from the same stream
until an entry yields a valid start or the list is empty. So the JSON order of the entries is part of the
definition, and the number of failed tries is too: it shifts the stream. The
structure tried gets its own fresh `LegacyRandom` seeded the same way, so a layout never sees the selection draws. One
chunk holds at most one start per *structure*, and a set whose structure
already has a valid start in the chunk is skipped, which is
unobservable under P7 because the index computes each chunk exactly once. The
one set with many entries is `abandoned_camp` (18 camps, weight 1 each): a
cell of that set almost always places *some* camp, whichever biome-valid
variant survives the loop.

**P7. The index is the placement.** `worldgen.md` §10 rejected placement as a
stage; nothing in P1–P6 reads a column, so nothing needs one. The reference's
`structure_starts` status, its radius-8 requirement on four later statuses and its `structure_references` scan are how a per-chunk store answers "which starts
reach me"; §8 answers the same question from the seed.

---

## 3. Layout: from a placed cell to a start

**L1. A start is a chunk, a structure and a non-empty list of pieces.** The
reference's start is exactly that plus a counter; it is valid iff it has a piece, and its bounding box is the union of the piece boxes, inflated by
12 on every axis when the structure has a terrain adaptation. A layout that produces no piece is a
rejection, and a rejection returns to P6's loop.

**L2. The site is chosen first, tested for biome second, and the pieces are
built third.** the layout function yields a position and a deferred piece
builder; the biome of the *chosen position*, sampled at quart resolution
including its `y`, must be in the structure's `biomes`; only then is the
builder run. The `y` is
not decorative: a stronghold is tested at `y = 0`, a fortress at 64, a
mineshaft at `50 + offset`, a surface structure at the first occupied height
of its heightmap. One structure builds its pieces *before* the test: the
mineshaft carries a filled builder in its stub,
so its draws happen whether or not the site passes. For a jigsaw structure the
site step is exactly: the `start_height` draw, the centre rotation, the centre
element, the optional named-jigsaw shuffle, the centre's box from the
template's *size*, the optional column-biome test and free-height query, and
the padding test; every piece after the
centre is built inside the deferred closure. The site is a
necessary condition for a start and is what search pays for (R6); the
layout is what fill and run pay for.

**L3. Heights come from the density column, not from blocks.** Every height a
layout reads is the base height query: the density tape evaluated over one strip,
substance applied by the aquifer, walked top-down until the heightmap
predicate holds, with no beardifier and no blending. `WORLD_SURFACE_WG` stops at
the first non-air cell, which includes water; `OCEAN_FLOOR_WG` at the first
motion-blocking one. The helpers are
few and all corner-shaped: the chunk-centre column
(at block 8,8), the four corners of a box for a mean
or a minimum, and the four corners of a 5×5 box offset by the
rotation (used by end cities and mansions). The consequence is the
one that matters: **a layout's heights ignore carving, surface rules and every
other structure**, so the layout is a function of the seed and of nothing that
any stage of the ladder writes. Under §7 the *materialised* piece will then sit
on carved, surfaced ground that the layout never saw; that is the reference's
behaviour too, and the beardifier of §6 exists to close the gap.

**L4. A jigsaw structure is data; the other fifteen types are algorithms whose
draws are the data.** the structure type registers sixteen
`type`s. `jigsaw` reads its whole layout from the registries (§4). The others —
`buried_treasure`, `desert_pyramid`, `end_city`, `fortress`, `igloo`,
`jungle_temple`, `mineshaft`, `nether_fossil`, `ocean_monument`, `ocean_ruin`,
`ruined_portal`, `shipwreck`, `stronghold`, `swamp_hut`, `woodland_mansion` —
are Java methods whose observable content is the sequence of draws they make
and the boxes they emit. For those the prescription of F2 applies: the draw
order is copied verbatim, because a draw moved by one changes every piece
after it. What is *not* copied is the object graph: a piece is data (L6), a
generator is a function from a seeded source to a piece list, and the static
mutable tables the reference keeps between calls (the stronghold and fortress piece weights with their place counts) are locals of that function. Their per-type
parameters are the `type`-specific JSON fields: `mineshaft_type`, `is_beached`,
`biome_temp`/`large_probability`/`cluster_probability`, the ruined portal's
`setups`, the fossil's `height`; everything else is the shared settings:
`biomes`, `spawn_overrides`, `step`, `terrain_adaptation`. The complete list of what a layout may read is the
reference's generation context: the registries, the
height and biome queries, the template manager, the 48-bit random of P6, the
raw seed, the chunk, the dimension's height range and the biome predicate —
and nothing that is not a function of those.

**L5. Every structure has a reach, and it is at most eight chunks.** A piece
farther than eight chunks from its start chunk is not part of the world: the
reference's reference scan stops there, and a jigsaw structure's
`max_distance_from_center.horizontal` plus its 12-block adaptation margin may
not exceed 128. Fortresses and
strongholds bound their pieces to 112 blocks from the start,
mineshafts to 80, and end cities and mansions
are bounded only by the cap. The index of §8 needs the bound *before* the
layout exists, and takes the reference's: every structure is looked for
within eight chunks. A tighter reach derived from `horizontal + margin` is not
a bound, because the pieces are held within that distance of the centre
piece's box centre, not of the start chunk, and a rotated centre piece moves
that centre outside the chunk; such a reach can fall a chunk short and drop
the start from the columns at the edge of its box. The actual bounding box,
once known, is what the query filters on.

**L6. A piece is its serialised form.** The reference writes each piece as
`id`, `BB`, `O`, `GD` plus type-specific fields, and a jigsaw piece as its
element, position, rotation, `ground_level_delta`, junctions and liquid
settings; the start as `id`,
`ChunkX`, `ChunkZ`, `references`, `Children`. Under the serde rule that layout *is* the Rust
type — one enum over piece kinds, each variant a typed struct, deriving the
codec that writes the save and the one that would cross the wire — and there
is no second in-memory shape. Two fields carry more than their name: `GD` is a
collision tag for end cities and a room flag for monuments and is stored verbatim; `references`
is gameplay truth (how many explorer maps have claimed the start, at most one,
a start) and is the one field a layout never sets.

---

## 4. Jigsaw assembly

The corpus: 245 template pools, 40 processor lists, 28 jigsaw structures,
1511 templates. This section is the reference's as a
definition rather than a class; the algorithm is copied draw for draw (L4),
the data structures are not.

**J1. The structure's fields.** `start_pool`, optional `start_jigsaw_name`,
`size` 0..20 (the depth), `start_height`, `use_expansion_hack`, optional
`project_start_to_heightmap`, `max_distance_from_center` as an int 1..128 or
`{horizontal 1..128, vertical 1..4064}`, `pool_aliases` (default empty),
`dimension_padding` as an int or `{bottom, top}` (default 0), `liquid_settings`
(default `apply_waterlogging`). A pool is `fallback` plus `elements` of
`{element, weight 1..150}`, and the weights are *expanded*: the pool's working
list holds each element `weight` times in JSON order, so a weight changes both
the odds and the number of draws a shuffle spends. An element is dispatched on
`element_type`: `single_pool_element` and `legacy_single_pool_element`
(`location`, `processors`, `projection`, optional `override_liquid_settings`),
`list_pool_element`, `feature_pool_element`, `empty_pool_element`. The legacy form differs from the
single form in one thing: its ignore processor drops air as well as structure
blocks and sits *last* in the chain, so a legacy template never carves the
terrain. Aliases are `direct`,
`random`, `random_group`, resolved in list order into a map by a
`LegacyRandom` positioned at the pre-projection start position; a duplicate alias is a freeze error.

**J2. One stream, in one order.** The layout random is the start's
`LegacyRandom` of P6. Its first draw is `start_height.sample`; then, in the assembly: the centre rotation (`nextInt(4)` over
`NONE, CW90, CW180, CCW90`); the centre element;
if `start_jigsaw_name` is set, the centre's jigsaw blocks are *shuffled* with
this stream before the named one is found (J4). Then the queue of J3 runs and
each child step spends, per source jigsaw: a shuffle of the target
pool's expanded list **only if `depth ≠ size`**, a shuffle of the fallback's
expanded list **always**, and per candidate element a shuffle of the four
rotations (three draws) and a shuffle of the candidate's jigsaw blocks
(a Fisher–Yates shuffle from the end,
`n−1` draws). Skipping the fallback shuffle at maximum depth because "it cannot
matter" desynchronises every draw after it.

**J3. The queue is priority, then FIFO, and higher priority pre-empts.** The
centre piece's children are tried first; every accepted piece with
`depth + 1 ≤ size` is enqueued under its source jigsaw's `placement_priority`;
the next piece processed is the first of the highest non-empty priority.
A piece at depth `size` is still processed but sees only fallbacks (J2). The
piece list is the centre followed by acceptance order, and the first piece is
what materialisation's reference position is taken from (§7).

**J4. A jigsaw block list is sorted by selection priority, ties in shuffle
order.** The blocks of a template are the palette's `minecraft:jigsaw` entries
in template order (T2), transformed to the piece's rotation, shuffled with the
stream, then stably sorted by `selection_priority` descending. The palette used to enumerate
them is chosen by a positional random of the piece position, and for a
*candidate* the position is the origin, so the palette is index
`hash(0,0,0) mod n` — which can differ from the palette later placed at the
real position.
Twenty of the 1511 templates carry several palettes. The quirk is data.

**J5. Attachment.** For each source jigsaw, with target position
`source + front`, a candidate `(element, rotation, jigsaw)` attaches iff
the attach test: the target's front is the source's opposite, the tops agree or
the source's joint is `rollable`, and the source's `target` equals the target's
`name`; the first candidate that
also fits (J6) wins and the loop moves to the next source jigsaw. An `empty_pool_element` met in the candidate
list *breaks* the list: nothing behind it is tried. The vertical
placement is where projection enters: with both sides `rigid`, the target box
is placed at `source box minY + Δy` where `Δy = sourceJigsawLocalY −
targetJigsawLocalY + front.stepY`; with either side `terrain_matching`, the
source jigsaw's column is queried once (L3, `WORLD_SURFACE_WG`, the *free*
height, i.e. one above the occupied) and the target box's `minY` is that height
minus the target jigsaw's local `y`. The `ground_level_delta` of a
rigid target is the source's minus `Δy`; of a terrain-matching one, the
element's own (always 1). Every accepted attachment records two
junctions, one on each piece, holding the *other* jigsaw's `x`, `z`, a ground
`y` and `Δy`; they are the only junctions there are and §6 reads
them.

**J6. Occupancy is inclusive integer box arithmetic.** The reference tests a
candidate box, deflated by a quarter block, against a voxel shape that starts
as the reach box minus the centre piece and loses every accepted box. For integer boxes the deflation makes the test exact:
a candidate fits iff its box lies within the context's bounds and intersects no
box already subtracted from that context, with
semantics (both ends inclusive). There are two kinds of context: the lineage's
shared one, whose bounds are the reach box `[c − h, c + h]` horizontally and
`[max(c − v, minY + bottomPad), min(c + v, maxY − topPad)]` vertically, and a per-piece interior one, created the first time a target
position falls *inside* its source piece's box, bounded by that box. A subtracted box is subtracted from the context the attachment
used, and children of a piece inherit the context their attachment used. The
expansion hack grows a candidate's *occupancy* box upward to
`max(expandTo + 1, height)` when the flag is on and the candidate is at most 16
tall, where the expansion is the tallest element of any pool a jigsaw of the
candidate would attach inside it. The grown box is not
only occupancy: it is the box the accepted piece is constructed with and keeps, so it is what the start's box unions
(L1) and what the beardifier reads (§6); only a piece reloaded from the save
recomputes its box from the element. A list of boxes per context is the
definition.

**J7. The centre is projected, then padded, then checked.** With
`project_start_to_heightmap` the whole column at the centre's `(x, z)` must
admit a valid biome (L2's column form), and the centre's ground row
(`minY + ground_level_delta`) is moved to `start_height + freeHeight(x, z)`; `dimension_padding` then rejects a centre whose box crosses the
padded floor or ceiling; the biome test of L2 runs at
`(centreX, bottomY + anchorY, centreZ)`.

**J8. Nothing in J1–J7 reads a block.** The only world queries are L3's height
and the biome field. Hence a layout is reproducible against the reference bit
for bit given the same seed and registries, and §13 tests it so.

---

## 5. Templates and processors

**T1. A template is an NBT file and its Rust type is that file.**
`structure/<path>.nbt`, gzip-compressed, root keys `size`, `blocks`
(`pos`, `state`, optional `nbt`), `palette` or `palettes` (20 files), `entities`
(172 files), `DataVersion`.
The type derives `Deserialize`/`Serialize` through `mcrs_minecraft_nbt`; the palette
entries resolve to `VoxelId` at freeze. Every shipped template carries
`DataVersion` 5011 against a world version of 5124. The reference runs the
structure datafixer over that gap;
the fixers registered between 5011 and 5124 that reach a template rename
explorer-map items and strip one data component, and a scan
of all 1511 files finds neither. So the template version is pinned at 5011 as
a named constant and checked at load; a file at any other version is a loud
error, as the project rule demands, and the pin moves when the corpus does.
The Rust type is the template, not the file; nothing downstream knows
which file it came from.

**T2. Block order is part of the definition.** After load the blocks are split
into three lists — with `nbt`; else "full" (collision shape is a full block
and not dynamic); else "other" — each sorted by `(y, x, z)` and concatenated
full, other, block-entity. This
order is the jigsaw enumeration order of J4 and the placement order of T4; it
is computed once at freeze and is why the block predicate "is a full block"
must resolve at freeze from the block schema.

**T3. Transform.** Mirror first (`LEFT_RIGHT` negates `z`, `FRONT_BACK`
negates `x`), then rotation about the pivot `(px, pz)`:
`CCW90 → (px − pz + z, y, px + pz − x)`, `CW90 → (px + pz − z, y, pz − px + x)`,
`180 → (2px − x, y, 2pz − z)`; entities use the continuous form
with `+1` terms. A piece's bounding box is the transformed box of
`(0,0,0)`–`(size − 1)` moved to the piece position, so for a
rotated piece the box minimum is not the position, which is why J5 tracks both.
Jigsaw pieces never mirror and always pivot at zero.

**T4. Placement is one pass in one order with one clip.** placement: choose the palette (J4's positional draw at the piece position);
for each block in T2 order compute its world position and **skip it before any
processor runs** if it lies outside the placement box — unless a `capped`
processor is in the chain, which sees the whole piece; run the
processors in chain order until one drops the block; run every processor's
the finalise step in chain order; then set the survivors, block entities
with a barrier placed first, loot seeds drawn from the *placement* random, and
waterlogging by the liquid rules. The chain for a jigsaw piece is
`[block_ignore(structure_block), jigsaw_replacement, …processor_list…,
gravity if terrain_matching]`, update flags 18, shape updates disabled by
known shape. `scattering.md` §5.2
declines the live-world shape updates; jigsaw pieces do not request
them.

**T5. Processors are a `type`-dispatched enum; their randomness is
positional.** Eleven types;
the corpus uses `rule` (39 lists), `protected_blocks` (7), `block_rot` (6) and
`capped` (4), and code injects `block_ignore`, `jigsaw_replacement` and
`gravity`. `rule` draws from `LegacyRandom(hash(worldPos))` shared across a
block's rules, in rule order, `input → location → position` short-circuited;
`block_rot` and `block_age` from the settings' positional random; `capped`
from a positional fork of the world seed at the piece position. Ten rule tests and three position tests are
`predicate_type`-dispatched enums, of which `RuleTest` already exists as a
serde type (`crates/mcrs_minecraft_worldgen_feature/src/rule_test.rs`, `RuleTest`), as
does the processor list itself, reached today only through the fossil and
template features (`crates/mcrs_minecraft_worldgen_feature/src/proto.rs`, `GeodeLayerSettings`, `StructureProcessor`). A `location_predicate`,
`protected_blocks` and `lava_submerged_block` read the *world* block at the
target position: under Wn2 that is the filled window plus the column's own
writes, and §7 says what that means for a piece placed from several columns.

**T6. Templates load eagerly, once.** 1511 files, 7.7 MB compressed, 43 MiB of
raw NBT. Fe1 says validate at load: every template a pool names must exist,
parse at the pinned version, and resolve its palette, or the freeze fails
naming the file. The reference substitutes an *empty* template for a missing
or unreadable one and carries on, so
that a pool with a typo places nothing silently; that is the failure mode Fe1
forbids. The packed size after freeze (a `u16` position and a `VoxelId` per
block, one list per template) is a measurement, not a guess; if it is too large
to hold, the fallback is a per-pool lazy load behind the same validated
manifest, never an unvalidated one. Two rules ship. A template a hardcoded
type names (its `TEMPLATES` list) must load or the freeze fails naming the
structure and the file. A template a pool element names but the byte-exact
corpus does not ship — there is one, `ancient_city/walls/intact_horizontal_wall_stairs_5`
— freezes to an empty template with the element's weight still counted: that
is what the reference does with the same data, so the element draws as in
vanilla and places nothing, and refusing it would refuse the shipped corpus.
Whatever the outcome, the *manifest* —
each template's id, size, palette count and jigsaw blocks — is a separate,
small, frozen table: it is all the site step (L2) ever
reads.

---

## 6. Terrain adaptation

`worldgen.md` §10 states B1–B4; this section supplies the data and settles the
node B4 owes.

**A1. The term.** For a column `U`, collect every start within reach whose
structure has `terrain_adaptation ≠ none`, and from each start every piece
whose box comes within 12 blocks of `U`'s footprint. A jigsaw
piece contributes as a *rigid* only if its projection is `rigid`, with its
`ground_level_delta`; any other piece contributes as a rigid with delta 0; and
every junction of a jigsaw piece whose `(x, z)` lies strictly inside the
footprint inflated by 12 contributes as a point. The union of the
collected boxes and points, inflated by 24, is the affected box; outside it the
term is exactly zero.

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

(`Beardifier`). `K` is 13 824 floats built once per process; the
`invsqrt` is the reference's fast inverse square root, part of the strict
profile's function. The corpus assigns `beard_thin` to villages, outposts,
camps and fossils, `beard_box` to the ancient city, `bury` to strongholds and
trail ruins, `encapsulate` to trial chambers; twelve structures adapt, the
rest do not.

**A3. The term is added per block at the root of `final_density`.** In the
shipped overworld graph the leaf sits as
`add(min(squeeze(interpolated(…)), noodle), beardifier)` — outside the
interpolation, which is B3. Adding at cell
corners puts the term inside the squeeze and trilerps it.

**A4. The term is added after the graph, not bound inside it.** The compiler
folds `minecraft:beardifier` to a declared constant zero with the infinite
interval (`crates/mcrs_minecraft_worldgen_density/src/compile.rs`, `compile`), and the fold
stays. Because the beardifier is the outermost `add` of `final_density` in
every shipped router (A3), a fill evaluates the graph over its volume and then
adds A2 into the density of every block of the volume the affected box
reaches, before the substance loop reads it — the aquifer decides on bearded
density, as in. That is the
graph's value bit for bit, up to the sign of a zero density, which no block
choice reads. A cell whose block box meets the affected box is filled block by
block without asking its interval bound; outside the box the term is exactly
zero and the bound holds unchanged. The term is a per-fill temporary written
into the fill's own density, not a stored projection; its owner is the fill
task. The shape is the precondition, so it is checked where the router is
built: a dimension with a live structure refuses a `final_density` that names
the beardifier anywhere but as an operand of its root `add`, and one with a
live adapted structure refuses a root that is not `add(_, beardifier)` in
either order. 

**A5. Layout heights ignore the term (L3), so starts stay independent.** The
reference passes no beardifier to the height query. Were the query bearded, a
village's site would depend on the outpost next to it, the index of §8 would
stop being per-cell, and P7 would fall. This is the invariant that makes the
whole of §2–§4 parallel.

---

## 7. Materialisation

Piece placement is not a stage. It is part of `Run`.

**M1. The write footprint is one column, like every scattered object.** The
reference clips each piece to the chunk's writable box — its own 16×16, from
one above the floor to the ceiling — and that clip holds for template pieces and
for the grid generators' block placement. Three hardcoded pieces widen it: the
ruined portal encapsulates its whole box and spreads netherrack within 14
blocks of its centre from the one column that holds the centre, the nether fossil
encapsulates its box from every column it touches, and buried treasure and the igloo write a
neighbour or two unclipped. Every such write lands within one column of the
placing column, which is the radius the reference's region permits, and
which Wn5 already routes into a delta. So `r_w = 1`, the ladder of §3.2 needs
no fourth stage, and a write beyond the ring is the data error S4 already
names.

**M2. The read rule is Wn2, and it changes what a piece may compute.** The
reference reads the *live* world at placement — heightmaps for the ground of a
pyramid, blocks for a mineshaft's supports — and freezes some of what it finds
into the piece for every later chunk ( stores
the mean height of the columns *inside the first decorating chunk*;
 likewise; rewrites
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
region (the four-corner minimum or mean), or
(b) computed by every placing column from the `Filled` snapshots only, never
from own writes, and only when the reads lie within the 3×3 of *every* column
that can place the piece, which is a check on the piece's extent at freeze.
(a) is the default and is what every type but one does: the scattered
pieces' `HPos` is the mean (jungle temple, swamp hut) or the four-corner
minimum (desert pyramid) of the `MOTION_BLOCKING_NO_LEAVES` density heights,
the shipwreck's and ocean ruin's floor the four-corner or footprint walk over
`OCEAN_FLOOR_WG` (`WORLD_SURFACE_WG` for a beached wreck), the igloo's
entrance height `WORLD_SURFACE_WG` at the entrance column; a draw the
reference takes beside such a read only in the first decorating chunk (the
beached wreck's `nextInt(3)`) moves to the end of the layout stream, and a
draw it takes on every call (the pyramid's `nextInt(3)`) keeps one burn per
column. (b) is used by the one piece whose extent admits it: buried treasure
reads `OCEAN_FLOOR_WG` of its own column's window at placement and walks down
to its resting block, leaving the piece box as laid out. Per-column decisions the reference
makes per chunk — the mineshaft's flooded-shell test on its own clipped box, a mansion's cobblestone footing under its
own columns — stay per column and read
the window as any object does.

**M4. Placement flags are derivable and are dropped.** the chest flag,
the trap flag, and their kin exist so that a re-run of a chunk's
decoration does not place a chest twice. Every block belongs to one column,
and a column's run is bit-identical on re-run (St1), so the flag equals "the
column holding that block has run" and is not stored. The one flag whose
block is drawn per chunk, the mineshaft corridor's spider flag, is
replaced by a column chosen at layout: chunks decorate in the reference in
the order they load, which players and pregenerators walk outward from the
origin, so the column nearest the origin among those holding a section
whose three candidate cells lie inside it under the ocean floor places the
spawner, columns nearer than it draw the reference's `nextInt(3)` per
section and place nothing, and columns farther draw nothing, as the
reference's chunks do once the flag is set. A reloaded piece carries no
host, since the reference's tag has no slot for one (the save). The reference's
unseeded draws — a pyramid's cellar, a
mansion's allay count — draw from the
placement stream instead, the same substitution St1 makes for spawning.

**M5. The step slot, and the stream inside it.** In step `s` a column first
places the starts of every structure whose `step` is `s`, in the registry
order of structure ids (path, then namespace:),
each structure reseeding the decoration source with
`decorationSeed + index + 10000·s` — the decoration seed being S2's, whose two
longs pass through — where `index` counts structures of that step
in that order whether or not they have a start here; then the step's features
run with their own indices (S6). All starts of
one structure that reach the column share one stream in sequence, in the order
of their source chunks; the reference's order is a hash table's. Decoration parity is
unattainable regardless (`worldgen.md` §15), so the order here is ascending
`(x, z)` of the start chunk, an input of the configuration hash. Every piece of
a start whose box meets the column is placed, in piece order, with the start's
reference position — the first piece's box centre at its floor — for the
position tests of T5; then the structure's
after-place hook, which two structures define.

**M6. Rungs.** The current ladder cuts the steps into rungs at
`vegetal_decoration` and `top_layer_modification`
(`crates/mcrs_minecraft_worldgen_generator/src/feature_program.rs`, `CompiledFossil`, `CompiledElement`).
Every structure step — `underground_structures` 3, `surface_structures` 4,
`strongholds` 5, `underground_decoration` 7 — lies in the first rung, so
materialisation never straddles a cut and a structure's blocks are merged
before any plant asks whether it can stand on them.

**M7. Block entities are data until delivery.** Chests, spawners, jigsaw
blocks left as `final_state`, brushable blocks: `scattering.md` gives the
column a typed list, and a piece appends to it. A loot table with its seed from
the placement stream (T4) is such an entry; an entity a piece spawns is a
second list of the same shape, `Vec<GeneratedEntity>` beside the block
entities, one typed entry per kind (witch, cat, elder guardian, drowned with
its chicken and nautilus mounts, shulker, item frame with elytra, evoker,
vindicator, allay, the igloo's villager and zombie villager, chest minecart),
each with the position, rotation, UUID and equipment its spawn setup draws
in reference order from the placement stream (St1). Delivery spawns them as
mobs linked to their section, a tracker pairs them to players in range and
removes them when the section unloads; nothing ticks them and nothing saves
them. The entities of a jigsaw template (172 templates carry some) are
frozen and counted but not placed.

---

## 8. The index

**I1. Classification.**

| Value | Category | Owner | Note |
| --- | --- | --- | --- |
| Structure sets, structures, pools, processor lists, templates, the frozen tables | truth | the freeze build | cloned into the sub-app once, like the routers (`crates/mcrs_minecraft_vanilla_worldgen/src/routers.rs`, `DimensionRouters`, `build_dimension_routers`) |
| Ring positions per dimension | projection of the seed | the index, at construction | P4; a few hundred kilobytes of biome sampling, once |
| `site(C, S)` for a chunk `C` and structure `S` | projection of the seed | the index, memoised per chunk | L2; cheap; what search asks (R6) |
| `starts_at(C)` for a chunk `C` | projection of the seed | the generation stage's staging store, by tile of 16 by 16 chunks (I2) | P2–P6 then §3–§4; expensive (a village layout), reused by fill, run and gameplay |
| The beard term of a fill | derived, temporary | the fill task | A4; added into the fill's density, never stored |
| `starts.<id>` and `References` in the chunk NBT | projection, written for the format | the save writer | I5 |
| A start's `references` counter | **truth** | gameplay, the map that claims it | L6 |
| A structure's blocks in a column | projection of the seed, until edited | the column's run | §7; `worldgen.md` §16 |

**I2. Shape and owner.** One index per dimension holds the frozen tables and the ring positions, and nothing in it changes after construction. It computes and does not keep: a start (§3, §4) is a pure function of the seed and the chunk, and so is a site (L2). Starts are a product of the ladder, like the snapshots and the deltas (`scattering.md` §3.2). The generation stage (`crates/mcrs_minecraft_vanilla_worldgen/src/generation_stage.rs`, `GenerationStage`) keeps them in its staging store, one entry per tile of 16 by 16 chunks. A pool task computes a tile's starts with one view of the terrain (`StructureIndex::tile_starts`) and sends them back as a message; only the stage inserts and evicts them, so the store has one writer and needs no lock. The chunks within eight of a column (L5) form a 17 by 17 square, which always meets exactly two by two tiles: a fill or a run is ready when those four tiles are in the store, and `StructureIndex::reaching` composes them into the starts of I3, in the order M5 gives. No two tasks compute one tile at once, and a tile computed again is bit-identical (St1). Search (§9) keeps nothing either: each site is computed when it is asked. Fill and run tasks, and the consumers of I4 in the sub-app, receive the index as an owning handle (X7).

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
position (`spawn_overrides.bounding_box` is
`piece` or `full`; the first structure in
iteration order with a matching override wins, and the nether fortress is
special-cased before the generic path);
every search of §9; and the save writer (I5). Each reads the same memo through
the same query; none writes it, except the map's claim (R5).

**I5. The save.** Vanilla tools and vanilla servers reading our world expect
`structures.starts` keyed by structure id in the start chunk and
`structures.References` keyed by structure id in every chunk a start reaches, both as
`mcrs_minecraft_nbt` writes the types of L6. `starts` is written for the chunks the memo
holds; `References` is recomputed at write time by I3, because it is a
projection and the memo answers it. On load, a stored start *primes* the memo
for its chunk and the seed is not consulted for it; absent, the seed derives
it. The two agree whenever the configuration hash matches (St3) — its inputs
now include the structure registries' content hash, the template version pin
(T1) and the ordering rules of M5 — and the hash is checked at load, so a
world whose datapack changed is refused rather than seamed. A stored
`references` counter is truth (L6) and never regenerated.

**I6. Eviction.** The starts leave the staging store through the store's own `retain`, beside the snapshots and the deltas (Q7), and a tile evicted and computed again is bit-identical (St1). A start is needed while a column it can reach may still be filled or run. Such a column lies within eight chunks of the start's chunk (L5), and the stage fills or runs a column only within `LOADING_RINGS` plus two rings per rung of a view (`GenerationStage::evict_outside_halo`). A tile is therefore kept while one of its chunks lies within that reach plus eight of a view, so the store holds starts for the views, not for the explored area. Search outside the loaded area derives each site from the seed; once the save carries starts (I5), a chunk on disk can answer from its `starts` tag instead. Until the starts move into the store, the index's memo is not evicted and grows with the explored area.

---

## 9. Search: `/locate`, maps and eyes

Everything in this section is a consumer of the first four layers of §1.1 and
adds no state. `/locate` and its kin hold the seed fixed and walk cells.

**R1. Two seeds.** The world seed is 64 bits. Every draw of §2 and §3 goes
through seeding with `s`, which is `(s ^ 0x5DEECE66D) & (2⁴⁸ − 1)`; the salted seeding adds the
seed to the cell hash before it, and
the large-feature seeding reseeds with the seed, draws two longs and XORs, then
reseeds again — in both the top 16 bits of the seed are discarded
before any draw. So the cell function (P2), all four frequency gates (P3), the
selection stream (P6) and the layout random (L2) are
functions of the low 48 bits, the *structure seed*. The climate field and the
density column are not: `RandomState` seeds a Xoroshiro source through
`upgradeSeedTo128bit`, a full 64-bit mix, so the biome test of L2, every height of L3 and the ring snap of P4
depend on the whole seed. Consequences: (a) a positional constraint on
structures is decided at the 48-bit tier and the 2¹⁶ seeds above it are
either all lifted or all not; (b) constraints on different sets intersect at
that tier, because each set is a function of the same 48 bits.

**R4. `/locate` is a walk over cells in the reference's answer order.** The
command asks the generator for the nearest start of a structure set within
100 lattice cells, never creating references. Structures are grouped by placement; every
`concentric_rings` placement is answered from its whole ring list by true
minimum distance (at the chunk centre and `y = 32`); then, for
`radius = 0..100`, every `random_spread` placement scans the *perimeter* of
the `(2r+1)²` cell square, `x` ascending then `z` ascending, computes the
potential chunk of each cell and returns the **first** cell whose start is
present; the hits of all placements at that radius are compared
by squared 3-D distance from the player to the locate position (the chunk's
minimum corner plus `locate_offset`, `y = 0`), and the walk stops at the first
radius with any hit. The answer is thus the nearest *among the
first hit per placement on the first non-empty ring*, not the nearest start:
observable to a player comparing with a vanilla server, and reproduced as the
default. A `nearest` mode continues the walk while a ring can still hold a
closer chunk, i.e. while `(r − 1) · spacing · 16 < best`, and finishes the ring
it is on.

The presence test behind each cell is where the two designs part. The
reference's start memo answers from its memo, else from the region
file's `structures.starts` tag by a field scan, else by the frequency gate and
a fresh site step with the pieces discarded — and if that passes it *loads the
chunk* to `structure_starts` to learn whether the start is valid. Here the index
answers `site(C, S)` from its site level (I2) — the three gates and the site
step, no chunk — and needs the layout only when a site can pass with no
piece, which is a per-type fact fixed at freeze: a jigsaw start always has
its centre piece, and the table of hardcoded types says which of them can
return empty. With that table a locate never runs a layout. The table is one
constant per type, and every row is *true*:

| Type | Why a passing site always has a piece |
| --- | --- |
| buried_treasure, desert_pyramid, jungle_temple, swamp_hut, igloo, nether_fossil, ruined_portal, shipwreck | the layout builds its piece from the site position with no further test |
| ocean_ruin | the first ruin is always built; the cluster draws only add to it |
| ocean_monument | the building is one piece |
| end_city | the base tower is always built; the section generators only add to it |
| woodland_mansion | the entrance and the first room are always built |
| fortress | the start bridge is always added; the walk only adds to it |
| stronghold | the layout retries with a re-seeded stream until a portal room exists |
| mineshaft | the site itself builds the tree (L2) and the room is its first piece |

So a search never runs a hardcoded layout either, and the index's site level
answers `/locate` for every shipped structure.

**R5. The other readers of the walk.** An explorer map calls the same walk
with `search_radius` (default 50), `skip_existing_chunks` (default on) and
*creates a reference*: with the flag the walk demands an unreferenced start
and claims the one it returns, having first claimed the start the player is
standing in so the map does not point home. That claim is
the one write any consumer makes and lands on the `references` counter L6
calls truth. The eye of ender walks `#eye_of_ender_located` at radius 100,
dolphins `#dolphin_located` at 50. The tags are data
and resolve at freeze the way
biome tags do; two ids are traps for a hand-written table — `cats_spawn_in`
is plural, and the eight abandoned-camp map tags end in the biome, not in
`_maps`. Every reader receives the `locate`
position of P1, never a piece position, and a map is drawn from it.

**R6. Cost, and what is memoised.** Per cell: one 48-bit seed, two or three
steps of the generator, one memo probe. Per site: one climate sample and, for
a projected structure, one density strip (L3). A `/locate` at radius 100 is at
most `201²` cells per placement, of which the memo answers every cell already
asked by fill; a stronghold locate is 128 entries. The reference pays a chunk
allocation and a layout run per candidate past its memo. The site
level of the memo is what search writes (I2); it is bounded with the memo
(I6) and a dropped site is recomputed bit-identical. Search never allocates a
column, a window or a template block list, and never blocks a fill: the index
is shared by handle and its slots are once-cells.

---

## 11. Assets and freeze

Fe1 and Fe2 of `scattering.md` apply unchanged. Four registries and one
binary asset join the worldgen macro:
`structure_set` (nested: names structures and, through an exclusion zone,
other sets), `structure` (nested: names pools, a `placed_feature` through a
feature element is reachable only through a pool), `template_pool` (nested:
elements name templates, processor lists, placed features, and pools name
their fallback and each other), `processor_list`, and `structure/<path>.nbt`
through a loader that gunzips and checks the pin of T1.

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
  (`generate/tests/corpus_generators.rs`); every one of the fifteen has one
  and the census is pinned empty, so a type that loses its generator is a
  test failure, not a silent empty start;
- the "site implies a piece" table of R4 for the hardcoded types;
- every structure tag of R5 resolves to structures that exist.

The pools' expanded lists (J1), the per-pool maximum height the expansion
hack reads, and the template manifest of
T6 are frozen tables; the biome holder sets of P5 and the `has_structure/*`
tags resolve to biome masks the way `BiomeMask` already does.

---

## 13. Verification

Bit for bit against the reference, where the reference computes without a
world, each fixture carrying
`WORLD_VERSION` 5124, and a template-version pin of 5011 that fails on
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
  oracle's stub world, the analogue of tree geometry parity, with the
  processor chains of the corpus;
- the hardcoded pieces: every non-jigsaw structure
  over three dimensions, five seeds and the sixteen site chunks, each piece as
  its serialised tag, with the fields
  the reference fills at placement dropped from both sides because layout fixes them;
- whole starts placed chunk by chunk: every
  non-jigsaw type at three chunks into a flat world with a dry, water or cave
  base, comparing the writes, the block entities
  and the spawned entities in arrival order with the UUID and the entity's own
  rolls masked; the packed entity-data bytes the oracle also records are not
  compared;
- `/locate`: the reference's answer for a seed set and a grid of player
  positions, including the cases where the first-ring answer is not the
  nearest start (R4);
- the 48-bit invariant: for every fixture above that R1 places at the 48-bit
  tier, flipping any of the top 16 seed bits changes nothing, and for a site
  fixture it changes the biome verdict and nothing before it;

Structurally: every shipped `structure_set`, `structure`, `template_pool` and
`processor_list` round-trips to an equal JSON value; every one of the 1511
templates round-trips to equal NBT; every id resolves at freeze; the census of
§11.

The ladder: `generate/tests/ladder.rs` runs one structure per region — a
village, an outpost and every hardcoded type in its own biome — and the
oracle of `scattering.md` §3.6 against the parallel scheduler through every
unobservable order (S5).

Decorated parity with a reference world is not attainable (`worldgen.md`
§15), and this document adds nothing that makes it so; what it adds is that
the *starts* of a reference world are, which is the half worth having.

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
