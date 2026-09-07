# Heightmaps: Invariants, Deriving One Map From Another, the Seam With Light

A specification for a from-scratch implementation. It describes what a heightmap
is as a mathematical object, which invariants the block implementation is obliged
to hold, how the order of computation follows from those invariants, and how a
heightmap relates to sky light.

The document deliberately does not describe the structure of Minecraft's classes.
References to the reference implementation are collected in the appendix — for
checking against, not for copying.

Terminology follows `worldgen.md` §2: a **strip** is the 1 × 1 × H run of blocks,
a **column** is 16 × 16 × H, a **section** is 16 × 16 × 16. Vanilla's word *chunk*
denotes the column.

---

## 1. The data model

A heightmap is defined for a predicate `P` over block state and stores one number
for each of the column's 256 strips:

```
H_P(x,z) = 1 + max { y : P(block(x,y,z)) },   if such a y exists
H_P(x,z) = minY,                              otherwise
```

That is, it stores **the first free Y**, not the Y of the topmost block. Three
reasons to prefer this form:

- an empty strip is representable without a separate sentinel value: `H = minY`;
- the most frequent query — "at what height do I stand, place, spawn" — is exactly
  `H`, with no arithmetic;
- monotonicity: `H` grows together with the strip rather than trailing it by one.

The value range is `[minY, minY + height]`, i.e. `height + 1` distinct values.
Store it as a dense bit array: `ceil(log2(height + 1))` bits per cell, 256 cells.
For a height of 384 that is 9 bits, 288 bytes per map.

**Representation invariant.** Values are stored biased by `minY` so that the bit
field is unsigned. Every read and write goes through one pair of functions
applying the bias; nowhere else needs to know about it.

---

## 2. The predicate family and its ordering

The maps are useful not individually but as an ordered family. Everything that
yields a saving follows from one fact:

> **Main lemma.** If `P ⟹ Q` (every block satisfying `P` also satisfies `Q`), then
> `H_P ≤ H_Q` per strip.

Proof: the topmost `P`-block of the strip is also a `Q`-block, so the maximum for
`Q` is no lower.

The minimal useful set of predicates:

```
SURFACE   = ¬air
SOLID     = blocksMotion(b)
MOTION    = blocksMotion(b) ∨ hasFluid(b)
NO_LEAVES = blocksMotionExceptLeaves(b) ∨ hasFluid(b)
```

For the ordering to work, the block registry must hold four invariants. These are
requirements on the data, not observations about it:

- **I1.** `air ∉ blocksMotion`.
- **I2.** `hasFluid(b) ⟹ ¬air(b)`. Air cannot be filled with fluid, and
  waterlogged air does not exist.
- **I3.** `blocksMotion = blocksMotionExceptLeaves ∪ leaves`. Set equality, not
  one-sided inclusion: from it follow both `NO_LEAVES ⟹ MOTION` and the converse
  fact that a motion-blocking non-leaf block is necessarily in no-leaves, which
  the fast path in §4 needs.
- **I4.** Predicates depend only on block state — not on neighbours, not on
  position, not on time. Otherwise incremental maintenance (§5) is incorrect.

From I1–I3 follows the height ordering:

```
H_SOLID ≤ H_MOTION ≤ H_SURFACE
H_NO_LEAVES       ≤ H_MOTION
H_SOLID  and  H_NO_LEAVES  —  incomparable
```

The incomparability is essential and is not a defect of the set: leaves belong to
`SOLID` but not to `NO_LEAVES`, and fluid the other way round. A water column
above sand puts `H_NO_LEAVES` at the water surface and `H_SOLID` at the bottom; a
canopy above a trunk does the reverse.

**The top of the ordering.** `SURFACE` is the upper bound of the whole family:
every predicate implies `¬air`. Hence the universal corollary everything else
relies on:

> **Invariant U.** `H_P ≤ H_SURFACE` for any `P` in the family. Scanning above
> `H_SURFACE` cannot produce a result for any map.

---

## 3. The rule for deriving one map from another

The general form, from which every special case follows:

