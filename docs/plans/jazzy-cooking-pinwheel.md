# Generic ResourceLocation\<S\>

## Context

`ResourceLocation` is backed by `Arc<str>` — every lookup in `StaticTags.contains()` and `StaticRegistry.id_of()` from a `TagKey` allocates an `Arc` via `TagKey::resource_location()`. Block/Item types store `Ident<&'static str>` (from valence\_ident) which requires a second allocation to convert to `ResourceLocation` for registry lookup. Making `ResourceLocation` generic over its string type eliminates these allocations in hot paths.

---

## Struct Design

```rust
// crates/mcrs_core/src/resource_location.rs

#[derive(Clone)]
pub struct ResourceLocation<S = Arc<str>> {
    string: S,        // always "namespace:path", normalized
    colon_pos: u16,   // byte offset of ':'
}

// Copy for &'static str variant
impl Copy for ResourceLocation<&'static str> {}
```

- Default `S = Arc<str>` keeps bare `ResourceLocation` backward-compatible
- `colon_pos: u16` gives O(1) `namespace()` / `path()` via slicing
- `ResourceLocation<&'static str>` is `Copy`, zero-alloc, const-constructible

---

## Cross-Variant HashMap Lookup

HashMaps store `ResourceLocation<Arc<str>>` keys. Lookups must accept `ResourceLocation<&'static str>` without allocating.

Strategy: `impl Borrow<str>` so `HashMap::get` accepts `&str`:

```rust
impl<S: AsRef<str>> Borrow<str> for ResourceLocation<S> {
    fn borrow(&self) -> &str { self.string.as_ref() }
}

impl<S: AsRef<str>> Hash for ResourceLocation<S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.string.as_ref().hash(state);  // consistent with <str as Hash>
    }
}

impl<S: AsRef<str>, T: AsRef<str>> PartialEq<ResourceLocation<T>> for ResourceLocation<S> {
    fn eq(&self, other: &ResourceLocation<T>) -> bool {
        self.string.as_ref() == other.string.as_ref()
    }
}
```

Lookup pattern: `self.index.get(loc.as_str())` — zero-alloc for any variant.

---

## Proc Macro `rl!()`

New crate `crates/mcrs_core_macros/` (proc-macro = true).

```rust
rl!("stone")              // → ResourceLocation<&'static str> { string: "minecraft:stone", colon_pos: 9 }
rl!("minecraft:stone")    // → ResourceLocation<&'static str> { string: "minecraft:stone", colon_pos: 9 }
rl!("mymod:custom_block") // → ResourceLocation<&'static str> { string: "mymod:custom_block", colon_pos: 5 }
rl!("INVALID")            // → compile error
```

The proc macro:
1. Validates charset: namespace `[a-z0-9_.-]`, path `[a-z0-9_.-/]`
2. Auto-prefixes `"minecraft:"` when no namespace given
3. Computes `colon_pos` as a literal
4. Emits a struct construction expression (works in const context)

Implementation: `rl!()` declarative macro in mcrs\_core delegates to `rl_impl!()` proc macro in mcrs\_core\_macros which returns a tuple `("validated:string", colon_pos_u16)`, then `rl!()` wraps it via `ResourceLocation::__from_validated()`.

---

## Files to Create

### `crates/mcrs_core_macros/Cargo.toml`
```toml
[package]
name = "mcrs_core_macros"
version.workspace = true
edition.workspace = true

[lib]
proc-macro = true

[dependencies]
proc-macro2 = "1"
syn = "2"
quote = "1"
```

### `crates/mcrs_core_macros/src/lib.rs`
- `#[proc_macro] pub fn rl_impl(input: TokenStream) -> TokenStream`
- Parse string literal, validate, normalize, emit `("full:string", colon_pos_u16)`

---

## Files to Modify

### Phase 1: Core type (`mcrs_core`)

