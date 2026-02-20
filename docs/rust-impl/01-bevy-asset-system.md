# Bevy Asset System — Implementation Reference

Source: `~/IdeaProjects/bevy/crates/bevy_asset/`
Relevant for: implementing Minecraft registries as Bevy assets with dependency tracking and hot reload.

---

## AssetLoader Trait (bevy_asset/src/loader.rs)

```rust
pub trait AssetLoader: TypePath + Send + Sync + 'static {
    type Asset: Asset;
    type Settings: Settings + Default + Serialize + for<'a> Deserialize<'a>;
    type Error: Into<BevyError>;

    fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &Self::Settings,
        load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<Self::Asset, Self::Error>>;

    fn extensions(&self) -> &[&str] { &[] }
}
```

**Key points:**
- Fully async — runs on Bevy's task pool (not main thread)
- `Settings` are serializable per-load configuration (e.g., which namespace to use)
- `extensions()` auto-matches files by extension — use `&["json"]` for registry loaders
- Errors convert to `BevyError` via `Into`

**Minecraft pattern:**

```rust
#[derive(Default)]
pub struct BiomeLoader;

impl AssetLoader for BiomeLoader {
    type Asset = Biome;
    type Settings = ();
    type Error = BiomeLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        ctx: &mut LoadContext,
    ) -> Result<Biome, BiomeLoadError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let raw: BiomeJson = serde_json::from_slice(&bytes)?;

        // Declare cross-registry dependencies — returns Handle immediately
        let features: [Vec<Handle<PlacedFeature>>; 11] = raw.features
            .iter()
            .map(|slot| slot.iter()
                .map(|id| ctx.load(format!("worldgen/placed_feature/{id}.json")))
                .collect())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        Ok(Biome { features, climate: raw.climate, effects: raw.effects, .. })
    }
}
```

---

## LoadContext — Methods During Loading (bevy_asset/src/loader.rs:336)

```rust
// Declare a deferred dependency (returns Handle immediately, does not block)
pub fn load<'b, A: Asset>(&mut self, path: impl Into<AssetPath<'b>>) -> Handle<A>

// Access the nested loader (more control)
pub fn loader(&mut self) -> NestedLoader<'a, '_, StaticTyped, Deferred>

// Create and return a labeled sub-asset within the same file
pub fn add_labeled_asset<A: Asset>(&mut self, label: String, asset: A) -> Handle<A>

// Read raw bytes of another file (tracks as loader dependency for hot reload)
pub async fn read_asset_bytes<'b, 'c>(
    &'b mut self,
    path: impl Into<AssetPath<'c>>,
) -> Result<Vec<u8>, ReadAssetBytesError>

// Call at the end of load() to finalize (called implicitly via return Ok(...))
pub fn finish<A: Asset>(self, value: A) -> LoadedAsset<A>
```

**Deferred vs Immediate loading:**

```rust
// Deferred: queue load, return Handle now (most common)
let handle: Handle<ConfiguredFeature> = ctx.load("worldgen/configured_feature/oak.json");

// Immediate: await the load synchronously (use sparingly — blocks loader task)
let loaded = ctx.loader().immediate().load::<ConfiguredFeature>("...").await?;
```

---

## Handle<T> — Ownership Semantics (bevy_asset/src/handle.rs)

```rust
pub enum Handle<A: Asset> {
    Strong(Arc<StrongHandle>),  // Reference-counted — keeps asset alive
    Uuid(Uuid, PhantomData<...>),  // Weak by UUID — does NOT keep alive
}
```

| Type | Keeps asset alive | Can be copied | Use for |
|------|-------------------|---------------|---------|
| `Handle::Strong` | Yes (Arc) | Clone increments refcount | Hold during gameplay |
| `Handle::Uuid` | No | Yes (cheap) | Stable cross-session IDs |
| `AssetId<A>` | No | Yes (Copy) | Lookup keys, NOT holding |
| `UntypedHandle` | Yes (if Strong) | Clone | Type-erased storage |

**For Minecraft registries:**
- Keep `Handle<Biome>` (Strong) in a `Vec<Handle<Biome>>` resource so all biomes stay loaded
- Use `AssetId<Biome>` as the stable index into registry lookup maps
- `Handle<T>` in cross-registry references (e.g., `Biome.features: Vec<Handle<PlacedFeature>>`)

---

## Assets<T> Resource — Internal Storage (bevy_asset/src/assets.rs)

```rust
#[derive(Resource)]
pub struct Assets<A: Asset> {
    dense_storage: DenseAssetStorage<A>,  // Vec<Entry> with generational indices
    hash_map: HashMap<Uuid, A>,            // UUID-keyed assets
    queued_events: Vec<AssetEvent<A>>,     // Flushed each frame
}

// Dense storage uses generational indices (fast, compact)
struct DenseAssetStorage<A: Asset> {
    storage: Vec<Entry<A>>,   // Entry = None | Some { value, generation }
    len: u32,
    allocator: Arc<AssetIndexAllocator>,
}
```

