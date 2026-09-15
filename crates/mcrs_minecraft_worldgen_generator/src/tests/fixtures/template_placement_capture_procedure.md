# Fixture Capture Procedure — `template_placement.bin`

**Source of truth:** vanilla `26.3-snapshot-10`, `world_version` 5015, read through
Fabric Loom's mapped jar. No server and no client is started. The data pack is
the jar's own, loaded the way a server loads it (`PlacementOracle.loadWorldRegistries`),
the templates come through a real `StructureTemplateManager` over a temporary
save directory, and every placement runs the real `StructureTemplate.placeInWorld`
against a `StubLevel` whose only state is a flat floor and the blocks placement
writes.

**Harness:** `tools/vanilla-oracle/src/main/java/mcrs/oracle/TemplatePlacementOracle.java`
(`StubLevel.java` supplies the level).

```sh
cd tools/vanilla-oracle
./gradlew dumpTemplatePlacement --console=plain --no-daemon \
    -PoracleOut=../../crates/mcrs_minecraft_worldgen_generator/src/tests/fixtures
```

Output is deterministic: two consecutive runs produce a byte-identical file
(5 376 220 bytes). The run log must contain no `Serialization errors` line —
`placeInWorld` reports block-entity load problems through its logger instead
of throwing, so a hit means a compound in the fixture was not loaded the way
the fixture claims. The capture that produced this file had none.

**Consumer:** `crates/mcrs_minecraft_worldgen_generator/src/tests/template_parity.rs`,
which compiles every processor chain through the real resolver (the ruined
portal chains from the properties their case key spells), places each case
into a box region with the same floors, and asserts the written-block hash
(or full list), the block-entity compounds, and the placement random's state
afterwards; and pins the three censuses that precede the cases.

The byte layout is in `tools/vanilla-oracle/README.md` under "Template
placement dumps".

---

## Not a lift

Pool cases go through `SinglePoolElement.place(templates, level, null, null,
position, reference, rotation, clip, random, liquid, false)` — the public entry
point a `PoolElementStructurePiece` calls. The null `StructureManager` and
`ChunkGenerator` are never read by `SinglePoolElement`; the data-marker pass it
runs after `placeInWorld` is a no-op for every pool element (`handleDataMarker`
is empty on `StructurePoolElement`). The settings are therefore exactly
`SinglePoolElement.getSettings` (or `LegacySinglePoolElement.getSettings`,
which pops the leading `block_ignore(structure_block)` and appends
`block_ignore(air, structure_block)`), including `setKnownShape(true)`.

Feature cases cannot call `TemplateFeature.place`: it reaches for
`level.getLevel().getServer().getStructureTemplateManager()`. The oracle
re-expresses it draw for draw — `templates.getRandomOrThrow(random)`,
`Util.getRandom(entry.rotations(), random)`, the half-size offsets along the
rotated west and north, `new StructurePlaceSettings().setRotation(rotation).setRandom(random)`
plus the feature's processors, `placeInWorld(level, pos, pos, settings, random, 3)` —
with one deliberate deviation: `setKnownShape(true)`. The feature leaves
`knownShape` false, which makes `placeInWorld` run `updateFromNeighbourShapes`
over every placed block afterwards; that pass is not reproduced by the port
and would need a full block-shape model in the stub, so it is skipped here and
listed below as something the fixture cannot pin.

Fossil cases are a lift: `FossilFeature.place(level, null, random, origin)`
with the origin the feature cases use, against a `StubLevel` whose
`getLevel()` is a `StubServerLevel` over `StubLevel.server(registries,
templates)`, so `getServer().getStructureTemplateManager()` answers with the
real manager. The chunk generator is never read. The reference leaves
`knownShape` false here, and the neighbour-shape pass runs in the stub: bone
blocks and ores have no shape to update, so it writes nothing, and the pass
is still listed below as something the fixture cannot pin. A fossil case
places eight seeds rather than three, so that more than one of the eight
template pairs and every rotation is drawn.

