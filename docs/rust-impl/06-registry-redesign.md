# Registry System Redesign

Complete specification for replacing `mcrs_registry::Registry<E>` and `Holder<E>`
with a two-tier system that integrates cleanly with Bevy.

---

## Two-Tier Registry Model

Minecraft registries fall into two categories with fundamentally different semantics:

| Tier | Types | Source | Changes | Bevy API |
|------|-------|--------|---------|----------|
| **Static** | Block, Item, EntityType, SoundEvent, Particle, GameEvent | Compile-time (macros) | Never | `StaticRegistry<T>` resource |
| **Dynamic** | Biome, DensityFunction, NormalNoise, DimensionType, NoiseSettings, ConfiguredFeature, PlacedFeature, Structure, StructureSet, ... | JSON files, data packs | Hot-reload | `Assets<T>` + `Handle<T>` |

---

## Tier 1: Static Registries

Static registries hold data that is fixed at compile time and never changes during
a server session. Entries are declared via macros (like the existing `declare_blocks!`)
and stored as `&'static T` references in a `Vec`.

### `StaticRegistry<T>`

```rust
/// A registry of compile-time-fixed entries, indexed by u32.
/// Entries are registered during plugin initialization and frozen thereafter.
#[derive(Resource)]
pub struct StaticRegistry<T: 'static + Send + Sync> {
    /// All entries in registration order. Index = protocol ID.
    entries: Vec<&'static T>,
    /// Fast lookup by identifier
    by_id: HashMap<ResourceLocation, u32>,
}

impl<T: 'static + Send + Sync> StaticRegistry<T> {
    pub fn register(&mut self, id: ResourceLocation, entry: &'static T) -> StaticId<T> {
        let index = self.entries.len() as u32;
        self.entries.push(entry);
        self.by_id.insert(id, index);
        StaticId::new(index)
    }

    pub fn get(&self, id: StaticId<T>) -> &'static T {
        self.entries[id.index as usize]
    }

    pub fn get_by_loc(&self, loc: &ResourceLocation) -> Option<(StaticId<T>, &'static T)> {
        let index = *self.by_id.get(loc)?;
        Some((StaticId::new(index), self.entries[index as usize]))
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, &'static T)> + '_ {
        self.entries.iter().enumerate().map(|(i, e)| (i as u32, *e))
    }

    pub fn len(&self) -> usize { self.entries.len() }
}
```

### `StaticId<T>`

```rust
/// A stable numeric reference to a static registry entry.
/// Equivalent to the protocol numeric ID for static types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StaticId<T> {
    pub index: u32,
    _marker: PhantomData<fn() -> T>,
}

impl<T> StaticId<T> {
    pub const fn new(index: u32) -> Self {
        Self { index, _marker: PhantomData }
    }
}
```

### Block Example

```rust
// Declare blocks at compile time (existing macro pattern)
pub static STONE: Block = Block { id: 1, name: "minecraft:stone", hardness: 1.5, ... };
pub static GRASS_BLOCK: Block = Block { id: 2, name: "minecraft:grass_block", ... };

// In MinecraftCorePlugin::build():
fn register_blocks(mut registry: ResMut<StaticRegistry<Block>>) {
    registry.register(rl!("minecraft:stone"), &STONE);
    registry.register(rl!("minecraft:grass_block"), &GRASS_BLOCK);
    // ... all 900+ blocks
}
```

---

## Tier 2: Dynamic Registries

Dynamic registries hold data loaded from JSON at runtime. Each type is a Bevy `Asset`,
loaded via `AssetLoader`, with full dependency tracking and hot-reload support.

### The Pattern

```rust
// Each dynamic registry type implements Asset
#[derive(Asset, TypePath)]
pub struct Biome {
    pub climate: ClimateParameters,
    pub effects: BiomeEffects,
    pub features: [Vec<Handle<PlacedFeature>>; 11],
    pub carvers: BiomeCarvers,
    // network-synced subset (NETWORK_CODEC)
    // generation settings stay server-side only
}

// Loader uses ctx.load() for cross-registry references
#[derive(Default)]
pub struct BiomeLoader;

impl AssetLoader for BiomeLoader {
    type Asset = Biome;
    type Settings = ();
    type Error = BiomeLoadError;

    async fn load(&self, reader: &mut dyn Reader, _: &(), ctx: &mut LoadContext)
        -> Result<Biome, BiomeLoadError>
    {
        let bytes = reader.read_to_end().await?;
        let raw: BiomeJson = serde_json::from_slice(&bytes)?;

        // Declare deps — returns Handle immediately, Bevy resolves later
        let features: [Vec<Handle<PlacedFeature>>; 11] = raw.features.iter()
            .map(|slot| slot.iter()
                .map(|id| ctx.load(format!("minecraft/worldgen/placed_feature/{id}.json")))
                .collect())
            .try_into().unwrap();

        Ok(Biome { climate: raw.climate, effects: raw.effects, features })
    }
}
```

