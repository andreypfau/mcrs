# Worldgen — Bevy Asset Integration

How to connect the existing highly-optimized worldgen code in `mcrs_minecraft_worldgen`
to Bevy's asset system. The goal: preserve all optimizations, add proper dependency
tracking and tier-ordered loading.

---

## The Core Challenge

The existing worldgen system has two layers:

1. **`ProtoDensityFunction`** (JSON deserialization) — in `density_function/proto.rs`
   - Handles the JSON format including string references to other DFs
   - Produces an unoptimized tree representation

2. **Compiled `DensityFunction`** (optimized) — in `density_function/mod.rs`
   - Flat stack representation with peephole optimizations
   - ~1.67ms per chunk column
   - Immutable + thread-safe (per-thread caches)

**Problem:** There is no Bevy `AssetLoader` connecting these two layers.
String references like `"argument": "minecraft:overworld/continents"` need to
load another `DensityFunction` asset and wait for it before compiling.

**Solution:** Two-phase loading:
1. Load JSON → `UnresolvedDensityFunction` (has `Handle<UnresolvedDensityFunction>` for string refs)
2. After `LoadedWithDependencies` on the root, compile everything into the optimized `DensityFunction`

---

## Two-Phase Density Function Loading

### Phase 1: `UnresolvedDensityFunction` (Asset with handles)

This intermediate type mirrors `ProtoDensityFunction` but uses `Handle<Self>` for refs:

```rust
/// Intermediate density function representation.
/// Arguments that are string refs hold Handle<UnresolvedDensityFunction>.
/// This is the Bevy Asset type; the compiled DensityFunction is NOT an Asset.
#[derive(Asset, TypePath)]
pub enum UnresolvedDensityFunction {
    Constant(f64),
    Reference(Handle<UnresolvedDensityFunction>),  // string ref → other DF
    Noise { noise: Handle<NormalNoise>, xz_scale: f64, y_scale: f64 },
    Add { a: Box<UnresolvedDensityFunction>, b: Box<UnresolvedDensityFunction> },
    Mul { a: Box<UnresolvedDensityFunction>, b: Box<UnresolvedDensityFunction> },
    Spline { spline: Box<UnresolvedSpline> },
    Interpolated(Box<UnresolvedDensityFunction>),
    FlatCache(Box<UnresolvedDensityFunction>),
    Cache2d(Box<UnresolvedDensityFunction>),
    // ... all ~28 DF types
    Clamp { input: Box<Self>, min: f64, max: f64 },
    YClampedGradient { from_y: i32, to_y: i32, from_value: f64, to_value: f64 },
    // etc.
}

impl VisitAssetDependencies for UnresolvedDensityFunction {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        match self {
            Self::Reference(h) => visit(h.id().untyped()),
            Self::Noise { noise, .. } => visit(noise.id().untyped()),
            Self::Add { a, b } | Self::Mul { a, b } => {
                a.visit_dependencies(visit);
                b.visit_dependencies(visit);
            }
            // ... recurse for all variants with sub-DFs
            _ => {}
        }
    }
}
```

### `UnresolvedDensityFunctionLoader`

