# Enchantment System in mcrs_vanilla

## Context

The enchantment registry is the first **dynamic** (Bevy Asset-based) registry in `mcrs_vanilla`. Currently, enchantments live in `mcrs_minecraft/src/enchantment/mod.rs` using the legacy `Registry<EnchantmentData>` with only 1 hardcoded entry (silk_touch). This plan migrates enchantments to `mcrs_vanilla` using the Bevy asset pipeline, loading all 43 enchantment JSON files from `assets/minecraft/enchantment/` and resolving 22 tag files from `assets/minecraft/tags/enchantment/`.

The goal: `mcrs_vanilla` provides everything the server needs for enchantments, so `mcrs_minecraft` can simply swap its backend.

---

## Step 1: Extend `Tags<T>` with handle tracking

**File**: `crates/mcrs_core/src/tag/dynamic_tags.rs`

Add a `handles` field and methods mirroring `StaticTags<T>`, but bounded on `T: Asset + TagRegistryType`:

```rust
// New field:
handles: HashMap<ResourceLocation, Handle<TagFile>>,

// New methods:
fn request(&mut self, key: &TagKey<T>, asset_server: &AssetServer)
fn drain_handles(&mut self) -> Vec<(ResourceLocation, Handle<TagFile>)>
fn all_handles_loaded(&self, asset_server: &AssetServer) -> bool
fn pending_handles_count(&self) -> usize
```

The `request()` method uses `TagKey<T>` to derive the asset path (via `key.asset_path()`) and loads with `TagFileSettings { registry_segment: T::REGISTRY_PATH }`. This requires adding `T: TagRegistryType` bound on the impl block for these methods (keep existing methods unbounded).

**Also update** `crates/mcrs_core/src/tag/mod.rs` and `crates/mcrs_core/src/lib.rs` to re-export `Tags` (it's already re-exported via `pub use dynamic_tags::Tags` but confirm it's available from `mcrs_core`).

---

## Step 2: Create `mcrs_vanilla/src/enchantment/` module

