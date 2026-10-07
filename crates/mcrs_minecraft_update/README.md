# mcrs_minecraft_update

Moves the repository to another Minecraft version. It replaces the data pack
corpus, the client jar descriptor, the generated reports and the block and item
definitions from the game itself, and it regenerates the fixtures that were
captured from the game. It ports no code: what the new version changes in the
Rust crates is found by the tests it breaks and by the diffs it prints.

The target at the time of writing is `26.4-snapshot-2` (protocol version
1073742164, world version 5120).

## Where the version lives

`assets/minecraft/version.json` is the only statement of the version. It is the
file of the same name from the client jar, copied byte for byte, and
`mcrs_minecraft_core::VERSION` embeds it. Nothing else states the version or
one of the values in it:

- A test of the core crate fails when a Rust, TOML or Gradle source file
  contains one of its values.
- `tools/vanilla-oracle` and `tools/inventory-uat` read the game version from
  the file when Gradle configures them.
- The world plugin refuses to start on a corpus whose `version.json` differs
  from the one the binary embeds.
- Chunks, save files and templates written at another `DataVersion` fail to
  load. There is no datafixer. Worlds written between an update and the ports
  that follow it are not worth keeping.

## What the corpus is

`assets/minecraft` is the data pack only: the `data/minecraft` tree of the
client jar, plus `version.json`. The resource pack is not in the repository;
the client reads it from the jar that
`crates/mcrs_minecraft_client_jar/src/release.json` describes.

The corpus is the game's data pack exactly: an update writes every jar entry
and deletes every file the jar lacks, whatever its name. Local data lives in
packs under `assets/mcrs/datapacks/`, layered over the game's data pack, and
the update never touches them.

Three folders ship no files: `worldgen/density_function`, `worldgen/noise_settings`
and `worldgen/noise`. Their entries are built by
`mcrs_minecraft_worldgen_builtin` and served to the loaders when no file
exists, so a file at the same path overrides the built-in. An update compares
every jar entry of these folders with what the code builds. An identical entry
is dropped. A different one is written as a file, which keeps the game correct,
and is printed as `differs from the built-in`: port the difference into the
code, run the update again, and the file goes away.

## Updating

```sh
cargo run -p mcrs_minecraft_update -- <version id> [--allow-dirty] [--diff-out <directory>]
```

The version id is the only argument, for example `26.4-snapshot-2`. The
options are:

- `--allow-dirty` lets the update run when `assets`, the descriptor or the font
  hint have uncommitted changes. Without it the tool prints those changes and
  stops before it writes anything, so that the diffs below can be read from git.
- `--diff-out <directory>` also writes the three diffs to `protocol_id.txt`,
  `definitions.txt` and `names.txt` in that directory.

The steps, in order:

1. Fetch Mojang's version manifest and find the id in it. The package descriptor
   it points at must match the SHA-1 the manifest gives, and the client jar must
   match the size and SHA-1 of the package descriptor. The jar is cached in the
   launcher directory (`versions/<id>/<id>.jar`) and downloaded only when no
   verified copy is there.
2. Read `version.json` from the jar and stop if its id is not the requested one.
3. Replace `assets/minecraft` with `data/minecraft` of the jar, delete what the
   jar no longer has and write `version.json`.
4. Write `crates/mcrs_minecraft_client_jar/src/release.json`, the descriptor of
   the jar, its central directory and the package descriptor, and
   `crates/mcrs_minecraft_client_jar/src/font_hint.json`.
5. Run the game's data generator through the `dumpReports` task of
   `tools/vanilla-oracle` and the `dumpDefinitions` task, both into temporary
   directories.
6. Compare the dumped block definitions with the `blocks.json` the generator
   wrote and stop on any disagreement: both name the same blocks, the reported
   state ids are a gapless range, and for every block the state count, the id of
   every state computed from the declared property order and the default state
   agree. `blocks.json` is checked here and not stored.
7. Store `registries.json`, `packets.json` and `datapack.json` in
   `assets/mcrs/reports`, and replace `assets/mcrs/block_definition`,
   `assets/mcrs/item_definition` and `assets/mcrs/registry_definition` (the game
   rules) with what the dump wrote, including the README of the block
   definitions.
8. Write `assets/mcrs/reports/names.json` from the jar (see below), then
   regenerate the registry key sources from the three reports.

Three diffs are printed, and each is computed before the files it describes are
replaced:

- The `protocol_id` diff compares the new `registries.json` with the stored one:
  every registry entry whose protocol id moved, was added or was removed.
- The definition diff lists, per file of the block and item definitions, which
  fields were added, removed or changed, and counts the files that changed only
  in how they are written.
- The names diff lists the entries and tags the new `names.json` adds (`+`) or
  drops (`-`) against the stored one; a tag is written with a leading `#`.

The Gradle tasks run through `tools/vanilla-oracle/gradlew` with `--no-daemon`.
The first run downloads the game and its libraries.

## The names report

```sh
cargo run -p mcrs_minecraft_update -- names
```

`assets/mcrs/reports/names.json` lists, per registry, the names of its entries
and the names of its tags, in sorted order. The corpus does not hold every entry
of a registry (the entries the code builds ship no file), so the vanilla names
come from the client jar that `release.json` names, never from the corpus or the
built-in crate. The entries are those of every registry that `datapack.json`
marks as having elements; a registry with elements and no file in the jar is
listed empty. The tags are those of every registry with a tag file below
`data/minecraft/tags/`. `minecraft:dimension` has no files of its own: its
entries are the dimensions named by the world presets. The experimental packs
under `data/minecraft/datapacks/` contribute nothing.

