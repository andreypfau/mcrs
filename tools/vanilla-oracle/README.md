# vanilla-oracle

Dumps vanilla Minecraft output so the Rust worldgen tests can compare against it
element by element: density-function values (`dumpOracle`) and the blocks a
chunk holds after the fill and the material rules (`dumpSurface`).

It runs **no server and no game client**. `VanillaOracle.main` calls
`SharedConstants.tryDetectVersion()`, `Bootstrap.bootStrap()` and
`VanillaRegistries.createWorldLookup()`, pulls `minecraft:overworld` out of
`Registries.NOISE_SETTINGS`, builds a `RandomState` for each seed and samples
every `NoiseRouter` root through vanilla's own
`DensitySamplerSet` / `DensityBuffer` / `DensityVolume`.

Standalone Gradle project: only Fabric Loom is used, and only to put the
mapped Minecraft jar on the classpath. Nothing is remapped, no mod is loaded,
no mixins are applied.

## Regenerating the dumps

```sh
cd tools/vanilla-oracle
./gradlew dumpOracle --console=plain \
    -PoracleOut=../../crates/mcrs_minecraft_worldgen/tests/fixtures/vanilla
```

Add `--no-daemon` if you hit Gradle lock contention. Never run two Gradle
invocations against this project at once. First run downloads the Minecraft jar
and its libraries; later runs take a few seconds.

Output is deterministic: re-running produces byte-identical files.

The Minecraft version comes from `gradle.properties`
(`minecraft_version=26.3-snapshot-10`). Change it there, not in the source.

## What is dumped

For `minecraft:overworld` at seeds 1, 2 and 42, and chunks
(0,0), (10,-7), (100,100), (-33,55), (7,7):

`overworld_s<seed>_c<chunkX>_<chunkZ>.bin` — all eight `NoiseRouter` roots
(`temperature`, `vegetation`, `continents`, `erosion`, `depth`, `ridges`,
`chunk_surface_level`, `final_density`, in `NoiseRouter` declaration order) over
the corner lattice
`DensityVolume(sizeX=5, sizeY=49, sizeZ=5, min=(chunkX*16, -64, chunkZ*16), step=(4, 8, 4))`
= 1225 values per root.

`overworld_s42_c0_0_dense.bin` — `final_density` alone over
`DensityVolume(16, 384, 16, min=(0, -64, 0), step=(1, 1, 1))` = 98304 values.
The dense volume exercises `InterpolatedFunction.sampleWithBlockStep`, so a
Rust implementation that gets interpolation scope wrong will disagree here even
when the corner lattice matches. At the corners the two volumes share, the
dense dump is bit-identical to the lattice dump.

Values are **f32**: vanilla's `DensityBuffer` is a `float[]` and
`DensitySampler.sampleValue` returns `float`.

Each root is sampled with a single `SamplerContext` per file, built with
`enableCaches()` — the same shape `NoiseChunk` uses for a real chunk. No
beardifier and no blender are installed, which matches a chunk with no
structures and no adjacent legacy terrain: `Beardifier.CONTEXT_KEY` being
absent makes vanilla fall back to a constant `0.0`.

## Binary layout

Little-endian throughout. `i32`/`u32` are 4 bytes, `i64` is 8, `f32` is 4
(IEEE-754, written from `Float.floatToRawIntBits`). A `str` is a `u32` byte
length followed by that many UTF-8 bytes, unterminated.

```
magic          8 bytes, ASCII "MCDFORCL"
format_version u32   currently 1
world_version  u32   SharedConstants.getCurrentVersion().dataVersion().version()
settings_id    str   e.g. "minecraft:overworld"
seed           i64
chunk_x        i32
chunk_z        i32
volume_count   u32

repeated volume_count times:
  name         str   density function root name, e.g. "final_density"
  size_x       i32
  size_y       i32
  size_z       i32
  min_block_x  i32
  min_block_y  i32
  min_block_z  i32
  step_block_x i32
  step_block_y i32
  step_block_z i32
  value_count  u32   == size_x * size_y * size_z
  values       f32 * value_count
```

`values` is vanilla's `DensityBuffer` verbatim, so the index of a lattice
position is `DensityVolume.indexUnchecked`:

```
index = y + (x + z * size_x) * size_y
```

and the block coordinate of `(x, y, z)` is
`(min_block_x + x * step_block_x, min_block_y + y * step_block_y, min_block_z + z * step_block_z)`.
No transposition is needed to compare against a Rust buffer that uses the same
order.

The file ends exactly at the last value; there is no trailer.

---

# Surface dumps

