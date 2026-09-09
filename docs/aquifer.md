# Aquifers: The Fluid Level Field, Its Bounds, and the Shell

A specification for a from-scratch implementation of the stage that decides
what fills the void: water, lava, air, or — near the boundary between two bodies
of water — stone. `worldgen.md` §7 states what the stage *is* (A1–A7); this
document specifies the 26.3 data it computes, the invariants an implementation
must hold, the bounds that let most of the volume be settled without the
per-block search, and how the rest is made cheap.

The reference is the authority on the data — the lattice, the constants, the
order of the surface samples, the pressure formula, the tie rule — and on
nothing else. The reference computes the field one block at a time with a
mutable flag left behind as a side effect; nothing of that shape is kept.

§11 says what to measure first.

---

## 1. What the stage computes

Fill (`worldgen.md` §7) has one question left after the density sign:

```
substance(p, d) = solid,          if d(p) > 0
                = solid,          if d(p) ≤ 0 and d(p) + barrier(p) > 0
                = type(p),        if d(p) ≤ 0 and y < level(p)
                = air,            otherwise
```

Both `level` and `barrier` come from one object, the **fluid level field**: a
Voronoi partition of space over a jittered lattice, each cell carrying a
*status* `(level, type)`, and a pressure term between neighbouring cells of
different status. Alongside the substance the stage yields one bit per placed
fluid block — whether the block must receive a fluid tick after generation —
which is how water in a cliff-side cave starts flowing. That bit is specified
here but not computed: nothing simulates fluids yet, and the field derives it
from the seed alone whenever it is wanted.

Where the dimension has no aquifer field (`noise_settings` without `aquifers`:
the Nether, the End, the `caves` and `floating_islands` presets) the field
degenerates to the **global rule**:

```
m       = min(−54, sea_level)
G(y)    = (−54, lava)                     if y < m
        = (sea_level, default_fluid)      otherwise
s.at(y) = s.type  if y < s.level,  air otherwise
```

**Q0.** The global rule has a lava floor in every dimension whose height reaches
below −54, aquifers or not. An implementation that writes
`y < sea_level ? fluid : air` is wrong for the `caves` preset and for the
overworld alike.

---

## 2. The lattice of centres

Space is cut into cells of **16 × 12 × 16** blocks. The cell of a block and the
centre of a cell:

```
g(p)  = ( ⌊(x − 5) / 16⌋,  ⌊(y + 1) / 12⌋,  ⌊(z − 5) / 16⌋ )
c(g)  = ( 16·gₓ + jₓ,  12·g_y + j_y,  16·g_z + j_z )
        jₓ ∈ [0, 10),  j_y ∈ [0, 9),  j_z ∈ [0, 10)   drawn in that order
```

The jitter is three bounded draws from a positional random source seeded from
the world's positional factory forked under the hash of `minecraft:aquifer`,
then positioned at `g` through the block-position hash. The offsets −5 and +1
in `g(p)` are part of the definition: they put the block's own cell and the one
after it, and the row above and below, into the candidate set.

**Q1. A centre is a pure function of the seed and the cell index.** Nothing
about it depends on the column being generated. Every cache of centres or
statuses is therefore legal at any scope, and a region of any size computes the
same field as a single column. This is what makes A7 of `worldgen.md` a
statement rather than a hope.

### The candidate set

For a block `p` the candidates are the twelve cells

```
C(p) = { g(p) + (δₓ, δ_y, δ_z) :  δₓ ∈ {0, 1},  δ_y ∈ {−1, 0, 1},  δ_z ∈ {0, 1} }
```

enumerated in exactly that nesting order: `δₓ` outermost, `δ_z` innermost.
Distance is squared Euclidean in integers, and four nearest are kept.

**Q2. The horizontal window is fixed per strip.** `gₓ` and `g_z` depend only on
`x` and `z`; within a strip only the row `g_y` moves, once every twelve blocks.
Everything a strip needs from the lattice is indexed by row.

**Q3. Ties go to the later candidate.** The reference inserts with `≥`, so a
candidate at the same distance as an already-kept one displaces it. A sorting
network reproduces that with one key: `key = 16·d² + (11 − i)` for the `i`-th
candidate in enumeration order; the smallest key wins, and equal distances
resolve to the larger `i`. The bound `d² ≤ 20² + 22² + 20² = 1284` puts the key
in fifteen bits.

