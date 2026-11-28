# Section biomes: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states how a section stores its biomes, how world generation fills them, and which
value each reader of a biome takes. The save layout of the containers is in `persistence.md`,
SAVE-CHUNK-001. The biome source of a dimension and its climate parameters are outside this
document.

## 0. Conventions

### 0.1 Terms

- **Chunk**: the 16 × 16 block columns whose block coordinates `x` and `z` have the same values of
  `x >> 4` and `z >> 4`. The shift is arithmetic. `cx` and `cz` are these two values.
- **Section**: the 16 × 16 × 16 blocks of a chunk with the same value of `y >> 4`. `sy` is this
  value.
- **Quart**: a cube of 4 × 4 × 4 blocks. The quart coordinate of a block coordinate `v` is
  `v >> 2`. The corner block of the quart `q` is `4 · q`.
- **World quart rows**: the quart `y` coordinates from `min_y >> 2` to `(min_y + height) >> 2 − 1`.
  `min_y` and `height` are the fields of the dimension type.
- **Biome source**: the field `biome_source` of the generator of the dimension.
- **Source biome**: the biome that the biome source gives at a quart. For the type
  `minecraft:multi_noise`, the biome source samples the climate at the corner block of the quart.
- **Biome container**: the container of a section with one cell for each block (BIO-SEC-001).
- **Noise biome container**: the container of a section with one cell for each quart
  (BIO-SEC-001).
- **Stored biome**: the entry of the biome container at a block.
- **Noise biome**: the entry of a noise biome container at a quart.
- **Zoom**: the function of BIO-SEC-004 from a block position to one of eight quarts.
- **Hashed seed**: the first 8 bytes of the SHA-256 digest of the world seed, read as a signed
  64-bit integer. The input of the digest is the 8 bytes of the seed, low byte first. The 8 bytes of
  the digest are read low byte first.
- **Neighbourhood of a section**: the 6 × 6 × 6 quarts with `x` from `4 · cx − 1` to `4 · cx + 4`,
  `z` from `4 · cz − 1` to `4 · cz + 4`, and `y` from `4 · sy − 1` to `4 · sy + 4`.
- **Corner quart of a section**: the quart `(4 · cx, 4 · sy, 4 · cz)`.
- **Held chunk**: a chunk that the server holds in memory at the status `minecraft:biomes` or a
  later status.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents".

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `BIO-SEC` | the biome containers of a section, their generation and their readers |

---

## 1. Containers

### BIO-SEC-001: Containers and cell order

- **Observer:** a save file and the vanilla client.
- **Behavior:** a section has two containers of biome ids.

| Container | Cells | Cell size | Cell index from local coordinates | Life |
|-----------|-------|-----------|-----------------------------------|------|
| biome container | 4096 | 1 block | `y · 256 + z · 16 + x`, each from 0 to 15 | always |
| noise biome container | 64 | 1 quart | `y · 16 + z · 4 + x`, each from 0 to 3 | at the statuses `minecraft:noise_biomes`, `minecraft:biomes` and `minecraft:terrain` |

- **Biome container before generation:** each cell holds `minecraft:plains` until the status
  `minecraft:biomes` (BIO-SEC-003).
- **Noise biome container:** the status `minecraft:noise_biomes` makes it (BIO-SEC-002). The step
  of the status `minecraft:features` removes it before the features place. Thus `minecraft:terrain`
  is the last status of a chunk that holds it. It never goes on the wire.
- **Bits of an entry:** let `b` be the bits of each cell index. These values of `b` are valid:

| Container | `b` = 0 | Palette with `2^b` entries at most | No palette, global ids |
|-----------|---------|------------------------------------|------------------------|
| biome container | one entry | `b` from 1 to 8 | `b` ≥ 9 |
| noise biome container | one entry | `b` from 1 to 3 | `b` ≥ 4 |

- **Save:** the save key of the biome container is `biomes`. The save key of the noise biome
  container is `noise_biomes`. The writer removes each palette entry that no cell uses. Then `n`
  is the number of entries, and `b` is `ceil(log2(n))`. The field `data` is absent when `n` is 1.
  `persistence.md`, SAVE-CHUNK-001, states the other parts of the layout.
