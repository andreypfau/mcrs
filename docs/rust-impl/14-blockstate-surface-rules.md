# BlockState Encoding & Surface Rules — Spec

Covers BlockState JSON and wire encoding, the global block state ID registry,
and the complete surface rules type system.
Sources: `BlockState.java`, `StateHolder.java`, `Block.java`, `IdMapper.java`,
`SurfaceRules.java`, `NoiseGeneratorSettings.java`.

---

## 1. BlockState JSON Encoding

### Format

Two-field object:

```json
{ "Name": "minecraft:stone" }
```

```json
{
  "Name": "minecraft:grass_block",
  "Properties": {
    "snowy": "false"
  }
}
```

```json
{
  "Name": "minecraft:water",
  "Properties": {
    "level": "0"
  }
}
```

**Rules**:
- `"Name"` — always present, fully-qualified block identifier
- `"Properties"` — omitted when the block has no state properties; otherwise a flat
  string→string object (all property values are encoded as strings)
- The codec is lenient: `"Properties"` is optional on deserialization

### Codec Implementation

```java
// BlockState.java
public static final Codec<BlockState> CODEC =
    codec(BuiltInRegistries.BLOCK.byNameCodec(), Block::defaultBlockState).stable();

// StateHolder.java — generic dispatch
protected static <O, S extends StateHolder<O, S>> Codec<S> codec(
    Codec<O> ownerCodec, Function<O, S> defaultState
) {
    return ownerCodec.dispatch(
        "Name",
        s -> s.owner,
        o -> {
            S defaultValue = defaultState.apply((O)o);
            return defaultValue.getValues().isEmpty()
                ? MapCodec.unit(defaultValue)
                : defaultValue.propertiesCodec.codec()
                    .lenientOptionalFieldOf("Properties")
                    .xmap(opt -> opt.orElse(defaultValue), Optional::of);
        }
    );
}
```

### Rust Representation

```rust
/// Used in JSON resources (worldgen noise settings, etc.)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockStateJson {
    #[serde(rename = "Name")]
    pub name: ResourceLocation,
    #[serde(rename = "Properties", default, skip_serializing_if = "HashMap::is_empty")]
    pub properties: HashMap<String, String>,
}
```

---

## 2. Global Block State ID Registry

The global block state ID (`Block.BLOCK_STATE_REGISTRY`) maps every `BlockState` to a
sequential `int` (typically fits in `u16` for all vanilla states, ~6,000–8,000 total).

### Java Source

```java
// Block.java
public static final IdMapper<BlockState> BLOCK_STATE_REGISTRY = new IdMapper<>();

// Populated during static init (Blocks.java):
static {
    for (Block block : BuiltInRegistries.BLOCK) {
        for (BlockState state : block.getStateDefinition().getPossibleStates()) {
            Block.BLOCK_STATE_REGISTRY.add(state);
        }
    }
}

public static int getId(@Nullable BlockState state) {
    if (state == null) return 0;
    int id = BLOCK_STATE_REGISTRY.getId(state);
    return id == -1 ? 0 : id;
}

public static BlockState stateById(int id) {
    BlockState state = BLOCK_STATE_REGISTRY.byId(id);
    return state == null ? Blocks.AIR.defaultBlockState() : state;
}
```

### Assignment Order

IDs are assigned by iterating `BuiltInRegistries.BLOCK` (block registration order), then
iterating `block.getStateDefinition().getPossibleStates()` (all state combinations in the
order the `StateDefinition` builder enumerates them). ID 0 is air (fallback for nulls).

### Network Wire Encoding

Block states on the wire use a `VarInt` encoding of this same ID:

```
[VarInt] block_state_id
```

The chunk section block palette (chunk data packet) references these IDs:
- Indirect palette: `[VarInt paletteBits][VarInt count][VarInt id...]` per 16×16×16 section
- Direct (no palette): raw `VarInt` per block position
- Threshold: indirect if palette_bits ≤ 8, direct otherwise

### Rust Representation

```rust
/// Runtime type — resolved after Bootstrap.
/// In static registry: Vec<BlockState> indexed by this u32.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockStateId(pub u32);

impl BlockStateId {
    pub const AIR: BlockStateId = BlockStateId(0);
}
```

