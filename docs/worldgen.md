# World Generation: Invariants, Fields, Pipeline

A specification for a from-scratch implementation. It describes what procedural
generation is as a mathematical object, which invariants an implementation is
obliged to hold, how the order of computation and the limits of parallelism follow
from those invariants, and what the whole thing costs.

The document deliberately does not describe the structure of Minecraft's classes.
References to the reference implementation are collected in the appendix — for
checking against, not for copying.

The numbers in §17 come from counting operations and reading sources. Not one of
them was measured on a running server.

Heightmaps are specified separately in `heightmap.md`, which this document defers
to rather than restates: the lattice of predicates, the merged descent,
incremental maintenance, and the sky light source map all live there.

---

## 1. The world as a function

A procedural world is a mapping from position to block state, fully determined by
a single number.

```
world : Seed × ℤ³ → BlockState
```

The mapping is total, deterministic, and **independent of evaluation order**.
Everything else in this document is about computing it fast enough without
breaking those three properties.

If `world` were computable pointwise and independently, there would be no
document. All of the difficulty grows out of one fact:

> **The central difficulty.** `world` is locally computable but **not pointwise
> independent**. The value at `p` depends on objects whose position is decided
> outside `p`: a tree from the neighbouring column drops leaves above `p`, a
> structure eight columns away puts a wall there, a cave eight columns away
> removes the stone.

From this — and only from this — follow stages, dependencies, neighbour windows,
and the limits on parallelism.

### Footprints

Let `U` be a unit of work and `S` a stage.

`W(S,U)` — the **write footprint**, the set of positions `S` may write to while
processing `U`.
- `R(S,U)` — the **read footprint**, the set of positions it may read.

Both footprints must be **bounded and known in advance** — not "usually small",
but computable before the stage runs. Without that you can neither plan the order,
nor parallelise, nor prove correctness.

The whole architecture follows from three statements about footprints:

**C1. Stage order.** If `S₂` reads what `S₁` writes, `S₁` must finish over the
entire footprint intersection before `S₂` starts. That is the pipeline: it is not
chosen, it is derived. **C2. Neighbour window.** To bring `U` to stage `S`, every
unit intersecting `R(S,U)` must first reach the previous stage. The window radius
equals the read footprint radius, and it accumulates down the chain of stages.
**C3. Parallelism condition.** Two units of the same stage may be processed
concurrently if and only if `W∩W = ∅` and `W∩R = ∅` in both directions.

Note what is absent from these consequences: any mention of chunks, threads, or
statuses. This is pure combinatorics of footprints.

### Two classes of stage

| Class | Write footprint | Example | Property |
| --- | --- | --- | --- |
| **Field** | its own unit only | density, biomes, surface | Trivially parallel; order between units is unobservable |
| **Scattering** | its own unit and a neighbourhood | trees, lakes, ore, structures | Requires C3; order is observable and must be pinned down |

A field stage evaluates a function. A scattering stage is a sequence of writes,
and when two such sequences overlap, the result depends on which went first.

**W1.** For scattering stages the **order in which units are processed is part of
the definition of the world**, not an implementation detail. The order must
therefore be a function of the seed and coordinates, not of thread scheduling.

The reference implementation violates W1: decoration order is decided by the order
columns arrive at the dispatcher, which is to say by where the player walked. The
same world generated along different traversal routes differs at column
boundaries.

---

## 2. Units of volume

Four different volumes take part in generation. They are easy to confuse —
vanilla's word *chunk* denotes two of them — and confusing them is expensive,
because each is tied to its own invariant and does not substitute for the others.

| Unit | Size | Tied to | Why this shape |
| --- | --- | --- | --- |
| **Strip** | 1 × 1 × H | surface rules, fill traversal | Depth above and below are quantities along the vertical |
| **Section** | 16 × 16 × 16 | block storage, palette, simulation | A cube: adjacency is symmetric on every axis. The size is fixed by the format and the protocol |
| **Column** | 16 × 16 × H | scheduling and dependencies | Heightmaps and the preliminary surface level are vertically complete quantities |
| **Tile** | multiple of a cell | field evaluation | Chosen from cache size and lattice step, not from world geometry |

**V1.** The section is the **only** unit of block storage. A column does not
contain sections, it refers to them: a column is an ordered view over sections
plus its own projections. **V2.** The absence of a section from the index
**means** uniform air, not "data not loaded". Distinguishing those two states is
the job of the layer above, and it already does so through the column's status.
**V3.** A tile need not coincide with a section, but when it does, the output of
field evaluation lands in storage without repacking. **V4.** The vertical range
`H` is a parameter of the rules layer, not of the engine. The engine is
three-dimensional; the column exists as a concept only where `H` is defined.

The main storage saving follows from V2, and it is free: on a typical
surface-world column roughly fifteen of twenty-four sections are air. Over sixty
percent of the volume is never allocated, never enters cache, and is never
traversed by any system. An implementation with a fixed array of sections per
column cannot obtain this saving.

The cost of V2 at the protocol boundary: the wire format expects every section in
the packet, empty ones included. Air sections are synthesised from a constant
during serialisation. The reverse direction matters more — **data arriving from
outside must not create a section for uniform air**, or loading a world from disk
undoes the saving entirely.

---

## 3. Density as a scalar field

Terrain is not defined by a heightmap. It is defined by the sign of a scalar
field.

```
d : ℤ³ → ℝ            density field
solid(p) ⟺ d(p) > 0
```

The form is not arbitrary. A heightmap `h(x,z)` represents only those worlds where
solid never sits above solid: no overhangs, no arches, no caves, no floating
islands. The sign of a three-dimensional field represents any closed surface while
remaining *pointwise computable*, which is what makes the stage a field stage in
the sense of §1.

A field is not written whole; it is composed from primitives. A field is a
directed acyclic graph whose leaves are constants, coordinates and noises, and
whose interior nodes are operations over fields. A graph, not a tree: the climate
fields feed both biome selection and the terrain splines.

### What every node must report about itself

Three node properties are part of the definition, not a convenience of the
implementation.

**D1. Interval.** Every node knows `[lo, hi]`, guaranteed bounds on its values.
Correct, but not necessarily tight. **D2. Rank.** Every node knows the subset of
axes `⊆ {X, Y, Z}` its value actually depends on. Conservative: an extra axis is
safe, a missing one is not. **D3. Bounded stencil.** The value at `p` depends on
inputs within a bounded, known neighborhood of `p`. For almost every operation the
neighbourhood is the point itself; the exceptions are interpolation and the
vertical surface search, whose stencils are known from their parameters. **D4.
Purity.** A node keeps no state between calls. Caching is allowed but must be
transparent. **D5. Structural stability.** The graph is fixed before generation
starts. This is what allows all compilation and analysis to happen once per world.

Three optimizations follow from these five properties — as consequences, not as
techniques:

**D1 → branch elimination.** A range-choice node whose input has an interval
disjoint from the range is equivalent to one of its arms; the other is deleted
together with its entire subtree. Likewise `min` and `max` with disjoint operand
intervals. **D2 → rank reduction.** A node of rank `{X,Z}` is constant along the
vertical within a three-dimensional volume. For a 16 × 16 × 384 column that is 256
evaluations instead of 98,304 — a factor of 384 across the whole climate layer.
**D4, D5 → common subexpression elimination.** Structurally equal subtrees are
evaluated once. No manual annotation of such places by the data author is
required: the equality is derivable.