### Registration

```rust
impl Plugin for EnginePlugin {
    fn build(&self, app: &mut App) {
        // Static registries
        app.init_resource::<StaticRegistry<Block>>()
           .init_resource::<StaticRegistry<Item>>()
           .init_resource::<StaticRegistry<EntityType>>();

        // Dynamic registries — one init_asset per type
        app.init_asset::<NormalNoise>()
           .init_asset_loader::<NormalNoiseLoader>()
           .init_asset::<DensityFunction>()
           .init_asset_loader::<DensityFunctionLoader>()
           .init_asset::<DimensionType>()
           .init_asset_loader::<DimensionTypeLoader>()
           .init_asset::<NoiseGeneratorSettings>()
           .init_asset_loader::<NoiseGeneratorSettingsLoader>()
           .init_asset::<Biome>()
           .init_asset_loader::<BiomeLoader>()
           .init_asset::<PlacedFeature>()
           .init_asset_loader::<PlacedFeatureLoader>()
           .init_asset::<ConfiguredFeature>()
           .init_asset_loader::<ConfiguredFeatureLoader>()
           // ... all ~40 dynamic types
           ;
    }
}
```

---

## Eliminating `Holder<E>`

The existing `Holder<E>` type:

```rust
pub enum Holder<E> {
    Reference(Ident<Cow<'static, str>>),  // → Handle<T> from ctx.load()
    Direct(E),                             // → Handle<T> from assets.add()
}
```

**Replace with `Handle<T>` everywhere** for dynamic registry types.

In an `AssetLoader`:
- `Holder::Reference(id)` → `ctx.load(path_from_id(id))`
- `Holder::Direct(val)` → `ctx.add_labeled_asset(label, val)` (returns `Handle<T>`)

Both cases produce a `Handle<T>`. The loader caller gets a handle; Bevy's dependency
system ensures the referenced asset is loaded before the parent fires `LoadedWithDependencies`.

**For static types** (blocks, items), use `StaticId<T>` directly — not handles.

---

## `RegistrySnapshot` — Stable Network IDs

After all dynamic registries are loaded (WorldgenFreeze state), assign stable `u32` IDs
to all dynamic registry entries. These IDs are used in the Configuration protocol phase.

### Why a Snapshot Is Needed

`AssetId<T>` is a generational index internal to Bevy — it can change between hot-reloads.
The Minecraft protocol requires stable integer IDs (e.g., biome ID 0, 1, 2 ...) that are
consistent between server and client for the lifetime of a connection.

### Design

```rust
/// Stable numeric IDs for all dynamic registry entries.
/// Assigned once per WorldgenFreeze, invalidated on hot-reload.
#[derive(Resource, Default)]
pub struct RegistrySnapshot {
    /// Per registry type: asset path → stable u32 network ID
    biomes:     BiomeSnapshot,
    dimensions: DimensionSnapshot,
    // ... one per synced registry type
}

pub struct BiomeSnapshot {
    /// Ordered list of biomes (index = network ID)
    pub entries: Vec<(ResourceLocation, Handle<Biome>)>,
    /// Reverse lookup: AssetId → network ID
    pub by_asset: HashMap<AssetId<Biome>, u32>,
}

impl BiomeSnapshot {
    pub fn build(assets: &Assets<Biome>, order: &BiomeSendOrder) -> Self {
        let mut entries = Vec::new();
        let mut by_asset = HashMap::new();
        for (loc, handle) in &order.biomes {
            let network_id = entries.len() as u32;
            entries.push((loc.clone(), handle.clone()));
            by_asset.insert(handle.id(), network_id);
        }
        Self { entries, by_asset }
    }

    pub fn network_id(&self, id: AssetId<Biome>) -> Option<u32> {
        self.by_asset.get(&id).copied()
    }
}
```

### Building the Snapshot