---

## 3. Surface Rules

Surface rules are evaluated during `buildSurface()` (SURFACE chunk status).
They replace blocks near the terrain surface based on position, biome, noise, and depth.

### Evaluation API

```rust
pub trait RuleSource: Send + Sync {
    /// Compile this source into an evaluable rule bound to the generation context.
    fn apply(&self, ctx: &Context) -> Box<dyn SurfaceRule>;
}

pub trait SurfaceRule: Send + Sync {
    /// Return the block state for this position, or None to fall through.
    fn try_apply(&self, block_x: i32, block_y: i32, block_z: i32) -> Option<BlockState>;
}
```

---

## 4. Rule Types (4 RuleSource variants)

All rules use `"type"` dispatch in JSON.

### 4.1 Block Rule — `minecraft:block`

Returns a fixed block state.

```json
{
  "type": "minecraft:block",
  "result_state": { "Name": "minecraft:stone" }
}
```

```rust
pub struct BlockRuleSource {
    pub result_state: BlockState,
}

impl SurfaceRule for BlockRuleSource {
    fn try_apply(&self, ..) -> Option<BlockState> {
        Some(self.result_state)
    }
}
```

### 4.2 Sequence Rule — `minecraft:sequence`

Tries each rule in order; returns the first non-None result.

```json
{
  "type": "minecraft:sequence",
  "sequence": [ { ...rule1... }, { ...rule2... } ]
}
```

```rust
pub struct SequenceRule {
    pub rules: Vec<Box<dyn SurfaceRule>>,
}

impl SurfaceRule for SequenceRule {
    fn try_apply(&self, x: i32, y: i32, z: i32) -> Option<BlockState> {
        self.rules.iter().find_map(|r| r.try_apply(x, y, z))
    }
}
```

### 4.3 Condition Rule — `minecraft:condition`

If the condition is true, evaluates and returns the inner rule; otherwise None.

```json
{
  "type": "minecraft:condition",
  "if_true": { ...condition... },
  "then_run": { ...rule... }
}
```

```rust
pub struct TestRule {
    pub condition: Box<dyn Condition>,
    pub followup: Box<dyn SurfaceRule>,
}

impl SurfaceRule for TestRule {
    fn try_apply(&self, x: i32, y: i32, z: i32) -> Option<BlockState> {
        if self.condition.test() {
            self.followup.try_apply(x, y, z)
        } else {
            None
        }
    }
}
```

### 4.4 Bandlands Rule — `minecraft:bandlands`

Applies the Badlands clay band pattern (192-block repeating color sequence).
No parameters.

```json
{ "type": "minecraft:bandlands" }
```

Returns a colored terracotta based on `context.system.get_band(x, y, z)`.

---

## 5. Condition Types (11 ConditionSource variants)

### 5.1 Biome — `minecraft:biome`

True if current block's biome is in the given list.

```json
{
  "type": "minecraft:biome",
  "biome_is": ["minecraft:swamp", "minecraft:mangrove_swamp"]
}
```

**Caching**: Lazy, re-evaluated on Y-position change (biome is queried per block column).

### 5.2 Noise Threshold — `minecraft:noise_threshold`

True if the named noise value at (X, 0, Z) is within [min, max].

```json
{
  "type": "minecraft:noise_threshold",
  "noise": "minecraft:surface",
  "min_threshold": -0.5,
  "max_threshold": 0.5
}
```

**Caching**: Lazy XZ — re-evaluated only when X or Z changes.

### 5.3 Vertical Gradient — `minecraft:vertical_gradient`

Probabilistic transition: always true below `true_at_and_below`, always false above
`false_at_and_above`, probability interpolated linearly between those heights.

```json
{
  "type": "minecraft:vertical_gradient",
  "random_name": "minecraft:bedrock_floor",
  "true_at_and_below": { "above_bottom": 0 },
  "false_at_and_above": { "above_bottom": 5 }
}
```

