# Worldgen column generation: measured baseline

All numbers below were taken on 2026-08-26 at commit 78c7417f with the harness
`crates/mcrs_minecraft/src/world/generate/tests/bench_columns.rs`. Nothing here
is estimated or carried over from an earlier document; anything not measured is
marked as such.

This file replaces `IMPLEMENTED_OPTIMIZATIONS.md`, which described types that no
longer exist (`SectionInterpolator`, `ChunkColumnCache`, `FlattenedSpline`), a
`surface-skip` feature that was removed in d4ac6c77, and a `examples/bench_worldgen.rs`
harness that was deleted in the same commit. Its headline figure of 0.942 ms per
column came from a benchmark with its own private `generate_column` that skipped
`precompute_column_grid` and never wrote a block palette, so it is not comparable
to anything measured here.

## Machine and build

| | |
|---|---|
| CPU | Apple M4 Max, 16 cores (12 performance + 4 efficiency) |
| OS | macOS, Darwin 25.6.0 |
| Toolchain | rustc stable, aarch64-apple-darwin |
| Profile | `--release`; the workspace defines no `[profile.release]`, so opt-level 3, no LTO, 16 codegen units, no debug info |
| Features | `mcrs_minecraft_worldgen` defaults: `serde`, `bevy`, `lazy-range-choice`, `batch-noise` |

The machine was **not** idle. A Steam helper and a CrossOver `wineserver` held
roughly one core for the whole session, and other `cargo` builds in the same
repository intermittently saturated every core. Runs taken during that
contention are reported separately below; they are the reason this file quotes
medians rather than means.

## How to reproduce

```
cargo test -p mcrs_minecraft --release --lib bench_overworld_columns -- --ignored --nocapture
```

`MCRS_BENCH_COLS` (default 64) sets the number of columns per repetition and
`MCRS_BENCH_REPS` (default 5) the number of repetitions. Run the overworld and
beta benchmarks in **separate invocations**, or pass `--test-threads=1`: cargo
runs both tests concurrently by default, and two single-threaded benchmarks
sharing the machine inflate each other by roughly 5%.

Each column is the full dimension height, y −64..320, 24 sections, seed 845.
The harness calls the real `generate_column`, so `populate_columns`,
`precompute_column_grid`, the per-section plane fills and every `BlockPalette`
write are inside the timed region. Asset loading is strict: every density
function and noise JSON under `assets/minecraft/worldgen/` must parse, every
reference must resolve, and the block corpus must load, or the test panics.
(The previous version of this harness dropped any file that failed to parse and
any density function that was not a JSON object. Both are now hard errors. The
resulting graph is identical — all 65 density functions and 64 noises loaded
either way — so the fix changes no number, only the trust in them.)

## Overworld baseline

256 columns per repetition, three separate process invocations of three
repetitions each, taken while no build was running:

| | mean | median | p95 | min | max |
|---|---|---|---|---|---|
| range over 9 repetitions | 1.169 – 1.219 ms | 1.165 – 1.214 ms | 1.245 – 1.302 ms | 1.086 ms | 1.629 ms |

**Headline: 1.18 ms per column, ~845 columns/second, single-threaded.**

Run-to-run spread of the mean across process invocations is about 4%. Within a
repetition the distribution is tight — p95 sits 7% above the median — with
occasional single-column outliers up to 1.6 ms.

Under heavy contention (two `rustc` processes saturating the machine) the median
held at 1.22 – 1.27 ms while the mean rose to between 2.0 and 4.0 ms and p95 to
11 ms. The median is the number to track.

Sub-phase: `populate_columns` alone (zone A at the 25 grid corners) is
0.020 ms per column, 1.7% of the column.

## Beta baseline

Same configuration, `bench_beta_columns`, which takes the exact-f64 terrain path
rather than the f32 density stack:

| | mean | median | p95 |
|---|---|---|---|
| range over 9 repetitions | 0.597 – 0.720 ms | 0.579 – 0.692 ms | 0.713 – 0.963 ms |

