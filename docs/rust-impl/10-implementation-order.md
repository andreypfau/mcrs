# Implementation Order

Concrete step-by-step sequence for implementing the redesigned system in `mcrs`.
Accounts for what already exists and what needs to be built or rewritten.
Supersedes `04-implementation-roadmap.md` for the Rust/Bevy rewrite.

---

## Starting State (mcrs PoC)

**Already good — do not touch:**
- `density_function/mod.rs` — optimized stack (6,564 lines)
- `noise/` — ImprovedNoise, OctavePerlinNoise, NoiseSampler
- `spline/` — CubicSpline
- `climate.rs` — ClimateParameters, ParamPoint
- `mcrs_protocol/` — protocol encoding/decoding
- `mcrs_network/` — network I/O
- `mcrs_nbt/` — NBT
- `mcrs_random/` — RNG

**Needs redesign (in order):**
`mcrs_registry` → static registry + tag system → worldgen asset loaders → AppState → DF compilation → Configuration protocol → chunk pipeline → hot-reload

**Completion status (as of Feb 2026):** Step 1 ✅ DONE (infrastructure crate). Steps 2–13 all pending.

> **Critical:** `mcrs_core` is created but has ZERO dependents — no crate in the workspace imports it.
> The old PoC code is still in use everywhere:
> - `mcrs_minecraft` uses `mcrs_registry::Registry<T>` + `TagRegistry<T>` with `fs::read_dir`
> - `mcrs_minecraft_worldgen` has 3 PoC asset loaders (NoiseParam, DensityFunction, NoiseGeneratorSettings)
>   that use internal `proto` types, not `mcrs_core`; CarverLoader and ProcessorListLoader do NOT exist
>
> **Step 2 must come first** — add `mcrs_core` to all consumer crates' Cargo.toml, then replace old types.

---

## Step 1: `mc_core` — Foundation Types ✅ DONE

**Crate:** `mcrs_core` (added to workspace; `mcrs_registry` still present for enchantment legacy)

Create these types. No Minecraft game logic yet — pure infrastructure.

```
mcrs_core/src/
├── lib.rs
├── resource_location.rs   // ResourceLocation + rl!() macro
├── registry/
│   ├── static_registry.rs // StaticRegistry<T>, StaticId<T>
│   └── snapshot.rs        // RegistrySnapshot (empty impl for now)
├── tag/
│   ├── file.rs            // TagFile asset + TagFileLoader
│   ├── key.rs             // TagKey<T>, TagRegistryType trait
│   ├── static_tags.rs     // StaticTags<T>
│   └── dynamic_tags.rs    // Tags<T: Asset>
└── state.rs               // AppState enum
```

### 1a. `ResourceLocation`

```rust
#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLocation {
    pub namespace: Arc<str>,
    pub path: Arc<str>,
}
impl ResourceLocation {
    pub fn new(ns: &str, path: &str) -> Self { ... }
    pub fn minecraft(path: &str) -> Self { Self::new("minecraft", path) }
    pub fn parse(s: &str) -> Result<Self, ResourceLocationError> { ... }
    pub fn to_asset_path(&self) -> String { format!("{}/{}", self.namespace, self.path) }
}
macro_rules! rl { ($s:literal) => { ResourceLocation::parse($s).unwrap() }; }
```

**Decision:** Keep `valence_ident::Ident` in `mcrs_protocol` for the wire format.
`ResourceLocation` lives in `mcrs_core` for all application-level registry, tag,
and asset path code. Add `From<ResourceLocation> for Ident<String>` and vice versa.

### 1b. `StaticRegistry<T>` and `StaticId<T>`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StaticId<T> { pub index: u32, _marker: PhantomData<fn() -> T> }

