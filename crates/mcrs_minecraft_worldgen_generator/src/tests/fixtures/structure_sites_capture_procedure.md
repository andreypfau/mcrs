# Fixture Capture Procedure — `structure_sites.bin`

**Source of truth:** vanilla `26.3-snapshot-10`, `world_version` 5015, read through
Fabric Loom's mapped jar. No server and no client is started. The structures,
structure sets, template pools, templates and biome tags are the jar's own data
pack, loaded through the same `RegistryDataLoader` path a dedicated server takes,
so every `#minecraft:has_structure/*` tag is bound.

**Harness:** `tools/vanilla-oracle/src/main/java/mcrs/oracle/PlacementOracle.java`
(writes this file and `structure_cells.bin` in one run)

```sh
cd tools/vanilla-oracle
./gradlew dumpPlacement --console=plain --no-daemon -PoracleOut=<dir>
cp <dir>/structure_sites.bin ../../crates/mcrs_minecraft_worldgen_generator/src/tests/fixtures/
```

Output is deterministic: re-running produces a byte-identical file (121 607
bytes).

**Consumer:** `crates/mcrs_minecraft_worldgen_generator/src/tests/structure_sites.rs`
and `structure_index.rs`, which assert, per seed, the 128 stronghold ring chunks
in list order; for every jigsaw structure, and again for every hardcoded type
except the mineshaft, live in the overworld and the nether, whether the start
step yields a site at each of 16 placement chunks, the site position, and
whether the biome at that position admits the structure; the two
heightmap-flavoured base heights at 64 columns per dimension; and for every
structure set live in those dimensions, which entry the weighted draw with
removal selects at each of 16 placement chunks.

---

## Bootstrap

Identical to `structure_cells.bin` (see
`crates/mcrs_minecraft_worldgen_structure/tests/fixtures/vanilla/structure_cells_capture_procedure.md`):
`Bootstrap.bootStrap()`, the vanilla pack's `MultiPackResourceManager`, then
`RegistryLayer.createRegistryAccess()` → `TagLoader.loadTagsForExistingRegistries`
→ `TagLoader.buildUpdatedLookups` → `RegistryDataLoader.load(..., WORLD_REGISTRIES, ...)`.
The static-layer pending tags are applied afterwards so block tags resolve too.

Additionally a real `StructureTemplateManager(resources, storage, DataFixers.getDataFixer(), BuiltInRegistries.BLOCK)`
is built, where `storage` is `LevelStorageSource.createDefault(tempDir).createAccess("oracle")`
over a fresh temporary directory (it only supplies the `generated/` path, which
stays empty).

Each `Structure.GenerationContext` is constructed as `StructureCheck` does:
`new Structure.GenerationContext(registryAccess, chunkGenerator, biomeSource, climateSampler, randomState, structureTemplateManager, seed, chunkPos, heightAccessor, structure.biomes()::contains)`
with `climateSampler = randomState.createClimateSampler(SamplerContext.builder().enableCaches().build())`
(one sampler per seed and dimension, shared across cases). The constructor
itself seeds the context's `WorldgenRandom` from `(seed, chunkPos)`, so the
second context built for `findValidGenerationPoint` starts from the same
random stream as the first one built for `findGenerationPoint`.

Height accessors: `LevelHeightAccessor.create(-64, 384)` for the overworld and
`LevelHeightAccessor.create(0, 256)` for the nether (the `min_y` / `height` of
the shipped `dimension_type` files).

## Binary layout

Little-endian. `str` is a `u32` byte length followed by that many UTF-8 bytes.