> Suppose `H_P` is wanted for a strip, and the following are already known: `H_Q`
> with `P ⟹ Q` (an upper bound) and `H_R` with `R ⟹ P` (a lower bound). Then it
> suffices to scan `y` from `H_Q − 1` down to `H_R`: the first `y` with `P(block)`
> gives `H_P = y + 1`; if the scan is exhausted, `H_P = H_R`.

Both halves need justifying separately.

The upper bound is correct by the main lemma: above `H_Q` there are no `Q`-blocks,
hence no `P`-blocks.

The lower bound is correct because the block at `H_R − 1` satisfies `R` and
therefore `P`. So exhausting the scan is not "we did not find one" but "we found
one exactly at the boundary".

Applied to the set from §2:

| Map | Upper bound | Lower bound |
| --- | --- | --- |
| `H_SURFACE` | — (the top) | — |
| `H_MOTION` | `H_SURFACE` | `H_SOLID` |
| `H_SOLID` | `H_MOTION` | none |
| `H_NO_LEAVES` | `H_MOTION` | none |

The absence of a lower bound for `H_SOLID` and `H_NO_LEAVES` means that in the
worst case they require a descent to the bottom of the world. This is irreducible:
a transparent or leafy strip all the way down is a legal configuration.

---

## 4. Optimal evaluation: one fused descent

The naive implementation computes each map with its own scan. That is the wrong
choice: the block has to be fetched from the palette once per map.

**The right form is a single downward pass per strip, evaluating every
still-unknown predicate on each state it fetches.** A map leaves the unknown set
as soon as its predicate first fires; the pass ends when the set is empty.

```
unresolved := {SOLID, MOTION, NO_LEAVES}
for y from H_SURFACE − 1 down to minY:
    b := block(x, y, z)
    for P in unresolved:
        if P(b):
            H_P := y + 1
            remove P from unresolved
            if P = SOLID and MOTION ∈ unresolved:
                H_MOTION := y + 1          # by the lemma: SOLID ⟹ MOTION
                remove MOTION from unresolved
    if unresolved is empty: break
for P in unresolved: H_P := minY
```

Three properties of this form:

- **One fetch per level**, rather than one per map.
- **Start from `H_SURFACE`, not from the column ceiling.** By invariant U
  everything above is useless. The start must come from *this strip's* value, not
  from a column-wide quantity such as "the top of the highest non-empty section":
  otherwise one tall strip forces all 256 to scan from the top. This is the same
  mistake as the fluid skip threshold in `worldgen.md` A4, and it recurs whenever
  a per-strip bound is derived from a column-wide aggregate.
- **The depth of the pass** equals `max(depth of H_SOLID, depth of H_NO_LEAVES)`,
  because `H_MOTION` and `H_SURFACE` resolve no lower than those.

### The fast path

> **Claim.** If the topmost non-air block of a strip satisfies `SOLID` and is not
> a leaf block, then all four maps equal `H_SURFACE`.

Proof: the block satisfies `SOLID` and hence `MOTION`. By I3, from
`blocksMotion ∧ ¬leaves` follows `blocksMotionExceptLeaves`, hence `NO_LEAVES`.
All four maps are bounded above by `H_SURFACE` and attain it at this block.

In practice this means **one block read and one leaf test settle the whole strip**
for stone, dirt, sand and wood-as-material — that is, for the overwhelming
majority of strips in an ordinary world. A full pass is needed only where water,
grass, flowers, a snow layer or a canopy sits on top.

This fast path is worth making an explicit first branch rather than hoping the
general loop exits on its first iteration: the general loop still pays for
maintaining the unresolved set.

### Who maintains `H_SURFACE`

Since everything starts from `H_SURFACE`, it must be obtained cheaply. The right
place is the terrain generator: it already writes the blocks of a strip from the
top down, and updating the map on each write costs one comparison (§5, the "rise"
branch). By the end of terrain generation the map is ready without a single extra
read.

The other three maps are computed once, after terrain, surface rules and carving
have all finished, by the fused descent above.

