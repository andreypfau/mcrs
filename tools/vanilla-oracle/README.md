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
