# Tag System Redesign

Complete specification for a demand-driven tag system.
**No directory scanning. No `load_folder`. Tags are only loaded when something references them.**

---

## Core Principle: Demand-Driven Loading

Tags are loaded through exactly two mechanisms:

1. **From JSON asset loaders** — when a JSON file contains `"biomes": "#minecraft:is_ocean"`,
   the loader calls `ctx.load()` to get a `Handle<TagFile>`. This is automatic.

2. **From code** — game code that needs a specific tag declares it as a `TagKey<T>` constant
   and triggers its load explicitly. The `TagKey` knows its own asset path.

**No scanning, no `load_folder`, no directory enumeration — ever.**

Tags for nested references inside tag files (`#minecraft:base_stone_overworld` inside another tag)
are loaded recursively by `TagFileLoader` via `ctx.load()` — same mechanism as (1).

---

## `TagRegistryType` — Path Derivation

Each type that participates in the tag system knows its registry path segment:

```rust
/// Implemented by types that have tags.
/// Provides the path segment used to derive tag file asset paths.
pub trait TagRegistryType: 'static + Send + Sync {
    /// Registry path segment for tag files.
    /// Appended as: "{namespace}/tags/{REGISTRY_PATH}/{tag_path}.json"
    ///
    /// Examples:
    ///   Block  → "block"
    ///   Item   → "item"
    ///   Biome  → "worldgen/biome"
    ///   Structure → "worldgen/structure"
    const REGISTRY_PATH: &'static str;
}

impl TagRegistryType for Block         { const REGISTRY_PATH: &'static str = "block"; }
impl TagRegistryType for Item          { const REGISTRY_PATH: &'static str = "item"; }
impl TagRegistryType for EntityType    { const REGISTRY_PATH: &'static str = "entity_type"; }
impl TagRegistryType for GameEvent     { const REGISTRY_PATH: &'static str = "game_event"; }
// Dynamic (Asset) types:
impl TagRegistryType for Biome         { const REGISTRY_PATH: &'static str = "worldgen/biome"; }
impl TagRegistryType for Structure     { const REGISTRY_PATH: &'static str = "worldgen/structure"; }
impl TagRegistryType for ConfiguredWorldCarver { const REGISTRY_PATH: &'static str = "worldgen/configured_carver"; }
```

---

## `TagKey<T>` — Typed Tag Identifier

```rust
/// A typed, named reference to a tag, with built-in path derivation.
/// Declare as a static constant; load on demand via asset_server or ctx.load().
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct TagKey<T: TagRegistryType> {
    pub id: ResourceLocation,
    _marker: PhantomData<fn() -> T>,
}

impl<T: TagRegistryType> TagKey<T> {
    /// Construct from a static string (for use in const context).
    pub const fn of(namespace: &'static str, path: &'static str) -> Self {
        Self {
            id: ResourceLocation::new_static(namespace, path),
            _marker: PhantomData,
        }
    }

    /// Construct from a runtime ResourceLocation.
    pub fn from_loc(id: ResourceLocation) -> Self {
        Self { id, _marker: PhantomData }
    }

    /// Derives the asset path for this tag's JSON file.
    ///
    /// TagKey<Block>{ id: "minecraft:mineable/pickaxe" }
    ///   → "minecraft/tags/block/mineable/pickaxe.json"
    ///
    /// TagKey<Biome>{ id: "minecraft:is_ocean" }
    ///   → "minecraft/tags/worldgen/biome/is_ocean.json"
    pub fn asset_path(&self) -> String {
        format!("{}/tags/{}/{}.json",
            self.id.namespace,
            T::REGISTRY_PATH,
            self.id.path)
    }

    /// Explicitly trigger loading via AssetServer. Returns a Handle<TagFile>.
    /// Call this once during plugin initialization for tags declared in code.
    pub fn load(&self, asset_server: &AssetServer) -> Handle<TagFile> {
        asset_server.load(self.asset_path())
    }

    /// Load from within an AssetLoader (declares as dependency of the parent).
    pub fn load_from_context<'a>(&self, ctx: &mut LoadContext<'a>) -> Handle<TagFile> {
        ctx.load(self.asset_path())
    }
}
```

**Declared as static constants in game code:**