**Q4. Similarity is an integer test.** The reference's
`σ(a, b) = 1 − (d_b − d_a)/25` is compared against `0` and against
`F = σ(100, 144) = −0.76`:

```
σ_ab > 0   ⟺   d_b − d_a < 25
σ_ab ≥ F   ⟺   d_b − d_a ≤ 44
```

Only the barrier product needs `σ` as a double, and there it must be computed
as the reference does — `1.0 − (d_b − d_a) / 25.0` — because it multiplies into
the `d + barrier > 0` decision.

---

## 3. The status of a cell

A status is `(level: i32, type: BlockState)`; two statuses are equal when both
fields are. The sentinel level `WAY_BELOW = −32512` (`MIN_Y << 4`) marks a dry
cell: `at(y)` is air for every `y`, but the sentinel is still a number that
enters the pressure formula.

The status of the cell whose centre is `c = (x, y, z)`:

**Step 1 — the surface samples.** `surfQ(x, z)` is the preliminary surface
(`worldgen.md` §6, the `surface_level` function of the aquifer config)
evaluated at the quart-aligned point `((x >> 2) << 2, 0, (z >> 2) << 2)` and
floored. The centre samples it at thirteen chunk offsets, **in this order**:

```
(0,0)  (−2,−1) (−1,−1) (0,−1) (1,−1)  (−3,0) (−2,0) (−1,0) (1,0)  (−2,1) (−1,1) (0,1) (1,1)
```

The set is asymmetric — three chunks west, one east — and the order decides
which early return fires. Both are data. For each offset `o`, with
`adj = surfQ + 8`, `top = y + 12`, `bottom = y − 12`:

```
if o = (0,0) and bottom > adj:                     return G(y)               (a)
if top > adj or o = (0,0):
    s = G(adj) ; if s.at(adj) ≠ air:
        if o = (0,0): under ← true
        if top > adj:                               return s                 (b)
lowest ← min(lowest, surfQ)
```

(a) is a centre more than twenty blocks above its own surface: no aquifer, the
global rule. (b) is a centre whose cell pokes above a surface that lies under
sea level: the sea. `under` records that the centre's own surface is under sea
level; `lowest` is the minimum raw (unadjusted) surface over all thirteen.

**Step 2 — floodedness.**

```
if exclusion(c) > 0:  partial = full = −1
else:
    below  = lowest + 8 − y
    factor = under ? clampedMap(below, 0, 64, 1, 0) : 0
    n      = clamp(floodedness(c), −1, 1)
    full   = n − map(factor, 1, 0, −0.3, 0.8)
    partial= n − map(factor, 1, 0, −0.8, 0.4)

level = G(y).level                                          if full > 0
      = min(lowest, 40·⌊y/40⌋ + 20 + 3·⌊(spread(⌊x/16⌋, ⌊y/40⌋, ⌊z/16⌋) ·ₛ 10) / 3⌋)
                                                            if partial > 0
      = WAY_BELOW                                           otherwise
```

`·ₛ` is a single-precision product: the reference multiplies the sampler's
`float` by `10.0F` before widening. The strict profile keeps that rounding.

**Step 3 — type.** `G(y).type`, except when `level ≤ −10`, `level ≠ WAY_BELOW`,
the global type is not lava, and `|lava(⌊x/64⌋, ⌊y/40⌋, ⌊z/64⌋)| > 0.3`: then
lava. This is what makes deep aquifers lava lakes.

### What follows from the definition

**Q5. `level ≤ max(sea_level, surfQ(c))` for every status.** Path (a) and the
`full` branch give `G.level ∈ {sea_level, −54}`; path (b) gives
`G(adj).level`, the same set; the `partial` branch is bounded by `lowest`, and
`lowest ≤ surfQ` at the centre's own offset, which is always sampled first.
This one inequality is what §5 builds on: a body of water cannot sit higher
than the preliminary surface above its own centre.

**Q6. Exclusion is a conjunction.** The overworld's exclusion is
`min(−0.225 − erosion, max(depth − 0.9, 0))`, and `min(a, max(b, 0)) > 0`
holds iff `a > 0 ∧ b > 0`. So `exclusion > 0 ⟺ erosion < −0.225 ∧ depth > 0.9`,
and `depth = gradient(y) + offset(x, z)`. Evaluate `erosion` first — one
shifted noise — and `offset` only where erosion passes; most centres never pay
for the splines. This is exact, not a heuristic: the compiler's D1 elimination
cannot see it because the intervals are not disjoint, but the algebra is
unconditional.

