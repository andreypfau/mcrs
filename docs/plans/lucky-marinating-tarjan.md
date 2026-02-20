# Plan: Create `mcrs_vanilla` Crate — Static Registry Complete Rewrite

## Context

`mcrs_core` (Step 1 of the restructure) is done: it provides `StaticRegistry<T>`,
`StaticId<T>`, `StaticTags<T>`, `TagKey<T>`, `TagFile`, `TagFileLoader`, `AppState`,
and `ResourceLocation`.

The previous Step 2 implementation was a bridge: `VanillaPlugin` copied entries from
the old `Registry<&'static Block>` into `StaticRegistry<Block>`. That approach still
depended on the legacy registration path and added an extra layer of indirection.

This plan is a **complete rewrite** following the target architecture in
`docs/rust-impl/09-crate-restructure.md`. The result:

- A new `mcrs_vanilla` crate that owns all Block/Item type vocabulary and their
  static constants (pure data, no game logic).
- `MinecraftCorePlugin::finish()` populates `StaticRegistry<Block>` and
  `StaticRegistry<Item>` directly from the `&'static Block` constants — no
  intermediary `Registry<T>`.
- The old `Registry<&'static Block>` and `VanillaPlugin` are deleted.
- `mcrs_minecraft` (future `mcrs_server`) gains a clean dependency on `mcrs_vanilla`
  instead of owning the type definitions itself.

**Scope**: Block and Item static registries only. Dynamic registries (worldgen
biomes, noise, etc.), enchantments, and entity types are not touched.

---

## Target Architecture

```
ServerPlugin (mcrs_minecraft)
  └── MinecraftEnginePlugin (mcrs_core)  — AppState, TagFile, StaticRegistry init
  └── MinecraftCorePlugin (mcrs_vanilla) — registers Block/Item data, drives state machine
        build()  — adds tag-loading and state-transition systems
        finish() — populates StaticRegistry<Block/Item> from &'static Block consts
  └── BlockTagPlugin / ItemTagPlugin (mcrs_minecraft) — protocol UpdateTags packet
  └── TntBlockPlugin (mcrs_minecraft) — game-logic only
  └── ... other gameplay plugins
```

---

## New Crate: `crates/mcrs_vanilla/`

### `Cargo.toml`
```toml
[package]
name = "mcrs_vanilla"
version.workspace = true
edition.workspace = true

[dependencies]
mcrs_core.workspace = true
mcrs_protocol.workspace = true
bevy_app.workspace = true
bevy_ecs.workspace = true
bevy_asset.workspace = true
bevy_state.workspace = true
paste = "1"
bitflags = "2"
tracing.workspace = true
```

### File Structure
```
crates/mcrs_vanilla/src/
├── lib.rs                  # MinecraftCorePlugin (build + finish)
├── block/
│   ├── mod.rs              # Block, BlockState, BlockUpdateFlags, TagRegistryType impl
│   ├── behaviour.rs        # Properties struct (pure data — no BlockBehaviour trait)
│   ├── macros.rs           # generate_block_states!, block_state_idents!
│   ├── tags.rs             # TagKey<Block> constants
│   └── minecraft/
│       ├── mod.rs          # declare_blocks! macro + register_all_blocks() fn
│       ├── air.rs
│       ├── stone.rs
│       └── ... (38 block files — pure data, no game logic)
├── item/
│   ├── mod.rs              # Item, ItemStack, TagRegistryType impl
│   ├── component/
│   │   ├── mod.rs          # ItemComponents
│   │   └── tool.rs         # Tool, ToolRule, ToolTagRef (uses TagKey<Block>)
│   ├── tags.rs             # TagKey<Item> constants
│   └── minecraft/
│       └── mod.rs          # declare_items! + register_all_items()
├── sound/
│   └── mod.rs              # SoundType, SoundEvent (no Music/Holder)
└── material/
    └── mod.rs              # MapColor, PushReaction, NoteBlockInstrument
```

---

## Step-by-Step Implementation

### Step 1 — Add `mcrs_vanilla` to workspace