```rust
fn build_registry_snapshot(
    mut snapshot: ResMut<RegistrySnapshot>,
    biomes: Res<Assets<Biome>>,
    density_functions: Res<Assets<DensityFunction>>,
    // ... other registries
    send_order: Res<BiomeSendOrder>,
) {
    snapshot.biomes = BiomeSnapshot::build(&biomes, &send_order);
    // ... build snapshots for all 23 synced registries
}

// Run once when entering WorldgenFreeze:
app.add_systems(OnEnter(AppState::WorldgenFreeze), build_registry_snapshot);
```

---

## `ResourceLocation` Type

Replace `valence_ident::Ident<String>` with a purpose-built type:

```rust
/// Minecraft resource identifier: namespace:path
/// Example: "minecraft:overworld", "minecraft:blocks/stone"
#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLocation {
    pub namespace: Arc<str>,
    pub path: Arc<str>,
}

impl ResourceLocation {
    pub fn new(namespace: impl Into<Arc<str>>, path: impl Into<Arc<str>>) -> Self { ... }

    /// Parse "namespace:path" or "path" (defaults to "minecraft" namespace)
    pub fn parse(s: &str) -> Result<Self, ResourceLocationError> { ... }

    /// Shorthand: ResourceLocation::minecraft("stone") → "minecraft:stone"
    pub fn minecraft(path: &str) -> Self {
        Self::new("minecraft", path)
    }

    pub fn to_asset_path(&self) -> String {
        format!("{}/{}", self.namespace, self.path)
    }
}

impl Display for ResourceLocation {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.path)
    }
}

// Convenience macro
macro_rules! rl {
    ($namespace:literal : $path:literal) => {
        ResourceLocation::new($namespace, $path)
    };
    ($s:literal) => {
        ResourceLocation::parse($s).unwrap()
    };
}
```

**Note:** `valence_ident::Ident` can be kept as a protocol-level type for the network
layer where the Minecraft wire format requires it. `ResourceLocation` is used in all
application-level registry, tag, and asset path code.

---

## Migration Plan

### Phase 1: Static Registry

1. Add `StaticId<T>` type to `mcrs_registry` (or new `mc_core` crate)
2. Add `StaticRegistry<T>` resource backed by `Vec<&'static T>` + `HashMap`
3. Change all uses of `Registry<&'static Block>` to `StaticRegistry<Block>`
4. Remove `RegistryId<T>` — callers use `StaticId<T>` or `AssetId<T>` directly
5. Remove `RegistryRef<T>` — callers use `StaticId<T>` or `Handle<T>` directly
6. Remove `Holder<T>` — callers use `Handle<T>` for dynamic types

### Phase 2: Dynamic Registries via Assets<T>

1. Add `AssetLoader` impls for all 40+ dynamic registry types (see `08-worldgen-asset-integration.md`)
2. Add `BuiltinHandles` resource to keep all built-in JSON assets alive
3. Add `BiomeSendOrder` / `DensionSendOrder` resources defining canonical network ordering

### Phase 3: Registry Snapshot

1. Add `RegistrySnapshot` resource
2. Add `build_registry_snapshot` system running on `OnEnter(AppState::WorldgenFreeze)`
3. Replace all uses of `RegistryId::Index` in packet building with `snapshot.network_id()`

---

## Invariants

- `StaticRegistry<T>` is write-once: populated in `CorePlugin::build()`, frozen after `Startup`
- `Assets<T>` for dynamic types: managed by Bevy's `AssetServer`; readable via `Res<Assets<T>>`
- `RegistrySnapshot`: built once on `WorldgenFreeze`; must be rebuilt after hot-reload
- `Handle<T>` (Strong): keeps asset alive; only held in `BuiltinHandles` and cross-registry refs
- `AssetId<T>`: non-owning stable key within a session; use for lookups, NOT for holding

## See also

- [../worldgen/01-registry-system.md](../worldgen/01-registry-system.md) — Java registry system (ResourceLocation, Holder, Registry) this redesigns
- [../worldgen/10-engine-architecture.md](../worldgen/10-engine-architecture.md) — three-tier engine model placing registries in context
- [07-tag-system-redesign.md](07-tag-system-redesign.md) — tag system companion spec (StaticTags<T> and Tags<T: Asset>)
- [03-registry-asset-mapping.md](03-registry-asset-mapping.md) — full Java→Rust type mapping for all 40+ registry types
