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

The local files are the ones with a path component that begins with `beta`
(for example `worldgen/biome/beta_forest.json` and `worldgen/noise_settings/beta.json`).
An update keeps them, reports how many it kept, and refuses a jar entry that
would land on one.

## Updating

```sh
cargo run -p mcrs_minecraft_update -- <version id> [--allow-dirty] [--diff-out <directory>]
```

The version id is the only argument, for example `26.4-snapshot-2`. The
options are:

- `--allow-dirty` lets the update run when `assets`, the descriptor or the font
  hint have uncommitted changes. Without it the tool prints those changes and
  stops before it writes anything, so that the diffs below can be read from git.
- `--diff-out <directory>` also writes the two diffs to `protocol_id.txt` and
  `definitions.txt` in that directory.

The steps, in order:

1. Fetch Mojang's version manifest and find the id in it. The package descriptor
   it points at must match the SHA-1 the manifest gives, and the client jar must
   match the size and SHA-1 of the package descriptor. The jar is cached in the
   launcher directory (`versions/<id>/<id>.jar`) and downloaded only when no
   verified copy is there.
2. Read `version.json` from the jar and stop if its id is not the requested one.
3. Replace `assets/minecraft` with `data/minecraft` of the jar, delete what the
   jar no longer has (except the `beta` files) and write `version.json`.
4. Write `crates/mcrs_minecraft_client_jar/src/release.json`, the descriptor of
   the jar, its central directory and the package descriptor, and
   `crates/mcrs_minecraft_client_jar/src/font_hint.json`.
5. Run the game's data generator through the `dumpReports` task of
   `tools/vanilla-oracle`, and store `registries.json`, `packets.json`,
   `blocks.json` and `datapack.json` in `assets/mcrs/reports`.
6. Run the `dumpDefinitions` task and replace `assets/mcrs/block_definition` and
   `assets/mcrs/item_definition` with what it wrote, including the README of the
   block definitions.

Two diffs are printed, and each is computed before the files it describes are
replaced:

- The `protocol_id` diff compares the new `registries.json` with the stored one:
  every registry entry whose protocol id moved, was added or was removed.
- The definition diff lists, per file of the block and item definitions, which
  fields were added, removed or changed, and counts the files that changed only
  in how they are written.

The Gradle tasks run through `tools/vanilla-oracle/gradlew` with `--no-daemon`.
The first run downloads the game and its libraries.

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
