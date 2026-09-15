# Fixture Capture Procedure — `structure_layouts.bin`

**Source of truth:** vanilla `26.3-snapshot-10`, `world_version` 5015, read through
Fabric Loom's mapped jar. No server and no client is started. The structures,
structure sets, template pools, templates, placed features and biome tags are
the jar's own data pack, loaded through the same `RegistryDataLoader` path a
dedicated server takes.

**Harness:** `tools/vanilla-oracle/src/main/java/mcrs/oracle/JigsawOracle.java`
(shares its bootstrap, dimensions, seeds, structure enumeration and case chunks
with `PlacementOracle.java`, which writes `structure_sites.bin`)

```sh
cd tools/vanilla-oracle
./gradlew dumpJigsaw --console=plain --no-daemon -PoracleOut=<dir>
cp <dir>/structure_layouts.bin ../../crates/mcrs_minecraft_server/src/world/generate/tests/fixtures/
```

Output is deterministic: re-running produces a byte-identical file (3 830 805
bytes).

**Consumer:** the jigsaw assembly tests in `crates/mcrs_minecraft_server`, which
assert, for every jigsaw structure live in the overworld and the nether and each
of the same 16 placement chunks `structure_sites.bin` uses, whether
`Structure.generate` yields a valid start and, when it does, the start bounding
box and the full piece list: element, projection, position, rotation, bounding
box, ground level delta and junctions, in vanilla's piece order.

---

## Bootstrap

Identical to `structure_sites.bin` (see
`structure_sites_capture_procedure.md`): `Bootstrap.bootStrap()`, the vanilla
pack's `MultiPackResourceManager`, `RegistryLayer.createRegistryAccess()` →
`TagLoader.loadTagsForExistingRegistries` → `TagLoader.buildUpdatedLookups` →
`RegistryDataLoader.load(..., WORLD_REGISTRIES, ...)`, static-layer tags applied
afterwards, and a real `StructureTemplateManager` over an empty temporary
`LevelStorageAccess`.

Per seed and dimension a fresh `RandomState.create(noises, seed, settings)` and
`ChunkGeneratorStructureState.createForNormal(randomState, seed, ChunkPos.ZERO, biomeSource, structureSets)`
are built, plus one
`randomState.createClimateSampler(SamplerContext.builder().enableCaches().build())`
shared across the structure's cases.

Each case calls

```java
structure.generate(holder, dimensionKey, registryAccess, chunkGenerator, biomeSource,
    climateSampler, randomState, structureTemplateManager, seed, chunkPos,
    /* references */ 0, heightAccessor, structure.biomes()::contains)
```

with `dimensionKey = Level.OVERWORLD` or `Level.NETHER`
(`structure/Structure.java:90-135`). `generate` builds its own
`GenerationContext` (so the `WorldgenRandom` is seeded from `(seed, chunkPos)`
exactly as the server does), runs `findValidGenerationPoint`, and wraps the
`StructurePiecesBuilder` in a `StructureStart`; the start is present iff
`isValid()`, i.e. at least one piece was added. Because `generate` goes through
`findValidGenerationPoint`, a case is present here exactly when
`structure_sites.bin` has `site_present == 1 && biome_ok == 1`.

Height accessors: `LevelHeightAccessor.create(-64, 384)` for the overworld and
`LevelHeightAccessor.create(0, 256)` for the nether.

## Binary layout

Little-endian. `str` is a `u32` byte length followed by that many UTF-8 bytes.

```
magic            8 bytes, ASCII "MCJIGSW0"
format_version   u32   currently 1
world_version    u32   SharedConstants.getCurrentVersion().dataVersion().version() = 5015

seed_count       u32   5
repeated seed_count times, seeds in the order 1, 42, 12345, -7, 0x7FFF_FFFF_0000_0001:
  seed           i64
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
        present        u8    1 when structure.generate(...).isValid()
        if present == 1:
          start_box      i32 * 6   minX minY minZ maxX maxY maxZ of StructureStart.getBoundingBox()
          piece_count    u32
          repeated piece_count times, in StructureStart.getPieces() order:
            element            str      see "Element rendering"
            projection         u8       0 rigid, 1 terrain_matching (element.getProjection())
            x, y, z            i32 * 3  PoolElementStructurePiece.getPosition()
            rotation           u8       0 NONE, 1 CLOCKWISE_90, 2 CLOCKWISE_180, 3 COUNTERCLOCKWISE_90
            piece_box          i32 * 6  minX minY minZ maxX maxY maxZ of piece.getBoundingBox()
            ground_level_delta i32      piece.getGroundLevelDelta()
            junction_count     u32
            repeated junction_count times, in piece.getJunctions() order:
              source_x         i32
              source_ground_y  i32
              source_z         i32
              delta_y          i32
              dest_projection  u8       0 rigid, 1 terrain_matching
```

The file ends after the last case of the last structure of the last seed; there
is no trailer.

**Case chunks.** The same walk as `structure_sites.bin`: expanding square rings
around (0, 0) up to radius 600, x ascending then z ascending along the
perimeter, keeping chunks where `placement.isStructureChunk(state, x, z)`
holds, stopping after 16.

**Element rendering.** One string per `StructurePoolElement`, recursive for
lists:

| Element class | Rendered as |
|---|---|
| `SinglePoolElement` | `single:<getTemplateLocation()>` e.g. `single:minecraft:trial_chambers/corridor/end_1` |
| `LegacySinglePoolElement` | `legacy:<getTemplateLocation()>` e.g. `legacy:minecraft:village/plains/houses/plains_small_house_1` |
| `FeaturePoolElement` | `feature:<placed feature registry key>` e.g. `feature:minecraft:pile_hay` |
| `ListPoolElement` | `list[` + inner elements rendered the same way, joined by `,` + `]` |
| `EmptyPoolElement` | `empty` |

`getTemplateLocation()` is `Either.orThrow()` on the element's `location`: every
shipped pool references templates by id, so the inline-template side never
occurs and would abort the dump. `FeaturePoolElement` has no accessor for its
`Holder<PlacedFeature>`; the harness reads the private `feature` field through
reflection (the Loom runtime classpath carries named mappings, so the field
name resolves). No `empty` element appears in the shipped output; the row is
here so a datapack that uses one still dumps.

## Per-field semantics

| Dumped value | Vanilla source |
|---|---|
| structure list, `set_id`, case chunks | as in `structure_sites_capture_procedure.md` |
| `present` | `Structure.generate` → `findValidGenerationPoint` → `new StructureStart(...).isValid()` (`structure/Structure.java:90-135`, `structure/StructureStart.java:127-129`) |
| `start_box` | `StructureStart.getBoundingBox()` (`structure/StructureStart.java:75-83`): `PiecesContainer.calculateBoundingBox()` (the union of every piece's box) passed through `Structure.adjustBoundingBox`, which inflates by 12 on every axis when `terrain_adaptation` is not `none` (`structure/Structure.java:86-88`) |
| piece order | `JigsawPlacement.addPieces` adds the centre piece first (`structure/pools/JigsawPlacement.java:100`), then `Placer.tryPlacingChildren` appends each accepted child as it is placed (`:575`); the list is `StructurePiecesBuilder.build()` unchanged |
| `element`, `projection` | `PoolElementStructurePiece.getElement()`; `StructurePoolElement.getProjection()` (`structure/pools/StructurePoolElement.java:87-94`), the projection the pool entry carries; for `ListPoolElement` the outer projection is written, the inner elements carry the same one (`ListPoolElement.setProjectionOnEachElement`) |
| `x, y, z` | `PoolElementStructurePiece.getPosition()`: the template anchor after `moved(0, yOffset, 0)` (`JigsawPlacement.java:498`) |
| `rotation` | `PoolElementStructurePiece.getRotation()`, the `Rotation` chosen for the piece |
| `piece_box` | `StructurePiece.getBoundingBox()`: the box handed to the `PoolElementStructurePiece` constructor. For the centre piece it is the template box at the anchored start position after `centerPiece.move(0, bottomY − oldAbsoluteGroundY, 0)` shifts it so its ground level sits at the start height (`JigsawPlacement.java:99-129`); for children it is `rawTargetBB.moved(0, yOffset, 0)` **after** the `use_expansion_hack` encapsulation (`JigsawPlacement.java:497-506`), so a hack-grown piece box is taller than the template it places |
| `ground_level_delta` | `PoolElementStructurePiece.getGroundLevelDelta()`: `source − delta_y` for rigid children, `element.getGroundLevelDelta()` (1) for terrain-matching ones (`JigsawPlacement.java:520-526`) |
| junctions | `PoolElementStructurePiece.getJunctions()` in insertion order. Each accepted child appends one junction to the source piece (target jigsaw x/z, `junctionY − sourceJigsawLocalY + sourceGroundLevelDelta`, `deltaY`, target projection) and one to the child (source jigsaw x/z, `junctionY − targetJigsawLocalY + targetGroundLevelDelta`, `−deltaY`, source projection) (`JigsawPlacement.java:557-574`). The centre piece therefore lists its children in acceptance order; a child lists its parent first, then its own children |

## Cases

5 seeds × 28 structures × 16 chunks = 2 240 cases; 206 present starts;
28 011 pieces; 55 610 junctions. Element kinds over all pieces: 26 038
`single`, 1 751 `legacy`, 196 `feature`, 26 `list`. Projections: 26 824 rigid,
1 187 terrain matching.

| Structure | present (of 80) | what it pins |
|---|---|---|
| `trial_chambers` | 78 | the largest layouts (roughly 250 pieces each), `single` elements, `dimension_padding`, no expansion hack |
| `bastion_remnant` (nether) | 69 | nether height accessor, `single` elements, rigid-only |
| `pillager_outpost` | 16 | `legacy` elements, the `list[...]` watchtower alternatives, expansion hack, terrain-matching feature pieces |
| `ancient_city` | 5 | `start_jigsaw_name` anchoring, `list[...]` camp alternatives, `sculk_patch_ancient_city` feature pieces |
| `trail_ruins` | 7 | terrain-matching children with `ground_level_delta` from the element |
| `village_*` | 13 in total | `legacy` elements, expansion hack, `feature:` pile and tree pieces, terrain-matching streets and their junction `source_ground_y` |
| `abandoned_camp_*` | 18 in total | the smallest layouts (2–7 pieces) |