```rust
// mc_vanilla/src/tag/block.rs — all vanilla block tags
pub static MINEABLE_PICKAXE:     TagKey<Block> = TagKey::of("minecraft", "mineable/pickaxe");
pub static MINEABLE_AXE:         TagKey<Block> = TagKey::of("minecraft", "mineable/axe");
pub static MINEABLE_SHOVEL:      TagKey<Block> = TagKey::of("minecraft", "mineable/shovel");
pub static LOGS:                 TagKey<Block> = TagKey::of("minecraft", "logs");
pub static LEAVES:               TagKey<Block> = TagKey::of("minecraft", "leaves");
pub static NEEDS_CORRECT_TOOL:   TagKey<Block> = TagKey::of("minecraft", "needs_correct_tool_for_drops");
pub static DRAGON_IMMUNE:        TagKey<Block> = TagKey::of("minecraft", "dragon_immune");
// ... all ~200 vanilla block tags

// mc_vanilla/src/tag/item.rs
pub static LOGS_THAT_BURN:       TagKey<Item> = TagKey::of("minecraft", "logs_that_burn");
pub static SWORDS:               TagKey<Item> = TagKey::of("minecraft", "swords");
// ... all ~150 vanilla item tags

// mc_vanilla/src/tag/biome.rs
pub static IS_OCEAN:             TagKey<Biome> = TagKey::of("minecraft", "is_ocean");
pub static IS_OVERWORLD:         TagKey<Biome> = TagKey::of("minecraft", "is_overworld");
// ... all ~30 vanilla biome tags
```

---

## `TagFile` Asset — The Loading Primitive

Registry-type-agnostic. Just a list of identifiers and nested tag references.

```rust
/// Parsed content of a single tag JSON file.
/// Agnostic of which registry type it belongs to.
#[derive(Asset, TypePath, Debug)]
pub struct TagFile {
    pub entries: Vec<TagFileEntry>,
    pub replace: bool,
}

#[derive(Debug)]
pub enum TagFileEntry {
    /// Direct element reference: "minecraft:stone"
    Element(ResourceLocation),
    /// Optional element: { "id": "minecraft:foo", "required": false }
    OptionalElement(ResourceLocation),
    /// Nested tag reference: "#minecraft:base_stone_overworld"
    /// Loaded as another TagFile via ctx.load() — Bevy tracks as dependency.
    Tag(Handle<TagFile>),
}

impl VisitAssetDependencies for TagFile {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        for entry in &self.entries {
            if let TagFileEntry::Tag(handle) = entry {
                visit(handle.id().untyped());
            }
        }
    }
}
```

### `TagFileLoader`

```rust
#[derive(Default, TypePath)]
pub struct TagFileLoader;

impl AssetLoader for TagFileLoader {
    type Asset = TagFile;
    type Settings = ();
    type Error = TagFileLoadError;

    async fn load(&self, reader: &mut dyn Reader, _: &(), ctx: &mut LoadContext)
        -> Result<TagFile, TagFileLoadError>
    {
        let bytes = reader.read_to_end().await?;
        let raw: TagFileJson = serde_json::from_slice(&bytes)?;

        let entries = raw.values.into_iter().map(|entry| {
            if entry.is_tag_ref {
                // "#minecraft:logs" → derive sibling tag path and load it
                let sibling_path = derive_sibling_tag_path(entry.id, ctx.path());
                let handle = ctx.load::<TagFile>(sibling_path);
                TagFileEntry::Tag(handle)
            } else if entry.required {
                TagFileEntry::Element(entry.id)
            } else {
                TagFileEntry::OptionalElement(entry.id)
            }
        }).collect();

        Ok(TagFile { entries, replace: raw.replace })
    }
}

/// Derive the asset path for a nested tag reference.
///
/// current path:  "minecraft/tags/block/mineable/pickaxe.json"
/// tag ref id:    "minecraft:base_stone_overworld"
/// result:        "minecraft/tags/block/base_stone_overworld.json"
///
/// The registry type segment ("block", "worldgen/biome", etc.) is preserved
/// from the current file's path — nested tags must be in the same registry.
fn derive_sibling_tag_path(tag_id: ResourceLocation, current: &AssetPath) -> String {
    // Extract the registry segment from the current path
    // Pattern: "{ns}/tags/{registry_segment}/{path}.json"
    let path_str = current.path().to_str().unwrap_or("");
    let tags_marker = "/tags/";
    if let Some(tags_idx) = path_str.find(tags_marker) {
        let after_ns = &path_str[..tags_idx];  // "minecraft"
        let after_tags = &path_str[tags_idx + tags_marker.len()..]; // "block/mineable/pickaxe.json"

        // Find the registry segment (everything before the last slash-separated path component)
        // For "block" registry: "block"
        // For "worldgen/biome" registry: "worldgen/biome"
        // Heuristic: find the "registered" segment by checking what comes before the first
        // segment that isn't a known registry prefix
        let registry_segment = extract_registry_segment(after_tags);

        return format!("{}/tags/{}/{}.json",
            tag_id.namespace,
            registry_segment,
            tag_id.path);
    }
    // Fallback: use flat path
    format!("{}/tags/{}.json", tag_id.namespace, tag_id.path)
}
```

