# mcrs_minecraft_update

Moves the repository to another Minecraft version. It replaces the version
file, the client jar descriptor, the generated reports and the block and item
definitions from the game itself, and it regenerates the fixtures that were
captured from the game. It ports no code: what the new version changes in the
Rust crates is found by the tests it breaks and by the diffs it prints.

## Where the version lives

`assets/minecraft/version.json` is the only statement of the version. It is the
file of the same name from the client jar, copied byte for byte, and
`mcrs_minecraft_core::VERSION` embeds it. Nothing else states the version or
one of the values in it:

- A test of the core crate fails when a Rust, TOML or Gradle source file
  contains one of its values.
- `tools/vanilla-oracle` reads the game version from the file when Gradle
  configures it.
- The world plugin refuses to start on a data pack whose `version.json`
  differs from the one the binary embeds.
- Chunks, save files and templates written at another `DataVersion` fail to
  load. There is no datafixer. Worlds written between an update and the ports
  that follow it are not worth keeping.

## Where the data pack lives

The game's data pack is not in the repository. Every consumer reads the
`data/minecraft` tree and `version.json` of the client jar that
`crates/mcrs_minecraft_client_jar/src/release.json` describes, unpacked once
into the user's cache directory (`mcrs_minecraft_client_jar::vanilla_data`) and
served as the `minecraft` directory of the pack root. The server takes it
from `ServerCorePlugin::data_pack`: a directory, or by default the client
jar of a launcher's installation or else a download. The tests read the same
directory: the nextest setup script installs the jar before any test runs. The
resource pack is read from the same jar.

`assets/minecraft` holds only `version.json`. Local data lives in packs under
`assets/mcrs/datapacks/`, layered over the game's data pack, and the update
never touches them.

`mcrs_minecraft_worldgen_builtin` builds the entries of
`worldgen/density_function`, `worldgen/noise_settings`, `worldgen/noise`,
`worldgen/template_pool`, `worldgen/biome` and the village templates of
`structure`. A built entry replaces the game's file of the same name, and a file
of a local pack replaces the built entry. A test of that crate compares every
built entry with the game's file and fails, naming each one, when they differ:
after an update, port the difference into the code.

## Updating

```sh
cargo run -p mcrs_minecraft_update -- <version id> [--allow-dirty] [--diff-out <directory>]
```

The version id is the only argument, for example `26.4-snapshot-7`. The
options are:

- `--allow-dirty` lets the update run when the version file, the reports, the
  definitions, the descriptor or the font hint have uncommitted changes. Without it the tool prints those changes and
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
3. Write `version.json` of the jar to `assets/minecraft/version.json`.
4. Write `crates/mcrs_minecraft_client_jar/src/release.json`, the descriptor of
   the jar, its central directory and the package descriptor,
   `crates/mcrs_minecraft_client_jar/src/font_hint.json` and
   `crates/mcrs_minecraft_client_jar/src/index_objects.json`, the font and
   title background folders of the asset index the package descriptor names.
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
   rules, the effects of each potion and the colour of each effect) with what
   the dump wrote, including the README of the block definitions.
8. Write `assets/mcrs/reports/names.json` from the jar (see below), then
   regenerate the registry key sources from the three reports and the
   definitions `game_rule.json`, `potion.json` and `mob_effect.json` of
   `assets/mcrs/registry_definition`.
9. Write `assets/mcrs/reports/creative_tabs.json` from a run of the game (see
   below).

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
and the names of its tags, in sorted order. The vanilla names come from the
client jar that `release.json` names, never from the built-in crate. The entries are those of every registry that `datapack.json`
marks as having elements; a registry with elements and no file in the jar is
listed empty. The tags are those of every registry with a tag file below
`data/minecraft/tags/`. `minecraft:dimension` has no files of its own: its
entries are the dimensions named by the world presets. The experimental packs
under `data/minecraft/datapacks/` contribute nothing.

The `names` command reads the stored `datapack.json`, verifies the jar against
the descriptor, prints the names diff and writes the report. It touches neither
`assets/minecraft/version.json`, the descriptor nor another report, and it runs no Gradle
task. A second run over the same jar prints `names diff: 0 rows` and leaves the
file unchanged.

## The creative report

```sh
cargo run -p mcrs_minecraft_update -- creative-tabs
```

`assets/mcrs/reports/creative_tabs.json` holds the stacks of each items tab of the
creative screen and the feature flags of each item that needs a flag other than
`minecraft:vanilla`. The data pack does not contain the tab lists, so the task
`dumpCreativeTabs` of `tools/vanilla-oracle` takes them from a run of the game on
the vanilla data pack with each feature flag on. `docs/spec/client-creative-inventory.md`,
CCI-CNT-001, states the shape.