`SurfaceOracle.main` builds a `NoiseBasedChunkGenerator` over
`minecraft:overworld` and the overworld multi-noise preset, then runs
`buildTerrain` on a bare `ProtoChunk` and records every block. It still runs no
server: the chunk is created against a `PalettedContainerFactory` assembled from
the biome lookup, structures are stubbed out with a `StructureManager` whose
`startsForStructure` returns nothing (so the beardifier is `Beardifier.EMPTY`,
as for a chunk with no structures), the blender is `Blender.empty()`, and the
`BiomeManager` is the biome source's own uncached resolver under
`BiomeManager.obfuscateSeed(seed)` — the same values a `WorldGenRegion` would
read out of the neighbouring chunks' palettes.

`buildTerrain` also carves, so the task runs with `-DMC_DEBUG_ENABLED
-DMC_DEBUG_DISABLE_CARVERS` and `main` refuses to write a dump without them: the
Rust stage under test runs before carving.

```sh
cd tools/vanilla-oracle
./gradlew dumpSurface --console=plain \
    -PoracleOut=../../crates/mcrs_minecraft_server/src/world/generate/tests/fixtures
```

Two search modes pick the coordinates. `findBiomes` walks outward from the origin
for a chunk whose sixteen quart cells at y = 64 are all one biome, and
`findIcebergs` generates the chunks around those coordinates until one contains
packed ice. Both print what they find and write nothing.

## Dumped chunks

| File | What it covers |
| --- | --- |
| `surface_s42_c0_0.bin` | ocean floor: gravel, the water level |
| `surface_s42_c100_100.bin` | grass over dirt, an iron vein, lava at depth |
| `surface_s2_c3_-7.bin` | grass over deep dirt, an iron vein |
| `surface_s42_c-118_-119.bin` | eroded badlands: the clay bands and a pillar |
| `surface_s2_c19_-1.bin` | frozen ocean without an iceberg |
| `surface_s2_c52_36.bin` | deep frozen ocean, a copper vein |
| `surface_s2_c20_-3.bin` | frozen ocean with an iceberg |

## Binary layout

Little-endian, same primitives as the density dumps. Blocks are stored as a
palette of `BlockStateParser.serialize` strings plus a run-length encoding over
the whole chunk, which is what keeps a 98304-block chunk under 50 kB.

```
magic          8 bytes, ASCII "MCSURFC0"
format_version u32   currently 1
world_version  u32   SharedConstants.getCurrentVersion().dataVersion().version()
settings_id    str   "minecraft:overworld"
seed           i64
chunk_x        i32
chunk_z        i32
min_y          i32   -64
height         i32   384
palette_count  u32
palette        str * palette_count
run_count      u32
runs           (u32 palette index, u32 length) * run_count
```

The runs expand to one palette index per position, indexed
`((z * 16 + x) * height) + (y - min_y)`, and their lengths sum to
`16 * 16 * height`.

---

# Feature step dumps

`FeatureStepOracle.main` reproduces the ordering `ChunkGenerator` derives from a
biome source: `FeatureSorter.buildFeaturesPerStep(List.copyOf(biomeSource.possibleBiomes()),
b -> b.value().getGenerationSettings().features(), true)`, called for the three
shipped biome sources. It runs no server: `VanillaRegistries.createWorldLookup()`
supplies the biome, placed-feature and multi-noise-preset registries, exactly as
`SurfaceOracle` does.

```sh
cd tools/vanilla-oracle
./gradlew dumpFeatureSteps --console=plain \
    -PoracleOut=../../crates/mcrs_minecraft_worldgen/tests/fixtures/vanilla
```

One file, `feature_steps.bin`. Sources and what they resolve to:

| `source_id` | Biome source | Biomes | Steps | Features per step |
| --- | --- | --- | --- | --- |
| `minecraft:overworld` | `MultiNoiseBiomeSource.createFromPreset(OVERWORLD)` | 56 | 11 | 0, 4, 5, 4, 4, 0, 34, 7, 3, 109, 1 |
| `minecraft:the_nether` | `MultiNoiseBiomeSource.createFromPreset(NETHER)` | 5 | 10 | 0, 0, 1, 0, 3, 0, 0, 23, 0, 10 |
| `minecraft:the_end` | `TheEndBiomeSource.create(biomes)` | 5 | 11 | 1, 0, 0, 0, 2, 0, 0, 0, 0, 1, 1 |

## Binary layout

Little-endian, same primitives as the density dumps.

```
magic          8 bytes, ASCII "MCFSTEP0"
format_version u32   currently 1
world_version  u32   SharedConstants.getCurrentVersion().dataVersion().version()
source_count   u32

repeated source_count times:
  source_id    str
  biome_count  u32
  biome_ids    str * biome_count      possibleBiomes in List.copyOf order
  step_count   u32
  repeated step_count times:
    feature_count u32
    feature_ids   str * feature_count  the sorted order of that step
  repeated biome_count times:          outer loop is the biome
    repeated step_count times:         inner loop is the step
      byte_len u32                     == (feature_count of that step + 7) / 8
      bits     byte * byte_len
```

