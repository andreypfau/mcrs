# Surface: Material Rules over a Strip

Fill produces a world of one material. The surface stage turns it into a world of
sand, grass, gravel, terracotta and snow, by rewriting a strip from the top down.

`worldgen.md` §8 states what the stage *is*: a rewriting system over a strip, its
context, the memoisation scheme that follows from the context, and the three cost
facts Sf1–Sf3. That section is not restated here. This document specifies the
26.3 data model the stage reads, the compiled representation it runs as, and how
both land on this codebase.

The reference is the authority on the data — the rule and condition kinds, their
parameters, the numeric constants — and on nothing else. The compiled shape below
is ours and differs from it deliberately; the divergences are listed in the
appendix.

---

## 1. Position in the pipeline

The stage runs over one column, after the density fill has written stone and
fluid, and before carving. Both halves of that sentence are part of the
definition: rules read `depthAbove`/`depthBelow` off real blocks, so fill must be
complete; and carving must come after, because carved rock is not re-surfaced —
a cave wall is stone, not grass.

The reference does re-apply the rules at one position when a carver opens a
cavity directly under grass or mycelium, over a 1 × 1 × 1 volume with the depths
pinned to one and biome folding disabled. That path is **not implemented here**;
until it is, dirt under a carved overhang stays dirt where the reference would
put grass.

Its footprints are zero in both directions (`worldgen.md` §13). Horizontal
gradients are read from the column's own 16 × 16 height map with the edges
clamped inward, so no neighbour is touched. The stage is therefore trivially
parallel and its order across columns is unobservable.

---

## 2. The context

Rules never test blocks directly. They test a context, recomputed as the descent
moves:

| Quantity | Rate | Definition |
| --- | --- | --- |
| `blockY` | per block | the position under rewrite |
| `depthAbove` | per block | solid blocks running upward from here; air resets it to zero |
| `depthBelow` | per block | distance down to the nearest void, from a look-ahead scan |
| `waterLevel` | per block | height of the last fluid seen from above; sentinel when none was |
| `biome` | per block | biome at `(x, y, z)`, through the zoom of §10 |
| `surfaceDepth` | per strip | `surface` noise `· 2.75 + 3.0 + rand · 0.25`, truncated to an integer |
| `surfaceSecondary` | per strip | `surface_secondary` noise, raw |
| `minSurfaceLevel` | per strip | `floor(chunk_surface_level(x, 0, z)) + surfaceDepth − 8` |
| `gradientX`, `gradientZ` | per strip | height difference between the neighbouring strips |

**S1. `surfaceDepth` may be zero or negative.** That is not a degenerate case to
guard against, it is the definition of a hole: the `hole` condition is exactly
`surfaceDepth <= 0`, and it is what puts gravel and air at the bottom of a frozen
ocean pit.

**S2. `minSurfaceLevel` is rank XZ.** It comes from `chunk_surface_level`, which
is the preliminary surface level of `worldgen.md` §6 wrapped in an interpolation
over a 16 × 1 lattice — a 2D field over the column. One buffer of 16 × 1 × 16 per
column, evaluated once. Sampling it per block is the expensive mistake here (P1).

**S3. The eight is not a rounding.** `minSurfaceLevel` sits a fixed eight blocks
below the preliminary level, and every rule guarded by `above_preliminary_surface`
starts there. The constant is part of the data.

---

## 3. The strip descent

For each of the 256 strips of a column:

1. Read the top: one block above the strip's highest non-air block.
2. If the biome there is eroded badlands, build the pillars (§8) — they write
   blocks.
3. Read the top **again**, and the two gradients, clamped to the chunk.
4. Bump `genXZ`, which invalidates every per-strip cache; compute `surfaceDepth`.
5. Walk down, tracking three quantities:
   - air resets `depthAbove` and `waterLevel`;
   - a fluid block records `waterLevel` if this is the first of its stack;
   - a solid block increments `depthAbove`, bumps `genY`, and runs the rules.
