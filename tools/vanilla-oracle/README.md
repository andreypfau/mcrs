# vanilla-oracle

An oracle tool in the sense of `docs/clean-room.md`. It runs parts of the vanilla game in a Gradle
project and dumps their output as test fixtures. The Rust tests compare their results with these
fixtures. Code in `crates/` never copies from the oracle sources.

No task starts a server. The client capture starts a game client. Fabric Loom only puts the mapped
game jar on the class path: nothing is remapped, no mod is loaded and no mixin is applied.

## Regenerate a dump

The game version comes from `assets/minecraft/version.json`, which `build.gradle` reads when Gradle
configures the project. A new version reaches this project through `mcrs_minecraft_update`; see
`tools/mcrs_minecraft_update/README.md`.

The recapture command runs the Gradle task into a temporary directory, puts the files where the
tests read them and records the version of the capture in `tools/captures.json`:

```sh
cargo run -p mcrs_minecraft_update -- recapture <fixture name>
cargo run -p mcrs_minecraft_update -- recapture --all
```

`tools/fixtures.json` is the one table of fixture names, Gradle tasks and destinations. A task also
runs by hand with `-PoracleOut=<directory>`. Then nothing copies the files or updates the manifest.

Run Gradle with `--console=plain --no-daemon`. Never run two Gradle invocations against this
project at once. A dump is deterministic: a second run gives byte-identical files.

A new snapshot can need compile fixes in the oracles in `src/main`. The `data` source set
(`src/data`) compiles apart from them, so `dumpReports`, `dumpDefinitions` and `dumpGolden` keep
running while the oracles are fixed. `stubGen` prints the skeleton of a stub class for an interface.

## Binary dumps

All binary dumps are little-endian. They share these primitives: `i32`, `u32`, `i64`, `f32` (IEEE
754), `u8` and `str` (a `u32` byte length, then UTF-8 bytes). A file starts with an 8-byte ASCII
magic, a `u32` `format_version` and a `u32` `world_version`, the data version of the game that
wrote it. A file ends at its last value and has no trailer.

A block list is a palette of block state strings and a list of `(i32 x, i32 y, i32 z, u32 palette
index)`. A dump that records a random generator also records two `nextLong()` values taken after the
call. A comparison of these values pins the number of random draws.

| Fixture | Task | Magic | What it holds |
| --- | --- | --- | --- |
| `density` | `dumpOracle` | none | The density function roots of the overworld at seeds 1, 2 and 42, as f32 volumes. |
| `surface` | `dumpSurface` | `MCSURFC0` | The uncarved terrain of a chunk, as a palette and runs. |
| `feature_steps` | `dumpFeatureSteps` | `MCFSTEP0` | The feature order of each shipped biome source, and the feature set of each biome per step. |
| `ore_vein` | `dumpOreVeins` | `MCOREVN0` | The blocks of each ore vein case in write order, and the random state after it. |
| `tree_geometry` | `dumpTrees` | `MCTREEG0` | The blocks of each tree feature at four seeds, in first-write order, and the random state. |
| `templates` | `dumpTemplates` | `MCTMPLT0` | The size, palettes, block order and jigsaws of every structure template. |
| `structure_placement` | `dumpPlacement` | `MCPLACE0`, `MCSITES1` | Where structures are placed, and the jigsaw start sites. |
| `template_placement` | `dumpTemplatePlacement` | `MCTMPLP2` | What the placement of a template writes, with block entities and the random state. |
| `registry_census` | `dumpRegistryCensus` | `MCREGCE0` | The keys of the registries whose numeric ids cross the wire, in id order. |
| `structure_pieces` | `dumpStructurePieces` | `MCSTRPC0` | The pieces of each non-jigsaw structure start, as saved. |
| `structure_geometry` | `dumpStructureGeometry` | `MCSTRGE0` | What each chunk of a structure writes over a flat world. |
| `loot` | `dumpLoot` | `MCLOOTR0` | Loot table rolls, container fills, condition tests and number provider samples. |

The dumps that load a data pack do so as a server does. A dump of terrain runs with the carvers off,
so it holds the terrain that the Rust surface test compares against. The density volume index of a
lattice position is `y + (x + z * size_x) * size_y`, so a Rust buffer with the same order needs no
transposition. A volume has 5 by 49 by 5 corner lattice values per root and chunk, and one dense
volume of 16 by 384 by 16 values at seed 42.

The layout of a dump, the cases, what each case pins and what the fixture cannot pin are beside the
fixture in a `*capture_procedure.md` file. The surface dump has two search modes, `findBiomes` and
`findIcebergs`, that pick chunk coordinates and write nothing.

## Data tasks

These tasks live in the `data` source set, and `mcrs_minecraft_update` runs all of them.

```sh
cd tools/vanilla-oracle
./gradlew dumpReports -PreportsOut=<dir> --console=plain --no-daemon
./gradlew dumpDefinitions -PdefinitionsOut=<dir> --console=plain --no-daemon
./gradlew dumpCreativeTabs -PcreativeTabsOut=<file> --console=plain --no-daemon
./gradlew dumpGolden -Pgolden=<name> -PgoldenIn=<current file> -PgoldenOut=<dir> --console=plain --no-daemon
```

