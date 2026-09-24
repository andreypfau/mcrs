# Fixture Capture Procedure — `beard.bin`

**Source of truth:** vanilla `26.4-snapshot-1`, `world_version` 5119, read through
Fabric Loom's mapped jar. No server, no client, no registries and no world are
involved: `Beardifier` is a pure function of its rigid pieces, its jigsaw
junctions and its affected box, so every case is synthetic.

**Harness:** `tools/vanilla-oracle/src/main/java/mcrs/oracle/BeardOracle.java`

```sh
cd tools/vanilla-oracle
./gradlew dumpBeard --console=plain --no-daemon \
  -PoracleOut=../../crates/mcrs_minecraft_worldgen/tests/fixtures/vanilla
```

Output is deterministic: re-running produces a byte-identical file (3 999 134
bytes, SHA-256 `044aa889a251c9ad5282512d3d0e7142b0ce34dd6512948d15b75468ae5d126c`).

**Consumer:** the structure terrain adaptation tests, which compare the kernel
table and every sampled density value bit for bit.

---

## Bootstrap and sampling

`SharedConstants.tryDetectVersion()`, `Bootstrap.bootStrap()`.

`BEARD_KERNEL` is private; the harness reads it through reflection.

Each case builds a `Beardifier` through the `@VisibleForTesting` constructor
`new Beardifier(List<Beardifier.Rigid>, List<JigsawJunction>, BoundingBox affectedBox)`,
except `empty`, which uses `Beardifier.EMPTY` itself. It then calls
`beardifier.sampleVolume(context, buffer, volume)` with
`context = SamplerContext.builder().enableCaches().build()` and
`buffer = DensityBuffer.createUnpooled(volume.size())`.

The affected box of a synthetic case is computed the way
`Beardifier.forStructuresInChunk` computes it: the union
(`BoundingBox.encapsulating`) of every rigid box and of the single-block box at
each junction's `(sourceX, sourceGroundY, sourceZ)`, then `inflatedBy(24)`. The
chunk-proximity filters of `forStructuresInChunk` (`isCloseToChunk(chunkPos, 12)`
and the junction window) are not applied: every listed rigid and junction is
passed to the constructor.