**Q7. Spread and lava are sampled on cell lattices, not at the centre.** The
spread noise takes `(⌊x/16⌋, ⌊y/40⌋, ⌊z/16⌋)`, and `⌊x/16⌋ = gₓ` exactly,
because the jitter never leaves `[0, 10)`. The lava noise takes
`(gₓ >> 2, ⌊y/40⌋, g_z >> 2)`. Both are shared across rows with the same
`⌊y/40⌋` and, for lava, across sixteen horizontal cells. They are memoised by
those integer keys, not by centre.

**Q8. The surface lookup is a table, never a map.** The thirteen offsets over
the whole lattice region sweep a known rectangle of quart points, fixed by the
region and the spacing (§6). The reference fills a hash map lazily; here the
rectangle is a flat array filled before any status is asked for.

---

## 4. The substance at a block

For a block `p = (x, y, z)` with density `d ≤ 0`:

```
if y < m:                                            → lava, flag = false   (L)
d₁ ≤ d₂ ≤ d₃ ≤ d₄, s₁..s₄     the four nearest by Q3
if d₂ − d₁ ≥ 25:                                     → s₁.at(y),
                                                       flag = (d₂ − d₁ ≤ 44) ∧ (s₁ ≠ s₂)
elif s₁.at(y) = water ∧ y = m:                       → water, flag = true
else:
    if d + σ₁₂·P(s₁,s₂) > 0                          → solid
    if d₃ − d₁ < 25 ∧ d + σ₁₂·σ₁₃·P(s₁,s₃) > 0       → solid
    if d₃ − d₂ < 25 ∧ d + σ₁₂·σ₂₃·P(s₂,s₃) > 0       → solid
    flow = (s₁≠s₂) ∨ (d₃−d₂ ≤ 44 ∧ s₂≠s₃) ∨ (d₃−d₁ ≤ 44 ∧ s₁≠s₃)
    flag = flow ∨ (d₃−d₁ ≤ 44 ∧ d₄−d₁ ≤ 44 ∧ s₁≠s₄)
                                                     → s₁.at(y)
```

The reference has one more branch, before (L): above a constant of the volume
it was built over it answers `G(y).at(y)` without a search:

```
maxSurf = max surfQ over every possible centre of the volume's lattice region
skipY   = 12·(⌊(maxSurf + 20)/12⌋ + 1) + 10          ≥ maxSurf + 31
```

Lemma K of §5 proves the branch changes nothing, and it is not implemented.

### Pressure

```
P(a, b):
    tₐ = a.at(y), t_b = b.at(y)
    if {tₐ, t_b} = {lava, water}:          return 2
    Δ = |a.level − b.level| ; if Δ = 0:     return 0
    h = y + 0.5 − (a.level + b.level)/2
    e = Δ/2 − |h|
    if h > 0:  c = e      ; g = c > 0 ? c/1.5 : c/2.5
    else:      c = 3 + e  ; g = c > 0 ? c/3   : c/10
    n = (−2 ≤ g ≤ 2) ? barrier(x, y, z) : 0
    return 2·(n + g)
```

`e` is the distance from the block to the nearer of the two levels, measured
inward: positive between them, negative outside. The gradient is steeper above
the pair than below, which is why a lake has a thin lid and a thick floor. The
barrier noise is read only where `|g| ≤ 2` and is the same value for all three
pressure calls at one block.

**Q9. The pressure is a function of `(y, a, b)` and one noise sample.** It has
no dependence on `x` and `z` except through the noise. Everything in §5 comes
from reading this formula as a function of `y` with the levels held fixed.

**Q10. Solid by barrier is solid.** The stone a barrier writes is
indistinguishable from stone the density wrote: it is the default block, it
enters the heightmaps, it is surfaced by the rules. The surface stage therefore
sees a lake shore as a stone lip a few blocks tall and paints it, which is the
intended look.

---

## 5. Window bounds: settling runs without the search

The search in §4 is per block: twelve distances, a selection, up to three
pressures, a noise. The reference runs it for every void block below its skip
constant — on hilly terrain, thirty to a hundred blocks of open air per strip.
Most of that work has an answer that does not depend on which candidate is
nearest.