What does **not** follow from D1–D5: any licence to **change the function
itself**. Eliminating an arm is legal because the deleted arm is unreachable, not
because it "barely matters". Rank reduction is legal because the value along the
axis is constant, not almost constant. The boundary is treated in §15.

---

## 4. Discretisation: when it is legal to sample less often

Evaluating a field at every block is expensive. The standard move is to evaluate
on a sparse lattice and interpolate. The move is not always legal, and the
legality condition determines the structure of the whole graph.

```
A lattice with step (cₓ, c_y, c_z), aligned to the origin.
ĩ(p) = trilinear interpolation of d over the eight corners of p's cell.
```

At lattice nodes `ĩ = d` exactly. Between them it is an approximation.

**L1.** `d` may be replaced by `ĩ` when the **characteristic scale of variation of
`d` is no smaller than the lattice step**. The trilinear interpolation error is
bounded by second differences over the cell, so a field with detail finer than a
cell loses that detail. **L2.** The loss does not "smooth", it **destroys**. A
tunnel one block thick that falls inside a cell without touching any corner
disappears entirely.

Hence the principal structural decision of the whole graph. **Below the
interpolation** goes everything whose scale exceeds a cell: continentalness,
erosion, ridges, terrain splines, the base 3D noise, the large cave fields — the
overwhelming majority of the graph by cost. **Above the interpolation** goes
everything at block scale: thin worm tunnels and the structural contribution to
terrain, which would otherwise vanish by L2. The order of addition matters: the
coarse part is interpolated first, then the fine part is added per block.

### Lattice geometry

The lattice step is anisotropic: finer horizontally, coarser vertically. This
follows from the structure of the field, it is not a tuning knob.

The vertical component of density is nearly linear — the height gradient
dominates. A linear function is interpolated trilinearly *exactly*, so the Y step
can be twice as coarse without losing detail. The horizontal component is a sum of
noises with a scale of a few blocks, and there the step must stay fine. The
working proportion is a **vertical step twice the horizontal one**; for a world of
height 384 that is a 4 × 8 × 4 cell.

**L3. Edge redundancy.** `n` cells along an axis need `n + 1` nodes. Two adjacent
regions evaluated independently compute their shared boundary plane twice.

For a 16 × 16 column with a 4 × 8 × 4 cell that is 5 × 49 × 5 = 1225 nodes for 4 ×
48 × 4 cells. For a region of `W × W` columns the per-column count is
`(4W+1)² · 49 / W²`:

```
W = 1  →  1225      W = 4  →  885  (−28%)
W = 2  →   992      W = 16 →  809  (−34%)
```

Enlarging the evaluation region removes up to a third of the work on the most
expensive layer of the graph, and it is **exact**, not approximate: the value at a
lattice node does not depend on which region it was computed as part of.

### Cell bounds

Interpolation has a second property, more useful than the first: it lets you
answer questions about a whole cell without evaluating a single point inside it.

> **Convex hull lemma.** Trilinear interpolation is a convex combination of the
> eight corner values: the weights are non-negative and sum to one. Therefore, for
> any point of the cell, `min(corners) ≤ ĩ(p) ≤ max(corners)`.

The interpolant never leaves the hull of its corners, and eight samples suffice
for a correct bound over the entire cell.

The bound does not stop at the interpolation. The nodes *above* it — arithmetic,
clamping, selection — are operations for which interval arithmetic gives a correct
result by D1. So one can climb from the corner intervals up the graph to the root
and obtain an interval for the final density over the whole cell.

**L4.** If the interval lies entirely on one side of zero, **the sign of the
density is constant throughout the cell**. Substance is determined without a
single per-block sample: the cell is filled with one value. **L5.** The bound must
be **conservative**. An interval that does not contain every value in its cell
leads to stone being written where air belongs — and the error will not surface in
any test that does not check for it specifically. **L6.** Not every node admits an
interval estimate. If any node on the path to the root does not, the cell is
evaluated per block. This must be checked **once at compile time**: admissibility
depends on the kind of node, never on the values.

The payoff is out of proportion to the cost. Deep below the terrain and high above
it almost every cell is uniform, and those cells are most of the world's volume.

Why this is stronger than compile-time analysis: branch elimination by D1 works
with intervals known *for the whole world*, and is therefore conservative to the
point of uselessness on most nodes. A cell bound uses intervals known *for this
cell*, which are orders of magnitude tighter. The same machinery, applied at run
time, answers where compile time has nothing to say.

---

## 5. Evaluating fields

The definition of a field is a graph. Evaluating that graph as a graph of objects,
each node asking its children for values, is the slowest form available: every
point pays for pointer chasing and an indirect call. D1–D5 allow better.

The graph is compiled **once per world** (by D5) into a linear sequence of
operations over buffers. Each operation is a flat loop over an array; dispatch
happens once per operation rather than once per point.

```
Program { ops: [Op], axes: [Axes], out: NodeId }

Op ::= Const  { dst, value }
     | Noise  { dst, params }
     | Add    { dst, a, b }
     | Clamp  { dst, a, lo, hi }
     | Interp { dst, src, cell }
     | Guard  { test, range, skip_to }
     | …
```

### A node's stratum

D2 is realised neither by inserting slice nodes nor by copying, but by the shape
of the buffer: **a node stores exactly its stratum** — the product of the sizes
over the axes it depends on.

| Rank | Axes | Length for tile s³ | Read strides |
| --- | --- | --- | --- |
| Scalar | — | 1 | 0, 0, 0 |
| Y | Y | s | 0, 1, 0 |
| XZ | X, Z | s² | 1, 0, s |
| XYZ | X, Y, Z | s³ | s, 1, s² |

**E1. Layout.** Y is the fastest axis: `index = y + (x + z·sₓ)·s_y`. A stratum is
then indexed by the same formula as the full volume, only with zero strides on the
dropped axes. **E2. Rank inference.** A node's rank is the union of its inputs'
ranks, with two exceptions: the vertical surface search always yields XZ
regardless of inputs, and an explicit slice subtracts an axis. **E3.
Interpolation.** An interpolated node's rank equals its input's, but its *inner*
volume is the cell lattice, not blocks. Buffer length is computed over the
lattice; conflating the two counts is the most common bug in the arena allocator.
**E4. There is no expansion.** A dropped axis gets a stride of zero, and the same
value is simply re-read by every consumer. No copy happens, physically or
logically. **E5. The coordinate of a dropped axis is pinned**, not arbitrary. A
dropped Y reads the volume's first row; dropped X and Z read block zero — a node
loses its horizontal dependence by zeroing the scale, and multiplying a coordinate
by zero preserves the sign. **E6. Shape is resolved outside the inner loop.**
Within one strip a node holds either a scalar or a run of `s_y` values. Which one
is settled once before the loop, and the loop then runs with no branch and no
indirection. Testing "scalar or run" inside the per-element loop costs more than
the operation itself.

The practical effect of E1 and D2 together: the entire climate layer, splines
included, is rank XZ and is evaluated per strip rather than per block.

### The tile

Tile size has nothing to do with world geometry. It follows from two constraints.

**T1. Cell alignment.** The tile is a multiple of the lattice step on each axis.
Otherwise boundary cells fall partly inside the tile and interpolation is forced
to recompute nodes. **T2. Cache.** The live set of buffers must fit in L2, and one
operation's working slice in L1. With `k` live buffers and a tile of `n` values
that is `4kn` bytes.

