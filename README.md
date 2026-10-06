# mcrs

**Minecraft Java Edition in Rust: a set of crates, a server and a client, built on [Bevy](https://bevyengine.org) ECS.**

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Minecraft 26.4-snapshot-2](https://img.shields.io/badge/minecraft-26.4--snapshot--2-62b47a.svg)](assets/minecraft/version.json)
[![Bevy 0.19](https://img.shields.io/badge/bevy-0.19-232326.svg)](https://bevyengine.org)
[![Discord](https://img.shields.io/badge/discord-join-5865F2.svg?logo=discord&logoColor=white)](https://discord.gg/ged3nMRzwG)

mcrs is a server and a client for the latest Minecraft snapshot, and the crates
both of them are made of. Those crates are meant to be taken on their own: a
proxy, a bot, a minigame server, a world map or a structure viewer should take
only the layers it needs and pay for nothing above them.

> **Status: early development.** The world generator, the renderer and the
> protocol are well along; the public library API is not stable yet. See
> [Status](#status).

## Why mcrs

- **Tracks the newest snapshot.** One version at a time, currently
  `26.4-snapshot-2`. Data and reports are regenerated from the game jar by a
  single command, so moving to the next snapshot is mostly data, not code.
- **Data-driven.** Registries, biomes, dimensions, world generation and
  environment attributes are read from the vanilla data pack. A data pack that
  adds a biome or a dimension is picked up without a code change.
- **One codec for every format.** Serde types read the data pack JSON, write the
  NBT of a save and cross the network, the way vanilla's `Codec` does.
- **Vanilla-exact world generation.** Density functions, aquifers, ore veins and
  tree shapes are checked against vanilla output in tests.
- **Server and client in one ECS.** The same world data, registries and rules
  are read by both, and a dimension runs as its own Bevy sub-app.
- **Built for scale.** Work is proportional to change, not to world size; idle
  cost, memory per section and frame times are measured in [`PERF.md`](PERF.md).

## Layers

Each layer depends only on the layers below it.

| Layer | What it holds | Crates |
|---|---|---|
| Data and wire | NBT, text, keys, registries, typed packets in both directions, framing | `nbt`, `text`, `core`, `keys`, `registry`, `protocol` |
| Network | connections, login, packet I/O on tokio | `network` |
| Game data | blocks, items, biomes, dimensions, chunks, Anvil saves, lighting | `block`, `item`, `biome`, `dimension`, `chunk`, `anvil`, `light` |
| Engine | worlds, dimensions, players, entities as Bevy plugins | `world`, `level`, `entity`, `inventory` |
| Vanilla | gameplay and world generation | `server`, `worldgen*` |
| Rendering | meshing, terrain, sky and light pipelines | `mesh`, `render`, `render_*` |
| Applications | the server and the client | `mcrs`, `client` |

Every crate is named `mcrs_minecraft_<name>` under [`crates/`](crates).

## Status

| Area | State |
|---|---|
| World generation (noise, surface, carvers, features, structures) | Matches vanilla in parity tests |
| Lighting (sky, block and coloured light) | Working |
| Client rendering (terrain, sky, clouds, light) | Working; numbers in [`PERF.md`](PERF.md) |
| Anvil saves | Loading; the server does not save yet |
| Protocol | Status, login, configuration and play; about half the packets typed |
| Gameplay | Movement, digging, placing, chat, items, TNT, experience, early mobs |
| Online mode, encryption, proxy forwarding | Not yet |
| Crates usable without Bevy | Partly: `nbt`, `protocol`; more in progress |
| Library examples (proxy, bot, lobby server, map renderer) | Planned |

## Quick start

mcrs builds with a nightly Rust toolchain, which [`rust-toolchain.toml`](rust-toolchain.toml)
selects for you.

Run a server on a new world, or on an existing Java Edition save:

```bash
cargo run --release
cargo run --release -- "<path to a world folder>"
```

Run the client, which starts its own server for the world you give it:

```bash
cargo run --release -p mcrs_minecraft_client -- "<path to a world folder>"
```

The client reads textures and models from the official game jar; the
repository ships no Mojang art. A vanilla client of the same snapshot can also
join the server in offline mode.

Look at block models without a world:

```bash
cargo run --release -p mcrs_minecraft_client --example block_viewer
```

## Development

```bash
cargo nextest run -p <crate>        # one crate
cargo nextest run --workspace       # everything, before a commit
cargo nextest run --profile full    # plus the exhaustive sweeps
```

Updating to a new snapshot is one command; the procedure is in
[`crates/mcrs_minecraft_update/README.md`](crates/mcrs_minecraft_update/README.md).

## Community

Questions, ideas and progress reports: [Discord](https://discord.gg/ged3nMRzwG).
Issues and pull requests are welcome.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. Code adapted from
[valence](https://github.com/valence-rs/valence) keeps its own
[MIT notice](LICENSE-VALENCE-MIT).

mcrs is not affiliated with Mojang or Microsoft. Minecraft is a trademark of
Mojang Synergies AB.
