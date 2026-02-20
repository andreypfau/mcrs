# Bevy Plugin & ECS — Implementation Reference

Source: `~/IdeaProjects/bevy/crates/bevy_app/` and `~/IdeaProjects/bevy/crates/bevy_ecs/`
Relevant for: structuring MinecraftEnginePlugin, MinecraftCorePlugin, and scheduling systems.

---

## Plugin Trait (bevy_app/src/plugin.rs)

```rust
pub trait Plugin: Any + Send + Sync {
    /// Called once during App::add_plugins(). Register systems, resources, assets, events here.
    fn build(&self, app: &mut App);

    /// Called after all plugins are built but before the first update.
    /// Use for initialization that requires other plugins to be built first.
    fn finish(&self, app: &mut App) {}

    /// Called after App::finish(). Final cleanup.
    fn cleanup(&self, app: &mut App) {}

    /// Return false to allow multiple instances of the same plugin type.
    fn is_unique(&self) -> bool { true }

    fn name(&self) -> &str { core::any::type_name::<Self>() }

    /// Return false to defer building until ready (e.g., async resource unavailable).
    fn ready(&self, _app: &App) -> bool { true }
}
```

**Lifecycle:**
1. `build()` runs immediately when `app.add_plugins(MyPlugin)` is called
2. `ready()` is polled; plugin is held in `Adding` state until it returns `true`
3. `finish()` runs after ALL plugins return `ready() == true`
4. `cleanup()` runs after `finish()`

**`PluginsState` enum** — tracks plugin initialization phase:
```rust
pub enum PluginsState {
    Adding,    // Plugins being added via add_plugins()
    Ready,     // All plugins returned ready() == true
    Finished,  // finish() has been called on all plugins
    Cleaned,   // cleanup() has been called
}
```

**`Plugin::ready()` — gating on prerequisite resources:**

Use `ready()` when a plugin's `build()` requires resources that another plugin registers.
This avoids explicit ordering dependencies between `add_plugins()` calls:

```rust
/// DataPackPlugin waits until static registries are populated by MinecraftCorePlugin.
pub struct DataPackPlugin;

impl Plugin for DataPackPlugin {
    fn ready(&self, app: &App) -> bool {
        // Defer build() until MinecraftCorePlugin has registered all factory types
        app.world().contains_resource::<StaticRegistries>()
    }

    fn build(&self, app: &mut App) {
        // safe to use StaticRegistries here — ready() guarantees it exists
        app.add_systems(OnEnter(LoadingState::LoadingDatapacks), load_datapacks_from_json);
    }
}
```

**For Minecraft — three plugins:**

```rust
/// Infrastructure: asset sources, tag system, network framing
pub struct MinecraftEnginePlugin;

/// Type vocabulary: registers all static registry factories
pub struct MinecraftCorePlugin;

/// Content: loads built-in data pack JSON files
pub struct MinecraftDataPlugin;

impl Plugin for MinecraftEnginePlugin {
    fn build(&self, app: &mut App) {
        app
            .init_asset::<NormalNoiseParameters>()
            .init_asset_loader::<NormalNoiseLoader>()
            .init_asset::<DensityFunction>()
            .init_asset_loader::<DensityFunctionLoader>()
            .init_asset::<Biome>()
            .init_asset_loader::<BiomeLoader>()
            // ... all 40 dynamic registry types
            .init_resource::<StaticRegistries>()
            .init_resource::<Tags<Biome>>()
            .add_event::<WorldgenFreezeEvent>()
            .add_state::<AppState>()
            .add_systems(OnEnter(AppState::LoadingDataPack), trigger_worldgen_loads)
            .add_systems(
                Update,
                check_worldgen_complete.run_if(in_state(AppState::LoadingDataPack)),
            )
            .add_systems(OnEnter(AppState::WorldgenFreeze), resolve_tags);
    }
}

impl Plugin for MinecraftCorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StaticRegistries>();
    }

    fn finish(&self, app: &mut App) {
        // Runs after MinecraftEnginePlugin.build() — can access StaticRegistries
        let mut regs = app.world_mut().resource_mut::<StaticRegistries>();

        // Register all 28 DensityFunction types
        regs.density_function_types.insert(
            "minecraft:add".parse().unwrap(),
            Box::new(DensityFunctionAddFactory),
        );
        // ... all other types

        // Register all 60+ Feature types
        regs.feature_types.insert("minecraft:tree".parse().unwrap(), Box::new(TreeFeatureFactory));
        // ...
    }
}
```