- **Wire:** the chunk packet carries the biome container of each section after its block state
  container. The layout is:
  1. one byte with `b`;
  2. when `b` is 0, one VarInt with the biome id;
  3. when `b` is from 1 to 8, one VarInt with the palette length, then one VarInt biome id for each
     palette entry;
  4. when `b` is 9 or more, no palette;
  5. `ceil(4096 / floor(64 / b))` longs with no length prefix. There is no long when `b` is 0.
- **Global ids:** when the biome container has no palette, the client reads `G` bits for each
  cell. `G` is `ceil(log2(N))`, and `N` is the number of entries of the biome registry. The server
  must send `b` = `G` in that case.
- **Choice of `b`:** the server can send each `b` from 1 to 8 that is not less than
  `ceil(log2(palette length))`. The vanilla client accepts each such choice. A palette entry that
  no cell uses is valid.
- **Packed form:** each long holds `floor(64 / b)` cell indices from the low bits. A cell index never
  spans two longs.

Provenance: `source_suggested`. Kind: wire for the wire layout and the save layout, unspecified
for the choice of `b` on the wire. Confidence: medium.

---

## 2. Generation

### BIO-SEC-002: Noise biome sample

- **Observer:** a save file at the status `minecraft:noise_biomes`, and each stored biome through
  BIO-SEC-004.
- **Behavior:** the status `minecraft:noise_biomes` comes after `minecraft:structure_references`
  and before `minecraft:biomes`. It fills the noise biome container of each section of the chunk.
  The cell `(qx, qy, qz)` of the section `sy` holds the source biome of the quart
  `(4 · cx + qx, 4 · sy + qy, 4 · cz + qz)`.
- **Range:** the status fills each section of the dimension height. It reads no other chunk.
- **Old chunks:** near a chunk of an older world version, the game blends the source biome.
  SAVE-VERSION-001 rejects such a chunk, so this document states no blend.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### BIO-SEC-003: Margin and vertical clamp

- **Observer:** each stored biome of a section at the edge of a chunk or of the world.
- **Behavior:** the status `minecraft:biomes` fills the biome container of each section of the
  chunk from a grid of noise biomes. The grid has these quarts:
  - `x` from `4 · cx − 1` to `4 · cx + 4`;
  - `z` from `4 · cz − 1` to `4 · cz + 4`;
  - `y` from one row below the world quart rows to one row above them.
- **Margin:** a quart of the grid outside the chunk is in one of the eight neighbour chunks. Its
  value is the noise biome of that quart in that neighbour chunk. Each neighbour chunk is at the
  status `minecraft:noise_biomes`, `minecraft:biomes` or `minecraft:terrain` at this time. The
  status `minecraft:features` of a chunk needs its eight neighbour chunks at `minecraft:terrain` or
  a later status. Thus no neighbour reaches `minecraft:features` before this chunk is at
  `minecraft:biomes`.
- **Vertical clamp:** the row below the world quart rows takes the noise biome of the lowest world
  quart row at the same `x` and `z`. The row above the world quart rows takes the noise biome of the
  highest world quart row.
- **Clamp and zoom:** the clamp changes only the biome of a grid quart. The zoom of BIO-SEC-004
  uses the true quart coordinates of the grid row for its offsets.
- **Result:** the stored biome of each block of the chunk is the noise biome at the quart that
  the zoom of the block selects.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### BIO-SEC-004: Zoom

- **Observer:** each stored biome after the status `minecraft:biomes`.
- **Inputs:** a block position `(x, y, z)`, the hashed seed `S`, and a function that gives a noise
  biome for a quart.
- **Definitions:** all integer operations wrap at 64 bits. `L(r, k)` is
  `r · (r · 6364136223846793005 + 1442695040888963407) + k`, where `k` is a signed 64-bit value. A
  coordinate `k` is a 32-bit integer that extends its sign to 64 bits. The seed `S` as `k` is the
  full 64-bit value, with no truncation. `F(r)` is `(((r >> 24) & 1023) / 1024.0 − 0.5) · 0.9`
  in IEEE 754 double precision.
