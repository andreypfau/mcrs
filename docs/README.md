# docs/

Technical documents of mcrs, written for spec authors, implementers and AI agents. They are exact and long.

For people: start at https://mc.rs/docs/ (to read the source of the site, see `docs-site/`).

| File | About | Read if you |
|---|---|---|
| `clean-room.md` | who may read what, and what the repository may contain | send any PR |
| `spec/` | behavior of the game as rules with IDs. Start with `spec/README.md` | write or implement a specification |
| `minecraft-bevy-sharding-architecture.md` | principles of the Bevy server: compatibility levels, sharding, ticks, determinism | change the engine design |
| `physics-architecture.md` | per-tick simulation of entities, block domain, movement checks | touch physics or entity ticking |
| `cells.md` | cells, the Muxer and the data layer | read it with its status block in hand |
| `replication.md` | how the Muxer sends entities of a world to clients | work on network scale |
| `worldgen.md`, `aquifer.md`, `surface.md`, `structures.md`, `scattering.md`, `heightmap.md` | world generation as invariants and fields | change world generation. Do not copy their text |
| `molang.md` | compiler and runtime of `mcrs_molang` | work on Molang |
| `molang-corpus.md` | how the tool extracts Molang expressions from the Bedrock packs | work on the Molang corpus tool |
| `entity-render.md`, `entity-render-budget.md` | GPU entity rendering and its workload model | work on the renderer |
| `registry-graph.md` | which registries a value refers to | add or move a registry |
| `damage.md` | design of health, the hit pipeline, death and respawn; mobs as ECS entities (#1067) | touch health, damage, death or the player attack |
| `item-design.md` | public API and crate map of the item and inventory rework: a stack as one entity, the read and write tools, the steps | touch items, inventories or containers |
| `command-dimensions.md` | commands across dimensions | touch commands |
| `bevy-asset-cycles.md` | how cyclic registries reference assets by id and join at freeze | load assets with cycles |
