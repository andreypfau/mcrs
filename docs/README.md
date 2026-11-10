# docs/

Technical documents of mcrs, written for spec authors, implementers and AI agents. They are exact and long.

For people: start at https://mc.rs/docs/ (to read the source of the site, see `docs-site/`).

How to read the status column: it comes from the header of the file. Only `cells.md` calls itself a working note. `cells.md` names `physics-architecture.md` and decisions 22 to 26 of `minecraft-bevy-sharding-architecture.md` as the text that supersedes it, so these two are listed as authority. A file whose header states no status is listed as "design note (no status in header)". If a header changes, change this table in the same PR.

| File | About | Status | Read if you |
|---|---|---|---|
| `clean-room.md` | who may read what, and what the repository may contain | authority | send any PR |
| `spec/` (67 files) | behavior of the game as rules with IDs. Start with `spec/README.md` | authority for behavior | write or implement a specification |
| `minecraft-bevy-sharding-architecture.md` | architectural decisions for the Bevy server | authority (decisions 22 to 26 supersede parts of `cells.md`) | change the engine design |
| `physics-architecture.md` | per-tick simulation of entities, block domain, movement checks | authority (supersedes parts of `cells.md`) | touch physics or entity ticking |
| `cells.md` | cells, the Muxer and the data layer | working note | read it with its status block in hand |
| `replication.md` | how the Muxer sends entities of a world to clients | design note (no status in header) | work on network scale |
| `worldgen.md`, `aquifer.md`, `surface.md`, `structures.md`, `scattering.md`, `heightmap.md` | world generation as invariants and fields | design note (no status in header). They name the game's code, so the clean-room exception applies | change world generation. Do not copy their text |
| `molang.md` | compiler and runtime of `mcrs_molang` | design note (no status in header) | work on Molang |
| `molang-corpus.md` | statistics of Molang expressions in the Bedrock packs | design note (no status in header) | decide Molang features |
| `entity-render.md`, `entity-render-budget.md` | GPU entity rendering and its workload model | design note (no status in header) | work on the renderer |
| `registry-graph.md` | which registries a value refers to | design note (no status in header) | add or move a registry |
| `resource-location.md` | borrowed and owned resource location types | design note (no status in header) | touch keys |
| `text-item-hover.md` | text with an item hover at the wire layer | design note (no status in header) | touch text or item components |
| `command-dimensions.md` | commands across dimensions | design note (no status in header) | touch commands |
| `bevy-asset-cycles.md` | cyclic asset dependencies in `bevy_asset` 0.19.1 | design note (no status in header) | load assets with cycles |
| `farms/gold-farm.md` | benchmark scene of a gold farm | design note (no status in header) | write a benchmark |
| `perf/` | screenshots used by `PERF.md` | data | read `PERF.md` |

Related files in the root: `PERF.md` (measurements, each with machine and scenario), `DECISIONS.md` (one line per decision), `README.md`.