> A **window** `W(g)` is the twelve statuses of `C(p)` for any `p` with
> `g(p) = g`. By Q2 a strip meets one window per row.

Four aggregates per window:

```
uniform(W)   all twelve statuses equal
Lmax(W)      max level over the twelve, WAY_BELOW if all are dry
Lmin(W)      min level over the twelve
oneType(W)   all twelve types equal and no level is WAY_BELOW
```

and one constant per world: `N`, the upper bound of the barrier node's interval
(`worldgen.md` D1). From it two margins:

```
m_above = min(5,  ⌈2.5·N − 0.5⌉)        m_below = min(24, ⌈10·N + 3.5⌉)
```

The pressure reads the noise only while `|g| ≤ 2`; past that the gradient alone
is negative, which is where the two caps come from. The overworld barrier is one
octave of amplitude 0.955, whose declared interval is `±1.91`, so the margins
are **5 and 23**. That interval is the six-sigma bound the compiler already
trusts for branch elimination and for the cell bounds; the margins inherit
exactly that trust, no more and no less.

**Lemma U — uniform.** If `uniform(W)` with status `s`, the substance at every
`p` in the window is `s.at(y)`. Pressure between equal statuses is zero by
`Δ = 0`, so no barrier term can flip the sign; every branch returns `s₁.at(y)`
and `s₁ = s`. The flag is false on every branch except one: at `y = m` with
`s.type = water`, the reference sets it when `d₂ − d₁ < 25`. That single row
per strip still needs the two nearest distances — integer work, no noise.

**Lemma A — above.** If `y ≥ Lmax(W) + m_above`, the substance is air. Every
level is `≤ Lmax < y`, so every `at(y)` is air and the lava-water case cannot
arise. For any pair with `Δ ≠ 0`: `h > 0`, `c = e = max − y − 0.5 ≤ −m_above −
0.5 < 0`, hence `g = c/2.5`, and either `g < −2` and the noise is never read, or
`g ≤ −N` and the noise cannot outweigh it; either way `2(n + g) ≤ 0` and
`d + σ·P ≤ d ≤ 0`. The block is not solid, and `s₁.at(y) = air`. The flag is
irrelevant for air.

**Lemma B — below.** If `oneType(W)` with type `T` and `y ≤ Lmin(W) −
m_below`, the substance is `T`. All `at(y)` equal `T`, so the lava-water case
cannot arise. For any pair with `Δ ≠ 0`: `h < 0`, `c = 3 + y + 0.5 − min ≤ 3.5
− m_below < 0`, hence `g = c/10`, and again either `g < −2` or `g ≤ −N`; the
pressure is non-positive and `d + σ·P ≤ 0`. The substance is `s₁.at(y) = T`.
**The flag is not settled** by
this lemma: it depends on `s₁..s₄` and the distance gaps, so the selection
still runs — integers only, no pressure, no noise, no density.

**Lemma K — the volume constant is redundant.** Above `skipY` the reference
answers `G(y).at(y)` without a search. Every candidate of such a block has
`c_y ≥ y − 22 ≥ maxSurf + 10`, and `maxSurf` bounds `surfQ` at every candidate
centre by construction of the region, so each candidate's top pokes above its
own adjusted surface. Two cases. If every candidate's own surface is under sea
level (`surfQ ≤ sea_level − 9`), each is the sea by path (b) at its first
sample — or `G(c_y)`, the same status, by path (a) — the window is uniform and
Lemma U gives `G(y).at(y)`. Otherwise some candidate has
`surfQ ≥ sea_level − 8`, hence `maxSurf ≥ sea_level − 8`, hence
`y ≥ sea_level + 24`; and `y ≥ surfQ(c) + 32` for every candidate; by Q5 the
block clears every level by more than `m_above`, Lemma A gives air, and
`G(y).at(y)` is air at that height. The two cases are exhaustive, so the
constant never changes an answer and is not implemented. Its one precondition:
`surfQ ≥ m − 8` everywhere, so that `G(adj)` at a sampled surface is never the
lava status; the overworld's preliminary surface cannot fall that low, and the
surface table asserts it once per region.

What the lemma says about the reference: its constant is a hand-derived margin
— eight plus twelve, rounded up to the row — that happens to be wide enough,
with the proof nowhere and an unused constant `MAX_REASONABLE_DISTANCE_TO_AQUIFER_CENTER`
left beside it. It is also computed from whatever volume the field was built
over, so the single-strip probe used for structure heights carries a different
`skipY` from the fill at the same position; by this lemma that difference is
unobservable, but only by this lemma. Lemma A is the same idea with the
margin derived from the data it depends on — the levels a window can hold and
the barrier's interval — and it is per strip, which is A4 of `worldgen.md`
made exact.