```
magic            8 bytes, ASCII "MCSITES1"
format_version   u32   currently 1
world_version    u32   SharedConstants.getCurrentVersion().dataVersion().version() = 5015

seed_count       u32   5
repeated seed_count times, seeds in the order 1, 42, 12345, -7, 0x7FFF_FFFF_0000_0001:
  seed           i64

  ring_set_count u32   overworld sets whose placement is concentric_rings: 1
  repeated ring_set_count times:
    set_id         str   "minecraft:strongholds"
    position_count u32   128
    positions      (i32 chunk_x, i32 chunk_z) * position_count
                         state.getRingPositionsFor(placement) in list order

  dim_count      u32   2
  repeated dim_count times, "minecraft:overworld" then "minecraft:the_nether":
    dimension        str
    structure_count  u32   structures in registry order that are JigsawStructure
                           and have a non-empty getPlacementsForStructure in this
                           dimension: 27 overworld, 1 nether
    repeated structure_count times:
      structure_id   str   e.g. "minecraft:village_plains"
      set_id         str   the set whose placement is the structure's placement
      case_count     u32   16
      repeated case_count times:
        chunk_x        i32
        chunk_z        i32
        site_present   u8    1 when structure.findGenerationPoint(context).isPresent()
        if site_present == 1:
          x, y, z        i32 * 3   GenerationStub.position()
          biome_ok       u8        1 when structure.findValidGenerationPoint(context)
                                   .isPresent() on a fresh context

  dim_count      u32   2
  repeated dim_count times, "minecraft:overworld" then "minecraft:the_nether":
    dimension      str
    probe_count    u32   64
    repeated probe_count times, i = 0..63:
      x                i32   i*37 - 1000
      z                i32   i*53 - 700
      world_surface_wg i32   generator.getBaseHeight(x, z, WORLD_SURFACE_WG, heightAccessor, randomState)
      ocean_floor_wg   i32   generator.getBaseHeight(x, z, OCEAN_FLOOR_WG, heightAccessor, randomState)

  dim_count      u32   2
  repeated dim_count times, "minecraft:overworld" then "minecraft:the_nether":
    dimension        str
    structure_count  u32   structures in registry order that are neither
                           JigsawStructure nor MineshaftStructure and have a
                           non-empty getPlacementsForStructure in this
                           dimension: 18 overworld, 3 nether
    repeated structure_count times:
      the same structure record as the jigsaw section above

  dim_count      u32   2
  repeated dim_count times, "minecraft:overworld" then "minecraft:the_nether":
    dimension      str
    set_count      u32   state.possibleStructureSets() in list order: 18 overworld, 3 nether
    repeated set_count times:
      set_id         str
      case_count     u32   16
      repeated case_count times:
        chunk_x        i32
        chunk_z        i32
        selected       str   the structure id the set's draw settles on, "" when
                             every entry is refused
```

The file ends after the last selection case of the last seed; there is no
trailer.

**Case chunks.** For each structure the harness walks chunks in expanding
square rings around (0, 0): radius 0, 1, 2, … up to 600; within a ring, x
ascending from −r to r, z ascending from −r to r, keeping only the perimeter
(`max(|x|, |z|) == r`). A chunk becomes a case when the set's
`placement.isStructureChunk(state, x, z)` holds; the walk stops after 16 cases.
Every shipped structure reaches 16 well inside the radius bound.

## Provenance Map

| Dumped value | Vanilla source |
|---|---|
| ring `positions` | `ChunkGeneratorStructureState.getRingPositionsFor` → `generateRingPositions` (`world/level/chunk/ChunkGeneratorStructureState.java:131-196`), seeded with the level seed (`createForNormal`, `:60-71`), biome snap via `BiomeSource.findBiomeHorizontal` (`world/level/biome/BiomeSource.java:47-57, 103-166`) |
| structure list and order | `Registry<Structure>.listElements()` filtered by `instanceof JigsawStructure` and `state.getPlacementsForStructure(holder)` non-empty (`:215-218`) |
| `set_id` | the `possibleStructureSets()` entry whose `placement()` is the structure's single placement |
| case chunks | `StructurePlacement.isStructureChunk` of that placement (`placement/AbstractSpreadingStructurePlacement.java:89-94`) |
| `site_present`, `x, y, z` | `JigsawStructure.findGenerationPoint` (`structure/structures/JigsawStructure.java:155-174`): `startHeight.sample` then `JigsawPlacement.addPieces` (`structure/pools/JigsawPlacement.java:51-141`); the position is the `GenerationStub` centre `(centerX, centerY, centerZ)` |
| `biome_ok` | `Structure.findValidGenerationPoint` (`structure/Structure.java:235-237`) = `findGenerationPoint` filtered by `GenerationContext.isValidBiome` (`:289-300`), which resolves the biome at the stub position through `biomeSource.createResolver(climateSampler)` and tests `structure.biomes()::contains` |
| `world_surface_wg`, `ocean_floor_wg` | `NoiseBasedChunkGenerator.getBaseHeight` (`levelgen/NoiseBasedChunkGenerator.java:157-166, 204-252`) |
| hardcoded `site_present`, `x, y, z` | each type's `findGenerationPoint`, invoked reflectively because `Structure` declares it protected: `onTopOfChunkCenter` (`structure/Structure.java:138-159`) for the treasure, hut, igloo, shipwreck, ocean ruin and monument; `SinglePieceStructure.findGenerationPoint` (`structure/SinglePieceStructure.java:25-36`) for the pyramid and jungle temple; `getLowestYIn5by5Box` (`Structure.java:215-231`) for the end city and mansion; `structures/NetherFossilStructure.java:37-70`; `structures/RuinedPortalStructure.java:66-166, 178-238`; the fortress and stronghold at fixed heights |
| `selected` | `ChunkGenerator.createStructures` (`world/level/chunk/ChunkGenerator.java:560-641`): a single entry is tried directly; otherwise the weighted draw with removal on a `WorldgenRandom` seeded by `setLargeFeatureSeed(seed, x, z)`; each try is `Structure.generate` (`structure/Structure.java:90-134`) with zero references, and succeeds when the start `isValid()` |

