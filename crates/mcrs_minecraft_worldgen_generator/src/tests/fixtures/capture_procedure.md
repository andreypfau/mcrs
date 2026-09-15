# Fixture Capture Procedure — `beta_surface_corpus.json`

**Seed:** 12345
**Source of truth:** back2beta-server-1.7.9 (verbatim-extraction harness)
**Extraction type:** Verbatim — noise classes copied byte-for-byte, method bodies lifted verbatim with documented mechanical substitutions only.
**Cross-check:** moderner-beta live capture deferred (see below).

---

## Verbatim Extraction vs. Paraphrase

This corpus was produced by a **verbatim-extraction harness** built directly from the
back2beta-server-1.7.9 decompiled sources. It is NOT a paraphrase or re-implementation
of the Beta algorithm. The distinction matters because the parity gate that consumes this
corpus is only meaningful if the reference data was produced by the same algorithm the
gate tests against.

The harness is located at:
```
~/src/back2beta-server-1.7.9/.b2b-verbatim-harness/
```

Noise classes copied **byte-for-byte** (package declaration removed to compile without package
structure — the ONLY edit; verified by `diff`):

| Copied file | Source |
|---|---|
| `NoiseGenerator.java` | `net/minecraft/server/NoiseGenerator.java` |
| `NoiseGenerator2.java` | `net/minecraft/server/NoiseGenerator2.java` |
| `NoiseGeneratorOctaves.java` | `net/minecraft/server/NoiseGeneratorOctaves.java` |
| `NoiseGeneratorOctaves2.java` | `net/minecraft/server/NoiseGeneratorOctaves2.java` |
| `NoiseGeneratorPerlin.java` | `net/minecraft/server/NoiseGeneratorPerlin.java` |
| `MathHelper.java` | `net/minecraft/server/MathHelper.java` |

---

## Provenance Map

One row per lifted code block. The reviewer uses this table to verify verbatim-ness.

| Harness symbol | Source file | Source lines | Mechanical edits applied |
|---|---|---|---|
| `B2bCapture(long seed)` constructor | `ChunkProviderGenerate.java` | 30–41 | None to noise/RNG init. WorldChunkManager constructor (lines 18–21 of `WorldChunkManager.java`) inlined here. |
| `computeDensity(...)` | `ChunkProviderGenerate.java` | 206–304 | `this.p.getWorldChunkManager().temperature` → `wcmTemperature`; `.rain` → `wcmRain`. World object dropped (does not affect numeric output). |
| `fillDensityTerrain(...)` | `ChunkProviderGenerate.java` | 43–110 | `BiomeBase[]` parameter type → `int[]` (biome IDs). All numeric logic unmodified. |
| `replaceBlocksForBiome(...)` | `ChunkProviderGenerate.java` | 112–185 | `BiomeBase[]` → `int[]`; `biomebase.topBlock` → `BIOME_TOP[biomeId]`; `biomebase.fillerBlock` → `BIOME_FILL[biomeId]`. BIOME_TOP/FILL arrays populated from `BiomeBase.java` constructor (lines 49–50) plus `generateBiomeLookup` override (lines 86–87). |
| `generateChunk(int,int,long)` | `ChunkProviderGenerate.java` | 193–204 | `new Chunk(...)` dropped (no-op — byte[] already allocated). `chunk.initLighting()` dropped (does not write block IDs). `this.p.getWorldChunkManager().a(...)` → `getBiomeArray(...)`. |
| `getBiomeArray(...)` | `WorldChunkManager.java` | 68–111 | `BiomeBase[]` → `int[]` (biome IDs). `getBiomeFromLookup(d3,d4)` call preserved, returns `int` ID instead of `BiomeBase` reference. |
| `getBiome(float,float)` | `BiomeBase.java` | 120–132 | None — verbatim static logic. Returns `int` ID constant instead of `BiomeBase` reference. |
| `getBiomeFromLookup(double,double)` | `BiomeBase.java` | 114–118 | None. |
| BIOME_LOOKUP table init | `BiomeBase.java` | 79–88 | None — verbatim `generateBiomeLookup` logic. DESERT and ICE_DESERT top/fill overridden to SAND exactly as in lines 86–87. |
| `generateCaves(long,int,int,byte[])` | `MapGenBase.java` | 9–21 | `world.getSeed()` → `worldSeed` parameter. `IChunkProvider` and `World` parameters dropped. |
| `cavesA(int,int,int,int,byte[])` | `MapGenCaves.java` | 162–185 | `World` parameter dropped (not referenced in body). |
| `caveTunnel(...)` | `MapGenCaves.java` | 6–8 | None. |
| `caveTunnelFull(...)` | `MapGenCaves.java` | 10–158 | None — verbatim, including `MathHelper.sin/cos` against the byte-for-byte copied `MathHelper` sine lookup table. |