**Alternative approach**: pass the registry segment as `TagFileLoader::Settings`:

```rust
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct TagFileSettings {
    /// e.g., "block", "item", "worldgen/biome"
    pub registry_segment: String,
}

impl AssetLoader for TagFileLoader {
    type Settings = TagFileSettings;
    // ...
    async fn load(&self, reader, settings: &TagFileSettings, ctx) -> Result<TagFile> {
        // ...
        let handle = ctx.loader()
            .with_settings::<TagFileSettings>(|s| s.registry_segment = settings.registry_segment.clone())
            .load::<TagFile>(format!("{}/tags/{}/{}.json",
                tag_id.namespace, settings.registry_segment, tag_id.path));
        // ...
    }
}

// When loading a tag from a TagKey<T>:
impl<T: TagRegistryType> TagKey<T> {
    pub fn load_with_settings(&self, asset_server: &AssetServer) -> Handle<TagFile> {
        asset_server.load_with_settings(self.asset_path(), |s: &mut TagFileSettings| {
            s.registry_segment = T::REGISTRY_PATH.to_string();
        })
    }
}
```

This is more explicit and removes path-parsing heuristics.

---

## Two-Flavor Resolved Storage

### `StaticTags<T>` — For Static Registry Types

```rust
/// Resolved tags for compile-time-fixed types (Block, Item, EntityType).
/// Maps TagKey → Vec of StaticId entries.
/// Populated once in OnEnter(WorldgenFreeze).
#[derive(Resource, Default)]
pub struct StaticTags<T: TagRegistryType + 'static> {
    tags: HashMap<ResourceLocation, Vec<StaticId<T>>>,
    /// Loaded handles — kept alive until resolved, then can be dropped
    handles: HashMap<ResourceLocation, Handle<TagFile>>,
}

impl<T: TagRegistryType> StaticTags<T> {
    /// Register a tag to be loaded. Call in plugin build().
    pub fn request(&mut self, key: &TagKey<T>, asset_server: &AssetServer) {
        let handle = key.load(asset_server);
        self.handles.insert(key.id.clone(), handle);
    }

    /// After resolution: look up entries in a tag.
    pub fn get(&self, key: &TagKey<T>) -> Option<&[StaticId<T>]> {
        self.tags.get(&key.id).map(Vec::as_slice)
    }

    pub fn contains(&self, key: &TagKey<T>, id: StaticId<T>) -> bool {
        self.tags.get(&key.id).map(|v| v.contains(&id)).unwrap_or(false)
    }
}
```

### `Tags<T>` — For Dynamic Registry Types (Asset)

```rust
/// Resolved tags for runtime-loaded asset types (Biome, Structure, ...).
/// Maps TagKey → Vec<Handle<T>>.
/// Populated once in OnEnter(WorldgenFreeze).
#[derive(Resource, Default)]
pub struct Tags<T: Asset + TagRegistryType> {
    tags: HashMap<ResourceLocation, Vec<Handle<T>>>,
    /// Loaded handles (from JSON asset loaders via ctx.load())
    handles: HashMap<ResourceLocation, Handle<TagFile>>,
}

impl<T: Asset + TagRegistryType> Tags<T> {
    pub fn get(&self, key: &TagKey<T>) -> Option<&[Handle<T>]> {
        self.tags.get(&key.id).map(Vec::as_slice)
    }

    pub fn contains(&self, key: &TagKey<T>, handle: &Handle<T>) -> bool {
        self.tags.get(&key.id).map(|v| v.contains(handle)).unwrap_or(false)
    }
}
```