The `names` command reads the stored `datapack.json`, verifies the jar against
the descriptor, prints the names diff and writes the report. It touches neither
`assets/minecraft`, the descriptor nor another report, and it runs no Gradle
task. A second run over the same jar prints `names diff: 0 rows` and leaves the
file unchanged.

## The registry key sources

```sh
cargo run -p mcrs_minecraft_update -- keys
```

The update and the `names` command end by writing the registry key sources
from `registries.json`, `datapack.json` and `names.json`; `keys` writes them
from the stored reports alone. A source file the generator no longer produces
is deleted. Never edit a generated source by hand.

`OWNERS` in `src/owners.rs` names, per registry, the crate that owns it and its
value type: a type the crate defines, or, for a static registry with entries, an
enum the generator writes. An owned registry is keyed in `src/keys` of its
owner, which holds the `RegistryKey`, its type binding and one module per
registry:

- a static registry keyed by an enum is a `static_registry!` enum of its
  entries, numbered by protocol id, with their locations in `ENTRIES`;
- another static registry is a `static_keys!` module of constants and
  `ENTRIES`, in protocol id order;
- a data registry with entries in `names.json` is a module of `ResourceKey`
  constants, except `minecraft:recipe` and `minecraft:advancement`.

`crates/mcrs_minecraft_keys` keys only the registries no crate owns, with
locations in place of typed keys. A registry with tags in `names.json` also
gets a `<module>_tags` module of `TagKey` constants, so `block_tags::LOGS` and
`item_tags::LOGS` are two constants of two registries. A constant is the entry
path in upper case with `/`, `.` and `-` as `_`; the jukebox songs `5`, `11`
and `13` become `FIVE`, `ELEVEN` and `THIRTEEN`, and a `brigadier:` entry takes
the prefix `BRIGADIER_`. Any other digit-leading name, a namespace without a
rule and two names that make one constant stop the generator with the name.

`crates/mcrs_minecraft_registry_catalog`, manifest included, is generated as
well: `STATIC_REGISTRIES` lists every static registry with its `ENTRIES`, and
`bindings()` chains the owners' type bindings. An owner that depends on the catalog is left out of it and binds
its own types.

A test of this crate regenerates the sources from the stored reports and fails,
naming the first file that differs, if a checked-in source is not what the
generator writes.

## Recapturing fixtures

Some test fixtures are output of the game, taken by the oracle tasks of
`tools/vanilla-oracle`. After an update they are stale, and one command
regenerates them:

```sh
cargo run -p mcrs_minecraft_update -- recapture <fixture name>
cargo run -p mcrs_minecraft_update -- recapture --all
```

A recapture runs the fixture's Gradle task into a temporary directory, copies
the files it wrote to their fixture directories and records the fixture in
`tools/captures.json`. The table `FIXTURES` in `src/fixtures.rs` is the one
place that states a fixture's name, task and destination. With `--all` every
fixture is recaptured in turn, a failure does not stop the others, and the exit
status is non-zero if any failed. An unknown name prints the known names. The
update command itself recaptures nothing.

`tools/captures.json` maps each fixture name to the id of the corpus version it
was captured at. A test in this crate fails by name for every fixture whose
entry is not the id in `assets/minecraft/version.json`, for a fixture missing
from the file and for an entry that names no fixture. After an update that test
is the list of fixtures to recapture.

A golden that carries its own cases is rewritten from them: the inputs are
kept and the game supplies every expected value. When the game rejects an input
it accepted at the previous capture, or accepts one it rejected, the run stops
and names the case (a golden without named cases prints the input itself), the
file stays as it was and nothing is recorded. The vanilla player file is such a
golden: its item stacks are rewritten through the game's item codecs and the
file is stamped with the game's data version.

Not regenerated from the current game and outside the fixture table: the Beta
fixtures (`beta_surface_corpus.json`, `beta_climate.json`,
`beta_draw_counts.json`), `corpus_ore_bands.txt`, which is a regression band
recorded from this project's own output, and the two light update fixtures of
the protocol crate, which are built by hand.

## After an update

The tool does not read the reference checkout. What a new version changes in
the protocol and the serialization formats is found by reading the reference
sources, and this list says where to look: the five protocol table files
(handshake, status, login, configuration and game), the directories
`network/` and `core/registries/`, and the files `RegistryDataLoader.java`,
`DataComponents.java`, `EntityDataSerializers.java`,
`SerializableChunkData.java`, `Strategy.java`, `DataFixers.java` and
`SharedConstants.java`. In the reference checkout, with the tags of the old and
the new version:

```sh
N=src/main/java/net/minecraft
git diff --stat <old tag> <new tag> -- \
    $N/network/protocol/handshake/HandshakeProtocols.java \
    $N/network/protocol/status/StatusProtocols.java \
    $N/network/protocol/login/LoginProtocols.java \
    $N/network/protocol/configuration/ConfigurationProtocols.java \
    $N/network/protocol/game/GameProtocols.java \
    $N/network $N/core/registries \
    $N/resources/RegistryDataLoader.java \
    $N/core/component/DataComponents.java \
    $N/network/syncher/EntityDataSerializers.java \
    $N/world/level/chunk/storage/SerializableChunkData.java \
    $N/world/level/chunk/Strategy.java \
    $N/util/datafix/DataFixers.java \
    $N/SharedConstants.java
```

A new snapshot may also need a few compile fixes in the worldgen oracles of
`tools/vanilla-oracle` before the fixtures can be recaptured; see its README.

## Requirements and limits

- JDK 25 on the path of Gradle.
- Network access to Mojang's metadata and downloads, and to the Fabric Maven
  repository, from which Gradle fetches Fabric Loom.
- One run at a time. Two runs of the tool against one working tree are not
  supported, and Gradle invocations against `tools/vanilla-oracle` are never run
  in parallel.
