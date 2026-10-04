# Fixture Capture Procedure — `ore_vein.bin`

**Source of truth:** the game version that `assets/minecraft/version.json`
states, read through Fabric Loom's mapped jar. The fixture header holds the
world version of that file, and `tools/captures.json` records the version id the
fixture was captured at. No server and no client is started.

**Harness:** `tools/vanilla-oracle/src/main/java/mcrs/oracle/OreOracle.java`

```sh
cargo run -p mcrs_minecraft_update -- recapture ore_vein
```

Output is deterministic: re-running produces a byte-identical file.

**Consumer:** `crates/mcrs_minecraft_worldgen_feature_place/tests/it/ore_vein_parity.rs`, which
asserts, per case, the return value, every written position and state in write
order, and the two `nextLong` values the source yields afterwards. The last of
those pins the draw count, so a diverging number of draws fails even when the
blocks happen to agree.

---

## Provenance Map

Nothing is lifted. The harness holds no copy of any block of the game's code, so
there is no source line range to keep in step with a new snapshot.

| Harness symbol | Source file | Mechanical edits applied |
|---|---|---|
| `feature.place(...)`, which reaches `doPlace` | `world/level/levelgen/feature/OreFeature.java` | None: the real `OreFeature` instance runs, unedited, on the stub level below. |
| `canPlaceOre(...)`, `isAdjacentToAir`, `checkNeighbors`, `shouldSkipAirCheck` | `world/level/levelgen/feature/AbstractOreFeature.java` | None: called by the placement on the real instance. |
| The rule tests | `world/level/levelgen/structure/templatesystem/{AlwaysTrueTest,BlockMatchTest,RandomBlockMatchTest}.java` | None: real instances. |
| The chunk sections | `world/level/chunk/{LevelChunkSection,ProtoChunk,BulkSectionAccess}.java` | None: the game's own classes; the harness subclasses `LevelChunkSection` only to record writes. |

**Dropped world state** (provably cannot affect the recorded blocks): lighting,
block entities, neighbour updates and chunk status, none of which the vein reads
or writes.

---

## The stub level

The level is a `java.lang.reflect.Proxy` over `WorldGenLevel`. Its handler
answers four methods itself, by name and parameter types:

| Method | Answer |
|---|---|
| `getMinY()` | -64 |
| `getHeight()` | 384 |
| `getHeight(Heightmap.Types, int, int)` | 320, the same height for every column, so the probe always succeeds and the probe box is not covered by the dump |
| `getChunk(int, int, ChunkStatus, boolean)` | the stub chunk of that position, built on first use and kept for the case |

`hashCode`, `equals` and `toString` are answered for the proxy itself. Every
other default method of the interface runs its own body through
`InvocationHandler.invokeDefault`: that is how `anyHeightMatches`, `getMaxY`,
`getSectionIndex`, `getSectionsCount`, the two-argument `getChunk` and
`ensureCanWrite` run the game's code. Any other abstract method throws
`UnsupportedOperationException` naming it, so a harness that has fallen behind
the game stops the capture. The dump is assembled in memory and written only
after every case ran, so a stopped capture leaves no file.

A stub chunk is a `ProtoChunk` over 24 sections. Each section is a subclass of
`LevelChunkSection` that knows its position and the case's placement list; it is
filled with stone first, and from then on its five-argument `setBlockState`
appends the world position and the state to the list before delegating to the
game's own method. The world a case sees is stone from y -64 to 319 and air
elsewhere, because the game's section access answers air where no section exists.

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