```rust
#[derive(Default, TypePath)]
pub struct UnresolvedDensityFunctionLoader;

impl AssetLoader for UnresolvedDensityFunctionLoader {
    type Asset = UnresolvedDensityFunction;
    type Settings = ();
    type Error = DFLoadError;

    async fn load(&self, reader: &mut dyn Reader, _: &(), ctx: &mut LoadContext)
        -> Result<UnresolvedDensityFunction, DFLoadError>
    {
        let bytes = reader.read_to_end().await?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
        parse_df_value(&value, ctx)
    }
}

fn parse_df_value(value: &Value, ctx: &mut LoadContext) -> Result<UnresolvedDensityFunction, DFLoadError> {
    match value {
        // String reference: "minecraft:overworld/continents"
        Value::String(id) => {
            let loc = ResourceLocation::parse(id)?;
            let path = format!("{}/worldgen/density_function/{}.json",
                loc.namespace, loc.path);
            let handle = ctx.load::<UnresolvedDensityFunction>(path);
            Ok(UnresolvedDensityFunction::Reference(handle))
        }

        // Numeric literal: 0.5
        Value::Number(n) => {
            Ok(UnresolvedDensityFunction::Constant(n.as_f64().unwrap()))
        }

        // Object: { "type": "minecraft:add", "argument1": ..., "argument2": ... }
        Value::Object(map) => {
            let type_id = map["type"].as_str()
                .ok_or(DFLoadError::MissingType)?;
            parse_df_object(type_id, map, ctx)
        }

        _ => Err(DFLoadError::UnexpectedValue),
    }
}

fn parse_df_object(
    type_id: &str,
    map: &Map<String, Value>,
    ctx: &mut LoadContext,
) -> Result<UnresolvedDensityFunction, DFLoadError> {
    match type_id {
        "minecraft:constant" => {
            Ok(UnresolvedDensityFunction::Constant(map["argument"].as_f64().unwrap()))
        }
        "minecraft:add" => Ok(UnresolvedDensityFunction::Add {
            a: Box::new(parse_df_value(&map["argument1"], ctx)?),
            b: Box::new(parse_df_value(&map["argument2"], ctx)?),
        }),
        "minecraft:noise" => {
            let noise_id = ResourceLocation::parse(map["noise"].as_str().unwrap())?;
            let noise_path = format!("{}/worldgen/noise/{}.json", noise_id.namespace, noise_id.path);
            let noise_handle = ctx.load::<NormalNoise>(noise_path);
            Ok(UnresolvedDensityFunction::Noise {
                noise: noise_handle,
                xz_scale: map["xz_scale"].as_f64().unwrap_or(1.0),
                y_scale: map["y_scale"].as_f64().unwrap_or(1.0),
            })
        }
        "minecraft:spline" => {
            let spline = parse_spline(&map["spline"], ctx)?;
            Ok(UnresolvedDensityFunction::Spline { spline: Box::new(spline) })
        }
        "minecraft:interpolated" => Ok(UnresolvedDensityFunction::Interpolated(
            Box::new(parse_df_value(&map["argument"], ctx)?)
        )),
        // ... all 28 types
        unknown => Err(DFLoadError::UnknownType(unknown.to_string())),
    }
}
```

### Phase 2: Compilation (OnEnter WorldgenFreeze)

After all `UnresolvedDensityFunction` assets are `LoadedWithDependencies`,
compile them into the optimized representation.

```rust
/// Compiled, optimized density function.
/// NOT a Bevy Asset — exists only in RAM during world generation.
pub struct CompiledNoiseRouter {
    pub final_density: DensityFunctionStack,  // the optimized flat stack
    pub noise_router: NoiseRouter,             // all 15 named DF slots
}

fn compile_density_functions(
    unresolved: Res<Assets<UnresolvedDensityFunction>>,
    noise_assets: Res<Assets<NormalNoise>>,
    noise_settings: Res<Assets<NoiseGeneratorSettings>>,
    mut commands: Commands,
    worldgen_handles: Res<WorldgenHandles>,
) {
    let router = compile_noise_router(
        &worldgen_handles.overworld_noise_settings,
        &unresolved,
        &noise_assets,
    );

    // Attach compiled router to the dimension entity
    commands.insert_resource(CompiledNoiseRouter::for_overworld(router));
}

app.add_systems(OnEnter(AppState::WorldgenFreeze), compile_density_functions);
```

The compilation step calls into the existing `DensityFunctionStack::build()` or
equivalent optimizer from `density_function/mod.rs` — this code does NOT change.

---

## All Tier Loaders

### Tier 0: No Dependencies (Load in Parallel)

```rust
// NormalNoise: firstOctave + amplitudes
#[derive(Asset, TypePath, Deserialize)]
pub struct NormalNoise {
    pub first_octave: i32,
    pub amplitudes: Vec<f64>,
}
// Loader: trivial serde_json parse, no ctx.load() calls

// DimensionType: 14 fields, no cross-registry refs
// Already implemented as DimensionTypeAsset — rename/clean up

// ConfiguredWorldCarver: type dispatch, inline config
// StructureProcessorList: list of processor configs
```

### Tier 1: `UnresolvedDensityFunction` (see above)
Self-referential via string refs. `ctx.load()` for each string reference.

### Tier 2: `NoiseGeneratorSettings`