Portal cases place the thirteen `ruined_portal/*` templates the way a
`RuinedPortalPiece` does, minus the piece's own post-processing. The settings
come from the piece's private static `makeSettings(registries, mirror,
rotation, verticalPlacement, pivot, properties)`, reached by reflection so the
chain is the reference's own — `block_ignore(structure_block)` or
`block_ignore(air, structure_block)` by `airPocket`, the gold, lava and
netherrack rules, `block_age(mossiness)`,
`protected_blocks(#features_cannot_replace)`, `lava_submerged_block`, and
`blackstone_replace` when `replaceWithBlackstone` — with the pivot the
structure computes, `(size.x / 2, 0, size.z / 2)`. Then
`placeInWorld(level, position, reference, settings, random, 2)`, the call
`TemplateStructurePiece.postProcess` makes; no clip, which is what the
portal's `chunkBB.encapsulate(boundingBox)` amounts to. The same deliberate
deviation as the feature cases applies: `setKnownShape(true)`, because the
piece leaves `knownShape` false and the neighbour-shape pass is not
reproduced. `spreadNetherrack`, the drip columns, vines and leaves run after
`placeInWorld` in the piece and are not part of these cases.

The `processors` field of `SinglePoolElement` is protected and read by
reflection, only to derive the case key. A pool element whose inline list is
non-empty aborts the dump; the shipped corpus has none. Feature nodes may carry
a non-empty inline list (`desert_well`'s two `suspicious_sand` nodes carry the
`append_loot` rule inline and share the key `inline`).

`StubLevel` is not a `ServerLevel`. It answers `getSeed()` with the constant
`0x5EED` for every case, and its `getLevel()` returns a `ServerLevel` subclass
allocated through `Unsafe.allocateInstance` — no constructor runs, no server
exists — that overrides `getSeed()` (the same constant) and throws from
`enabledFeatures()` and `getNextEntityId()`. Only
`CappedProcessor.finalizeProcessing` reads `getLevel().getSeed()`; the parity
program is built with world seed `0x5EED` so its capped chains fork the same
positional random. Template entities are built by
`createEntityIgnoreException`, whose first level access is
`EntityType.canSpawn` reading `enabledFeatures()` (and `Entity`'s constructor
calling `getNextEntityId()` right after); every one of them fails inside that
method's `catch (Exception)` and `placeEntities` spawns nothing, exactly as
the port skips them. `placeEntities` draws nothing from the placement random,
so `rng_lo`/`rng_hi` are unaffected.

Two more things a real server has that the stub had to supply: item
components are bound (`DATA_COMPONENT_INITIALIZERS.build(access).forEach(apply)`,
what `ReloadableServerResources` does after the registries load) because
`VaultConfig`'s default key item is an `ItemStack`; and `enabledFeatures()`
returns `FeatureFlags.DEFAULT_FLAGS`, the filter `JigsawReplacementProcessor`
parses `final_state` through. Particles, sounds, level events and game events
are swallowed the way `WorldGenRegion` swallows them (a lit candle placed into
water is extinguished by `CandleBlock.placeLiquid` and emits smoke).

Floors: `0` is dirt for `y <= 63`, air above; `1` is stone for `y <= 60`,
source water for `61..=63`, air above; `2` (portal cases only) is stone for
`y <= 60`, source lava for `61..=63`, air above, so `lava_submerged_block`
sees lava under the piece. Every placement gets a fresh
`StubLevel` and a fresh `XoroshiroRandomSource(seed)`; the level's own random
(`getRandom()`) is a `XoroshiroRandomSource(0)` that nothing in placement
draws from.

## Provenance Map

| Dumped value | Vanilla source |
|---|---|
| `type_ids` | `BuiltInRegistries.BLOCK_ENTITY_TYPE` iteration order, `getKey(type)` |
| `block_id`, `type_id`, `loot_seeded` | every `EntityBlock` in `BuiltInRegistries.BLOCK` order; `newBlockEntity(BlockPos.ZERO, defaultBlockState())`, its `getType()`, `instanceof RandomizableContainer` |
| rotation rows | every `getStateDefinition().getPossibleStates()` of every block; `state.rotate(CLOCKWISE_90)`, `rotate(CLOCKWISE_180)`, `rotate(COUNTERCLOCKWISE_90)`, `mirror(LEFT_RIGHT)`, `mirror(FRONT_BACK)`; a row only when any differs; strings via `BlockStateParser.serialize` |
| pool case key | `getTemplateLocation()`, the `processors` holder's key (`ref:` + id) or `inline`, `getProjection()`, `instanceof LegacySinglePoolElement` |
| pool case order | `Registries.TEMPLATE_POOL` sorted by `Identifier.toString()`, `getTemplates()` raw pairs in order, `ListPoolElement.getElements()` in order, first occurrence of a key |
| `rotation`, `liquid` | `Rotation.values()[k % 4]`; `k % 8 == 7 ? IGNORE_WATERLOGGING : APPLY_WATERLOGGING` |
| `mirror`, `pivot` | kind 0 and 1: `NONE`, `BlockPos.ZERO` (what `SinglePoolElement` and `TemplateFeature` leave on the settings); kind 2: `Mirror.values()[k % 3]` and `(size.x / 2, 0, size.z / 2)` of the template, the portal structure's pivot |
| portal case key | `portal:` + `VerticalPlacement.getSerializedName()`, `cold`, `air_pocket`, `mossiness` (`Float.toString`), `blackstone` from the `Properties` record |
| portal case order | the thirteen templates in `STRUCTURE_LOCATION_PORTALS` then `STRUCTURE_LOCATION_GIANT_PORTALS` order, six setups each; over the running portal index `k`: placement `values()[k % 6]`, cold `k % 9 < 4`, mossiness `{0, 0.2, 0.5, 0.8, 1}[k % 5]`, air pocket `k % 7 < 3`, blackstone `k % 11 < 4`, `overgrown` and `vines` false (they act after `placeInWorld`) |
| `reference` (kind 2) | `template.getBoundingBox(settings, position)` with the case's mirror, rotation and pivot: `(center.x, minY, center.z)` |
| `reference` | `template.getBoundingBox(new StructurePlaceSettings().setRotation(rotation), position)`: `(center.x, minY, center.z)` — what `PoolElementStructurePiece.place` passes |
| `clip` | `(0, minY + 1, 0, 15, minY + height - 1, 15)` of the overworld `DimensionType`, the chunk box `ChunkGenerator.applyBiomeDecoration` hands a piece |
| feature case key | `TemplateFeature.templates().unwrap()` entry ids joined by `,`; `processors()` holder key, `inline`, or `none`. A `FossilFeature`: `fossilStructures()` joined by `,`, `;`, `overlayStructures()` joined by `,`; `fossilProcessors()` key, `;`, `overlayProcessors()` key |
| feature case order | `desert_well`, `sulfur_spring`, `fossil_coal` then `fossil_diamonds`; `Stream.concat(Stream.of(holder), holder.value().getSubFeatures())` filtered to `TemplateFeature` and `FossilFeature`; first occurrence of a key |
| `template_drawn`, `rotation_drawn`, `x, y, z` (kind 1) | the draws and offset `TemplateFeature.place` makes, recorded as diagnostics; a fossil: `Rotation.getRandom` and the `nextInt` index replayed on a fresh random of the same seed, and the origin |
| `placed` | the boolean `place` / `placeInWorld` returned |
| `count`, `hash`, `entries` | `StubLevel.written()`: every position `setBlock` touched, first-write order, final state (a block-entity position is written twice — barrier, then the state — and keeps its first-write slot) |
| block entities | `StubLevel.blockEntities()`: the instances `getBlockEntity` created for `placeInWorld`, after `loadWithComponents`; `saveWithFullMetadata(access)` through `NbtIo.write` |
| `rng_lo`, `rng_hi` | `random.nextLong()` twice after placement |

## Cases

1469 cases, 3068 placements, 726 757 written positions in total, 1693
distinct written states, 3304 block entities; 85 placements carry the full
list. Censuses: 49 block-entity types; 190 entity blocks, of which 33 create a
`RandomizableContainer` (`chest` and the eight copper chests → `chest`,
`trapped_chest`, `barrel`, `dispenser`, `dropper`, `hopper`, `crafter`,
`decorated_pot`, `shulker_box` and its sixteen colours); 30 206 block states
that some rotation or mirror changes, over 587 blocks (18 668 of them mirror,
the rest rotate only: axis blocks, the anvils, and every `facing` off the
mirrored axis).

Pool cases (1383) by template family: `village` 573, `abandoned_camp` 297,
`trial_chambers` 193, `bastion` 167, `trail_ruins` 84, `ancient_city` 58,
`pillager_outpost` 11. 881 are legacy elements, 174 use
`terrain_matching` (so carry the gravity processor), 172 place with
`ignore_waterlogging`. 651 have an empty inline processor list; the rest
reference 36 distinct shipped lists, the most used being
`trail_ruins_houses_archaeology` (72), `street_snowy_or_taiga` (68),
`mossify_10_percent` (53), `stable_degradation` (51) and
`trial_chambers_copper_bulb_degradation` (46).

The one template the pack references but does not ship,
`ancient_city/walls/intact_horizontal_wall_stairs_5`, is case 353: the
manager substitutes an empty template, both placements return `placed = 0`
with nothing written and the random untouched.

Feature cases (8): `desert_well/well` (no processors),
`desert_well/suspicious_sand` (the inline `append_loot` rule; the two nodes
share the key), the four `sulfur_spring` size classes, each a weighted
list of one to four templates over all four rotations, no processors, and
the two fossils, `fossil_coal` and `fossil_diamonds`, which share the eight
fossil templates and the `fossil_rot` chain and differ in the overlay
templates' chain (`fossil_coal` rots the coal ore to a tenth;
`fossil_diamonds` also rules it into deepslate diamond ore). The eight seeds
draw `spine_2`, `spine_3`, `skull_3` and `skull_4` under every rotation; all
32 placements place, their corner 15 to 24 blocks below the lowest
`OCEAN_FLOOR_WG` under the footprint, which is 64 on the dirt floor and 61 on
the stone one (water does not block motion).

