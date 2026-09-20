# Fixture Capture Procedure — `structure_geometry.bin`

**Source of truth:** vanilla `26.3-snapshot-10`, `world_version` 5015, read through
Fabric Loom's mapped jar. No server and no client is started. The data pack is
the jar's own, loaded the way a server loads it (`PlacementOracle.loadWorldRegistries`),
the templates come through a real `StructureTemplateManager` over a temporary
save directory, every start is built by the real `Structure.generate` over a
flat chunk generator, and every chunk of it is placed by the real
`StructureStart.placeInChunk` into a `StubLevel` whose only state is the flat
base, the blocks placement writes, the block entities it loads and the
entities it spawns.

**Harness:** `tools/vanilla-oracle/src/main/java/mcrs/oracle/StructureGeometryOracle.java`
(`StubLevel.java` supplies the level)

```sh
cd tools/vanilla-oracle
./gradlew dumpStructureGeometry --console=plain --no-daemon \
    -PoracleOut=../../crates/mcrs_minecraft_worldgen_generator/src/tests/fixtures
```

Output is deterministic in every value: two consecutive runs produce a file
of the same size (580 701 bytes) that differs only in the order of the
`attributes` list inside the igloo's villagers and the ocean ruin's drowned,
which the JVM's identity hashes decide, so a consumer comparing entity NBT
must sort that list. The run log must contain no `Serialization errors`
line; the capture that produced this file had none.

**Consumer:** `crates/mcrs_minecraft_worldgen_generator/src/tests/structure_geometry.rs`,
which runs the site and the layout over a `SiteWorld` that is the same flat
base with the same one biome, asserts the start box, piece count, step and
step index, then places the start chunk by chunk into a box region built
from the same base — heights answered from the base alone — and asserts, per
chunk, the written positions and states (count and hash, or the full list),
the block entities, the entities, and the two `nextLong` values the placement
stream yields afterwards. An entity is compared by its type and, over the
compound `GeneratedEntity` serialises to with `UUID` dropped and its
passengers as arrivals of their own, every field it writes against the
reference's save tag; the fields only the reference writes
(health, air, the follow-range roll, the brain) are the kind's defaults, and
the packed entity data is left to the server's pairing, which derives it from
components. A present case this build has no layout for is counted by its
structure type; the set of those types is pinned and shrinks as types are
ported.

---

## Not a lift

**The world is flat.** `Structure.generate` is given a `ChunkGenerator`
(`StructureGeometryOracle.FlatGenerator`) over a `FixedBiomeSource` whose
`getBaseHeight`, `getBaseColumn`, `getSeaLevel`, `getMinY` and `getGenDepth`
are answered from a list of layers and the dimension: `FlatLevelSource`'s
semantics with the sea level, floor and height of the real dimension instead
of that class's constants. The height accessor is the dimension's
(`LevelHeightAccessor.create(-64, 384)` for the overworld, `(0, 256)` for the
nether and the end); the `RandomState` and climate sampler are the real ones
for the world seed and are never consulted, since a fixed biome source ignores
them. The site and layout therefore read the flat floor where a server reads
the noise column, which is what lets the consumer reproduce them without a
density tape.

**Placement follows `applyBiomeDecoration` chunk by chunk.** Every chunk the
start's bounding box touches, x ascending then z ascending, gets

```java
start.placeInChunk(level, level.structureManager(), generator, random, writableArea, chunkPos)
```

with `writableArea = (minBlockX, minY + 1, minBlockZ, maxBlockX, maxY, maxBlockZ)`,
the box `ChunkGenerator.getWritableArea` hands a piece. The random of each
chunk is a fresh `XoroshiroRandomSource(streamSeed)` where

```java
decorationSeed = new WorldgenRandom(new XoroshiroRandomSource(0)).setDecorationSeed(WORLD_SEED, minBlockX, minBlockZ)
streamSeed     = decorationSeed + index + 10000 * step
```

