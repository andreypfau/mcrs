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
    -PoracleOut=../../crates/mcrs_minecraft_worldgen_density/tests/fixtures/vanilla
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
    -PoracleOut=../../crates/mcrs_minecraft_worldgen_generator/src/tests/fixtures
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
    -PoracleOut=../../crates/mcrs_minecraft_worldgen_feature/tests/fixtures/vanilla
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
    -PoracleOut=../../crates/mcrs_minecraft_worldgen_feature_place/tests/fixtures/vanilla
```

One file, `ore_vein.bin`.

The cases, what each one pins, and the provenance map of the lifted code are
beside the fixture in
`crates/mcrs_minecraft_worldgen_feature_place/tests/fixtures/vanilla/capture_procedure.md`.

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
    -PoracleOut=../../crates/mcrs_minecraft_worldgen_feature_place/tests/fixtures/vanilla
```

One file, `tree_geometry.bin`: all 45 `minecraft:tree` features at seeds 42, 1,
7 and 12345, placed at (0, 64, 0). 180 cases.

The level (`StubLevel`, a `WorldGenLevel` over flat dirt whose every method
throws until a tree calls it), why the placed object is the codec round-trip
rather than the bootstrap one, and what the fixture cannot pin, are beside the
fixture in
`crates/mcrs_minecraft_worldgen_feature_place/tests/fixtures/vanilla/tree_geometry_capture_procedure.md`.
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

---

# Template dumps

`TemplateOracle.main` loads every structure template the jar ships through the
loader a server uses — `ResourceManagerTemplateSource` over the vanilla data
pack, with the real `DataFixers.getDataFixer()` and `BuiltInRegistries.BLOCK` —
and records, for all 1511 of them, what `StructureTemplate.load` produced: the
size, the palettes, the three-section block ordering, the file palette entries
as resolved, and every jigsaw. For 33 listed templates it also writes the full
ordered block list per palette. It runs no server: only
`Bootstrap.bootStrap()` and the vanilla pack's `ResourceManager` are built.
`StructureTemplateManager` is not constructed (it demands a world save
directory); the resource-manager source it delegates to is called directly, and
the private `palettes` list is read by reflection.

```sh
cd tools/vanilla-oracle
./gradlew dumpTemplates --console=plain --no-daemon \
    -PoracleOut=../../crates/mcrs_minecraft_worldgen_generator/src/tests/fixtures
```

One file, `templates.bin`. The task throws if the pack lists anything other
than 1511 templates, if any template fails to load, or if a listed id is not
in the corpus — vanilla substitutes an empty template where it cannot load one,
and the dump must not.

The listed subset, what each entry pins, and what the fixture cannot pin are
beside the fixture in
`crates/mcrs_minecraft_worldgen_generator/src/tests/fixtures/templates_capture_procedure.md`.

## Binary layout

Little-endian, same primitives as the density dumps. The block lists in the
listed section are the same `(palette, blocks)` shape as the ore-vein and tree
dumps, followed by one extra bitset.

```
magic            8 bytes, ASCII "MCTMPLT0"
format_version   u32   currently 1
world_version    u32   SharedConstants.getCurrentVersion().dataVersion().version()

dynamic_count    u32   blocks whose Block.hasDynamicShape() is true
dynamic_ids      str * dynamic_count      in BuiltInRegistries.BLOCK order

template_count   u32   1511
repeated template_count times, ids ascending by Identifier.toString():
  id             str   e.g. "minecraft:village/plains/houses/plains_small_house_1"
  size_x         i32   StructureTemplate.getSize()
  size_y         i32
  size_z         i32
  palette_count  u32   1, or 8 for the shipwrecks
  repeated palette_count times:
    full_count     u32   blocks with no nbt whose state is a full collision
                         shape and whose block has no dynamic shape
    other_count    u32   blocks with no nbt that are not full
    entity_count   u32   blocks with nbt; the three sum to Palette.blocks().size()
    entry_count    u32   the file's palette list length
    entries        str * entry_count      BlockStateParser.serialize of
                                          NbtUtils.readBlockState over each entry
    jigsaw_count   u32   Palette.jigsaws().size()
    repeated jigsaw_count times, in Palette.jigsaws() order:
      x, y, z              i32 * 3
      state                str   e.g. "minecraft:jigsaw[orientation=up_north]"
      front                str   JigsawBlock.getFrontFacing(state): "down" … "east"
      top                  str   JigsawBlock.getTopFacing(state)
      joint                str   "rollable" | "aligned"
      name                 str
      pool                 str
      target               str
      placement_priority   i32
      selection_priority   i32
      final_state_raw      str   nbt "final_state", or "minecraft:air" when absent
      final_state          str   BlockStateParser.parseForBlock of the raw string,
                                 serialized; "" when the parse throws

listed_count     u32   33
repeated listed_count times:
  id             str
  palette_count  u32
  repeated palette_count times:
    palette_count  u32
    palette        str * palette_count    interned in first-use order over Palette.blocks()
    block_count    u32
    blocks         (i32 x, i32 y, i32 z, u32 palette index) * block_count
    nbt_bits       byte * ((block_count + 7) / 8)
```

