# Crate Restructure Plan

How to reorganize the `mcrs` workspace crates to align with the redesigned
registry, tag, and asset systems. Guided by the three-plugin architecture
from `02-bevy-plugin-ecs.md`.

---

## Current Crate Layout (as of Feb 2026)

```
mcrs/crates/
├── mcrs_core/             — NEW ✅: ResourceLocation, StaticRegistry, StaticTags, AppState
├── mcrs_registry/         — LEGACY: Registry<E>, RegistryId, Holder (still used by enchantments)
├── mcrs_engine/           — Minimal; world, entity, math modules
├── mcrs_minecraft/        — Large: tags, config, world, network glue (not yet split)
├── mcrs_minecraft_worldgen/ — Worldgen engine + Tier 0 Bevy asset loaders ✅
├── mcrs_protocol/         — Protocol encoding (KEEP)
├── mcrs_protocol_macros/  — Derive macros (KEEP)
├── mcrs_network/          — Network I/O (KEEP)
├── mcrs_nbt/              — NBT (KEEP)
└── mcrs_random/           — RNG (KEEP)
```

**Completed so far:**
- `mcrs_core` added with StaticRegistry<T>, StaticTags<T>, TagFile, AppState (Step 1)
- `mcrs_minecraft` block/item tags migrated to demand-driven StaticTags (Step 2 )
- `mcrs_minecraft_worldgen` has Tier 0 asset loaders (NormalNoise, DimensionType, ConfiguredWorldCarver, StructureProcessorList) (Step 3)

**Still needed:**
- Crate rename from `mcrs_*` to `mcrs_*` — NOT done, see Renaming Strategy below
- Split `mcrs_minecraft` into server logic + tag system — NOT done
- Bevy integration for density functions (tiers 1–9) — NOT done
- Delete `mcrs_registry` after enchantment system migrated — NOT done

---

## Target Crate Layout

```
mcrs/crates/
│
│  ── INFRASTRUCTURE ──────────────────────────────────────────
├── mcrs_core/               # NEW: foundation types and plugin infrastructure
│   ├── src/
│   │   ├── lib.rs
│   │   ├── resource_location.rs  # ResourceLocation type + rl!() macro
│   │   ├── registry/
│   │   │   ├── static_registry.rs   # StaticRegistry<T>, StaticId<T>
│   │   │   └── snapshot.rs          # RegistrySnapshot (stable network IDs)
│   │   ├── tag/
│   │   │   ├── file.rs              # TagFile asset + TagFileLoader
│   │   │   ├── static_tags.rs       # StaticTags<T>
│   │   │   └── dynamic_tags.rs      # Tags<T>, TagKey<T>, TagSet<T>
│   │   └── state.rs                 # AppState enum, loading lifecycle
│
│  ── STATIC TYPE VOCABULARY ──────────────────────────────────
├── mcrs_vanilla/            # RENAMED from parts of mcrs_engine + mcrs_minecraft
│   ├── src/
│   │   ├── lib.rs                   # MinecraftCorePlugin
│   │   ├── block/
│   │   │   ├── mod.rs               # Block struct, BlockState
│   │   │   └── registry.rs          # declare_blocks! + registration system
│   │   ├── item/
│   │   │   └── registry.rs          # declare_items!
│   │   ├── entity/
│   │   │   └── registry.rs          # declare_entity_types!
│   │   └── sound/
│   │       └── registry.rs          # SoundEvent registry
│
│  ── WORLDGEN ALGORITHMS + ASSET LOADERS ─────────────────────
├── mcrs_worldgen/           # RENAMED from mcrs_minecraft_worldgen + loaders
│   ├── src/
│   │   ├── lib.rs                   # WorldgenAssetsPlugin
│   │   ├── density_function/
│   │   │   ├── mod.rs               # KEEP: optimized DensityFunctionStack
│   │   │   ├── unresolved.rs        # NEW: UnresolvedDensityFunction asset
│   │   │   └── loader.rs            # NEW: UnresolvedDensityFunctionLoader
│   │   ├── noise/
│   │   │   ├── mod.rs               # KEEP: ImprovedNoise, OctavePerlinNoise
│   │   │   ├── normal_noise.rs      # KEEP: NoiseSampler
│   │   │   └── loader.rs            # NEW: NormalNoiseLoader (trivial)
│   │   ├── spline/
│   │   │   └── mod.rs               # KEEP: CubicSpline
│   │   ├── climate.rs               # KEEP: ClimateParameters, ParamPoint
│   │   ├── biome/
│   │   │   ├── mod.rs               # Biome asset type
│   │   │   └── loader.rs            # BiomeLoader
│   │   ├── noise_settings/
│   │   │   ├── mod.rs               # NoiseGeneratorSettings asset
│   │   │   └── loader.rs            # NoiseGeneratorSettingsLoader
│   │   ├── feature/
│   │   │   ├── configured.rs        # ConfiguredFeature asset
│   │   │   ├── placed.rs            # PlacedFeature asset
│   │   │   └── loader.rs            # ConfiguredFeatureLoader, PlacedFeatureLoader
│   │   ├── structure/
│   │   │   ├── mod.rs               # Structure, StructureSet, StructureTemplatePool
│   │   │   └── loader.rs            # All structure loaders
│   │   ├── carver/
│   │   │   ├── mod.rs               # ConfiguredWorldCarver
│   │   │   └── loader.rs            # CarverLoader
│   │   ├── dimension/
│   │   │   ├── mod.rs               # DimensionType, LevelStem
│   │   │   └── loader.rs            # DimensionTypeLoader, LevelStemLoader
│   │   ├── compile.rs               # NEW: compile_density_functions system
│   │   └── handles.rs               # WorldgenHandles resource
│
│  ── PROTOCOL ────────────────────────────────────────────────
├── mcrs_protocol/           # RENAMED from mcrs_protocol (keep content)
├── mcrs_protocol_macros/    # RENAMED from mcrs_protocol_macros (keep)
│
│  ── NETWORK ─────────────────────────────────────────────────
├── mcrs_network/            # RENAMED from mcrs_network (keep content)
│
│  ── SERVER LOGIC ────────────────────────────────────────────
├── mcrs_server/             # RENAMED from mcrs_minecraft (trimmed)
│   ├── src/
│   │   ├── lib.rs                   # ServerPlugin (much smaller now)
│   │   ├── login.rs
│   │   ├── configuration.rs         # Network Configuration phase
│   │   ├── keep_alive.rs
│   │   ├── world/
│   │   │   ├── mod.rs               # WorldPlugin
│   │   │   ├── chunk.rs             # Chunk generation pipeline
│   │   │   └── dimension.rs         # Dimension spawning
│   │   └── client_info.rs
│
│  ── UTILITIES ───────────────────────────────────────────────
├── mcrs_nbt/                # RENAMED from mcrs_nbt (keep)
└── mcrs_random/             # RENAMED from mcrs_random (keep)
```