Before a value is written the harness checks that
`beardifier.sampleValue(context, blockX(x), blockY(y), blockZ(z))` has the same
raw float bits as the buffer entry at `volume.indexUnchecked(x, y, z)`, and aborts
the dump otherwise. This holds for every point of every case, including the
lattice rows `sampleVolume` visits below the affected box (see "Lattice
clipping").

## Binary layout

Little-endian. `str` is a `u32` byte length followed by that many UTF-8 bytes.

```
magic            8 bytes, ASCII "MCBEARD0"
format_version   u32   currently 1
world_version    u32   SharedConstants.getCurrentVersion().dataVersion().version() = 5119

kernel_len       u32   13824
kernel           f32 * kernel_len   Beardifier.BEARD_KERNEL, in array order

case_count       u32   12
repeated case_count times:
  name             str
  rigid_count      u32
  repeated rigid_count times, in constructor list order:
    box            i32 * 6   minX minY minZ maxX maxY maxZ of Rigid.box()
    adjustment     u8        see "Enum codes"
    ground_delta   i32       Rigid.groundLevelDelta()
  junction_count   u32
  repeated junction_count times, in constructor list order:
    source_x         i32
    source_ground_y  i32
    source_z         i32
    delta_y          i32
    dest_projection  u8      see "Enum codes"
  has_affected_box u8        0 for a null box (Beardifier.EMPTY), else 1
  if has_affected_box == 1:
    affected_box   i32 * 6   minX minY minZ maxX maxY maxZ
  volume_min       i32 * 3   minBlockX minBlockY minBlockZ
  volume_size      i32 * 3   sizeX sizeY sizeZ
  volume_step      i32 * 3   stepBlockX stepBlockY stepBlockZ
  values           f32 * sizeX*sizeY*sizeZ
```

The file ends after the last case; there is no trailer.

**Value order.** `for z in 0..sizeZ { for x in 0..sizeX { for y in 0..sizeY } }`,
each value read from `volume.indexUnchecked(x, y, z)`, which is
`y + (x + z * sizeX) * sizeY`. The file order is therefore exactly the buffer's
index order. The block coordinate of an index is `minBlock + index * step` on
each axis.

## Enum codes

Written by a `switch` over the enum constant, not from `ordinal()`, so a
reordering of either enum cannot silently renumber the file.

| `TerrainAdjustment` | code |
|---|---|
| `NONE` | 0 |
| `BURY` | 1 |
| `BEARD_THIN` | 2 |
| `BEARD_BOX` | 3 |
| `ENCAPSULATE` | 4 |

| `StructureTemplatePool.Projection` | code |
|---|---|
| `RIGID` | 0 |
| `TERRAIN_MATCHING` | 1 |

`dest_projection` has no effect on the beard value; it is recorded so the input
round-trips.

## Kernel expression

`BEARD_KERNEL[zi * 576 + xi * 24 + yi]` for `xi, yi, zi` in `0..24` is
`(float) computeBeardContribution(xi - 12, yi - 12, zi - 12)`, which evaluates, in
doubles, `Math.pow(Math.E, -(dx² + (dy + 0.5)² + dz²) / 16.0)` and narrows the
result to `float`. It is `Math.pow(Math.E, …)`, not `Math.exp(…)`; the two can
differ in the last bit.

At sample time `getBeardContribution(dx, dy, dz, yToGround)` multiplies the kernel
entry by `-(yToGround + 0.5f) * (float) Mth.fastInvSqrt(lengthSquared / 2.0f) / 2.0f`
in float arithmetic.

## Cases

Chunk volumes are `16 × 384 × 16` at `(chunkX * 16, -64, chunkZ * 16)` with step
`1 × 1 × 1` (98 304 values). Lattice volumes are `5 × 49 × 5` at the same origin
with step `4 × 8 × 4` (1 225 values), the noise cell lattice of one overworld
chunk.

| # | name | volume | inputs | affected box | non-zero values |
|---|---|---|---|---|---|
| 1 | `adjustment_none` | chunk (0, 0) | rigid `9..21, 60..72, 4..11` none, delta 1 | `-15..45, 36..96, -20..35` | 0 |
| 2 | `adjustment_bury` | chunk (0, 0) | rigid `10..20, 300..312, -3..6` bury, delta 2 | `-14..44, 276..336, -27..30`, clipped by the volume top at y 319 | 2 569 |
| 3 | `adjustment_beard_thin` | chunk (0, 0) | rigid `12..24, 64..75, 9..18` beard_thin, delta 3 | `-12..48, 40..99, -15..42` | 5 760 |
| 4 | `adjustment_beard_box` | chunk (0, 0) | rigid `-6..5, 58..70, 11..22` beard_box, delta 4 | `-30..29, 34..94, -13..46` | 7 936 |
| 5 | `adjustment_encapsulate` | chunk (0, 0) | rigid `4..13, -60..-44, -8..3` encapsulate, delta -2 | `-20..37, -84..-20, -32..27`, clipped by the volume bottom at y -64 | 7 296 |
| 6 | `junctions_only` | chunk (0, 0) | junctions `(3, 65, 14)` rigid, `(15, 70, -2)` terrain_matching, `(-4, 62, 8)` terrain_matching, `(20, 68, 20)` rigid | `-28..44, 38..94, -26..44` | 7 169 |
| 7 | `village` | chunk (2, 1) | 7 rigids (3 beard_thin, bury, beard_box, encapsulate, none) and 7 junctions with ground y from 60 to 72 and mixed projections, overlapping each other and the chunk edges | `-2..79, 26..99, -14..64` | 9 714 |
| 8 | `affected_box_misses` | chunk (0, 0) | rigid `200..210, 64..72, 200..210` beard_thin, junction `(190, 66, 195)` | `166..234, 40..96, 171..234`, disjoint from the volume | 0 |
| 9 | `empty` | chunk (0, 0) | `Beardifier.EMPTY`: no rigids, no junctions, null box | none | 0 |
| 10 | `stepped_lattice` | lattice (1, 0) | rigid `30..40, 17..29, 10..14` beard_thin, delta 2, junction `(34, 25, 12)` rigid | `6..64, -7..53, -14..38` | 75 |
| 11 | `negative_chunk` | chunk (-3, -2) | rigids `-38..-27, 70..80, -22..-12` beard_box delta 2 and `-55..-44, 66..71, -36..-28` bury delta 1; junctions `(-35, 72, -18)` terrain_matching and `(-47, 67, -30)` rigid | `-79..-3, 42..104, -60..12` | 9 094 |
| 12 | `negative_stepped_lattice` | lattice (-3, -2) | the inputs of `negative_chunk` | as `negative_chunk` | 116 |

Boxes are written `minX..maxX, minY..maxY, minZ..maxZ`, inclusive. Every rigid
in cases 1–5 crosses at least one horizontal edge of its chunk, so the volume
sees both the inside and the outside of the piece. `adjustment_none` carries an
affected box and still samples to zero, because `NONE` contributes nothing.

## Lattice clipping

`sampleVolume` clips its loops to
`floorDiv(max(0, affected.min − volume.min), step)` and
`min(size − 1, floorDiv(affected.max − volume.min, step))`. The lower bound
rounds down, so the first visited row can lie below the affected box, where
`sampleValue` would return 0 without evaluating anything, while `sampleVolume`
evaluates the unchecked sum. The sum is 0 there anyway, because the affected box
is inflated by 24 and no contribution reaches further than 12 blocks, which is
what the harness's bit-identity check confirms.

- `stepped_lattice`: y indices 7..14 (`floorDiv(57, 8)` to `floorDiv(117, 8)`);
  row 7 is block y -8, one below the affected box's min y -7. The x and z
  ranges clip only at the volume bounds.
- `negative_stepped_lattice`: y indices 13..21 (`floorDiv(106, 8)` to
  `floorDiv(168, 8)`); row 13 is block y 40, two below the affected box's min y
  42.