**The ordering is forced, not conventional.** Carving removes blocks after fill,
and surface rules change block types. Both can lower a map. A `SOLID` or
`NO_LEAVES` map built during fill would be stale by the end of the stage, and
repairing it through §5 would hit the expensive branch — the descent after a top
block is removed — on every carved strip. `H_SURFACE` is the exception only
because the generator writes strips top-down, so its updates are all the cheap
"rise" branch.

### On separating maps by lifetime

A map's type is the pair "predicate + lifetime". Two maps with the same predicate
are the same data, and keeping both is a bookkeeping error, not an optimisation.
Which maps to persist to disk and which to send to the client should be decided
independently of which maps are maintained in memory and from what moment.

A reasonable choice: hold `H_SURFACE` from the first block write, add the other
three once terrain generation completes, and persist and send according to what
consumers actually need.

---

## 5. Incremental maintenance

Once built, a map is updated on every block write. Let `H` be the strip's current
value, `y` the height of the changed block, `s` the new state.

| Condition | Action | Cost |
| --- | --- | --- |
| `y < H − 1` | nothing | O(1) |
| `P(s) ∧ y ≥ H` | `H := y + 1` | O(1) |
| `P(s) ∧ y = H − 1` | nothing | O(1) |
| `¬P(s) ∧ y ≥ H` | nothing | O(1) |
| `¬P(s) ∧ y = H − 1` | descend from `y − 1` to the first `P`-block | O(depth) |

**The key invariant:** the first row rejects the overwhelming majority of changes.
Any edit strictly below the topmost `P`-block can neither raise the map (it is
already higher) nor lower it (the top block was not touched).

The update is **exact**, not approximate: an incrementally maintained map is
bit-identical to one rebuilt from scratch. That is what makes it safe to trust the
map after an arbitrarily long series of edits and not rebuild it "just in case".

### A lower bound for the descent

The expensive branch — removing the top block — can stop at `H_SOLID` for
`H_MOTION`: the block at `H_SOLID − 1` satisfies `SOLID` and therefore `MOTION`.

The price is a **mandatory update order**: `H_SOLID` must be updated before
`H_MOTION` for the same block change. If the top block was the shared top of both
maps, `H_SOLID` descends first and `H_MOTION` then uses the new value.

For `H_SOLID` and `H_NO_LEAVES` there is no lower bound and the descent is
unbounded.

---

## 6. The sky light source map is a different object

The temptation to reuse `H_SURFACE` for sky light is strong and wrong. It is a map
of a different family, and it cannot be derived from the block maps by an
equality.

**Definition.** `LowestSourceY(x,z)` is the smallest `Y` such that every block of
the strip at that height and above receives full sky light **from directly
above**. Light that arrives sideways from a neighbouring strip is the business of
propagation, and counting it here would defeat the map's whole purpose: it is a
seed for propagation, not a record of how deep the light finally reaches.

**The predicate is defined on an edge, not on a block.** An edge is a pair of
vertically adjacent positions:

```
occludes(above, below) = dampening(below) ≠ 0
                       ∨ faceOccludes(bottomFace(above), topFace(below))
```

The reason for this form: two blocks that each individually transmit light can
seal the boundary between them — a top slab under a bottom slab produces a
continuous horizontal surface. The first term covers ordinary opaque blocks, the
second covers partial shapes.

`LowestSourceY` is the `Y` of the upper block of the topmost occluding edge.

### Invariants

- **S1.** An "air over air" edge never occludes: air's dampening is zero, both
  occlusion shapes are empty, and two empty shapes meeting is false.
- **S2.** Hence `LowestSourceY ≤ H_SURFACE`. **This is the only bridge between the
  two families**, and it is one-way: `H_SURFACE` is a correct upper bound for the
  light source scan and, as in §4, the start must come from this strip's value
  rather than from a column-wide quantity.
- **S3.** No block map is a lower bound. Glass belongs to `SOLID` but has zero
  dampening and an empty occlusion shape, so light passes below `H_SOLID`. The
  scan must be able to reach the bottom of the world.
- **S4.** The equality `LowestSourceY = H_SURFACE` does not hold in general. It
  holds if and only if the topmost non-air block of the strip occludes the edge
  above itself. That is a frequent case, not an invariant.