**`Cargo.toml` (root)**:
```toml
mcrs_vanilla = { path = "crates/mcrs_vanilla" }
```

Also add to `[workspace] members = ["crates/*"]` (already a glob, no change needed).

### Step 2 — Create `mcrs_vanilla` with pure-data type modules

Move from `mcrs_minecraft` into `mcrs_vanilla` (file contents largely copied as-is):

| Source (`mcrs_minecraft/src/`) | Destination (`mcrs_vanilla/src/`) | Notes |
|-------------------------------|-----------------------------------|----|
| `world/block/mod.rs` | `block/mod.rs` | Drop `use mcrs_engine`; keep Block, BlockState, BlockUpdateFlags |
| `world/block/behaviour.rs` | `block/behaviour.rs` | Drop `BlockBehaviour` trait (uses BlockPos from mcrs_engine); keep only `Properties` struct and its `impl` methods |
| `world/block/macros.rs` | `block/macros.rs` | Copy verbatim |
| `world/block/minecraft.rs` | `block/minecraft/mod.rs` | Rewrite `declare_blocks!` to generate `register_all_blocks()` instead of `MinecraftBlockPlugin` (see Step 4) |
| `world/block/minecraft/*.rs` | `block/minecraft/*.rs` | Copy verbatim **except** remove game-logic content (e.g., `TntBlockPlugin` stays in `mcrs_minecraft`) |
| `world/item/mod.rs` | `item/mod.rs` | Copy (Item, ItemStack, ItemCommands) |
| `world/item/component.rs` + submodules | `item/component/` | Copy pure data; `tool.rs` updated (see Step 5) |
| `world/item/minecraft.rs` | `item/minecraft/mod.rs` | Copy, update imports |
| `sound/mod.rs` | `sound/mod.rs` | Copy `SoundType`, `SoundEvent` only (drop `Music` which uses `mcrs_registry::Holder`) |
| `world/material/mod.rs` | `material/mod.rs` | Copy `MapColor`, `PushReaction` verbatim |
| `world/block/minecraft/note_block.rs` | `block/minecraft/note_block.rs` | Copy; `NoteBlockInstrument` enum lives here |

`tnt.rs` split:
- `mcrs_vanilla/src/block/minecraft/tnt.rs` — only `BLOCK`, `PROPERTIES`, `UNSTABLE_STATE`, `DEFAULT_STATE` consts
- `mcrs_minecraft` — `TntBlockPlugin` struct and its systems stay (see Step 8)

### Step 3 — Add `TagRegistryType` impls in `mcrs_vanilla`

`mcrs_vanilla/src/block/mod.rs`:
```rust
use mcrs_core::tag::key::TagRegistryType;
impl TagRegistryType for Block {
    const REGISTRY_PATH: &'static str = "block";
}
```

`mcrs_vanilla/src/item/mod.rs`:
```rust
use mcrs_core::tag::key::TagRegistryType;
impl TagRegistryType for Item {
    const REGISTRY_PATH: &'static str = "item";
}
```

### Step 4 — Rewrite `declare_blocks!` macro

`mcrs_vanilla/src/block/minecraft/mod.rs` — replace `MinecraftBlockPlugin` generation
with a `register_all_blocks()` function:

```rust
use mcrs_core::{ResourceLocation, StaticRegistry};
use std::str::FromStr;

macro_rules! declare_blocks {
    (
        $(
            $module:ident => $const_name:ident
        ),* $(,)?
    ) => {
        $(pub mod $module;)*
        $(pub use $module::BLOCK as $const_name;)*

        pub fn register_all_blocks(registry: &mut StaticRegistry<Block>) {
            $(
                {
                    let loc = ResourceLocation::from_str($const_name.identifier.as_str())
                        .expect("block identifier must be a valid ResourceLocation");
                    registry.register(loc, &$const_name);
                }
            )*
        }

        // STATE_TO_BLOCK LUT — essential for block lookup by BlockStateId
        const STATE_TABLE_LEN: usize = 1 << 16;
        static STATE_TO_BLOCK: [Option<&'static Block>; STATE_TABLE_LEN] = { /* ... unchanged ... */ };
    };
}

// TryFrom impls for BlockStateId, Ident<String>, AsRef<Block> stay here
```