**`crates/mcrs_core/Cargo.toml`** — add `mcrs_core_macros` dependency

**`crates/mcrs_core/src/resource_location.rs`** — full rewrite:
- Generic `ResourceLocation<S = Arc<str>>` with `colon_pos: u16`
- `impl ResourceLocation<&'static str>`: `new_static(s)` const fn, `__from_validated()` for macro
- `impl<S: AsRef<str>> ResourceLocation<S>`: `namespace()`, `path()`, `as_str()`, `to_arc()`, `to_asset_path()`
- `impl ResourceLocation<Arc<str>>`: `parse(s)` replacing `FromStr`, `new(ns, path)`, `minecraft(path)`
- Trait impls: `Hash` (on string content), `PartialEq` (cross-variant), `Eq`, `Borrow<str>`, `Display`, `Debug`, `Serialize`, `Deserialize` (for String and Arc variants)
- `From<ResourceLocation<&'static str>> for ResourceLocation<Arc<str>>` and similar conversions
- `rl!()` declarative macro delegating to `mcrs_core_macros::rl_impl!()`

**`crates/mcrs_core/src/lib.rs`** — update re-exports

### Phase 2: Tag system (`mcrs_core`)

**`crates/mcrs_core/src/tag/key.rs`**:
- `TagKey` stores `ResourceLocation<&'static str>` instead of two `&'static str` fields
- `TagKey::new(rl: ResourceLocation<&'static str>)` replaces `TagKey::of(ns, path)`
- `resource_location()` returns `ResourceLocation<&'static str>` (Copy, zero-alloc)
- `asset_path()` uses `self.rl.namespace()` / `self.rl.path()` (O(1) via colon_pos)

**`crates/mcrs_core/src/tag/static_tags.rs`**:
- Internal storage stays `HashMap<ResourceLocation<Arc<str>>, ...>` (owned keys)
- `request()`: converts `key.resource_location()` to Arc via `.to_arc()` for HashMap insert (init only)
- `contains()` / `get()`: zero-alloc lookup via `self.inner.get(tag.resource_location().as_str())`
- `contains_rl()` / `get_rl()`: accept `&str` or generic `<S: AsRef<str>>`

**`crates/mcrs_core/src/tag/dynamic_tags.rs`** — same pattern as static\_tags

**`crates/mcrs_core/src/tag/file.rs`** — `TagEntry::Element(ResourceLocation<Arc<str>>)` stays Arc (parsed from JSON at runtime)

**`crates/mcrs_core/src/registry/static_registry.rs`**:
- Internal: `index: HashMap<ResourceLocation<Arc<str>>, u32>`
- `register()`: accepts `impl Into<ResourceLocation<Arc<str>>>`
- `id_of()` / `get_by_loc()`: accept `&str` for zero-alloc lookup

### Phase 3: Vanilla types (`mcrs_vanilla`)

**`crates/mcrs_vanilla/Cargo.toml`** — add `mcrs_core_macros` (if not transitive)

**`crates/mcrs_vanilla/src/block/mod.rs`**:
- `Block.identifier: ResourceLocation<&'static str>` (was `Ident<&'static str>`)
- Remove `use mcrs_protocol::Ident`

**`crates/mcrs_vanilla/src/item/mod.rs`**:
- `Item.identifier: ResourceLocation<&'static str>` (was `Ident<&'static str>`)
- Remove `use mcrs_protocol::Ident`

**`crates/mcrs_vanilla/src/block/macros.rs`**:
- `generate_block_states!` uses `rl!($block_name)` instead of `ident!($block_name)` (lines 65, 108)

**`crates/mcrs_vanilla/src/block/minecraft/*.rs`** (~40 files):
- `identifier: ident!("stone")` → `identifier: rl!("stone")`
- Import `mcrs_core::rl` instead of `mcrs_protocol::ident`
- Files that don't use the macro (like stone.rs): direct change