- **S5.** The map is a pure function of the blocks and is **not persisted**. It is
  always reconstructed on load: one bounded pass over the strip is cheaper than
  storing it and rules out desynchronisation from the block data.

### The sentinel

A strip with no occluding edge at all must be distinguishable from a strip whose
occluder lies at the very bottom of the world. The technique: store the value
biased by `minY − 1` and map that value to "minus infinity" on read, so that the
test `y ≥ LowestSourceY` is true for the entire strip.

---

## 7. How light uses this map

The source map exists not to answer "how far down is it lit" but to **avoid
enqueueing anything that has nothing to propagate**. That is its main purpose.

For how this stage is parallelised, and why the frontier rather than the section
is the unit of parallel work there, see `worldgen.md` §14.

### Seeding a column

For each strip, fill 15 from the top down to `LowestSourceY`. What goes into the
queue is not everything, but only the cells on the boundary of the lit region:

```
for each strip c, for y from the top down to max(LowestSourceY(c), minY):
    light(c, y) := 15
    enqueue(c, y)  ⟺  (y = LowestSourceY(c) ∧ y ≥ minY)
                   ∨  y < max over the 4 neighbours n of LowestSourceY(n)
```

Both bounds are written against the sentinel of §6, not merely against the value.
`LowestSourceY(c) = minY − 1` says the strip has no occluding edge anywhere, and
the loop must stop at `minY` rather than step onto a row that is not in the world.
There is no downward boundary cell in that case either: the strip is lit to the
floor, so there is nothing underneath for light to descend into.

The first condition is the bottom of the lit column: light needs to go down from
there. The second covers cells whose neighbouring strip is darker to the side:
light needs to go sideways. Everything else is the interior of a monolithic block
of fifteens and has nothing to give its neighbours.

It is useful to pass a **set of directions** into the queue as well, not just a
level: down if `y = LowestSourceY(c)`; toward neighbour `n` if
`y < LowestSourceY(n)`. That relieves the queue handler of re-testing the four
neighbours.

It is precisely for these tests that per-strip granularity is needed: without the
neighbours' `LowestSourceY` the whole lit volume would have to be enqueued.

### Section-level shortcuts

- Sections entirely above the column-wide `max LowestSourceY` are uniformly lit
  and can be stored as a constant instead of a per-cell array. Here the
  column-wide aggregate is the correct conservative choice, unlike the scan bounds
  in §4 and S2.
- The downward seeding loop over sections can terminate early as soon as no strip
  in the current section has a source below its floor.
- An optional refinement: when light crosses a section boundary through a run of
  entirely empty sections, they can be filled directly without unrolling a
  per-cell traversal.

### Reacting to a block change

The order of tests, cheapest first:

1. Did the light properties change at all — dampening, emission, use of the shape
   for occlusion. If not, do nothing: neither the map nor the queue.
2. If they did, update the strip's `LowestSourceY`. Testing the two edges around
   the changed height is enough; a full descent is needed only if the occluding
   edge was exactly that one and it stopped occluding.
3. Enqueue the block for light recomputation.

The cut-off at step 1 matters more than the rest: replacing one opaque block with
another opaque block must touch neither the map nor the light engine.

---

## 8. What to store, what to recompute

The only source of truth is the block data. Everything else must be recoverable
from it. Beyond that it is only a question of the cost of recovery. This table is
the heightmap-specific case of the classification in `worldgen.md` §16.

| Structure | Store | Why |
| --- | --- | --- |
| Blocks, biomes | yes | the source of truth |
| Block heightmaps | yes | ~288 bytes per map against a pass over the strip |
| Sky and block light data | yes | recovery requires traversal across column borders |
| The "light is correct" flag | yes | decides whether to trust stored light or recompute |
| The sky light source map | **no** | one bounded pass, see S5 |

When loading heightmaps:

- check the stored coordinate system — the lowest row and the height, not merely
  the array length it implies, since two ranges of equal height give arrays of
  equal size while numbering their rows differently; on any mismatch **rebuild
  rather than trust**, because the world's vertical range may have changed
  between sessions;