**Key API:**
```rust
fn get(&self, id: impl Into<AssetId<A>>) -> Option<&A>
fn get_mut(&mut self, id: impl Into<AssetId<A>>) -> Option<&mut A>
fn add(&mut self, asset: impl Into<A>) -> Handle<A>  // Assigns new ID
fn insert(&mut self, id: impl Into<AssetId<A>>, asset: A)
fn remove(&mut self, id: impl Into<AssetId<A>>) -> Option<A>
fn iter(&self) -> impl Iterator<Item = (AssetId<A>, &A)>
```

---

## AssetEvent<T> — All Variants (bevy_asset/src/event.rs)

```rust
pub enum AssetEvent<A: Asset> {
    Added { id: AssetId<A> },
    Modified { id: AssetId<A> },
    Removed { id: AssetId<A> },
    Unused { id: AssetId<A> },
    LoadedWithDependencies { id: AssetId<A> },
}
```

**`LoadedWithDependencies`** — fires when:
1. The asset itself is loaded (load state = `Loaded`)
2. ALL direct dependencies are loaded
3. ALL recursive (transitive) dependencies are loaded

This is the "freeze" signal: when you get `LoadedWithDependencies` for every biome handle, the entire worldgen registry tree is ready.

**Listening to events:**
```rust
fn check_worldgen_ready(
    mut events: EventReader<AssetEvent<Biome>>,
    mut next_state: ResMut<NextState<AppState>>,
    loading: Res<WorldgenLoading>,
) {
    for event in events.read() {
        if let AssetEvent::LoadedWithDependencies { id } = event {
            loading.mark_loaded(*id);
            if loading.all_done() {
                next_state.set(AppState::Playing);
            }
        }
    }
}
```

---

## Dependency Tracking Internals (bevy_asset/src/server/info.rs)

```rust
pub(crate) struct AssetInfo {
    pub(crate) load_state: LoadState,
    pub(crate) dep_load_state: DependencyLoadState,
    pub(crate) rec_dep_load_state: RecursiveDependencyLoadState,
    loading_dependencies: HashSet<ErasedAssetIndex>,       // Direct deps in progress
    failed_dependencies: HashSet<ErasedAssetIndex>,
    loading_rec_dependencies: HashSet<ErasedAssetIndex>,   // Transitive deps
    failed_rec_dependencies: HashSet<ErasedAssetIndex>,
    dependents_waiting_on_load: HashSet<ErasedAssetIndex>,
    dependents_waiting_on_recursive_dep_load: HashSet<ErasedAssetIndex>,
    loader_dependencies: HashMap<AssetPath<'static>, AssetHash>,  // For hot reload
}
```

**Three tiers of dependency tracking:**
1. **Direct** (`dep_load_state`) — assets loaded via `ctx.load()` in this loader
2. **Recursive** (`rec_dep_load_state`) — transitive deps (dep of dep of dep…)
3. **Loader** (`loader_dependencies`) — file hashes for hot-reload change detection

**When a file changes:**
1. File watcher fires `AssetSourceEvent::ModifiedAsset(path)`
2. Server reloads the asset (re-runs its loader)
3. `AssetEvent::Modified` fires for all system observers
4. Loader dependency hashes are re-checked — if a loader dependency changed, dependents reload too

---

## Hot Reload — File Watcher (bevy_asset/src/io/file/file_watcher.rs)

```rust
pub struct FileWatcher {
    _watcher: Debouncer<RecommendedWatcher, RecommendedCache>,
}
```

Uses `notify_debouncer_full` with debouncing to batch rapid writes.

**Events mapped to asset server:**
- `AccessKind::Close(Write)` → `AssetSourceEvent::ModifiedAsset(path)`
- `CreateKind::File` → `AssetSourceEvent::AddedAsset(path)`
- `RemoveKind::Any` → `AssetSourceEvent::RemovedAsset(path)`
- `RenameMode::Both` → `AssetSourceEvent::RenamedAsset { old, new }`

**Enable hot reload:**
```rust
AssetPlugin {
    watch_for_changes_override: Some(true),  // dev builds only
    ..default()
}
// OR via feature flag in Cargo.toml:
// bevy = { features = ["file_watcher"] }
```

---

## Labeled Assets — Multiple from One File

Load multiple asset types from a single JSON file:

```rust
// In loader: add labeled sub-assets
let features_handle = ctx.add_labeled_asset("Features".to_string(), feature_vec);
let mob_data_handle = ctx.add_labeled_asset("Mobs".to_string(), mob_data);

// Reference labeled sub-assets from other loaders:
let features: Handle<Vec<PlacedFeature>> =
    ctx.load("worldgen/biome/plains.json#Features");
```