6. `depthBelow` comes from a look-ahead: on entering a solid run, scan down once
   to the first non-solid block and remember where it was; every step inside the
   run then subtracts.

**S3a. The top is read twice, and the second read is not redundant.** The pillar
pass of step 2 raises the strip's own height and its neighbours' gradients, so a
pillar is surfaced by the rules that follow and perturbs the steepness its
neighbours see. The first read still feeds the biome lookup and the iceberg pass.

**S3b. The descent ends at the bottom of the dimension, not of the dispatch.** A
position the column does not carry is skipped, not treated as the end. Deriving
the range from the carried sections would give each dispatch its own idea of
where the bottom is.

The stage is owed a whole column, and the scheduler now guarantees one: a
dispatch generates every section of the dimension and hands back only the ones it
owes. That is not a detail of scheduling. Bedrock is a material rule, so a column
the stage skips is a column open at the bottom of the world — the failure mode is
a hole, not a missing decoration.

**S3c. The look-ahead reads one block below the bottom, and that read is
non-solid by definition.** The reference relies on the out-of-range read
answering air so the run terminates at the floor; treating out-of-range as "stop"
instead leaves `depthBelow` at a sentinel and fires every ceiling rule in an
all-stone column.

**S4. The descent reads and writes the same buffer.** A rule sees blocks written
by rules that ran above it in the same strip. This is load-bearing — the badlands
band rules depend on it — and it is why the stage cannot be reordered against
itself.

**S5. There is no early exit.** Ore vein rules apply at any depth and carry no
bounding condition, so the descent runs to the bottom of the column (Sf3).

---

## 4. The data model

In 26.3 rules and conditions are **loadable registries**, not a tree embedded in
the noise settings: `worldgen/material_rule/**` and `worldgen/material_condition/**`.
The noise settings name a rule by id (`material_rule`), and any node inside a
tree may itself be a bare id string instead of an inline object.

**S6. A holder is either an id or an object, and round-trips as it was written.**
The same shape `DensityFunctionHolder` already has. The point of the split is
sharing: `on_floor`, `not_underwater` and their six siblings are one object each,
referenced dozens of times across the overworld tree, and interning depends on
that identity being visible to the compiler.

### Rules

| Type | Result |
| --- | --- |
| `block` | a constant block state |
| `sequence` | the first member that produces a state |
| `condition` | the inner rule if the condition holds, nothing otherwise |
| `bandlands` | the clay band at this Y, offset by the `clay_bands_offset` noise |
| `ore_vein` | §7 |

### Conditions

| Type | Holds when |
| --- | --- |
| `stone_depth` | `depth <= 1 + offset + (add_surface_depth ? surfaceDepth : 0) + secondary`, where `secondary` maps `surfaceSecondary` from [−1, 1] onto [0, `secondary_depth_range`], and `surface_type` picks `depthAbove` or `depthBelow` |
| `water` | `blockY + (add_stone_depth ? depthAbove : 0) >= waterLevel + offset + surfaceDepth · multiplier`; always true where no fluid was seen |
| `y_above` | the same comparison against a resolved vertical anchor instead of the water level |
| `biome` | the biome at this position is in the set |
| `noise_threshold` | the named noise is within `[min, max]`; `is_3d` selects the sampler |
| `vertical_gradient` | true at and below one anchor, false at and above another, and a linearly falling coin flip between them |
| `temperature` | the biome is cold enough to snow at this position |
| `steep` | `gradientX <= −4 or gradientZ >= 4` |
| `hole` | `surfaceDepth <= 0` |
| `above_preliminary_surface` | `blockY >= minSurfaceLevel` |
| `not` | inversion |

**S7. `steep` is asymmetric, and that is the data.** One axis tests a fall, the
other a rise. It has been that way since the rule system was introduced; making
it symmetric changes where gravel appears on every slope in the world.