### 2a. `enchantment/data.rs` — EnchantmentData as a Bevy Asset

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct EnchantmentData {
    pub description: serde_json::Value,
    pub min_cost: EnchantmentCost,
    pub max_cost: EnchantmentCost,
    pub anvil_cost: u32,
    pub slots: Vec<String>,
    pub supported_items: String,
    pub weight: u32,
    pub max_level: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclusive_set: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnchantmentCost {
    pub base: u32,
    pub per_level_above_first: u32,
}

impl Asset for EnchantmentData {}
impl VisitAssetDependencies for EnchantmentData {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}
```

### 2b. `enchantment/loader.rs` — AssetLoader

```rust
pub struct EnchantmentDataLoader;

impl AssetLoader for EnchantmentDataLoader {
    type Asset = EnchantmentData;
    type Settings = ();
    type Error = EnchantmentLoaderError;

    async fn load(&self, reader, _settings, _load_context) -> Result<EnchantmentData, _> {
        let bytes = reader.read_to_end(...);
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &[]  // no extension claim — always use typed load::<EnchantmentData>()
    }
}
```

Return `&[]` from `extensions()` to avoid `.json` conflict with `TagFileLoader`. All loads will use typed `asset_server.load::<EnchantmentData>(path)`.

### 2c. `enchantment/registry.rs` — Registration order + loaded enchantments tracker

Define the 43 enchantment paths in Java bootstrap order as a const array:

```rust
pub const VANILLA_ENCHANTMENTS: &[&str] = &[
    "minecraft:protection",
    "minecraft:fire_protection",
    "minecraft:feather_falling",
    "minecraft:blast_protection",
    "minecraft:projectile_protection",
    "minecraft:respiration",
    "minecraft:aqua_affinity",
    "minecraft:thorns",
    "minecraft:depth_strider",
    "minecraft:frost_walker",
    "minecraft:binding_curse",
    "minecraft:soul_speed",
    "minecraft:swift_sneak",
    "minecraft:sharpness",
    "minecraft:smite",
    "minecraft:bane_of_arthropods",
    "minecraft:knockback",
    "minecraft:fire_aspect",
    "minecraft:looting",
    "minecraft:sweeping_edge",
    "minecraft:efficiency",
    "minecraft:silk_touch",
    "minecraft:unbreaking",
    "minecraft:fortune",
    "minecraft:power",
    "minecraft:punch",
    "minecraft:flame",
    "minecraft:infinity",
    "minecraft:luck_of_the_sea",
    "minecraft:lure",
    "minecraft:loyalty",
    "minecraft:impaling",
    "minecraft:riptide",
    "minecraft:lunge",
    "minecraft:channeling",
    "minecraft:multishot",
    "minecraft:quick_charge",
    "minecraft:piercing",
    "minecraft:density",
    "minecraft:breach",
    "minecraft:wind_burst",
    "minecraft:mending",
    "minecraft:vanishing_curse",
];
```

Define a resource to track loaded enchantments and map `ResourceLocation` ↔ `AssetId`:

```rust
#[derive(Resource)]
pub struct LoadedEnchantments {
    /// Insertion-order list: index = protocol_id
    entries: Vec<(ResourceLocation, Handle<EnchantmentData>)>,
    /// RL → index for fast lookup
    index: HashMap<ResourceLocation, u32>,
}
```

Methods:
- `protocol_id_of(&self, loc: &ResourceLocation) -> Option<u32>`
- `get_handle(&self, protocol_id: u32) -> Option<&Handle<EnchantmentData>>`
- `resolve_asset_id(&self, loc: &ResourceLocation, assets: &Assets<EnchantmentData>) -> Option<AssetId<EnchantmentData>>`
- `iter() -> impl Iterator<Item = (u32, &ResourceLocation, &Handle<EnchantmentData>)>`
- `len() -> usize`

### 2d. `enchantment/tags.rs` — TagKey constants + TagRegistryType impl

```rust
impl TagRegistryType for EnchantmentData {
    const REGISTRY_PATH: &'static str = "enchantment";
}

// 15 top-level + 7 exclusive_set
pub const TOOLTIP_ORDER: TagKey<EnchantmentData> = TagKey::of("minecraft", "tooltip_order");
pub const NON_TREASURE: TagKey<EnchantmentData> = TagKey::of("minecraft", "non_treasure");
pub const TREASURE: TagKey<EnchantmentData> = TagKey::of("minecraft", "treasure");
pub const CURSE: TagKey<EnchantmentData> = TagKey::of("minecraft", "curse");
pub const IN_ENCHANTING_TABLE: TagKey<EnchantmentData> = TagKey::of("minecraft", "in_enchanting_table");
pub const TRADEABLE: TagKey<EnchantmentData> = TagKey::of("minecraft", "tradeable");
pub const DOUBLE_TRADE_PRICE: TagKey<EnchantmentData> = TagKey::of("minecraft", "double_trade_price");
pub const ON_MOB_SPAWN_EQUIPMENT: TagKey<EnchantmentData> = TagKey::of("minecraft", "on_mob_spawn_equipment");
pub const ON_TRADED_EQUIPMENT: TagKey<EnchantmentData> = TagKey::of("minecraft", "on_traded_equipment");
pub const ON_RANDOM_LOOT: TagKey<EnchantmentData> = TagKey::of("minecraft", "on_random_loot");
pub const SMELTS_LOOT: TagKey<EnchantmentData> = TagKey::of("minecraft", "smelts_loot");
pub const PREVENTS_BEE_SPAWNS_WHEN_MINING: TagKey<EnchantmentData> = TagKey::of("minecraft", "prevents_bee_spawns_when_mining");
pub const PREVENTS_DECORATED_POT_SHATTERING: TagKey<EnchantmentData> = TagKey::of("minecraft", "prevents_decorated_pot_shattering");
pub const PREVENTS_ICE_MELTING: TagKey<EnchantmentData> = TagKey::of("minecraft", "prevents_ice_melting");
pub const PREVENTS_INFESTED_SPAWNS: TagKey<EnchantmentData> = TagKey::of("minecraft", "prevents_infested_spawns");
// exclusive_set/
pub const EXCLUSIVE_SET_ARMOR: TagKey<EnchantmentData> = TagKey::of("minecraft", "exclusive_set/armor");
pub const EXCLUSIVE_SET_BOOTS: TagKey<EnchantmentData> = TagKey::of("minecraft", "exclusive_set/boots");
pub const EXCLUSIVE_SET_BOW: TagKey<EnchantmentData> = TagKey::of("minecraft", "exclusive_set/bow");
pub const EXCLUSIVE_SET_CROSSBOW: TagKey<EnchantmentData> = TagKey::of("minecraft", "exclusive_set/crossbow");
pub const EXCLUSIVE_SET_DAMAGE: TagKey<EnchantmentData> = TagKey::of("minecraft", "exclusive_set/damage");
pub const EXCLUSIVE_SET_MINING: TagKey<EnchantmentData> = TagKey::of("minecraft", "exclusive_set/mining");
pub const EXCLUSIVE_SET_RIPTIDE: TagKey<EnchantmentData> = TagKey::of("minecraft", "exclusive_set/riptide");
```

### 2e. `enchantment/mod.rs` — Module root

Re-exports `data`, `loader`, `registry`, `tags` submodules.

---

## Step 3: Wire into `MinecraftCorePlugin` lifecycle

**File**: `crates/mcrs_vanilla/src/lib.rs`

### 3a. Plugin `build()` additions

```rust
// Register EnchantmentData as asset + loader
app.init_asset::<EnchantmentData>()
   .register_asset_loader(EnchantmentDataLoader)
   .init_resource::<Tags<EnchantmentData>>()

// Add to OnEnter(LoadingDataPack):
request_enchantment_assets,  // loads all 43 JSONs + requests all 22 tags
request_enchantment_tags,

// Update check_tags_ready to also check enchantment tags
// Add to OnEnter(WorldgenFreeze):
resolve_enchantment_tags,
```

### 3b. New systems

**`request_enchantment_assets`** — iterates `VANILLA_ENCHANTMENTS`, calls `asset_server.load::<EnchantmentData>(path)` for each, inserts `LoadedEnchantments` resource with handles in order.

**`request_enchantment_tags`** — calls `tags.request(&tag_key, &asset_server)` for all 22 tag constants.

**`check_tags_ready`** — extend existing function to also check `enchantment_tags.all_handles_loaded(&asset_server)`.

**`resolve_enchantment_tags`** — drain handles from `Tags<EnchantmentData>`, expand each `TagFile` using `LoadedEnchantments` to map `ResourceLocation` → `AssetId<EnchantmentData>`, call `tags.insert(loc, ids)`.

This requires a new `expand_dynamic_tag_file()` function (analogous to existing `expand_tag_file()` but maps via `LoadedEnchantments` + `Assets<EnchantmentData>` instead of `StaticRegistry<T>`).

---

## Step 4: Update Cargo.toml

**File**: `crates/mcrs_vanilla/Cargo.toml`

Add `serde_json` and `bevy_reflect` (for `TypePath`) to dependencies. Both should already be available in workspace.

---

## Files Modified (summary)

| File | Change |
|------|--------|
| `crates/mcrs_core/src/tag/dynamic_tags.rs` | Add handle tracking (request/drain/all_loaded) |
| `crates/mcrs_vanilla/src/enchantment/mod.rs` | **NEW** — module root |
| `crates/mcrs_vanilla/src/enchantment/data.rs` | **NEW** — EnchantmentData asset type |
| `crates/mcrs_vanilla/src/enchantment/loader.rs` | **NEW** — AssetLoader impl |
| `crates/mcrs_vanilla/src/enchantment/registry.rs` | **NEW** — VANILLA_ENCHANTMENTS order + LoadedEnchantments resource |
| `crates/mcrs_vanilla/src/enchantment/tags.rs` | **NEW** — TagRegistryType impl + 22 TagKey constants |
| `crates/mcrs_vanilla/src/lib.rs` | Wire enchantment systems into MinecraftCorePlugin lifecycle |
| `crates/mcrs_vanilla/Cargo.toml` | Add serde_json, bevy_reflect deps |

---

## Verification

1. `cargo build -p mcrs_vanilla` — compiles without errors
2. `cargo build` — full workspace compiles (mcrs_minecraft still has its old enchantment module; no conflicts since we don't touch it)
3. Run the server — check logs for:
   - `"registered X enchantment assets"` during LoadingDataPack
   - `"resolved StaticTags<EnchantmentData>"` or similar during WorldgenFreeze
   - All 43 enchantments loaded, 22 tag sets resolved
