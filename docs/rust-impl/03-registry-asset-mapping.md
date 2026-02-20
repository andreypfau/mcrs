# Registry → Asset Mapping

Complete mapping of all Minecraft dynamic registries to Bevy Asset types,
including loader dependencies, network sync behavior, and self-referential resolution.

---

## Loading Tiers (Topological Order)

Assets at the same tier can load in parallel. An asset at tier N can only resolve
all its Handle dependencies after all assets at tiers < N are loading (handles
are deferred, so loads can start in parallel — they just won't fire
`LoadedWithDependencies` until dependencies settle).

### Tier 0 — No cross-registry dependencies

| Registry | Rust type | Loader | Notes |
|----------|-----------|--------|-------|
| `worldgen/noise` | `NormalNoiseParameters` | `NormalNoiseLoader` | Just `firstOctave` + `amplitudes: Vec<f64>` |
| `dimension_type` | `DimensionType` | `DimensionTypeLoader` | 14 primitive/enum fields |
| `worldgen/configured_carver` | `ConfiguredWorldCarver` | `CarverLoader` | References block tags (static) only |
| `worldgen/processor_list` | `StructureProcessorList` | `ProcessorListLoader` | References static block types only |

### Tier 1 — Depends on Tier 0

| Registry | Rust type | Loader | Handle deps |
|----------|-----------|--------|------------|
| `worldgen/density_function` | `DensityFunction` | `DensityFunctionLoader` | `Handle<NormalNoiseParameters>` for `noise`/`shifted_noise` types; `Handle<DensityFunction>` for string references (self) |

**Self-referential note:** String references like `"minecraft:overworld/depth"` in
a `DensityFunction` JSON become `ctx.load("worldgen/density_function/overworld/depth.json")`.
Bevy defers resolution automatically.

### Tier 2 — Depends on Tiers 0+1

| Registry | Rust type | Loader | Handle deps |
|----------|-----------|--------|------------|
| `worldgen/noise_settings` | `NoiseGeneratorSettings` | `NoiseSettingsLoader` | `Handle<DensityFunction>` (15 NoiseRouter slots), `Handle<NormalNoiseParameters>` (via inline DFs) |

### Tier 3 — Self-referential

| Registry | Rust type | Loader | Handle deps |
|----------|-----------|--------|------------|
| `worldgen/configured_feature` | `ConfiguredFeature` | `ConfiguredFeatureLoader` | `Handle<ConfiguredFeature>` (self, for random_selector types) |

**Self-referential note:** `random_selector`, `simple_random_selector`,
`random_boolean_selector` all contain `List<Holder<ConfiguredFeature>>`.
These become `Vec<Handle<ConfiguredFeature>>` with `ctx.load()`.

### Tier 4

| Registry | Rust type | Loader | Handle deps |
|----------|-----------|--------|------------|
| `worldgen/placed_feature` | `PlacedFeature` | `PlacedFeatureLoader` | `Handle<ConfiguredFeature>` |

### Tier 5 — Mixed

| Registry | Rust type | Loader | Handle deps |
|----------|-----------|--------|------------|
| `worldgen/template_pool` | `StructureTemplatePool` | `TemplatePoolLoader` | `Handle<StructureTemplatePool>` (fallback, self), `Handle<StructureProcessorList>`, `Handle<PlacedFeature>` (feature pool elements) |
| `worldgen/biome` | `Biome` | `BiomeLoader` | `Handle<PlacedFeature>` (×11 slots, each a list), `Handle<ConfiguredWorldCarver>` |

**Biome note:** The `Biome` asset has two Serde structs:
- `BiomeFull` — server-side: includes `generationSettings` (feature handles, carver handles) and `mobSettings`
- `BiomeNetwork` — client-side: climate params + effects only (no generation/mob data)

### Tier 6

| Registry | Rust type | Loader | Handle deps |
|----------|-----------|--------|------------|
| `worldgen/structure` | `Structure` | `StructureLoader` | `Handle<Biome>` (biomes tag), `Handle<StructureTemplatePool>` (jigsaw start_pool), `Handle<StructureProcessorList>` |

### Tier 7

| Registry | Rust type | Loader | Handle deps |
|----------|-----------|--------|------------|
| `worldgen/structure_set` | `StructureSet` | `StructureSetLoader` | `Handle<Structure>` |

### Tier 8