- **Behavior:**
  1. For each axis `v` of `x`, `y` and `z`, let `a = v − 2`, `p = a >> 2` and `f = (a & 3) / 4.0`.
  2. Examine the eight corners in the order `i` = 0 to 7. Bit 4 of `i` selects `px + 1` for
     `x`, else `px`. Bit 2 of `i` selects `py + 1`, else `py`. Bit 1 of `i` selects `pz + 1`, else
     `pz`.
  3. For each axis, the offset `d` is `f` when the corner takes `p`, and `f − 1.0` when the corner
     takes `p + 1`.
  4. Let `(qx, qy, qz)` be the corner. Set `r` to `S`. Then set `r` to `L(r, qx)`, `L(r, qy)`,
     `L(r, qz)`, `L(r, qx)`, `L(r, qy)` and `L(r, qz)`, in this order.
  5. Set `ex` to `F(r)`. Set `r` to `L(r, S)`, and set `ey` to `F(r)`. Set `r` to `L(r, S)`, and
     set `ez` to `F(r)`.
  6. Compute the distance `D` from the three squares `(dx + ex)²`, `(dy + ey)²` and `(dz + ez)²` in
     double precision. The status `minecraft:biomes` adds them in the order `x`, `y`, `z`.
  7. Select the corner with the smallest `D`. When two corners have the same `D`, select the corner
     with the lower `i`.
  8. The result is the noise biome at the selected corner.
- **Postconditions:**
  - The result is the noise biome of one of the eight corners.
  - The lookup with no held chunk adds the three squares in a different order (BIO-SEC-008).
    Section 4 records the two orders.

Provenance: `source_suggested`. Kind: semantic, quirk for the addition order of step 6.
Confidence: medium.

### BIO-SEC-005: Uniform neighbourhood and uniform cell

- **Observer:** the stored biomes and the wire form of the biome container.
- **Uniform neighbourhood:** when each quart of the neighbourhood of a section has the same noise
  biome, each block of the section has that stored biome. The biome container then has one palette
  entry, and `b` is 0.
- **Uniform cell:** the blocks with the same three values of `p` in BIO-SEC-004 form one cell.
  When the eight corners of a cell have the same noise biome, each block of the cell has that
  stored biome.
- **Postconditions:** these two results are equal to the results of BIO-SEC-004. An
  implementation can give them without the zoom of each block. The neighbourhood uses the vertical
  clamp of BIO-SEC-003.

Provenance: `source_suggested`. Kind: semantic for the stored biomes, unspecified for the wire
form. Confidence: medium.

### BIO-SEC-006: Palette after generation

- **Observer:** the vanilla client, and the feature set of BIO-SEC-007.
- **Behavior:** after the status `minecraft:biomes`, the palette of the biome container of a
  section holds each stored biome of the section.
- **Corner entry:** the palette also holds the noise biome of the corner quart of the section as its
  first entry. This entry stays when no block of the section has that biome.
- **Bits:** the game uses `b = ceil(log2(k))` in memory and on the wire. `k` is the number of
  different noise biomes in the neighbourhood of the section. When `k` is 1, BIO-SEC-005 applies.
- **Save:** the save removes each palette entry that no cell uses (BIO-SEC-001). Thus a corner
  entry that no block uses is not in the save, and a load does not restore it. A section that a
  load restores has no such entry.

Provenance: `source_suggested`. Kind: unspecified for `b` and the palette order, semantic for
the stored biomes in the palette, quirk for the corner entry. Confidence: medium.

---

## 3. Readers

### BIO-SEC-007: Readers of each kind

- **Observer:** a player in normal play: terrain, features, mobs, commands and the colours of the
  client.
- **Behavior:** each reader of a biome takes one of three values.

| Value | Reader | Position |
|-------|--------|----------|
| stored biome | the biome conditions of the surface rules | the block of the rule |
| stored biome | the extra surface pass of `minecraft:eroded_badlands` | one block above the top block of the terrain column |
| stored biome | the extra surface pass of `minecraft:frozen_ocean` and `minecraft:deep_frozen_ocean` | one block above the top block of the terrain column |
| stored biome | the placement modifier `minecraft:biome` of a placed feature | the position of the modifier |
| stored biome | a biome check of a feature during its placement | the position of the check |
| stored biome | natural mob spawns | the spawn position |
| stored biome | `execute if biome` | `command-execute.md`, CMD-EX-057 |
| stored biome | the client: colours of grass, foliage and water, fog, and the other effects of a biome | the block |
| palette set | the possible features of a chunk | see below |
| source biome | the carvers of a source chunk | the quart `(4 · sx, 0, 4 · sz)` |
| source biome | the biome checks of a structure | the quart of each check |
| source biome | `locate biome` | `command-locate.md`, CMD-LOC-003 |