**Lemma L — the lava floor.** For `y < m` the substance is lava with no flag.
This is branch (L) of §4 and needs no window.

What is left after the four lemmas is the **shell**:

```
shell(W) = { y :  Lmin(W) − m_below < y < Lmax(W) + m_above }  ∖  uniform rows
```

The shell is where the search runs. It is a band around the levels present in
the window — five blocks above the highest, twenty-three below the lowest — and
it is empty wherever the window is uniform. Where a window holds a dry cell
next to a flooded one, `Lmin` is the sentinel and the shell reaches down to the
world floor: that is the stone wall between a lake and dry rock, and no lemma
can shorten it because the wall is real.

**Q11. The lemmas are exact rewrites, not approximations.** Each one restates
the outcome of §4 under a condition on the window. A block answered by a lemma
must produce the same substance, and where the lemma settles it, the same flag,
as the per-block search. That equality is a test (§12), not an assumption.

**Q12. The margins come from the interval, not from a constant.** `N` is read
from the compiled barrier node. A datapack that raises the barrier amplitude
widens the shell without a code change; hard-coding 5 and 23 would be a silent
error under such a datapack.

### Against the density cells

Fill settles a density cell — 4 × 8 × 4 here — from an interval over its eight
corners (`worldgen.md` L4). A void cell still needs a substance, and the
substance now depends on the fluid field.

A cell meets up to eight windows: `x − 5` crosses a multiple of sixteen in one
cell column out of four, `y + 1` crosses a multiple of twelve in two cell rows
out of three, likewise `z`. The cell's outcome is the intersection over the
windows it meets, evaluated over the cell's own `y` range:

| Windows agree on | Cell outcome |
| --- | --- |
| uniform, same status `s` | `s.type` for `y < s.level`, air above; flag by the `y = m` rule |
| Lemma A over `[y₀, y₁]` | air |
| Lemma B over `[y₀, y₁]`, same `T` | `T`; flags by the selection per block |
| otherwise | the cell is **not settled**: the density is evaluated per block and each block goes through the lemmas of its own window, then the search |

**Q13. A void cell is settled only when the fluid field is settled over it.**
The interval says the density is negative everywhere in the cell; it says
nothing about `d + barrier`. A shell cell must have its density, block by
block, because the search adds a pressure to it. Using the interval's upper
bound in place of the block's density there is the same mistake as L5 of
`worldgen.md`: stone through water, invisible to every test but one.

**Q14. Solid needs no window.** A cell settled solid by its interval is solid
whatever the fluid field says: pressure is only ever added when `d ≤ 0`.

---

## 6. Evaluating the field over a region

By Q1 everything below is a pure function of the seed, so the region may be
any size and the tables below are computed once per region task. The lattice
region for `W × W` columns starting at column `(cₓ, c_z)`:

```
gₓ ∈ [ ⌊(16cₓ − 5)/16⌋ ,  ⌊(16(cₓ + W) − 6)/16⌋ + 1 ]        W + 2 cells
g_y ∈ [ ⌊(minY + 1)/12⌋ − 1 ,  ⌊(maxY + 1)/12⌋ + 1 ]           35 rows for −64..320
g_z   likewise                                                   W + 2 cells
```

Centres lie in `[16·gₓ, 16·gₓ + 9]`; the thirteen offsets reach 48 blocks west
and 16 in the other directions; so the surface table covers, in quarts,

```
x:  ⌊(16·gₓ,min − 48)/4⌋ .. ⌊(16·gₓ,max + 25)/4⌋      4W + 23 columns
z:  ⌊(16·g_z,min − 16)/4⌋ .. ⌊(16·g_z,max + 25)/4⌋      4W + 15 rows
```

### Tables, in the order they are built

1. **The quart surface table** `surfQ`. Its interior — the `4W × 4W` quart
   points inside the region — coincides with the biome lattice (`worldgen.md`
   Cl2): the same climate values at the same points, already evaluated for
   biome classification. The preliminary surface adds the splines and the
   vertical search on top of them. The halo is evaluated pointwise. The halo
   is the cost of this stage at small `W`.