| Registry | Rust type | Loader | Handle deps |
|----------|-----------|--------|------------|
| `worldgen/world_preset` | `WorldPreset` | `WorldPresetLoader` | `Handle<DimensionType>`, `Handle<NoiseGeneratorSettings>`, biome source (inline), feature/structure handles (transitive) |
| `worldgen/flat_level_generator_preset` | `FlatLevelGeneratorPreset` | — | `Handle<Biome>` |
| `worldgen/multi_noise_biome_source_parameter_list` | `MultiNoiseBiomeSourceParameterList` | — | `Handle<Biome>` |

### Tier 9 — DIMENSIONS

| Registry | Rust type | Loader | Handle deps |
|----------|-----------|--------|------------|
| `dimension` | `LevelStem` | `LevelStemLoader` | All WORLDGEN registries transitively |

### Tier 10 — RELOADABLE (independent)

| Registry | Rust type | Notes |
|----------|-----------|-------|
| `loot_table` | `LootTable` | No chunk invalidation on reload |
| `recipe` | `Recipe` | No chunk invalidation on reload |
| `advancement` | `Advancement` | No chunk invalidation on reload |
| `villager_trade` | `VillagerTradeList` | No chunk invalidation on reload |

---

## Network-Synced Registries (23 total)

Sent during the Configuration protocol phase via `RegistryData` packet.
Client receives these before entering Play state.

| Registry | Network codec omits |
|----------|---------------------|
| `minecraft:biome` | `generationSettings`, `mobSettings` |
| `minecraft:dimension_type` | *(sends all 14 fields)* |
| `minecraft:damage_type` | *(sends all fields)* |
| `minecraft:chat_type` | *(sends all)* |
| `minecraft:wolf_variant` | `spawnConditions` |
| `minecraft:pig_variant` | `spawnConditions` |
| `minecraft:cat_variant` | `spawnConditions` |
| `minecraft:frog_variant` | *(sends all)* |
| `minecraft:painting_variant` | *(sends all)* |
| `minecraft:banner_pattern` | *(sends all)* |
| `minecraft:enchantment` | *(sends all)* |
| `minecraft:jukebox_song` | *(sends all)* |
| `minecraft:instrument` | *(sends all)* |
| `minecraft:trim_pattern` | *(sends all)* |
| `minecraft:trim_material` | *(sends all)* |
| `minecraft:armor_trim_pattern` | *(sends all)* |
| `minecraft:armor_trim_material` | *(sends all)* |
| `minecraft:dialog` | *(sends all)* |
| `minecraft:timeline` | *(sends all)* |
| `minecraft:zombie_nautilus_variant` | *(sends all)* |
| `minecraft:data_component_type` | *(sends all)* |
| Cow variant, Armadillo variant | *(sends all)* |

**Important:** Worldgen registries (`noise`, `density_function`, `noise_settings`,
`configured_feature`, `placed_feature`, `structure`, `structure_set`, `template_pool`)
are NOT sent to clients. Client has no terrain generation logic.

---

## Static Registries (Type Vocabulary — Registered in CorePlugin)

These are registered during `Plugin::build()` as factory maps, not as `Assets<T>`.

### Registered as `HashMap<ResourceLocation, Box<dyn Factory>>`

| Java type | Rust equivalent |
|-----------|-----------------|
| `BuiltInRegistries.CHUNK_GENERATOR` | `HashMap<Id, ChunkGeneratorFactory>` |
| `BuiltInRegistries.BIOME_SOURCE` | `HashMap<Id, BiomeSourceFactory>` |
| `BuiltInRegistries.DENSITY_FUNCTION_TYPE` | `HashMap<Id, DensityFunctionTypeFactory>` |
| `BuiltInRegistries.FEATURE` | `HashMap<Id, FeatureFactory>` |
| `BuiltInRegistries.CARVER` | `HashMap<Id, CarverFactory>` |
| `BuiltInRegistries.STRUCTURE_TYPE` | `HashMap<Id, StructureTypeFactory>` |
| `BuiltInRegistries.STRUCTURE_PLACEMENT_TYPE` | `HashMap<Id, StructurePlacementFactory>` |
| `BuiltInRegistries.POOL_ELEMENT_TYPE` | `HashMap<Id, PoolElementFactory>` |
| `BuiltInRegistries.STRUCTURE_PROCESSOR_TYPE` | `HashMap<Id, ProcessorTypeFactory>` |
| `BuiltInRegistries.PLACEMENT_MODIFIER_TYPE` | `HashMap<Id, PlacementModifierFactory>` |
| `BuiltInRegistries.SURFACE_CONDITION_TYPE` | `HashMap<Id, SurfaceConditionFactory>` |
| `BuiltInRegistries.SURFACE_RULE_TYPE` | `HashMap<Id, SurfaceRuleFactory>` |
| `BuiltInRegistries.BLOCK` | `HashMap<Id, Arc<BlockDefinition>>` |
| `BuiltInRegistries.ENTITY_TYPE` | `HashMap<Id, EntityTypeDefinition>` |