For a 4 × 8 × 4 cell and six live buffers: a 16³ tile gives 16 KB per buffer and
about 96 KB for the set — comfortable for L2, and every operation walks with unit
stride. If measurement shows L1 pressure, the next step down is 8 × 16 × 8, i.e. 4
KB per buffer. A tile coinciding with a section (V3) is a bonus, not a
requirement.

Two easy mistakes when choosing a tile:

**Counting one buffer instead of the set.** "A tile of six thousand values is 24
KB, it fits in L1" — 24 KB is *one* buffer, and several are live. **Tying the tile
to the fluid lattice.** The fluid lattice is not a regular batched sampling; its
caches live at column scope and do not depend on the tile. The only hard
requirement is T1.

### Branching

A range-choice node that survived D1 elimination must, in a naive tape, evaluate
both arms: a sequence of operations cannot skip evaluation for a subset of
elements. What saves it is that an arm is needed or not needed **by an entire
strip at once**, not by individual points.

> The **exclusive cone** of a guarded input is the set of nodes reachable no other
> way than through that input: not from the other arm, not from another consumer,
> not from another root.

A cone, not a whole subtree: a node used elsewhere cannot be skipped even when
this arm is unneeded.

Operation order within the tape is free up to topological order. So a cone can be
gathered into a contiguous run, and "this arm is unneeded" becomes not an element
mask but a **jump over a piece of the schedule**.

```
for each strip:
    for each schedule entry:
        if the entry is a guard:
            read one already-evaluated run
            answer for the whole strip at once
            if the cone is unneeded → jump past the run
        else:
            execute the operation over the strip
```

The guard test reads **one already-evaluated run** and answers for the strip as a
whole. That is cheaper than an element mask and does not require the condition to
align with tiles. A site whose input was skipped needs something written into it:
selection operations need nothing — they read the chosen arm and never the skipped
one; for the rest it is part of the guard's definition.

Independently of branching, tiled evaluation buys a second win: intermediate
buffers stop travelling to memory. Evaluating "one operation at a time over the
whole volume" materialises a full buffer per graph node — for a column that is
hundreds of kilobytes per buffer and megabytes of live set, which on a many-core
machine evicts the shared cache and *degrades* scaling as cores are added.

### What the graph compiler does

1. Resolves references to named fields, reducing the graph to an arena of nodes
   addressed by integer indices instead of pointers.
2. Finds common subexpressions by structural equality and assigns them one buffer
   (from D4, D5).
3. Computes intervals bottom-up and deletes unreachable arms (from D1).
4. **Folds a constant operand into the operation itself.** "Multiply by a constant
   and add a constant" is one operation, not two with a buffer between them.
   Likewise: min and max against a constant, subtraction from a constant, division
   of a constant, raising to a constant power, a piecewise-linear function with
   different slopes either side of zero. Each fold removes a node, a buffer and an
   entire pass over the data; on a real terrain graph there are dozens of such
   places.
5. Computes ranks and assigns buffer lengths (from D2).
6. Computes liveness and reuses buffers. Without this pass a column's program
   would hold dozens of full-volume buffers — tens of megabytes per task.
7. Deletes nodes unreachable from any root.

**E7. Root reachability.** Evaluating a root traverses **only the nodes reachable
from it**, not the whole program array.

This is not a micro-optimisation but protection against recursive blow-up. An
unrelated interpolated node that happens to be in the program for another root
will, under a whole-array traversal, materialise its own lattice; evaluating that
materialises the first node's lattice; and so on. Cost grows multiplicatively in
the number of independent interpolations, not additively.

---

## 6. The terrain field, dissected

This section dissects one concrete field — surface-world terrain — as an
application of §3–5. The numeric parameters come from the reference
implementation, because they *are* the content of the world; the structure,
however, follows from the invariants.

```
A  climate    shift_x, shift_z ──▶ continents, erosion, ridges,          rank XZ
                                   temperature, vegetation
B  splines    (continents, erosion, ridges) ──▶ offset, factor,          rank XZ
                                                jaggedness
C  3D         depth   = y_gradient + offset                              rank XYZ
              initial = quarterNeg((depth + jaggedness) · factor) · 4
              sloped  = initial + base_3d_noise
D  caves      range_choice(sloped < threshold)
                  ├─ above surface: entrances · 5
                  └─ below:         cheese, layers, spaghetti, pillars
E  post       slide ──▶ blend ──▶ × 0.64 ──▶ interpolated(4 × 8) ──▶ squeeze
              ──▶ min(…, noodle) + beardifier  =  final_density
```

What this illustrates:

**Layers A and B are rank XZ throughout** — half the graph's nodes, evaluated per
strip by D2. The factor is the world height. **Exactly one interpolation point**,
placed where large-scale fields end and block-scale ones begin. Precisely the L1
boundary. **The vertical gradient is nearly linear**, which is what justifies the
twice-coarser lattice step along Y. **The range choice in layer D** is the node
the exclusive cones exist for: the underground arm is expensive, and in a strip
above the surface it need not be evaluated at all. **The structural contribution
is added last** and outside the interpolation. Otherwise a wall cutting into a
slope would be smeared across a four-block cell.

### Preliminary surface level

```
surfaceLevel(x,z) ≈ the greatest y where the simplified density > 0
```

Simplified means without the 3D noise and without caves: layers A, B and C minus
the noise term. The search runs top-down with a coarse step from an analytic upper
estimate derived from `offset` and `factor`.

This is an **upper estimate** of the real surface, not the surface itself: the
real one is lower, because noise and caves only remove material. That
one-sidedness is exactly the property that makes the quantity usable as a scan
bound in §7 and §8.

**P1.** `surfaceLevel` is a rank-XZ field and must be evaluated as one. Treating
it as three-dimensional is the most expensive mistake in this part of the graph.

---

## 7. From field to substance: fluid

The sign of density answers "solid or void". What fills the void remains.

```
substance(p, d) = solid,     if d(p) > 0
                = fluid(p),  if d(p) ≤ 0 and y < level(p)
                = air,       otherwise
```

The naive choice `level ≡ sea level` gives a correct but impoverished world: every
cavity below sea level is flooded, every one above is dry. There are no isolated
underground lakes and no bodies of water perched above a void.

For those to exist, `level` must be a field of a particular kind. Space is
partitioned into cells by a Voronoi diagram over a lattice with deterministically
jittered centres; each cell carries its own fluid level and type; the level at a
point is decided by the nearest centre.

**A1.** The lattice is **anisotropic**: the vertical step is finer. Bodies of
water must separate by height more often than horizontally, or multi-storey caves
all share one level through their whole depth. **A2.** A centre's offset within
its cell is a function of the seed and the cell index. Hence **the level field is
pointwise computable**; no traversal or flood fill is required. **A3.** At the
boundary between two cells with different levels a **barrier** is inserted: a term
growing with proximity to the boundary is added to the density. Without it two
neighbouring bodies of water would meet at a vertical wall of water.

### Cost, and how to remove it

This is the most expensive per-block operation in generation, and once the other
stages are optimised it becomes the largest single item.

**A4. Skip from above.** Above `surfaceLevel` plus a margin no isolated body of
water can exist. The threshold must be computed **per strip**, which is an
instance of a rule that recurs across this pair of documents:

> **Per-strip bounds.** A scan bound derived from a column-wide aggregate is not a
> bound, it is the worst case. One tall peak denies the cheap path to all 256
> strips. The same mistake appears in heightmap construction and in sky light
> seeding; see `heightmap.md` §4 and S2.