2. **Centres.** `(W + 2)² × 35` triples of draws into a flat array indexed
   `(g_y·sizeZ + g_z)·sizeX + gₓ`, the reference's order, so the twelve
   candidates of one window are two runs of two in `x` per row.

3. **Statuses, phase one.** Step 1 of §3 for every cell: thirteen table reads
   and integer compares. It classifies each cell as *global*, *sea*, or
   *pending*. Pending cells are the ones below or near a land surface; the sky
   is never pending.

4. **Statuses, phase two, lazy.** Step 2 and 3 for a pending cell, on the
   first window that needs it, recorded in a bitmap. Exclusion by Q6: erosion
   first, offset only where it passes. For centres inside the region's own
   columns, erosion and offset are read from the region's rank-XZ strata
   (`worldgen.md` E1) — the same function at the same block, kept live past
   the density fill for this purpose. Border centres, `1 − W²/(W+2)²` of them,
   are evaluated pointwise. Floodedness is pointwise per centre; spread and
   lava are memoised by their cell keys (Q7).

5. **Windows**, lazy per window, with the four aggregates of §5 and, when
   uniform, the status. `(W + 1)² × 33` records of sixteen bytes.

**Q15. Phase one is eager, phase two is lazy.** The reference resolves a cell
only when it becomes one of the four nearest; a cell deep in solid rock is
never asked for. Phase one costs thirteen loads per cell and is cheaper than a
bitmap test; phase two costs a shifted noise and sometimes the splines, and
half the underground cells are never asked. Eager phase two would evaluate
erosion at every underground centre for nothing.

**Q16. Nothing here is a hash map.** Every table is a dense array over a
rectangle known before the first lookup. The reference's `Long2IntOpenHashMap`
for the surface and its lazily-filled location array are what A6 of
`worldgen.md` forbids.

### Scope

At `W = 1` the surface halo is 513 quart points per column against 16 in the
interior; at `W = 8` it is 40 per column. The centres and statuses divide the
same way, `(W + 2)²/W²`. Region scope is not an optimisation of this stage; it
is the condition under which the stage is affordable. A surface tile cache
shared across region tasks would remove the halo altogether at the price of
synchronisation; it is not prescribed, and it is what to try if the halo is
still visible in the profile at the chosen `W`.

---

## 7. The per-block search, made cheap

Inside the shell the search of §4 runs. What makes it cheap is not skipping any
of it but arranging it so that a run of blocks in one strip and one row shares
everything but `y`.

**Per strip and row**, before the run: the twelve candidates' horizontal
distances `hₖ = dxₖ² + dzₖ²` and centre heights `cyₖ`, and the twelve status
indices. Twelve of each; the row changes every twelve blocks.

**Per block**, over the run in `y`:

1. `keyₖ = 16·(hₖ + (cyₖ − y)²) + (11 − k)` — twelve sixteen-bit lanes.
2. Four smallest keys with their lane index: four rounds of horizontal
   minimum, mask out, repeat; or a fixed partial sorting network. Both are
   branchless. Across a run, lanes can instead be the `y` values — eight blocks
   at a time — with the candidates as the outer loop and four running minima
   updated by compare-and-blend: twelve candidates times four slots.
3. The integer gates of §4 on `d₂ − d₁`, `d₃ − d₁`, `d₃ − d₂`, `d₄ − d₁`.
4. Pressures only on the branch that reaches them, in the reference's double
   arithmetic, with the barrier noise read once per block and only where
   `|g| ≤ 2`. The gate `|g| ≤ 2` is a band of at most eight blocks around each
   level of the pair, so across a run the noise is needed in short intervals,
   and those intervals are known before the run from the levels alone.
5. The flag from the same comparisons; a set flag appends the block to the
   column's fluid-tick list.

**Q17. The search vectorises across `y`, not across candidates.** Twelve is an
awkward width and the selection is a reduction; eight consecutive blocks with
the same candidate set are a natural width and the selection becomes
compare-and-blend. The shell is a band in `y` (§5), so runs are long enough.

**Q18. The strict profile keeps the reference's arithmetic.** `σ` as
`1.0 − (Δd)/25.0`; the pressure exactly as written, in the order written;
`d` widened from the fill's `f32` to `f64` before the sum. The sign test
`d + σ·P > 0` is where a reassociation changes a block, and the fast profile of
`worldgen.md` §15 may reassociate it; the strict one may not.

