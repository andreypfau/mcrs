# Resource locations: a borrowed and an owned type

This note states the shape of the resource location types in `mcrs_minecraft_resource`. It also records the audit that decides where a name can stay a name and where it must become a numeric registry id.

## Today

A `ResourceLocation<S>` keeps `namespace:path` as one string `S` with the position of the colon. Three storage types exist: `Arc<str>`, `&'static str` and `Cow<'a, str>`. The keys `ResourceKey<T, S>` and `TagKey<R, S>` carry the same parameter.

This layout has three costs:

- A borrowed location cannot omit its namespace. The command parser reads `stone` as `minecraft:stone`, which is not in the input. So the command crate has a second type, `Identifier<'a>`.
- The owned form is `Arc<str>`. That form is shared ownership, which the project removes from library code.
- Code converts between the three storage types. The workspace has 121 calls to `to_arc()`.

## Scale

The counts are from `main` at the time of this note.

| Item | Uses | Files |
|------|------|-------|
| `rl!` | 6,060 | 213 |
| `StaticResourceLocation` | 901 | 69 |
| `ResourceLocation` | 1,173 | 224 |
| `ResourceKey<_, &'static str>` | 3,217 | 69 |
| `TagKey<_, &'static str>` | 910 | 25 |
| `to_arc()` | 121 | 56 |
| `Identifier` | 106 | 20 |
| `Borrow<str>` lookups | 5 | 2 |

The key generator writes almost all of the `ResourceKey` and `TagKey` constants. A change to the generator templates moves them.

## Runtime audit

**Maps.** Non-test code has 121 maps or sets with a location or a key as the key. Of these, 104 are in the worldgen crates, the registries and the world crate. They are filled and read while the data pack loads. The audit found no such map that a tick reads.

**Parses and clones.** 23 files hold Bevy systems that name a location. Of these, only the network handshake, the client joined-game handler, the client column stream, the world clock, the world options and the worldgen registries plugin parse one. These parses occur once per connection or once per load, not per tick.

**Conclusion.** Names stay at the edges: data pack load, the wire, commands and the save format. Code that runs per tick already uses numeric ids. The split adds no runtime lookup by name.

## Shape

```rust
#[derive(Clone, Copy)]
pub struct ResourceLocation<'a> {
    namespace: &'a str,
    path: &'a str,
}

/// Always canonical `namespace:path`.
pub struct ResourceLocationBuf {
    string: Box<str>,
    colon: u16,
}
```

- `ResourceLocation<'a>` is the borrowed form. It is `Copy`. A parse of `stone` gives the namespace `"minecraft"` and the path `"stone"` without an allocation, because a `&'static str` fits any `'a`.
- `ResourceLocationBuf` is the owned form. It holds one `Box<str>` and no `Arc`.
- `ResourceLocationBuf::as_location` gives the borrowed form. The name is not `as_ref`, because `AsRef::as_ref` must return a reference, and this method returns a value. `From<&ResourceLocationBuf>` gives the same, so a lookup can take `impl Into<ResourceLocation<'_>>`.
- `ResourceLocation::to_buf` and `From<ResourceLocation<'_>>` give the owned form. They allocate one time.
- `rl!("ns:path")` gives a `const` `ResourceLocation<'static>`, split at compile time. `StaticResourceLocation` and the type parameter `S` go away.
- `ResourceLocationBuf::as_str` gives the canonical text. The borrowed form has no `as_str`, because its two parts are not one string. `Display` writes `namespace:path` for both types.

**Equality, hash and order.** `Eq` and `Hash` operate on the namespace and the path. `Ord` compares the path first and the namespace second, both byte-wise, which is the total order of identifiers in REG-ID-002 of `docs/spec/registry.md`. The implementations are the same for both types, and `PartialEq` works across the two types. So `minecraft:stone` and a parsed `stone` are equal and have the same hash.

The order changes from today, where `Ord` compares the whole `namespace:path` string. For identifiers in one namespace the two orders are the same. Three outputs follow the order of a map keyed by a location, so they change when more than one namespace is present: the baked dimension list after the three vanilla dimensions, the order of the tags in the tags packet, and the key order of the structure starts and references in an Anvil chunk.

**Keys.** `ResourceKey` and `TagKey` follow the same split:

- `ResourceKey<'a, T>` and `TagKey<'a, R>` wrap a `ResourceLocation<'a>`. They are `Copy`, and the generated constants are `ResourceKey<'static, T>`.
- `ResourceKeyBuf<T>` and `TagKeyBuf<R>` wrap a `ResourceLocationBuf`. A value that keeps a key, such as the `recipes` item component, holds the owned form.

**Maps.** A map keeps the owned form as its key. The std `Borrow` trait cannot give a `ResourceLocation<'_>` from a `ResourceLocationBuf`, because `Borrow` returns a reference and the borrowed form is a value made from two parts. So a std map cannot look up an owned key by a borrowed location.

- A map that is read by name after load is a `hashbrown::HashMap`. The `Equivalent` implementations let a borrowed location look up an owned key without an allocation. This applies to the registry tables, the names and tags of a registry, the block and item definitions, the structure templates and the client item models.
- A `BTreeMap` that is read by name only at load time or in a test makes a `ResourceLocationBuf` for the lookup.

**Codecs.**

- Serde: `ResourceLocationBuf` reads an owned string. `ResourceLocation<'a>` reads a borrowed string and fails when the input cannot lend one, for example a JSON string with an escape. Both write `namespace:path`.
- NBT goes through the same serde implementations.
- Wire: `Decode<'a>` for `ResourceLocation<'a>` borrows from the frame. A decoder makes a `ResourceLocationBuf` only for a location that it keeps. `Encode` writes the length and then the parts, without a buffer. The `Cow` form goes away.

**Commands.** The command parser returns a `ResourceLocation<'a>`. `Identifier` goes away.

**Size.** `ResourceLocation<'a>` is 32 bytes, against 24 bytes for the `&'static str` form today. `ResourceLocationBuf` is 24 bytes. A test pins both sizes. A value that is stored per section or per entity must hold a numeric id, not a location.

**Allocation.** A parse of a borrowed location from a `&str` allocates nothing. A test with the counting allocator of `mcrs_test_alloc` pins that.

## Pull requests

Each step stays green and builds on the step before it.

1. This note.
2. Add `ResourceLocationBuf`, `ResourceKeyBuf` and `TagKeyBuf` with their codecs and the round-trip tests for JSON, NBT and the wire. Move every owned location and key to them: `ResourceLocation<Arc<str>>`, the default parameter and `to_arc()`. The `&'static str` and `Cow` forms stay for one more step.
3. Change the borrowed form to `ResourceLocation<'a>` with two parts, and `rl!` to a two-part `const`. Change `ResourceKey` and `TagKey` to the borrowed shape, and change the generator templates. Remove `StaticResourceLocation`, the type parameter `S`, the `Cow` form and `Identifier`. Add the size and allocation tests. This step closes the issue.

Steps 2 and 3 each touch more than 200 files. The compiler finds each call site, so a split by crate gains nothing: a change in a public type of a lower crate reaches every crate above it.