is the reference's arithmetic (`WorldgenRandom.setDecorationSeed`,
`setFeatureSeed`), `step` the structure's `GenerationStep.Decoration` ordinal
and `index` its position among the structures of that step in registry order,
as `applyBiomeDecoration` counts them. The stream seed is written per chunk so
the consumer seeds the same raw Xoroshiro source; as in every other fixture the
`WorldgenRandom` wrapper's `next(bits)` routing is not reproduced, which is a
project-wide convention rather than something this fixture decides. One stub
level holds the whole start, so a chunk sees what earlier chunks wrote, as the
region does.

**The stub level's random is the placement stream.** `level.getRandom()`, on
the `WorldGenLevel` and on the `ServerLevel` the entities are built in, is the
same `XoroshiroRandomSource` `placeInChunk` receives, reseated per chunk. The
reference draws a desert pyramid's cellar, a mansion's allay count and every
`finalizeSpawn` from the region's random, which is the server's and is seeded
from the clock; here those draws come from the stream, so they are
reproducible and the consumer draws them from the same stream.

**Heightmaps answer the unwritten base.** `getHeight(type, x, z)` walks the
base layers, never the written blocks: the reference primes its `_WG` maps
at the terrain step and never updates them for structure writes, and the
`WORLD_SURFACE` and `OCEAN_FLOOR` maps a piece may ask for are, on a chunk
that is still generating, the same terrain. A piece that reads a height after
another piece wrote under it therefore sees the floor.

**A beached shipwreck is lowered before any chunk is placed.** The
reference lowers a `ShipwreckPiece` that fits its region in the first chunk
that decorates it (`ShipwreckPieces.ShipwreckPiece.postProcess`): the
minimum of `getHeight(WORLD_SURFACE_WG)` over the template's unrotated
footprint, less half the template height, less a `nextInt(3)` of that
chunk's placement stream. The port fixes the height at layout and spends the
draw at the end of the layout stream. So that the two agree, the harness
runs the same footprint walk over the stub's heights before placement and
takes the draw from the layout's `WorldgenRandom` — `setLargeFeatureSeed`
for the case chunk, replayed past the rotation and template picks of
`ShipwreckStructure.generatePieces` — then calls `adjustPositionHeight`,
which also marks the piece adjusted so `postProcess` draws nothing. The
ocean shipwreck takes the mean over the same footprint with no draw and is
left to `postProcess`. The start box is written before the piece moves, so
it is the box `generate` built at y 90, as `structure_pieces.bin` records
it; the placed blocks and the palette pick (positional on the lowered
template position) follow the lowered piece.

**Difficulty is Normal at overworld clock time zero.** `getCurrentDifficultyAt`
answers `DifficultyInstance(NORMAL, 0, 0, 0.0)`, what `ServerLevel` answers
for a chunk that is not yet full; the special multiplier is 0, so no armour
or enchantment roll passes and their extra draws never happen. `getDifficulty`
is `NORMAL`, so monsters pass `EntityType.canSpawn`.

**Entities are built for real.** `getLevel()` returns a `ServerLevel`
allocated without a constructor, with `Level.random` and
`soundSeedGenerator` seated by reflection and every method an entity reaches
while it is built, finalized and saved delegated to the stub: registries,
dimension type, blocks, fluids, block entities, heights, sea level, biome,
difficulty, game time 0, feature flags, the environment attributes and the
structure manager; sounds, level events and game events are swallowed as
`WorldGenRegion` swallows them. `getServer()` is a `DedicatedServer` allocated
the same way, holding the registries, the template manager and an empty
`ServerDebugSubscribers`, which is all a mob's path finder asks of it. The
structure manager answers every start query with the start being placed, so
a spawn condition asking which structure stands here (the cat's) sees the
placing start and nothing else. The environment attributes are the static
layers `WorldGenRegion` builds — the dimension type's and the biome's — so the
moon phase is the attribute default, full, brightness 1.0. `addFreshEntity`
records each entity as it arrives; `addFreshEntityWithPassengers` arrives as
the vehicle then each passenger.

**Template pieces place with `knownShape = true`.** `TemplateStructurePiece`
leaves it false, which runs `updateFromNeighbourShapes` over every placed
block against the live world's neighbours and light (a crop consults the light
engine); the port does not reproduce that pass, so each template piece's
`placeSettings` is flipped by reflection before placement, the same deliberate
deviation `template_placement.bin` makes. Pieces built from `placeBlock`
(`setBlock(pos, state, 2)`) have no such pass. `markPosForPostProcessing`,
which `placeBlock` calls for fences, bars and walls, is taken by an empty
`ProtoChunk` per chunk and resolved by nothing: the live-world post-processing
pass `docs/scattering.md` already lists as not reproduced.

