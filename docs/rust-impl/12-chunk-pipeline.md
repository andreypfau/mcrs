# Chunk Generation Pipeline — Spec

Covers `ChunkStatus` lifecycle, `RandomState` / `DimensionRandomState`, `NoiseChunk`,
`NoiseRouter`, and the aquifer system. Sources: `ChunkStatus.java`, `NoiseBasedChunkGenerator.java`,
`RandomState.java`, `NoiseChunk.java`, `NoiseRouter.java`, `Aquifer.java`,
`NoiseGeneratorSettings.java`, `NoiseSettings.java`.

---

## 1. ChunkStatus Lifecycle

**Source**: `ChunkStatus.java`

Status chain (linked list, traversable from FULL → EMPTY via `getParent()`):

| Index | Status | Chunk Type | Heightmaps |
|-------|--------|-----------|------------|
| 0 | `EMPTY` | PROTOCHUNK | — |
| 1 | `STRUCTURE_STARTS` | PROTOCHUNK | — |
| 2 | `STRUCTURE_REFERENCES` | PROTOCHUNK | — |
| 3 | `BIOMES` | PROTOCHUNK | — |
| 4 | `NOISE` | PROTOCHUNK | `OCEAN_FLOOR_WG`, `WORLD_SURFACE_WG` |
| 5 | `SURFACE` | PROTOCHUNK | `OCEAN_FLOOR_WG`, `WORLD_SURFACE_WG` |
| 6 | `CARVERS` | PROTOCHUNK | `OCEAN_FLOOR`, `WORLD_SURFACE`, `MOTION_BLOCKING`, `MOTION_BLOCKING_NO_LEAVES` |
| 7 | `FEATURES` | PROTOCHUNK | all 4 |
| 8 | `INITIALIZE_LIGHT` | PROTOCHUNK | all 4 |
| 9 | `LIGHT` | PROTOCHUNK | all 4 |
| 10 | `SPAWN` | PROTOCHUNK | all 4 |
| 11 | `FULL` | LEVELCHUNK | all 4 |

**Heightmap groups**:
- `WORLDGEN_HEIGHTMAPS` (`OCEAN_FLOOR_WG` + `WORLD_SURFACE_WG`) — used from EMPTY through SURFACE
- `FINAL_HEIGHTMAPS` — all four types, used from CARVERS through FULL

**Key methods**:
```rust
pub fn index(self) -> u8      // 0-11
pub fn parent(self) -> Option<ChunkStatus>
pub fn is_or_after(self, other: ChunkStatus) -> bool
pub fn is_or_before(self, other: ChunkStatus) -> bool
pub fn max(a: ChunkStatus, b: ChunkStatus) -> ChunkStatus
```

---

## 2. NoiseGeneratorSettings & NoiseSettings

**Source**: `NoiseGeneratorSettings.java`, `NoiseSettings.java`

### NoiseSettings

Controls cell sizing for trilinear interpolation:

```rust
pub struct NoiseSettings {
    pub min_y: i32,                    // Minimum block Y
    pub height: i32,                   // Total height in blocks
    pub noise_size_horizontal: i32,    // Cell width = noise_size_horizontal * 4 blocks
    pub noise_size_vertical: i32,      // Cell height = noise_size_vertical * 4 blocks
}
```

**Cell size formulas**:
- `cell_width = noise_size_horizontal * 4`  (quarts → blocks)
- `cell_height = noise_size_vertical * 4`

**Vanilla presets**:
| Preset | minY | height | nSH | nSV | Cell W | Cell H |
|--------|------|--------|-----|-----|--------|--------|
| Overworld | -64 | 384 | 1 | 2 | 4 | 8 |
| Nether | 0 | 128 | 1 | 2 | 4 | 8 |
| End | 0 | 128 | 2 | 1 | 8 | 4 |
| Caves | -64 | 192 | 1 | 2 | 4 | 8 |
| Floating Islands | 0 | 256 | 2 | 1 | 8 | 4 |

### NoiseGeneratorSettings

```rust
pub struct NoiseGeneratorSettings {
    pub noise_settings: NoiseSettings,
    pub default_block: BlockState,
    pub default_fluid: BlockState,
    pub noise_router: NoiseRouter,     // 15 density functions
    pub surface_rule: SurfaceRuleSource,
    pub spawn_target: Vec<Climate::ParameterPoint>,
    pub sea_level: i32,
    pub aquifers_enabled: bool,
    pub ore_veins_enabled: bool,
    pub use_legacy_random_source: bool,
}
```