These never reload — they are part of the engine/plugin binary.

---

## Holder/Tag System

Java's `HolderSet<T>` (tags) has no direct Bevy equivalent. Implement as:

```rust
/// Named tag — references registered in tags/*.json
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct TagId<T> {
    pub id: ResourceLocation,
    _phantom: PhantomData<fn() -> T>,
}

/// A resolved tag — set of handles
#[derive(Clone)]
pub struct TagSet<T: Asset> {
    pub entries: Vec<Handle<T>>,
}

/// Global tag registry (one per registry type)
#[derive(Resource, Default)]
pub struct Tags<T: Asset> {
    pub tags: HashMap<TagId<T>, TagSet<T>>,
}
```

Tags are loaded after all registry entries via tag JSON files at
`data/*/tags/<registry_path>/*.json`. Each tag file lists entry IDs and/or
other tag IDs. Tags are not hot-reloadable via standard asset system — they
update via `ClientboundUpdateTagsPacket` or full data pack reload.

---

## Two Struct Pattern for Biome (and other network-synced types)

```rust
/// Full server-side biome (loaded from JSON via DIRECT_CODEC)
#[derive(Asset, Reflect, Deserialize)]
pub struct Biome {
    pub climate: ClimateSettings,
    pub effects: BiomeSpecialEffects,
    pub generation: GenerationSettings,  // Contains Handle<PlacedFeature> lists
    pub mob_settings: MobSpawnSettings,
}

/// Client-side biome subset (encoded in NETWORK_CODEC for registry sync)
#[derive(Serialize, Deserialize)]
pub struct BiomeNetwork {
    pub climate: ClimateSettings,   // Same as full biome
    pub effects: BiomeSpecialEffects, // Same as full biome
    // No generation or mob fields
}

impl From<&Biome> for BiomeNetwork {
    fn from(b: &Biome) -> Self {
        Self { climate: b.climate.clone(), effects: b.effects.clone() }
    }
}
```

When building the `RegistryData` packet (Configuration phase):
```rust
for biome in biomes.iter() {
    let network_form = BiomeNetwork::from(biome);
    let nbt = fastnbt::to_value(&network_form)?;
    packet.add_entry("minecraft:biome", biome_id, Some(nbt));
}
```

---

## Density Function Type Dispatch

In Java, `DensityFunction` uses `Codec.dispatch("type", ...)`. In Rust,
implement as a Serde tagged enum:

```rust
#[derive(Asset, Reflect, Clone)]
pub enum DensityFunction {
    // Constants
    Constant(f64),

    // Binary ops
    Add(Box<DensityFunction>, Box<DensityFunction>),
    Mul(Box<DensityFunction>, Box<DensityFunction>),
    Min(Box<DensityFunction>, Box<DensityFunction>),
    Max(Box<DensityFunction>, Box<DensityFunction>),

    // Unary
    Abs(Box<DensityFunction>),
    Square(Box<DensityFunction>),
    // ...

    // Noise
    Noise {
        noise: Handle<NormalNoiseParameters>,
        xz_scale: f64,
        y_scale: f64,
    },

    // Caching markers
    Interpolated(Box<DensityFunction>),
    FlatCache(Box<DensityFunction>),
    Cache2D(Box<DensityFunction>),
    CacheOnce(Box<DensityFunction>),
    CacheAllInCell(Box<DensityFunction>),

    // References to other registered density functions
    Reference(Handle<DensityFunction>),

    // Spline
    Spline(SplineDefinition),

    // Blending
    BlendDensity(Box<DensityFunction>),
    BlendAlpha,
    BlendOffset,

    // ... all 28 types
}
```

The loader parses JSON `"type"` field to discriminate, with special handling
for string references and numeric literals:

```rust
impl<'de> Deserialize<'de> for DensityFunction {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        match &value {
            Value::Number(n) => Ok(DensityFunction::Constant(n.as_f64().unwrap())),
            Value::String(s) => Ok(DensityFunction::Reference(/* resolve via ctx */)),
            Value::Object(_) => {
                let type_id: ResourceLocation = value["type"].as_str()
                    .ok_or_else(|| D::Error::missing_field("type"))?
                    .parse()?;
                // dispatch by type_id ...
            }
            _ => Err(D::Error::custom("expected number, string, or object")),
        }
    }
}
```