```rust
#[derive(Asset, TypePath)]
pub struct NoiseGeneratorSettings {
    pub noise: NoiseSettings,            // inline
    pub default_block: BlockState,       // static ref
    pub default_fluid: BlockState,       // static ref
    pub noise_router: UnresolvedNoiseRouter,  // 15 UDF handles
    pub surface_rule: UnresolvedSurfaceRule,
    pub spawn_target: Vec<ClimateParameter>,
    pub sea_level: i32,
    pub disable_mob_generation: bool,
    pub aquifers_enabled: bool,
    pub ore_veins_enabled: bool,
    pub legacy_random_source: bool,
}

pub struct UnresolvedNoiseRouter {
    pub barrier:               Handle<UnresolvedDensityFunction>,
    pub fluid_level_floodedness: Handle<UnresolvedDensityFunction>,
    pub fluid_level_spread:    Handle<UnresolvedDensityFunction>,
    pub lava:                  Handle<UnresolvedDensityFunction>,
    pub temperature:           Handle<UnresolvedDensityFunction>,
    pub vegetation:            Handle<UnresolvedDensityFunction>,
    pub continents:            Handle<UnresolvedDensityFunction>,
    pub erosion:               Handle<UnresolvedDensityFunction>,
    pub depth:                 Handle<UnresolvedDensityFunction>,
    pub ridges:                Handle<UnresolvedDensityFunction>,
    pub initial_density_without_jaggedness: Handle<UnresolvedDensityFunction>,
    pub final_density:         Handle<UnresolvedDensityFunction>,
    pub vein_toggle:           Handle<UnresolvedDensityFunction>,
    pub vein_ridged:           Handle<UnresolvedDensityFunction>,
    pub vein_gap:              Handle<UnresolvedDensityFunction>,
}
```

### Tier 3: `ConfiguredFeature`

Self-referential via `RandomSelector` and `WeightedStateSelector` that can
reference other configured features. Use the same deferred-handle pattern.

### Tier 4: `PlacedFeature`

```rust
#[derive(Asset, TypePath)]
pub struct PlacedFeature {
    pub feature: Handle<ConfiguredFeature>,
    pub placement: Vec<PlacementModifier>,
}
```

### Tier 5: `Biome` and `StructureTemplatePool`

```rust
#[derive(Asset, TypePath)]
pub struct Biome {
    pub climate: ClimateParameters,
    pub effects: BiomeEffects,
    pub features: [Vec<Handle<PlacedFeature>>; 11],   // 11 decoration steps
    pub carvers: HashMap<GenerationStep, Vec<Handle<ConfiguredWorldCarver>>>,
}

#[derive(Asset, TypePath)]
pub struct StructureTemplatePool {
    pub fallback: Handle<StructureTemplatePool>,  // self-referential (usually "empty")
    pub elements: Vec<WeightedPoolElement>,
}
```

### Tier 6: `Structure`

```rust
#[derive(Asset, TypePath)]
pub struct Structure {
    pub biomes: TagKey<Biome>,           // tag reference (resolved after tag loading)
    pub start_pool: Handle<StructureTemplatePool>,
    // ... config per structure type
}
```

### Tier 7: `StructureSet`

```rust
#[derive(Asset, TypePath)]
pub struct StructureSet {
    pub structures: Vec<WeightedStructure>,
    pub placement: StructurePlacement,
}

pub struct WeightedStructure {
    pub structure: Handle<Structure>,
    pub weight: u32,
}
```

### Tier 8 (Parallel): `WorldPreset`, `FlatLevelGeneratorPreset`, `MultiNoiseBiomeSourceParameterList`

### Tier 9: `LevelStem`

```rust
#[derive(Asset, TypePath)]
pub struct LevelStem {
    pub dimension_type: Handle<DimensionType>,
    pub generator: ChunkGeneratorConfig,
}

pub enum ChunkGeneratorConfig {
    Noise {
        biome_source: BiomeSourceConfig,
        settings: Handle<NoiseGeneratorSettings>,
    },
    Flat { settings: FlatLevelGeneratorSettings },
    Debug,
}
```

---

## `WorldgenHandles` Resource

Keeps all built-in JSON assets alive with strong handles:

```rust
#[derive(Resource, Default)]
pub struct WorldgenHandles {
    // Tier 0
    pub noise_params: Vec<Handle<NormalNoise>>,
    pub dimension_types: Vec<Handle<DimensionType>>,
    pub configured_carvers: Vec<Handle<ConfiguredWorldCarver>>,
    pub processor_lists: Vec<Handle<StructureProcessorList>>,

    // Tier 1
    pub density_functions: Vec<Handle<UnresolvedDensityFunction>>,

    // Tier 2
    pub noise_settings: Vec<Handle<NoiseGeneratorSettings>>,

    // Tier 3
    pub configured_features: Vec<Handle<ConfiguredFeature>>,

    // Tier 4
    pub placed_features: Vec<Handle<PlacedFeature>>,

    // Tier 5
    pub biomes: Vec<Handle<Biome>>,
    pub template_pools: Vec<Handle<StructureTemplatePool>>,

    // Tier 6
    pub structures: Vec<Handle<Structure>>,

    // Tier 7
    pub structure_sets: Vec<Handle<StructureSet>>,

    // Tier 8
    pub world_presets: Vec<Handle<WorldPreset>>,

    // Tier 9
    pub level_stems: Vec<Handle<LevelStem>>,
}
```