**Preset ResourceKeys**: `OVERWORLD`, `LARGE_BIOMES`, `AMPLIFIED`, `NETHER`, `END`, `CAVES`,
`FLOATING_ISLANDS`.

---

## 3. RandomState

**Source**: `RandomState.java`

`RandomState` is the per-dimension, per-world-seed bundle of seeded noise functions and random
sources. Created once per dimension per server start. In the Rust redesign this becomes a
`Component` on each dimension entity.

### Creation

```rust
pub fn create(
    settings: &NoiseGeneratorSettings,
    noises: &HolderGetter<NormalNoise::NoiseParameters>,
    seed: i64,
) -> RandomState
```

### Initialization

1. **Base positional random factory**:
   ```rust
   let base = match settings.use_legacy_random_source {
       true  => LegacyRandomSource::new(seed).fork_positional(),
       false => XoroshiroRandomSource::new(seed).fork_positional(),
   };
   ```

2. **Specialized randoms** (derived from base by name hash):
   ```rust
   self.aquifer_random = base.from_hash_of("aquifer").fork_positional();
   self.ore_random     = base.from_hash_of("ore").fork_positional();
   ```

3. **Surface system**:
   ```rust
   self.surface_system = SurfaceSystem::new(
       &self,
       settings.default_block,
       settings.sea_level,
       &base,
   );
   ```

4. **Noise wiring** — traverse all 15 `NoiseRouter` density functions via `DensityFunction::Visitor`.
   The `NoiseWiringHelper` instantiates every referenced `NormalNoise` object.
   Special cases: `TEMPERATURE`, `VEGETATION`, `SHIFT` noises get dedicated seeds.

5. **Climate sampler** — flatten the 6 climate density functions from the router into a
   `Climate::Sampler` for biome placement.

### Public API

```rust
pub fn get_or_create_noise(&self, key: ResourceKey<NormalNoise::NoiseParameters>) -> &NormalNoise
pub fn get_or_create_random_factory(&self, name: Identifier) -> PositionalRandomFactory
pub fn router(&self) -> &NoiseRouter
pub fn sampler(&self) -> &Climate::Sampler
pub fn surface_system(&self) -> &SurfaceSystem
pub fn aquifer_random(&self) -> &PositionalRandomFactory
pub fn ore_random(&self) -> &PositionalRandomFactory
```

### ECS Placement

`RandomState` is held as a `Component` on each dimension entity.
Created in `OnEnter(AppState::WorldgenFreeze)` after noise assets are loaded.

```rust
fn create_dimension_random_states(
    dimensions: Query<(Entity, &DimensionSettingsHandle)>,
    noise_settings: Res<Assets<NoiseGeneratorSettings>>,
    noise_params: Res<Assets<NormalNoise::NoiseParameters>>,
    world_seed: Res<WorldSeed>,
    mut commands: Commands,
) {
    for (entity, settings_handle) in &dimensions {
        let settings = noise_settings.get(settings_handle.0).unwrap();
        let random_state = RandomState::create(settings, &noise_params, world_seed.0);
        commands.entity(entity).insert(random_state);
    }
}
```

---

## 4. NoiseRouter

**Source**: `NoiseRouter.java`

15-slot record containing all density functions needed for chunk generation:

```rust
pub struct NoiseRouter {
    // Aquifer system
    pub barrier_noise:                   DensityFunction,
    pub fluid_level_floodedness_noise:   DensityFunction,
    pub fluid_level_spread_noise:        DensityFunction,
    pub lava_noise:                      DensityFunction,

    // Climate (biome placement)
    pub temperature:   DensityFunction,
    pub vegetation:    DensityFunction,
    pub continents:    DensityFunction,
    pub erosion:       DensityFunction,
    pub depth:         DensityFunction,   // also used for Deep Dark detection
    pub ridges:        DensityFunction,

    // Terrain
    pub preliminary_surface_level: DensityFunction,  // pre-surface-rules height
    pub final_density:             DensityFunction,  // solid block density

    // Ore veins
    pub vein_toggle:  DensityFunction,
    pub vein_ridged:  DensityFunction,
    pub vein_gap:     DensityFunction,
}
```