---

## App Struct — Key Methods (bevy_app/src/app.rs)

```rust
// Plugin management
app.add_plugins(plugin_or_tuple_of_plugins)
app.is_plugin_added::<T>() -> bool
app.get_added_plugins::<T>() -> Vec<&T>

// System registration
app.add_systems(ScheduleLabel, systems)
app.configure_sets(ScheduleLabel, system_sets)
app.remove_systems_in_set(ScheduleLabel, set, policy)
app.register_system(system) -> SystemId  // push-based, not scheduled

// Resources
app.insert_resource(resource)       // replaces existing
app.init_resource::<R>()            // uses Default or FromWorld
app.insert_non_send_resource(resource)

// Assets
app.init_asset::<A>()               // registers Assets<A> resource
app.init_asset_loader::<L>()        // registers loader (extension-based routing)
app.register_asset_loader(loader)   // same but value-based

// Type system
app.register_type::<T>()            // registers in AppTypeRegistry (for reflection)
app.add_event::<E>()                // sets up event queue (prefer Messages<E> now)
app.add_message::<M>()              // new preferred message system

// Sub-apps
app.get_sub_app(label) -> Option<&SubApp>
app.get_sub_app_mut(label) -> Option<&mut SubApp>

// World access
app.world() -> &World
app.world_mut() -> &mut World
```

---

## PluginGroup — Grouping Related Plugins

```rust
pub struct MinecraftPlugins;

impl PluginGroup for MinecraftPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(MinecraftEnginePlugin)
            .add(MinecraftCorePlugin)
            .add(MinecraftDataPlugin)
    }
}

// Usage:
App::new()
    .add_plugins(DefaultPlugins)
    .add_plugins(MinecraftPlugins)
    .run();

// Or customize:
App::new()
    .add_plugins(MinecraftPlugins.build().disable::<MinecraftDataPlugin>())
    .run();
```

---

## States — Loading Lifecycle (bevy_ecs/src/schedule/state.rs)

```rust
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Bootstrap,        // Before any loading — register static types in Plugin::finish()
    LoadingDataPack,  // Dynamic registry JSON loading in progress
    WorldgenFreeze,   // All worldgen assets loaded — resolving tags, assigning IDs
    Playing,          // World running normally
    Reconfiguring,    // Hot-reload: client in Configuration protocol phase
}
```

**State-scoped systems:**
```rust
// Runs once when entering the state
app.add_systems(OnEnter(AppState::LoadingDataPack), trigger_worldgen_loads);
app.add_systems(OnEnter(AppState::WorldgenFreeze), resolve_tags_and_ids);
app.add_systems(OnEnter(AppState::Playing), spawn_world_entities);

// Runs every frame while in the state
app.add_systems(
    Update,
    check_worldgen_complete
        .run_if(in_state(AppState::LoadingDataPack))
);

// Runs once when exiting the state
app.add_systems(OnExit(AppState::Reconfiguring), flush_chunk_cache);
```

**Transition:**
```rust
fn check_worldgen_complete(
    biome_events: EventReader<AssetEvent<Biome>>,
    loading: Res<WorldgenLoading>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if loading.all_loaded() {
        next_state.set(AppState::WorldgenFreeze);
    }
}
```

**Multiple independent state machines:**

Bevy supports multiple `States` resources that run concurrently. Use separate enums
for orthogonal lifecycle concerns rather than one mega-enum:

```rust
/// Static type vocabulary loading (runs once at startup)
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum BootstrapState {
    #[default]
    Uninitialized,
    Loading,
    Ready,
}

/// Dynamic registry (data pack) loading
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum LoadingState {
    #[default]
    Uninitialized,
    LoadingDatapacks,
    ResolvingTags,
    Ready,
}

/// Game play state (independent from loading)
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum PlayState {
    #[default]
    MainMenu,
    Playing,
    Paused,
}

// Systems can condition on any combination:
app.add_systems(Update,
    generate_chunks.run_if(
        in_state(LoadingState::Ready).and(in_state(PlayState::Playing))
    )
);
```

**`StateTransitionSystems` ordering** — within each frame, state transitions run in this order:
```
DependentTransitions  → apply NextState<S> (from ResMut<NextState<S>>)
ExitSchedules         → run OnExit(old_state) systems
TransitionSchedules   → run OnTransition { from, to } systems
EnterSchedules        → run OnEnter(new_state) systems
```

`StateTransitionEvent<S>` is sent whenever a state changes:
```rust
fn on_any_state_change(mut events: EventReader<StateTransitionEvent<AppState>>) {
    for event in events.read() {
        info!("State changed from {:?} to {:?}", event.before, event.after);
    }
}
```

---

## SystemSet — Ordered System Groups

Define execution order within a schedule using `SystemSet`:

```rust
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChunkGenSet {
    StructureStarts,
    StructureReferences,
    Biomes,
    Noise,
    Surface,
    Carvers,
    Features,
    Light,
}

app.configure_sets(
    Update,
    (
        ChunkGenSet::StructureStarts,
        ChunkGenSet::StructureReferences,
        ChunkGenSet::Biomes,
        ChunkGenSet::Noise,
        ChunkGenSet::Surface,
        ChunkGenSet::Carvers,
        ChunkGenSet::Features,
        ChunkGenSet::Light,
    ).chain()  // each runs after the previous
     .run_if(in_state(AppState::Playing)),
);

// Assign systems to sets:
app.add_systems(Update, (
    generate_structure_starts.in_set(ChunkGenSet::StructureStarts),
    fill_biomes.in_set(ChunkGenSet::Biomes),
    generate_noise_terrain.in_set(ChunkGenSet::Noise),
    apply_surface_rules.in_set(ChunkGenSet::Surface),
    carve_caves.in_set(ChunkGenSet::Carvers),
    place_features.in_set(ChunkGenSet::Features),
));
```

**Ordering combinators:**
```rust
system_a.before(system_b)
system_b.after(system_a)
system_a.in_set(MySet)
(system_a, system_b).chain()   // a before b
```

---

## Run Conditions

```rust
// Built-in conditions:
system.run_if(in_state(AppState::Playing))
system.run_if(resource_exists::<MyResource>())
system.run_if(resource_changed::<MyResource>())
system.run_if(on_event::<MyEvent>())

// Combining:
system.run_if(
    in_state(AppState::Playing)
        .and(resource_exists::<WorldgenRegistries>())
)

// Custom condition:
fn worldgen_ready(worldgen: Option<Res<WorldgenRegistries>>) -> bool {
    worldgen.is_some()
}
system.run_if(worldgen_ready)
```

---

## Schedules — When Systems Run

| Schedule | When |
|----------|------|
| `PreStartup` | Before app starts, once |
| `Startup` | Once at app start |
| `PostStartup` | After startup, once |
| `First` | Every frame, before Update |
| `PreUpdate` | Every frame, before Update |
| `Update` | Every frame — main game logic |
| `PostUpdate` | Every frame, after Update |
| `Last` | Every frame, after PostUpdate |
| `OnEnter(State)` | Once when entering state |
| `OnExit(State)` | Once when exiting state |
| `OnTransition { from, to }` | Once on specific transition |
| `FixedUpdate` | Fixed timestep (default 64Hz) |

