# Fixture Capture Procedure — `ore_vein.bin`

**Source of truth:** vanilla `26.3-snapshot-10`, `world_version` 5015, read through
Fabric Loom's mapped jar. No server and no client is started.

**Harness:** `tools/vanilla-oracle/src/main/java/mcrs/oracle/OreOracle.java`

```sh
cd tools/vanilla-oracle
./gradlew dumpOreVeins --console=plain --no-daemon \
    -PoracleOut=../../crates/mcrs_minecraft_worldgen_feature/tests/fixtures/vanilla
```

Output is deterministic: re-running produces a byte-identical file.

**Consumer:** `crates/mcrs_minecraft_worldgen_feature/tests/ore_vein_parity.rs`, which
asserts, per case, the return value, every written position and state in write
order, and the two `nextLong` values the source yields afterwards. The last of
those pins the draw count, so a diverging number of draws fails even when the
blocks happen to agree.

---

## Provenance Map

One row per lifted code block. The reviewer uses this table to verify
verbatim-ness against the mapped sources.

| Harness symbol | Source file | Source lines | Mechanical edits applied |
|---|---|---|---|
| `place(...)` | `world/level/levelgen/feature/OreFeature.java` | 38–70 | `level.getHeight(OCEAN_FLOOR_WG, x, z)` → the constant `MAX_Y`. The harness world is stone everywhere with no heightmap, so the probe always succeeds; the probe box itself is therefore *not* covered by this dump and is pinned separately in `the_probe_box_is_the_reference_box`. Everything before the probe — the three draws and all of the segment arithmetic — is unmodified. |
| `doPlace(...)` | `world/level/levelgen/feature/OreFeature.java` | 72–198 | `BulkSectionAccess` and `LevelChunkSection` replaced by a `Map<BlockPos, BlockState>` defaulting to stone; `level.isOutsideBuildHeight(y)` → the same test against `MIN_Y = -64` / `MAX_Y = 320`; `level.ensureCanWrite` and the `section != null` guard dropped (every position is writable). No numeric or draw-order edit. `this.size` read through `feature.size()`. |
| `canPlaceOre(...)`, `isAdjacentToAir`, `checkNeighbors`, `shouldSkipAirCheck` | `world/level/levelgen/feature/AbstractOreFeature.java` | 59–100 | None — called on the real `OreFeature` instance. |
| The rule tests | `world/level/levelgen/structure/templatesystem/{AlwaysTrueTest,BlockMatchTest,RandomBlockMatchTest}.java` | — | None — real instances. |

**Dropped world state** (provably cannot affect the recorded blocks): lighting,
block entities, neighbour updates and chunk status, none of which the vein reads
or writes.

---

## Cases

21 cases. The first twelve cover the size range, the target kinds and the two
build-height clips; the rest were each added to make one otherwise-invisible
part of the algorithm observable in a world that is stone everywhere.

| Case | What it is there for |
|---|---|
| `size0`, `size1`, `size9`, `size17`, `size33`, `size64` | the size range, including the empty vein |
| `size12_always_true`, `size20_random_match` | the non-trivial rule tests; `random_block_match` draws only after its block matches |
| `size8_discard_half` | the fractional air-exposure draw |
| `size16_two_targets` | a first target that never matches |
| `size24_clipped_at_top`, `size24_clipped_at_bottom` | the build-height clips |
| `size32_sin_index`, `size64_sin_index_a`, `size64_sin_index_b` | seeds where `Mth.sin`'s **double** table index and the Beta **float** index disagree by one entry, and the disagreement moves a block |
| `size18_sin_table`, `size30_sin_table` | seeds where the sine **table** and a real `Math.sin` disagree enough to move a block |
| `ceiling_always_true`, `floor_always_true` | a target that matches everything at each build-height boundary, so dropping the clip writes outside the world |
| `ceiling_discard_half` | candidates genuinely adjacent to air (the void above y 320), so the sense of the `>=` in `shouldSkipAirCheck` is observable |
| `two_matching_targets` | two targets that both match, so the `break` after the first is observable |

The four horizontal neighbours of the air probe are **not** covered: the only air
in this world is above and below the build-height boundary, so a horizontal
neighbour is never air. Covering them needs a harness world with a cave in it.