**Visitor pattern** — `map_all(visitor)` returns a new `NoiseRouter` with all 15 DFs transformed.
Used during `NoiseChunk` initialization to install caches and wrap noise functions.

---

## 5. NoiseChunk

**Source**: `NoiseChunk.java`

`NoiseChunk` is the per-chunk trilinear interpolation engine. Allocated once per chunk per
generation pass and reused across multiple `ChunkStatus` stages.

### Factory

```rust
pub fn for_chunk(
    chunk: &ChunkAccess,
    random_state: &RandomState,
    beardifier: BeardifierOrMarker,
    settings: &NoiseGeneratorSettings,
    global_fluid_picker: FluidPicker,
    blender: &Blender,
) -> NoiseChunk
```

### Constructor Parameters

```rust
pub fn new(
    cell_count_xz: i32,         // 16 / cell_width
    random_state: &RandomState,
    chunk_min_block_x: i32,
    chunk_min_block_z: i32,
    noise_settings: &NoiseSettings,
    beardifier: BeardifierOrMarker,
    settings: &NoiseGeneratorSettings,
    global_fluid_picker: FluidPicker,
    blender: &Blender,
) -> Self
```

### Interpolation Data Layout

Two **slices** (swapped per X-column), each of shape `[Z+1][Y+1]`:

```
slice0: [[f64; cellCountY+1]; cellCountXZ+1]
slice1: [[f64; cellCountY+1]; cellCountXZ+1]
```

Per interpolator, 8 corner density values + intermediate values:
```
noise000, noise001, noise100, noise101   (first Y face)
noise010, noise011, noise110, noise111   (second Y face)
valueXZ00, valueXZ10, valueXZ01, valueXZ11  (Z-interpolated)
valueZ0, valueZ1                             (Y-interpolated)
value                                        (final result)
```

**Formula**: `lerp3(factorX, factorY, factorZ, noise000..noise111)`

### Iteration API

Called in this order during `fillFromNoise()`:

```rust
// 1. Begin first X column
noise_chunk.initialize_for_first_cell_x();

// Outer loop: X cells
for cell_x in 0..cell_count_x {
    // 2. Advance to next X column (fills slice1, swaps)
    noise_chunk.advance_cell_x(cell_x);

    // Inner loop: Z cells
    for cell_z in 0..cell_count_z {
        let cell_y = cell_count_y - 1;  // top of column

        // Middle loop: Y cells (top to bottom)
        for cell_y in (0..cell_count_y).rev() {
            // 3. Load 8 corner densities for this (Y,Z) cell
            noise_chunk.select_cell_yz(cell_y, cell_z);

            // Inner Y: sub-blocks within cell
            for y_in_cell in (0..cell_height).rev() {
                let pos_y = (noise_settings.min_y / cell_height + cell_y) * cell_height + y_in_cell;
                let factor_y = y_in_cell as f64 / cell_height as f64;
                noise_chunk.update_for_y(pos_y, factor_y);

                // Inner X: sub-blocks
                for x_in_cell in 0..cell_width {
                    let factor_x = x_in_cell as f64 / cell_width as f64;
                    noise_chunk.update_for_x(pos_x, factor_x);

                    // Innermost Z: sub-blocks
                    for z_in_cell in 0..cell_width {
                        let factor_z = z_in_cell as f64 / cell_width as f64;
                        noise_chunk.update_for_z(pos_z, factor_z);

                        // 4. Get interpolated block state
                        let block_state = noise_chunk.get_interpolated_state();
                        // → set block in chunk
                    }
                }
            }
        }
    }

    // 5. Swap slices for next X column
    noise_chunk.swap_slices();
}

// 6. End interpolation
noise_chunk.stop_interpolation();
```

### Caching Markers

The density function tree contains marker nodes that NoiseChunk replaces with cached variants:

| Marker | NoiseChunk Replacement | Description |
|--------|------------------------|-------------|
| `Interpolated` | `NoiseInterpolator` | Trilinear interpolation (slice grid) |
| `FlatCache` | `FlatCache` | 2D XZ grid cache (per-column result) |
| `Cache2D` | `Cache2D` | 2D cache keyed by block (X,Z) |
| `CacheOnce` | `CacheOnce` | Single-value cache, reset per interpolation step |
| `CacheAllInCell` | `CacheAllInCell` | Full cell storage |

### Block State Computation