**Dropped World state** (provably does not affect block IDs in the captured stages):

- `chunk.initLighting()` — lighting computation, writes no block data.
- `getChunkAt(IChunkProvider,int,int)` — the decoration/population step (trees, ores, snow layers).
  This method runs *after* the two stages we capture (post-surface and post-cave) and places blocks
  only on the surface. It is entirely outside our capture scope.

---

## Per-Chunk RNG Seed Constants

From `ChunkProviderGenerate.java` line 194 (verbatim):

```java
this.j.setSeed(i * 341873128712L + j * 132897987541L);
```

Where `i` = chunk X coordinate, `j` = chunk Z coordinate.
The RNG draw order from this seeded state:
1. Three `nextDouble()` calls inside `replaceBlocksForBiome` (lines 122–124): `flag`, `flag1`, `i1`.
2. Per-Y-column inner loop: `nextInt(5)` for bedrock threshold; then conditional `nextInt(4)` for sandstone transition.

These draw orders are preserved verbatim in `replaceBlocksForBiome`.

---

## Capture Stages

Two stages per column are recorded, matching the two states from `getOrCreateChunk`:

- **pre_cave** — state of `abyte[]` after `replaceBlocksForBiome` returns, BEFORE the cave-carve call (`this.u.a(...)`, line 201 of `ChunkProviderGenerate.java`). This is the surface-parity gate input.
- **post_cave** — state of `abyte[]` after `generateCaves` (the cave-carve call) completes. Committed for the cave-parity gate.

---

## Cave Trig Fidelity (post-cave stage)

`MathHelper.sin/cos` in the original Beta server reads from a precomputed 65536-entry float
lookup table (`sin[i] = (float)Math.sin(i * PI * 2.0 / 65536.0)`), which is lower precision than
Java's double `Math.sin/cos`. The harness compiles `MathHelper.java` copied **byte-for-byte** from
the back2beta source (sine table included) and the cave carver calls that `MathHelper.sin/cos`
verbatim. The post-cave block arrays therefore match real Beta cave carving exactly — no
trig approximation.

The **pre_cave stage** captures state before any cave carving and is the surface-parity gate
input. The **post_cave stage** is committed for the future cave-parity gate.

---

## Captured Regions

### Near-Origin Square

Chunk coordinates: -2 through +1 in both X and Z (4x4 = 16 chunks = 4096 block-columns).
World block coordinates: wx -32..31, wz -32..31.

Biomes present: Savanna (4), Desert (7).

**Desert** (biome_id=7): 3047 of 4096 near-origin columns have sand/sandstone surface.
This exercises the `if (flag)` branch in `replaceBlocksForBiome` lines 153–158 that replaces
top/filler with SAND, and the `nextInt(4)` sandstone transition at line 176.

### Supplemental Cold-Biome Region

Cold biome (Tundra, biome_id=10) was not found in the near-origin square at seed 12345.
A biome scan at chunk radius 64 found Tundra at chunk (39, -42). A 4x4 chunk grid around
that location (chunks 37..40 in X, -44..-41 in Z) was captured.