---

## 8. Carving and the surface stage

A carved position asks the field with `d = 0` (`worldgen.md` §9). Every lemma
and the search apply unchanged; with `d = 0` a positive barrier alone makes
stone, which is how a carved tunnel through a lake shore stays walled. The
window tables are the same tables — the carver runs on the same region task, or
recomputes them by Q1 if it does not.

The reference re-runs the surface rules at one block when a carver opens a
cavity under grass; `surface.md` §1 records that this is not implemented. The
flag from a carved fluid block goes to the same tick list as one from the fill.

The surface stage reads nothing from the field. It reads blocks, and the
stone a barrier wrote is stone to it (Q10).

---

## 9. What is truth, what is derived, what enters the ECS

`worldgen.md` X1 stands: generation runs outside the ECS and only results
enter. The classification below is what decides which results those are.

| Value | Category | Owner | Lifetime |
| --- | --- | --- | --- |
| Seed; `aquifers` config; the six density functions | truth | the loaded `noise_settings` asset | the world |
| Compiled field: node ids of the six functions, the positional seed pair, `N`, `m`, `G` | truth, compiled once | the `NoiseRouter` resource, cloned into the dimension sub-app with the other immutable registries | the dimension |
| `surfQ`, centres, statuses, windows | derived, materialised | the region task; buffers pooled and reused | one region task |
| Blocks written by the stage | projection of the seed, then truth after the first edit | the section arena | as `worldgen.md` §16 |
| Fluid-tick positions | **temporal fact** | one component on the column entity | until the column enters simulation |

**Q19. There is no aquifer object.** The reference's `Aquifer` carries a
`shouldScheduleFluidUpdate` field that `computeSubstance` writes as a side
effect and the caller reads afterwards. That is the reference's object graph
leaking in its purest form: a result smuggled through mutable state. Here the stage returns a value — substance and flag together — and the
tables of §6 are plain arrays owned by the task. Nothing about the field is a
component, a resource with interior mutability, or an object with an `update`
method.

**Q20. The fluid-tick list is a temporal fact, and it is stored.** It says
"generation decided this block must tick once", which current block state
cannot reconstruct without rerunning the field, and it must survive until the
column becomes live, which may be many ticks later, and across a save in
between — the reference persists it as the chunk's post-processing list. So it
is neither a `Message` (would not survive) nor derived on read (would rerun the
field). It is one component, written once by the system that applies a
finished generation result to the column entity, read and removed by the
system that promotes the column into simulation. Two systems, one direction,
no churn: the component appears once and disappears once per column, which is
what X9 of `worldgen.md` allows a marker-like component to do.

**Q21. The disabled field is the same interface, not a second path.** A
dimension without `aquifers` has every window uniform with status `G` and
Lemma U answers every block. Two code paths that must agree on the lava floor
(Q0) is how the current fill and carver came to disagree with the reference in
the same way.

---

## 11. Determinism

The field is a pure function of the seed (Q1) and the tables are exact
re-evaluations of it, so region size, task order and thread count are
unobservable. Three places need care.

**The tie rule (Q3).** A different tie order is a different nearest centre, a
different status, a different world along every cell boundary. It is in the
key.

**The sample order (§3).** Which of the thirteen offsets returns first decides
between the global status and the sea status where their levels differ — that
is, where `adj < −54`, which the overworld cannot reach but a datapack can. The
order is reproduced, not sorted.

**The double arithmetic of the shell (Q18).** The one place where the fast
profile may diverge, and by `worldgen.md` R2 the divergence is measured, not
estimated.
---

## 12. Tests to keep

1. **The naive port.** §3 and §4 transcribed as a pointwise function, kept
   behind the same interface as the tables, for the life of the project. It is
   the oracle for everything below.
2. **Lemma parity.** For every block a lemma answers, the oracle agrees on
   the substance. Run over several seeds
   and over a coast, a mountain and a deep-dark region. This is the R6 test of
   `worldgen.md` for this stage: a wrong margin writes water through stone and
   nothing else notices.
3. **Reference dump.** The existing surface-parity test tolerates one percent
   of positions differing and attributes them to the missing field. Once the
   field exists that tolerance is zero and the attribution is deleted.
4. **The margin under a datapack.** A barrier with doubled amplitude must widen
   the shell and still pass test 2.

---

## 13. What not to do