Portal cases (78): thirteen templates times six setups, three floors each,
234 placements, all placed; 136 851 written positions, 41 full lists (one
setup per template, the cases whose index is divisible by six, plus the
running-index hundreds). Every rotation, every mirror (26 `none`, 26
`left_right`, 26 `front_back`), every vertical placement and every shipped
mossiness occurs, with and without the cold, air-pocket and blackstone flags.
The block entities are 204 chests and 90 jigsaw blocks: a ruined portal is
not a jigsaw piece, so its templates' jigsaw block is written as it is and
loads a `minecraft:jigsaw` block entity. A chest that `lava_submerged_block`
turns into lava loads nothing and draws no seed. The full lists carry the
aged, blackstone-replaced and lava-submerged states — cracked and mossy
stone bricks, mossy slabs, stairs and walls, crying obsidian, every polished
blackstone family member, iron chains, magma and lava.

Block entities placed, by type: `brushable_block` 1258, `chest` 600, `barrel`
436, `campfire` 226, `banner` 174, `furnace` 68, `bell` 54, `blast_furnace`
34, `smoker` 30, `brewing_stand` 28, `trial_spawner` 28, `lectern` 22,
`copper_golem_statue` 16, `dispenser` 10, `decorated_pot` 8, `sign` 8,
`vault` 4, `creaking_heart` 2, `hopper` 2, `skull` 2. Every `LootTable` in the
file is accompanied by a `LootTableSeed` (2268 of each). No `mob_spawner`,
`comparator` or `sculk_sensor` fell inside a case's clip box, so those shapes
are pinned only by their own unit tests.