**A5. Skip by confidence.** If the nearest centre beats the second by a margin,
the barrier need not be evaluated — its contribution cannot flip the sign. **A6.
The cell cache is a flat array.** The query region is known before the start; it
follows from the volume and the lattice step. A hash table has no business in this
hot loop. **A7. Caches live at column or region scope**, not tile scope. It
follows that the fluid lattice imposes no constraint on tile size (T1).

Why this stage does not vectorise with the rest: selecting nearest centres is a
search with branching and sparse access, not arithmetic over a contiguous array.
When everything around it is accelerated, fluid's share of the cost grows — from
about a tenth to about a sixth. What is possible: batch the barrier noise, replace
the cascade of insertions with a branchless sorting network, apply A4 per strip.
What is not: remove the search itself without changing the world.

---

## 8. Surface: a rewriting system over a strip

Fill produces a world of one material. Surface rules turn it into a world of sand,
grass, gravel and snow.

A rule is a tree of conditions and substitutions, evaluated at every point of a
strip from the top down. The result is either a new block state or "leave alone".
Conditions are not tested against blocks directly but against a **context**:

```
context = { depthAbove, depthBelow, waterLevel,
            biome, surfaceLevel, gradientX, gradientZ, y }
```

`depthAbove` — how many solid blocks run upward from here; reset by air. "How deep
below the surface." `depthBelow` — the distance down to the nearest void, from a
look-ahead scan. "How close is the cave ceiling below." `waterLevel` — the height
of the last fluid seen from above. Distinguishes a lake bed from a hillside.
`gradient` — the height difference between neighbouring strips. Distinguishes a
slope from a plateau: gravel instead of grass.

### Memoisation as a consequence of the definition

Context quantities change at different rates: biome, steepness and 2D noises are
constant over a strip, while depths and water level change at every step downward.
From this follows an exact caching scheme with no hash tables and no keys.

```
the context holds two counters: genXZ and genY
moving to a new strip: genXZ += 1; genY += 1
moving to a new y:                 genY += 1

a lazy condition holds its own generation and its result:
    if its own == the matching counter → return the cache
    otherwise → recompute, record the generation
```

**M1.** The scheme is **exact**, not approximate: by D4 a condition is a pure
function of the context, so with the generation unchanged the result cannot have
changed.
- **M2.** A cache hit costs one integer comparison. Cheaper than any keyed cache.
  **M3.** A two-dimensional condition is evaluated once per strip, a
  three-dimensional one once per block, **regardless of how many rules referred to
  it**. This is what makes rule trees of hundreds of nodes practical.

### Cost

**Sf1. The tree is cheaper than it looks.** The overwhelming majority of nodes sit
behind one shared condition, "above the preliminary surface". A deep block runs
through single-digit numbers of nodes, not hundreds. The tree is not what needs
optimising. **Sf2. Reading the block is expensive.** The descent reads a state at
every step, and if that read goes through a palette with bit unpacking it
dominates the stage. A flat strip array removes it entirely. **Sf3. Early exit is
impossible.** Ore vein rules apply at any depth and carry no bounding condition.
The descent goes to the bottom.

Some landforms express poorly as rules and are handled by separate passes: eroded
badlands pillars are built *before* the rule pass, icebergs *after*. The order is
part of the definition here: pillars must be subject to the rules, icebergs must
not.

---

## 9. Carving: caves as a set

Large tunnels and canyons have a shape extended along a trajectory, which a field
expresses badly. The second mechanism is direct carving.

Carving produces a **set of positions** `C ⊆ ℤ³`. A carving source is the unit of
volume where the trajectory begins; the overall result is a union:

```
C = ⋃ carve(seed, source)   over all sources
```

Phrasing it as a set rather than a sequence of writes is not stylistic.

**K1.** The union is **idempotent and order-independent**. Two sources that carved
the same position give the same result as one. Hence the processing order of
sources is unobservable — unlike the scattering stages of §11. **K2. Geometry is
separated from substance.** The set does not know what to put in the space it
freed; substitution is a separate pass using the fluid level field of §7. **K3.**
The set is **bounded by construction**: a trajectory has bounded length, so for
every unit there is a finite radius beyond which no source can reach it.

From K1 follows the main practical property: **a mask may be built over a region
of any size, and the result coincides exactly with the union of the masks of its
parts.** That makes enlarging the evaluation region free as far as correctness
goes. For a region of `W × W` the source count per unit area falls as
`(W + 2r)² / W²`: for radius 8 and a 16 × 16 region that is four carver passes per
column instead of 289.

Contrast with scattering stages, where enlarging the region changes write order
and therefore changes the world. The difference is entirely K1.

### Representation

**K4.** A bit array with the **vertical in the low bits**: the segments of one
strip lie contiguously. Tunnels and shafts produce long runs, and traversing
set-bit runs is an order of magnitude cheaper than bit by bit. **K5.** Traversal
hands the consumer **ranges** `(x, z, from, to)`, not individual positions. The
consumer works by strips anyway.

### Trajectory pruning

A trajectory must be able to answer, at each step, whether the remaining path can
still reach the target region; if not, the recursion stops. With such pruning the
real redundancy of the trajectory walk is tens of times, not hundreds, and the
walk is the smaller part of the stage's cost: rasterising the shape into the mask
dominates.

Hence a planning conclusion: **caching trajectories per source buys little.** The
win comes from enlarging the region (K1), which removes the redundancy of both the
walk and the rasterisation at once.

A carved block is passed to `substance` with a density of zero rather than the
real one. This is deliberate: in a newly opened cavity the question "solid or
void" has already been settled by carving, and only "air or fluid" remains.

---

## 10. Structures: placement and terrain adaptation

A structure is a built object occupying a volume that may greatly exceed the unit
of scheduling. The problem splits into two independent ones: where the structure
stands, and how the terrain adapts to it.

### Placement

```
gridCell = ⌊position / spacing⌋
offset   = f(seed, gridCell, salt)      deterministic draw
starts(position) ⟺ gridCell · spacing + offset = position
```

Scatter with a fixed spacing and a random offset within the cell yields a
distribution that has both a minimum distance between instances and visible
irregularity. Pure Poisson scatter would clump; a pure lattice would be visibly a
grid.

**G1.** Placement is a **pure function of the seed, the cell coordinates and the
set's salt**. It does not read the world: not terrain, not biome, not neighbouring
structures.

G1 is stronger than it looks. The reference implementation makes placement a
stage: a column is advanced to a "structure starts" status, the result is stored
in it, neighbours read it. That produces the widest dependency in the whole
pipeline — to bring one column to decoration you must materialise its window of
radius eight, i.e. 289 columns.

But by G1 no column is needed at all. An **index from grid cell to list of
starts**, computed lazily and cached, gives the same answer without
materialisation. That removes radius eight from four stages at once and unties the
size of the generation region.

Biome suitability checks and module layout for composite structures do read the
world, but they run *after* the index has said "there is a start here" — for
single cells, not for 289 columns.

### Terrain adaptation

A structure dropped into finished terrain will hang in the air on a slope or drown
in a hill. The adjustment is expressed as an addition to the density field:

```
d'(p) = d(p) + Σ contribution(piece, p)   over nearby structure pieces
```

**B1. Bounded support.** Each piece's contribution is zero outside a neighbourhood
of fixed radius. Without this the field would stop being locally computable and
the write footprints would stop being bounded. **B2. Additivity.** Contributions
add rather than replace. Two nearby pieces give a joint foundation, not a dispute
over one. **B3.** The addition is applied **after interpolation** (§4): its scale
is per block, and a four-block cell would smear it.