`blocks` is `Palette.blocks()` verbatim: the full-block section, then the
other-block section, then the block entities, each sorted by y, then x, then z.
Positions are template-relative. Bit `i` of `nbt_bits` is
`nbt_bits[i >> 3] & (1 << (i & 7))` and is set when block `i` carries a block
entity compound.

The file ends exactly at the last bitset; there is no trailer.

---

# Structure placement dumps

`PlacementOracle.main` records where vanilla puts structures and whether the
jigsaw start step yields a site there, for seeds 1, 42, 12345, -7 and
0x7FFF_FFFF_0000_0001. It runs no server, but it does load the data pack the
way a server does: `RegistryLayer.createRegistryAccess()`,
`TagLoader.loadTagsForExistingRegistries` on the static layer,
`TagLoader.buildUpdatedLookups`, then
`RegistryDataLoader.load(resources, worldContextRegistries, WORLD_REGISTRIES, executor)`.
`VanillaRegistries.createWorldLookup()` (what the density and surface dumps use)
is useless here: it wraps every tag lookup in an empty holder set, so every
structure's `biomes` tag is empty, `ChunkGeneratorStructureState.createForNormal`
keeps zero sets, and nothing places. The task throws unless the live set counts
come out as 18 overworld, 3 nether and 1 end.

Per dimension it builds `NoiseBasedChunkGenerator` over the loaded noise
settings and a biome source from the loaded parameter list (`TheEndBiomeSource`
for the end); per seed, `RandomState.create` and
`ChunkGeneratorStructureState.createForNormal(randomState, seed, ChunkPos.ZERO, biomeSource, structureSets)`.
For the jigsaw sites a real `StructureTemplateManager` is built over a temporary
`LevelStorageSource` access, and each `Structure.GenerationContext` is
constructed the way `StructureCheck.canCreateStructure` constructs it, with a
climate sampler from `randomState.createClimateSampler(SamplerContext.builder().enableCaches().build())`.

```sh
cd tools/vanilla-oracle
./gradlew dumpPlacement --console=plain --no-daemon -PoracleOut=<dir>
cp <dir>/structure_cells.bin ../../crates/mcrs_minecraft_worldgen_structure/tests/fixtures/vanilla/
cp <dir>/structure_sites.bin ../../crates/mcrs_minecraft_worldgen_generator/src/tests/fixtures/
```

Two files, both deterministic:

- `structure_cells.bin` (magic `MCPLACE0`): for every random-spread set live in
  each of the three dimensions, `getPotentialStructureChunk` and the full
  `isStructureChunk` verdict over the chunk squares `[-24, 25)²` and
  `[2000, 2025)²`.
- `structure_sites.bin` (magic `MCSITES0`): the 128 stronghold ring chunks; for
  every jigsaw structure live in the overworld and the nether, 16 placement
  chunks found by walking square rings out from (0, 0), with
  `findGenerationPoint` presence, the stub position, and the
  `findValidGenerationPoint` biome verdict from a fresh context; and
  `getBaseHeight` for `WORLD_SURFACE_WG` and `OCEAN_FLOOR_WG` at 64 columns per
  dimension.

The field-by-field layouts, the provenance of every value and the case summaries
are beside each fixture:
`crates/mcrs_minecraft_worldgen_structure/tests/fixtures/vanilla/structure_cells_capture_procedure.md`
and
`crates/mcrs_minecraft_worldgen_generator/src/tests/fixtures/structure_sites_capture_procedure.md`.

---

# Template placement dumps

`TemplatePlacementOracle.main` places every distinct template pool element the
data pack ships — and the two `minecraft:template` feature nodes
(`desert_well`, `sulfur_spring`) — into a `StubLevel` over a flat floor and
records what `StructureTemplate.placeInWorld` wrote: the written positions
with their final states (hashed, or in full for a fixed subset), every block
entity it loaded, and the placement random's state afterwards. Ahead of the
cases it writes three censuses: the `BLOCK_ENTITY_TYPE` registry order, which
block creates which block entity and whether that entity is a
`RandomizableContainer`, and every block state that any rotation changes with
its three rotations.

```sh
cd tools/vanilla-oracle
./gradlew dumpTemplatePlacement --console=plain --no-daemon \
    -PoracleOut=../../crates/mcrs_minecraft_worldgen_generator/src/tests/fixtures
```

One file, `template_placement.bin`, deterministic. The run log must contain no
`Serialization errors` line: `placeInWorld` reports block-entity load problems
through its logger rather than throwing, and a hit means a compound was not
loaded the way the fixture claims.