**Skip the sky by a volume-wide constant.** One peak denies the cheap path
to every strip, and the margin has to be proven for every volume the field is
built over; Lemma A is per strip and proves itself (A4, K). **Settle a void cell from its
density interval alone.** The barrier is
added to the density, not to the bound (Q13). **Apply Lemma B without
`oneType`.** A dry cell in the window puts a wall all the way down. **Take the
margins as constants.** They are a function of the barrier's interval (Q12).
**Evaluate exclusion as written.** It is a conjunction, and the cheap half goes
first (Q6). **Sample spread or lava at the centre.** They are sampled on
coarser lattices, and the lattice coordinate of a centre is the cell index
(Q7). **Keep a hash map of surface samples** or of centres (Q16). **Compute the
field per column when a region is available.** The halo is the cost (§6).
**Return the flag through a field.** It is half of the result (Q19). **Write
`y < sea_level` for the global rule.** There is a lava floor (Q0).
**Re-order the thirteen samples or resolve ties the other way.** That is a
different world (§11). **Vectorise across the twelve candidates.** Across `y`
(Q17). **Skip the selection where Lemma B settles the substance.** The flag
still needs it.

---

## Appendix: correspondences in the reference

For checking against, not for copying. Paths are relative to
`src/main/java/net/minecraft/`, version 26.3.

| What | Where |
| --- | --- |
| Lattice spacing, jitter, anchor offsets | `world/level/levelgen/Aquifer.java:92-113, :260-262, :286-290, :482-504` |
| The lattice region of a column | `Aquifer.java:162-175` |
| The column-wide skip constant | `Aquifer.java:176-185, :251-253` |
| Surface quantisation and the hash map | `Aquifer.java:188-196` |
| The batched surface sample | `Aquifer.java:198-233` |
| Candidate loop, tie rule, four nearest | `Aquifer.java:272-325` |
| Similarity and its threshold | `Aquifer.java:104, :412-415` |
| The decision tree and the flag | `Aquifer.java:242-405` |
| Pressure | `Aquifer.java:417-480` |
| The thirteen surface offsets and their order | `Aquifer.java:132-146` |
| Status: surface phase | `Aquifer.java:520-561` |
| Status: floodedness and level | `Aquifer.java:567-623` |
| Status: type | `Aquifer.java:625-644` |
| The sentinel | `world/level/dimension/DimensionType.java:50-52` |
| The global rule | `world/level/levelgen/NoiseBasedChunkGenerator.java:81-95` |
| The disabled field | `Aquifer.java:27-41` |
| Fill and the flag's consumer | `NoiseBasedChunkGenerator.java:479-513` |
| Carving with `d = 0` | `NoiseBasedChunkGenerator.java:345-372` |
| The positional factory under `minecraft:aquifer` | `world/level/levelgen/RandomState.java:78, :148-149`, `world/level/levelgen/NoiseChunk.java:45-55` |
| Positional seeding and the bounded draw | `world/level/levelgen/XoroshiroRandomSource.java:56-75, :114-128`, `util/Mth.java:367-371` |
| The overworld config | `world/level/levelgen/NoiseRouterData.java:521-545`; `worldgen/noise_settings/overworld.json` |
| The exclusion function | `world/level/biome/OverworldBiomeBuilder.java:1376-1381` |

### Deliberate divergences from the reference

| Topic | Reference | Here | Reason |
| --- | --- | --- | --- |
| Skip above the surface | one constant per volume, and the search for every void block below it | none; Lemma A per strip | Lemma K: the constant is redundant, and its margin is hand-derived where Lemma A's follows from the data |
| The sky band | searched block by block | settled by Lemma A | §5 |
| Runs inside a body of water or dry rock | searched block by block | Lemmas U and B | §5 |
| Cells | resolved on first use, location array filled lazily, surface in a hash map | dense tables over a known rectangle, surface phase eager, noise phase lazy | Q8, Q15, Q16 |
| Scope | the column | the region | Q1, §6 |
| Exclusion | evaluated as one function | erosion first, offset only past it | Q6 |
| Spread and lava noise | sampled per status | memoised by cell key | Q7 |
| The flag | a mutable field read after the call | half of the returned value | Q19 |
| The fluid-tick list | a per-section short list in the proto-chunk | one component on the column entity, written once, removed once | Q20 |
| The margins | none; the search decides | from the barrier node's interval | Q12 |
