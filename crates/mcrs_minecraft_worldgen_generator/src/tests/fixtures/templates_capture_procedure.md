# Fixture Capture Procedure — `templates.bin`

**Source of truth:** vanilla `26.3-snapshot-10`, `world_version` 5015, read through
Fabric Loom's mapped jar. No server and no client is started. The templates are
the jar's own `data/minecraft/structure/**/*.nbt` (1511 files, every one already
at `DataVersion` 5015), opened through the vanilla pack's `ResourceManager`.

**Harness:** `tools/vanilla-oracle/src/main/java/mcrs/oracle/TemplateOracle.java`

```sh
cd tools/vanilla-oracle
./gradlew dumpTemplates --console=plain --no-daemon \
    -PoracleOut=../../crates/mcrs_minecraft_server/src/world/generate/tests/fixtures
```

Output is deterministic: re-running produces a byte-identical file (4 620 732
bytes).

**Consumer:** `crates/mcrs_minecraft_server/src/world/generate/tests/template_manifest.rs`,
which freezes the shipped `assets/minecraft/structure` tree — the jar's own
structure tree, every file at `DataVersion` 5015 — and asserts, for every one
of the 1511 templates, the size, the palette count,
the three section lengths of the block ordering, that every file palette entry
resolves to the same state, and every jigsaw field by field; and for the 33
listed templates, the full ordered block list per palette and which positions
carry a block entity.

---

## Not a lift

Nothing is copied. `ResourceManagerTemplateSource.load` is the real loader a
server uses: it reads the compressed tag, runs
`DataFixTypes.STRUCTURE.updateToCurrentVersion` over the real
`DataFixers.getDataFixer()` (a no-op here, since the files are already at the
jar's version), and calls `StructureTemplate.load` against
`BuiltInRegistries.BLOCK`. `StructureTemplateManager` itself is not built:
it insists on a world save directory for `generated/` templates, and the only
source that matters for the shipped corpus is the resource-manager one it
delegates to. The `palettes` list is read by reflection because
`StructureTemplate` has no public accessor for it; every value written comes
from `Palette.blocks()`, `Palette.jigsaws()` and `StructureTemplate.getSize()`.

The file palette entries are the one thing read twice: the oracle re-opens the
`.nbt`, takes the `palette` / `palettes` lists off the raw tag and resolves each
entry with `NbtUtils.readBlockState`, the same call `loadPalette` makes.
`readBlockState` is lenient — an unknown block becomes air, an unknown property
is skipped, an unparsable value keeps the default — so the serialized strings
are how the consumer checks that none of that leniency fires on the shipped
corpus.

## Provenance Map

| Dumped value | Vanilla source |
|---|---|
| `size` | `StructureTemplate.getSize()` |
| `palette_count` | `StructureTemplate.palettes.size()` — 1, or 8 for the twenty `shipwreck/*` templates |
| `full_count`, `other_count`, `entity_count` | the three lists `StructureTemplate.addToLists` fills: `nbt != null` → entity; else `!hasDynamicShape() && isCollisionShapeFullBlock(EmptyBlockGetter.INSTANCE, BlockPos.ZERO)` → full; else other |
| `entries` | `BlockStateParser.serialize(NbtUtils.readBlockState(BLOCK, entry))` over the file palette list, in file order |
| jigsaw `pos`, `state`, `joint`, `name`, `pool`, `target`, priorities | `Palette.jigsaws()` = `JigsawBlockInfo.parse` over `blocks(Blocks.JIGSAW)`, in that order |
| jigsaw `front`, `top` | `JigsawBlock.getFrontFacing` / `getTopFacing` of the state's `orientation` |
| jigsaw `final_state_raw` | `nbt.getStringOr("final_state", "minecraft:air")`, the literal file string |
| jigsaw `final_state` | `BlockStateParser.parseForBlock(BLOCK, raw, true)` serialized, or `""` when it throws — the parse `JigsawReplacementProcessor` runs at placement |
| listed `blocks` | `Palette.blocks()` in list order: full blocks, then other blocks, then block entities, each section sorted by y, x, z |
| listed `nbt_bits` | bit *i* set when `blocks().get(i).nbt() != null` |
| `dynamic_ids` | every block in `BuiltInRegistries.BLOCK` order whose `hasDynamicShape()` is true (23) |

## Cases

The manifest covers all 1511 templates: 20 multi-palette, 4272 jigsaws, 526 012
full blocks, 702 550 other blocks, 6213 block entities across all palettes.

The 33 listed templates (173 palettes, 114 812 block records) and what each pins:

| Template | Pins |
|---|---|
| `shipwreck/*` (20, 8 palettes each) | per-palette ordering where degraded palettes swap planks for air or water; the only `palettes` files in the corpus |
| `empty` | the degenerate case: size [1,1,1], one air block, one palette entry |
| `village/plains/houses/plains_small_house_1` | a 7×7×7 house with 2 jigsaws and a 24-entry palette |
| `village/savanna/savanna_lamp_post_01` | a 1×2×1 piece with no full block at all and a `final_state` naming every property of `acacia_fence` |
| `village/desert/houses/desert_small_house_1` | a 6×6×5 house with 2 jigsaws where the other section (97) outnumbers the full one (81) |
| `trial_chambers/corridor/entrance_1` | 7220 blocks, 3 jigsaws |
| `trial_chambers/hallway/encounter_4` | `pointed_dripstone`, a dynamic-shape block, lands in the other section |
| `trial_chambers/chamber/chamber_2` | 13 224 blocks, 31 jigsaws, `powder_snow` |
| `bastion/units/center_pieces/center_0` | 10 jigsaws in one palette |
| `ancient_city/city_center/city_center_1` | a 96-entry palette |
| `pillager_outpost/watchtower` | a tall (21) template with 1 jigsaw |
| `abandoned_camp/camp/snowy_taiga/campsite_snowy_taiga_4` | `powder_snow` plus entities in the file |
| `trail_ruins/tower/tower_top_1` | 46 blocks, the smallest jigsaw piece listed |
| `spring/sulfur_spring_medium_1` | `sulfur_spike`, a dynamic-shape block; no block entities and no jigsaws, placed by a `minecraft:template` feature |

## What the fixture cannot pin

- **`hasDynamicShape` as an ordering input.** The only dynamic-shape blocks in
  any shipped template are `powder_snow`, `pointed_dripstone` and
  `sulfur_spike`, and none of them has a full collision shape, so they fall in
  the other section for either reason. A shulker box would be the first block
  where the clause matters (full collision box, dynamic shape → other), and no
  template contains one. `dynamic_ids` lists the 23 ids so a port can pin the
  set without modelling it.
- **Trailing input after a `final_state`'s closing `]`.** `parseForBlock`
  ignores it, but no 5015 template carries any, so the corpus never exercises
  the clause; `final_state` and `final_state_raw` differ only where the raw
  string names none or only some of the block's properties.
- **The datafixer.** Every file is already at the jar's `DataVersion`, so the
  fixer runs but rewrites nothing.
