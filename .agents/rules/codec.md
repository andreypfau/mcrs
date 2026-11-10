---
paths:
  - "crates/mcrs_minecraft_*registr*/**"
  - "crates/mcrs_minecraft_{nbt,snbt,datapack,pack_file,biome_file,model_file,text,loot}/**"
  - "crates/mcrs_minecraft_{environment,dimension,dimension_environment,predicate,block_predicate,value_provider,enchantment,damage}/**"
  - "crates/mcrs_minecraft_worldgen*/**"
  - "crates/mcrs_minecraft_{protocol,protocol_game,wire,save,anvil}/**"
---

# Serde is the codec

The game describes each data shape once, and that one description drives the data pack JSON,
the NBT in the save and the network buffer. Serde plays that role here. A type with
`Serialize` / `Deserialize` reads from `serde_json`, writes through `mcrs_minecraft_nbt`, and
crosses the wire without a second parser.

> Never express a shape as a function over `serde_json::Value`. Express it as a
> `Deserialize` / `Serialize` implementation: derived where the shape is ordinary, hand-written
> where it is not.

A hand-rolled `parse(&serde_json::Value)` is wrong twice: it reimplements what serde already
does, and it welds the type to one encoder. Registry snapshots reach the client as **NBT**
(`mcrs_minecraft_nbt::to_nbt_compound`), so a JSON-shaped field is silently NBT-encoded too.
NBT is strongly typed (byte, short, int, long, float, double), while
`serde_json::Value::Number` picks between i64, u64 and f64 from how the text looked. A tick
count lands in an NBT long where the game writes an int.

- **A value that is either a bare scalar or a full object** is a hand-written `Deserialize` plus
  a symmetric `Serialize`, so the two shapes round-trip unchanged. `TimeMarker` in
  `crates/mcrs_minecraft_environment/src/time_marker.rs` is the model in this repository.
- **Validate at load, not at use.** Parsing deferred to the moment a value is consumed means a
  malformed data pack surfaces late, with worse context, and only if something touches that
  entry. A malformed asset fails at load.
- **A value whose type depends on a sibling is still typed.** When a map key chooses the type of
  its value (a timeline keyframe is parsed into the type of the attribute that is its key), use
  `DeserializeSeed`: a `MapAccess` visitor reads the key, resolves what it selects, and calls
  `next_value_seed` with it. "The type is only known at runtime" is a reason for a seed, never a
  reason to keep `serde_json::Value`.
- **Raw retention does not buy round-trip fidelity.** Make the codec symmetric and test it; do
  not keep bytes as a substitute.
- An unknown field in a data pack file is ignored. A missing or malformed known field is a loud
  error at load. Do not put `deny_unknown_fields` on a type read from a data pack.
