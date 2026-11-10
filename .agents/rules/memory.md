---
paths:
  - "crates/mcrs_minecraft_{chunk,volume,block,entity_rows,light_engine,heightmap,anvil}/**"
  - "crates/mcrs_voxel/**"
  - "crates/mcrs_engine_store/**"
  - "crates/mcrs_minecraft_engine_world/**"
---

# Memory budget

The server's footprint is four terms, each with its own budget in bytes: the base (registries
and tables, no player), the cost per loaded section, the cost per player and per entity, and
the transient peak. The per-section term is the multiplier that matters: a hundred bytes more
per section is megabytes per player.

- **Every large owner can state its size.** `BlockDefinitions` counts its bytes; sections, light
  and queues do the same, so growth is visible and attributable.
- **The size of a multiplied type is pinned by a test.** A component carried by every section or
  every entity has an `assert!(size_of::<T>() <= N)`.
- **Everything retained has an owner and a release condition.** A collection with no stated
  bound is a leak: a memo table, a send queue for a slow client, a cache kept just in case.
- **One copy of one fact.** Do not hold the parsed and the encoded form together.
- **The peak is bounded by back-pressure.** The allocator rarely returns memory, so the process
  costs its peak, not its average. Work in flight and per-thread scratch buffers have explicit
  limits.
- **A fixed table is cheap; a cache per section or per entity needs a number.**
- **The source does not outlive the bake.** Parsed data pack JSON is dropped once the dense
  tables are built.
- **Keep what the palette says; do not expand it.**