**S8. Validation happens at load.** An unknown rule or condition type, a missing
referenced id, a block state that does not resolve — all are load failures, not
values to skip at evaluation time. A condition kind that parses but that this
build cannot yet evaluate fails the same way, loudly, at compile time: the
`temperature` condition is in that position, and no shipped asset uses it.

The asymmetry with the terrain graph is deliberate and worth stating, because it
looks like an inconsistency. A density root that will not lower degrades to a
constant and is reported; a material rule that will not compile fails the whole
build. The reason is what each failure leaves behind: a degraded density root
loses one term of a sum, while a skipped rule set loses bedrock, and a world
generated without its material rules is not a poorer world but a broken one.

---

## 5. Compilation

The rule tree is compiled **once per world** into a flat tape. Not once per
column: the reference recompiles per chunk because that is the only moment it can
fold biome conditions, and §6 below gets the same folding without paying for it.

```
Op ::= Guard { cond: CondId, skip_to: u32 }   // !test() -> pc = skip_to
     | Block { state: VoxelId }               // return
     | Bandlands                              // return
     | OreVein { vein: VeinId }               // may return, else pc += 1
```

**S9. `sequence` compiles to nothing.** Its members are laid out consecutively;
the first instruction that yields a state returns. There is no per-sequence
bookkeeping and no result slot.

**S10. `condition` compiles to one forward jump.** The guard's `skip_to` is the
end of its own subtree. A rule tree of several hundred nodes becomes an array
walked by `while pc < len`, with no pointer chasing and no indirect calls.

**S11. Conditions are interned by structural equality.** Equal condition
subtrees share one `CondId`, hence one cache slot, hence one evaluation per
scope, regardless of how many rules referred to them (M3). Interning is what
makes the shared-id files of §4 pay off.

**S12. Every condition carries a scope**: `Static`, `Xz`, or `Y`.

| Kind | Scope |
| --- | --- |
| `steep`, `hole` | `Xz` |
| `noise_threshold` | `Xz` when `is_3d` is false, `Y` when true |
| `stone_depth`, `water`, `y_above`, `biome`, `temperature` | `Y` |
| `above_preliminary_surface` | `Y` — it reads `blockY` |
| `not` | inherits from its operand |

A wrong scope is the one error in this subsystem that produces plausible output:
an `Xz`-cached `above_preliminary_surface` gives every block in a strip the
topmost block's answer, and nothing else in the pipeline notices. §12 says how it
is caught.

A biome set that folds to `never` or `always` carries no scope at all: the fold
is a separate per-column array, consulted ahead of the cache, so a settled
condition never reaches the stamp comparison. A third scope for it would be a
second mechanism for the same fact.

---

## 6. Memoisation

`worldgen.md` §8 derives the scheme; this is its representation.

```
struct CondCache { stamp: Vec<u32>, value: BitVec }   // indexed by CondId
struct NoiseCache { stamp: Vec<u32>, value: Vec<f32> } // indexed by NoiseId
```

Two counters live in the scratch: `genXZ` bumps on entering a strip, `genY` on
every step down (and on entering a strip, since a new strip is also a new Y). A
hit is `stamp[id] == gen_of(scope)`: one integer comparison, no keys, no hashing.

**S13. Noise samplers are interned like conditions**, by name *and* by
dimensionality. A 2D sampler caches against `genXZ`, a 3D one against `genY`, so
one noise read both ways is two samplers with two slots — merging them would give
one of the two readings the other's cache lifetime. Within a dimensionality, a
noise named from four different rules is evaluated once.

**S14. Biome sets fold to a tri-state per column.** Each distinct `biome_is` set
is interned as a bitset over registry ids. On entering a column, the set of
biomes the column can *select* settles each interned set to `never`, `always` or
`maybe`, in a per-column array the guard consults before it reaches the condition
cache. A rule tree for a biome that does not occur in this column therefore costs
one array read and one jump.

