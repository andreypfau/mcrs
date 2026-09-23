# Cyclic asset dependencies never settle in `bevy_asset`

Observed on `bevy_asset` 0.19.1. Paths are relative to that crate's `src/`.

## The problem

An asset's `RecursiveDependencyLoadState` becomes `Loaded` only when every
dependency's recursive state is `Loaded`. When `A` depends on `B` and `B`
depends on `A`, neither condition can be met first, and both stay `Loading`
forever. A self-dependency (`A` depends on `A`) is the one-vertex case and
hangs the same way.

The mechanism, in `server/info.rs`, `AssetInfos::process_asset_load`:

1. When `A` finishes loading, each dependency `dep` is examined. If
   `dep.rec_dep_load_state` is `Loading` or `NotLoaded`, `A` is added to
   `dep.dependents_waiting_on_recursive_dep_load` and `dep` stays in
   `A.loading_rec_dependencies`.
2. `A.rec_dep_load_state` is set to `Loading` while `loading_rec_dependencies`
   is non-empty. Waiters are only released when the state becomes `Loaded` or
   `Failed`.
3. When `B` finishes loading, step 1 runs for `B` and finds `A` in state
   `Loading` (from step 2), so `B` waits on `A`. Both now wait on each other.
   For a self-dependency, `A` finds itself in state `Loading` and waits on
   itself.

Nothing detects the cycle. `AssetServer::recursive_dependency_load_state`
answers `Loading` indefinitely, `LoadedWithDependencies` is never sent, and any
gate built on it (the world crate's `all_handles_settled`) never opens.

## Why a loader cannot avoid it

Every API that yields a handle inside an `AssetLoader` records that handle as a
dependency of the asset being loaded:

- `LoadContext::load` and the `NestedLoader` builder
  (`loader_builders.rs`, `self.load_context.dependencies.insert(index)`).
- `LoadContext::get_label_handle` (`loader.rs`).
- `#[dependency]` fields and hand-written `VisitAssetDependencies`
  (`LoadedAsset::new_with_dependencies`, `loader.rs`).

There is no way to hold a `Handle<T>` to another asset from within a loader
without it entering the dependency set, so a registry whose entries reference
each other cannot be modelled as handles.

## Where it bites here

Minecraft's `worldgen/template_pool` registry is cyclic by design:
`template_pool/empty.json` is `{"elements": [], "fallback": "minecraft:empty"}`,
and every other pool names a fallback pool, which chains to `empty`.
`structure_set` can also be cyclic through `exclusion_zone.other_set`, and
`structure` → `template_pool` → `feature_pool_element` → `placed_feature` can
in principle loop back.

## What this repository does

Cross-references between assets of a cyclic registry are kept as ids and
joined at freeze, never as `#[dependency]` handles:

- `template_pool`, `structure_set` and `structure` files are requested flat
  (every file in the folder, recursively) rather than pulled in by reference.
- Only acyclic edges are asset dependencies: a pool's templates
  (`structure/*.nbt`) and processor lists.
- Every id is resolved once at freeze, and an id that names nothing is a load
  error naming the asset.

The cost is a recursive directory listing where the registry is nested, and
that a reference to a missing file surfaces at freeze rather than at load.

## How to fix it upstream

Either of these would remove the limitation.

1. **Cycle-aware recursive state.** In `process_asset_load`, when a
   dependency `dep` is in state `Loading`, check whether the loading asset is
   already (transitively) in `dep`'s own `loading_rec_dependencies`. If it is,
   the two are on a cycle; treat `dep` as satisfied for the purpose of the
   loading asset's recursive state, and when the last member of the cycle
   finishes loading, release every waiter of the cycle together. The reverse
   set needed for the walk already exists
   (`dependents_waiting_on_recursive_dep_load`).
2. **An untracked handle.** Add `LoadContext::load_untracked` (or a
   `NestedLoader::untracked()` builder flag) that returns a strong handle via
   `AssetServer::get_or_create_path_handle` without inserting it into
   `dependencies`. The caller then opts out of the recursive guarantee for
   that edge and waits on the referenced asset itself, which is what a
   registry with a freeze step does anyway.

Option 2 is the smaller change and matches how a registry wants to behave;
option 1 makes the default correct for every user.