World block coordinates: wx 592..655, wz -704..-657.
Supplemental columns: 4096, of which 843 have biome_id=10 (Tundra).

Tundra uses default grass/dirt top/filler blocks (same as `BiomeBase` base class, lines 49–50).
The temperature noise produces values in the cold range (`f < 0.1`) yielding Tundra from `getBiome`.

---

## Byte Layout

Array index formula (verbatim from `MapGenCaves.java` line 101):

```
index = (x * 16 + z) * 128 + y
```

where x and z are local chunk coordinates (0–15), y is block height (0–127).
Y=0 is bedrock (block ID 7), Y=127 is the top of the world.
Each column in the JSON stores 128 bytes: Y=0 at position 0, Y=127 at position 127.
Bytes are base64-encoded (128 raw bytes → 172 base64 characters).

Block IDs used in the corpus (matching `Block.java`):

| ID | Name |
|----|------|
| 0 | Air |
| 1 | Stone |
| 2 | Grass |
| 3 | Dirt |
| 7 | Bedrock |
| 9 | Stationary Water |
| 10 | Lava |
| 12 | Sand |
| 13 | Gravel |
| 24 | Sandstone |
| 79 | Ice |

---

## JSON Format (serde shape)

```json
{
  "seed": 12345,
  "capture_source": "back2beta-server-1.7.9 verbatim-extraction harness",
  "byte_layout": "(z*16+x)*128 + y, y=0 is bottom (bedrock), y=127 is top",
  "biome_id_map": { "7": "Desert", "10": "Tundra", ... },
  "near_origin_region": "chunk coords -2..1 x -2..1 (wx -32..31, wz -32..31), 4096 columns",
  "supplemental_region": "chunk region (37..40)x(-44..-41) for cold biome (Tundra), 4096 columns",
  "columns": [
    {
      "wx": -32,
      "wz": -32,
      "biome_id": 7,
      "pre_cave": "<base64-128-bytes>",
      "post_cave": "<base64-128-bytes>"
    }
  ]
}
```

A Rust `serde::Deserialize` struct to load this:

```rust
#[derive(serde::Deserialize)]
pub struct BetaSurfaceCorpus {
    pub seed: u64,
    pub columns: Vec<ColumnFixture>,
}

#[derive(serde::Deserialize)]
pub struct ColumnFixture {
    pub wx: i32,
    pub wz: i32,
    pub biome_id: u8,
    #[serde(with = "serde_base64")]
    pub pre_cave: Vec<u8>,
    #[serde(with = "serde_base64")]
    pub post_cave: Vec<u8>,
}
```

The loader pattern (matching `beta_seed.rs` lines 63–77 style):

```rust
fn load_corpus() -> BetaSurfaceCorpus {
    serde_json::from_str(include_str!("fixtures/beta_surface_corpus.json"))
        .expect("valid fixture JSON")
}
```

---

## moderner-beta Cross-Check

**Deferred.** Running a live moderner-beta capture requires a full Fabric/modern Minecraft
environment which was not available during this capture run. The cross-check is documentation
only — back2beta is the designated source of truth, and moderner-beta is captured solely to
record divergences.

The back2beta verbatim-extraction corpus is self-sufficient for the parity gate. When a moderner-beta
run is eventually available, divergences should be recorded here under a "Divergence Log" section.
Known prior divergence: a phantom extra octave in moderner-beta's noise seeding vs. back2beta
(discovered during earlier noise-parity work).

---

## Compilation and Execution

```bash
cd ~/src/back2beta-server-1.7.9/.b2b-verbatim-harness/compile_dir
javac NoiseGenerator.java NoiseGenerator2.java NoiseGeneratorOctaves.java \
      NoiseGeneratorOctaves2.java NoiseGeneratorPerlin.java MathHelper.java \
      B2bCapture.java
java B2bCapture /path/to/output.txt
python3 convert_corpus_final.py   # produces beta_surface_corpus.json
```

The compile_dir contains package-stripped copies of the noise classes (package declaration
line removed; verified identical to originals otherwise by diff).