- **Top block of the terrain column:** the highest block of the column that the noise makes solid,
  before the surface rules. When the position above it is outside the world, the reader takes the
  stored biome of the highest block of the world at the same `x` and `z`.
- **Possible features:** the feature decoration of the chunk `C` reads the palettes of the biome
  containers in the 3 × 3 chunks around `C`. It takes the union of these palettes. It keeps only
  the biomes that the biome source can give. It tries each placed feature of these biomes, in each
  step.
- **Effect of the possible features:** a placed feature with no placement modifier
  `minecraft:biome` can place in `C` when a biome of the union lists it. BIO-SEC-006, corner entry,
  can add a biome to the union.
- **Surface rules:** a surface rule must give the same result as a check of the stored biome at
  each block. An implementation can skip the check when no palette biome of the chunk matches, or
  when each palette biome matches.
- **Carvers:** `sx` and `sz` are the chunk coordinates of the source chunk of the carver. The
  quart `y` is 0 in each dimension.
- **Source biome:** these readers do not zoom. They read the biome source, not the containers.
- **Command `fillbiome`:** it sets the stored biome of each block in its box. It does not change a
  noise biome container.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### BIO-SEC-008: Biome at a position with no held chunk

- **Observer:** a player in normal play, through a server reader of a biome at a position with no
  held chunk.
- **Behavior on the server:**
  - In a held chunk, the biome of a block is its stored biome. The `y` coordinate clamps to the
    blocks of the dimension height.
  - With no held chunk, the biome of a block is the zoom of BIO-SEC-004 over source biomes. The
    function of noise biomes is the source biome of each quart.
  - This zoom has no vertical clamp, no margin and no blend.
  - This zoom adds the three squares of step 6 in the order `z`, `y`, `x`. Section 4 records this
    order.
- **Behavior on the client:** in a chunk that the client holds, the biome of a block is the stored
  biome from the chunk packet. With no such chunk, the biome is `minecraft:plains`. The client
  never zooms.

Provenance: `source_suggested`. Kind: semantic on the server, quirk for the addition order,
client agreement on the client. Confidence: medium.

---

## 4. Known vanilla quirks

### Two addition orders of the zoom

- **Rule:** BIO-SEC-004, step 6, and BIO-SEC-008.
- **Accident:** the status `minecraft:biomes` adds the three squares in the order `x`, `y`, `z`.
  The lookup with no held chunk adds them in the order `z`, `y`, `x`. Floating-point addition is
  not associative. Thus the two orders can give a different last bit of `D`. Then they can select
  a different corner when two corners have almost the same `D`.
- **Observer:** the save file and the chunk packet show the stored biomes. With another
  generation order, the stored biome of a few blocks differs from the game for the same seed. With
  another lookup order, a reader with no held chunk can take another biome than the game at a few
  positions.
- **Decision:** #1053.
- **Bug reference:** none.

### Corner entry of the palette

- **Rule:** BIO-SEC-006, corner entry.
- **Accident:** after the status `minecraft:biomes`, the palette holds the noise biome of the
  corner quart of the section, also when no block of the section has that biome.
- **Observer:** the blocks that the features place, which a save file shows. The possible features
  of BIO-SEC-007 include the features of the corner biome. A placed feature of that biome with no
  placement modifier `minecraft:biome` can then place in the chunk. Without the entry, such a
  feature does not place, and the blocks differ from the game for the same seed.
- **Limit:** the game keeps the entry only in a continuous run. The save removes the entry, so a
  neighbour chunk that a load restores has no such entry. The result then depends on whether the
  neighbour was unloaded before `minecraft:features`. mcrs copies the same behaviour.
- **Decision:** #1053.
- **Bug reference:** none.

## 5. Intentional deviations

None.

## 6. Unresolved

None.