---

## How Tags Are Loaded (Three Paths, No Scanning)

### Path 1: From `TagKey` constants in game code

Plugin `build()` requests specific tags by key. The tag's `asset_path()` derives the file path.

```rust
impl Plugin for BlockBehaviorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, register_block_tags);
    }
}

fn register_block_tags(
    mut tags: ResMut<StaticTags<Block>>,
    asset_server: Res<AssetServer>,
) {
    use crate::tag::block::*;
    // Only tags used by this plugin are loaded:
    tags.request(&MINEABLE_PICKAXE, &asset_server);
    tags.request(&MINEABLE_AXE, &asset_server);
    tags.request(&MINEABLE_SHOVEL, &asset_server);
    tags.request(&NEEDS_CORRECT_TOOL, &asset_server);
    tags.request(&LOGS, &asset_server);
    // ... etc.
}
```

For vanilla, `mc_vanilla` declares and requests all vanilla tags. Any data pack
plugin that adds new tags also calls `tags.request(...)` for its tags.

### Path 2: From JSON asset loaders (dynamic assets)

When `StructureLoader` parses `"biomes": "#minecraft:is_overworld"`, it calls:

```rust
// Inside StructureLoader::load():
let biomes_tag: Handle<TagFile> = tag_key.load_from_context(ctx);
// OR directly:
let biomes_tag: Handle<TagFile> = ctx.load_with_settings(
    "minecraft/tags/worldgen/biome/is_overworld.json",
    |s: &mut TagFileSettings| s.registry_segment = "worldgen/biome".into(),
);
```

This handle becomes a dependency of the `Structure` asset. Bevy waits for it before
firing `LoadedWithDependencies` on the `Structure`.

```rust
#[derive(Asset, TypePath)]
pub struct Structure {
    pub biomes: Handle<TagFile>,  // loaded from "#minecraft:is_overworld"
    pub start_pool: Handle<StructureTemplatePool>,
    // ...
}

impl VisitAssetDependencies for Structure {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        visit(self.biomes.id().untyped());
        visit(self.start_pool.id().untyped());
    }
}
```

The `Tags<Biome>` resource is populated during `WorldgenFreeze` by reading all
`Handle<TagFile>` references that were loaded during JSON parsing.

### Path 3: Nested tag references inside tag files

`TagFileLoader` automatically calls `ctx.load()` for any `#tag` reference found
in a tag file. No additional code needed. Bevy's dependency system ensures the
parent `TagFile` is only `LoadedWithDependencies` after all nested tags resolve.

```
MINEABLE_PICKAXE (loaded by game code)
    → minecraft/tags/block/mineable/pickaxe.json
        → #minecraft:base_stone_overworld  (ctx.load() in TagFileLoader)
            → minecraft/tags/block/base_stone_overworld.json
                → #minecraft:stone_buttons (ctx.load())
                    → ...
```

---

## Tag Resolution — OnEnter(WorldgenFreeze)

After all assets (and their tag dependencies) are `LoadedWithDependencies`, resolve
tag file entries into typed registry references. This runs exactly once.

### Resolving Static Tags (Block example)