Note: The `[$plugin]` syntax is removed from `declare_blocks!`. Block-specific plugins
(e.g., `TntBlockPlugin`) are added explicitly in `mcrs_minecraft::ServerPlugin`.

Similarly, `declare_items!` in `item/minecraft/mod.rs` generates `register_all_items()`.

### Step 5 — Update `ToolTagRef` to use `TagKey<Block>`

`mcrs_vanilla/src/item/component/tool.rs`:

Replace `DynamicIdent(&'static str)` with `TagKey<Block>`:
```rust
// Before (mcrs_minecraft):
pub enum ToolTagRef {
    Static(BlockTagSet),
    DynamicIdent(&'static str),
}

// After (mcrs_vanilla):
pub enum ToolTagRef {
    TagKey(TagKey<Block>),
}
```

The `for_mineable_blocks_dynamic()` method on `ToolMaterial` now takes a `TagKey<Block>`
instead of `&'static str`. Items in `item/minecraft/mod.rs` use `block_tags::MINEABLE_PICKAXE`
etc. from `mcrs_vanilla::block::tags`.

### Step 6 — Add `MinecraftCorePlugin` to `mcrs_vanilla/src/lib.rs`

```rust
use crate::block;
use crate::item;
use crate::block::tags as block_tags;
use crate::item::tags as item_tags;
use mcrs_core::{AppState, StaticRegistry, StaticTags};
use mcrs_core::tag::file::TagFile;
use bevy_app::{App, Plugin, PostStartup, Update};
use bevy_asset::{Assets, AssetServer};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use std::collections::HashSet;
use mcrs_core::{StaticId, TagEntry};
use mcrs_core::tag::key::TagRegistryType;

pub struct MinecraftCorePlugin;

impl Plugin for MinecraftCorePlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<StaticRegistry<block::Block>>()
            .init_resource::<StaticRegistry<item::Item>>()
            .init_resource::<StaticTags<block::Block>>()
            .init_resource::<StaticTags<item::Item>>()
            .add_systems(PostStartup, start_loading_data_pack)
            .add_systems(
                OnEnter(AppState::LoadingDataPack),
                (request_block_tags, request_item_tags),
            )
            .add_systems(
                Update,
                check_tags_ready.run_if(in_state(AppState::LoadingDataPack)),
            )
            .add_systems(
                OnEnter(AppState::WorldgenFreeze),
                (resolve_block_tags, resolve_item_tags, transition_to_playing).chain(),
            );
    }

    fn finish(&self, app: &mut App) {
        {
            let mut blocks = app.world_mut().resource_mut::<StaticRegistry<block::Block>>();
            block::minecraft::register_all_blocks(&mut blocks);
            tracing::info!(count = blocks.len(), "registered StaticRegistry<Block>");
        }
        {
            let mut items = app.world_mut().resource_mut::<StaticRegistry<item::Item>>();
            item::minecraft::register_all_items(&mut items);
            tracing::info!(count = items.len(), "registered StaticRegistry<Item>");
        }
    }
}

// State-machine systems (same as current VanillaPlugin)
fn start_loading_data_pack(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::LoadingDataPack);
}

fn request_block_tags(mut tags: ResMut<StaticTags<block::Block>>, asset_server: Res<AssetServer>) {
    tags.request(&block_tags::MINEABLE_PICKAXE, &asset_server);
    tags.request(&block_tags::MINEABLE_AXE, &asset_server);
    tags.request(&block_tags::MINEABLE_SHOVEL, &asset_server);
    tags.request(&block_tags::MINEABLE_HOE, &asset_server);
    tags.request(&block_tags::NEEDS_CORRECT_TOOL, &asset_server);
    tags.request(&block_tags::LOGS, &asset_server);
    tags.request(&block_tags::LEAVES, &asset_server);
    tags.request(&block_tags::SAND, &asset_server);
    tags.request(&block_tags::WOOL, &asset_server);
    tags.request(&block_tags::SNOW, &asset_server);
}

fn request_item_tags(mut tags: ResMut<StaticTags<item::Item>>, asset_server: Res<AssetServer>) {
    tags.request(&item_tags::SWORDS, &asset_server);
    tags.request(&item_tags::PICKAXES, &asset_server);
    // ...
}

fn check_tags_ready(
    block_tags: Res<StaticTags<block::Block>>,
    item_tags: Res<StaticTags<item::Item>>,
    asset_server: Res<AssetServer>,
    mut next: ResMut<NextState<AppState>>,
) {
    if block_tags.all_handles_loaded(&asset_server) && item_tags.all_handles_loaded(&asset_server) {
        next.set(AppState::WorldgenFreeze);
    }
}

// resolve_block_tags, resolve_item_tags, transition_to_playing — same as VanillaPlugin
```