## What the fixture cannot pin

- **`WorldgenRandom` semantics.** The placement random here is a raw
  `XoroshiroRandomSource`, as in every other fixture. A real server hands the
  piece a `WorldgenRandom`, whose `nextInt`/`nextLong` route through
  `next(bits)`; that is a project-wide convention, not something this fixture
  decides.
- **Reads outside the column.** `protected_blocks` and a rule's
  `location_predicate` read the level at the target position. The stub is a
  flat floor, so those reads see either the floor or a block this same
  placement wrote; a real server can see a neighbouring column's earlier
  pieces.
- **Start order and multi-piece interaction.** Each case is one element placed
  alone. Which piece of which start writes first into a shared column, and
  what a later piece sees of an earlier one, is not exercised.
- **Template entities.** None is spawned (see above), so nothing pins the
  entity list or what `finalizeSpawn` would do.
- **The neighbour-shape post pass** of `minecraft:template` and
  `minecraft:fossil` features and of the ruined portal piece (`knownShape = false`). With the deliberate
  `setKnownShape(true)`, a sulfur spike at a template's edge keeps its file
  state and a portal's walls, fences and iron bars keep their template
  connections; a real server recomputes them against the terrain.
- **The ruined portal's own post-processing.** `spreadNetherrack`, the drip
  columns, vines and leaves draw from the piece random after `placeInWorld`
  and are not in these cases; the portal cases pin the chain and the
  transform only.
- **Feature-flag filtering of `final_state`.** `DEFAULT_FLAGS` is the vanilla
  set; a world with experimental packs enabled would parse the same strings
  through a wider lookup, which changes nothing for the shipped corpus.
- **Block-entity load leniency.** `loadWithComponents` reports problems rather
  than throwing; the capture log is the only evidence that none occurred.