This is the reference's per-chunk specialisation without the per-chunk
compilation.

**S14a. The fold runs over the widened grid of §10, never over the stored
palette.** The zoom can select a cell in the border ring, so a biome present only
in the ring can still win for a block inside the column. Folding against the
inner 4 × 4 × 4 cells turns a condition that should have matched into `never` and
writes the wrong block, with no error and no failing test. The reference is
looser still — it folds against a three-by-three neighbourhood of chunks — and
looseness is the safe direction: folding is an optimisation, and only the number
of guard evaluations may differ between a tight sound set and a loose one.

---

## 7. Ore veins

Ore veins are a material rule in 26.3, not a separate pass. The rule holds three
density functions — density, richness and filler gap — plus the ore, raw ore and
filler states and the raw-ore chance.

Evaluation: a non-positive density yields nothing; otherwise a positional draw
against the density decides whether the position is in the vein at all; inside,
a second draw against richness together with a negative filler gap yields ore
(raw ore at the configured chance), and everything else yields the filler stone.

**S15. Vein density functions compile into the same program as the router.** They
are ordinary density functions, they share subexpressions with the terrain graph,
and a separate program would mean a second `Workspace` and no common
subexpression elimination between them. Material compilation therefore happens
alongside router compilation rather than after it.

**S15a. The tape stores root indices, not raw node ids.** A program registers a
fill plan for its roots and for a few interior nodes it knows about; sampling any
other node asserts at run time, and the prune that follows compilation deletes
every node no root reaches — so a node compiled and not registered as a root does
not fail loudly, it silently becomes some other node's id. Every density function
a rule names is appended to the root vector, and the tape refers to it by its
index there.

**S16. Density and richness are prefilled over the column, the filler gap is
not.** The first two are read at nearly every position of the descent; the third
only inside veins.

---

## 8. What the rules cannot express

Three pieces of the surface are not rules, and their order around the rule pass
is part of the definition.

**Clay bands** are a table of 192 block states, generated once per world from the
seed: runs of orange terracotta at random intervals, then bands of yellow, brown
and red, then nine to fifteen white ones with occasional light-grey neighbours.
The `bandlands` rule indexes it by `Y` plus a rounded offset from the
`clay_bands_offset` noise, wrapping.

**Eroded badlands pillars** are built **before** the rule pass, so the rules then
paint them. A pillar's height comes from the minimum of two noises; the fill runs
top-down over air and aborts if it would grow out of water.

**Frozen ocean icebergs** are built **after**, so the rules do not touch them.
Same two-noise shape, plus a snow cap of random depth above a random height and
packed ice below, with probabilistic holes.

**S17. Neither extension may be moved across the rule pass.** Pillars before,
icebergs after; swapping either changes what the world looks like, not just how
it is computed.

**S17a. The arithmetic is not uniformly double.** The reference's noise returns a
float, and its expressions mix widths deliberately: one multiplies the float by a
float constant and *then* widens, another widens first, and the band offset
rounds a float product. Porting all of it in one width moves pillar heights,
iceberg heights and clay band offsets. The same care separates truncation toward
zero — the surface depth and the stone-depth comparison — from a real floor, used
for the preliminary level and the pillar top.

**S17b. The iceberg melt check is deferred.** The reference lowers an iceberg by
two blocks where the biome is warm enough, which needs a per-biome temperature
that nothing in this codebase can reach at generation time yet. Until it exists,
icebergs in deep frozen ocean are two blocks taller than the reference's.

---

## 9. State classification and the ECS boundary

| Value | Category | Where it lives | Owner |
| --- | --- | --- | --- |
| Seed, rule and condition assets, block registry | authoritative truth | assets and config | the loader |
| Compiled tape, interned conditions, noises, biome sets, vein nodes | derived, materialized once per world | `Resource(Arc<MaterialProgram>)` | the compile system |
| `surfaceDepth`, `minSurfaceLevel`, gradients, biome tri-states | derived per column | task-local scratch | the descent |
| `depthAbove`, `depthBelow`, `waterLevel`, condition and noise caches | derived per step | the same scratch | the descent |
| Blocks after the pass | truth once edited | `ColumnBlocks`, then section palettes | the generation task |