---

## Resources — Shared Data

```rust
// Define
#[derive(Resource, Default)]
pub struct WorldgenLoading {
    pub expected: HashSet<AssetId<Biome>>,
    pub loaded: HashSet<AssetId<Biome>>,
}

impl WorldgenLoading {
    pub fn mark_loaded(&mut self, id: AssetId<Biome>) {
        self.loaded.insert(id);
    }
    pub fn all_loaded(&self) -> bool {
        self.expected == self.loaded
    }
}

// Initialize — two options:
app.init_resource::<WorldgenLoading>();        // Default::default()
app.insert_resource(WorldgenLoading { .. });   // Manual construction

// Access in systems:
fn my_system(loading: Res<WorldgenLoading>, mut loading_mut: ResMut<WorldgenLoading>) { }

// FromWorld — init using world access:
impl FromWorld for StaticRegistries {
    fn from_world(world: &mut World) -> Self {
        // Can access other resources here
        StaticRegistries::default()
    }
}
```

---

## Events and Messages — Decoupled Communication

Bevy has two event systems. **`Messages<T>`** is the modern API (Bevy 0.15+); the older
`Events<T>` / `EventReader` / `EventWriter` still works but is being phased out.

**Modern: `Messages<T>`**
```rust
// Define (same Event derive works for both systems)
#[derive(Event)]
pub struct WorldgenFreezeEvent;

#[derive(Event)]
pub struct ChunkInvalidationEvent {
    pub chunk_pos: ChunkPos,
    pub min_status: ChunkStatus,
}

// Register
app.add_message::<WorldgenFreezeEvent>();
app.add_message::<ChunkInvalidationEvent>();

// Send — use Commands or direct world access
fn send_freeze(mut commands: Commands) {
    commands.send_message(WorldgenFreezeEvent);
}

// Receive — Messages<T> is a system parameter
fn handle_freeze(messages: Messages<WorldgenFreezeEvent>) {
    for event in messages.iter() {
        // worldgen is frozen
    }
}
```

**Legacy: `Events<T>` / `EventReader` / `EventWriter`** (still functional, use if Messages<T> unavailable)
```rust
// Register
app.add_event::<WorldgenFreezeEvent>();

// Send
fn send_freeze(mut writer: EventWriter<WorldgenFreezeEvent>) {
    writer.send(WorldgenFreezeEvent);
}

// Receive
fn handle_freeze(mut reader: EventReader<WorldgenFreezeEvent>) {
    for _ in reader.read() {
        // worldgen is frozen
    }
}
```

**Key difference**: `Messages<T>` drains automatically each frame (no double-read risk),
while `EventReader` must be read within 2 frames or events are lost.

---

## FromWorld — Resource Initialization With Dependencies

Use when a resource needs to be constructed from other resources already in the world:

```rust
#[derive(Resource)]
pub struct DimensionRandomState {
    pub seeded_noise: HashMap<ResourceLocation, NormalNoise>,
}

impl FromWorld for DimensionRandomState {
    fn from_world(world: &mut World) -> Self {
        let noise_params = world.resource::<Assets<NormalNoiseParameters>>();
        let settings = world.resource::<Assets<NoiseGeneratorSettings>>();
        // Wire up noise objects from loaded assets
        let seeded_noise = build_noise_map(noise_params, settings);
        Self { seeded_noise }
    }
}

// Initialized after worldgen freeze:
app.add_systems(OnEnter(AppState::Playing), |world: &mut World| {
    world.init_resource::<DimensionRandomState>();
});
```

---

## Three-Tier Plugin Architecture