**Minecraft use case:** load a biome JSON and produce separate handles for
the biome metadata, the generation settings, and the mob settings — then
different systems only need to load what they use.

---

## App Registration Pattern

```rust
impl Plugin for WorldgenAssetsPlugin {
    fn build(&self, app: &mut App) {
        // Register each asset type and its loader
        app.init_asset::<NormalNoise>()
           .init_asset_loader::<NormalNoiseLoader>()
           .init_asset::<DensityFunction>()
           .init_asset_loader::<DensityFunctionLoader>()
           .init_asset::<NoiseGeneratorSettings>()
           .init_asset_loader::<NoiseSettingsLoader>()
           .init_asset::<Biome>()
           .init_asset_loader::<BiomeLoader>()
           .init_asset::<ConfiguredFeature>()
           .init_asset_loader::<ConfiguredFeatureLoader>()
           .init_asset::<PlacedFeature>()
           .init_asset_loader::<PlacedFeatureLoader>()
           // ... all 40 registry types
           ;
    }
}
```

---

## Two-Pass Loading for Self-Referential Registries

`density_function`, `configured_feature`, and `template_pool` are self-referential.
Standard deferred loading handles most cases automatically — `ctx.load()` returns a
Handle immediately and Bevy resolves it when the target is loaded. However, there
is a subtlety: **the loader for entry A may run before entry B exists**, leaving a
pending handle.

Bevy's deferred dependency system handles this correctly:
- `ctx.load("path/to/entry_B.json")` returns a Handle
- The handle is initially unresolved
- When entry B finishes loading, the handle becomes valid
- `LoadedWithDependencies` fires for entry A only after B is resolved

For the **inline string reference** pattern in density functions
(`"argument1": "minecraft:overworld/depth"` where the value is a string, not a `{type: ...}` object),
the JSON parser needs to detect this case and call `ctx.load()` on the referenced path.

```rust
// In DensityFunctionLoader, when parsing an argument:
fn parse_argument(value: &serde_json::Value, ctx: &mut LoadContext)
    -> Result<DensityFunctionArg, Error>
{
    match value {
        Value::String(id) => {
            // String reference → deferred load from registry
            let handle = ctx.load(format!("worldgen/density_function/{id}.json"));
            Ok(DensityFunctionArg::Reference(handle))
        }
        Value::Number(n) => {
            // Numeric literal → inline constant
            Ok(DensityFunctionArg::Constant(n.as_f64().unwrap()))
        }
        Value::Object(_) => {
            // Inline definition → recurse
            Ok(DensityFunctionArg::Inline(parse_density_function(value, ctx)?))
        }
        _ => Err(Error::unexpected_value(value)),
    }
}
```

---

## AssetServer State Checking

```rust
// Is this specific asset (and all its deps) loaded?
asset_server.is_loaded_with_dependencies(&handle)

// What is the load state of this asset?
asset_server.get_load_state(&handle)  // LoadState::NotLoaded | Loading | Loaded | Failed

// What is the recursive dependency state?
asset_server.get_recursive_dependency_load_state(&handle)

// Trigger a reload (useful after re-configuration)
asset_server.reload(&handle);
```

---

## AssetPath — Anatomy

```
"worldgen/density_function/overworld/continents.json"
 └─ default source (assets/ folder)

"custom://worldgen/biome/plains.json"
 └─ custom source named "custom"

"worldgen/biome/plains.json#Features"
 └─ labeled sub-asset (Features within plains.json)
```

For Minecraft data packs, configure a custom `AssetSource` that reads from the
data pack directory instead of the default `assets/` folder:

```rust
app.register_asset_source(
    "data",
    AssetSource::build().with_reader(|| {
        Box::new(FileAssetReader::new("data"))
    }),
);

// Then load as:
let handle: Handle<Biome> = asset_server.load("data://minecraft/worldgen/biome/plains.json");
```

---

## Summary Table

| Java concept | Bevy equivalent |
|---|---|
| `Holder<T>` | `Handle<T>` (Strong) |
| `Holder.Reference<T>` | `Handle<T>` (returned by `ctx.load()`) |
| `Holder.Direct<T>` | `Handle<T>` (returned by `assets.add()`) |
| `Registry<T>` | `Assets<T>` resource |
| `ResourceLocation` | `AssetPath<'static>` |
| `ResourceKey<T>` | `AssetId<T>` |
| `BuiltInRegistries.freeze()` | `AssetEvent::LoadedWithDependencies` |
| `RegistryDataLoader` | `AssetServer` + `AssetLoader` impls |
| `HolderSet.Named<T>` (tag) | Custom `TagSet<T>` resource (tags are not built-in to Bevy) |
| `TagKey<T>` | Custom `TagId<T>` newtype |
| `DIRECT_CODEC` | `serde::Deserialize` on full server struct |
| `NETWORK_CODEC` | Separate `serde::Serialize` struct (subset of fields) |