**A start's chunks are placed nearest the origin first.** The chunks a
start's box covers are sorted by the squared distance of their centre from
(0, 0), ties by x then z, before any is placed, and nothing is reset between
them. `MineShaftCorridor.hasPlacedSpider` lives on the piece the start's
chunks share: the cave spider spawner goes to whichever chunk decorates
first with a candidate section inside it, and every chunk after that skips
the `nextInt(3)` draw of every section. In a live world that order is the
player's route; the port fixes it as this order, choosing at layout the
first column in it that holds a section whose three candidate cells all lie
inside it under the ocean floor, and the file records the reference walked
in the same order. Every other placement flag (`hasPlacedChest`,
`placedTrap`, `Witch`, `Mob` and their kin) guards a block that belongs to
exactly one chunk, so the order does not touch them.

**Entity-own random values are masked.** `Entity.random` is seeded from the
clock and cannot be reproduced. Before an entity's tag is written: `UUID` is
removed; the shulker's `Rotation` is removed (the yaw `LivingEntity`'s
constructor rolls, which every other spawned mob overwrites with `snapTo`);
and the `attributes` entries `minecraft:knockback_resistance` and
`minecraft:spawn_reinforcements` are removed, the two values
`Zombie.handleAttributes` rolls from the entity's random. Passengers are
masked the same way under their own `id`. Everything else an entity carries
was drawn from the placement stream or is constant, and the consumer draws
its own values for the masked fields from a fork of the stream.

Two more things a real server has that the stub supplies: item components
are bound (`DATA_COMPONENT_INITIALIZERS`), and `enabledFeatures()` returns
`FeatureFlags.DEFAULT_FLAGS`.

## Bases

Layers from the dimension floor up to y = 63; `null` is air; everything above
63 is air; the sea level is the noise settings' (63 overworld, 32 nether, 0
end). The base is chosen by structure type.

| Base | u8 | Layers | Types |
|---|---|---|---|
| dry | 0 | stone ≤ 60, dirt 61–62, grass block 63 | every other type, the beached shipwreck |
| water | 1 | stone ≤ 30, gravel 31–40, water 41–62 | ocean ruin, monument, buried treasure, the ocean shipwreck, a ruined portal whose every setup is `on_ocean_floor` |
| cave | 2 | stone ≤ 63 with air at 33–40 | nether fossil, mineshaft |

The beached flag is `ShipwreckStructure.isBeached` and the portal setups are
`RuinedPortalStructure.setups`, both read by reflection.

## Cases

Every structure of `Registries.STRUCTURE` in registry order that is not a
`JigsawStructure`, plus the jigsaw structures in `JIGSAW_CASES`
(`minecraft:trail_ruins`, whose templates carry no entities), each in the
first dimension of `PlacementOracle.dims` whose sets place it, at the chunks
(0, 0), (7, −3) and (−12, 25), world seed `StubLevel.WORLD_SEED` = 0x5EED
throughout (the capped processors of the trail ruins fork from it), the
biome being the first of the structure's `biomes` set. The set placement is
not consulted: `generate` runs at the chunk whether or not a set would start
there, and the flat floor with a matching biome makes every site pass.

75 cases, 75 present, 1 751 placed chunks, 818 388 written positions, 740
palette states, 36 full lists (every 50th placed chunk), 634 block entities
(347 `brushable_block`, 205 `chest`, 36 `banner`, 17 `mob_spawner`, 7
`jigsaw`, 6 `dispenser`, 4 `campfire`, 4 `furnace`, 2 `brewing_stand`, 2
`ender_chest`, and one each of `blast_furnace`, `sign`, `skull`,
`trapped_chest`), 163 entities (47 `vindicator`, 36 `shulker`, 28
`chest_minecart`, 15 `drowned`, 11 `evoker`, 9 `elder_guardian`, 7 `allay`,
3 `witch`, 3 `cat`, one each of `item_frame`, `villager`, `zombie_villager`
and `zombie_nautilus` with a drowned riding it).