---

## Loading Trigger System

On entering `LoadingDataPack` state, trigger all asset loads:

```rust
fn trigger_worldgen_loads(
    asset_server: Res<AssetServer>,
    mut handles: ResMut<WorldgenHandles>,
) {
    // Tier 0 — load all noise param files
    let noise_dir_handle = asset_server.load_folder("minecraft/worldgen/noise");
    // ... or use compile-time path list

    // Tier 1 — density functions (loaded as UnresolvedDensityFunction)
    handles.density_functions.push(
        asset_server.load("minecraft/worldgen/density_function/overworld/continents.json")
    );
    // ... all density functions

    // Tier 5 — biomes (trigger last explicitly, Bevy waits for deps automatically)
    handles.biomes.push(asset_server.load("minecraft/worldgen/biome/plains.json"));
    // ...
}
```

Because `ctx.load()` is used in every loader, Bevy automatically waits for all
dependencies before firing `LoadedWithDependencies` on parent assets.

---

## Freeze Detection

```rust
fn check_worldgen_ready(
    mut biome_events: MessageReader<AssetEvent<Biome>>,
    handles: Res<WorldgenHandles>,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    // Check if all biomes (the top of the dependency tree) are fully loaded
    let all_ready = handles.biomes.iter().all(|h| {
        asset_server.is_loaded_with_dependencies(h)
    });

    if all_ready && !handles.biomes.is_empty() {
        next_state.set(AppState::WorldgenFreeze);
    }
}

app.add_systems(Update,
    check_worldgen_ready.run_if(in_state(AppState::LoadingDataPack))
);
```

---

## Compilation Pipeline (WorldgenFreeze)

On entering `WorldgenFreeze`, in order:

```rust
app.add_systems(OnEnter(AppState::WorldgenFreeze), (
    // 1. Compile density functions into optimized stacks
    compile_density_functions,
    // 2. Build RegistrySnapshot (stable numeric IDs for network)
    build_registry_snapshot,
    // 3. Resolve tags (now that all assets are loaded)
    resolve_block_tags,
    resolve_biome_tags,
    // 4. Build UpdateTags packet cache
    build_tags_packet_cache,
    // 5. Transition to Playing
    |mut next: ResMut<NextState<AppState>>| next.set(AppState::Playing),
).chain());
```

---

## Thread Safety: NoiseRouter Usage

The existing `DensityFunctionStack` is designed for thread-safe use:
- The compiled stack is **immutable** (stored in `Arc` or as a resource)
- Per-thread caches are created locally during chunk generation
- No `Mutex` needed during generation

```rust
// Chunk generation system (can run on multiple threads via ComputeTaskPool)
fn generate_noise_chunks(
    query: Query<&ChunkPos, With<NeedsNoise>>,
    router: Res<CompiledNoiseRouter>,  // immutable, shared
) {
    query.par_iter().for_each(|pos| {
        let mut cache = router.new_per_thread_cache();  // thread-local
        let column = router.generate_column(pos, &mut cache);
        // ...
    });
}
```

---

## Summary: Integration Points

| Existing Code | Integration Point | Action |
|--------------|-------------------|--------|
| `density_function/proto.rs::ProtoDensityFunction` | Replace with `UnresolvedDensityFunction` (has handles) | Rewrite loader |
| `density_function/mod.rs` (optimizer) | Called from `compile_density_functions` system | Keep unchanged |
| `noise/improved_noise.rs` | Used by compiled DF stack | Keep unchanged |
| `spline/mod.rs` | Used by compiled DF stack | Keep unchanged |
| `climate.rs` | Used by `MultiNoiseBiomeSource` | Keep unchanged |
| `world_preset_loader.rs` | Extend to `WorldPreset` → full tier system | Extend |
| `DimensionTypeLoader` | Keep, rename/clean | Minor |

## See also

- [../worldgen/06-density-functions.md](../worldgen/06-density-functions.md) — Java density function types being loaded by these asset loaders
- [../worldgen/07-noise-settings.md](../worldgen/07-noise-settings.md) — Java noise settings schema loaded by these asset loaders
- [../worldgen/11-registry-dependency-graph.md](../worldgen/11-registry-dependency-graph.md) — load tier order followed by the loaders
- [06-registry-redesign.md](06-registry-redesign.md) — StaticRegistry<T> + Assets<T> pattern used by these loaders