Four contribution shapes suffice: bury, grow a foundation beneath, encapsulate,
leave alone.

From B1 follows a cheap test: the union bounding box of nearby pieces, inflated by
the support radius. If a tile does not intersect it, the whole contribution is
zero and costs one zero fill — which is the case for the overwhelming majority of
the world's columns. The convolution kernel is a table depending only on geometry
and is built once per process.

---

## 11. Scattered objects: order as part of the definition

Trees, ore, grass, lakes, boulders — what §1 calls a scattering stage. Here the
write footprint leaves the unit, and here alone the processing order is
observable.

A scattered object is a pair of placer and generator. The placer is a chain of
modifiers, each mapping one position to zero or more positions.

```
positions := [origin]
for each modifier m in the chain:
    positions := ⋃ m(position) over all positions
for each resulting position: generator.place(position)
```

Typical modifiers: repeat `n` times, drop with a probability, jitter within the
unit, raise to the surface via a heightmap, draw a height from a distribution,
test the block below, test the biome.

**F1.** An object's seed derives from the triple "unit seed, decoration step
number, global object index". **F2. The global index is part of the definition of
the world.** It comes from a topological sort: each biome fixes its own order of
objects, a precedence graph is assembled from all biomes, and the sort yields a
single numbering. A different sort is a different world under the same seed. **F3.
The biome test is mandatory and is not an optimisation.** An object runs for every
biome present in the window, and the filter rejects placements where the object
does not belong. Without it, cactus appears in forests along biome borders.
**F4.** Unit order is pinned deterministically (W1). Thread scheduling must not
influence it.

F2 is the least obvious point in this specification. It seems as if object order
were an implementation detail. The difference is that the index feeds the seed, so
shifting an index by one changes every draw of that object across the entire
world. The topological sort is reproduced verbatim, depth-first traversal and
vertex visiting order included. This is the only place in the document where
copying an algorithm is prescribed rather than deriving it.

### Execution

**F5.** The modifier chain is an **explicit stack machine** with reused buffers,
not recursion or lazy sequences. There are hundreds of objects per unit, and
allocating intermediate lists dominates the useful work. **F6.** The window's
biome set is computed **once per batch of units**, not per unit. It reads the
palettes of every neighbouring section. **F7.** Block access goes through a flat
representation of the window, not a palette. Same reason as Sf2.

Besides scattered objects, this stage also contains **materialisation of structure
pieces**: template layout, rotations, replacement predicates, thousands of block
writes inside a village. Its load profile is entirely different, and cost
estimates must separate the two. This is the largest and least predictable stage
of the pipeline.

---

## 12. Biomes: classification in climate space

A biome is defined as the nearest point in a multidimensional parameter space.

```
climate(p) ∈ ℝⁿ        n climate fields, evaluated as fields per §3
biome(p) = argmin_b  dist(climate(p), region_b)
```

Each biome is associated not with a point but with a **hyperrectangle** — one
interval per parameter. The distance to a rectangle is zero inside it and grows
outside; the metric is the sum of squared per-coordinate distances.

A rectangle rather than a point, because a biome should occupy a region of climate
rather than be nearest to an ideal. Zero inside, because within its own region a
biome wins unconditionally, without competing for the centre.

**Cl1. Integer quantisation.** Parameters and bounds are converted to integers by
a fixed scale, and the whole metric is computed in integer arithmetic. The reason
is portability: comparing floats at the border of two biomes otherwise yields
different results on different platforms. **Cl2. Resolution coarser than a
block.** A biome is defined on a lattice with a step of several blocks. This is
both an economy and a substantive decision: a biome is a property of terrain, not
of a point. **Cl3.** Climate space is an **input, not an output**. The climate
fields are evaluated by the graph of §3 and are also used by the terrain splines;
by D4 they are evaluated once.

### Search structure

The biome count is in the tens, the dimensionality in the single digits. That is
the range where the answer is not obvious.

**Linear scan**: with `N` biomes and `n` parameters, `N·n` interval comparisons.
In a per-parameter array layout that is a few linear passes over contiguous
memory, fully vectorisable, with no pointer chasing.

**Tree**: a hierarchy of bounding rectangles with branch and bound. Asymptotically
better, but with dependent loads and cache misses. It wins thanks to one trick:
the previously found leaf is passed in as the starting candidate, and the first
bound then prunes almost the whole tree.

Recommendation: start with the linear scan — it is simpler and needs neither
thread-local state nor an index build — and switch to a tree only if measurement
justifies it. The stage is single-digit percent of a column's cost.

From Cl2 and D2 it follows that biomes are a **batch stage by construction**: the
climate fields are evaluated over the lattice in one program run, and
classification then walks ready arrays. Evaluating climate pointwise per cell is
the most common mistake in this stage.

---

## 13. Footprints and order: where the pipeline comes from

Sections 6–12 defined the stages. Now each one's footprints can be computed and
C1–C3 applied.

| Stage | R | W | Class | Where the radius comes from |
| --- | --- | --- | --- | --- |
| Climate and biomes | 0 | 0 | field | Pointwise function of coordinates |
| Density → blocks | 0 | 0 | field | The field's stencil is absorbed by the tile |
| Surface | 0 | 0 | field | Gradient over own strips, edges clamped |
| Carving | r_c | 0 | field | Trajectory length; by K1 order is unobservable |
| Structure placement | — | — | outside the pipeline | By G1 it does not read the world at all |
| Structure materialisation | r_s | r_s | scattering | Structure extent |
| Scattered objects | 1 | 1 | scattering | A tree at the edge reaches the neighbour with its canopy |
| Light | 1 | 1 | scattering | Propagation distance over unit size |

### Deriving the window

By C2 radii accumulate down the chain. Were structure placement a stage with
radius `r_s = 8`, the accumulated window would look like this:

```
layer                   radius   units
empty                        9     361
structure_starts             9     361
structure_references         1       9
biomes                       1       9
terrain                      1       9
features                     1       9
light                        0       1
```

That table is the cost of a decision we **rejected** in §10. The widest dependency
in the pipeline is produced not by the physics of structures but by placement
having been made a stage that stores its result in a column. With an index instead
of a stage the accumulated radius drops to the maximum of `r_c` and the scattering
radii — single digits.

### Deriving parallelism

Field stages are unconditionally parallel by C3. Scattering stages need a test:

```
conflict(U₁, U₂) ⟺ |Δx| ≤ r_w + r_r  AND  |Δz| ≤ r_w + r_r
```

A conjunction, not a disjunction: footprints are squares, and two squares miss
each other if they separate far enough on *either* axis. Phrasing it with "or"
gives an incorrect, needlessly conservative conflict set.

Colouring by residue: `colour(U) = (x mod k, z mod k)` where `k = r_w + r_r + 1`.
Within one colour any two distinct units differ by at least `k` on at least one
axis, and `k > r_w + r_r`, so by the predicate they do not conflict.

**N1.** The colouring is correct for reads as well, because `k` is computed from
the sum of both radii. Taking `k = 2r_w + 1` and forgetting the read footprint is
a bug that surfaces as rare, irreproducible seam artefacts. **N2.** The colour
order is fixed, so write order is reproducible, so W1 holds. This is how the
nondeterminism gets fixed, not merely how throughput improves. **N3.** The wave
barrier between colours is paid for at the maximum unit cost in the batch, not the
mean. The variance is large: a unit containing a structure costs several times an
empty one.

