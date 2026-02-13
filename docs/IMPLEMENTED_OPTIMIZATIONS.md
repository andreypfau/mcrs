# Implemented Optimizations in MCRS Worldgen

This document catalogs all optimizations currently implemented in the
`mcrs_minecraft_worldgen` density function engine and the chunk generation
pipeline. Each section describes the optimization, where it lives in the
codebase, and its measured or estimated impact.

**Baseline reference**: vanilla Minecraft 1.20.1 uses a recursive tree of
`DensityFunction` objects with per-node caching wrappers and virtual dispatch.
MCRS replaces the entire evaluation model.

---

## 1. Flat Stack Linearization

**File**: `density_function/mod.rs` — `build_functions()`, `DensityFunctionComponent`

Instead of a recursive tree of boxed trait objects, all density functions are
compiled into a flat `Vec<DensityFunctionComponent>` (the "stack"). Each entry
stores its operation and the *indices* of its inputs within the same vector.

Evaluation is a single forward loop:

```rust
for i in 0..=root {
    cache.scratch[i] = stack[i].sample_cached(&cache.scratch, &stack, pos);
}
```

**Impact**: Eliminates virtual dispatch, pointer chasing, and recursive call
overhead. Data locality is maximized — the entire computation graph lives in
contiguous memory.

---

## 2. Forward Evaluation with `sample_cached`

**File**: `density_function/mod.rs` — `sample_cached()` on each component

Every stack entry reads its inputs from a flat `scratch: Vec<f32>` buffer by
index. Because the stack is topologically sorted, all inputs are already
computed when an entry is evaluated. No tracing, no memoization hash maps —
just array reads.

```rust
fn sample_cached(&self, scratch: &[f32], stack: &[DensityFunctionComponent], pos: IVec3) -> f32
```

**Impact**: O(1) input lookup per dependency. No HashMap overhead that vanilla's
caching wrappers use.

---

## 3. `optimize_stack` — 11 Peephole Optimization Passes

**File**: `density_function/mod.rs` — `optimize_stack()`

A single forward pass over the stack applies 11 peephole optimizations:

### 3.1 Cache Wrapper Elimination
Removes `CacheOnce` and `CacheAllInCell` wrappers by redirecting their
consumers directly to the wrapped input. These caches are unnecessary because
the flat stack already evaluates each entry exactly once per forward sweep.

`FlatCache` and `Cache2d` are kept — they serve as column-caching barriers.

### 3.2 Binary Constant Folding
When both inputs to a `Binary` operation (Add, Mul, Min, Max) are constants,
replaces the node with a precomputed `Constant`.

### 3.3 Binary-to-Linear Demotion
When one input to a `Binary(Add/Mul)` is constant, demotes to a cheaper
`Linear { input, scale, offset }` node:
- `Add(x, c)` → `Linear { scale: 1.0, offset: c }`
- `Mul(x, c)` → `Linear { scale: c, offset: 0.0 }`

### 3.4 Min/Max Range Elimination
When one input to `Min` or `Max` can be statically proven to always dominate
(via min/max value ranges), replaces the binary op with its dominant input.

### 3.5 Single-Input Constant Folding
`Unary`, `Clamp`, and `RangeChoice` operations on constant inputs are folded
to constants.

### 3.6 Affine Fusion (12 patterns)
Detects chains of `Linear` and `Affine` operations and fuses them into a
single `Affine { input, scale, offset }`:

- `Linear(Linear(x))` → `Affine(x, s1*s2, s1*o2+o1)`
- `Linear(Affine(x))` → `Affine(x, ...)`
- `Add(Affine, const)` → `Affine(x, s, o+c)`
- `Mul(Affine, const)` → `Affine(x, s*c, o*c)`
- `Add(Linear, const)` → `Affine(x, s, o+c)`
- `Mul(Linear, const)` → `Affine(x, s*c, o*c)`
- And 6 more symmetric/nested patterns

### 3.7 Linear-to-Affine Promotion
Plain `Linear` nodes are promoted to `Affine` for uniform handling (one
multiply-add instead of branching on offset presence).

### 3.8 Identity and Zero Elimination
- `Add(x, 0)` → redirect to `x`
- `Mul(x, 1)` → redirect to `x`
- `Mul(x, 0)` → `Constant(0)`
- `Affine(x, 1.0, 0.0)` → redirect to `x`

### 3.9 Clamp Elimination
When an input's value range already fits within the clamp bounds, the `Clamp`
node is replaced with a redirect to its input.

### 3.10 RangeChoice Range Elimination
When the `when_in_range` input's min/max bounds guarantee it always (or never)
falls within the range, the branch is statically resolved.