---

## Plugin Hierarchy

```
ServerPlugin (mcrs_server)
    └── MinecraftEnginePlugin (mcrs_core)
        └── MinecraftCorePlugin (mcrs_vanilla)
            └── WorldgenAssetsPlugin (mcrs_worldgen)
                └── TagPlugin (mcrs_core)
```

```rust
// mcrs_core: infrastructure + state machine
pub struct MinecraftEnginePlugin;
impl Plugin for MinecraftEnginePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AssetPlugin::default())
           // Static registries (empty, populated by CorePlugin)
           .init_resource::<StaticRegistry<Block>>()
           .init_resource::<StaticRegistry<Item>>()
           .init_resource::<StaticRegistry<EntityType>>()
           // State machine
           .init_state::<AppState>()
           // Tag infrastructure
           .add_plugins(TagPlugin)
           // Worldgen assets
           .add_plugins(WorldgenAssetsPlugin);
    }
}

// mcrs_vanilla: registers all static type factories
pub struct MinecraftCorePlugin;
impl Plugin for MinecraftCorePlugin {
    fn build(&self, app: &mut App) { /* nothing */ }

    fn finish(&self, app: &mut App) {
        // Populate StaticRegistry<Block>, StaticRegistry<Item>, etc.
        let mut blocks = app.world_mut().resource_mut::<StaticRegistry<Block>>();
        blocks.register(rl!("minecraft:stone"), &STONE);
        blocks.register(rl!("minecraft:air"), &AIR);
        // ... all 900+ blocks

        let mut items = app.world_mut().resource_mut::<StaticRegistry<Item>>();
        // ... all items
    }
}

// mcrs_worldgen: registers asset types + loaders, triggers loading
pub struct WorldgenAssetsPlugin;
impl Plugin for WorldgenAssetsPlugin {
    fn build(&self, app: &mut App) {
        // Register all dynamic registry asset types
        app.init_asset::<NormalNoise>()
           .init_asset_loader::<NormalNoiseLoader>()
           .init_asset::<UnresolvedDensityFunction>()
           .init_asset_loader::<UnresolvedDensityFunctionLoader>()
           .init_asset::<NoiseGeneratorSettings>()
           .init_asset_loader::<NoiseGeneratorSettingsLoader>()
           .init_asset::<Biome>()
           .init_asset_loader::<BiomeLoader>()
           // ... all 40 types
           .init_resource::<WorldgenHandles>()
           // Systems
           .add_systems(OnEnter(AppState::LoadingDataPack), trigger_worldgen_loads)
           .add_systems(Update,
               check_worldgen_ready.run_if(in_state(AppState::LoadingDataPack))
           )
           .add_systems(OnEnter(AppState::WorldgenFreeze), (
               compile_density_functions,
               build_registry_snapshot,
           ).chain());
    }
}
```