**A hole created by parallelising.** Light reads neighbours at radius 1, and
scattering writes at radius 1. So a unit at distance 2 may write into a unit at
distance 1 that light is reading at that moment — even though the "light after
scattering" dependency is declared only at radius 1. In a sequential
implementation there is no race; it appears from the parallelisation itself and
must be closed explicitly: either declare light's dependency on scattering at
radius 2, or place a barrier on scattering for light's halo.

### Enlarging the region

Three mechanisms benefit from the evaluation unit being larger than the storage
unit:

**L3.** Lattice edge redundancy falls as `(cW+1)²/(cW)²` — up to a third of the
work on the most expensive layer. **K1.** Carving is computed once per region:
sources per unit area fall as `(W+2r)²/W²`. **C3.** Inside a region, scattering
stages need neither colouring nor locks — conflicts are possible only at the seam.
The seam fraction falls as `1 − ((W−2)/W)²`.

Constraint: only lattice evaluation may be enlarged. A block-resolution buffer
grows as `W²`, and for a 4 × 4 column region that is already megabytes,
contradicting T2. The block-resolution part stays fine-grained and tiled.

---

## 14. Granularity in the ECS

One rule: an entity is justified where things are **filtered** by it and where it
is **iterated**.

**X1. Generation lives outside the ECS.** It is a dependency graph with heavy work
and random spatial access; a component query does not express "give me the unit at
this coordinate", and advancing a status on task completion does not fit a
per-frame schedule. Only the finished result enters the ECS world. **X2. The
column is the lifecycle entity.** Tickets, generation status, heightmaps, network
packet assembly, the modified flag. **X3. The section is the simulation entity.**
Light, block ticking, block entities, membership in the simulation radius. **X4.
Heavy data lives in arenas, not components.** A section's block array conflicts
with every reader, and change detection over it is meaningless at that
granularity.

X3 looks excessive: two dozen sections per column. In fact there are fewer — by V2
air sections do not exist, and a typical column has about nine live. But the count
is not the point.

The real argument is **structural filtering**, with parallelism as a consequence.
With column entities a ticking system walks every column and branches internally
across slots: there is nothing to filter on at section granularity. With section
entities membership becomes an archetypal property and the system never sees what
does not concern it. Parallel iteration gets thousands of work items instead of
hundreds.

### Representation invariants

**X5. An arena key carries a generation.** An index key without one is reused
after a slot is freed, and a stale reference then silently points at someone
else's data. Keys inevitably outlive their entity — they go into background
saving, into the network layer, into the I/O queue — and that confusion produces
data corruption that is nearly impossible to debug. **X6. Each arena has its own
key type.** There are at least two arenas; with a single key type the compiler
will not catch indexing one arena with the other's key. **X7. What crosses a
subsystem boundary is an owning snapshot, not a key.** A key lives within the
tick. Saving, networking and I/O receive a reference-counted snapshot: lifetime is
explicit, a background task cannot observe substituted data, and a stale key
becomes impossible by construction rather than by check. **X8. The
column-to-sections link is a custom relationship**, not the built-in hierarchy.
The built-in one drags transform propagation along, which sections do not need and
which surfaces when code is shared with a client.

### Marker or field

**X9.** A marker is for rarely changing membership that is genuinely filtered on:
within the simulation radius, has block entities, has randomly ticking blocks,
column modified. **X10.** Frequently changing state is not a marker. Generation
status changes ten times in a row per column; "light needs recomputation" is
raised and cleared every tick. Both as markers cause constant archetype migration.
The first is an enum field, the second a change list in a resource. **X11.**
Component change detection replaces neither: it is not an archetypal filter, and
the query still walks every section.

### Section systems that write across the border

Free parallelism over sections works exactly while a system writes only into its
own section. Light propagation crosses the border by definition, and exclusive
access to six neighbours cannot be obtained from one query. Four workable forms:

What is being parallelised is specified in `heightmap.md` §7: the source map
exists precisely so that seeding enqueues boundary cells rather than the whole lit
volume.

1. Compute locally and push cross-border output into a delta queue applied in a
   separate phase.
2. The three-dimensional analogue of the §13 colouring: a one-section halo gives
   27 classes.
3. Give light a whole region and do not split it by section at all.
4. **Change the unit of parallelism**: parallelise over propagation frontier cells
   rather than sections. The question of section borders then does not arise: the
   frontier knows nothing of sections, and relaxation rounds give a natural
   barrier. The section remains the unit of storage and filtering, but not of this
   particular parallel work.

AI is harder: entities migrate between sections, so membership is a derived index
with a single owner, and the systems must iterate entities while reading
neighbouring sections, rather than iterate sections.

---

## 15. The determinism contract

The world is a function of the seed. The question is how precisely that function
is pinned down and what the implementation is allowed to change.

**Allowed** is anything that computes the same function differently: a different
order of arithmetic, fused multiply-add, vectorisation, approximation of
elementary functions. Such changes move the surface by fractions of a block where
the density is near zero — a thin shell around existing boundaries, fractions of a
percent of the volume.

**Forbidden** is anything that changes the function itself: a different random
number generator, a different topological sort order, a different structure
placement lattice, a different hash. That is not "a few blocks differ" — it is a
different world under the same seed, a hundred percent different rather than five.

| Technique | strict | fast | Note |
| --- | --- | --- | --- |
| Replacing the vertical interpolation recurrence with the direct formula | no | yes | **The main one.** Enables vectorising along the contiguous axis without transposition |
| Fused multiply-add | no | yes | One rounding instead of two |
| Vectorising across noise octaves | no | yes | Reassociates a sequential accumulator |
| Fast approximations of elementary functions | no | yes | Selectively, guided by a profile |
| Own seeded spawn draw | no | yes | Fixes the reference implementation's irreproducibility |
| Single-precision noise coordinates | no | **no** | Far from the origin the step is comparable to a block |
| A different RNG or hash | no | **no** | Changes the function, not the precision |
| A different topological sort order | no | **no** | Same |
| A different structure placement lattice | no | **no** | Same |

**R1.** The strict profile **must remain buildable** even when the fast one ships:
it serves as the oracle. **R2. Divergence is measured, not estimated.** The
fraction of differing blocks between profiles over a fixed seed set is a budgeted
test metric. A jump in one stage points at the culprit. **R3. Topology is checked
separately from blocks.** Heightmaps matching within one block, and the biome set
matching. Percent divergence with matching heightmaps is rounding noise; the same
percent with heightmaps that have drifted is a broken function. **R4.** Comparison
against an external world means **decoded content, never file bytes**: the
compression implementation is ours, and so is the allocation order inside the
container. **R5. The oracle carries its source version, and that version is
checked.** Reference dumps are taken from a specific version; when the version
changes, the world legitimately changes. The test must *fail* on a version
mismatch rather than skip the comparison: an oracle that has silently become
worthless is worse than no oracle, because it manufactures confidence. **R6. An
optimisation that grants the right not to compute is verified separately.** A cell
bound (L5) does not crash and does not hurt throughput when it is wrong — it
writes stone through air. A test is needed that, for every "settled" cell, checks
the bound against every density inside it.

Where parity with an external world is unattainable in principle: the scattering
stages. By W1 their result depends on unit processing order, and in the reference
implementation that order is decided by the player's route. A fixed colour order
makes *our* worlds more reproducible, but not equal to a particular foreign one.
Parity is attainable for climate, biomes, terrain and structure placement, and
unattainable for decoration.

---

## 16. What to store and what to recompute