### 3.11 Slide Fusion
Detects the 5-node pattern:
```
Affine(+c) ← Mul(ygrad2, Affine(+b) ← Mul(ygrad1, Affine(+a, input)))
```
and fuses it into a single `Slide` node with:
- Two Y-gradient parameters (`grad1`, `grad2`)
- Three offsets (`offset_a`, `offset_b`, `offset_c`)
- A fast-path Y range where both gradients saturate to 1.0, reducing to
  `input + combined_offset`

### Additional Passes
- **PiecewiseAffine fusion**: Detects `Unary` operations on `Affine` inputs
  (specifically HalfNeg/QuarterNeg) and fuses them into a `PiecewiseAffine`
  node that applies the affine transform and piecewise scaling in one step.
- **Binary same-index identity**: `Mul(x, x)` → `Square(x)` with correct
  range computation.

**Measured output** (overworld, typical): ~30 affine fusions, ~4 piecewise
affine fusions, ~15 constants folded, ~40 caches eliminated, ~5 identities
eliminated, ~10 binary demotions, 2 slide fusions.

---

## 4. Custom Fused Node Types

**File**: `density_function/mod.rs`

### 4.1 `Affine`
```rust
struct Affine { input_index, scale, offset, min_value, max_value }
```
Single fused multiply-add: `input * scale + offset`. Replaces chains of
Linear/Add/Mul operations.

### 4.2 `PiecewiseAffine`
```rust
struct PiecewiseAffine { input_index, scale, offset, neg_scale, min_value, max_value }
```
Affine transform followed by piecewise negative scaling (HalfNeg or
QuarterNeg). One node instead of two.

### 4.3 `Linear`
```rust
struct Linear { input_index, scale, offset, min_value, max_value }
```
Intermediate representation for `Binary(Add/Mul, x, const)` demotion.
Most are further promoted to `Affine`.

### 4.4 `Slide`
```rust
struct Slide {
    input_index, grad1, grad2,
    offset_a, offset_b, offset_c, combined_offset,
    fast_path_min_y, fast_path_max_y,
    min_value, max_value,
}
```
Fuses the 5-node slide chain (two Y-gradients + three affine offsets).
Fast path: for interior Y (both gradients = 1.0), returns
`input + combined_offset`.

### 4.5 `FlattenedSpline` (prepared, disabled)
```rust
struct FlattenedSpline {
    coord_indices, coord_min, coord_inv_range,
    grid_sizes, strides, lut: Box<[f32]>,
    min_value, max_value,
}
```
Pre-samples multi-coordinate splines into a 3D lookup table with monotone
cubic Hermite interpolation. Currently commented out in `optimize_stack`
Phase 3 pending accuracy validation.

---

## 5. Zone-Based Stack Reordering

**File**: `density_function/mod.rs` — `reorder_stack_for_evaluation()`

After optimization, the stack is reordered into three zones:

| Zone | Range | Contents | Evaluation |
|------|-------|----------|------------|
| A | `[0..column_boundary)` | Column-only entries for `final_density` | Once per (X,Z) at Y=0 |
| B | `[column_boundary..fd_boundary)` | Per-Y entries for `final_density` | Every (X,Y,Z) |
| C | `[fd_boundary..n)` | Other roots (aquifer, veins, etc.) | On demand |

This reordering is determined by `compute_per_block()`, which propagates
Y-dependency forward through the graph. `FlatCache`/`Cache2d` wrappers act
as barriers, forcing their outputs to be column-only regardless of inputs.

**Impact**: Zone B evaluations (the hot inner loop) never touch Zone A or
Zone C entries, eliminating the `column_changed` branch from the critical
path.

---

## 6. `evaluate_forward` Zone-Based Dispatch

**File**: `density_function/mod.rs` — `evaluate_forward()`

The main evaluation entry point dispatches based on which zone the requested
root falls in:

- **Zone A root**: Only evaluates `0..=root` at Y=0 when column changes
- **Zone B root** (final_density): Evaluates Zone A at Y=0 on column change
  (branchless), then Zone B at actual Y (branchless — no `per_block` checks
  since all Zone B entries are per_block by construction)
- **Zone C root**: Full forward sweep with `per_block` checks

**Impact**: Zone B evaluation is ~40% of the stack but runs without any
conditional per-entry branching.

---

## 7. `ChunkColumnCache` — Pre-populated Column Cache

**File**: `density_function/mod.rs` — `ChunkColumnCache`,
`populate_columns()`, `final_density_from_column_cache()`

For chunk generation (as opposed to single-point queries), Zone A is
pre-populated for all 17x17 (289) XZ corner positions of the interpolation
grid in a single pass:

```rust
let mut column_cache = router.new_column_cache(block_x, block_z);
router.populate_columns(&mut column_cache);
```

The cache stores Zone A results in a flat `column_data` buffer indexed by
`(grid_position * zone_a_count + entry_index)`. Column switching is a simple
pointer offset via `load_column(local_x, local_z)`.