**S18. This subsystem contributes no temporal facts.** Nothing here is a message
or an observer event. "Surface built" is reconstructable from the column's
generation status, which already exists; a marker component on a section for it
would be a second representation of a fact that already has one.

**S19. It contributes no components either.** Generation lives outside the ECS
(`worldgen.md` X1). The only ECS-visible objects are the rule and condition
assets and the compiled program. The program is part of the router's own build,
so it is *built* where the router is built rather than cloned in, and it never
touches the per-tick extract. Today that is one router, from the one noise
settings the world preset names, shared by every sub-app; when a preset carries
settings per dimension, each dimension's rules come with its own router and
nothing about this stage changes.

**S20. The scratch is reused per worker, not allocated per column.** The
condition and noise caches, the preliminary-surface buffer and the prefilled vein
densities are all column-scoped; a fresh set per column costs more than the
descent.

---

## 10. What the stage rests on

Five things the stage needs did not exist when it was specified. All five are in
place; they are recorded here because each is a place where a plausible-looking
shortcut is wrong, and a later reader deciding to simplify one of them should
know what it costs.

**The non-air height map, produced by the fill.** The descent starts one block
above the strip's highest non-air block, and the gradients read that map for the
neighbouring strips. It is the same `¬air` map `heightmap.md` §3–4 permits to be
maintained during fill, so no second pass over the blocks is owed — but it is not
free by inspection either, because the sweep descends per *cell*, and a cell
spans several strips horizontally. Per class: a solid or fluid cell settles every
strip in its footprint at the cell's top; a sea cell settles at one below sea
level, because that is where its fill stops; an air cell settles nothing; only a
mixed cell answers per strip, from its inner loop. Cells outside the carried
sections are skipped before they are classified and must not be recorded. The
block-by-block fallback path needs the same treatment.

**The biome zoom.** Rules read the biome through the fiddled Voronoi zoom over
eight quart corners, seeded from a hash of the world seed — not from the 4 × 4 × 4
palette cell. Without it every biome-conditioned rule shifts its boundary by up
to a few blocks. The corners reach outside the column on **all three axes**, so
the answer must come from re-evaluating climate there, never from a neighbour's
stored palette; that keeps the stage's read footprint at zero. Concretely the
existing column climate fill widens by one quart cell in every direction — its
origin moves back four blocks on each axis and it grows by two cells on each
axis — the inner cells still feed the palette, and the whole grid feeds both the
zoom and the fold of S14a. Getting the resulting index offset wrong shifts every
biome by one quart, which looks like plausible terrain.

**Material compilation inside router compilation.** Required by S15.

**A block-state resolver reaching the compiler.** A rule's `result_state` must
become a stored block id at compile time, and the crate that compiles has no
block registry — it is handed the two default states already resolved. The rule
tape needs some forty more, so the same door widens: compilation takes a resolver
from its caller, and an unresolvable state is a load error (S8). The alternative,
compiling material rules on the far side of the crate boundary where the registry
lives, costs S15.

**The assets have to reach the loader at all.** The runtime dependency walk
starts from the terrain roots and nothing else, so today none of the nine noises
the stage samples directly and none of the vein density functions is loaded, even
though every one of them is on disk. Two sources close it: a walk of the material
rule tree, which finds the noises named by `noise_threshold` and the density
functions named by `ore_vein`; and a fixed list for the rest, because
`clay_bands_offset`, `surface_secondary` and the four pillar noises are named by
no datapack file at all — the reference hardcodes them too. A test that loads the
whole asset directory from disk will not catch this; only the server's own
dependency walk will.