- `dumpReports` runs the data generator of the game. It writes `registries.json`, `packets.json`,
  `blocks.json` and `datapack.json` to `<dir>/reports`.
- `dumpDefinitions` loads the vanilla data pack and writes one definition file per block and per
  item to `<dir>/block_definition` and `<dir>/item_definition`. The first directory holds the
  `README.md` that describes the format. The task stops if the game reports another world version.
- `dumpCreativeTabs` loads the vanilla data pack and writes the creative report to `<file>`: the
  tab list and the search list of each items tab with each feature flag on, once with the operator
  view off to set `needs_operator` and once with it on for the lists.
- `dumpGolden` rewrites one golden file of the Rust tests with what the codecs of the game produce.
  The file in the repository supplies the cases, and the game supplies each expected value. The task
  fails without `-Pgolden` and `-PgoldenIn`. `recapture <name>` fills in the properties and puts the
  file back.

## Player dimensions

`dumpPlayerDimensions` writes `player_dimensions.json`: the width, the height, the eye height and
the fixed flag of `minecraft:player` for each pose, with `minecraft:scale` at 1 and at 2. No server
and no level run. `recapture player_dimensions` runs it.

## Sign text

`dumpSignText` runs the sign text renderers against a recording collector. It writes what they
submit: the transform from block space to text space for each sign family, side and state, the
position of each line, the colours and the light for each dye, and the style of each code point. The
font is a stub, so the file pins no glyph metrics.

## Client frames

The subproject `client-capture` is a client mod. It runs each scene of `tools/scenes/` in the
vanilla client and writes a PNG image and a record for each frame. For each scene it also writes
`<scene>.packets.json`, the entity packets that the client applied, and `<scene>.entity_ticks.json`,
the state of each entity after each entity update. `docs/spec/client-entities.md`, section 0.2,
states their keys.

```sh
./gradlew captureClientFrames -PoracleOut=/tmp/frames
./gradlew captureClientFrames -PoracleOut=/tmp/frames -Pscene=pig_walk
```

The task opens a game window. The fixture `client_frames` copies the output to
`crates/mcrs_minecraft_client/tests/fixtures/vanilla/frames/`.

## Client recordings

`recordInventoryE17` runs `tools/mcrs_minecraft_client_recorder/record.sh inventory_e17` with
`AUTOPLAY=true` and `CHEAT=remove:player_action`. It needs `MINECRAFT_EULA=true` in the
environment. The fixture `inventory_e17` copies the recordings to
`tools/mcrs_minecraft_client_recorder/recordings/inventory/`.

## Fonts

`dumpFont` writes `font.json` to `crates/mcrs_minecraft_client_gui/tests/fixtures/vanilla`. It loads
the font definitions without a window, from the jar and from the asset index. It fetches the font
objects into `build/font-assets` and checks each SHA-1 against the index. For each font
configuration the file lists the providers in search order and, for a set of code points, the
provider, the advance, the offsets of the bold strike and of the shadow, and the glyph box. It also
holds the widths of sample strings.

## Attributes

`dumpAttributes` writes `attributes.json` to `crates/mcrs_minecraft_entity/tests/fixtures/vanilla`.
It is experiment A1 of `docs/spec/attributes.md`. Each f64 in the file is a string of 16
hexadecimal digits: the bits of the value, the high bits first.

| Part | One row |
|------|---------|
| `game_version` | the name of the game version |
| `operations` | the save names of the operations, in the order of their wire ids |
| `default_attributes` | an entity type id, then the attribute and the base value for each attribute of the type |
| `value_cases` | a name, an attribute, a base value, the steps and the computed value |
| `order_cases` | an operation id, the steps and the modifier ids in the order of computation |

A step of `value_cases` is `["add", id, amount, operation id]` or `["remove", id]`. A step of
`order_cases` is a string: `id` adds a modifier, `-id` removes it, and `id|n` adds a modifier of the
operation `n` that the order does not list.

## Loot

`dumpLoot` rolls loot tables through the loot code of the game and writes, for each case, the input,
the full result and the state of the random generator after the call. The level answers only what a
roll asks: every position is loaded and holds the biome of the case, and no structure exists.
Contexts that need an entity are out of scope: no case sets `this_entity`, an attacker or a damage
source.

The generator kind 0 is the generator that the game makes from a loot seed. For each table roll and
each fill, the capture calls the game with the seed and with a generator that it made, and stops
unless the two results are equal. No case uses the seed 0, which the game reads as "no seed".

A kind 0 result is one roll as a caller receives it: a stack larger than its maximum size is already
divided. A kind 1 result is the whole fill: the roll, the division and the spread over the slots.
For the kinds 2 to 4, one context and one generator serve all samples in sequence. The kinds are: 0
roll of a table, 1 fill of a container, 2 condition test, 3 int provider sample and 4 float provider
sample. A stack is an item id, an `i32` count and the component patch as SNBT.