The only source of truth is the seed and the data set. Everything else is
recoverable; the remaining question is only the cost of recovery, and whether a
value has stopped being recoverable.

| Data | Category | Store | Why |
| --- | --- | --- | --- |
| Seed, data set, compiled programs | truth | yes | the source of everything else |
| Blocks of an untouched unit | projection | conditionally | recoverable from the seed, see below |
| Blocks after gameplay edits | **truth** | yes | stopped being a function of the seed |
| Heightmaps | projection of blocks | yes | hundreds of bytes against a pass over strips; see `heightmap.md` §8 |
| Sky light source map | projection of blocks | **no** | one bounded pass per strip is cheaper than storing it; `heightmap.md` S5 |
| Light | projection of blocks | yes | recovery requires traversal across unit borders |
| Biome palette | projection of the seed | yes | cheaper than recomputing climate |
| Ticket level, target status | projection of players | no | pure function of current state |
| Structure placement index | projection of the seed | no | by G1 computed on demand |
| Serialised network blob | projection of blocks | cache | identical for every recipient |

### Untouched units

Content prior to the first edit is a pure function of the seed, materialised only
because recomputation costs. Hence the temptation not to store such units and to
regenerate them on load. The conditions under which that is legal:

**St1. Generation really is deterministic.** The reference implementation is
defective here: the generation-time creature draw consults an unseeded level-wide
randomness source. Our own seeded draw fixes it, but by §15 that is available only
in the fast profile. **St2. The "untouched" flag is raised on entity entry too**,
not only on a block write. A creature that wandered in from a neighbouring unit is
also a state change, invisible to a block dirty bit. **St3. A configuration hash
is stored alongside.** Regeneration is valid only while the generator version, the
precision profile and the data set are unchanged. Otherwise a server update
silently redraws untouched regions of the world.

The payoff when all three hold is save size and cold read, not generation
throughput.

**St4.** Check dimensions against the current world configuration and **rebuild
rather than trust** on a mismatch: world height may have changed between sessions.
**St5.** Treat incoming values as **untrusted input**. Heightmaps arrive from disk
and over the network, and the bit field is wider than the real range. If a value
is used as an index — and every descent optimisation does exactly that — it must
be clamped.

---

## 17. Cost and limits

Not one quantity below was measured on a running server. Three numbers the result
depends on linearly remain unknown; measuring them is step 0 in §19.

### How much work a column requires

Orders of magnitude for a 16 × 16 × 384 column:

**XZ layer** — 256 evaluations per rank-XZ field. Half the graph's nodes and a
small share of the cost, thanks to D2. **Lattice** — about 1,200 interpolation
nodes against 98,304 blocks. This is where the arithmetic lives. Of those nodes,
cell bounds (L4) settle outright the ones deep below the terrain and high above it
— most of the volume. **Per block** — 98,304 substance evaluations, as many block
writes, roughly 30,000 surface rule applications, mask rasterisation, thin tunnels
and the structural contribution. **Scattering** — hundreds of objects with
modifier chains, plus structure piece materialisation whose cost differs by an
order of magnitude between "empty" and "village".

The key observation: **once the lattice part is accelerated, the per-block part
dominates**, and it vectorises poorly. Optimising the density graph has a ceiling
set by what stands next to it.

### Two independent limits

```
throughput ∝ (N · efficiency · parallel_fraction) · (1 / work_per_unit)
```

The first factor grows only with core count and is capped by the fraction of work
an implementation can actually run in parallel. The second does not depend on
cores and is bounded below by how much work the definition requires.

Summing stage shares by Amdahl with realistic improvement factors — terrain ~2.8,
scattering ~2.2, surface ~2.5, carving ~2.5, biomes ~1.8, light ~3, I/O ~2 — gives
a work reduction of roughly **2.5×**, and the spread of estimates does not leave
2.4–2.7. That is a hard ceiling: the limit is set by stage shares, not by the
quality of each stage.

The reference implementation runs only two of ten stages in parallel: biomes and
terrain. Structure placement, reference collection, scattering and spawning
execute on a single sequential executor, light on a second, finalisation on the
main thread. For scattering that is not a detail: by volume of work it is
comparable to terrain. The reason is understandable — the write footprint leaves
the unit, and sequencing is the simplest way to satisfy C3. But not the only way:
the colouring of §13 satisfies the same condition and is parallel.

Hence: **the main factor comes not from code quality but from the definition
admitting parallelism that the reference implementation does not use.**

| Cores | Ratio | Comment |
| --- | --- | --- |
| 8 | ≈ 4× | No higher: Amdahl on the sequential part is the limit |
| 16 | ≈ 8× | Spread of 6 to 12, depending on three unmeasured quantities |
| 24 | ≈ 12× | A tenfold ratio stops being optimistic |

**On the baseline.** More than half of the ratio comes from the first factor, i.e.
from parallelism the other implementation leaves unused. Compared against an
implementation that already uses it, only the second factor remains — about 2.5×.
Both numbers are honest, but they are different claims; agree in advance which one
is being measured.

### What binds after all the work

1. **Scattering.** Data-dependent branching, a sequential draw inside each object,
   scattered writes. Neither tiles nor vectorises.
2. **The parallel fraction.** The first factor is linear in it, and it is
   unmeasured; estimates differ by a factor of two.
3. **Substance.** Its share grows once its surroundings are accelerated (§7).
4. **Whatever we re-serialise ourselves.** The allocator inside the storage
   container, finalisation on the main thread, barriers between colours.

What is **not** on the list: memory and disk. With proper tiling, memory traffic
is two orders of magnitude below bandwidth. The impression that generation is
memory-bound arises only for implementations that materialise full-volume buffers
per graph node.

---

## 18. What not to do

**Treat a cell bound optimistically.** The interval must contain every value in
the cell (L5). An understated bound surfaces in no test except one written
specifically against it. **Materialise a broadcast by copying.** A dropped axis is
a zero stride, not duplicated data (E4). **Traverse the whole program array to
evaluate one root.** Cost grows multiplicatively in the number of independent
interpolations (E7). **Treat scattering order as an implementation detail.** It
feeds the seed through the object index (F2) and the result through overlapping
write footprints (W1). **Write the conflict predicate with "or".** Footprints are
squares; the predicate is a conjunction. **Derive the colouring step from the
write radius alone.** It is the sum of write and read radii plus one (N1). **Put
block-scale fields below the interpolation.** They will not blur, they will vanish
(L2). **Treat the preliminary surface as a three-dimensional field.** It is rank
XZ (P1). **Derive the fluid skip threshold from the column-wide surface maximum.**
One peak denies the cheap path to all 256 strips (A4). **Count on an early exit
from the surface rule descent.** Ore vein rules carry no bounding condition (Sf3).
**Plan on caching carver trajectories as a separate optimisation.** The win comes
from enlarging the region (§9). **Substitute the real density when filling carved
space.** The zero there is deliberate (§9). **Make structure placement a pipeline
stage.** By G1 it is a pure function, and making it a stage buys the widest
dependency in the pipeline for nothing. **Store a fixed array of sections per
column.** That cancels V2 and the whole saving on empty volume. **Use an index key
without a generation** for data whose keys outlive the entity (X5).
- **Make a marker out of state that changes every tick** (X10). **Change the RNG,
  the hash or the sort order for speed.** That is a different world, not a faster
  computation of the same one (§15). **Take noise coordinates in single
  precision.** Far from the origin the step is comparable to a block. **Vectorise
  across noise octaves in the strict profile.** It reassociates a sequential
  accumulator. **Build a hierarchical terrain representation with refinement.**
  Density is not band-limited: thin tunnels are single-octave and block-scale, so
  a coarse pass gives no conservative estimate of the fine one. The error would
  not be a shell around boundaries but structurally different caves. **Replace the
  generation unit with lazy per-block generation.** Collision, pathfinding, light
  and fluid ticks all need the full volume. Same work, worse locality. **Count on
  deduplicating identical units.** To discover that two units match you must
  generate both. **Optimise storage before generation is fast.** That raises a
  ceiling, it does not produce a win. **Move generation to the GPU.** The stages
  that dominate after optimisation — scattering and structure materialisation —
  are branchy pointer-chasing code with a sequential draw. Field stages would map
  to a GPU, but those are precisely the ones that stop being the bottleneck.