---

## 11. Module layout

The rule system belongs with the other data-driven worldgen machinery, beside the
carver configuration: proto types with their holders, the compiler, and the
evaluator with its scratch. The strip descent and the two hardcoded extensions
belong with the generation passes, because they operate on the dense column
buffer.

The dense column buffer is already the flat representation Sf2 asks for, and its
interior mutability already allows a reader and a writer at once, which the
descent needs by S4.

---

## 12. Verification

**S21. The tape has an oracle.** A naive tree walker behind the same interface,
kept for the life of the project, differentially tested against the tape over a
fixed set of columns. Same arrangement as the density interpreter of
`worldgen.md` §19.

**S22. Memoisation is tested by disabling it.** A run with every cache forced to
miss must produce bit-identical blocks. A condition given the wrong scope (S12)
produces plausible terrain and passes every other test; this is the one that
catches it.

**S23. Parity is measurable here.** The stage is a field stage with zero
footprints, so unlike decoration it can be compared block for block against a
reference world at the same seed.

---

## 13. What not to do

**Compile a closure per rule node.** A chain of indirect calls per block costs
more here than in the JIT-compiled reference the shape was written for; the tape
of §5 exists for that reason. **Recompile the tape per column.** §6 obtains the
same folding statically. **Key the noise cache by name or hash.** An interned
index into a flat array is the whole mechanism. **Treat `minSurfaceLevel` as a
three-dimensional field** (S2). **Plan an early exit from the descent** (S5).
**Read a neighbouring column's biome palette for the zoom** (§10). **Give the
surface stage a component, a marker or a message** (S18, S19). **Move either
hardcoded extension across the rule pass** (S17). **Skip an unknown rule type at
load** (S8).

---

## Appendix: correspondences in Minecraft

For checking behaviour against, not for copying. Paths are relative to
`src/main/java/net/minecraft/`, version 26.3.

| What | Where |
| --- | --- |
| The strip descent, depths, water level | `world/level/levelgen/material/MaterialSystem.java` |
| The context and its two generation counters | `world/level/levelgen/material/MaterialRuleContext.java` |
| Rule kinds and their codecs | `world/level/levelgen/material/rule/` |
| Condition kinds and their codecs | `world/level/levelgen/material/condition/` |
| Registry ids and holder codecs | `world/level/levelgen/material/MaterialRules.java` |
| The rule referenced by a dimension | `world/level/levelgen/NoiseGeneratorSettings.java`, field `material_rule` |
| Where the stage runs in the pipeline | `world/level/levelgen/NoiseBasedChunkGenerator.java` |
| The one-position re-application after carving | `MaterialSystem.topMaterial` |
| Clay bands, badlands pillars, icebergs | `MaterialSystem.generateBands`, `erodedBadlandsExtension`, `frozenOceanExtension` |
| The biome zoom | `world/level/biome/BiomeManager.java` |
| The preliminary surface level and its interpolation | `world/level/levelgen/NoiseRouterData.java`, `chunk_surface_level` |

### Deliberate divergences from the reference

| Topic | Reference | Here | Reason |
| --- | --- | --- | --- |
| Rule representation | a graph of objects compiled to closures | a flat tape with forward jumps | S9, S10: no indirect call per block |
| Compilation frequency | once per chunk | once per world | S14 removes the reason for per-chunk work |
| Biome folding | recompiles the tree against the chunk's biome set | a per-column tri-state over interned biome sets | Same folding, no recompilation |
| Condition caches | a generation stamp per condition object | two flat arrays indexed by interned id | S11: sharing follows from interning, not from object identity |
| Vein density functions | sampled through the chunk's sampler set | nodes in the same program as the terrain graph | S15: common subexpression elimination across both |
| Where the stage lives | a method on a world-scoped object holding the noises | a compiled resource plus a pure descent over the column buffer | S18, S19: no state to own, nothing to notify |