### Step 7 — Update `mcrs_minecraft` to use `mcrs_vanilla`

**`crates/mcrs_minecraft/Cargo.toml`**:
```toml
mcrs_vanilla.workspace = true
```

**`crates/mcrs_minecraft/src/lib.rs`**:
```rust
// Remove VanillaPlugin
// Add MinecraftCorePlugin
use mcrs_vanilla::MinecraftCorePlugin;
use crate::world::block::minecraft::tnt::TntBlockPlugin; // re-located game logic

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AssetPlugin::default());
        app.add_plugins(MinecraftEnginePlugin);   // AppState, TagFile, TagFileLoader
        app.add_plugins(MinecraftCorePlugin);     // StaticRegistry + StaticTags
        app.add_plugins(TntBlockPlugin);          // explicit (no longer in declare_blocks!)
        app.add_plugins(BlockTagPlugin);          // protocol UpdateTags
        app.add_plugins(ItemTagPlugin);
        // ... rest unchanged
    }
}
```

**`crates/mcrs_minecraft/src/world/block/mod.rs`**:
Re-export the types from `mcrs_vanilla` to maintain backward-compatible paths:
```rust
pub use mcrs_vanilla::block::{Block, BlockState, BlockUpdateFlags};
pub use mcrs_vanilla::block::behaviour::Properties;
pub mod behaviour { pub use mcrs_vanilla::block::behaviour::*; }
pub mod minecraft; // now only contains TntBlockPlugin
```

**`crates/mcrs_minecraft/src/world/block/minecraft.rs`** (now just game-logic plugins):
```rust
// Only TntBlockPlugin remains here
use mcrs_vanilla::block::minecraft::tnt::{UNSTABLE_STATE, DEFAULT_STATE};
// ... TntBlockPlugin impl
```

**`crates/mcrs_minecraft/src/world/item/mod.rs`**:
```rust
pub use mcrs_vanilla::item::{Item, ItemStack, ItemCommands};
pub use mcrs_vanilla::item::component::ItemComponents;
```

**Delete `crates/mcrs_minecraft/src/vanilla_plugin.rs`** — replaced entirely by
`MinecraftCorePlugin` in `mcrs_vanilla`.

### Step 8 — Update `BlockTagPlugin` to use `StaticRegistry<Block>`

`crates/mcrs_minecraft/src/tag/block.rs`:

- Replace `Res<Registry<&'static Block>>` with `Res<StaticRegistry<Block>>` in
  `process_loaded_tags` and `resolve_tag_entries`.
- `TagRegistry<Block>` internal store changes from `Vec<RegistryId<T>>` to
  `Vec<StaticId<Block>>` to avoid `mcrs_registry::RegistryId`.
- `build_block_registry_tags` gets protocol_id from `registry.get(id).protocol_id`
  instead of going through the old `RegistryId::Index → Registry<T>` lookup.
- Remove `MinecraftBlockPlugin` dependency (no longer exists).

This removes the last usage of `Registry<&'static Block>` in `mcrs_minecraft`.

---

## Ordering Constraints