## Is the engine noise-bound?

The design model claims roughly 149,000 Perlin octave evaluations per column at
about 6.3 ns each, which would be 0.94 ms and therefore essentially all of the
column time. **The claim does not survive measurement. Noise is about two thirds
of the time, not all of it.**

Two independent methods agree.

### Method 1 — sampling profile

`samply record --rate 4999` over 512 columns x 8 repetitions (~5.3 s of timed
work), built with `CARGO_PROFILE_RELEASE_DEBUG=1`, symbolicated afterwards with
`atos` against the release binary's debug map. 27,724 samples landed on the
benchmark thread. Self time by leaf frame, aggregated by symbol:

| share | category | dominant symbols |
|---|---|---|
| 65.6% | Perlin noise | `ImprovedNoise<f32>::sample_batch` 58.6%, `OctavePerlinNoise<f32>::get_batch` 4.7%, `NoiseSampler::get` 1.4%, `NoiseSampler::get_batch` 0.8% |
| 13.7% | density-graph arithmetic | `NoiseRouter::evaluate_plane_batch` 12.6%, `IndependentDensityFunction` 0.7% |
| 13.6% | palette writes | `PalettedContainer::from_cube` 4.7%, `HeterogeneousPaletteData::set` 4.3%, `PalettedContainer::fill_box` 3.8%, `PalettedContainer::set` 0.8% |
| 5.2% | system libraries | `libsystem_platform` 3.0% (memcpy/memset), `libsystem_kernel` 1.7% (page faults), `libsystem_malloc` 0.5% |
| 1.9% | everything else | block-definition setup, serde, `generate_section` glue |

The system-library time is almost entirely allocation and bulk copying driven by
the per-section palettes, so charging it to palette work puts that category
nearer 18% and noise nearer 66%.

### Method 2 — counting octaves

A temporary `AtomicU64` was incremented in `ImprovedNoise<f32>::sample_and_lerp`,
which is the single choke point through which every Perlin octave evaluation on
the f32 path passes (its only two callers are `sample` and `sample_2d`). The
counter has since been removed; the working tree contains no instrumentation.

**127,004 octave evaluations per column**, identical on every repetition. That is
15% below the model's 149,000.

Dividing the profile's noise share by the count gives the real per-octave cost:

```
0.656 x 1.18 ms / 127,004 = 6.09 ns per octave evaluation
```

The model's 6.3 ns per octave was very nearly right. What was wrong was the
count, and — decisively — the conclusion drawn from the product: 127,004 x 6.1 ns
is 0.77 ms, which is 66% of a 1.18 ms column, not 100% of it.

The two methods are independent (one measures where the program counter sits,
the other counts calls and divides), and they agree to within a percent.

**Consequence.** By Amdahl's law, driving Perlin evaluation to zero cost caps
out at a 2.9x speedup. The remaining third — density-graph arithmetic and
palette writes — has to be attacked separately, or it becomes the floor.

### Limits of the method

- The profiler perturbs. Under `samply` the mean rose from ~1.18 ms to ~1.30 ms,
  about 10%. The category shares are proportions and survive that; the absolute
  times in the profiled run are not the baseline.
- `sample_and_lerp` is `#[inline(always)]` and has no symbol of its own, so its
  cost is attributed to `sample_batch`. Symmetrically, anything the compiler
  inlined into `evaluate_plane_batch` is counted as arithmetic. If noise math was
  inlined there, the noise share is understated, not overstated.
- The counter itself costs something: 32.5 million relaxed `fetch_add`s per
  256-column repetition. No slowdown was resolvable above run-to-run noise, but
  a few percent could be hiding under it. The count is exact regardless.
- Leaf-frame self time only. There is no call-tree attribution, so a cost cannot
  be assigned to the caller that provoked it.
- Terrain-dependent. The 256 columns are chunks x 0..8, z 0..32 at seed 845.
  Different terrain changes how often the all-solid/all-air fast path fires and
  therefore how much palette work runs.