```
┌──────────────────────────────────────────────────────────┐
│                   MinecraftEnginePlugin                  │
│  - AssetLoader registration (all registry types)         │
│  - Tag system (Tags<T> resources)                        │
│  - AppState definition and transitions                   │
│  - Network protocol plugin (packets, Configuration)      │
│  - AssetSource for data/ directory                       │
│  - Hot-reload propagation systems                        │
└──────────────────────────────────────────────────────────┘
                            │ depends on
┌──────────────────────────────────────────────────────────┐
│                  MinecraftCorePlugin                     │
│  - Registers all DensityFunction type factories          │
│  - Registers all Feature type factories (60+)            │
│  - Registers all PlacementModifier factories (15)        │
│  - Registers all SurfaceRule/Condition factories         │
│  - Registers all Carver, Structure, BiomeSource types    │
│  - Block static registry                                 │
│  - Entity type registry                                  │
└──────────────────────────────────────────────────────────┘
                            │ loads
┌──────────────────────────────────────────────────────────┐
│                  MinecraftDataPlugin                     │
│  - Triggers loading of all built-in JSON files           │
│  - Embedded via include_dir!() or file-based loader      │
│  - Holds strong Handles to keep built-in assets alive    │
│  - Can be REPLACED by a user data pack plugin            │
└──────────────────────────────────────────────────────────┘
```

**Registering static factories in CorePlugin::finish():**

```rust
impl Plugin for MinecraftCorePlugin {
    fn build(&self, app: &mut App) {
        // Nothing in build() — defer to finish() so EnginePlugin is ready
    }

    fn finish(&self, app: &mut App) {
        let mut regs = app.world_mut().resource_mut::<StaticRegistries>();

        // DensityFunction types
        use density::*;
        regs.register_df_type("minecraft:constant",       ConstantFactory);
        regs.register_df_type("minecraft:add",            AddFactory);
        regs.register_df_type("minecraft:mul",            MulFactory);
        regs.register_df_type("minecraft:noise",          NoiseFactory);
        regs.register_df_type("minecraft:spline",         SplineFactory);
        regs.register_df_type("minecraft:interpolated",   InterpolatedFactory);
        regs.register_df_type("minecraft:flat_cache",     FlatCacheFactory);
        // ... all 28 types

        // Feature types
        use features::*;
        regs.register_feature("minecraft:tree",           TreeFeatureFactory);
        regs.register_feature("minecraft:ore",            OreFeatureFactory);
        // ... all 60+ types

        // BiomeSource types
        regs.register_biome_source("minecraft:multi_noise", MultiNoiseBiomeSourceFactory);
        regs.register_biome_source("minecraft:the_end",     TheEndBiomeSourceFactory);
        regs.register_biome_source("minecraft:fixed",       FixedBiomeSourceFactory);
    }
}
```

**Loading built-in data in DataPlugin:**

```rust
impl Plugin for MinecraftDataPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BuiltinHandles>();
    }

    fn finish(&self, app: &mut App) {
        let asset_server = app.world().resource::<AssetServer>();

        // Load all vanilla JSON files — keep handles alive in resource
        let mut handles = app.world_mut().resource_mut::<BuiltinHandles>();

        // Load all noise files
        handles.noise.push(asset_server.load("data://minecraft/worldgen/noise/temperature.json"));
        // ... or scan directory

        // Load all biomes
        handles.biomes.push(asset_server.load("data://minecraft/worldgen/biome/plains.json"));
        handles.biomes.push(asset_server.load("data://minecraft/worldgen/biome/forest.json"));
        // ... all 65 biomes
    }
}

#[derive(Resource, Default)]
pub struct BuiltinHandles {
    pub noise: Vec<Handle<NormalNoiseParameters>>,
    pub density_functions: Vec<Handle<DensityFunction>>,
    pub biomes: Vec<Handle<Biome>>,
    // ... all registry types
}
```

---

## App Extension Trait Pattern

Plugins often expose a trait to extend App with domain-specific methods:

```rust
pub trait MinecraftAppExt {
    fn register_density_function_type<F: DensityFunctionFactory>(
        &mut self,
        id: &str,
        factory: F,
    ) -> &mut Self;

    fn register_feature_type<F: FeatureFactory>(
        &mut self,
        id: &str,
        factory: F,
    ) -> &mut Self;
}

impl MinecraftAppExt for App {
    fn register_density_function_type<F: DensityFunctionFactory>(
        &mut self,
        id: &str,
        factory: F,
    ) -> &mut Self {
        let mut regs = self.world_mut().resource_mut::<StaticRegistries>();
        regs.density_function_types.insert(id.parse().unwrap(), Box::new(factory));
        self
    }

    fn register_feature_type<F: FeatureFactory>(
        &mut self,
        id: &str,
        factory: F,
    ) -> &mut Self {
        let mut regs = self.world_mut().resource_mut::<StaticRegistries>();
        regs.feature_types.insert(id.parse().unwrap(), Box::new(factory));
        self
    }
}

// Usage in a third-party plugin:
impl Plugin for MyCustomFeaturesPlugin {
    fn build(&self, app: &mut App) {
        app.register_feature_type("myplugin:custom_crystal", CustomCrystalFactory);
    }
}
```

---

## ECS Component Patterns for Chunks

```rust
/// Marks a chunk entity with its current generation status
#[derive(Component, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
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

/// Chunk position in chunk coordinates
#[derive(Component, Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct ChunkPos { pub x: i32, pub z: i32 }

/// Dimension this chunk belongs to
#[derive(Component, Clone, Debug)]
pub struct InDimension(pub ResourceLocation);

/// Marks a chunk that needs re-generation from this status
#[derive(Component)]
pub struct NeedsRegenFrom(pub ChunkStatus);

// System: find chunks that need noise generation
fn generate_noise_chunks(
    mut query: Query<
        (Entity, &ChunkPos, &InDimension, &mut ChunkStatus),
        With<NeedsRegenFrom>,
    >,
    dim_states: Query<(&DimensionRandomState, &DimensionEntity)>,
    mut commands: Commands,
) {
    for (entity, pos, dim, mut status) in &mut query {
        if *status < ChunkStatus::Noise {
            // ... run noise generation
            *status = ChunkStatus::Noise;
            commands.entity(entity).remove::<NeedsRegenFrom>();
        }
    }
}
```

---

## Summary of Bevy APIs Used

| Task | Bevy API |
|------|----------|
| Register asset type | `app.init_asset::<T>()` |
| Register asset loader | `app.init_asset_loader::<L>()` |
| Load asset | `asset_server.load("path/to/file.json")` |
| Declare dep in loader | `ctx.load("path")` → `Handle<T>` |
| Create sub-asset | `ctx.add_labeled_asset("Label", value)` |
| Watch for all deps loaded | `AssetEvent::LoadedWithDependencies { id }` |
| Watch for asset change | `AssetEvent::Modified { id }` |
| Enable hot reload | `AssetPlugin { watch_for_changes_override: Some(true) }` |
| Gate plugin on resource | `fn ready(&self, app: &App) -> bool { app.world().contains_resource::<R>() }` |
| Track plugin init phase | `app.plugins_state()` → `PluginsState` enum |
| State transitions | `NextState::set(AppState::Playing)` |
| State-scoped systems | `OnEnter(State)` / `OnExit(State)` |
| Specific transition | `OnTransition { from: A, to: B }` |
| State change events | `EventReader<StateTransitionEvent<S>>` |
| Multiple state machines | Separate `#[derive(States)]` enums, `add_state::<S>()` each |
| Modern events (send) | `commands.send_message(MyEvent)` |
| Modern events (receive) | `Messages<MyEvent>` system param |
| Legacy events (send) | `EventWriter<MyEvent>` |
| Legacy events (receive) | `EventReader<MyEvent>` |
| Conditional systems | `.run_if(in_state(State::X))` |
| Ordered system sets | `.configure_sets(.chain())` |
| Register plugin | `app.add_plugins(MyPlugin)` |
| Init resource | `app.init_resource::<R>()` |
| Custom App methods | `impl MyAppExt for App { ... }` |
