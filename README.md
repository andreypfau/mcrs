# mcrs

**Minecraft Java Edition in Rust: a set of crates, a server and a client, built on [Bevy](https://bevyengine.org) ECS.**

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Minecraft 26.4-snapshot-4](https://img.shields.io/badge/minecraft-26.4--snapshot--4-62b47a.svg)](assets/minecraft/version.json)
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

A new Minecraft server in Rust is announced about twice a year; there is a
[counter](https://dayssincelastrustmcserver.com/) for it, and a
[tracker](https://areweminecraftyet.com/) that still answers "not yet" for all
of them. Most follow the same recipe: take the Java server's classes, rewrite
them as Rust structs behind `Arc<Mutex<…>>`, keep the single game-tick thread,
and put async tasks around it. That is Java's object graph with a borrow
checker. It inherits the design that makes the vanilla server slow, and adds
locks and deadlocks of its own.

A new language does not fix that, and neither does a prompt that says
"rewrite Minecraft in Rust, make no mistakes". The hard part is deciding how
the game should be built once the Java is gone, and that is most of the work in
this repository. mcrs starts with the engine.

- **The ECS is the engine.** World state is components and resources; behaviour
  is systems, each one transformation over every entity that has the data it
  names. Bevy's scheduler runs systems in parallel when their data access does
  not overlap, and a system can split its entities across threads. Nobody
  writes a lock for that: mcrs code holds no `Arc`, no `Mutex` and no trait
  object on the hot path.
- **Parallel by what the work touches.** Java server forks move one job at a
  time off the main thread and sell it as a feature: async pathfinding, async
  entity tracking, parallel world ticking, sometimes as a paid jar. Folia goes
  further and cuts the world into regions, but a crowded spot still ends up on
  one thread. mcrs looks at what each piece of work reads and writes. A mob
  farm and the redstone that drives it sit in the same chunks, yet mob AI and
  redstone run in parallel, and so do two farms at opposite ends of a base.
  Work that really interacts stays together and runs in order, and the result
  is the same on any number of threads.
- **Work is proportional to change.** A resting entity and an untouched section
  cost nothing. Idle cost, memory per section and frame times are measured in
  [`PERF.md`](PERF.md), and every exception to these rules cites a number
  there.
- **Specified before it is built.** Each domain is written from a
  specification in [`docs/spec/`](docs/spec) under
  [clean-room rules](docs/clean-room.md).
  Vanilla is the authority on data and observable behaviour; the shape of the
  code is ours.
- **Parity is declared and tested.** Every subsystem is `exact` (protocol,
  saves, player movement, world generation), `semantic` (redstone, fluids,
  block updates) or an intentional deviation that is written down. Exact parts
  are compared with output of the game; world generation already matches in
  parity tests.
- **A toolkit, not a monolith.** Most Minecraft crates do one job: read NBT,
  open region files, parse a schematic, draw a map. There are many NBT crates
  alone, each with its own types, and gluing two of them together means
  converting between those types. mcrs crates can be taken one at a time in the
  same way, and they share their types: a block state read from a save is the
  same value the renderer meshes and the world generator places. Nothing
  below the engine needs Bevy, and only the network layer needs tokio.
- **Data-driven at run time.** Registries, biomes, dimensions, world
  generation and environment attributes are read from the vanilla data pack. A
  data pack that adds a biome or a dimension is picked up without a rebuild.
  Serde types read the data pack JSON, write the NBT of a save and cross the
  network, the way vanilla's `Codec` does.
- **Tracks the newest snapshot.** One version at a time, currently
  `26.4-snapshot-4`. Data and reports are regenerated from the game jar by a
  single command, so moving to the next snapshot is mostly a data update.

## Layers

Each layer is meant to depend only on the layers below it, so a user takes
any prefix and pays for nothing above it. A few crates do not follow this yet:
the protocol and network crates still reach into game data. Below layer 4,
Bevy is an optional feature; below layer 2, so is tokio.

| # | Layer | What it holds |
|---|---|---|
| 1 | Protocol | ids, positions, NBT, text, keys; the codec traits; framing, compression, encryption; handshake, status, login and configuration packets |
| 2 | Network | the connection state machine, login on both sides, authentication, proxy forwarding, frames encoded once per broadcast; tokio, Bevy optional |
| 3 | Game data and rules | registries baked from the data pack, blocks, items, entities, chunks, Anvil saves and their packets; kernels without Bevy for lighting, world generation and block simulation |
| 4 | Engine | Bevy plugins for worlds, dimensions, chunk sources, players, events and commands, and the drivers of the kernels; no vanilla behaviour by default |
| 5 | Vanilla | what blocks, items and mobs do, world generation and lighting, as removable plugins |
| 6 | Rendering | meshing and pipelines over layer 3 data; no network, client or server |
| 7 | Applications | the server binary, the client and tools |

Every crate is named `mcrs_minecraft_<name>` under [`crates/`](crates).

## Status

| Area | State |
|---|---|
| World generation (noise, surface, carvers, features, structures) | Matches vanilla in parity tests |
| Lighting (sky, block and coloured light) | Working |
| Client rendering (terrain, sky, clouds, light) | Working; numbers in [`PERF.md`](PERF.md) |
| Anvil saves | Loading; the server does not save yet |
| Protocol | Status, login, configuration and play; about half the packets typed |
| Gameplay | Joining, chat, game modes and a plain flight stub; the rest returns from specifications |
| Online mode, encryption, proxy forwarding | Not yet |
| Crates usable without Bevy | Layers 1 to 3 with default features off, except `assets` |
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
[`tools/mcrs_minecraft_update/README.md`](tools/mcrs_minecraft_update/README.md).

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