Zone B entries are then evaluated using `final_density_from_column_cache()`
which reads Zone A values from the pre-populated cache instead of
re-evaluating them.

**Impact**: Each Zone A entry is evaluated exactly 289 times total (one per
XZ corner), shared across all Y levels. Without this, Zone A would be
re-evaluated at every corner of every cell of every section.

---

## 8. `SectionInterpolator` — Trilinear Interpolation

**File**: `density_function/mod.rs` — `SectionInterpolator`

Samples `final_density` only at cell corners (5x3x5 grid = 75 points per
section) and trilinearly interpolates all 4,096 interior block positions.

Cell dimensions: 4x8x4 blocks (h_cell_blocks=4, v_cell_blocks=8).
Grid: h_cells=4, v_cells=2 → 5 X-planes, 3 Y-levels, 5 Z-levels = 75 corners.

The interpolator maintains two Y-Z plane buffers (`start_buf`, `end_buf`)
and sweeps along X, swapping buffers between cells:

```
interpolate_y(delta_y)  // 8 corners → 4 values
interpolate_x(delta_x)  // 4 values → 2 values
interpolate_z(delta_z)  // 2 values → 1 result
```

**Impact**: Reduces expensive density evaluations from 4,096 to 75 per
16x16x16 section — a 54.6x reduction.

---

## 9. Y-Boundary Sharing Across Adjacent Sections

**File**: `density_function/mod.rs` — `fill_plane_cached_reuse()`,
`end_section()`

Adjacent Y-sections share cell corners at their boundary. The top Y-row of
section *s* (at Y = section_top) is identical to the bottom Y-row of section
*s+1* (at Y = section_bottom). Rather than re-evaluating these 25 corners,
they are saved in a `saved_top_y` buffer and restored for the next section.

```rust
pub fn fill_plane_cached_reuse(&mut self, plane_seq, is_start, x, base_y, base_z, router, column_cache) {
    // For each Z corner:
    //   if section_boundary_valid: reuse saved top-Y instead of evaluating
    //   evaluate remaining Y corners
    //   save the new top-Y value
}
```

After processing a section, `end_section()` marks the boundary as valid.

**Impact**: Saves 25 density evaluations per section for all sections after
the first. Over the full Y range (-64..320 = 24 sections), this saves
`25 * 23 = 575` evaluations out of `75 * 24 = 1,800` total — a 32% reduction.

**Benchmark**: 2.42ms → 1.67ms per chunk column (31% faster, 45% more
throughput).

---

## 10. `corners_uniform_sign` — All-Solid/All-Air Fast Path

**File**: `density_function/mod.rs` — `corners_uniform_sign()`
**File**: `world/generate/mod.rs` — `generate_section()`

After sampling the 8 corners of a cell, checks if all corners agree on sign:
- All positive → entire cell is solid, fill with stone without interpolation
- All non-positive → entire cell is air, skip entirely
- Mixed → proceed with full trilinear interpolation

```rust
match interp.corners_uniform_sign() {
    Some(true)  => { /* fill 4*8*4 = 128 blocks with stone */ }
    Some(false) => { /* skip entirely */ }
    None        => { /* trilinear interpolation */ }
}
```

**Impact**: Skips the 128-block inner interpolation loop for uniform cells.
In typical overworld terrain, ~70-80% of cells are uniform (mostly air above
surface, solid below deep underground).

---

## 11. Thread-Safe Architecture

**File**: `density_function/mod.rs` — `NoiseRouter`, `DensityCache`,
`ChunkColumnCache`

The `NoiseRouter` is fully immutable after construction (no `&mut self`
methods). All mutable state lives in per-thread caches:

- `DensityCache` — scratch buffer + column tracking for `evaluate_forward`
- `ChunkColumnCache` — pre-populated Zone A values for 17x17 grid
- `SectionInterpolator` — Y-Z plane buffers for trilinear interpolation

This allows sharing one `NoiseRouter` across all chunk generation threads
(via `Arc<NoiseRouter>` or `&NoiseRouter`) without synchronization.

**Impact**: Enables embarrassingly parallel chunk generation. The benchmark
generates 441 chunks single-threaded; the server uses 4+ threads for
near-linear throughput scaling.

---

## Performance Summary

| Metric | Value |
|--------|-------|
| Release performance | ~1.67ms per chunk column |
| Throughput (single-threaded) | ~597 columns/sec |
| Density evaluations per column (with Y-boundary) | ~1,225 |
| Trilinear reduction factor | 54.6x (4,096 → 75 per section) |
| Y-boundary savings | 32% fewer evaluations |
| Stack size (overworld) | ~120 entries (post-optimization) |
| Zone A (column-only) | ~50 entries |
| Zone B (per-Y, final_density) | ~40 entries |
| Zone C (other roots) | ~30 entries |