## Cases

5 seeds × (128 ring positions + 28 jigsaw structures × 16 chunks + 2 × 64
height probes). Site outcomes summed over the five seeds (80 cases each):

| Structure | present | biome ok | what it pins |
|---|---|---|---|
| `trial_chambers` | 80 | 78 | `uniform` start height, `dimension_padding` 10, no heightmap projection; the two biome rejections |
| `bastion_remnant` (nether) | 80 | 69 | nether height accessor (0, 256); `nether_complexes` set shared with the fortress |
| `ancient_city` | 80 | 5 | absolute start height, `start_jigsaw_name` anchoring; `deep_dark` rarely at the centre |
| `pillager_outpost` | 16 | 16 | `legacy_type_1` frequency reducer plus `villages` exclusion zone selecting the case chunks; `project_start_to_heightmap` column check |
| `trail_ruins` | 8 | 7 | a heightmap-projected start whose biome test can still fail |
| `village_plains` / `_desert` / `_snowy` / `_taiga` / `_savanna` | 6 / 2 / 3 / 1 / 1 | same | `couldStructureExistInColumn` rejecting most chunks (absent site); heightmap projection at the survivors |
| `abandoned_camp_*` (18) | 0–5 each, 18 in total | same | the 37-spacing set with one structure per biome family; most biome columns reject |

Sites are absent only through `couldStructureExistInColumn` (the biome column
test before heightmap projection); no shipped start pool is empty and no
`start_jigsaw_name` is missing from its start pool, so
`JigsawPlacement.addPieces` never returns empty for another reason here.

Hardcoded types, 21 per seed (18 overworld, 3 nether), 1680 cases in total, 866
present and 447 biome ok:

| Structure | present | biome ok | what it pins |
|---|---|---|---|
| `fortress`, `stronghold`, `ruined_portal_nether` | 80 | 80 | sites that always exist; the fixed `y` of 64 and 0, the portal's nether height draws |
| `ruined_portal*` (overworld, 6) | 80 each | 40 / 2 / 3 / 2 / 34 / 1 | setup weights, the giant-portal draw, rotation and mirror, the template box, `findSuitableY` per placement |
| `shipwreck` / `shipwreck_beached` | 45 / 2 | same | `onTopOfChunkCenter` with `OCEAN_FLOOR_WG` against `WORLD_SURFACE_WG` |
| `ocean_ruin_cold` / `_warm` | 41 / 6 | same | the same chunk-centre column test |
| `monument` | 13 | 13 | `getBiomesWithin` over the 29-block cube at sea level |
| `nether_fossil` | 29 | 8 | the base-column walk down from the sampled height, then the biome at the found block |
| `igloo` / `swamp_hut` / `mansion` | 6 / 2 / 2 | same | the surface column test; the mansion's 5×5 lowest corner and rotation draw |
| `buried_treasure`, `desert_pyramid`, `jungle_pyramid` | 0 | 0 | the column test refuses every case chunk near the origin for these seeds |

Selection, 21 sets per seed (18 overworld, 3 nether); `mineshafts` is dumped
but its site is not ported, so the consumer compares the other 20: 1600 cases,
582 selected. `nether_complexes` pins the mixed draw: the fortress site always
passes, so the bastion is selected only when the draw lands on it first.