How `StubLevel` stands in for a server, the two entry points the cases go
through, what each case pins and what the fixture cannot pin are beside the
fixture in
`crates/mcrs_minecraft_worldgen_generator/src/tests/fixtures/template_placement_capture_procedure.md`.

## Binary layout

Little-endian, same primitives as the other dumps; `u8` is one raw byte.

```
magic            8 bytes, ASCII "MCTMPLP0"
format_version   u32   currently 1
world_version    u32   SharedConstants.getCurrentVersion().dataVersion().version()

type_count       u32   BuiltInRegistries.BLOCK_ENTITY_TYPE size
type_ids         str * type_count          registration order

entity_block_count  u32
repeated entity_block_count times, BuiltInRegistries.BLOCK order, every
EntityBlock whose newBlockEntity(ZERO, defaultBlockState()) is non-null:
  block_id       str
  type_id        str   BLOCK_ENTITY_TYPE key of the created entity
  loot_seeded    u8    1 iff the created entity is a RandomizableContainer

rotation_palette_count  u32
rotation_palette        str * count       BlockStateParser.serialize, interned in
                                          first-use order over (state, cw90,
                                          cw180, ccw90)
rotation_count   u32   states that at least one rotation changes
repeated rotation_count times, BLOCK order then getPossibleStates() order:
  state, cw90, cw180, ccw90   u32 * 4     palette indices; state.rotate(
                                          CLOCKWISE_90 | CLOCKWISE_180 |
                                          COUNTERCLOCKWISE_90)

palette_count    u32   global block-state palette over every written block,
palette          str * palette_count      interned in first-use order, file order

case_count       u32
repeated case_count times:
  kind           u8    0 template pool element, 1 template feature node
  template_key   str   kind 0: getTemplateLocation(); kind 1: entry template
                       ids joined by ","
  processors_key str   "ref:<processor list id>" | "inline" | "none" (feature
                       without processors); a kind-0 inline list is always
                       empty (the dump aborts otherwise), a kind-1 one is the
                       feature's own list (desert_well's append_loot rule)
  projection     u8    0 rigid, 1 terrain_matching; kind 1: 0
  legacy         u8    kind 0: 1 iff LegacySinglePoolElement; kind 1: 0
  rotation       u8    kind 0: Rotation.values()[caseIndex % 4]; kind 1: 0
  liquid         u8    kind 0: 1 (ignore_waterlogging) iff caseIndex % 8 == 7;
                       kind 1: 0
  px, py, pz     i32 * 3   kind 0: (8, 62, 8); kind 1: origin (8, 64, 8)
  rx, ry, rz     i32 * 3   kind 0: (center.x, minY, center.z) of
                           template.getBoundingBox(rotation, position);
                           kind 1: the origin
  has_clip       u8    kind 0: 1; kind 1: 0
  clip           i32 * 6   when has_clip: min x, y, z, max x, y, z, inclusive;
                           always (0, -63, 0, 15, 319, 15)
  placement_count  u32   kind 0: 2; kind 1: 6
  repeated placement_count times:
    floor          u8    0: dirt for y <= 63; 1: stone for y <= 60, water
                         source 61..=63; air above either
    seed           i64   XoroshiroRandomSource(seed) is the placement random;
                         kind 0: the case index; kind 1: 0, 1, 2
    template_drawn str   kind 0: template_key; kind 1: the entry the weighted
                         draw picked
    rotation_drawn u8    kind 0: rotation; kind 1: the drawn rotation
    x, y, z        i32 * 3   the position handed to placeInWorld
    placed         u8    what the placement returned
    count          u32   distinct written positions
    hash           u64   FNV-1a 64 over (i32 x, i32 y, i32 z, u32 palette
                         index) per written entry, first-write order, final
                         state
    full           u8    1 when the running placement index % 100 == 0, or
                         kind 1 with seed 0
    entries        (i32 x, i32 y, i32 z, u32 palette index) * count, when full
    entity_count   u32
    repeated entity_count times, sorted by (x, y, z):
      x, y, z      i32 * 3
      type_id      str   BLOCK_ENTITY_TYPE key of be.getType()
      len          u32
      nbt          len bytes   NbtIo.write of be.saveWithFullMetadata(access):
                               uncompressed, named (TAG_Compound, "", payload)
    rng_lo         i64   random.nextLong() after placement
    rng_hi         i64   random.nextLong() again
```

Kind-0 cases come first: pools sorted by `Identifier.toString()`,
`getTemplates()` raw pairs in order, `ListPoolElement` children in
`getElements()` order, the first occurrence of each
`(template, processors, projection, legacy)` key. Kind-1 cases follow:
`desert_well` then `sulfur_spring`, nodes in
`Stream.concat(Stream.of(self), getSubFeatures())` order, first occurrence of
each `(template ids, processors)` key. The case index and the running
placement index both run over the whole file. The file ends at the last
`rng_hi`; there is no trailer.
