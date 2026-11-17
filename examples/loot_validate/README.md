# Loot validate

Load a data pack over the vanilla pack, bake every loot table and print each
problem on its own line. A problem of the bake is `<table> at <path>: <problem>`.
A problem of the load is the diagnostic of the registry load.

```sh
cargo run -p mcrs_minecraft_loot_validate -- path/to/pack path/to/vanilla path/to/assets
```

The executable takes three directories:

1. The data pack: a directory that holds `pack.mcmeta` and `data/`.
2. The vanilla directory: the `data/minecraft` tree of the pinned client jar.
   `mcrs_minecraft_fixtures::vanilla_dir` (`tools/mcrs_minecraft_fixtures`) gives it after
   `cargo run -p mcrs_minecraft_client_jar --example fetch`.
3. The assets directory: the `assets` directory of this repository. It holds
   the block definitions in `mcrs/block_definition`. The bake needs them
   because a table names blocks and block state properties, and the bake
   checks each name against them.

The exit status tells the result:

- 0: the load and the bake found no problem.
- 1: the bake found a problem, or the load gave an error about a file of the
  data pack. Each problem is on the standard output, and so is each warning of
  the load about the data pack.
- 2: the validator did not run. The reason is on the standard error. A wrong
  argument, a read error, registries that do not load, and a data pack that is
  missing, not available or of unknown compatibility give this status.

A table of the vanilla pack is checked as well as a table of the pack.

The executable uses the registry, block, item table and loot table crates without
Bevy and without the engine.

```sh
cargo nextest run -p mcrs_minecraft_loot_validate
cargo nextest run -p xtask
```

The checks follow LOOT-CTX-013 in `docs/spec/loot.md`.