```rust
fn resolve_block_tags(
    mut block_tags: ResMut<StaticTags<Block>>,
    tag_files: Res<Assets<TagFile>>,
    block_registry: Res<StaticRegistry<Block>>,
) {
    // The handles map contains all tags requested by game code
    let handles: Vec<(ResourceLocation, Handle<TagFile>)> =
        block_tags.handles.drain().collect();

    for (tag_id, handle) in handles {
        let entries = if let Some(tag_file) = tag_files.get(&handle) {
            resolve_static_tag_file(tag_file, &tag_files, &block_registry, tag_file.replace)
        } else {
            // Should not happen after LoadedWithDependencies
            tracing::warn!("Tag file not loaded: {}", tag_id);
            continue;
        };

        if tag_files.get(&handle).map(|f| f.replace).unwrap_or(false) {
            block_tags.tags.insert(tag_id, entries);
        } else {
            block_tags.tags.entry(tag_id).or_default().extend(entries);
        }
    }
}

fn resolve_static_tag_file<T: TagRegistryType>(
    tag_file: &TagFile,
    tag_files: &Assets<TagFile>,
    registry: &StaticRegistry<T>,
) -> Vec<StaticId<T>> {
    let mut result = Vec::new();
    for entry in &tag_file.entries {
        match entry {
            TagFileEntry::Element(loc) => {
                if let Some((id, _)) = registry.get_by_loc(loc) {
                    result.push(id);
                } else {
                    tracing::warn!("Tag references unknown block: {}", loc);
                }
            }
            TagFileEntry::OptionalElement(loc) => {
                if let Some((id, _)) = registry.get_by_loc(loc) {
                    result.push(id);
                }
                // Not found = silently skip (optional)
            }
            TagFileEntry::Tag(nested_handle) => {
                // Recursively expand — nested file is guaranteed loaded
                if let Some(nested) = tag_files.get(nested_handle) {
                    result.extend(resolve_static_tag_file(nested, tag_files, registry));
                }
            }
        }
    }
    result
}
```

### Resolving Dynamic Tags (Biome example)

```rust
fn resolve_biome_tags(
    mut biome_tags: ResMut<Tags<Biome>>,
    tag_files: Res<Assets<TagFile>>,
    asset_server: Res<AssetServer>,
    structures: Res<Assets<Structure>>,
    // Also need to collect tag handles that came from JSON loaders:
    worldgen_handles: Res<WorldgenHandles>,
) {
    // Collect all TagFile handles that were loaded as Structure.biomes deps
    let mut biome_tag_handles: HashMap<ResourceLocation, Handle<TagFile>> = HashMap::new();

    for handle in &worldgen_handles.structures {
        if let Some(structure) = structures.get(handle) {
            let tag_id = tag_file_to_resource_location(&structure.biomes, &asset_server);
            biome_tag_handles.insert(tag_id, structure.biomes.clone());
        }
    }

    // Also tags requested via TagKey<Biome> constants
    let keys_handles: Vec<_> = biome_tags.handles.drain().collect();
    biome_tag_handles.extend(keys_handles);

    for (tag_id, tag_handle) in biome_tag_handles {
        if let Some(tag_file) = tag_files.get(&tag_handle) {
            let entries = resolve_dynamic_tag_file(
                tag_file, &tag_files, &asset_server
            );
            biome_tags.tags.insert(tag_id, entries);
        }
    }
}

fn resolve_dynamic_tag_file<T: Asset>(
    tag_file: &TagFile,
    tag_files: &Assets<TagFile>,
    asset_server: &AssetServer,
) -> Vec<Handle<T>> {
    let mut result = Vec::new();
    for entry in &tag_file.entries {
        match entry {
            TagFileEntry::Element(loc) | TagFileEntry::OptionalElement(loc) => {
                // Map ResourceLocation → asset path → Handle<T>
                let path = loc.to_asset_path_for::<T>();
                if let Some(handle) = asset_server.get_handle::<T>(&path) {
                    result.push(handle);
                } else if matches!(entry, TagFileEntry::Element(_)) {
                    tracing::warn!("Tag references unloaded asset: {}", loc);
                }
            }
            TagFileEntry::Tag(nested_handle) => {
                if let Some(nested) = tag_files.get(nested_handle) {
                    result.extend(resolve_dynamic_tag_file::<T>(nested, tag_files, asset_server));
                }
            }
        }
    }
    result
}
```

---

## Network Sync: `UpdateTags` Packet

Only tags that were loaded (via any of the three paths) are sent. For vanilla,
all vanilla tags are declared as `TagKey` constants in `mc_vanilla` and loaded
at startup — so the packet is complete.

