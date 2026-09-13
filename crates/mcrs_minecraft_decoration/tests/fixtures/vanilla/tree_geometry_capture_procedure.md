# Fixture Capture Procedure — `tree_geometry.bin`

**Source of truth:** vanilla `26.3-snapshot-10`, `world_version` 5015, read through
Fabric Loom's mapped jar. No server and no client is started.

**Harness:** `tools/vanilla-oracle/src/main/java/mcrs/oracle/{TreeOracle,StubLevel}.java`

```sh
cd tools/vanilla-oracle
./gradlew dumpTrees --console=plain --no-daemon \
    -PoracleOut=../../crates/mcrs_minecraft_decoration/tests/fixtures/vanilla
```

Output is deterministic: re-running produces a byte-identical file.

**Consumer:** `crates/mcrs_minecraft_decoration/tests/tree_geometry_parity.rs`,
which asserts, per case, the return value, every written position and state in
**first-write order**, and the two `nextLong` values the source yields
afterwards. The last of those pins the draw count, so a diverging number of
draws fails even when the blocks happen to agree.

**Cases:** all 45 shipped `minecraft:tree` features at seeds 42, 1, 7 and 12345,
placed at (0, 64, 0) — 180 cases. 19 of the 45 are covered by the Rust test; the
rest write blocks its hand-built block table does not model.

---

## Not a lift

Unlike `ore_vein.bin`, nothing here is copied. `TreeFeature.place`, all ten trunk
placers, all twelve foliage placers, every decorator, every state provider,
`FeatureSize`, `FoliagePlacer.tryPlaceLeaf` and `TreeFeature.updateLeaves` are
the real 26.3 classes, constructed by the real codecs and called. Lifting them
verbatim would be a port of its own; the level is stubbed instead.

`StubLevel` starts with every `WorldGenLevel` method throwing
`UnsupportedOperationException`, and `TreeOracle` catches that and skips the
feature by name. The eighteen methods below are the ones a tree turned out to
call — discovered by running, not designed. Every other method still throws, so
no level state is silently standing in.

## The feature is the one a data pack carries

`TreeOracle` does not place the object `VanillaRegistries` builds. It encodes it
through `Feature.DIRECT_CODEC` and parses it back, which is the value a server —
vanilla or ours — gets from `data/minecraft/worldgen/feature/*.json`.

The two differ. `CherryFoliagePlacer.CODEC` reads `corner_hole_chance` into the
field but writes `wide_bottom_layer_hole_chance` back out, so the shipped
`cherry.json` holds 0.25 where `TreeFeatures.createCherry` holds 0.5. Placing
the bootstrap object would measure a cherry tree no world ever grows.

## Provenance Map

One row per stubbed level method. The reviewer uses this table to verify that
none of them is standing in for something that could move a block.

| `StubLevel` method | Stands in for | Implementation |
|---|---|---|
| `getBlockState` | the chunk's palettes | air outside `[-64, 320)`, else the last `setBlock` at that position, else `minecraft:dirt` at y ≤ 63 and `minecraft:air` above |
| `setBlock(pos, state, flags, limit)` | `WorldGenRegion.setBlock` | records the write and returns true |
| `isStateAtPosition`, `isFluidAtPosition`, `getFluidState` | the same on `LevelReader` | the predicate against `getBlockState` / its `getFluidState` |
| `dimensionType` | the level's dimension | `minecraft:overworld`, which is where the inherited `getMinY` and `getHeight` come from |
| `getHeight(type, x, z)` | the chunk's live heightmap | scans down from `getMaxY` for the first state `Heightmap.Types.isOpaque()` accepts — the real predicate, over the live world, so `place_on_ground` sees the tree it just grew |
| `getSeaLevel` | the level's sea level | 64, the overworld's, one above the floor |
| `getRandom` | `WorldGenRegion`'s own source | a **separate** `XoroshiroRandomSource(0)`. The region's random is not the feature's, and `updateShape` draws from it through `updateShapeAtEdge`; sharing one source would corrupt the draw count |
| `registryAccess` | the server's registries | the built-in registries, for the decorators that look one up |
| `getBlockEntity` | the chunk's block entities | `EntityBlock.newBlockEntity` for the state at that position, memoised. `BeehiveDecorator` draws its occupants inside `ifPresent`, so returning null would silently drop draws |
| `getBlockTicks`, `getFluidTicks`, `nextSubTickCount`, `getGameTime` | the level's tick containers | a counting no-op |

Block tags are the real ones: `TagLoader.loadTagsForExistingRegistries` binds
them from the vanilla data pack before any feature runs, so `#logs`,
`#leaves`, `#replaceable_by_trees`, `#dirt` and
`#cannot_replace_below_tree_trunk` answer as they do on a server.

**Dropped level state**, and why it cannot move a block:

- `ensureCanWrite` keeps its `true` default. In a real pass it clips writes to
  the 3×3 chunk region the feature may touch; the dump is deliberately the
  unclipped tree, and the clip is tested separately.
- Scheduled ticks are counted and discarded. `LeavesBlock.updateShape` schedules
  a decay tick; it is read back only when the chunk is loaded and its leaves
  decay, which is after generation, and scheduling draws nothing.
- Lighting, entities and chunk status are never read by a tree.

## What the fixture cannot pin

Two outcomes are `java.util.HashSet` iteration order rather than the reference's
arithmetic, and no port reproduces them without porting `java.util.HashMap`:

- **A leaf's `distance`.** `TreeFeature.updateLeaves` pops from
  `List<Set<BlockPos>> toCheck` and does not re-check the shape on pop, so a
  position queued at two distances is written twice and the *last* write wins.
  Which is last is the set's order. The consumer blinds `distance` and compares
  the block and every other property.
- **Where a beehive lands on a branching tree.** `TreeDecorator.Context` sorts
  its logs by y alone, over a list built from a `HashSet`, so logs sharing the
  hive's y keep the set's order and `Util.shuffle` shuffles an already-arbitrary
  list. It bites only when several logs share one y: `cherry_bees_005` is left
  out of the consumer for it, while every straight-trunk `*_bees_*` feature is
  covered.