#[derive(Resource)]
pub struct StaticRegistry<T: 'static + Send + Sync> {
    entries: Vec<(ResourceLocation, &'static T)>,
    by_loc:  HashMap<ResourceLocation, u32>,
}
impl<T: 'static + Send + Sync> StaticRegistry<T> {
    pub fn register(&mut self, id: ResourceLocation, entry: &'static T) -> StaticId<T>
    pub fn get(&self, id: StaticId<T>) -> &'static T
    pub fn get_by_loc(&self, loc: &ResourceLocation) -> Option<(StaticId<T>, &'static T)>
    pub fn iter(&self) -> impl Iterator<Item=(StaticId<T>, &'static T)>
    pub fn len(&self) -> usize
}
```

### 1c. `TagFile` asset + `TagFileLoader`

Use `TagFileSettings { registry_segment: String }` to avoid path-parsing heuristics.
`TagFileLoader` calls `ctx.load_with_settings::<TagFile>(sibling_path, |s| ...)` for
`#tag` references.

### 1d. `TagKey<T>` + `TagRegistryType`

```rust
pub trait TagRegistryType: 'static + Send + Sync {
    const REGISTRY_PATH: &'static str;
}
pub struct TagKey<T: TagRegistryType> {
    pub id: ResourceLocation,
    _marker: PhantomData<fn() -> T>,
}
impl<T: TagRegistryType> TagKey<T> {
    pub const fn of(ns: &'static str, path: &'static str) -> Self
    pub fn asset_path(&self) -> String {
        format!("{}/tags/{}/{}.json", self.id.namespace, T::REGISTRY_PATH, self.id.path)
    }
    pub fn load(&self, asset_server: &AssetServer) -> Handle<TagFile>
}
```

### 1e. `AppState`

```rust
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default] Bootstrap,
    LoadingDataPack,
    WorldgenFreeze,
    Playing,
    Reconfiguring,
}
```

**Test:** Workspace compiles with `mcrs_core` added. Old `mcrs_registry` still compiles.

---

## Step 2: Migrate Static Registries

**Crate:** `mcrs_minecraft` (rename not done yet)

Replace `Registry<&'static Block>` and `Registry<&'static Item>` with
`StaticRegistry<Block>` and `StaticRegistry<Item>`.

- Implement `TagRegistryType for Block { const REGISTRY_PATH = "block"; }`
- Implement `TagRegistryType for Item  { const REGISTRY_PATH = "item"; }`
- Move `declare_blocks!` macro output to use `StaticRegistry`
- Declare all vanilla block tag `TagKey<Block>` constants in `mc_vanilla::tag::block`
- Declare all vanilla item tag `TagKey<Item>` constants in `mc_vanilla::tag::item`

Replace `BlockTagPlugin` / `ItemTagPlugin` with:
```rust
fn register_block_tags(mut tags: ResMut<StaticTags<Block>>, asset_server: Res<AssetServer>) {
    use crate::tag::block::*;
    tags.request(&MINEABLE_PICKAXE, &asset_server);
    tags.request(&MINEABLE_AXE, &asset_server);
    // ... all declared TagKey<Block> constants
}
```

Delete `TagRegistry<T>` and `RegistryId<T>` usage from block/item code.

**Test:** Server starts, block/item tags load via `TagFile` assets, tag resolution
fires on `OnEnter(AppState::WorldgenFreeze)`.

---

## Step 3: Tier 0 Asset Loaders

**Crate:** `mcrs_minecraft_worldgen` (plugin currently named `NoiseGeneratorSettingsPlugin`, not yet `WorldgenAssetsPlugin`)

These have no cross-registry dependencies — add loaders to `WorldgenAssetsPlugin`:

```rust
app.init_asset::<NormalNoise>()
   .init_asset_loader::<NormalNoiseLoader>()   // firstOctave + amplitudes, trivial
   .init_asset::<DimensionType>()
   .init_asset_loader::<DimensionTypeLoader>() // exists in PoC, needs mcrs_core types
   .init_asset::<ConfiguredWorldCarver>()
   .init_asset_loader::<CarverLoader>()        // NOT YET IMPLEMENTED
   .init_asset::<StructureProcessorList>()
   .init_asset_loader::<ProcessorListLoader>() // NOT YET IMPLEMENTED
```

> **PoC state:** `mcrs_minecraft_worldgen` already has `NoiseParamAsset`, `DensityFunctionAsset`,
> and `NoiseGeneratorSettingsAsset` from the old PoC. These are backed by internal `proto` types
> and do NOT use `mcrs_core`. They must be replaced (or retrofitted) to use `ResourceLocation`,
> `StaticRegistry<T>`, etc. from `mcrs_core` as part of this step.

**Test:** Load a sample `NormalNoise` JSON from `assets/`, verify fields parse.

---

## Step 4: Tier 1 — `UnresolvedDensityFunctionLoader`

The most complex loader. Replaces `density_function/proto.rs` deserialization
as the Bevy integration layer (the proto.rs internal types remain for the compiler).

```rust
app.init_asset::<UnresolvedDensityFunction>()
   .init_asset_loader::<UnresolvedDensityFunctionLoader>()
```

Loader behavior:
- `Value::String(id)` → `ctx.load_with_settings::<UnresolvedDensityFunction>(path, ...)`
- `Value::Number(n)` → `UnresolvedDensityFunction::Constant(n)`
- `Value::Object` → dispatch by `"type"` field:
  - `"minecraft:noise"` → load `NormalNoise` handle via `ctx.load()`
  - `"minecraft:spline"` → recurse into spline args
  - All 28 types → appropriate struct with boxed sub-DFs

```rust
pub enum UnresolvedDensityFunction {
    Constant(f64),
    Reference(Handle<UnresolvedDensityFunction>),
    Noise { noise: Handle<NormalNoise>, xz_scale: f64, y_scale: f64 },
    Add { a: Box<Self>, b: Box<Self> },
    // ... all 28 types, sub-DFs as Box<Self>
}
```

`VisitAssetDependencies` recursively visits all `Handle<NormalNoise>` and
`Handle<UnresolvedDensityFunction>` within the tree.

**Test:** Load `assets/minecraft/worldgen/density_function/overworld/continents.json`,
verify all string references resolve to handles.

---

## Step 5: Tier 2 — `NoiseGeneratorSettingsLoader`

```rust
app.init_asset::<NoiseGeneratorSettings>()
   .init_asset_loader::<NoiseGeneratorSettingsLoader>()
```

Parses the 15-slot `noise_router` (each slot → `Handle<UnresolvedDensityFunction>`)
and `surface_rule` (deserialized as `UnresolvedSurfaceRule` — keep as `serde_json::Value`
initially if surface rule spec is not yet finalized).

```rust
pub struct NoiseGeneratorSettings {
    pub noise: NoiseSettings,          // height, min_y, size_horizontal, size_vertical
    pub default_block: BlockStateRef,  // "minecraft:stone" → StaticId<Block> lookup
    pub default_fluid: BlockStateRef,
    pub noise_router: UnresolvedNoiseRouter,  // 15 Handle<UnresolvedDensityFunction>
    pub surface_rule: serde_json::Value,      // defer: parse at compile time
    pub sea_level: i32,
    pub aquifers_enabled: bool,
    pub ore_veins_enabled: bool,
    pub legacy_random_source: bool,
}
pub struct UnresolvedNoiseRouter {
    pub barrier: Handle<UnresolvedDensityFunction>,
    pub fluid_level_floodedness: Handle<UnresolvedDensityFunction>,
    // ... 15 fields
}
```

**Test:** Load `assets/minecraft/worldgen/noise_settings/overworld.json` (127KB),
verify 15 DF handles are created.

---

## Step 6: Tiers 3–5 Asset Loaders

**Tier 3: `ConfiguredFeatureLoader`**
```rust
app.init_asset::<ConfiguredFeature>()
   .init_asset_loader::<ConfiguredFeatureLoader>()
```
Parses `{"type": "minecraft:random_selector", "config": {"features": [...]}}`.
For `random_selector` and similar: `features: Vec<Handle<ConfiguredFeature>>`.
For all other types: store config as `serde_json::Value` initially (fill in proper
typed configs over time — see gaps list).

**Tier 4: `PlacedFeatureLoader`**
```rust
app.init_asset::<PlacedFeature>()
   .init_asset_loader::<PlacedFeatureLoader>()
```
`feature: Handle<ConfiguredFeature>` + `placement: Vec<PlacementModifier>`.
`PlacementModifier` deserialized by type dispatch.

**Tier 5: `BiomeLoader` + `StructureTemplatePoolLoader`**
```rust
app.init_asset::<Biome>()
   .init_asset_loader::<BiomeLoader>()
   .init_asset::<StructureTemplatePool>()
   .init_asset_loader::<StructureTemplatePoolLoader>()
```
`Biome` stores `features: [Vec<Handle<PlacedFeature>>; 11]` (11 decoration steps).
`StructureTemplatePool` stores `fallback: Handle<StructureTemplatePool>` (self-ref).

**Test:** Load `assets/minecraft/worldgen/biome/plains.json`, verify all 11 feature
slot handles are created.

---

## Step 7: Tiers 6–9 Asset Loaders

```rust
// Tier 6
app.init_asset::<Structure>()
   .init_asset_loader::<StructureLoader>()   // biome tag Handle<TagFile> + start_pool handle
// Tier 7
app.init_asset::<StructureSet>()
   .init_asset_loader::<StructureSetLoader>()
// Tier 8
app.init_asset::<WorldPreset>()
   .init_asset_loader::<WorldPresetLoader>() // extend existing loader
app.init_asset::<MultiNoiseBiomeSourceParameterList>()
   .init_asset_loader::<MultiNoiseBiomeSourceParameterListLoader>()
// Tier 9
app.init_asset::<LevelStem>()
   .init_asset_loader::<LevelStemLoader>()
```

**Test:** Load `assets/minecraft/worldgen/world_preset/normal.json`, verify
`LoadedWithDependencies` fires after ALL transitive deps resolve.

---

## Step 8: `WorldgenHandles` + Full Load Trigger

```rust
#[derive(Resource, Default)]
pub struct WorldgenHandles {
    pub noise_params: Vec<Handle<NormalNoise>>,
    pub dimension_types: Vec<Handle<DimensionType>>,
    pub density_functions: Vec<Handle<UnresolvedDensityFunction>>,
    pub noise_settings: Vec<Handle<NoiseGeneratorSettings>>,
    pub configured_features: Vec<Handle<ConfiguredFeature>>,
    pub placed_features: Vec<Handle<PlacedFeature>>,
    pub biomes: Vec<Handle<Biome>>,
    pub template_pools: Vec<Handle<StructureTemplatePool>>,
    pub structures: Vec<Handle<Structure>>,
    pub structure_sets: Vec<Handle<StructureSet>>,
    pub world_presets: Vec<Handle<WorldPreset>>,
    pub level_stems: Vec<Handle<LevelStem>>,
}

fn trigger_worldgen_loads(asset_server: Res<AssetServer>, mut handles: ResMut<WorldgenHandles>) {
    // Strategy: load the top-tier assets (WorldPreset covers most deps transitively).
    // Also explicitly load noise params and DFs to start them in parallel.
    handles.world_presets.push(
        asset_server.load("minecraft/worldgen/world_preset/normal.json")
    );
    // Explicitly start parallel loads for tier 0-1 (they'll be resolved as deps anyway,
    // but starting them early maximizes parallelism):
    for path in VANILLA_NOISE_PATHS { handles.noise_params.push(asset_server.load(path)); }
}

// OnEnter(LoadingDataPack) → trigger_worldgen_loads
// Update (in LoadingDataPack) → check_worldgen_ready
fn check_worldgen_ready(
    handles: Res<WorldgenHandles>,
    asset_server: Res<AssetServer>,
    mut next: ResMut<NextState<AppState>>,
) {
    let ready = handles.world_presets.iter()
        .all(|h| asset_server.is_loaded_with_dependencies(h));
    if ready && !handles.world_presets.is_empty() {
        next.set(AppState::WorldgenFreeze);
    }
}
```

---

## Step 9: WorldgenFreeze Systems

All run in `OnEnter(AppState::WorldgenFreeze)`, chained in order:

```rust
app.add_systems(OnEnter(AppState::WorldgenFreeze), (
    resolve_block_tags,    // StaticTags<Block> from loaded TagFile assets
    resolve_item_tags,     // StaticTags<Item>
    resolve_biome_tags,    // Tags<Biome> — collect from Structure.biomes handles
    compile_density_functions,  // UnresolvedDensityFunction → DensityFunctionStack
    build_registry_snapshot,    // RegistrySnapshot: stable u32 IDs for 23 registries
    cache_update_tags_packet,   // Pre-build UpdateTags packet for new connections
    |mut next: ResMut<NextState<AppState>>| next.set(AppState::Playing),
).chain());
```

### 9a. DF Compilation

`compile_density_functions` walks `UnresolvedDensityFunction` trees (now all resolved),
converts to the `ProtoDensityFunction` intermediate form, then calls the existing
optimizer to produce `DensityFunctionStack`. Stores compiled stacks in
`CompiledNoiseRouter` resource (one per dimension).

The existing `DensityFunctionStack` optimizer code is called unchanged:
```rust
let proto = unresolved_to_proto(&unresolved_df, &unresolved_assets, &noise_assets);
let stack = DensityFunctionStack::build(proto, noise_params);
```

### 9b. `RegistrySnapshot`

After `build_registry_snapshot`, assigns stable u32 to all entries in 23 synced registries.
Order matches Minecraft's canonical send order (alphabetical within each registry).

---

## Step 10: Configuration Protocol

**Crate:** `mcrs_server/src/configuration.rs`

When a client completes Login and enters Configuration state:

1. Send `ClientboundRegistryData` packets for all 23 synced registries
   — data comes from `RegistrySnapshot` (which has NBT-encoded network codecs)
2. Send `ClientboundUpdateTags` packet — from pre-built cache
3. Send `ClientboundFinishConfiguration`

```rust
fn send_registry_data(
    client: &mut Connection,
    snapshot: &RegistrySnapshot,
    biomes: &Assets<Biome>,
) {
    for (registry_name, entries) in snapshot.iter_synced_registries() {
        let packet = ClientboundRegistryData { registry: registry_name, entries };
        client.send(packet);
    }
}
```

**Note:** This step requires `RegistrySnapshot` to store pre-serialized NBT per entry,
built at WorldgenFreeze time, so the Configuration send is just a memory copy.

---

## Step 11: Chunk Generation Pipeline

**Crate:** `mcrs_server/src/world/`

Connect compiled worldgen to ECS chunk management.

```rust
// Components
#[derive(Component)] pub struct ChunkPos { pub x: i32, pub z: i32 }
#[derive(Component, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum ChunkStatus { Empty=0, StructureStarts=1, Biomes=2, Noise=3, Surface=4,
                       Carvers=5, Features=6, Light=7, Full=8 }
#[derive(Component)] pub struct InDimension(pub ResourceLocation);

// System ordering
app.configure_sets(Update, (
    ChunkGenSet::Biomes,
    ChunkGenSet::Noise,
    ChunkGenSet::Surface,
    ChunkGenSet::Carvers,
    ChunkGenSet::Features,
    ChunkGenSet::Light,
).chain().run_if(in_state(AppState::Playing)));
```

`generate_noise_terrain` system:
```rust
fn generate_noise_terrain(
    query: Query<(Entity, &ChunkPos, &InDimension), Without<ChunkStatus>>,
    routers: Res<HashMap<ResourceLocation, CompiledNoiseRouter>>,
    mut commands: Commands,
) {
    for (entity, pos, dim) in &query {
        let router = routers.get(&dim.0).unwrap();
        let mut cache = router.new_thread_cache();
        let column = router.generate_column(pos.x, pos.z, &mut cache);
        commands.entity(entity)
            .insert(column)
            .insert(ChunkStatus::Noise);
    }
}
```

---

## Step 12: Tier 10 — Reloadable Registries

Independent of chunk generation. Add any time after Step 3:

```rust
app.init_asset::<LootTable>()
   .init_asset_loader::<LootTableLoader>()
   .init_asset::<Recipe>()
   .init_asset_loader::<RecipeLoader>()
```

These don't affect worldgen or chunk generation. No chunk invalidation on reload.

---

## Step 13: Hot Reload

After everything works:

```rust
// Enable in dev builds
AssetPlugin { watch_for_changes_override: Some(true), ..default() }

// React to changes
fn on_asset_modified(
    mut events: EventReader<AssetEvent<NormalNoise>>,
    mut next: ResMut<NextState<AppState>>,
) {
    for event in events.read() {
        if let AssetEvent::Modified { .. } = event {
            // NormalNoise change → recompile DFs → regenerate chunks
            next.set(AppState::WorldgenFreeze);
        }
    }
}
// Also for: UnresolvedDensityFunction, NoiseGeneratorSettings, Biome, TagFile
```

On re-entering WorldgenFreeze:
1. Recompile DFs
2. Rebuild RegistrySnapshot (IDs may change)
3. Re-resolve tags
4. Send `ClientboundStartConfiguration` to all connected clients
5. Flush chunk cache (old IDs invalid)

---

## Dependency Graph Between Steps

```
Step 1: mc_core types
    └── Step 2: StaticRegistry<Block/Item> + StaticTags
        └── Step 3: Tier 0 loaders (NormalNoise, DimensionType, ...)
            └── Step 4: UnresolvedDensityFunction loader
                └── Step 5: NoiseGeneratorSettings loader
                    └── Step 6: ConfiguredFeature, PlacedFeature, Biome loaders
                        └── Step 7: Structure, WorldPreset, LevelStem loaders
                            └── Step 8: WorldgenHandles + load trigger
                                └── Step 9: WorldgenFreeze systems (tags, DF compile, snapshot)
                                    ├── Step 10: Configuration protocol (needs snapshot)
                                    └── Step 11: Chunk pipeline (needs compiled router)

Step 12: Loot/Recipe (independent — can do anytime after Step 3)
Step 13: Hot reload (after everything works end-to-end)
```

---

## What Each Step Verifies

| Step | Verification |
|------|-------------|
| 1 | `cargo build` — `mcrs_core` compiles cleanly |
| 2 | Server starts, block tags load, `StaticTags<Block>` resolves |
| 3 | `NormalNoise` JSON files load, `AssetEvent::Added` fires |
| 4 | DF `continents.json` loads, all string refs become pending handles |
| 5 | `overworld.json` (127KB) loads with 15 DF handles |
| 6 | `plains.json` biome loads with all feature slot handles |
| 7 | `normal` world preset loads, `LoadedWithDependencies` fires on biomes |
| 8 | All 3,027 JSON files load without errors |
| 9 | WorldgenFreeze completes, `DimensionRandomState` is populated |
| 10 | Client connects and enters play state (receives registries + tags) |
| 11 | Chunk at (0,0) generates noise terrain |
| 12 | Loot tables deserialize |
| 13 | Edit a noise JSON, chunk regenerates within 1 second |

---

## Known Gaps in Documentation (Blockers for Later Steps)

| Gap | Blocks | Status |
|-----|--------|--------|
| Configuration protocol sequence | Step 10 | **Resolved** — see `11-configuration-protocol.md` |
| `RegistrySnapshot` with pre-serialized NBT | Step 10 | **Resolved** — see `11-configuration-protocol.md §5` |
| `BlockState` encoding (JSON + wire) | Steps 5, 11 | **Resolved** — see `14-blockstate-surface-rules.md §1–2` |
| Surface rule structure (4 types, 11 conditions) | Step 9 | **Resolved** — see `14-blockstate-surface-rules.md §3–7` |
| Chunk generation pipeline details | Step 11 | **Resolved** — see `12-chunk-pipeline.md` |
| `MultiNoiseBiomeSource` R-tree / nearest-neighbor | Step 11 | **Resolved** — see `13-biome-source.md` |
| Feature config schemas (60+ types) | Step 6 | Still open — large effort, use `serde_json::Value` initially |
| Aquifer / OreVeinifier config loading | Step 11 | Partially covered in `12-chunk-pipeline.md §6` |
| Hot-reload invalidation dependency graph | Step 13 | Still open — write when implementing Step 13 |

## See also

- [../worldgen/11-registry-dependency-graph.md](../worldgen/11-registry-dependency-graph.md) — registry dependency DAG that determined the step ordering
- [../worldgen/09-chunk-generation-pipeline.md](../worldgen/09-chunk-generation-pipeline.md) — chunk pipeline detailed in Step 11
- [05-poc-analysis.md](05-poc-analysis.md) — PoC analysis informing keep/redesign decisions in this plan