```
MinecraftEnginePlugin::build()   → init_state::<AppState>(), init_asset::<TagFile>()
MinecraftCorePlugin::build()     → init_resource::<StaticRegistry<Block>>()
                                   init_resource::<StaticTags<Block>>()
                                   adds state-machine systems
MinecraftCorePlugin::finish()    → populates StaticRegistry<Block/Item>
                                   (all plugins built, resources initialized)

PostStartup:
  start_loading_data_pack()      → Bootstrap → LoadingDataPack

OnEnter(LoadingDataPack):
  request_block_tags()           → loads 10 game-logic block tag JSON files
  request_item_tags()            → loads 5 game-logic item tag JSON files

Update [while LoadingDataPack]:
  check_tags_ready()             → → WorldgenFreeze when all handles loaded

OnEnter(WorldgenFreeze):
  resolve_block_tags()           → StaticTags<Block> populated
  resolve_item_tags()            → StaticTags<Item> populated
  transition_to_playing()        → → Playing

BlockTagPlugin (parallel, unchanged timing):
  Startup: load_block_tags()     → loads all 200+ block tags for protocol
```

`Plugin::finish()` is called after all `build()` calls, so `StaticRegistry<Block>`
is guaranteed to be initialized before `register_all_blocks()` is called.

---

## Files Changed Summary

### New crate
- `crates/mcrs_vanilla/` — 15–20 new files (types moved + new plugin)

### Modified
| File | Change |
|------|--------|
| `Cargo.toml` (root) | Add `mcrs_vanilla = { path = "crates/mcrs_vanilla" }` to workspace deps |
| `crates/mcrs_minecraft/Cargo.toml` | Add `mcrs_vanilla.workspace = true`; remove `mcrs_registry` usage after Step 8 |
| `crates/mcrs_minecraft/src/lib.rs` | Replace `VanillaPlugin` with `MinecraftCorePlugin`; add `TntBlockPlugin` explicitly |
| `crates/mcrs_minecraft/src/world/block/mod.rs` | Re-export from `mcrs_vanilla::block` |
| `crates/mcrs_minecraft/src/world/item/mod.rs` | Re-export from `mcrs_vanilla::item` |
| `crates/mcrs_minecraft/src/tag/block.rs` | Replace `Registry<T>` with `StaticRegistry<T>` |
| `crates/mcrs_minecraft/src/tag/item.rs` | Same as above for items |

### Deleted
| File | Reason |
|------|--------|
| `crates/mcrs_minecraft/src/vanilla_plugin.rs` | Replaced by `MinecraftCorePlugin` |
| `crates/mcrs_minecraft/src/world/block/minecraft/*.rs` | Moved to `mcrs_vanilla` (game-logic parts remain in mcrs_minecraft) |
| `crates/mcrs_minecraft/src/tag/block_tags.rs` | Moved to `mcrs_vanilla::block::tags` |
| `crates/mcrs_minecraft/src/tag/item_tags.rs` | Moved to `mcrs_vanilla::item::tags` |

---

## What Is NOT Changed

- `TagRegistry<T>` structure (only its backing type changes `Registry<T>` → `StaticRegistry<T>`)
- `BlockTagPlugin` / `ItemTagPlugin` existence and protocol role
- All gameplay systems (`digging.rs`, combat, etc.)
- `BlockBehaviour` trait (stays in `mcrs_minecraft`, uses `BlockPos` from `mcrs_engine`)
- Worldgen, enchantments, entity types — untouched
- All network/protocol crates

---

## Verification

### Automated integration test — headless app (no player, no network)

Add a test in `crates/mcrs_vanilla/tests/registry_lifecycle.rs` that spins up a
minimal Bevy `App` with only the infrastructure plugins, runs through the full
`AppState` lifecycle, and asserts the registries are correctly populated.