---

## 19. Implementation order

**Step 0 — measure before designing.** Costs a day. All of the arithmetic in §17
rests on three unmeasured quantities.

a. Run the reference implementation on 4, 8, 16 and 32 cores and check the
prediction that throughput is flat above a small core count. If it is not flat,
the parallel fraction is larger than estimated and half the expected ratio
disappears.

b. Take the stage shares. Nothing needs instrumenting: the reference
implementation already emits per-stage events.

c. Separately measure the scattering share, split into "objects" and "structure
pieces". This is the largest uncertainty in the whole plan.

| # | What | Reference to check against |
| --- | --- | --- |
| 1 | Noise and RNGs, bit for bit. Everything else stands on them | Control values from the reference |
| 2 | Field graph and a naive pointwise interpreter. Slow, but certainly correct | Heightmaps checked numerically |
| 3 | Storage: sections, palette, flat representation, heightmaps (`heightmap.md`) | Isolated structural tests |
| 4 | Fill, substance, surface rules, carving. The first real world | Bit-for-bit block comparison |
| 5 | The footprint scheduler. On a single-threaded executor first | The same result as step 4 |
| 6 | Structures and scattering. Prototype and measure *before* committing to the rest | Sort order against the reference |
| 7 | Compiling the graph into a tape: strata, liveness, constant folding, tiles, exclusive cones, cell bounds | The naive interpreter of step 2, bit for bit |
| 8 | Enlarging the evaluation region (§13) | The same result as with a one-column region |
| 9 | The fast profile, light, I/O | Differential test against the strict profile |

### What to keep in the tests forever

1. **The naive graph interpreter** behind the same interface as the tape. It
   remains the oracle for the life of the project.
2. **The single-threaded deterministic executor** for the scheduler. A divergence
   between it and the multi-threaded one is a race, not rounding.
3. **The strict precision profile**, even when the fast one ships (R1).
4. **Reference dumps with a source version check** (R5).
5. **The cell bound conservativeness test** (R6). The only optimisation in this
   document whose failure is invisible to every other test.

---

## Appendix: correspondences in Minecraft

For checking behaviour against, not for copying. Paths are relative to
`src/main/java/net/minecraft/`, version 26.3-pre-1.

| What | Where |
| --- | --- |
| Stages, dependencies, radii | `world/level/chunk/status/ChunkPyramid.java:14-52` |
| Radius accumulation down the chain | `world/level/chunk/status/ChunkStep.java` |
| Which stages go to the background pool | `world/level/chunk/status/ChunkStatusTasks.java:67, :89, :180, :216` |
| The sequential executors | `server/level/ChunkMap.java:200, :202` |
| Write footprint and the read footprint check | `server/level/WorldGenRegion.java:122, :277, :327` |
| Per-stage profiler instrumentation | `world/level/chunk/status/ChunkStep.java:34-36` |
| The field graph node interface | `world/level/levelgen/densityfunction/DensityFunction.java` |
| Graph compilation and rewrite rules | `densityfunction/DensityFunctionCompiler.java:17-31`, `DfRewriteRule.java:14` |
| Interpolation and lattice geometry | `densityfunction/op/InterpolatedFunction.java:114-126, :224-225` |
| Range choice | `densityfunction/op/RangeChoiceFunction.java:117-136` |
| Vertical surface search | `densityfunction/op/FindTopSurfaceFunction.java` |
| The surface-world terrain graph | `world/level/levelgen/NoiseRouterData.java` |
| The fluid level field | `world/level/levelgen/Aquifer.java:96-100, :158-172, :185, :465` |
| Fill and heightmaps during fill | `world/level/levelgen/NoiseBasedChunkGenerator.java:479-507` |
| Surface rules and the context | `world/level/levelgen/material/MaterialSystem.java`, `MaterialRuleContext.java` |
| The carving mask | `world/level/chunk/CarvingMask.java` |
| Trajectory pruning by reachability | `world/level/levelgen/carver/WorldCarver.java:83` |
| The carving source loop | `world/level/levelgen/NoiseBasedChunkGenerator.java:288-400` |
| Structure placement on a lattice | `levelgen/structure/placement/RandomSpreadStructurePlacement.java:84-92` |
| Terrain adaptation to structures | `world/level/levelgen/Beardifier.java:31, :226` |
| Topological sort of objects | `world/level/biome/FeatureSorter.java` |
| The decoration loop and structure materialisation | `world/level/chunk/ChunkGenerator.java:388-471, :422` |
| The placement modifier machine | `world/level/levelgen/placement/FeaturePlacer.java` |
| Biome classification and quantisation | `world/level/biome/Climate.java` |
| Batched climate sampling | `world/level/biome/MultiNoiseBiomeSource.java:70-115` |
| Sections, palette, counters | `world/level/chunk/LevelChunkSection.java:63-105`, `PalettedContainer.java` |
| Heightmaps | `world/level/levelgen/Heightmap.java` |
| The unseeded draw during spawning | `world/level/NaturalSpawner.java:455`, `world/level/Level.java:127` |
| Storage container codecs | `world/level/chunk/storage/RegionFileVersion.java:26-58` |

### Deliberate divergences from the reference

| Topic | Reference | Here | Reason |
| --- | --- | --- | --- |
| Structure placement | a pipeline stage, result stored in a column | a sparse index computed on demand | G1: a pure function of the lattice; removes radius 8 from four stages |
| Scattering stages | sequential executor, order from the player's route | colouring by footprints, order from the colour | C3 and W1: parallel and deterministic at once |
| Section storage | fixed array per column | sparse index, absence = air | V2: two thirds of the volume is never allocated |
| Range choice | both arms over the whole volume | exclusive cones and a jump in the schedule | D1 applied at run time, not only at compile time |
| Cell bounds | none | an interval over eight corners settles a whole cell | The convex hull lemma (§4) |
| Intermediate buffers | a full-volume buffer per node | tile-sized buffers in cache | T2: otherwise scaling degrades as cores are added |
| Rank reduction | slice nodes | a stratum with zero stride | E4: no copying at all |
| Carving region | mask per column | mask per region | K1: the union is idempotent, the result is exactly the same |
| Heightmaps | a separate pass from the chunk ceiling | the fused descent bounded by the ordering of predicates | `heightmap.md` §3–4. Only the `¬air` map is maintained during fill; the rest must wait for carving, which removes blocks |
| Fluid skip threshold | one per column from the surface maximum | per strip | A4: one peak otherwise denies the cheap path to the whole volume |
| Precision | one implementation | two profiles, strict as the oracle | §15: separates "computed differently" from "a different function" |
| Spawn draw | unseeded level-wide source | seeded from coordinates | St1: otherwise generation is not reproducible at all |