```rust
fn get_interpolated_state(&self) -> Option<BlockState> {
    // 1. Get final density from interpolator
    // 2. Pass to aquifer.compute_substance(context, density)
    //    - if density > 0.0 → solid block
    //    - else → aquifer determines fluid or air
    // 3. Apply OreVeinifier (if enabled)
}
```

---

## 6. Aquifer System

**Source**: `Aquifer.java`, inner class `NoiseBasedAquifer`

The aquifer system determines fluid placement underground using a grid of randomly placed
"aquifer centers", each with its own fluid type and surface level.

### Grid Configuration

```rust
const X_SPACING: i32 = 16;   // bits (>> 4)
const Y_SPACING: i32 = 12;   // blocks (floorDiv by 12)
const Z_SPACING: i32 = 16;   // bits (>> 4)

const X_RANGE: i32 = 10;     // random offset range within grid cell
const Y_RANGE: i32 = 9;
const Z_RANGE: i32 = 10;

// Grid coordinate conversions:
fn grid_x(block: i32) -> i32 { block >> 4 }
fn grid_y(block: i32) -> i32 { block.div_floor(12) }
fn grid_z(block: i32) -> i32 { block >> 4 }
```

### compute_substance() Algorithm

```rust
pub fn compute_substance(
    &mut self,
    ctx: &DensityFunction::FunctionContext,
    density: f64,
) -> Option<BlockState>  // None = solid, Some = fluid or air
```

1. If `pos_y > skip_sampling_above_y` → return global fluid (ocean surface layer)
2. If global fluid is lava at `pos_y` → return lava immediately
3. **Find 4 nearest aquifer centers** in 3×3×3 neighborhood:
   - Each grid cell has a random center within its cell (using `X_RANGE`/`Y_RANGE`/`Z_RANGE` offset)
   - Track closest 4 by squared distance
4. **Barrier check** for each pair of closest centers:
   - Compute "barrier pressure" from `barrier_noise` + fluid level difference
   - If `density + barrier > 0.0` → return solid (barrier prevents fluid)
5. If all barriers pass → determine fluid type and surface level:
   - `compute_fluid_type()` — uses `lava_noise` to distinguish lava vs water
   - `compute_surface_level()` — uses `fluid_level_floodedness_noise`, `fluid_level_spread_noise`
6. Return `fluid_status.at(pos_y)` — returns fluid if below surface, air if above

### FluidStatus

```rust
pub struct FluidStatus {
    pub fluid_level: i32,
    pub fluid_type: BlockState,
}

impl FluidStatus {
    pub fn at(&self, block_y: i32) -> BlockState {
        if block_y < self.fluid_level { self.fluid_type }
        else { BlockState::AIR }
    }
}
```

### Global Fluid Picker (default)

```rust
fn create_fluid_picker(settings: &NoiseGeneratorSettings) -> impl FluidPicker {
    let lava  = FluidStatus { fluid_level: -54, fluid_type: BlockState::LAVA };
    let water = FluidStatus { fluid_level: settings.sea_level, fluid_type: settings.default_fluid };
    move |x, y, z| if y < min(-54, settings.sea_level) { &lava } else { &water }
}
```

---

## 7. Generation Stages and Their Methods

### fillFromNoise() — NOISE status

```rust
pub async fn fill_from_noise(
    &self,
    blender: &Blender,
    random_state: &RandomState,
    structure_manager: &StructureManager,
    chunk: &mut ChunkAccess,
)
```

- Creates/reuses `NoiseChunk`
- Runs the trilinear interpolation loop (see §5)
- Updates `OCEAN_FLOOR_WG` and `WORLD_SURFACE_WG` heightmaps
- Runs async on background thread pool: `"wgen_fill_noise"`

### createBiomes() — BIOMES status

```rust
pub async fn create_biomes(
    &self,
    random_state: &RandomState,
    blender: &Blender,
    structure_manager: &StructureManager,
    chunk: &mut ChunkAccess,
)
```

- Gets/creates `NoiseChunk`
- Samples `cachedClimateSampler()` at quart positions (4×4×4 biome grid)
- Calls `chunk.fill_biomes_from_noise()` with the climate sampler and blender

### buildSurface() — SURFACE status

```rust
pub fn build_surface(
    &self,
    region: &WorldGenRegion,
    structure_manager: &StructureManager,
    random_state: &RandomState,
    chunk: &mut ChunkAccess,
)
```

- Gets/creates `NoiseChunk`
- Delegates to `random_state.surface_system().build_surface()` with `SurfaceRuleSource`

