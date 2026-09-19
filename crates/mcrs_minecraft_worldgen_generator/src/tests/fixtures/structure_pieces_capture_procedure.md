# Fixture Capture Procedure — `structure_pieces.bin`

**Source of truth:** vanilla `26.3-snapshot-10`, `world_version` 5015, read through
Fabric Loom's mapped jar. No server and no client is started. The data pack is
the jar's own, loaded the way a server loads it (`PlacementOracle.loadWorldRegistries`),
the templates come through a real `StructureTemplateManager` over a temporary
save directory.

**Harness:** `tools/vanilla-oracle/src/main/java/mcrs/oracle/StructurePieceOracle.java`
(shares its bootstrap, dimensions, seeds and case chunks with
`PlacementOracle.java`, which writes `structure_sites.bin`)

```sh
cd tools/vanilla-oracle
./gradlew dumpStructurePieces --console=plain --no-daemon \
    -PoracleOut=../../crates/mcrs_minecraft_worldgen_generator/src/tests/fixtures
```

Output is deterministic: two consecutive runs produce a byte-identical file
(4 439 357 bytes).

**Consumer:** `crates/mcrs_minecraft_worldgen_generator/src/tests/structure_pieces.rs`,
which builds the same start through the index over the same noise world and
asserts, per present case, the start box, the piece count and every piece's
NBT against `Piece::nbt` serialised through `to_nbt_compound`, keys sorted and
the placement-fixed fields dropped from both sides. A present case this build
has no layout for is counted by its structure type; the set of those types is
pinned and shrinks as types are ported.

---

## Bootstrap

Identical to `structure_layouts.bin` (see
`structure_layouts_capture_procedure.md`), with the composite registry access
(`RegistryLayer.createRegistryAccess().replaceFrom(WORLD, loaded).compositeAccess()`)
handed to `Structure.generate`, because the ruined portal's `makeSettings`
looks the block registry up through it. Per seed and dimension a fresh
`RandomState` and `ChunkGeneratorStructureState.createForNormal(...)`, one
climate sampler shared across the dimension's cases.

Each case calls

```java
structure.generate(holder, dimensionKey, access, chunkGenerator, biomeSource,
    climateSampler, randomState, structureTemplateManager, seed, chunkPos,
    /* references */ 0, heightAccessor, structure.biomes()::contains)
```

and serialises the start's pieces with

```java
new StructurePieceSerializationContext(resources, access, structureTemplateManager)
piece.createTag(context)
```

which is `StructurePiece.createTag` — `id`, `BB`, `O`, `GD` — followed by the
type's `addAdditionalSaveData` (`structure/StructurePiece.java:90-99`). Nothing
is placed: every start is fresh from `generate`, so every placement flag is at
its construction value and every height the reference reads at placement is
still unset.

## Cases

Dimensions in `PlacementOracle.dims` order: `minecraft:overworld`,
`minecraft:the_nether`, `minecraft:the_end`. Seeds in `PlacementOracle.SEEDS`
order: 1, 42, 12345, −7, 0x7FFF_FFFF_0000_0001. Within a seed and dimension,
every structure of `Registries.STRUCTURE` (registry order) whose
`getPlacementsForStructure` is non-empty there:

- a structure that is not a `JigsawStructure` gets the same sixteen case
  chunks `structure_sites.bin` uses (the expanding square walk around the
  origin, first sixteen chunks passing `isStructureChunk`), present or not;
- a `JigsawStructure` gets its first present case over all seeds, once, and
  no case at all if no seed yields one at those chunks. This is the parity
  check of the jigsaw piece codec; the layouts themselves are pinned by
  `structure_layouts.bin`;
- a structure named in `MORE_PRESENT` gets, per seed and dimension, that many
  present cases more: the same expanding square walk continues past the
  shared sixteen cells (`PlacementOracle.cellsBeyond`), and each cell whose
  `generate` yields a valid start is written until the count is met or the
  walk reaches its radius cap. The desert pyramid, the buried treasure and
  the jungle pyramid ask for 2 each; seed 1 has only one pyramid within the
  cap.

1 967 cases, 572 present, 34 900 pieces. Per structure over all seeds:

| Structure | cases | present | pieces |
|---|---|---|---|
| `stronghold` | 80 | 80 | 12 284 |
| `mineshaft` | 80 | 76 | 10 268 |
| `fortress` (nether) | 80 | 80 | 9 182 |
| `mansion` | 80 | 2 | 1 097 |
| `mineshaft_mesa` | 80 | 2 | 339 |
| `ocean_ruin_cold` | 80 | 41 | 315 |
| `ruined_portal_nether` (nether) | 80 | 80 | 80 |
| `shipwreck` | 80 | 45 | 45 |
| `ruined_portal` | 80 | 40 | 40 |
| `ruined_portal_ocean` | 80 | 34 | 34 |
| `igloo` | 80 | 6 | 28 |
| `monument` | 80 | 13 | 13 |
| `ocean_ruin_warm` | 80 | 6 | 10 |
| `nether_fossil` (nether) | 80 | 8 | 8 |
| `ruined_portal_jungle` | 80 | 3 | 3 |
| `ruined_portal_desert`, `ruined_portal_mountain`, `shipwreck_beached`, `swamp_hut` | 80 each | 2 each | 2 each |
| `ruined_portal_swamp` | 80 | 1 | 1 |
| `desert_pyramid` | 89 | 9 | 9 |
| `buried_treasure`, `jungle_pyramid` | 90 each | 10 each | 10 each |
| `end_city` (end) | 80 | 0 | 0 |
| jigsaw: `ancient_city` 88, `bastion_remnant` 176, `trial_chambers` 188, five villages 84–148, `pillager_outpost` 8, `trail_ruins` 17, eight `abandoned_camp_*` 2–3 | 1 each | 1 each | 1 116 in total |

The one type with no present case is not pinned by this file; its own
cases are added with its port through `MORE_PRESENT`. The monument writes one piece: its
rooms are held in memory and not serialised (`OceanMonumentPieces`), so the
codec carries the building alone.

## Binary layout

Little-endian. `str` is a `u32` byte length followed by that many UTF-8 bytes.

```
magic            8 bytes, ASCII "MCSTRPC0"
format_version   u32   currently 1
world_version    u32   SharedConstants.getCurrentVersion().dataVersion().version() = 5015

case_count       u32
repeated case_count times, seeds outer, dimensions middle, structures inner:
  seed           i64
  dimension      str   "minecraft:overworld" | "minecraft:the_nether" | "minecraft:the_end"
  structure_id   str   e.g. "minecraft:stronghold"
  chunk_x        i32
  chunk_z        i32
  present        u8    1 when structure.generate(...).isValid()
  if present == 1:
    start_box    i32 * 6   minX minY minZ maxX maxY maxZ of StructureStart.getBoundingBox()
    piece_count  u32
    repeated piece_count times, in StructureStart.getPieces() order:
      len        u32
      nbt        len bytes   NbtIo.write of piece.createTag(context): uncompressed,
                             named (TAG_Compound, "", payload)
```

The file ends after the last case; there is no trailer.

## Per-field semantics

| Dumped value | Vanilla source |
|---|---|
| structure list, case chunks | as in `structure_sites_capture_procedure.md`, minus the mineshaft exclusion: every non-jigsaw structure with a placement is here, plus the `MORE_PRESENT` cells |
| `present`, `start_box` | as in `structure_layouts_capture_procedure.md` |
| piece order | `StructurePiecesBuilder.build()` unchanged: the order each type's `addPieces` appends them |
| `id` | `BuiltInRegistries.STRUCTURE_PIECE.getKey(piece.getType())` (`structure/StructurePiece.java:92`) |
| `BB` | the piece box as an int array `[minX, minY, minZ, maxX, maxY, maxZ]` |
| `O` | `orientation.get2DDataValue()`, or −1 for a piece with no orientation (`:95`) |
| `GD` | `genDepth` (`:96`): a collision tag for end cities, a room flag for monuments, 0 for a jigsaw piece |
| the rest | each type's `addAdditionalSaveData` |

## What the consumer drops before comparing

The reference fills some fields in at placement, from the live world of the
first chunk that decorates the piece; this build fixes them at layout from the
density heights. A fresh start carries their unset values, so they cannot
agree and are dropped from both sides:

| Piece `id` | Fields |
|---|---|
| `minecraft:tedp`, `minecraft:tejp`, `minecraft:tesh` | `HPos` (`ScatteredFeaturePiece.heightPosition`, −1 until placed) |
| `minecraft:shipwreck` | `TPY`, `height_adjusted` (`ShipwreckPieces.ShipwreckPiece.adjustPositionHeight`); `BB` and `TPX`/`TPZ` are compared, since the layout here keeps the box at the y 90 the reference builds it at and carries the lowered height beside it |

Placement flags — `hasPlacedChest0..3`, `placedMainChest`, `placedHiddenChest`,
`placedTrap1`, `placedTrap2`, `Witch`, `Cat`, `Chest`, `Mob`, `hps` — are
compared as written: a fresh start has them at their construction values, and
the codec here writes the same, since placement never mutates a piece.

## What the fixture cannot pin

- **The end.** Only the end city places there, and it has no present case at
  these chunks; the consumer has no end index yet and skips the dimension.
- **Placement.** Nothing is placed, so nothing here says what a piece writes;
  that is `structure_geometry.bin`.
- **A loaded start.** The consumer serialises pieces it laid out; that a
  serialised piece loads back to the same piece is the codec's own round-trip
  test.