Note: `Handle<DensityFunction>` for string references requires `LoadContext` access,
which is not available inside `Deserialize`. Solution: use a raw intermediate type
during deserialization that holds a string ID, then convert in the loader:

```rust
/// Intermediate type during deserialization
pub enum DensityFunctionRaw {
    Reference(String),  // will become Handle<DensityFunction>
    Constant(f64),
    Inline(Box<DensityFunctionRaw>),
    // ...
}

// In DensityFunctionLoader::load():
async fn load(&self, reader, settings, ctx) -> Result<DensityFunction, _> {
    let raw: DensityFunctionRaw = serde_json::from_reader(reader)?;
    Ok(raw.resolve(ctx))  // resolves string refs to ctx.load(...)
}
```

---

## Feature / Placement Modifier Type Dispatch

Similar pattern for `ConfiguredFeature` and `PlacedFeature`:

```rust
#[derive(Asset, Reflect, Clone)]
pub struct ConfiguredFeature {
    pub feature_type: ResourceLocation,  // "minecraft:tree", etc.
    pub config: FeatureConfig,           // Dynamically typed config
}

#[derive(Clone)]
pub enum FeatureConfig {
    Tree(TreeConfiguration),
    Ore(OreConfiguration),
    Geode(GeodeConfiguration),
    RandomSelector(Vec<Handle<ConfiguredFeature>>, Vec<f32>),
    // ... 60+ variants
}
```

PlacementModifiers as a typed enum with trait:

```rust
pub trait PlacementModifier: Send + Sync + 'static {
    fn get_positions(
        &self,
        ctx: &PlacementContext,
        input: impl Iterator<Item = BlockPos>,
    ) -> impl Iterator<Item = BlockPos>;
}

// Or as enum (simpler, avoids dyn dispatch in hot path):
pub enum PlacementModifier {
    Biome,
    Count(IntProvider),
    HeightRange(HeightProvider),
    Heightmap(HeightmapType),
    InSquare,
    RarityFilter { chance: u32 },
    // ... 15 types
}
```

---

## Tag Loading Flow

Tags must load AFTER all registry entries (requires knowing all valid IDs):

```rust
fn load_tags(
    asset_server: Res<AssetServer>,
    biomes: Res<Assets<Biome>>,
    mut biome_tags: ResMut<Tags<Biome>>,
    tag_data: Res<LoadedTagData>,  // loaded raw tag JSON
) {
    // Resolve each tag file
    for (tag_id, raw_tag) in tag_data.biome_tags.iter() {
        let mut entries = Vec::new();
        for entry in &raw_tag.values {
            match entry {
                TagEntry::Item(id) => {
                    // Look up by resource location
                    if let Some(handle) = biomes.get_by_location(id) {
                        entries.push(handle.clone());
                    }
                }
                TagEntry::Tag(other_tag_id) => {
                    // Recursively include other tag's entries
                    if let Some(other_tag) = biome_tags.tags.get(other_tag_id) {
                        entries.extend(other_tag.entries.clone());
                    }
                }
            }
        }
        biome_tags.tags.insert(tag_id.clone(), TagSet { entries });
    }
}
```

---

## Numeric ID Assignment (for Network Protocol)

After the Configuration phase, integer IDs are assigned in packet array order.
These IDs are used in chunk data (biome palette indices). They must be stable
for the duration of a play session.

```rust
#[derive(Resource)]
pub struct RegistrySnapshot {
    pub biome_ids: HashMap<ResourceLocation, u32>,
    pub dimension_type_ids: HashMap<ResourceLocation, u32>,
    // ... for each synced registry
}

impl RegistrySnapshot {
    pub fn build(biomes: &Assets<Biome>, send_order: &[ResourceLocation]) -> Self {
        let biome_ids = send_order
            .iter()
            .enumerate()
            .map(|(i, loc)| (loc.clone(), i as u32))
            .collect();
        Self { biome_ids, .. }
    }
}
```

After a re-configuration (hot-reload via `StartConfiguration`):
1. Rebuild `RegistrySnapshot` with new IDs
2. Flush ALL chunk data from client's chunk cache (old IDs are invalid)
3. Resume play with fresh chunk requests

## See also

- [../worldgen/11-registry-dependency-graph.md](../worldgen/11-registry-dependency-graph.md) — Java registry dependency DAG that defines the tier structure
- [06-registry-redesign.md](06-registry-redesign.md) — detailed StaticRegistry<T> + Assets<T> design spec
- [08-worldgen-asset-integration.md](08-worldgen-asset-integration.md) — Bevy AssetLoader implementations for each registry type