**`crates/mcrs_vanilla/src/block/minecraft/mod.rs`**:
- Remove `ident_to_resource_location()` helper
- `register_all_blocks`: `registry.register($const_name.identifier, &$const_name)` (identifier is already a ResourceLocation)

**`crates/mcrs_vanilla/src/item/minecraft/mod.rs`**:
- Same: remove conversion helper, pass identifier directly

**`crates/mcrs_vanilla/src/block/tags.rs`**:
- `TagKey::of("minecraft", "mineable/pickaxe")` → `TagKey::new(rl!("minecraft:mineable/pickaxe"))`
- ~11 constants

**`crates/mcrs_vanilla/src/item/tags.rs`**:
- Same pattern, ~5 constants

**`crates/mcrs_vanilla/src/enchantment/tags.rs`**:
- Same pattern, ~22 constants

**`crates/mcrs_vanilla/src/item/component/tool.rs`**:
- `ToolMaterial` consts: `TagKey::of(...)` → `TagKey::new(rl!(...))`  (~7 consts)
- `get_mining_speed()` line 76-79: `block_registry.id_of(block.identifier.as_str())` instead of `ResourceLocation::new(block.identifier.namespace(), block.identifier.path())`
- `is_correct_block_for_drops()` line 99-102: same simplification

**`crates/mcrs_vanilla/src/lib.rs`**:
- `request_enchantment_assets`: `ResourceLocation::from_str_const(name)` → `ResourceLocation::parse(name).unwrap()` (runtime, not const — these are `&str` from array)

### Phase 4: mcrs\_minecraft (if needed)

**`crates/mcrs_minecraft/src/configuration.rs`** — protocol boundary: convert `ResourceLocation<S>` → `Ident<Cow<str>>` for wire format

**Other mcrs\_minecraft files** — update imports, replace any `Ident<String>` used as registry keys with `ResourceLocation<Arc<str>>`

---

## Implementation Order

1. Create `crates/mcrs_core_macros/` with `rl_impl!()` proc macro
2. Add to workspace `Cargo.toml` members
3. Rewrite `resource_location.rs` with generic struct + `rl!()` macro
4. Update `mcrs_core/src/lib.rs` re-exports
5. Update `tag/key.rs` — TagKey stores `ResourceLocation<&'static str>`
6. Update `registry/static_registry.rs` — generic lookup methods
7. Update `tag/static_tags.rs` and `tag/dynamic_tags.rs` — zero-alloc lookup
8. Verify `mcrs_core` compiles: `cargo check -p mcrs_core`
9. Update `mcrs_vanilla/src/block/mod.rs` and `item/mod.rs` — identifier type change
10. Update `block/macros.rs` — `rl!()` instead of `ident!()`
11. Update all `block/minecraft/*.rs` files — `rl!()` macro
12. Update `block/minecraft/mod.rs` and `item/minecraft/mod.rs` — remove conversion helpers
13. Update tag declaration files (`block/tags.rs`, `item/tags.rs`, `enchantment/tags.rs`)
14. Update `item/component/tool.rs` — simplified lookups + TagKey::new
15. Update `lib.rs` — enchantment loading
16. Verify `mcrs_vanilla` compiles: `cargo check -p mcrs_vanilla`
17. Update `mcrs_minecraft` files as needed
18. Full build: `cargo build`
19. Run tests: `cargo test`

---

## Verification

1. `cargo check -p mcrs_core` — core compiles with generic ResourceLocation
2. `cargo check -p mcrs_vanilla` — all block/item consts compile with `rl!()`, tags compile with `TagKey::new()`
3. `cargo build` — full workspace builds
4. `cargo test` — existing tests pass
5. Manual: verify `rl!("stone")` produces `"minecraft:stone"` and `rl!("INVALID!")` fails at compile time
6. Manual: verify `StaticTags.contains()` path has no Arc allocation (check assembly or add a benchmark)
