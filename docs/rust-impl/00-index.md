# Rust/Bevy Implementation Guide — Index

Research and planning documents for reimplementing Minecraft worldgen in Rust/Bevy.
Sources: Bevy source code at `~/IdeaProjects/bevy/`, Minecraft source code at `~/IdeaProjects/minecraft/`,
PoC implementation at `~/RustroverProjects/mcrs/`.

## Document Map

### Reference (from Bevy source exploration)

| Document | Contents |
|----------|----------|
| [01-bevy-asset-system.md](01-bevy-asset-system.md) | AssetLoader API, Handle types, dependency tracking, hot reload, labeled assets |
| [02-bevy-plugin-ecs.md](02-bevy-plugin-ecs.md) | Plugin trait, App, SystemSet scheduling, States, events, Messages<T> |
| [03-registry-asset-mapping.md](03-registry-asset-mapping.md) | Java→Rust type mapping for all 40+ registry types, loader design |
| ~~04-implementation-roadmap.md~~ | *(deleted — superseded by 10)* |

### Design Specs (from mcrs PoC analysis)

| Document | Contents |
|----------|----------|
| ~~05-poc-analysis.md~~ | *(deleted — all problems described are now fixed in Steps 1–3)* |
| [06-registry-redesign.md](06-registry-redesign.md) | Two-tier registry: StaticRegistry<T> + Assets<T>, RegistrySnapshot |
| [07-tag-system-redesign.md](07-tag-system-redesign.md) | Demand-driven TagFile asset, StaticTags<T>, Tags<T: Asset>, TagKey<T> path derivation |
| [08-worldgen-asset-integration.md](08-worldgen-asset-integration.md) | Bevy asset loaders for all worldgen types, two-phase DF loading |
| [09-crate-restructure.md](09-crate-restructure.md) | New crate layout, migration order, plugin hierarchy |
| [10-implementation-order.md](10-implementation-order.md) | **Definitive step-by-step implementation sequence** (13 steps, with verification tests and gap list) |

### Gap-Fill Specs (from Java source analysis)

| Document | Contents |
|----------|----------|
| [11-configuration-protocol.md](11-configuration-protocol.md) | Login→Configuration→Play sequence, 23 synced registries, wire formats, known-packs optimization, RegistrySnapshot build |
| [12-chunk-pipeline.md](12-chunk-pipeline.md) | ChunkStatus lifecycle, RandomState/NoiseRouter, NoiseChunk trilinear interpolation, aquifer system, ECS integration |
| [13-biome-source.md](13-biome-source.md) | Climate 7D parameter space, quantization, R-tree construction & nearest-neighbor search, MultiNoiseBiomeSource, presets |
| [14-blockstate-surface-rules.md](14-blockstate-surface-rules.md) | BlockState JSON & wire encoding, global ID registry, 4 surface rule types, 11 condition types, evaluation context |

> **Note:** `04-implementation-roadmap.md` and `05-poc-analysis.md` were deleted (Feb 2026):
> 04 was superseded by 10; 05 described problems that are now fixed by Steps 1–3.

## Actual Crate Layout (current, Feb 2026)

> Note: crate rename from `mcrs_*` to `mc_*` has NOT happened yet. Target names shown in 09-crate-restructure.md.

```
mcrs/crates/
├── mcrs_core/              # ✅ NEW: MinecraftEnginePlugin, ResourceLocation, StaticRegistry,
│                           #         TagFile/StaticTags, AppState, RegistrySnapshot (stub)
├── mcrs_registry/          # LEGACY: old Registry<E>/Holder/RegistryId (used by enchantments)
├── mcrs_engine/            # Voxel engine: entity, chunk, dimension, math
├── mcrs_minecraft/         # ServerPlugin: world, tags (migrated), config, loot, login, etc.
├── mcrs_minecraft_worldgen/ # ✅ Worldgen optimizer + Tier 0 asset loaders
├── mcrs_protocol/          # Protocol encoding/decoding
├── mcrs_protocol_macros/   # Encode/Decode/Packet macros
├── mcrs_network/           # Tokio async network I/O
├── mcrs_nbt/               # NBT serialization
└── mcrs_random/            # Xoshiro + MD5 RNG
```

**Implementation progress:** Step 1 of `10-implementation-order.md` complete (mcrs_core crate created). Steps 2–13 all pending.

`mcrs_core` has **zero dependents** — no crate imports it yet. All existing code still uses old PoC types:
`mcrs_minecraft` → `mcrs_registry::Registry` + `TagRegistry<T>` + `fs::read_dir`;
`mcrs_minecraft_worldgen` → 3 PoC loaders (NoiseParam/DensityFunction/NoiseGeneratorSettings) using internal `proto` types, not `mcrs_core`.

## Key Design Decisions

1. **Two-tier registries** — Static (compile-time, `StaticRegistry<T>`) vs Dynamic (runtime, `Assets<T>`)
2. **`Handle<T>` replaces `Holder<T>`** — drop `mcrs_registry::Holder` and `RegistryId`; use Bevy primitives
3. **`TagFile` is registry-agnostic** — one loader, two resolvers (`StaticTags<T>` and `Tags<T: Asset>`)
4. **Tags are demand-driven** — NO directory scanning, NO `load_folder`. Tags loaded via:
   - `TagKey<T>::load()` in game code (explicitly declared constants)
   - `ctx.load()` in JSON asset loaders when parsing `#tag` refs
   - `TagFileLoader` calling `ctx.load()` for nested `#tag` refs inside tag files
5. **`TagKey<T>` derives its own path** — `TagKey<Block>{"minecraft:mineable/pickaxe"}` → `"minecraft/tags/block/mineable/pickaxe.json"` via `TagRegistryType::REGISTRY_PATH`
6. **Tags resolved once** — on `OnEnter(AppState::WorldgenFreeze)`, not polled on every frame
7. **Two-phase DF loading** — `UnresolvedDensityFunction` (asset with handles) → compiled `DensityFunctionStack`
8. **`RegistrySnapshot`** — assigns stable u32 network IDs after WorldgenFreeze for Configuration phase
9. **Keep worldgen engine** — `density_function/mod.rs` 6,564-line optimized stack is NOT changed
10. **`LoadedWithDependencies`** — the freeze signal; fires when asset AND all transitive deps are ready

## What to Keep From mcrs PoC (Unchanged)

| Component | Location |
|-----------|----------|
| Density function optimizer | `mcrs_minecraft_worldgen/density_function/mod.rs` |
| ImprovedNoise (Perlin) | `mcrs_minecraft_worldgen/noise/improved_noise.rs` |
| OctavePerlinNoise | `mcrs_minecraft_worldgen/noise/octave_perlin_noise.rs` |
| CubicSpline | `mcrs_minecraft_worldgen/spline/mod.rs` |
| Climate parameters | `mcrs_minecraft_worldgen/climate.rs` |
| Protocol encoding | `mcrs_protocol/` |
| Network I/O | `mcrs_network/` |
| NBT | `mcrs_nbt/` |
| RNG | `mcrs_random/` |
