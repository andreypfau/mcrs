# Fixture Capture Procedure — `structure_cells.bin`

**Source of truth:** vanilla `26.4-snapshot-1`, `world_version` 5119, read through
Fabric Loom's mapped jar. No server and no client is started. The structure sets,
structures and biome tags are the jar's own data pack, loaded through the same
`RegistryDataLoader` path a dedicated server takes, so every `#minecraft:has_structure/*`
tag is bound.

**Harness:** `tools/vanilla-oracle/src/main/java/mcrs/oracle/PlacementOracle.java`
(writes this file and `structure_sites.bin` in one run)

```sh
cd tools/vanilla-oracle
./gradlew dumpPlacement --console=plain --no-daemon -PoracleOut=<dir>
cp <dir>/structure_cells.bin ../../crates/mcrs_minecraft_worldgen_structure/tests/fixtures/vanilla/
```

Output is deterministic: re-running produces a byte-identical file (2 866 650
bytes).

**Consumer:** the placement tests in `crates/mcrs_minecraft_worldgen`, which
assert, for every random-spread structure set that is live in a dimension, the
potential structure chunk of every source chunk in two chunk squares and whether
the full placement test (`isStructureChunk`: the cell test, the frequency
reduction and the exclusion zone) holds there.

---

## Bootstrap

`SharedConstants.tryDetectVersion()`, `Bootstrap.bootStrap()`, then the vanilla
pack's `MultiPackResourceManager` and the registry load a server performs:
`RegistryLayer.createRegistryAccess()`, `TagLoader.loadTagsForExistingRegistries`
on the static layer, `TagLoader.buildUpdatedLookups`, and
`RegistryDataLoader.load(resources, worldContextRegistries, WORLD_REGISTRIES, executor)`.
`VanillaRegistries.createWorldLookup()` is **not** used: its lookups wrap every
tag as an empty holder set, which makes every structure biome-less and every set
dead.

Per dimension: `NoiseBasedChunkGenerator(biomeSource, noiseSettings)` with
`minecraft:overworld` / `minecraft:nether` / `minecraft:end` noise settings; the
biome source is `MultiNoiseBiomeSource.createFromPreset` over the loaded
`overworld` / `nether` parameter list and `TheEndBiomeSource.create` over the
loaded biome registry. Per seed:
`RandomState.create(noises, seed, settings)` and
`ChunkGeneratorStructureState.createForNormal(randomState, seed, ChunkPos.ZERO, biomeSource, structureSets)`.
The harness throws unless the live set counts are 18 (overworld), 3 (nether:
`nether_complexes`, `nether_fossils`, `ruined_portals`) and 1 (end: `end_cities`).

## Binary layout

Little-endian. `str` is a `u32` byte length followed by that many UTF-8 bytes.

```
magic            8 bytes, ASCII "MCPLACE0"
format_version   u32   currently 1
world_version    u32   SharedConstants.getCurrentVersion().dataVersion().version() = 5119

seed_count       u32   5
repeated seed_count times, seeds in the order 1, 42, 12345, -7, 0x7FFF_FFFF_0000_0001:
  seed           i64
  dim_count      u32   3
  repeated dim_count times, "minecraft:overworld", "minecraft:the_nether", "minecraft:the_end":
    dimension    str
    set_count    u32   sets in state.possibleStructureSets() whose placement is
                       random_spread, in that list's order (registry order);
                       17 overworld, 3 nether, 1 end
    repeated set_count times:
      set_id       str   e.g. "minecraft:villages"
      spacing      i32   RandomSpreadStructurePlacement.spacing()
      separation   i32   RandomSpreadStructurePlacement.separation()
      grid_count   u32   2
      repeated grid_count times, grids (-24, -24, 49) then (2000, 2000, 25):
        x0           i32
        z0           i32
        side         i32   the grid covers chunks [x0, x0+side) × [z0, z0+side)
        repeated side*side times, z outer ascending, x inner ascending:
          potential_x    i32   placement.getPotentialStructureChunk(seed, x, z).x()
          potential_z    i32   ...z()
          is_structure   u8    1 when placement.isStructureChunk(state, x, z)
```

The file ends after the last grid of the last set; there is no trailer.

## Provenance Map

| Dumped value | Vanilla source |
|---|---|
| live set list and order | `ChunkGeneratorStructureState.createForNormal` → `possibleStructureSets()` (`world/level/chunk/ChunkGeneratorStructureState.java:60-71, 97-99`) |
| `spacing`, `separation` | `RandomSpreadStructurePlacement.spacing()` / `separation()` |
| `potential_x`, `potential_z` | `RandomSpreadStructurePlacement.getPotentialStructureChunk(seed, x, z)` (`placement/RandomSpreadStructurePlacement.java:84-93`): `floorDiv` cell, `setLargeFeatureWithSalt(seed, cellX, cellZ, salt)`, two `spreadType.evaluate` draws |
| `is_structure` | `AbstractSpreadingStructurePlacement.isStructureChunk` (`:89-94`): `isPlacementChunk` && `applyAdditionalChunkRestrictions` (frequency reducers `:113-146`) && `applyInteractionsWithOtherStructures` (`ExclusionZone.isPlacementForbidden` → `ChunkGeneratorStructureState.hasStructureChunkInRange` `:224-238`) |

## Cases

5 seeds × 21 dimension-sets × 3026 chunks = 317 730 chunk records. Per set the
structure-chunk count over the two grids ranges from 0 (`woodland_mansions`,
spacing 80, at some seeds) to ~790 (`nether_fossils`, spacing 2, separation 1).
`pillager_outposts` exercises `legacy_type_1` at frequency 0.2 and the 6-chunk
`villages` exclusion zone; `mineshafts` exercises `legacy_type_3` at frequency
0.004 with spacing 1; `buried_treasures` exercises `legacy_type_2` at frequency
0.01 with spacing 1; `end_cities`, `ocean_monuments` and `woodland_mansions`
spread `triangular` (two draws averaged) where every other set spreads
`linear`. The 2000-offset grid pins the wrapping `i32` arithmetic of the salted
seed far from the origin.