- treat the values as untrusted input. The map arrives from disk and over the
  network, and the bit field is wider than the real height range (for a height of
  384, nine bits hold up to 511). If a map value is used as a section index — and
  every optimisation above does exactly that — it must be clamped to the world's
  upper bound. Without clamping, corrupt or hostile data indexes past the end of
  the section array.

Maps missing from the save are rebuilt by the same fused descent from §4, with the
same bounds from the ordering: the maps that are present narrow the scan for the
ones that are not.

---

## 9. What not to do

- **Use `H_MOTION` or `H_SOLID` as an upper bound.** Only `¬air` gives an upper
  bound. Blocks that do not block motion may stand above a motion-blocking top.
- **Use any block map as a lower bound for light.** See S3.
- **Treat `H_SOLID` and `H_NO_LEAVES` as comparable.** They are incomparable in
  both directions.
- **Persist the light source map.** It will drift out of sync with the blocks.
- **Keep two maps with the same predicate.** Lifetime is not part of the
  definition.
- **Start a strip scan from a column-wide height.** That turns one tall strip into
  a full scan of all 256.
- **Build the `SOLID`, `MOTION` or `NO_LEAVES` maps during fill.** Carving runs
  afterwards and lowers them; see §4.
- **Update `H_MOTION` before `H_SOLID`** when the lower bound from §5 is in use.

---

## 10. Implementation order

A sequence in which every step is verifiable on its own:

1. The bit storage and the pair of bias functions (§1).
2. The predicates and a check of I1–I4 against the block registry — better as a
   test than as an agreement (§2).
3. Full construction by the fused descent, without bounds, from the column
   ceiling. Slow, but certainly correct — this is the reference for every later
   comparison.
4. Incremental updates (§5). Check for equivalence with the reference after random
   series of edits.
5. Starting from `H_SURFACE` and the fast path (§4). Check against the reference
   again.
6. The light source map as a separate type, with a reference full scan (§6).
7. Seeding the light engine by boundaries (§7). Here an error shows up as holes in
   the lighting rather than as a crash, so checking against the reference is
   mandatory.

Steps 3, 4 and 5 are worth keeping behind one interface with a switch, so that the
reference implementation stays in the tests forever.

---

## Appendix: correspondences in Minecraft

For checking behaviour against. Paths are relative to
`src/main/java/net/minecraft/`.

| What | Where |
| --- | --- |
| Map and predicate definitions | `world/level/levelgen/Heightmap.java:28-31, 152-168` |
| Full construction (the fused descent) | `world/level/levelgen/Heightmap.java:43-82` |
| Incremental update | `world/level/levelgen/Heightmap.java:84-111` |
| Map sets per generation status | `world/level/chunk/status/ChunkStatus.java:16-24` |
| Map maintenance by the terrain generator | `world/level/levelgen/NoiseBasedChunkGenerator.java:479-480, 506-507` |
| Final maps built after carving | `world/level/chunk/status/ChunkStatusTasks.java`, `buildTerrain`'s `thenApply` |
| The sky light source map | `world/level/lighting/ChunkSkyLightSources.java` |
| Seeding sky light by boundaries | `world/level/lighting/SkyLightEngine.java`, `propagateLightSources` |
| The "light properties unchanged" cut-off | `world/level/lighting/LightEngine.java:42` |
| Serialisation of maps and light | `world/level/chunk/storage/SerializableChunkData.java` |
| Motion-blocking block tags | `src/main/resources/data/minecraft/tags/block/blocks_motion*.json` |

### Deliberate divergences from the reference

- The reference keeps `WORLD_SURFACE_WG` / `OCEAN_FLOOR_WG` separate from
  `WORLD_SURFACE` / `OCEAN_FLOOR` even though the predicates in each pair are
  literally identical — a split by lifetime rather than by meaning. So literally
  that `ImposterProtoChunk` substitutes one for the other on read. Here that split
  is discarded (§4).
- The reference builds all four final maps with a full pass from the column
  ceiling, using neither the already-built maps as bounds nor a per-strip start.
- The reference scans light sources from the top of the column's highest non-empty
  section rather than from the strip's `H_SURFACE`.