---

## `mcrs_core` Dependency Graph

```
mcrs_core depends on:
  - bevy_ecs, bevy_app, bevy_asset
  - mcrs_protocol (for protocol types like ResourceLocation in packet context)
  - mcrs_random (for XoroshiroRandom used in RegistrySnapshot seeding)

mcrs_vanilla depends on:
  - mcrs_core
  - mcrs_protocol

mcrs_worldgen depends on:
  - mcrs_core
  - mcrs_vanilla (for BlockState in default_block/default_fluid)
  - mcrs_random
  - mcrs_protocol

mcrs_server depends on:
  - mcrs_core
  - mcrs_vanilla
  - mcrs_worldgen
  - mcrs_network
  - mcrs_protocol
```

---

## Renaming Strategy

If renaming crates is disruptive, keep the `mcrs_` prefix but reorganize:

| Old Crate | New Crate | Action |
|-----------|-----------|--------|
| `mcrs_registry` | `mcrs_core` | Rewrite contents completely |
| `mcrs_engine` | Merge into `mcrs_core` | Move world/entity/math to server |
| `mcrs_minecraft` | `mcrs_server` | Remove tag/registry code; keep game logic |
| `mcrs_minecraft_worldgen` | `mcrs_worldgen` | Add asset integration layer |
| `mcrs_protocol` | `mcrs_protocol` | Keep as-is |
| `mcrs_protocol_macros` | `mcrs_protocol_macros` | Keep as-is |
| `mcrs_network` | `mcrs_network` | Keep as-is |
| `mcrs_nbt` | `mcrs_nbt` | Keep as-is |
| `mcrs_random` | `mcrs_random` | Keep as-is |

---

## Migration Order

Do these in order to avoid breaking the build at each step:

### Step 1: Add `mcrs_core` alongside existing `mcrs_registry`

Create `mcrs_core` with:
- `ResourceLocation` type
- `StaticRegistry<T>`, `StaticId<T>`
- `AppState` enum + loading lifecycle systems
- `TagFile` asset + `TagFileLoader`
- `StaticTags<T>`, `Tags<T>`, `TagKey<T>`

Keep `mcrs_registry` compiling — don't delete yet.

### Step 2: Migrate `mcrs_minecraft` block/item registries to `mcrs_core`

Move `StaticRegistry<Block>` usage. Update `BlockTagPlugin` to use `StaticTags<Block>`.
This breaks the dependency on `TagRegistry<&'static Block>` + `Registry<&'static Block>`.

### Step 3: Add asset loaders in `mcrs_worldgen`

Add `NormalNoiseLoader`, `UnresolvedDensityFunctionLoader`, `DimensionTypeLoader` (cleanup).
Add `WorldgenHandles` resource and `trigger_worldgen_loads` system.
Test: server starts and loads JSON without crashing.

### Step 4: Add `WorldgenFreeze` systems

Add `compile_density_functions`, `build_registry_snapshot`, tag resolution systems.
Wire `AppState` state machine.

### Step 5: Delete `mcrs_registry`

Remove `mcrs_registry` from workspace. Fix all compilation errors:
- Replace `Registry<T>` → `StaticRegistry<T>` or `Assets<T>`
- Replace `RegistryId<T>` → `StaticId<T>` or `AssetId<T>`
- Replace `Holder<T>` → `Handle<T>`

### Step 6: Restructure `mcrs_minecraft` → `mcrs_server`

Move tag system to `mcrs_core`. Move worldgen integration to `mcrs_worldgen`.
Keep only actual server game logic in `mcrs_server`.

---

## Feature Flags

```toml
# mcrs_worldgen/Cargo.toml
[features]
default = ["bevy", "serde", "lazy-range-choice", "surface-skip"]
bevy = ["dep:bevy_app", "dep:bevy_ecs", "dep:bevy_asset", "dep:bevy_tasks"]
serde = ["dep:serde", "dep:serde_json"]

# Worldgen optimizations (keep existing flags)
lazy-range-choice = []   # Skip 38% of Zone B entries
surface-skip = []         # Skip 44.6% of above-surface sections
```

```toml
# mcrs_core/Cargo.toml
[features]
default = ["bevy"]
bevy = ["dep:bevy_app", "dep:bevy_ecs", "dep:bevy_asset"]
```