```rust
fn compute(&self, ctx: &Context) -> bool {
    let y = ctx.block_y;
    if y <= true_at_and_below { return true; }
    if y >= false_at_and_above { return false; }
    let prob = Mth::map(y as f64, true_at_and_below as f64, false_at_and_above as f64, 1.0, 0.0);
    let rng = random_factory.at(ctx.block_x, y, ctx.block_z);
    rng.next_float() < prob as f32
}
```

**VerticalAnchor variants**: `absolute(y)`, `above_bottom(n)`, `below_top(n)`.

### 5.4 Y Above — `minecraft:y_above`

True if `blockY + (stoneDepthAbove if add_stone_depth) >= anchor + surfaceDepth * multiplier`.

```json
{
  "type": "minecraft:y_above",
  "anchor": { "absolute": 97 },
  "surface_depth_multiplier": 2,
  "add_stone_depth": false
}
```

`surface_depth_multiplier` is range-constrained to `[-20, 20]`.

**VerticalAnchor variants**: `{"absolute": y}`, `{"above_bottom": n}`, `{"below_top": n}`.

**Caching**: Lazy Y.

### 5.5 Water — `minecraft:water`

True if `blockY + (stoneDepthAbove if add_stone_depth) >= waterHeight + offset + surfaceDepth * multiplier`.
Also true if `waterHeight == MIN_INT` (no water nearby).

```json
{
  "type": "minecraft:water",
  "offset": 0,
  "surface_depth_multiplier": 0,
  "add_stone_depth": false
}
```

`surface_depth_multiplier` is range-constrained to `[-20, 20]`.

**Caching**: Lazy Y.

### 5.6 Temperature — `minecraft:temperature`

True if the biome is cold enough to snow at this Y level (`coldEnoughToSnow(pos, seaLevel)`).

```json
{ "type": "minecraft:temperature" }
```

**Caching**: Lazy Y (biome is position-dependent).

### 5.7 Steep — `minecraft:steep`

True if the terrain slope is steep: height difference ≥ 4 blocks in any cardinal direction.

```json
{ "type": "minecraft:steep" }
```

```rust
fn compute(&self, ctx: &Context) -> bool {
    let cx = ctx.block_x & 15;
    let cz = ctx.block_z & 15;
    let n = chunk.height(WORLD_SURFACE_WG, cx, max(cz-1, 0));
    let s = chunk.height(WORLD_SURFACE_WG, cx, min(cz+1, 15));
    let w = chunk.height(WORLD_SURFACE_WG, max(cx-1, 0), cz);
    let e = chunk.height(WORLD_SURFACE_WG, min(cx+1, 15), cz);
    s >= n + 4 || w >= e + 4
}
```

**Caching**: Lazy XZ.

### 5.8 Not — `minecraft:not`

Inverts a condition.

```json
{
  "type": "minecraft:not",
  "invert": { "type": "minecraft:steep" }
}
```

### 5.9 Hole — `minecraft:hole`

True if `surfaceDepth <= 0` (terrain dip, no surface cover).

```json
{ "type": "minecraft:hole" }
```

**Caching**: Lazy XZ.

### 5.10 Above Preliminary Surface — `minecraft:above_preliminary_surface`

True if `blockY >= minSurfaceLevel` where `minSurfaceLevel` is derived from the
preliminary surface density function's output for this column.

```json
{ "type": "minecraft:above_preliminary_surface" }
```

Used to prevent surface rules from applying below the natural terrain surface
(e.g., inside cave ceilings that happen to have the same block Y).

### 5.11 Stone Depth — `minecraft:stone_depth`

True if `stoneDepth <= 1 + offset + (surfaceDepth if add_surface_depth) + secondaryDepth`.

- `surface_type: "floor"` → uses `stoneDepthAbove` (distance from above surface)
- `surface_type: "ceiling"` → uses `stoneDepthBelow` (distance from below surface)
- `secondary_depth_range > 0` → adds a noise-derived offset in [0, secondary_depth_range]

```json
{
  "type": "minecraft:stone_depth",
  "offset": 0,
  "add_surface_depth": true,
  "secondary_depth_range": 0,
  "surface_type": "floor"
}
```

**Caching**: Lazy Y.

---

## 6. Evaluation Context