```rust
fn build_update_tags_packet(
    block_tags: Res<StaticTags<Block>>,
    item_tags: Res<StaticTags<Item>>,
    biome_tags: Res<Tags<Biome>>,
    structure_tags: Res<Tags<Structure>>,
    snapshot: Res<RegistrySnapshot>,
) -> ClientboundUpdateTagsPacket {
    ClientboundUpdateTagsPacket {
        groups: vec![
            block_tags.to_packet_groups(rl!("minecraft:block")),
            item_tags.to_packet_groups(rl!("minecraft:item")),
            biome_tags.to_packet_groups(rl!("minecraft:worldgen/biome"), &snapshot),
            structure_tags.to_packet_groups(rl!("minecraft:worldgen/structure"), &snapshot),
        ],
    }
}

impl<T: TagRegistryType> StaticTags<T> {
    /// For static types, index = protocol ID directly.
    pub fn to_packet_groups(&self, registry_name: ResourceLocation) -> RegistryTagsPacket {
        RegistryTagsPacket {
            registry: registry_name,
            tags: self.tags.iter().map(|(id, entries)| TagGroup {
                name: id.clone(),
                entries: entries.iter().map(|e| e.index).collect(),
            }).collect(),
        }
    }
}

impl<T: Asset + TagRegistryType> Tags<T> {
    /// For dynamic types, use RegistrySnapshot to map Handle → network ID.
    pub fn to_packet_groups(
        &self,
        registry_name: ResourceLocation,
        snapshot: &RegistrySnapshot,
    ) -> RegistryTagsPacket {
        RegistryTagsPacket {
            registry: registry_name,
            tags: self.tags.iter().map(|(id, handles)| TagGroup {
                name: id.clone(),
                entries: handles.iter()
                    .filter_map(|h| snapshot.network_id_for(h.id()))
                    .collect(),
            }).collect(),
        }
    }
}
```

---

## Plugin Registration

```rust
pub struct TagPlugin;

impl Plugin for TagPlugin {
    fn build(&self, app: &mut App) {
        // Shared asset type and loader
        app.init_asset::<TagFile>()
           .init_asset_loader::<TagFileLoader>();

        // One resource per tag-capable type
        app.init_resource::<StaticTags<Block>>()
           .init_resource::<StaticTags<Item>>()
           .init_resource::<StaticTags<EntityType>>()
           .init_resource::<Tags<Biome>>()
           .init_resource::<Tags<Structure>>();

        // Resolution happens once at WorldgenFreeze
        app.add_systems(OnEnter(AppState::WorldgenFreeze), (
            resolve_block_tags,
            resolve_item_tags,
            resolve_biome_tags,
            resolve_structure_tags,
        ).chain());
    }
}
```

---

## Key Properties

| Property | Behavior |
|----------|----------|
| No directory scan | Tags loaded only when referenced |
| Static type path derivation | `TagKey<Block>.asset_path()` → `"minecraft/tags/block/..."` |
| Dynamic type path | Same: `TagKey<Biome>.asset_path()` → `"minecraft/tags/worldgen/biome/..."` |
| Nested tags | `TagFileLoader` calls `ctx.load()` for `#tag` refs — Bevy handles transitively |
| JSON asset deps | `ctx.load()` in any asset loader creates tag dependency automatically |
| Resolution timing | Once, in `OnEnter(WorldgenFreeze)` — never polled each frame |
| Hot-reload | `AssetEvent<TagFile>::Modified` → re-enter WorldgenFreeze → re-resolve |
| Unknown entries | Required: warn. Optional: silently skip |
| `UpdateTags` packet | Contains exactly the tags that were loaded (complete for vanilla) |

---

## Summary of Changes from PoC

| Old | New |
|-----|-----|
| `fs::read_dir` in Startup system | Eliminated entirely |
| `asset_server.load_folder()` | Eliminated entirely |
| `TagRegistry<T>` with `RegistryId<T>` | `StaticTags<T>` with `StaticId<T>` + `Tags<T: Asset>` with `Handle<T>` |
| `ResourcePackTags` | `TagFile` (same concept, renamed + simplified) |
| `process_loaded_tags` (Update, every frame) | `resolve_*_tags` systems (OnEnter WorldgenFreeze, once) |
| `build_registry_tags` (drops entries) | `to_packet_groups` (uses snapshot IDs, no data loss) |
| Path-to-tag-ident parsing | `TagKey<T>::asset_path()` + `TagRegistryType::REGISTRY_PATH` |
| Separate `BlockTagPlugin`, `ItemTagPlugin` | Single `TagPlugin` + typed resources |