## Binary layout

Little-endian. `str` is a `u32` byte length followed by that many UTF-8 bytes.

```
magic            8 bytes, ASCII "MCSTRGE0"
format_version   u32   currently 1
world_version    u32   5015

palette_count    u32
palette          str * palette_count   BlockStateParser.serialize, first-use order

case_count       u32
repeated case_count times:
  structure_id   str
  dimension      str   "minecraft:overworld" | "minecraft:the_nether" | "minecraft:the_end"
  biome          str   the fixed biome
  base           u8    0 dry, 1 water, 2 cave
  seed           i64   0x5EED
  chunk_x        i32   the start chunk
  chunk_z        i32
  present        u8    1 when structure.generate(...).isValid()
  if present == 1:
    start_box      i32 * 6   StructureStart.getBoundingBox()
    piece_count    u32       start.getPieces().size()
    step           u8        structure.step().ordinal()
    index          u32       the structure's index among its step, registry order
    placed_count   u32       chunks of start_box, x ascending then z ascending
    repeated placed_count times:
      chunk_x      i32
      chunk_z      i32
      stream_seed  i64       decorationSeed + index + 10000 * step
      count        u32       positions written by this chunk's placeInChunk
      hash         u64       FNV-1a 64 over (i32 x, i32 y, i32 z, u32 palette
                             index) per written entry: first write fixes the
                             order, last write fixes the state, within this chunk
      full         u8        1 when the running placed-chunk index % 50 == 0
      entries      (i32 x, i32 y, i32 z, u32 palette index) * count, when full
      block_entity_count  u32   block entities created during this chunk
      repeated, sorted by (x, y, z):
        x, y, z    i32 * 3
        type_id    str    BLOCK_ENTITY_TYPE key
        len        u32
        nbt        len bytes   NbtIo.write of be.saveWithFullMetadata(access)
      entity_count  u32   entities addFreshEntity received during this chunk, arrival order
      repeated:
        type_id    str    ENTITY_TYPE key
        len        u32
        nbt        len bytes   NbtIo.write of entity.saveWithoutId(...), masked as above
        data_len   u32
        data       data_len bytes   each of getEntityData().getNonDefaultValues()
                                    through DataValue.write, then the byte 255:
                                    the payload of ClientboundSetEntityDataPacket
      rng_lo       i64   random.nextLong() after the chunk
      rng_hi       i64   random.nextLong() again
```

The file ends after the last placed chunk of the last case; there is no
trailer.

## What the fixture cannot pin

- **The real world's heights.** A layout here reads a flat floor. What a
  piece parameter fixed at layout is worth against carved, surfaced terrain
  is a question for a case over a noise world; `structure_pieces.bin` pins
  the layouts over the real world and this file pins what they write.
- **The `_WG` heightmap freeze.** The flat floor makes "heights answer the
  terrain" and "heights answer the base" the same thing; a carved base would
  tell them apart.
- **The neighbour-shape pass and the post-processing marks.** Switched off
  and swallowed, as above.
- **The masked entity fields.** Nothing here says what the shulker's yaw or a
  drowned's attribute rolls are.
- **The nether fossil's dried ghast.** `NetherFossilPiece.placeDriedGhast`
  draws a coin, a cell on the fossil's floor row and a rotation from a
  positional fork of the world seed at the fossil's box centre, and writes
  the ghast only where that cell is air. The site puts the floor row inside
  the top layer of the base (air above sturdy ground), so on the flat floor
  the cell is never air and the branch never writes; a base whose top varies
  per column would pin it. The port draws the same values and writes the
  ghast from every column the fossil touches against that column's own view,
  where the reference writes it once from the first chunk to decorate and
  the rest see it; the two agree on the world and differ only in which
  chunk's write list carries it, which this file would show on such a base.
- **Which chunk a piece's shared draws land in.** The reference draws an
  entity's `finalizeSpawn` in whichever chunk holds its marker; the file
  records that chunk. A port that moves a draw to another column will differ
  in both columns' streams.
- **The beached shipwreck's placement-stream draw.** Replaced above by the
  layout stream's; the reference's own value for it is in no fixture.