```rust
// crates/mcrs_vanilla/tests/registry_lifecycle.rs

use bevy_app::{App, AppExit, FixedMain, Last, Update};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use mcrs_core::{AppState, MinecraftEnginePlugin, StaticRegistry, StaticTags};
use mcrs_vanilla::{block, item, block::tags as block_tags, MinecraftCorePlugin};

/// Plugin that halts the app once it reaches the Playing state,
/// then verifies registry and tag contents before exit.
struct AssertPlugin;

impl bevy_app::Plugin for AssertPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::Playing),
            assert_registries_populated,
        );
    }
}

fn assert_registries_populated(
    blocks: Res<StaticRegistry<block::Block>>,
    items: Res<StaticRegistry<item::Item>>,
    block_tags_res: Res<StaticTags<block::Block>>,
    mut exit: EventWriter<AppExit>,
) {
    // StaticRegistry<Block> must contain all registered blocks
    assert!(blocks.len() >= 38, "expected at least 38 blocks, got {}", blocks.len());

    // Every known block constant must be resolvable by ResourceLocation
    use std::str::FromStr;
    use mcrs_core::ResourceLocation;
    let stone_loc = ResourceLocation::from_str("minecraft:stone").unwrap();
    assert!(blocks.id_of(&stone_loc).is_some(), "stone not found in StaticRegistry<Block>");
    let air_loc = ResourceLocation::from_str("minecraft:air").unwrap();
    assert!(blocks.id_of(&air_loc).is_some(), "air not found in StaticRegistry<Block>");

    // StaticRegistry<Item> must contain all registered items
    assert!(items.len() >= 5, "expected at least 5 items, got {}", items.len());

    // StaticTags<Block>: MINEABLE_PICKAXE tag must be resolved with entries
    let pickaxe_tag = block_tags_res.get(&block_tags::MINEABLE_PICKAXE);
    assert!(pickaxe_tag.is_some(), "MINEABLE_PICKAXE tag not resolved");
    assert!(!pickaxe_tag.unwrap().is_empty(), "MINEABLE_PICKAXE tag has no entries");

    tracing::info!("All registry assertions passed");
    exit.write(AppExit::Success);
}

#[test]
fn test_registry_lifecycle() {
    let mut app = App::new();
    app.add_plugins((
        bevy_asset::AssetPlugin::default(),
        MinecraftEnginePlugin,
        MinecraftCorePlugin,
        AssertPlugin,
    ));

    // Run until AppExit is emitted (AssertPlugin sends it from Playing state)
    // Use a frame limit as a safety net
    for _ in 0..1000 {
        app.update();
        if app.world().contains_resource::<Events<AppExit>>() {
            let events = app.world().resource::<Events<AppExit>>();
            let mut reader = events.get_cursor();
            if reader.read(events).next().is_some() {
                return; // success
            }
        }
    }
    panic!("App never reached Playing state within 1000 frames");
}
```

This test:
- Requires no network, no player, no world generation
- Runs the full `Bootstrap → LoadingDataPack → WorldgenFreeze → Playing` state machine
- Asserts `StaticRegistry<Block>` has all blocks (by count + specific lookup)
- Asserts `StaticTags<Block>` resolved `MINEABLE_PICKAXE` with entries
- Terminates via `AppExit` rather than relying on timeouts

### Manual verification steps

1. **Build** — `cargo build` succeeds with no errors.
2. **Unit test** — `cargo test -p mcrs_vanilla` passes the headless lifecycle test.
3. **Log check** — Server startup should log:
   ```
   registered StaticRegistry<Block> count=38
   registered StaticRegistry<Item> count=5
   all static tag files loaded — entering WorldgenFreeze
   resolved StaticTags<Block> resolved_entries=...
   entering Playing state
   ```
4. **Client connect** — Vanilla client connects and joins without issues
   (old protocol path via `BlockTagPlugin`/`ItemTagPlugin` unchanged).
5. **`cargo test`** — All existing workspace tests pass.

---

## Future Follow-on (not in this plan)

- Delete `mcrs_registry` once enchantment system is migrated (Step 5 of restructure).
- Replace `TagRegistry<T>` usage in `configuration.rs` with `StaticTags<T>`.
- Add more block/item data as game features require them.
- Rename `mcrs_minecraft` → `mcrs_server` (Step 6 of restructure).