## The abs/square fix made this slower, correctly

Before d4ac6c77 the interval rule for `abs` and `square` collapsed any input
straddling zero to a point interval. That fed the min/max domination pass, which
deleted live arms: the noodle cave subtree and the `spaghetti_3d_2` branch were
absent from the built stack entirely. Both are present now.

Measured on this harness, same machine, same session, by temporarily restoring
the old rule and then reverting:

| | octave evals / column | median ms / column | block checksum |
|---|---|---|---|
| old rule (arms deleted) | 73,104 | 0.723 | `0x87fc55d271b00cc4` |
| corrected rule | 127,004 | 1.170 | `0xc767b2c5632ab782` |

1.74x the octave work for 1.62x the time. The checksums differ because the
terrain differs: the old build was generating a world without those caves. This
is not a regression — the engine was skipping work vanilla does, and now does it.

## Structural facts about the build that exists

Verified against the code, not inferred:

- The interpolator type is `NoiseCellInterpolator`, the column cache is
  `ColumnCache`. There is no `SectionInterpolator` and no `ChunkColumnCache`.
- `ColumnCache::GRID_SIDE` is 17, so 17x17 = 289 slots are **allocated**, but
  `populate_columns` steps by `h_cell_blocks` (4) and fills only the 5x5 = **25**
  cell-corner positions. The earlier claim that each zone A entry runs 289 times
  per column was wrong by a factor of 11.6.
- Overworld stack after optimization: 39 zone A (column-only) entries;
  `final_density` sits at index 172.
- Cell geometry: 4x8x4 blocks, so 5x3x5 = 75 corners per section before Y-boundary
  reuse.
- There is no `surface-skip` feature and no `estimate_max_surface_y`. Any table
  contrasting "with" and "without" it describes a build that cannot be produced.
- `FlattenedSpline` does not exist.

## Where the column ended up

The measurements above were taken before parity with the reference was closed.
Four changes landed after them, and the last one more than paid for the rest.

| change | overworld ms/col |
|---|---|
| starting point | 1.18 |
| noise coordinates in double | 1.43 |
| interpolation moved inside the wrapper | 1.46 |
| accumulation order matched to the reference | 1.46 |
| arm-exclusive branches skipped | **1.01** |

Beta went 0.60 to 0.53 over the same span and is unaffected by the branch
schedule: its zone B is thirteen nodes with nothing to skip.

Three of the four are corrections that cost time, and one of them is not visible
as a line in the table: fixing the abs/square interval rule restored the noodle
subtree and the spaghetti_3d_2 branch, which is 74% more octave work per column
than the engine was doing when the 1.18 was measured. So the column is doing far
more work than it was and still finishes 15% sooner.

The branch schedule is where all of the speed came from, and it is worth being
precise about why it beat its own prediction. The pruning experiment bounded a
selector over a range of Y and resolved 26.5% of sites, which forecast an 18.8%
saving. Evaluating the selector exactly at each position resolves every site,
and keeping that resolution per position through the batch rather than requiring
a whole batch to agree is what turned 18.8% into 37% of zone B octaves and 31%
of column time.

Two things measured and not taken:

The octave kernel does not want hand vectorising on this target. The compiler
already pairs the gradient products two lanes wide of its own accord, and
reshaping the source into explicit arrays emits a byte-identical instruction
histogram. Going wider needs eight gradients from a sixteen-entry table at
data-dependent indices, and aarch64 has no gather; a byte-table lookup was tried
and lost 13%, and its fused multiply-add changed 884790 of 2097152 results,
confirming that the blend tree cannot tolerate fusion.

Interval pruning of whole Y runs is not worth building. It proves 48-51% of
lattice rows air, but those are the cheap rows, the walk costs 19% of what it
saves, and stone is unprovable at any depth because the density ends in a
minimum against a cave term that reaches -2.2 everywhere in the column. Net 21
to 23%, against 31% already taken by branch skipping for far less machinery.