The `Context` object is updated per XZ column and per Y block during `buildSurface()`:

```rust
pub struct Context<'a> {
    // Infrastructure
    pub system:       &'a SurfaceSystem,
    pub random_state: &'a RandomState,
    pub chunk:        &'a mut ChunkAccess,
    pub noise_chunk:  &'a NoiseChunk,

    // Updated per-column (updateXZ):
    pub block_x:       i32,
    pub block_z:       i32,
    pub surface_depth: i32,  // Surface height variation (noise-driven)

    // Updated per-block (updateY):
    pub block_y:          i32,
    pub water_height:     i32,  // MIN_INT if no nearby water
    pub stone_depth_above: i32, // Blocks from terrain surface (above)
    pub stone_depth_below: i32, // Blocks from terrain surface (below)

    // Lazy (computed on first access per Y):
    pub biome: LazyCell<Holder<Biome>>,

    // Secondary noise (for stone_depth secondary_depth_range):
    pub surface_secondary: f64,
}
```

### Lazy Evaluation

Conditions use change-detection caches to avoid recomputation:

| Cache Class | Re-evaluated when |
|-------------|-------------------|
| `LazyXZCondition` | `block_x` or `block_z` changes |
| `LazyYCondition` | `block_y` changes |
| Singleton (no cache) | Always, no state |

---

## 7. Example: Overworld Surface Rule Structure

From `data/minecraft/worldgen/noise_settings/overworld.json` (simplified):

```json
{
  "surface_rule": {
    "type": "minecraft:sequence",
    "sequence": [
      {
        "type": "minecraft:condition",
        "if_true": { "type": "minecraft:vertical_gradient",
                     "random_name": "minecraft:bedrock_floor",
                     "true_at_and_below": { "above_bottom": 0 },
                     "false_at_and_above": { "above_bottom": 5 } },
        "then_run": { "type": "minecraft:block",
                      "result_state": { "Name": "minecraft:bedrock" } }
      },
      {
        "type": "minecraft:condition",
        "if_true": { "type": "minecraft:above_preliminary_surface" },
        "then_run": {
          "type": "minecraft:sequence",
          "sequence": [
            {
              "type": "minecraft:condition",
              "if_true": { "type": "minecraft:stone_depth",
                           "offset": 0, "add_surface_depth": false,
                           "secondary_depth_range": 0, "surface_type": "floor" },
              "then_run": {
                "type": "minecraft:sequence",
                "sequence": [
                  {
                    "type": "minecraft:condition",
                    "if_true": { "type": "minecraft:biome",
                                 "biome_is": ["minecraft:frozen_ocean", ...] },
                    "then_run": { "type": "minecraft:block",
                                  "result_state": { "Name": "minecraft:gravel" } }
                  },
                  { "type": "minecraft:block",
                    "result_state": { "Name": "minecraft:grass_block",
                                      "Properties": { "snowy": "false" } } }
                ]
              }
            }
          ]
        }
      }
    ]
  }
}
```

---

## 8. Key Source Locations

| File | Purpose |
|------|---------|
| `world/level/block/state/BlockState.java` | `CODEC`, global ID accessors |
| `world/level/block/state/StateHolder.java:190–206` | Generic state codec dispatch |
| `world/level/block/Block.java:81,134–146` | `BLOCK_STATE_REGISTRY`, `getId()`, `stateById()` |
| `core/IdMapper.java` | Sequential ID assignment implementation |
| `world/level/levelgen/SurfaceRules.java` | All 4 rule types, 11 condition types, Context |
| `world/level/levelgen/NoiseGeneratorSettings.java:38` | `surface_rule` codec field |
| `data/minecraft/worldgen/noise_settings/overworld.json` | Full real-world surface rule example |

## See also

- [../worldgen/08-surface-rules.md](../worldgen/08-surface-rules.md) — Java surface rules spec (4 rule types, 11 conditions)
- [../worldgen/07-noise-settings.md](../worldgen/07-noise-settings.md) — NoiseGeneratorSettings.surface_rule field
- [12-chunk-pipeline.md](12-chunk-pipeline.md) — chunk pipeline evaluating surface rules in the SURFACE stage