### applyCarvers() — CARVERS status

```rust
pub fn apply_carvers(
    &self,
    region: &WorldGenRegion,
    seed: i64,
    random_state: &RandomState,
    biome_manager: &BiomeManager,
    structure_manager: &StructureManager,
    chunk: &mut ChunkAccess,
)
```

- Iterates 8-chunk radius (dx/dz in -8..=8)
- For each neighboring chunk, for each carver in biome's `BiomeGenerationSettings`:
  - If `carver.is_start_chunk(random)` → call `carver.carve(context, aquifer, carving_mask)`
- Uses `CarvingMask` to track which blocks were carved

### Climate Sampling (biome placement)

```rust
pub fn cached_climate_sampler(
    &self,
    noises: &NoiseRouter,
    spawn_target: &[Climate::ParameterPoint],
) -> Climate::Sampler
```

Wraps the 6 climate DFs (temperature, vegetation, continents, erosion, depth, ridges) from
the router into a sampler used by `MultiNoiseBiomeSource::get_biome_at()`.

---

## 8. Rust/Bevy Integration Design

### Component on Dimension Entities

```rust
#[derive(Component)]
pub struct DimensionRandomState(pub RandomState);

#[derive(Component)]
pub struct DimensionNoiseSettings(pub Handle<NoiseGeneratorSettings>);
```

### Chunk Generation ECS

```rust
#[derive(Component, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum ChunkStatus {
    Empty = 0,
    StructureStarts = 1,
    StructureReferences = 2,
    Biomes = 3,
    Noise = 4,
    Surface = 5,
    Carvers = 6,
    Features = 7,
    InitializeLight = 8,
    Light = 9,
    Spawn = 10,
    Full = 11,
}

#[derive(SystemSet, Hash, PartialEq, Eq, Debug, Clone)]
pub enum ChunkGenSet {
    StructureStarts,
    Biomes,
    Noise,
    Surface,
    Carvers,
    Features,
}

// Registration:
app.configure_sets(
    Update,
    (
        ChunkGenSet::StructureStarts,
        ChunkGenSet::Biomes,
        ChunkGenSet::Noise,
        ChunkGenSet::Surface,
        ChunkGenSet::Carvers,
        ChunkGenSet::Features,
    ).chain().run_if(in_state(AppState::Playing)),
);
```

### NoiseChunk Lifecycle

`NoiseChunk` is NOT a Bevy asset or component — it is an ephemeral, stack-allocated scratch
buffer created for each chunk generation pass. It is created in the BIOMES or NOISE stage
system and passed through subsequent stage systems via a staging `ChunkWorkspace` component.

```rust
#[derive(Component)]
pub struct ChunkWorkspace {
    pub noise_chunk: Option<Box<NoiseChunk>>,  // Present from BIOMES through CARVERS
    pub proto_chunk: ProtoChunk,
}
```

Removed from the entity when chunk reaches FEATURES status and `NoiseChunk` is no longer needed.

---

## 9. Key Source Locations

| File | Purpose |
|------|---------|
| `world/level/chunk/status/ChunkStatus.java` | Status chain, index, heightmap groups |
| `world/level/levelgen/NoiseBasedChunkGenerator.java` | All generation stage methods |
| `world/level/levelgen/RandomState.java` | Seed → noise wiring |
| `world/level/levelgen/NoiseChunk.java` | Trilinear interpolation engine |
| `world/level/levelgen/NoiseRouter.java` | 15-slot density function record |
| `world/level/levelgen/Aquifer.java` | NoiseBasedAquifer, FluidStatus |
| `world/level/levelgen/NoiseSettings.java` | Cell sizing |
| `world/level/levelgen/NoiseGeneratorSettings.java` | Full settings record + presets |

## See also

- [../worldgen/09-chunk-generation-pipeline.md](../worldgen/09-chunk-generation-pipeline.md) — Java chunk generation spec (11 ChunkStatus stages, algorithms)
- [../worldgen/06-density-functions.md](../worldgen/06-density-functions.md) — density function types evaluated by NoiseChunk
- [../worldgen/07-noise-settings.md](../worldgen/07-noise-settings.md) — NoiseRouter and NoiseGeneratorSettings driving the pipeline
- [13-biome-source.md](13-biome-source.md) — biome sampling in the BIOMES ChunkStatus stage