The command runs that task, reads its output with `mcrs_minecraft_creative_report`
and stops if the reader rejects it. Then it replaces the stored report. The tabs are
in the order of the registry, and the items and the keys of each stack are in
sorted order, so a second run gives the same bytes. A full
update writes this report after the names report. The reader knows the tabs from
the registry key sources that the binary was built with. When a version adds or
renames a tab, the full update stops at this step with an unknown tab. Run
`creative-tabs` again: the rebuild takes the new key sources.

## The registry key sources

```sh
cargo run -p mcrs_minecraft_update -- keys
```

The update and the `names` command end by writing the registry key sources
from `registries.json`, `datapack.json` and `names.json`, and from
`game_rule.json`, `potion.json` and `mob_effect.json` of
`assets/mcrs/registry_definition`; `keys` writes them from the stored reports
and definitions alone. A source file the generator no longer produces
is deleted. Never edit a generated source by hand.

`OWNERS` in `src/owners.rs` names, per registry, the crate that owns it and its
value type: a type the crate defines, or, for a static registry with entries, an
enum the generator writes. An owned registry is keyed in flat modules of its
owner: `src/registry_keys.rs` holds the `RegistryKey` and its `Registered`
implementation, and each registry gets `src/<registry>_keys.rs` and, with tags,
`src/<registry>_tag_keys.rs`. The owner declares these modules in its own
`lib.rs`; the generator stops and names a declaration that is missing. The
modules are:

- a static registry with entries is a `mcrs_minecraft_registries::static_registry!` enum of its
  entries, numbered by protocol id, with their locations in `ENTRIES`. In a
  `no_std` owner the enum has only the items of `static_registry_core!`, and
  the items of `static_registry!` when the owner's feature `std` is on. An
  owner that `OWNERS` marks as `NoStdEnum` depends on
  `mcrs_minecraft_static_registry` and declares the feature
  `std = ["mcrs_minecraft_static_registry/std"]`;
- a data registry with entries in `names.json` is a module of `ResourceKey`
  constants, except `minecraft:recipe` and `minecraft:advancement`.

`crates/mcrs_minecraft_keys` keys only the registries no crate owns, with
locations in place of typed keys. A registry with tags in `names.json` also
gets a tags module of `TagKey` constants, `<module>_tags` there and
`<module>_tag_keys` in an owner, so `block_tag_keys::LOGS` and
`item_tag_keys::LOGS` are two constants of two registries. A constant is the entry
path in upper case with `/`, `.` and `-` as `_`; the jukebox songs `5`, `11`
and `13` become `FIVE`, `ELEVEN` and `THIRTEEN`, and a `brigadier:` entry takes
the prefix `BRIGADIER_`. Any other digit-leading name, a namespace without a
rule and two names that make one constant stop the generator with the name.

`crates/mcrs_minecraft_registry_catalog`, manifest included, is generated as
well: `STATIC_REGISTRIES` lists every static registry with its `ENTRIES`. The
catalog depends only on the crates that own a static registry. An owner that
depends on the catalog cannot own a static registry. The catalog stays below
world generation: an owner in `OWN_LISTS` of `src/owners.rs` gets the
`STATIC_REGISTRIES` of its own static registries in `registry_keys`, and the
catalog does not list them.

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
`tools/captures.json`. The table in `tools/fixtures.json` is the one place that
states a fixture's name, task and destination; this tool and the tests read it
through `mcrs_minecraft_fixtures`. With `--all` every fixture is recaptured in
turn, a failure does not stop the others, and the exit status is non-zero if
any failed. An unknown name prints the known names. The update command itself
recaptures nothing.

`tools/captures.json` maps each fixture name to the id of the corpus version it
was captured at. A test in `mcrs_minecraft_fixtures` fails by name for every
fixture whose entry is not the id in `assets/minecraft/version.json`, for a
fixture missing from the file and for an entry that names no fixture. After an
update that test is the list of fixtures to recapture.

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

A new snapshot may also need a few compile fixes in the worldgen oracles of
`tools/vanilla-oracle` before the fixtures can be recaptured; see its README.

## Requirements and limits

- JDK 25 on the path of Gradle.
- Network access to Mojang's metadata and downloads, and to the Fabric Maven
  repository, from which Gradle fetches Fabric Loom.
- One run at a time. Two runs of the tool against one working tree are not
  supported, and Gradle invocations against `tools/vanilla-oracle` are never run
  in parallel.