The membership bitset is indexed by the position of a placed feature inside that
step's `feature_ids` list — the same integer `StepFeatureData.indexMapping`
returns, and the integer `WorldgenRandom.setFeatureSeed` is called with. Bit `i`
is `bits[i >> 3] & (1 << (i & 7))`. A biome whose own `features()` list is
shorter than `step_count` still gets an all-zero bitset for the missing steps, so
the framing is rectangular.

Ordering is the whole point of the fixture, so nothing about it is incidental:
`biome_ids` is `possibleBiomes()` iteration order, `feature_ids` is the reversed
DFS post-order over the `TreeMap` keyed by `(step, first-encounter index)`, and
`indexMapping` returns `-1` for a feature absent from a step (dumping throws
rather than writing such a bit).

The file ends exactly at the last bitset; there is no trailer.

---

# Ore vein dumps

`OreOracle.main` runs `OreFeature.place` over a world that is stone at every
in-world position, and records every block the vein writes, in write order, plus
the random state the call leaves behind. No registries and no level are built —
only `Bootstrap.bootStrap()`, for the block registry.

```sh
cd tools/vanilla-oracle
./gradlew dumpOreVeins --console=plain \
    -PoracleOut=../../crates/mcrs_minecraft_decoration/tests/fixtures/vanilla
```

One file, `ore_vein.bin`.

The cases, what each one pins, and the provenance map of the lifted code are
beside the fixture in
`crates/mcrs_minecraft_decoration/tests/fixtures/vanilla/capture_procedure.md`.

The random source is `new XoroshiroRandomSource(seed)`. `rng_after_lo` and
`rng_after_hi` are two `nextLong()` values taken immediately after `place`
returns; comparing them is what pins the draw count.

## Binary layout

Little-endian, same primitives as the density dumps.

```
magic          8 bytes, ASCII "MCOREVN0"
format_version u32   currently 1
world_version  u32   SharedConstants.getCurrentVersion().dataVersion().version()
case_count     u32

repeated case_count times:
  name              str
  seed              i64
  origin_x          i32
  origin_y          i32
  origin_z          i32
  size              i32
  discard_chance    f32
  target_count      u32
  repeated target_count times:
    rule_type       str   "block_match" | "random_block_match" | "always_true"
    rule_block      str   block id, "" for always_true
    rule_probability f32  0.0 unless random_block_match
    state           str   BlockStateParser.serialize of the ore state
  placed            u32   1 when doPlace returned true
  rng_after_lo      i64   random.nextLong() after place returned
  rng_after_hi      i64   the next one
  palette_count     u32
  palette           str * palette_count
  placement_count   u32
  placements        (i32 x, i32 y, i32 z, u32 palette index) * placement_count
```

`placements` is in the order `doPlace` wrote them, so it pins the loop order as
well as the set of positions.

---

# Tree geometry dumps

`TreeOracle.main` runs every shipped `tree` feature over a flat world and
records the blocks it writes and the random state it leaves behind. It runs no
server: `VanillaRegistries.createWorldLookup()` supplies the feature registry
and the overworld `DimensionType`, and the block tags the tree tests against are
bound from the vanilla data pack through `TagLoader.loadTagsForExistingRegistries`.

```sh
cd tools/vanilla-oracle
./gradlew dumpTrees --console=plain \
    -PoracleOut=../../crates/mcrs_minecraft_decoration/tests/fixtures/vanilla
```

One file, `tree_geometry.bin`: all 45 `minecraft:tree` features at seeds 42, 1,
7 and 12345, placed at (0, 64, 0). 180 cases.

The level (`StubLevel`, a `WorldGenLevel` over flat dirt whose every method
throws until a tree calls it), why the placed object is the codec round-trip
rather than the bootstrap one, and what the fixture cannot pin, are beside the
fixture in
`crates/mcrs_minecraft_decoration/tests/fixtures/vanilla/tree_geometry_capture_procedure.md`.
`StubGen` prints the stub skeleton for any interface
(`./gradlew stubGen -PstubClass=net.minecraft.world.level.WorldGenLevel`); run
it again when a version bump changes `WorldGenLevel`.

## Binary layout

Little-endian, same primitives as the density dumps.

```
magic          8 bytes, ASCII "MCTREEG0"
format_version u32   currently 1
world_version  u32   SharedConstants.getCurrentVersion().dataVersion().version()
case_count     u32

repeated case_count times:
  feature_id     str
  seed           i64
  origin_x       i32
  origin_y       i32
  origin_z       i32
  placed         u32   1 when place returned true
  rng_after_lo   i64   random.nextLong() after place returned
  rng_after_hi   i64   the next one
  palette_count  u32
  palette        str * palette_count
  block_count    u32
  blocks         (i32 x, i32 y, i32 z, u32 palette index) * block_count
```

`blocks` is in **first-write order** with the state the position ended up
holding. The leaf relaxation only rewrites positions already written, so it
cannot reorder the list; the order is therefore the trunk, foliage and decorator
order, and a port that visits cells in a different order fails on it.
