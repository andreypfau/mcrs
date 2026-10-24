# Registries and tags: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |
| World version (`DataVersion`) | `5124` |
| Data pack format | `124.0` |

Source of these values: `assets/minecraft/version.json`.

This document states what a server, a client, a data pack author or a save file
can observe of the registry and tag system. It does not prescribe an
architecture. Every rule is stated over data: files, keys, numeric ids, orderings,
errors and wire contents.

## 0. Conventions

### 0.1 Terms

- **Identifier**: a `namespace:path` string. Also called a key when it names an entry, and a tag id when it names a tag.
- **Registry**: a named, ordered set of entries. Each entry has a key and a numeric id. A registry is named by an identifier, for example `minecraft:worldgen/biome`.
- **Registry path**: the path part of a registry's identifier, for example `worldgen/biome`. Every registry at this version is in the `minecraft` namespace.
- **Static registry**: a registry whose entries are fixed by the game. A data pack cannot add, remove or reorder its entries. It can define tags for it.
- **Data-pack registry**: a registry whose entries are loaded from data packs.
- **Pack stack**: the enabled data packs in priority order. The first pack has the lowest priority and the last has the highest.
- **Winning file**: for a given resource path, the file in the highest-priority pack that provides that path and is not hidden by a filter (REG-PACK-004).
- **Tag**: a named, ordered list of entries of one registry, defined by tag files.
- **Stage**: a group of data-pack registries that load together (REG-SET-003).
- **Known pack**: a pack that both sides can identify by a `(namespace, id, version)` triple. When a pack is known to both sides, the server can leave out the data of the entries that pack supplied.
- **Registry data packet**: the configuration packet `minecraft:registry_data`.
- **Update tags packet**: `minecraft:update_tags`, sent in both the configuration and play states.

### 0.2 Byte order of strings

Every identifier is ASCII (REG-ID-001). "Lexicographic" in this document means
comparing strings byte by byte, where a proper prefix sorts before every
extension of it. The character order that matters is:

`-` (0x2D) < `.` (0x2E) < `/` (0x2F) < `0`–`9` < `_` (0x5F) < `a`–`z`

### 0.3 Provenance and confidence

- `black_box_confirmed`: shown by a game-produced artefact that is cited by path.
- `source_suggested_black_box_confirmed`: taken from the reference behaviour, and consistent with an artefact cited by path.
- `source_suggested`: taken from the reference behaviour, with no artefact yet. Confidence is at most medium, and a concrete experiment is attached.
- `documentation`: publicly documented player-facing behaviour.
- `unresolved`: not decided. It is collected in section 11.

None of the experiments in this document has been run. Each one is a proposal.

### 0.4 Artefacts cited

- `assets/minecraft/`: the game's built-in data pack. It is the `data/minecraft` tree of the client jar, written at the root of `assets/minecraft/`. For example, a file the game reads at `data/minecraft/tags/block/logs.json` is stored here as `assets/minecraft/tags/block/logs.json`. The built-in feature packs are under `assets/minecraft/datapacks/<id>/`, each with its own `pack.mcmeta`. `trade_rebalance` also has a `data/` tree; `minecart_improvements` and `redstone_experiments` contain only `pack.mcmeta` (REG-PACK-001 covers a pack without `data/`). The corpus leaves out the entry files of five registries that the client jar does contain (REG-ENTRY-007): `worldgen/biome`, `worldgen/noise`, `worldgen/density_function`, `worldgen/noise_settings` and `worldgen/template_pool`. For those registries, use `names.json` for entry names; the contents are in the jar.
- The client jar: the game's own archive. It is cited only where the corpus leaves something out, by its internal path, for example `data/minecraft/worldgen/biome/plains.json`.
- `assets/mcrs/reports/registries.json`: the game's registry report. It lists every static registry, the numeric id of each entry, and the default entry where there is one.
- `assets/mcrs/reports/datapack.json`: the game's data pack structure report. For every registry it records `elements` (the registry is loaded from data packs), `tags` (it accepts tag files) and `stable`. For a data-pack registry, `stable: true` means it is reloadable (REG-SET-002). Every static registry is also marked `stable: true`; for them the flag carries no meaning here, since static registries have no entries to reload.
- `assets/mcrs/reports/packets.json`: the game's packet id report.
- `assets/mcrs/reports/names.json`: entry names and tag names per registry, listed from the client jar. Use it as test data only. It is not produced by the game.

### 0.5 ID prefixes

| Prefix | Subject |
|--------|---------|
| `REG-ID` | identifiers and their order |
| `REG-SET` | which registries exist, and how they are classified and staged |
| `REG-PACK` | data pack layout, the pack stack, overrides, filters |
| `REG-ENTRY` | entry files, keys, numeric ids |
| `REG-REF` | references between entries |
| `REG-ERR` | failure classification |
| `REG-TAG` | tag files, merging, resolution |
| `REG-NET` | what is sent to the client |
| `REG-RELOAD` | behaviour on `/reload` |
| `REG-SAVE` | what reaches disk |

### 0.6 Scope

In scope: which registries exist, how packs are stacked and filtered, how entry and
tag files become keys, numeric ids and tag members, how failures are classified and
reported, what the client receives, what a reload changes, and what reaches disk.

Out of scope, explicitly: the field schema of each individual registry. That covers
its fields, their types, defaults and numeric ranges, the NBT numeric type of each
field on the wire, and the exact shape of a registry's reduced network form
(REG-NET-005). This document states only the rules that are common to every registry
(REG-ENTRY-003) and which registries use a reduced network form. A per-registry
schema is a separate specification; none exists yet, and nothing here substitutes
for one.

### 0.7 Names that appear in this document

The document names data, never code. Names written in the game's own spelling are
allowed when they are data or protocol terms: NBT keys of the save (`DataVersion`,
`DataPacks`, `Enabled`, `Disabled`), protocol type names (`VarInt`), JSON field names
of pack files, packet names from `packets.json`, translation keys, and command-line
flags of the dedicated server (`--safeMode`).

---

## 1. Identifiers

### REG-ID-001: Identifier syntax (short form)

A namespace is a string over `[a-z0-9_.-]` and must not be exactly
`..`. A path is a string over `[a-z0-9_.-/]`. The textual form is
`namespace:path`, split at the first `:`. Where the text has no `:`, or the `:` is
its first character, the namespace is `minecraft`. A string with any other
character, including a second `:`, is not a valid identifier.

The following cases are decided:

| Text | Valid? | Identifier |
|------|--------|------------|
| `minecraft:` | yes | namespace `minecraft`, empty path |
| `:x` | yes | `minecraft:x` |
| `a:b:c` | no | the second `:` is not a path character |
| `.:x` | yes | namespace `.` |
| `..:x` | no | the namespace `..` is excluded |
| `t:a/./b`, `t:a/../b` | yes | `.` and `..` segments are ordinary path text and are not normalised |
| `t:A` | no | uppercase |

The identifier itself has no length limit. Where an identifier travels on the wire it
is a protocol string and so holds at most 32 767 characters (REG-NET-010).

Provenance: `documentation` for the character sets. `source_suggested` for the
table and the length rule. Confidence: medium. Experiment: in a pack, give an
enchantment `"exclusive_set": ":sharpness"` and another `"exclusive_set":
"a:b:c"`. The first loads as a reference to `minecraft:sharpness`; the second
fails the load as an invalid identifier.

### REG-ID-002: Total order of identifiers

- **Inputs:** two identifiers `a = (na, pa)` and `b = (nb, pb)`.
- **Behavior:** `a < b` if and only if `pa <lex pb`, or `pa = pb` and `na <lex nb`. The path is compared first and the namespace second. `<lex` is the byte order of §0.2.
- **Postconditions:** this is a strict total order on valid identifiers.
- **Edge cases:**

| a | b | Order | Reason |
|---|---|-------|--------|
| `z:a` | `a:b` | `z:a` first | paths `a` < `b`; the namespace is not consulted |
| `a:x` | `b:x` | `a:x` first | paths equal, so the namespaces decide |
| `m:a` | `m:a_b` | `m:a` first | prefix |
| `m:a/b` | `m:a_b` | `m:a/b` first | `/` < `_` |

- **Invariants:**
  - REG-ENTRY-004 applies this order to the file identifier of each entry, `(<R path>/<p>.json, namespace)`, not to the bare key `(p, namespace)`. The two orders differ when a path contains `-` or `.` (see the table there).
  - REG-ERR-002 orders the entries of an error report by this order applied to the bare entry key.
  - REG-TAG-007 uses this order applied to tag ids for its local tie-break.
- **Conformance tests:** a pack adds four damage type files, `a:b`, `z:a`, `a:a` and `b:a`, each missing its required `message_id`. The crash summary of the failed start (REG-ERR-002) lists them in the order `a:a`, `b:a`, `z:a`, `a:b`.

Provenance: `source_suggested`. Confidence: medium. Experiment: the conformance
test above, read from the crash summary of a vanilla dedicated server.

---

## 2. The set of registries

### REG-SET-001: Static registries and their numeric ids

- **Inputs:** none. This data is fixed by the game.
- **Relevant state:** `assets/mcrs/reports/registries.json`, which lists 95 registries. For each one it gives the registry's own `protocol_id`, an `entries` map from key to `protocol_id`, and, where present, a `default` key.
- **Behavior:**
  - The static registries are exactly the registries in `registries.json`. They are also exactly the registries with `"elements": false` in `datapack.json`.
  - The numeric id of an entry of a static registry is the `protocol_id` given by the report. For each registry these ids form the gapless range `0..n-1`.
  - The numeric ids do not depend on the enabled data packs, on the enabled features, or on the session.
- **Postconditions:** a data pack's contents cannot change any static registry's entry set or numeric ids.
- **Edge cases:**
  - An entry that is gated behind a disabled feature keeps its id. The report lists all entries.
  - A file placed under `data/<ns>/<static registry path>/` is not read as an entry (REG-PACK-001).
- **Invariants:** these ids appear on the wire wherever a static registry entry is encoded by id, and in the update tags packet (REG-NET-008).
- **Conformance tests:**
  - **Initial state:** the report.
  - **Input:** look up `minecraft:block` → `minecraft:air`.
  - **Expected:** the id the report gives. The registry's `default` is `minecraft:air`.
  - **Further check:** for every registry, the sorted ids equal `0..n-1`.

Provenance: `black_box_confirmed` (`assets/mcrs/reports/registries.json`;
`assets/mcrs/reports/datapack.json`, where the 95 registries with
`elements: false` are the same set). Confidence: high.

### REG-SET-002: Data-pack registries and their reloadability

- **Inputs:** `assets/mcrs/reports/datapack.json`.
- **Behavior:**
  - The data-pack registries are exactly the registries with `"elements": true`. At this version there are 60.
  - Every registry, static or data-pack, accepts tag files (`"tags": true`).
  - A data-pack registry with `"stable": true` is reloadable: `/reload` re-reads its entries (REG-RELOAD-001). There are 8: `advancement`, `context_float_provider`, `context_int_provider`, `item_modifier`, `loot_table`, `predicate`, `recipe`, `slot_source`.
  - The other 52 are read only when the server starts.
- **Local rule: registries that the world generation plugin declares.** This implementation declares the static registries of world generation in the world generation plugin. The plugin also declares the values of the data-pack registries that only world generation reads. Without the plugin, these static registries are not declared, and these data-pack registries hold the names of their entries only. A declaration of a plugin replaces the declaration of the same key that the implementation makes. The game declares every registry.
- **Edge cases:**
  - `minecraft:dimension` is a data-pack registry. The game ships no files for it, and its default entries come from world presets (see `names.json`). A data pack may still add files under `data/<ns>/dimension/`.
  - The `others` section of the report (`function`, `structure`) describes pack content that is not a registry. It is out of scope.
- **Invariants:** a new registry in a future version shows up in the report. Classify registries from the report, not from a hand-written list.
- **Conformance tests:**
  - **Input:** count the `elements: true` registries in `datapack.json`.
  - **Expected:** 60, of which 8 have `stable: true`.

Provenance: `black_box_confirmed` (`assets/mcrs/reports/datapack.json`).
Confidence: high.

### REG-SET-003: Load stages and reference visibility

- **Inputs:** the classification from REG-SET-002.
- **Behavior:** registries load in four stages, in this order:

| Stage | Registries |
|-------|-----------|
| 0 | tags of all static registries (no entries) |
| 1 (world) | every `stable: false` data-pack registry except `minecraft:dimension` |
| 2 (dimension) | `minecraft:dimension` |
| 3 (reloadable) | every `stable: true` data-pack registry |

  - An entry in stage *k* can reference entries and tags of registries in stages 0..*k* (REG-REF-002).
  - Within one stage there is no ordering between files or between registries. An entry can reference an entry of the same stage that is defined in any file, in any registry of that stage, including its own registry.
  - Static registry tags (stage 0) are complete before stage 1 starts. A stage 1 entry that references a static tag sees that tag fully resolved.
- **Ordering constraints:**
  - Stage 0 comes before stage 1. Otherwise an entry that references an item or block tag would find the tag missing, and loading would fail.
  - Stage 1 comes before stage 2, because a dimension references a dimension type and the world generation registries.
  - Stage 2 comes before stage 3, because the reloadable registries can reference anything below them.
- **Postconditions:** a failure in any stage aborts the whole load (REG-ERR-001).
- **Edge cases:** a reference from a stage *k* entry to a registry of a later stage is not resolved at load time. A field that holds only a key keeps it unchecked; a field that takes a reference or an inline value accepts only the inline form. REG-REF-002 lists the vanilla fields of each kind.
- **Conformance tests:**
  - **Initial state:** a pack defines a damage type `t:a` whose fields are valid.
  - **Input:** a second stage 1 file, an enchantment, references `#t:my_damage_tag`, and `t:my_damage_tag` is defined in `tags/damage_type/`.
  - **Expected:** loading succeeds, whatever the alphabetical position of the files.
  - **Failing twin:** the same pack without `tags/damage_type/my_damage_tag.json`. Expected: startup fails, and the report names `minecraft:enchantment` and the tag `t:my_damage_tag`. Without this case, a loader that never checks same-stage references would also pass the first one.
- **Experiment:** use one data pack with an enchantment `t:e` whose `supported_items` is `#t:my_items`, an item tag defined only in the same pack. The server starts. Then remove the item tag file. The server refuses to start, and the report names `minecraft:enchantment/t:e` and the missing tag.

Provenance: `source_suggested`. Confidence: medium.

### REG-SET-004: Registries that must not be empty

- **Inputs:** the entry set of each data-pack registry after loading.
- **Behavior:** if any of these registries has zero entries, the load fails:
  - `wolf_variant`, `wolf_sound_variant`
  - `pig_variant`, `pig_sound_variant`
  - `frog_variant`
  - `cat_variant`, `cat_sound_variant`
  - `cow_variant`, `cow_sound_variant`
  - `chicken_variant`, `chicken_sound_variant`
  - `zombie_nautilus_variant`
  - `painting_variant`

  The client enforces the same rule when it loads the registries it receives (REG-NET-007).
- **Ordering constraints:** this check runs only after every registry of the stage has loaded without an entry, reference or tag error. When such an error exists, the report holds those errors and not the emptiness error.
- **Edge cases:** a data pack can only empty one of these by hiding every file with a filter (REG-PACK-004).
- **Experiment:** add a pack whose `pack.mcmeta` filter blocks `{"path": "^(tags/)?painting_variant/"}`. Expected: startup fails, and the report names `minecraft:painting_variant` as a registry that must be non-empty. A filter of only `^painting_variant/` leaves `tags/painting_variant/placeable.json` naming 47 absent required entries; the report then shows those unresolved keys instead (see the ordering constraint).

Provenance: `source_suggested`. Confidence: medium.

### REG-SET-005: Registry-wide validation (short form)

After every entry of the registry loads, two registries are checked as a whole:

- `minecraft:timeline`: one world clock cannot have the same time-marker id defined by two timelines.
- `minecraft:worldgen/biome`: a check on the in-chunk placement range of placed features. It belongs to the world generation specification.

A failure is a load failure (REG-ERR-001).

Provenance: `source_suggested`. Confidence: medium. Experiment: copy a vanilla
timeline under a new key in a data pack, so that two timelines declare the same
marker on the same clock. Expected: startup fails, and the error names the
marker and the clock.

---

## 3. Data packs

### REG-PACK-001: File layout

- **Inputs:** a data pack: a directory or a zip that holds a `pack.mcmeta` (REG-PACK-005). The pack's content files are under `data/`.
- **Behavior:**

| Content | Path inside the pack | Identity |
|---------|----------------------|----------|
| entry of data-pack registry R | `data/<ns>/<registry path of R>/<p>.json` | key `<ns>:<p>` |
| tag of any registry R | `data/<ns>/tags/<registry path of R>/<p>.json` | tag `#<ns>:<p>` |

  - `<p>` may contain `/`. Subdirectories become path segments.
  - The registry path can itself contain `/`, for example `worldgen/biome`. The registry's namespace is not part of the directory, whatever it is: a registry `m:things` reads `data/<ns>/things/` and `data/<ns>/tags/things/`. Two registries whose keys share a path, such as `minecraft:things` and `m:things`, would read the same files; this implementation refuses such a set of declarations at load.
  - Files that do not end in `.json` are ignored.
  - A file is ignored, with a log line, when the namespace directory name or the file's relative path is not a valid identifier (REG-ID-001). One example is an uppercase letter. The line names the namespace and the path. A directory pack logs it as an error and a zip pack as a warning; either way loading continues.
  - The `.json` test is case-sensitive. A file ending in `.JSON` is not read; its name is also not a valid identifier, so it is ignored with the log line above.
  - Files under the directory of a static registry (`data/<ns>/block/…`) are not entries. Directories that match no registry are ignored.
  - The tag directory is the registry path in the singular form that the registry has, for example `tags/block`, `tags/item`, `tags/worldgen/biome`. A plural directory such as `tags/blocks` is not read.
  - `data/<ns>/tags/function/` holds function tags. Functions are not a registry (REG-SET-002); function tags belong to the function subsystem and are out of scope here.
- **Edge cases:**
  - Two registries whose directories nest do not exist at this version.
  - `data/<ns>/structure/` holds structure templates. It is not the `worldgen/structure` registry.
  - A pack with no `data/` directory is valid. It contributes no entry and no tag, takes its place in the stack (REG-PACK-002) and, if it has a known-pack identity, is still offered (REG-NET-002). Fixtures: `assets/minecraft/datapacks/minecart_improvements/` and `assets/minecraft/datapacks/redstone_experiments/` hold only `pack.mcmeta`.
  - A file named exactly `.json` directly in a registry directory gives the key `<ns>:` with an empty path, which is a valid identifier (REG-ID-001).
  - In a zip pack, entry names are not normalised, so a name with a `..` segment keeps it in the key. A directory pack cannot produce such a name.
- **Conformance tests:**
  - `assets/minecraft/loot_table/blocks/` holds entries keyed `minecraft:blocks/<name>`.
  - `assets/minecraft/tags/block/mineable/` holds tags `#minecraft:mineable/<name>`.
  - `assets/minecraft/worldgen/placed_feature/` is the registry `minecraft:worldgen/placed_feature`.

Provenance: `source_suggested_black_box_confirmed` (the vanilla data loads with
these paths, see REG-ENTRY-001). Confidence: high for the layout table. The log
lines, the `.JSON`, empty-path and zip `..` cases are `source_suggested`, confidence
medium. Experiment: a world pack directory with `data/t/damage_type/Bad.json` and
`data/t/damage_type/.json` (a copy of `arrow.json`), started once as a directory and
once zipped. Expected: `Bad.json` is skipped with a log line naming `t` and the path
(error level for the directory, warning for the zip), and `t:` appears in the
`minecraft:damage_type` registry data packet.

### REG-PACK-002: The pack stack

- **Inputs:** the `DataPacks` compound of `level.dat` (REG-SAVE-001), the available packs, the command line.
- **Relevant state:** the available packs:
  - `vanilla`: the built-in pack.
  - The built-in feature packs: their ids are the directory names under `assets/minecraft/datapacks/`, which are `minecart_improvements`, `redstone_experiments` and `trade_rebalance`.
  - World packs: each directory or zip in the world's `datapacks/` folder, with the id `file/<file name>`. The file name is kept whole, so a directory `a` is `file/a` and a zip `a.zip` is `file/a.zip`. A directory is a pack only if it holds a `pack.mcmeta` file; any file whose name ends in `.zip` is a candidate and is dropped later if its `pack.mcmeta` is missing (REG-PACK-005). Other entries of the folder are ignored with an informational log line.
  - There are no other kinds. The game's dedicated server has exactly these three pack sources; only world packs lack a known-pack identity (REG-NET-002). No pack is required or pinned to a position, so nothing reorders the stack after it is built.
- **Behavior:**
  - The pack stack is the `Enabled` list in its stored order, after removing ids that are not available. Each removal is logged as a missing data pack.
  - A pack's format range (REG-PACK-005) plays no part in building the stack on a dedicated server. A pack whose range excludes `124.0` is selected and loaded like any other.
  - Index 0 has the lowest priority.
  - An available pack is appended to the end of the stack, and the server logs that it was found, when all of these hold:
    - its id is in neither `Enabled` nor `Disabled`;
    - it is a world pack or `vanilla`, not a feature pack;
    - the features it requests are allowed.

    Packs that qualify are appended in ascending order of pack id.
  - A selected pack that requests a feature the world does not enable is removed from the stack, with a warning. A new pack that would be appended but requests such a feature is not appended, with an informational log line naming the missing features. When the world is being created, every feature is allowed.
  - Both feature checks apply only to packs not listed in `Disabled`.
  - If the stack ends up empty, it becomes `["vanilla"]`, with a log line.
  - With `--safeMode`, the stack is `["vanilla"]`.
  - Feature packs are enabled explicitly. A world created with a feature turned on enables the feature pack. When an existing world's `enabled_features` names a feature that no selected pack provides, every feature pack whose requested features are all in `enabled_features` is appended, with a log line.
  - The world's resulting enabled features are the union of the features requested by the packs in the stack and those already stored for the world.
- **Local rule: embedded packs.** This implementation lets a plugin ship a pack in its binary, under an id that the plugin gives. An embedded pack is available as a built-in pack and is enabled by default. In the stack, it is above `vanilla` and below every world pack, in the order that the plugins add the embedded packs. It has no known-pack identity, so its entries are always sent with their data (REG-ENTRY-006, REG-NET-005). Its files and built entries (REG-ENTRY-007) take part in override, filtering and ordering as the files of any other pack do. The game has no embedded packs.
- **Postconditions:**
  - The stack's order decides every override (REG-PACK-003) and every tag merge (REG-TAG-003).
  - The stack, its pack ids and each pack's known-pack identity are fixed for the server session as far as stage 1 and 2 registries are concerned (REG-RELOAD-001). A later `/datapack enable` or `disable` changes the selection recorded in `level.dat` and the stack used for the tags of every registry and for stage 3 entries (REG-RELOAD-001).
- **Conformance tests:**
  - **Initial state:** `Enabled: ["vanilla", "file/b"]`, and a new folder `a` appears.
  - **Expected:** the stack is `["vanilla", "file/b", "file/a"]`, and `file/a` overrides `file/b`.
  - **Second case:** `Enabled: ["vanilla", "file/a", "file/b"]`, with both packs defining `minecraft:damage_type/x.json`. Expected: `file/b`'s file wins.
  - **Zip case:** `Enabled: ["vanilla"]`, and the folder holds a directory `b` and a zip `a.zip`. Expected: the stack is `["vanilla", "file/a.zip", "file/b"]`.

Provenance: `source_suggested`. Confidence: medium. Experiment: start a world
with two world packs that each define `data/t/damage_type/x.json` with a
different `message_id`. Kill a player with `/damage` using `t:x`, and see which
death message prints. Then swap their order in `Enabled` and repeat.

### REG-PACK-003: Entry override

- **Inputs:** files at the same resource path in several packs of the stack.
- **Behavior:**
  - The entry with key K in registry R is decoded from the winning file for `data/<ns>/<R path>/<p>.json` and from no other file.
  - Fields are not merged across packs. An overriding file replaces the lower file completely.
  - The entry set of R is the union, over all packs, of the paths that are present and not hidden.
- **Postconditions:**
  - A data pack can add or replace entries. It cannot delete an entry except with a filter (REG-PACK-004).
  - The entry's source pack is the pack of the winning file. REG-NET-005 depends on this.
- **Edge cases:**
  - A higher-priority file that does not parse does not fall back to the lower file. The entry fails (REG-ERR-001).
  - Identical files in two packs still have exactly one source: the higher pack.
- **Conformance tests:**
  - **Fixture:** `assets/minecraft/datapacks/trade_rebalance/data/minecraft/loot_table/chests/ancient_city.json` replaces `assets/minecraft/loot_table/chests/ancient_city.json` when `trade_rebalance` sits above `vanilla`.
  - **Expected:** `minecraft:chests/ancient_city` has the contents of the feature pack's file only.
  - **Second fixture:** `villager_trade/librarian/2/book_emerald.json` exists in both.
- **Experiment:** enable `trade_rebalance` on a fresh world. Generate loot with `/loot spawn ~ ~ ~ loot minecraft:chests/ancient_city` many times. Only the pools of the feature pack's file appear.

Provenance: `source_suggested`. Confidence: medium.

### REG-PACK-004: Pack filters

- **Inputs:** an optional `filter` section in a pack's `pack.mcmeta`: `{"filter": {"block": [{"namespace": <regex>?, "path": <regex>?}, ...]}}`.
- **Relevant state:** the pack stack. The filter of pack P affects only packs below P.
- **Behavior:**
  - A missing `namespace` or `path` matches anything.
  - The patterns use the regular-expression syntax of the Java platform's standard library, with default flags. That syntax includes backreferences (`\1`), look-ahead and look-behind (`(?=…)`, `(?<=…)`), possessive quantifiers (`a*+`), atomic groups (`(?>…)`) and inline flags (`(?i)`). A conforming implementation accepts every pattern that syntax accepts and matches it the same way. A pattern it cannot evaluate faithfully must be treated as an invalid pattern (next rule), never matched approximately.
  - When any pattern of the section does not compile, or the `filter` section is malformed, the pack's whole filter is ignored: an error naming the pack is logged, the pack still loads, and it hides nothing. The section is read only when the stack's files are assembled, not when packs are discovered, so a broken filter never removes the pack from the available list.
  - A regex matches when it matches any substring. It is not anchored.
  - The path tested is the file path inside the namespace, including directories and the extension, for example `worldgen/biome/plains.json` or `tags/block/logs.json`.
  - A file at `(ns, path)` in a pack below P is hidden when both of these hold:
    - some block pattern's `namespace` matches `ns`;
    - some block pattern's `path` matches `path`.

    The two can be different patterns. For example, `[{"namespace":"a"},{"path":"x"}]` hides every file in every namespace, because the second pattern's namespace matches anything and the first pattern's path matches anything.
  - P's own files and the files of packs above P are never hidden by P's filter.
  - A hidden entry file does not exist for loading. If no visible file remains, the entry does not exist.
  - Hidden tag files contribute nothing to the tag (REG-TAG-003).
- **Edge cases:** a filter can empty a registry. That fails for the registries in REG-SET-004.
- **Conformance tests:**
  - **Initial state:** the stack is `[vanilla, file/a, file/b]`. `file/a` adds `data/t/damage_type/x.json` and `data/t/damage_type/y.json`. `file/b` has the filter `{"block":[{"namespace":"^t$","path":"^damage_type/x\\.json$"}]}` and ships its own `data/t/damage_type/z.json`.
  - **Expected:** `minecraft:damage_type` contains `t:y` and `t:z` but not `t:x`.
  - **Invalid pattern:** the same stack, with `file/b`'s filter `{"block":[{"path":"("}]}`. Expected: an error naming `file/b` is logged, and `minecraft:damage_type` contains `t:x`, `t:y` and `t:z`.
- **Experiment:** use a pack with filter `{"block":[{"path":"cat_variant/black"}]}`. The vanilla `cat_variant/all_black.json` is not hidden, because `cat_variant/black` does not occur in `cat_variant/all_black.json`. `cat_variant/black.json` is hidden. No vanilla tag names `minecraft:black` in `cat_variant` (there are no `cat_variant` tags at this version), so stage 1 loads. The advancement `minecraft:husbandry/complete_catalogue` names the cat variant `minecraft:black` in a criterion; that reference goes to an earlier stage, so the advancement fails to decode and startup fails, naming `minecraft:advancement` and `minecraft:husbandry/complete_catalogue` (REG-REF-002, REG-ERR-001). That is the first expected outcome. (`husbandry/whole_pack.json` also names `minecraft:black`, but as a wolf variant, which the filter does not touch.) Then add a second block entry `{"path":"advancement/husbandry/complete_catalogue"}` to the filter. The server starts; count the `minecraft:cat_variant` entries in a capture of the registry data packet: 10 instead of 11 (`names.json` lists 11). See OQ-07.
- **Experiment (dialect):** the stack `[vanilla, file/a, file/b]`; `file/a` adds `t:damage_type/arrow2.json` and `t:damage_type/x.json`; `file/b` has the filter `{"block":[{"namespace":"^t$","path":"^damage_type/(?!arrow)"}]}`. Expected: `t:arrow2` remains and `t:x` is hidden. A regex engine without look-ahead cannot express this filter.

Provenance: `source_suggested`. Confidence: medium (low for the cross-pattern
case; see OQ-07).

### REG-PACK-005: `pack.mcmeta`

- **Inputs:** the `pack.mcmeta` file at the root of a pack, parsed as strict JSON (REG-ENTRY-002). Its root must be an object.
- **Relevant state:** the current data pack format, `124.0` (`data_major`, `data_minor` of `assets/minecraft/version.json`).
- **Preconditions:** the pack was found as a directory holding `pack.mcmeta` or as a `.zip` file (REG-PACK-002).
- **Behavior:**
  - **`pack` section** (required):

| Field | Type | Required | Meaning |
|-------|------|----------|---------|
| `description` | text component | yes | shown in pack lists |
| `min_format` | integer `M`, or array `[M]` or `[M, m]` | with `max_format` | lowest supported format; a missing minor is 0 |
| `max_format` | same shapes | with `min_format` | highest supported format; a missing minor means every minor |
| `pack_format` | integer | no | if present, must lie within the majors of `[min_format, max_format]` |
| `supported_formats` | integer, `[min, max]` or `{"min_inclusive", "max_inclusive"}` | no | an error when the range lies above format 81 |

    The format fields are checked together: `min_format` and `max_format` must both be present or both absent; `min_format` must not exceed `max_format`; a pack whose range is above 81 must use `min_format`/`max_format` and not `supported_formats`; `pack_format` alone, above 81, is an error. A pack whose range reaches 81 or below also needs `supported_formats` and `pack_format`.
  - **Outcome of the `pack` section:**

| Situation | Outcome |
|-----------|---------|
| `pack.mcmeta` missing, directory pack | not a pack; informational log line, ignored |
| `pack.mcmeta` missing, zip pack | pack not available; warning naming the pack |
| not valid JSON, or root not an object | pack not available; warning naming the pack |
| valid JSON without `pack` | pack not available; warning naming the pack |
| `pack` present but its format fields break the rules above | pack available, with only its `description` read and its compatibility unknown; a warning is logged |
| `description` itself invalid | pack not available; warning naming the pack |

  - **Compatibility:** formats compare by major, then minor. A pack whose `max_format` is below `124.0` is too old; one whose `min_format` is above it is too new; otherwise it is compatible. A dedicated server does not act on this: an incompatible pack is listed, appended and loaded like a compatible one (REG-PACK-002).
  - **`features` section** (optional): `{"enabled": [<feature id>...]}`, the features the pack requests (REG-PACK-002). A feature id the game does not know makes the pack unavailable, with a warning naming the pack. A malformed section has the same outcome.
  - **`filter` section** (optional): REG-PACK-004.
  - **`overlays` section** (optional): `{"entries": [{"directory": <name>, "min_format": …, "max_format": …}, ...]}`, where `<name>` is over `[-_a-zA-Z0-9.]` and the format fields follow the `pack` rules (with `formats` in place of `supported_formats`, and no `pack_format`). Every entry whose range contains `124.0` is applied: its directory, at the pack root, is read as an additional root of the same pack, so its content files are under `<name>/data/`. Within the pack, the last applicable entry in the list has the highest priority, then the earlier applicable entries in reverse list order, then the base `data/` tree. An entry with no usable format information is skipped with a warning. A malformed section makes the pack unavailable. Metadata, including `filter`, is read only from the pack's own `pack.mcmeta`, never from an overlay.
  - Fields that the game does not name are ignored, in every section (REG-ENTRY-003).
- **Ordering constraints:** `pack.mcmeta` is read when packs are discovered, before the stack is built; the `filter` section is read later, when the stack's files are assembled (REG-PACK-004).
- **Numeric semantics:** a format is a pair (major, minor) of non-negative integers; a bare integer `M` is `(M, 0)` as a minimum and `(M, every minor)` as a maximum.
- **Postconditions:** a pack that is not available cannot be in the stack. If `level.dat` lists it in `Enabled`, it is dropped as a missing pack (REG-PACK-002).
- **Edge cases:**
  - The built-in `vanilla` pack has no `pack.mcmeta` of its own; it behaves as one that declares format `124.0` to `124.*`, the default features, and no filter or overlays.
  - The shipped feature packs declare `"min_format": 124`, `"max_format": 124` and one feature each (`assets/minecraft/datapacks/*/pack.mcmeta`).
- **Invariants:** whether a pack is available depends only on its own `pack.mcmeta`, never on other packs or on the world.
- **Conformance tests:**
  - A world pack with `{"pack":{"description":"x","min_format":[100],"max_format":[110]}}` is available and loads, although it is too old.
  - A world pack with `{"pack":{"description":"x","min_format":[124,0]}}` (no `max_format`) is available, and a warning is logged.
  - A world pack with `{"pack":{"description":"x","min_format":124,"max_format":124},"features":{"enabled":["t:none"]}}` is not available.
  - A world pack whose `overlays` lists `{"directory":"o","min_format":124,"max_format":124}` and that ships `o/data/t/damage_type/x.json` and `data/t/damage_type/x.json`: the entry `t:x` is decoded from the overlay's file.

Provenance: `source_suggested`, except the shipped feature pack fields
(`black_box_confirmed`, `assets/minecraft/datapacks/*/pack.mcmeta`). Confidence:
medium. Experiment: the conformance tests above on a vanilla dedicated server,
checking the server log and, for the overlay case, the `message_id` of `t:x` in a
registry data packet captured with an empty negotiated set.

---

## 4. Entries

### REG-ENTRY-001: Key from file path

- **Inputs:** a winning file at `data/<ns>/<R path>/<p>.json`.
- **Behavior:** the entry key is `<ns>:<p>`. `<p>` keeps its `/` separators and loses the `.json` suffix.
- **Conformance tests (shipped data):**
  - `assets/minecraft/advancement/adventure/arbalistic.json` names its parent as `minecraft:adventure/ol_betsy`, and `assets/minecraft/advancement/adventure/ol_betsy.json` exists.
  - Enchantments in `assets/minecraft/enchantment/` name `#minecraft:enchantable/armor`, which is the file `assets/minecraft/tags/item/enchantable/armor.json`.
  - The enchantment's tag reference must resolve for vanilla to load (REG-REF-002). The advancement's `parent` is a plain identifier, not a checked reference: an advancement whose parent does not exist still loads as an entry, but it and every advancement below it are left out of the advancement tree, and an error listing them is logged. The load does not fail.

Provenance: `source_suggested_black_box_confirmed`. Confidence: high for the key
rule; medium for the missing-parent outcome, which is `source_suggested`.
Experiment: a pack with `advancement/t/orphan.json` whose `parent` is `t:none`
and `advancement/t/child.json` whose parent is `t:orphan`. Expected: the server
starts, an error naming both is logged, and a client's advancement screen shows
neither, even after `/advancement grant @s only t:child`.

### REG-ENTRY-002: JSON strictness (short form)

Entry files and tag files are parsed as strict JSON. Each of the following makes
the file malformed:

- comments;
- single-quoted strings;
- unquoted names;
- trailing commas;
- non-finite numbers (`NaN`, `Infinity`);
- unquoted values, and the literals `true`, `false`, `null` in any other case (`TRUE`);
- an unescaped control character (U+0000 to U+001F) inside a string;
- `;` between members, or `=` in place of `:`;
- any content after the top-level value.

Bytes and text:

- The file is decoded as UTF-8. A byte sequence that is not valid UTF-8 is replaced by U+FFFD and is not an error by itself; the file then fails only if the replacement breaks the JSON or the field's value.
- One U+FEFF (byte-order mark) at the very start of the file is skipped. A U+FEFF anywhere else is a syntax error.
- An empty file, or one holding only whitespace, is not malformed JSON; it reads as an absent value, which then fails to decode as an entry (REG-ENTRY-003) or as a tag file (REG-TAG-004).

Provenance: `source_suggested`. Confidence: medium. Experiment: add `// x` to a
copy of a vanilla damage type in a pack. Expected: startup fails, naming that
entry. For tags, expected: the tag file is skipped (REG-TAG-004). Repeat with a
leading byte-order mark (expected: loads) and with a lone `0xFF` byte inside the
`message_id` string (expected: loads, and the registry data packet carries U+FFFD
in that string).

A repeated key inside one JSON object is not malformed; REG-ENTRY-008 states what
it means.

### REG-ENTRY-003: Field decoding (short form)

The schema of each registry, meaning its fields, types, defaults and numeric
ranges, is out of scope (§0.6). The following holds for every registry:

- A missing required field, a value of the wrong type, a value out of range, or a string that is not a valid identifier where one is required is an entry error (REG-ERR-001).
- Fields that the schema does not name are ignored, at any depth of the entry, in every data-pack registry. They are not an error, not a warning, and they do not reach the network form or any other output. This is the game's behaviour and the rule this specification adopts (OQ-05, closed).

Provenance: `source_suggested`. Confidence: medium. Experiment: in a pack, copy
`damage_type/arrow.json` to `t:arrow2` and add `"zzz": 1`. Expected: the server
starts, and the registry data packet contains `t:arrow2` without `zzz`. To cover
the decoder families, repeat with one entry each of `wolf_variant`,
`worldgen/placed_feature`, `dialog`, `test_instance`, `enchantment` and
`timeline`, adding `"zzz": 1` at the top level and inside one nested object.

### REG-ENTRY-004: Numeric ids of data-pack registries

- **Inputs:** the set of winning entry files of registry R.
- **Relevant state:** none. The ids do not depend on which pack supplied each file or on how loading is scheduled.
- **Behavior:** for an entry e with namespace `n(e)` and path `p(e)`, define `F(e) = <R path> + "/" + p(e) + ".json"`. The entries are numbered `0..n-1` in ascending order of the pair `(F(e), n(e))` under the order of REG-ID-002: path string first, then namespace, both byte-wise. All entries share the `<R path>/` prefix, so this is the same as ascending `(p(e) + ".json", n(e))`.

  This order is not the order of the bare keys. The `.json` suffix takes part in the comparison:

| Entry paths | Order | Why |
|-------------|-------|-----|
| `a`, `a_b` | `a`, `a_b` | `.` < `_` |
| `a`, `a/b` | `a`, `a/b` | `.` < `/` |
| `a`, `a-b` | `a-b`, `a` | `-` < `.` (unconfirmed) |
| `a`, `a.b` | `a.b`, `a` | `a.b.json` vs `a.json`: `b` < `j` (unconfirmed) |
| `a`, `a.k` | `a`, `a.k` | `j` < `k` (unconfirmed) |

  The three rows marked unconfirmed are the only cases where the suffix changes the order. No registry shipped at this version has such a pair: for every registry in `names.json`, sorting by `p` and by `p + ".json"` gives the same order. Those rows therefore rest on the reference behaviour alone until the experiment below is run, and an implementation must not be considered conforming on them before that.
| `z:x`, `a:y` | `z:x`, `a:y` | the path decides before the namespace |
| `a:x`, `b:x` | `a:x`, `b:x` | equal paths, so the namespace decides |

- **Postconditions:**
  - Adding, removing or filtering one entry shifts the ids of every entry after it.
  - The ids are recomputed each time the registry is loaded.
- **Invariants:**
  - The order of entries in the registry data packet is the id order (REG-NET-005).
  - Every reference by id on the wire uses these ids (REG-NET-006).
- **Conformance tests:**
  - **Initial state:** a stack with no pack other than `vanilla`, so `minecraft:dimension_type` holds `overworld`, `overworld_caves`, `the_end`, `the_nether` (`names.json`).
  - **Expected ids:** 0, 1, 2, 3.
  - **Input:** add `t:aaa` and `a:the_end` in a world pack (any other pack that adds entries to this registry changes the expected ids accordingly).
  - **Expected order:** `t:aaa`, `minecraft:overworld`, `minecraft:overworld_caves`, `a:the_end`, `minecraft:the_end`, `minecraft:the_nether`.
- **Experiment:** use a pack that adds `painting_variant` entries `t:a`, `t:a-b`, `t:a.b` and `u:a`. Capture the registry data packet for `minecraft:painting_variant`. The relative order must be `t:a-b`, `t:a.b`, `t:a`, `u:a`. Also check that a painting with variant `t:a` placed in the world arrives in entity data with the index of `t:a` in that packet.

Provenance: `source_suggested`. Confidence: medium.

### REG-ENTRY-005: Entry set (short form)

The entries of a data-pack registry are exactly the keys of its winning files.
The game adds none from code at this version; this includes the world generation
registries whose files the corpus omits (REG-ENTRY-007). `minecraft:dimension` is the one
exception: the dimensions of the world's settings join it, and its specification
belongs to the world settings.

Provenance: `source_suggested`. Confidence: medium. Experiment: start with
`--safeMode` and compare the entries of every synced registry in the registry
data packet with the files of the vanilla pack, or with `names.json` for the
registries whose files the corpus omits.

### REG-ENTRY-006: Known-pack attribution (short form)

An entry whose winning file comes from a pack with a known-pack identity
(REG-NET-002) is attributed to that identity. An entry whose winning file comes
from any other pack, including every `file/…` pack, has no attribution. Only the
winning file counts. The lower packs that also had the path do not.

Provenance: `source_suggested`. Confidence: medium. The experiment is under REG-NET-005.

### REG-ENTRY-007: Where the vanilla world generation entries come from

- **Inputs:** the `vanilla` pack.
- **Relevant state:** the client jar's `data/minecraft/` tree.
- **Preconditions:** `vanilla` is in the stack (REG-PACK-002).
- **Behavior:** the vanilla entries of `worldgen/biome`, `worldgen/noise`, `worldgen/density_function`, `worldgen/noise_settings` and `worldgen/template_pool` are ordinary files of the built-in `vanilla` pack, at `data/minecraft/<R path>/<p>.json`, like the entries of every other data-pack registry. The game does not create them from code at run time. They therefore take part in ordering (REG-ENTRY-004), override (REG-PACK-003), filtering (REG-PACK-004) and known-pack attribution (REG-ENTRY-006) exactly as any other file of the `vanilla` pack.
- **Ordering constraints:** none of their own; they are numbered with every other entry of their registry (REG-ENTRY-004).
- **Numeric semantics:** none.
- **Postconditions:** a world pack file at the same path overrides one of them; a filter in a higher pack can hide one.
- **Edge cases:** of these five, only `worldgen/biome` is synced (REG-NET-004). A vanilla client fills an absent-data biome from the same file in its own copy of the `vanilla` pack (REG-NET-007).
- **Local rule: entries built in code.** This implementation can build an entry in code instead of reading its file. A built entry belongs to one pack and is at the path of its file in that pack, `data/<namespace>/<R path>/<path>.json`. It takes part in ordering, override, filtering and known-pack attribution as a file at that path does. It replaces a file at the same path in the same pack, and the implementation does not read that file. An entry built for a pack that is not in the stack does not exist. The world generation plugin adds the built entries; without it, the files give the entries. The game has no such entries; the observable result must equal the one the files give.
- **Invariants:** the counts below equal the entry names `names.json` lists for each registry.
- **Conformance tests:** on a stack with no pack other than `vanilla`, the entry counts are:

| Registry | Entries |
|----------|---------|
| `minecraft:worldgen/biome` | 68 |
| `minecraft:worldgen/noise` | 65 |
| `minecraft:worldgen/density_function` | 55 |
| `minecraft:worldgen/noise_settings` | 7 |
| `minecraft:worldgen/template_pool` | 245 |

  and every one of them is attributed to `(minecraft, core, 26.4-snapshot-4)`.

Provenance: `black_box_confirmed` for the files and counts (the client jar,
`data/minecraft/worldgen/{biome,noise,density_function,noise_settings,template_pool}/`,
68, 65, 55, 7 and 245 `.json` files; the same counts are in
`assets/mcrs/reports/names.json`). `source_suggested` for "not created from code at
run time". Confidence: high for the files, medium for the rest. Experiment: start a
vanilla server with a world pack that holds `data/minecraft/worldgen/biome/plains.json`
with a changed `temperature`, and capture the registry data packet with an empty
negotiated set: `minecraft:plains` carries the pack's value.

### REG-ENTRY-008: Repeated keys in one JSON object

- **Inputs:** an entry file, a tag file or a `pack.mcmeta` in which one JSON object names the same key more than once.
- **Relevant state:** none.
- **Preconditions:** the file is otherwise valid JSON (REG-ENTRY-002).
- **Behavior:** the file is not malformed. The object holds the key once, with the value of its last occurrence. Where the order of an object's keys is observable (for example a map whose keys are entry or criterion names), the key keeps the position of its first occurrence.
- **Ordering constraints:** only the textual order of the occurrences within the one object matters.
- **Numeric semantics:** none; the surviving value is decoded as if it were the only one.
- **Postconditions:** the earlier values are discarded before decoding, so they cannot cause an error even when they are invalid.
- **Edge cases:** the rule applies at every depth and in every registry alike, independently of whether the object is decoded as a record of named fields or as a map.
- **Invariants:** decoding a file with repeated keys gives the same value as decoding the file with every earlier occurrence removed.
- **Conformance tests:**
  - A damage type file `{"message_id": "a", "exhaustion": "bad", "scaling": "never", "exhaustion": 5.0}` loads, with `exhaustion` 5.0.
  - A tag file `{"values": ["minecraft:stone"], "values": ["minecraft:dirt"]}` gives `[dirt]`.

Provenance: `source_suggested`. Confidence: medium. Experiment: OQ-01's experiment,
now with the expected outcome `5.0`, plus the conformance cases above.

---

## 5. References between entries

### REG-REF-001: Reference forms

- **Inputs:** a field of an entry that names other entries. Which form a field accepts is fixed by that field's schema in the game.
- **Behavior:** a referencing field takes one of these forms:

| Form | JSON | Meaning |
|------|------|---------|
| key reference | `"ns:path"` | the entry with that key |
| inline value | `{ ... }` | an anonymous value, only where the field allows inline values |
| entry set, tag | `"#ns:path"` | the tag's members, as resolved |
| entry set, single | `"ns:path"` | a one-element set |
| entry set, list | `["ns:a", "ns:b", ...]` | those entries, in order. Where inline values are allowed, list items may be inline objects. A list item cannot be a `#tag`. |

  A field that does not allow inline values rejects an object, and that is an entry error.
- **Postconditions:**
  - An inline value has no key and no numeric id. It cannot be a tag member and cannot be referenced from anywhere else.
  - On the wire, an inline value is encoded in place and a reference is encoded as its key (REG-NET-005).
- **Conformance tests (shipped data):**
  - Key references: `assets/minecraft/worldgen/placed_feature/*.json` (`"feature": "minecraft:…"`).
  - Inline values: `assets/minecraft/worldgen/feature/forest_flowers.json`. Its `features` list holds an inline placed feature with an inline feature.
  - Tag sets: `assets/minecraft/enchantment/*.json` (`"supported_items": "#minecraft:enchantable/…"`) and `assets/minecraft/worldgen/structure/*.json` (`"biomes": "#minecraft:has_structure/…"`).

Provenance: `source_suggested_black_box_confirmed`. Confidence: high.

### REG-REF-002: Reference resolution

- **Inputs:** a key reference or tag reference from an entry in stage *k* to registry T.
- **Behavior:**

| Target registry T | Key or tag present | Key absent | Tag absent or failed |
|-------------------|--------------------|------------|----------------------|
| static, or an earlier stage | resolved | the entry fails to decode, and the load fails | the entry fails to decode, and the load fails |
| same stage *k* | resolved, from any file of the stage | the load fails, reported as unresolved keys of T | the load fails, reported as unresolved tags of T |
| later stage, field that holds only a key | not resolved at load time: the field keeps the key unchecked | loads; the key is looked up when used | n/a |
| later stage, field that takes a reference or an inline value | a key or tag reference fails to decode, because the later registry does not exist yet, and the load fails; an inline value loads | the same | the same |

  A tag that failed to resolve (REG-TAG-006) counts as absent.
  - Which kind a field is belongs to that field's schema (§0.6). The cross-stage fields of the vanilla stage 1 registries at this version are:

| Registry (stage 1) | Field | Target (stage 3) | Kind |
|--------------------|-------|------------------|------|
| `trial_spawner` | `loot_tables_to_eject[].data` | `loot_table` | key only |
| `trial_spawner` | `items_to_drop_when_ominous` | `loot_table` | key only |
| `trial_spawner` | `spawn_potentials[].data.equipment.loot_table` | `loot_table` | key only |
| `block_transformer` | `loot` | `loot_table` | key only |
| `worldgen/processor_list` | `loot_table` of an `append_loot` block entity modifier | `loot_table` | key only |
| any registry whose values carry item components | `loot_table` of a `minecraft:container_loot` component | `loot_table` | key only |
| `villager_trade` | `merchant_predicate` | `predicate` | reference or inline |
| `villager_trade` | `given_item_modifier` | `item_modifier` | reference or inline |
| `villager_trade` | `max_uses`, `xp`, `wants.count`, `additional_wants.count` | `context_int_provider` | reference or inline |
| `villager_trade` | `reputation_discount` | `context_float_provider` | reference or inline |
| `trade_set` | `amount` | `context_int_provider` | reference or inline |
| `enchantment` | `requirements` of each conditional effect in `effects`, including `post_attack` and `equipment_drops` | `predicate` | reference or inline |
| `worldgen/template_pool` | `processors` of a `single_pool_element` or `legacy_single_pool_element`, when it is an inline processor list with an `append_loot` block entity modifier | `loot_table` | key only |
| `worldgen/configured_feature` | `processors` of a `template` feature, when it is an inline processor list with an `append_loot` block entity modifier | `loot_table` | key only |
| `worldgen/configured_feature` | `fossil_processors` and `overlay_processors` of a `fossil` feature, when it is an inline processor list with an `append_loot` block entity modifier | `loot_table` | key only |
| any registry whose values carry item components | `recipes` component, each list item | `recipe` | key only |
| any registry whose values carry item components | `layers` of a `compostable` component | `context_int_provider` | key only; a number is also accepted |
| any registry whose values carry item components | `burn_time` of a `cooking_fuel` component | `context_int_provider` | key only; a number is also accepted |
| any registry whose values carry item components | `speed_multiplier` of a `cooking_fuel` component | `context_float_provider` | key only; a number is also accepted |
| any registry whose values carry item components | `uses` of a `brewing_fuel` component | `context_int_provider` | key only; a number is also accepted |
| any registry whose values carry item components | `speed_multiplier` of a `brewing_fuel` component | `context_float_provider` | key only; a number is also accepted |

  - A key-only field naming a loot table that does not exist behaves, when used, as an empty loot table.
  - The rule continues inside an inline value. An inline loot condition can hold more loot conditions, for example `terms` of `all_of` and `any_of`, and `term` of `inverted`. Each nested condition is also reference or inline, so a key in it also fails the load.
  - An inline `set_loot_table` function in an inline item modifier accepts only a reference. In a stage 1 entry, the key fails and an inline table is rejected. Thus such a function never loads in stage 1.
  - The processor list rows apply wherever a processor list is inline: in a template pool, in a feature, and in an inline feature or pool inside a biome, a placed feature or a structure.
  - The `block_transformer` item component can hold an inline block transformer. Thus the `loot` row of `block_transformer` also applies inside item components.
  - Fields that hold raw NBT are not references. Examples are the `custom_data`, `entity_data`, `block_entity_data` and `bucket_entity_data` components, and `spawn_potentials[].data.entity`. The load does not check them, even when they contain a loot table id. The game reads such an id only at use time.
  - No stage 1 field references `advancement` or `slot_source`.
  - The table comes from a search across all fields, not from a check of each feature type by hand. A rare feature type with an inline processor list can be missing from it. The field names `fossil_processors` and `overlay_processors` are not confirmed by a second check.
- **Ordering constraints:** same-stage references are checked only after every entry and tag of the stage has loaded. The order of files does not matter. Reordering them never changes success or failure.
- **Postconditions:** after a successful load, every same-stage or earlier-stage reference names an existing entry or tag.
- **Edge cases:**
  - An entry can reference its own registry, for example an enchantment's exclusive set naming enchantments.
  - Cycles among entries are not rejected at load time (see OQ-08).
  - An optional tag member that is absent is not a reference failure (REG-TAG-006).
- **Conformance tests:**
  - **Input:** a pack that adds `enchantment/t.json` with `"exclusive_set": "#t:none"` and no such tag.
  - **Expected:** startup fails, naming `minecraft:enchantment` and the tag `t:none`.
  - **Input:** the same pack, with `"supported_items": "#t:none"` (an item tag, so stage 0).
  - **Expected:** startup fails, naming the entry `t:t`.
  - **Input:** a pack that copies `trial_spawner/trial_chamber/breeze/ominous.json` to `t:s` and sets the first `loot_tables_to_eject` item's `data` to `t:no_such_table`, with no such loot table.
  - **Expected:** the server starts.
  - **Input:** a pack that adds a `villager_trade` entry whose `merchant_predicate` is a key string such as `"t:p"`, whether or not a predicate file `t:p` exists.
  - **Expected:** startup fails, naming that entry; the same predicate written inline loads.
  - **Input:** a pack that adds `enchantment/t.json` with `"requirements": "t:p"` in one conditional effect.
  - **Expected:** startup fails, naming that entry. The same condition written inline loads.
  - **Input:** a pack that adds a dialog with an item body whose item has `"minecraft:recipes": ["t:no_such_recipe"]`.
  - **Expected:** the server starts.

Provenance: `source_suggested`. Confidence: medium. Experiment: as in the
conformance tests. Compare the server log for each case. For the trial spawner,
also run an ominous trial spawner using `t:s` and check that the ejections that
pick `t:no_such_table` produce nothing.

---

## 6. Failures

### REG-ERR-001: Failure classification

- **Inputs:** the pack stack at startup, or on `/reload`.
- **Behavior:**

| Situation | At server start | On `/reload` |
|-----------|-----------------|--------------|
| file or namespace name is not a valid identifier | file ignored, logged (REG-PACK-001) | same |
| pack unavailable because of its `pack.mcmeta` (REG-PACK-005) | pack left out, warning logged | same |
| pack filter malformed or a pattern does not compile (REG-PACK-004) | filter ignored, error logged | same |
| entry file is malformed JSON (REG-ENTRY-002) | the load fails | stages 1–2: the file is not re-read, so there is no effect. Stage 3: the reload fails |
| entry fails to decode (REG-ENTRY-003) | the load fails | as above |
| unresolved reference (REG-REF-002) | the load fails | as above |
| empty registry from REG-SET-004 | the load fails | n/a (stage 1) |
| registry-wide validation fails (REG-SET-005) | the load fails | n/a (stage 1) |
| fatal loot validation problem in stage 3 (below) | the load fails | the reload fails |
| non-fatal loot validation problem (below) | warning logged | warning logged |
| advancement criterion validation problem | warning logged | warning logged |
| advancement whose `parent` does not exist (REG-ENTRY-001) | advancement left out of the tree, error logged | same |
| tag file malformed or schema-invalid | that file is skipped, error logged | same |
| tag has a missing required member | see REG-TAG-006 | see REG-RELOAD-002 |

  - "The load fails" means that the server does not start (REG-ERR-003). No partial registry set is used.
  - "The reload fails" means that the server keeps all previous data (REG-ERR-004).
  - A stage 3 entry that fails to decode is not skipped on its own: one broken loot table, recipe or advancement file fails the whole load or reload.
  - **Loot validation.** After stage 3 loads, every loot table, predicate, item modifier, slot source and context number provider is checked, following references into the others and into tags. Every problem found is logged as a warning naming the path to the problem (the root entry, then each followed reference or tag, field and index). The problems are of two classes:

| Problem | Class |
|---------|-------|
| an entry refers, directly or through other entries, back to itself | fatal |
| a tag refers, directly or through other tags or entries, back to an entry being checked | fatal |
| a part reads a context value that the context it is used in does not provide (for example a block-state condition inside an entity loot table) | non-fatal |
| a list has fewer elements than the field requires | non-fatal |
| an alternative of an `alternatives` entry can never be reached, because an earlier alternative always matches | non-fatal |
| a composite loot entry has no children | non-fatal |

    If at least one fatal problem was found, the load or reload fails after every problem has been logged. A reference to a loot table, predicate or item modifier that does not exist is not a validation problem; it is an unresolved reference (row above) and fails the load first.
- **Ordering constraints:**
  - Every entry error in a stage is collected before the stage fails, so one start reports all bad entries of that stage, not only the first.
  - Registry-wide checks (REG-SET-004, REG-SET-005) run only when the stage has no entry, reference or tag error; a report therefore holds errors of one kind or the other, never both.
- **Conformance tests:**
  - Use two bad damage type files in one pack. One start reports both.
  - A pack with `loot_table/t/a.json` whose only pool entry is `{"type": "minecraft:loot_table", "value": "t:a"}`. Expected: a warning names the recursion and startup fails.
  - A pack with an entity loot table `t:b` using the condition `minecraft:block_state_property`. Expected: a warning, and the server starts.

Provenance: `source_suggested`. Confidence: medium. Experiment: as in the
conformance tests, plus one case for each row.

### REG-ERR-002: Error report content (short form)

A failed load logs each error under its registry id and entry key. The errors are
grouped by registry; the groups are in ascending order of registry id and, within
a group, the errors are in ascending order of entry key, both under the order of
REG-ID-002 (path first, then namespace, byte-wise). The file of an entry plays no
part in the order. The crash summary lists the same errors, sorted by the same
pair, as `<registry>/<entry key>: <message>`. An entry has at most one error in a
report. An error about a whole registry (an empty registry under REG-SET-004,
entries referenced but never defined under REG-REF-002) is listed under the
registry `minecraft:root` with the failing registry's id as its key. Parse errors
name the source pack id. The exact wording is not part of this specification.

Provenance: `source_suggested`. Confidence: medium. The experiment is under
REG-ERR-001; the order is checked by REG-ID-002's conformance test.

### REG-ERR-003: Startup failure (short form)

When the load fails at startup, a dedicated server stops. Its log says that data
packs failed to load, and suggests fixing them or starting with `--safeMode`.
`--safeMode` loads only the `vanilla` pack (REG-PACK-002).

Provenance: `documentation`. Confidence: medium.

### REG-ERR-004: Reload failure (short form)

When `/reload`, `/datapack enable` or `/datapack disable` fails, the command
source receives the failure message `commands.reload.failure` ("Reload failed;
keeping old data"). Every entry, tag and the pack selection stay as they were.

Provenance: `documentation`. Confidence: medium. Experiment: break a loot table
in an enabled pack and run `/reload`. The message appears, and
`/loot spawn … loot <that table>` still uses the old table.

### REG-ERR-005: Diagnostics of a successful load

- **Inputs:** a load at startup or a reload that succeeds.
- **Relevant state:** the pack stack and its files.
- **Preconditions:** none.
- **Behavior:** a successful load can still produce errors and warnings. They are logged; none of them stops the load. Each names the following:

| Diagnostic | Level | Names |
|------------|-------|-------|
| a pack id in `Enabled` that is not available (REG-PACK-002) | warning | the pack id |
| a pack left out because of its `pack.mcmeta` (REG-PACK-005) | warning | the pack id |
| a selected pack removed for a feature the world does not enable | warning | the pack id and the features |
| a new pack found and appended, or not appended for missing features | informational | the pack id (and the features) |
| an empty stack replaced by `["vanilla"]` | informational | — |
| a pack filter ignored (REG-PACK-004) | error | the pack id |
| a file ignored for an invalid name (REG-PACK-001) | error (directory pack) or warning (zip pack) | the namespace and the path |
| a tag file skipped (REG-TAG-004) | error | the tag id, the file and the pack id |
| a tag that failed (REG-TAG-006) | error | the tag id and every missing member, each with the pack id of the file that listed it |
| a loot validation problem, non-fatal (REG-ERR-001) | warning | the path from the root entry to the problem, and the problem |
| an advancement criterion problem | warning | the advancement key and the problems |
| advancements left out of the tree (REG-ENTRY-001) | error | every advancement left out |

  A failed load logs the same kinds of diagnostic that were produced before it failed, and then the errors of REG-ERR-002.
  Override is silent: no diagnostic names an overridden entry, the pack whose file won or the lower packs it replaced (REG-PACK-003).
- **Ordering constraints:** diagnostics follow the order of the work: building the pack stack first, then stage 0 tags, stage 1, stage 2 and stage 3, and in each stage the tag file diagnostics before the tag resolution diagnostics, and loot and advancement diagnostics last. Within one of these phases the game gives no fixed order, because files are read in parallel; a conforming implementation may use any order within a phase, and should use a stable one.
- **Postconditions:** the set of diagnostics is a function of the pack stack and its files; loading the same input twice gives the same set.
- **Numeric semantics:** none.
- **Edge cases:** a diagnostic about a pack that is not in the stack (a missing or unavailable pack) is produced before any file is read.
- **Invariants:** a diagnostic of this table never makes the load fail, at startup or on reload.
- **Conformance tests:** a world pack with `data/minecraft/tags/block/bad.json` = `{"values": [3]}` and `data/minecraft/tags/block/t.json` = `{"values": ["t:nope"]}`. Expected: the server starts; one error names the tag `minecraft:bad`, its file and the pack id, and one error names the tag `minecraft:t`, the missing member `t:nope` and the pack id. The exact wording is not part of this specification.

Provenance: `source_suggested`. Confidence: medium. Experiment: the conformance
test above, plus one world pack for each row, reading the server log.

---

## 7. Tags

### REG-TAG-001: Tag file schema

- **Inputs:** a file at `data/<ns>/tags/<R path>/<p>.json`.
- **Behavior:**
  - The file is a JSON object with:
    - `values` (required): an array of member entries;
    - `replace` (optional boolean, default `false`).
  - Each member entry is one of:

| JSON | Kind | Required? |
|------|------|-----------|
| `"ns:path"` | element | yes |
| `"#ns:path"` | tag | yes |
| `{"id": "ns:path"}` or `{"id": "#ns:path"}` | element or tag | yes (`required` defaults to `true`) |
| `{"id": …, "required": false}` | element or tag | no (optional) |

  - A namespace left out of `id` defaults to `minecraft`.
  - Fields other than `values` and `replace` are ignored. Inside a member object, fields other than `id` and `required` are ignored too: `{"id": "minecraft:stone", "zzz": 1}` is a required element member.
  - A repeated key follows REG-ENTRY-008.
- **Edge cases:**
  - `"values": []` is valid. Fixture: `assets/minecraft/tags/damage_type/bypasses_cooldown.json`.
  - A missing `values`, or a member that is not a valid identifier, makes the whole file invalid (REG-TAG-004).
- **Conformance tests (shipped data):**
  - Element and tag members: `assets/minecraft/tags/block/logs.json` (three `#` members).
  - A `replace` file: `assets/minecraft/datapacks/trade_rebalance/data/minecraft/tags/villager_trade/librarian/level_1.json`.

Provenance: `source_suggested_black_box_confirmed`. Confidence: high.

### REG-TAG-002: Tag identity (short form)

A tag is identified by its registry and its id. The id `<ns>:<p>` comes from the
file path as in REG-ENTRY-001. Tags with the same id in different registries are
unrelated. Every registry, static or data-pack, can have tags.

Provenance: `source_suggested_black_box_confirmed` (`assets/minecraft/tags/` has
tags for static registries such as `block` and for data-pack registries such as
`damage_type`). Confidence: high.

### REG-TAG-003: Merging tag files across packs

- **Inputs:** the visible files `f_1 … f_k` for tag T, in pack stack order (lowest first), excluding files hidden by filters and files rejected by REG-TAG-004.
- **Behavior:**
  - Let `r` be the largest index for which `f_r.replace = true`. If no file has `replace`, let `r = 1`.
  - The effective member list of T is the concatenation `values(f_r) ++ values(f_{r+1}) ++ … ++ values(f_k)`, with each file's `values` kept in array order.
  - `replace: true` discards only the contributions of lower packs. Packs above a replacing file still append.
- **Ordering constraints:** pack order is part of the result. Swapping two non-replacing packs swaps the order of their contributions in the resolved tag (REG-TAG-005), and that order is visible in the update tags packet.
- **Postconditions:** T exists when at least one visible file for it is valid JSON, whether or not that file is a valid tag file (REG-TAG-004; OQ-03 records the experiment that confirms this).
- **Conformance tests:**
  - **Fixture:** with `trade_rebalance` above `vanilla`:
    - `#minecraft:librarian/level_1` in `villager_trade` is exactly the nine members of the feature pack's file, because that file sets `replace`.
    - `#minecraft:trades/desert_common` in `enchantment` is the feature pack's list with nothing removed, because that file does not set `replace`. The vanilla pack has no file for this tag.
  - **Synthetic:** the stack is `[P1: values [a], P2: values [b] replace, P3: values [c]]`. Expected: `[b, c]`.
  - The shipped fixtures do not exercise appending: every tag that both `vanilla` and `trade_rebalance` define is a `replace` file in `trade_rebalance`, and `trades/desert_common` has no vanilla file, so "nothing removed" holds trivially. The synthetic case is the test of appending.

Provenance: `source_suggested`. Confidence: medium. Experiment: three world packs
each define `tags/block/t.json`, as in the synthetic case, using blocks `a`, `b`,
`c`. Capture the update tags packet. `#minecraft:t` lists the ids of `b` and `c`
in that order. `/execute if block <pos of a> #minecraft:t` fails.

### REG-TAG-004: Invalid tag files

- **Behavior:**
  - A tag file that is malformed JSON, or valid JSON that breaks REG-TAG-001, is skipped. An error naming the tag, the file and the pack is logged.
  - The skipped file contributes nothing. If it had `replace: true`, it does not discard the lower contributions.
  - The load continues.
- **Edge cases:**
  - A tag whose only visible file is valid JSON but schema-invalid exists, as an empty tag, and is sent with zero members (REG-TAG-008). A tag whose only visible file is malformed JSON does not exist. This is `source_suggested`; OQ-03 holds the experiment.
- **Conformance tests:**
  - **Initial state:** pack P1 has `tags/block/t.json` `{"values":["minecraft:stone"]}`. P2, above it, has `{"replace": true, "values": [3]}`.
  - **Expected:** `#minecraft:t = [stone]`, and an error is logged for P2's file.

Provenance: `source_suggested`. Confidence: medium. Experiment: as in the
conformance tests, observed in the update tags packet.

### REG-TAG-005: Resolution and member order

- **Inputs:** the effective member list of T (REG-TAG-003), and the resolved members of every tag that T references.
- **Behavior:**
  - The resolved members of T form the sequence obtained by walking the effective list in order:
    - an element member contributes that entry;
    - a tag member contributes the resolved members of the referenced tag, in that tag's order.
  - When an entry would appear more than once, only its first position is kept.
  - The result has no duplicates.
- **Ordering constraints:** member order is observable in the update tags packet, and wherever the game picks among a tag's members by position. Do not sort members.
- **Postconditions:** resolution of T does not depend on the order in which tags are processed, except in the cycle case of REG-TAG-007.
- **Edge cases:**
  - Duplicates in one file, duplicates across packs and members that also come through a nested tag all collapse to the first position.
  - A tag that references an empty tag gains nothing from it.
- **Conformance tests:**
  - **Input:** `#t:x = [c, #t:y, a]` and `#t:y = [b, c, d]`.
  - **Expected:** `#t:x` resolves to `[c, b, d, a]`.
  - **Shipped case:** `#minecraft:logs` (`assets/minecraft/tags/block/logs.json`) resolves to the members of `#minecraft:logs_that_burn`, then `#minecraft:crimson_stems`, then `#minecraft:warped_stems`, each expanded in its own order.
- **Experiment:** capture the update tags packet on a vanilla-only server. The block ids listed for `minecraft:logs` must equal that expansion, with the ids taken from `registries.json`.

Provenance: `source_suggested`. Confidence: medium.

### REG-TAG-006: Missing members

- **Inputs:** a member that names an entry or tag that does not exist.
- **Behavior:**

| Member | Static registry (any time), or data-pack registry on `/reload` | Data-pack registry at server start |
|--------|---------------------------------------------------------------|------------------------------------|
| optional element, absent | skipped | skipped |
| required element, absent | **T fails** | the load fails, reported as unresolved keys of the registry |
| optional tag, absent or failed | skipped | skipped |
| required tag, absent or failed | **T fails** | **T fails** |

  - "T fails" means T does not exist at all. No partial tag is kept.
  - One error naming T and listing all of its missing members is logged.
  - Any tag that requires T fails in turn.
  - Every entry that references T then fails under REG-REF-002.
- **Ordering constraints:** none beyond dependency. A tag referenced later in the stack or in another pack resolves first.
- **Edge cases:**
  - Members are always keys of the tag's own registry. The same id in another registry does not satisfy a member.
  - For required elements in data-pack registries, startup and `/reload` behave differently. Both outcomes are observable and both are required (REG-RELOAD-002).
- **Conformance tests:**
  - **Input:** `tags/block/t.json` = `["minecraft:stone", "t:nope"]`.
  - **Expected:** `#minecraft:t` is absent, and `/execute if block ~ ~ ~ #minecraft:t` reports an unknown tag.
  - **Input:** the same file with `{"id": "t:nope", "required": false}`.
  - **Expected:** `#minecraft:t = [stone]`.
  - **Input:** `tags/damage_type/t.json` = `["t:nope"]` at startup.
  - **Expected:** the server does not start.

Provenance: `source_suggested`. Confidence: medium. Experiment: as in the
conformance tests.

### REG-TAG-007: Cycles

- **Inputs:** tags whose required or optional tag references form a cycle.
- **Behavior:**
  - **All references in the cycle are required**, including a tag that requires itself: every tag on the cycle fails (REG-TAG-006), and so does every tag that requires one of them.
  - **The cycle has at least one required edge and exactly one optional edge**, for example A requires `#B` and B optionally references `#A`. The tag holding the optional edge (B) resolves without the other tag's members. The other tag (A) resolves with B's members. Both exist.
  - **The cycle has two or more optional edges**, including the case where every edge is optional: exactly one optional edge is ignored. The tag holding the ignored edge resolves without the members it would have gained through that edge, and every other tag on the cycle resolves normally. No rule visible to a data pack author determines which optional edge the game ignores (OQ-02).
  - **Local rule for two or more optional edges (a free choice).** Conformance requires only the previous bullet: exactly one optional edge of the cycle is ignored. Within that freedom, this specification fixes the choice so that the same input always gives the same tags: the ignored edge is the optional edge of the cycle whose holding tag sorts last by REG-ID-002; when that tag holds more than one optional edge of the cycle, the one whose target tag sorts last by REG-ID-002 is ignored. If tags still form a cycle after that edge is removed, the rule is applied again to the remaining cycle. This rule is not the game's; it replaces whatever the game does until OQ-02 is answered, and a test must not treat a vanilla server that picks a different edge as wrong.
- **Postconditions:** resolution always terminates. A cycle never produces a tag that contains itself.
- **Conformance tests:**
  - `#t:a = [#t:b]`, `#t:b = [#t:a]`, both required. Expected: both are absent.
  - `#t:a = [x, #t:b]`, `#t:b = [y, {"id": "#t:a", "required": false}]`. Expected: `#t:b = [y]` and `#t:a = [x, y]`.
  - Local rule: `#t:a = [x, {"id":"#t:b","required":false}]`, `#t:b = [y, {"id":"#t:a","required":false}]`. `t:b` sorts after `t:a`, so the edge held by `t:b` is ignored. Expected: `#t:b = [y]`, `#t:a = [x, y]`, on every load. The conformance minimum alone accepts either this result or `#t:a = [x]`, `#t:b = [y, x]`.

Provenance: `source_suggested`. Confidence: medium (low for the all-optional case).
The local rule is a choice of this specification, not a fact about the game.

### REG-TAG-008: Empty tags (short form)

A tag whose resolved member list is empty exists. A lookup finds it, it matches
nothing, and the update tags packet sends it with zero members. Fixture:
`assets/minecraft/tags/damage_type/bypasses_cooldown.json`.

Provenance: `source_suggested_black_box_confirmed`. Confidence: medium.

### REG-TAG-009: Data-pack tags are the only tags (short form)

On a server running only the `vanilla` pack, the tags of every registry are
exactly the tag files of `assets/minecraft/tags/`, resolved. The game defines no
tag from code. The set of tag names per registry is in `names.json` under `tags`.

Provenance: `source_suggested`. Confidence: medium. Experiment: on a
`--safeMode` server, capture the update tags packet. For every synced registry
(REG-NET-008), the tag ids must equal the `names.json` list for that registry.

---

## 8. What the client receives

### REG-NET-001: Configuration sequence

- **Inputs:** a client that has finished login and entered the configuration state.
- **Behavior:** the server sends, in this order (ids from `packets.json`, configuration state):
  1. `minecraft:update_enabled_features` (clientbound 13), carrying the ids of the world's enabled features (`enabled_features` of REG-SAVE-001). A reload never changes them. Entries of data-pack registries carry no feature of their own: a feature restricts them only through which packs are in the stack (REG-PACK-002).
  2. `minecraft:select_known_packs` (clientbound 15), carrying the offered known packs (REG-NET-002).
  3. The server waits for `minecraft:select_known_packs` (serverbound 7).
  4. One `minecraft:registry_data` (clientbound 7) per synced registry, in the order of REG-NET-004.
  5. `minecraft:update_tags` (clientbound 14).

  Later configuration packets, and finally `minecraft:finish_configuration`, follow. Other packets that do not concern registries can come before step 2, for example the brand and server links.
- **Ordering constraints:**
  - No registry data is sent before the client answers. The server cannot know which entries to leave out until then.
  - All registry data packets come before the update tags packet. This is the order the game's server sends; the vanilla client itself only collects both until `finish_configuration` (REG-NET-007), so it does not depend on this order.
- **Postconditions:** registry data is sent only in the configuration state. Reconfiguration repeats the sequence.

Provenance: `source_suggested_black_box_confirmed` (packet ids:
`assets/mcrs/reports/packets.json`). Confidence: medium. Experiment: capture a
vanilla client joining a vanilla server, and check the order.

### REG-NET-002: Known packs offered by the server

- **Inputs:** the server's pack stack.
- **Behavior:**
  - The offered list holds, in pack stack order, one triple `(namespace, id, version)` for each enabled pack that has a known-pack identity:

| Pack | Triple |
|------|--------|
| `vanilla` | (`minecraft`, `core`, `<version id>`) |
| built-in feature pack `<id>` | (`minecraft`, `<id>`, `<version id>`) |
| any `file/…` or other pack | none (not offered) |

  - `<version id>` is the `id` of `assets/minecraft/version.json`, which is `26.4-snapshot-4`.
  - A pack with no `data/` directory is offered like any other pack with an identity (REG-PACK-001).
- **Postconditions:** the offered list, and the attribution of every entry it covers (REG-ENTRY-006), come from the pack stack that the stage 1 and stage 2 registries were loaded with at server start. That stack stays fixed for the session: a `/datapack enable` or `disable`, or a `/reload` that finds a new pack, does not change the offer made to clients that join afterwards.
- **Edge cases:**
  - This postcondition is a deliberate rule of this specification and differs from the game. The game computes the offer from the pack stack of the latest successful reload, while the synced entries and their attribution stay those of the start. With the packs shipped at this version the difference shows only in the `select_known_packs` packet: `trade_rebalance` overrides no entry of a synced registry, so no entry data differs. A pack with an identity that did override a synced entry would make a vanilla client fill that entry from a stack the server did not load it with; the rule above prevents that.
- **Conformance tests:**
  - A vanilla-only server offers `[(minecraft, core, 26.4-snapshot-4)]`.
  - With the stack `[vanilla, file/x, trade_rebalance]`, the server offers `[core, trade_rebalance]`, both with namespace `minecraft` and the version id.
  - Start with `[vanilla]`, run `/datapack enable "trade_rebalance"`, then join. Expected: the offer is `[core]`.

Provenance: `source_suggested`. Confidence: medium. Experiment: capture
`select_known_packs` from a vanilla server with and without `trade_rebalance`
enabled, and once after enabling it at run time (the game is expected to offer
`[core, trade_rebalance]` there, which this specification does not follow).

### REG-NET-003: Negotiation

- **Inputs:** the offered list O and the client's reply list A.
- **Behavior:**
  - If A equals O element by element (same length, same order, same triples), the negotiated set is the set of triples in O. Otherwise the negotiated set is empty, and the data of every entry is sent.
  - A reply with more than 64 triples is a protocol error.
  - A vanilla client replies with the triples of O that it has as built-in packs, in the order of O. With a matching version, that is all of O. With a different game version, the version strings differ, the reply is shorter, and the negotiated set is empty.
- **Edge cases:**
  - A reply that is a reordered O, or a subset of O, gives the empty negotiated set. No partial negotiation takes place.
  - An empty O with an empty A means equality, and the negotiated set is empty.
- **Conformance tests:**
  - O = [core], A = [core]: core entries are sent without data.
  - O = [core, trade_rebalance], A = [core]: every entry is sent with data.

Provenance: `source_suggested`. Confidence: medium. Experiment: a scripted client
replies with a subset and with an empty list. Capture the registry data, where
every entry must then carry data.

### REG-NET-004: Synced registries and their order

- **Inputs:** none. This data is fixed by the game, and no report shows it.
- **Behavior:**
  - The server sends exactly these data-pack registries, one registry data packet each, in this order:

    1. `minecraft:worldgen/biome`
    2. `minecraft:chat_type`
    3. `minecraft:trim_pattern`
    4. `minecraft:trim_material`
    5. `minecraft:wolf_variant`
    6. `minecraft:wolf_sound_variant`
    7. `minecraft:pig_variant`
    8. `minecraft:pig_sound_variant`
    9. `minecraft:frog_variant`
    10. `minecraft:cat_variant`
    11. `minecraft:cat_sound_variant`
    12. `minecraft:cow_variant`
    13. `minecraft:cow_sound_variant`
    14. `minecraft:chicken_variant`
    15. `minecraft:chicken_sound_variant`
    16. `minecraft:zombie_nautilus_variant`
    17. `minecraft:painting_variant`
    18. `minecraft:sulfur_cube_archetype`
    19. `minecraft:dimension_type`
    20. `minecraft:damage_type`
    21. `minecraft:banner_pattern`
    22. `minecraft:enchantment`
    23. `minecraft:jukebox_song`
    24. `minecraft:instrument`
    25. `minecraft:test_environment`
    26. `minecraft:test_instance`
    27. `minecraft:dialog`
    28. `minecraft:world_clock`
    29. `minecraft:timeline`
    30. `minecraft:decorated_pot_pattern`
    31. `minecraft:block_transformer`
    32. `minecraft:worldgen/block_state_provider`
    33. `minecraft:block_sound_set`

  - A packet is sent for each of these registries even when it has zero entries.
  - Static registries are never sent as entries, because both sides already know them (REG-SET-001).
  - Data-pack registries not on this list are never sent: the other world generation registries, `dimension`, and every stage 3 registry.
- **Local rule: registries outside the `minecraft` namespace.** This implementation can declare a registry whose key is outside the `minecraft` namespace. The server never sends such a registry, whether it is a static registry or a data-pack registry. It sends no registry data packet for it, and the update tags packet has no record for it (REG-NET-008). An entry outside the `minecraft` namespace in a `minecraft` registry is sent as any other entry. The game has no registry outside the `minecraft` namespace, so a vanilla client receives the same packets from this implementation as from the game.
- **Edge cases:** the vanilla client loads every synced registry together after it has received all packets. Whether a different packet order changes anything for it is OQ-06. For bit parity with vanilla, use the order above.
- **Invariants:** the list is a subset of the `elements: true` registries in `datapack.json`.

Provenance: `source_suggested`. Confidence: medium. Experiment: capture a vanilla
join and list the registry ids of the registry data packets in order.

### REG-NET-005: Registry data packet content

- **Inputs:** one synced registry R, its entries in id order (REG-ENTRY-004), and the negotiated set N (REG-NET-003).
- **Behavior:**
  - The packet lists every entry of R, in ascending numeric id. Each item is the entry key, plus the entry's data, which is present or absent.
  - The data is **absent** if and only if the entry's known-pack attribution (REG-ENTRY-006) is in N. Otherwise it is **present**.
  - Present data is the entry encoded as a network NBT value in the registry's network form:
    - For `worldgen/biome`, `dimension_type`, `timeline`, `wolf_variant`, `wolf_sound_variant`, `pig_variant`, `pig_sound_variant`, `frog_variant`, `cat_variant`, `cat_sound_variant`, `cow_variant`, `chicken_variant` and `zombie_nautilus_variant`, the network form is a reduced form that leaves out server-only fields.
    - For the other synced registries, the network form is the same shape as the data pack form.

    The per-registry shapes, including NBT numeric types, are out of scope (§0.6). The list of registries with a reduced form is `source_suggested`; the experiment below confirms or corrects it. In particular, `cow_sound_variant`, `chicken_sound_variant` and `sulfur_cube_archetype` are listed under the second bullet on purpose: their network form is suggested to be the full data pack form.
  - Inside present data, a reference to another entry is written as its key. An inline value is written in place. An entry set backed by a tag is written as `#tag`. Any other entry set is written as a list (REG-REF-001).
- **Postconditions:** the client can rebuild R with the same keys and the same numeric ids (REG-NET-006).
- **Edge cases:**
  - An entry overridden by a `file/…` pack always carries data, even when vanilla has a file at the same path.
  - An entry whose winning file comes from `trade_rebalance` is sent without data only when `trade_rebalance` is negotiated.
  - The server sends exactly one packet per synced registry and never splits a registry. A registry whose packet would exceed the size limits of REG-NET-010 cannot be sent; the game has no fallback.
- **Conformance tests:**
  - **Initial state:** a vanilla-only server, and a client that echoes the offer.
  - **Expected:** every entry of every synced registry has absent data.
  - **Input:** a world pack overrides `minecraft:damage_type/arrow`.
  - **Expected:** only `minecraft:arrow` in `minecraft:damage_type` carries data.

Provenance: `source_suggested`. Confidence: medium. Experiment: as in the
conformance tests, by packet capture. Experiment for the reduced forms: join a
vanilla server with a scripted client that replies to `select_known_packs` with an
empty list, so that every entry carries data. For each of the 32 synced registries,
decode the data of every entry and compare its set of fields, at every depth, with
the entry's file in the `vanilla` pack. Record, per registry, which fields the
network data leaves out. The registries with no missing field use the data pack
form; the others use a reduced form, and the recorded field lists become part of
that registry's own specification.

### REG-NET-006: Network ids of synced entries

- **Behavior:** the network id of an entry of a synced registry is its zero-based index in the registry data packet for that registry. That index equals the server-side numeric id (REG-ENTRY-004).
- **Invariants:** wherever a play or configuration packet refers to a synced entry by number, the number is that index. This includes the dimension type in the login and respawn packets, biome ids in chunk data, entity variant ids in entity data, the members of the update tags packet, and id-encoded values in item components.
- **Edge cases:** a client applies the ids in the order received. So a server that sends any order is self-consistent, but it matches vanilla only with the order of REG-ENTRY-004.
- **Conformance tests:** on join, the dimension type id in the play `login` packet (`packets.json`: play clientbound 50) is the index of `minecraft:overworld` (0 on a vanilla-only server) in the `minecraft:dimension_type` registry data packet.

Provenance: `source_suggested`. Confidence: medium. Experiment: as in the
conformance test, by packet capture.

### REG-NET-007: Client acceptance of registry data

- **Inputs:** the configuration packets of REG-NET-001, up to `minecraft:finish_configuration`.
- **Behavior (vanilla client):**
  - **Order of work.** While configuring, the client only collects: every registry data packet and every update tags packet is stored as received, and nothing is decoded or resolved. When `minecraft:finish_configuration` arrives, the client builds the synced registries from everything collected, binds their tags, decodes every entry, resolves every reference inside it (including references to tags), and then applies the tags of static registries. Received entries therefore never need tags that arrive earlier than they do; they need tags that arrive before `finish_configuration`.
  - Several registry data packets for the same registry are concatenated in arrival order, and the entry ids follow that concatenation. This is `source_suggested` and unconfirmed; OQ-06 holds the experiment.
  - Several update tags packets in one configuration: for each registry, the last packet that names it replaces the earlier ones.
  - For an entry with absent data, the client reads `data/<ns>/<R path>/<p>.json` from its own copies of the negotiated packs, stacked in negotiated order (REG-PACK-003 applied to that stack). If no such file exists, loading fails. If the server sent no `select_known_packs`, there are no negotiated packs and every absent-data entry fails.
  - The client applies REG-SET-004 to the synced registries. Those tags that received entries reference must be present in the collected update tags.
  - Tags of static registries come only from the update tags packet, never from files. Member ids that name no entry the client knows are dropped silently (REG-NET-008).
  - A registry that never arrives is empty on the client, which fails REG-SET-004 for the registries listed there.
  - Any failure stops the connection from completing configuration, with a registry-loading error naming the registry and entry. Whether the vanilla client then shows a disconnect screen or crashes is unconfirmed: the reference suggests the error escapes the packet handler and crashes the client.
- **Postconditions:** a server must send every tag that a synced entry references. Sending all tags (REG-NET-008) satisfies this.
- **Behavior (conforming client):** it accepts the registry data packets in any order of registries and accepts a registry split over several packets, concatenating them in arrival order. This accepts at least everything the vanilla server sends, whatever OQ-06 finds.
- **Experiment:** a scripted server leaves the tag `minecraft:exclusive_set/armor` out of `update_tags`. The vanilla client fails at `finish_configuration` and does not reach the play state. A second run sends `update_tags` before any registry data packet; the client joins normally.

Provenance: `source_suggested`. Confidence: medium.

### REG-NET-008: Update tags packet content

- **Inputs:** the resolved tags of every registry.
- **Behavior:**
  - The packet contains one record for each registry that has at least one tag and is either a static registry or a synced registry (REG-NET-004).
  - Registries with no tags are left out.
  - The tags of data-pack registries that are not synced are never sent. Examples are `worldgen/structure`, `villager_trade`, `worldgen/world_preset`, `worldgen/flat_level_generator_preset` and the stage 3 registries.
  - Local rule: a registry outside the `minecraft` namespace has no record (REG-NET-004).
  - Each record maps every tag id of the registry, including empty tags, to the numeric ids of its resolved members, in resolved order (REG-TAG-005). The ids are those of REG-SET-001 for static registries and of REG-NET-006 for synced registries.
- **Ordering constraints:**
  - The order of records, and of tags within a record, carries no meaning. Vanilla does not keep a stable order, and a client must accept any order.
  - The order of member ids within a tag carries meaning (REG-TAG-005).
- **Edge cases:** a vanilla client silently drops member ids that do not name an entry it knows.
- **Conformance tests:**
  - On a vanilla-only server, the record for `minecraft:block` has the tag names `names.json` lists under `tags."minecraft:block"`.
  - There is no record for `minecraft:worldgen/structure`, although `names.json` lists 33 tags for it.
  - A world pack defines `data/minecraft/tags/sound_event/t.json` = `{"values": ["minecraft:block.stone.break"]}`. Vanilla ships no `sound_event` tag. Expected: the packet has a `minecraft:sound_event` record holding `minecraft:t` with the one id `registries.json` gives that sound event. Every registry accepts tags (REG-SET-002), so tags a pack adds to any static or synced registry are sent.

Provenance: `source_suggested`. Confidence: medium. Experiment: as in the
conformance tests, by packet capture.

### REG-NET-009: Updates during play (short form)

Registry data is never sent in the play state. After every successful reload
(REG-RELOAD-001), the server sends a new full update tags packet (play
clientbound 137 in `packets.json`) to every connected player, with the same
content rules as REG-NET-008. The entries of synced registries cannot change
without a restart, so their ids stay valid.

The vanilla client applies a play-state update tags packet per registry: for each
registry the packet names, that registry's tags become exactly the packet's tags,
and a tag the packet leaves out no longer exists on the client. A registry that the
packet does not name keeps its previous tags. A registry with no tag at all is left
out of the packet (REG-NET-008), so on the client it keeps its previous tags; see
REG-RELOAD-003 for when that matters. The same content rules give the
configuration-state and the play-state packet; only the packet id differs.

Provenance: `source_suggested`. Confidence: medium. Experiment: add a block tag
file and run `/reload`. The capture shows one play `update_tags` packet that
includes the new tag.

### REG-NET-010: Wire layout (short form)

`VarInt` is the protocol's variable-length integer. A string is a `VarInt`
length followed by UTF-8. An identifier is a string.

- `select_known_packs` (both directions): `VarInt` count, then per pack: namespace string, id string, version string. The serverbound count is at most 64.
- `registry_data`: the registry id, a `VarInt` entry count, then per entry: the key, a boolean "data present", and, if present, one network NBT value. A network NBT value is a type byte and a payload, with no root name.
- `update_tags`: a `VarInt` registry count, then per registry: the registry id and a `VarInt` tag count. Per tag: the tag id (without `#`), a `VarInt` member count, and that many `VarInt` member ids.

Limits:

| Quantity | Limit | Outcome past it |
|----------|-------|-----------------|
| entries in one registry, at load | none beyond the `VarInt` range of ids | — |
| entries in one `registry_data` packet | none | — |
| tags per registry, members per tag, registries in `update_tags` | none | — |
| identifier or other string on the wire | 32 767 characters, and at most three bytes per character in UTF-8 | the receiver rejects the packet |
| network NBT value of one entry | 2 097 152 bytes of accounted size, nesting depth 512, checked by the receiver per entry | the receiver rejects the packet |
| one framed packet, after compression | 2 097 151 bytes (a three-byte length prefix) | the sender fails to encode it and the connection closes |
| one packet before compression, with compression on | 8 388 608 bytes | the sender fails to encode it and the connection closes; the vanilla client does not check this limit on what it receives |

There is no limit on the number of entries or tags at load time; a registry that
is too large to send fails only when a client joins. The server never splits a
registry over several packets (REG-NET-005).

Provenance: `source_suggested`. Confidence: medium. Experiment: decode a capture
with this layout and check that the whole frame is consumed. For the limits:
a world pack with 20 000 `painting_variant` entries, each with a long `title`
text, sized so that the `registry_data` packet exceeds 2 097 151 bytes after
compression. Expected: the server starts and a joining client fails in
configuration.

---

## 9. Reload

### REG-RELOAD-001: What `/reload` re-reads

- **Inputs:** `/reload`, or `/datapack enable|disable`, which reload with the new selection.
- **Behavior:**

| Content | Re-read on reload? |
|---------|--------------------|
| entries of stage 3 registries | yes |
| entries of stage 1 and 2 registries | no. Changes apply at the next server start |
| tags of every registry (static, stage 1, stage 2, stage 3) | yes |

  - The stack used for the reload is the current selection, plus available packs that are neither selected nor in `Disabled` (for `/reload`), or the current selection with the pack added or removed (for `/datapack`). It is used for tags and stage 3 entries only.
  - On success, the server switches to the new data, records the new selection (REG-SAVE-001) and resends tags (REG-NET-009).
  - On failure, see REG-ERR-004.
  - The world's enabled features do not change on reload (REG-NET-001).
- **Postconditions:**
  - After any reload, successful or not, the entry set, the decoded value of every entry and the numeric id of every entry of every stage 1 and stage 2 registry are identical to those loaded at server start. This holds even when their files changed on disk, when a pack that provides such entries was enabled or disabled, and when one of those files no longer parses. Every synced registry is in stage 1, so its wire ids and contents stay valid for the whole session.
  - The known-pack offer and the attribution of stage 1 entries stay those of the stack loaded at server start (REG-NET-002).
- **Edge cases:** enabling a pack that adds a biome or a damage type has no effect on those registries until restart, but the pack's tags apply at once.
- **Conformance tests:**
  - **Input:** while running, add `damage_type/t.json` and run `/reload`.
  - **Expected:** `/damage @s 1 t:t` reports an unknown damage type until restart.
  - **Input:** while running, change `message_id` in a world pack's `data/t/damage_type/x.json`, and replace the content of `data/t/damage_type/y.json` with `{`. Run `/reload`.
  - **Expected:** the reload succeeds; a new client receives `t:x` with the old `message_id` and `t:y` unchanged, at the same ids as before.

Provenance: `source_suggested`. Confidence: medium. Experiment: as in the
conformance tests.

### REG-RELOAD-002: Tag errors on reload

- **Behavior:**
  - On reload, the tags of every registry, data-pack registries included, follow the left-hand column of REG-TAG-006. A required element that is absent makes the tag fail, which is logged, and the reload continues.
  - At startup, the same file in a data-pack registry fails the load.
- **Experiment:** start with a valid pack, then edit `tags/damage_type/t.json` to name `t:nope` and run `/reload`. Expected: the reload succeeds, `#t` is missing from the next update tags packet, and an error is logged. A restart with the same file fails.

Provenance: `source_suggested`. Confidence: medium.

### REG-RELOAD-003: Tags that no longer exist after a reload

- **Inputs:** a successful reload after which a tag T of registry R that existed before does not exist: its files were removed or hidden by a filter, or it failed (REG-RELOAD-002).
- **Relevant state:** entries of stage 1 and 2 registries that were loaded at start and reference T, either as an entry set backed by T (REG-REF-001) or as a tag member.
- **Preconditions:** the reload succeeded. A failed reload changes nothing (REG-ERR-004).
- **Behavior:**
  - T has no members. Every test of an entry against T is false, and every use of an entry set backed by T sees an empty set.
  - This includes entry sets inside stage 1 and 2 entries loaded at start: they are not re-decoded, but they follow T's current members.
  - A lookup of T by id, for example in a command, reports an unknown tag.
  - T is absent from the update tags packet sent after the reload and from every later one, until T exists again.
  - A tag that exists again after a later reload has its new members, and entry sets backed by it follow them.
- **Ordering constraints:** none beyond REG-NET-009: the update tags packet is sent after the switch to the new data.
- **Numeric semantics:** none.
- **Postconditions:** the members of an entry set backed by a tag are always the current members of that tag, at all times between reloads.
- **Edge cases:**
  - The game itself is not consistent here: an entry set backed by a vanished tag still enumerates its old members, while a test of membership is false. This specification requires the consistent rule above; it is a minimum for this specification, not a claim about the game.
  - When R ends up with no tag at all after the reload, the game keeps all of R's previous tags unchanged and keeps sending them; it replaces a registry's tags only when the reload produced at least one tag for it. This specification does not follow that: R's tags are all absent, as above. The difference shows only for a registry that loses its last tag.
  - Whether a vanilla client that joins after the reload then fails to load a synced entry that references T is open (OQ-04).
- **Invariants:** a tag absent from update tags is never used by the server to match anything.
- **Conformance tests:**
  - **Initial state:** a world pack defines `data/t/tags/item/t.json` = `{"values": ["minecraft:stone"]}` and an enchantment `t:e` with `"supported_items": "#t:t"`, and the server is started.
  - **Input:** hide the tag with a filter pack and run `/datapack enable` on it.
  - **Expected:** the reload succeeds; `/enchant @s t:e` holding stone fails as an unsupported item; the next update tags packet has no `minecraft:t` in `minecraft:item`; `/execute if items entity @s weapon.mainhand #t:t` reports an unknown tag.

Provenance: `source_suggested` for the game's behaviour in the edge cases;
the main rule is a decision of this specification. Confidence: medium.
Experiment: the conformance test on a vanilla server, plus a second run that
starts with a world pack whose only content is `data/t/tags/sound_event/s.json` (vanilla has
no `sound_event` tag), removes that file and runs `/reload`, capturing update tags:
the game is expected to keep sending `minecraft:sound_event` with `t:s`.

---

## 10. Persistence

### REG-SAVE-001: Pack selection in `level.dat` (short form)

`level.dat` stores:

- `DataPacks: {Enabled: [<pack id>...], Disabled: [<pack id>...]}`. `Enabled` is in pack stack order, lowest priority first. `Disabled` lists the available packs that are not enabled.
- `enabled_features: [<feature id>...]`.

When `DataPacks` is absent, the default is `Enabled: ["vanilla"], Disabled: []`.
The selection is written back after every successful reload or pack change.

Provenance: `source_suggested`. Confidence: medium. Experiment: read `level.dat`
with an NBT viewer before and after `/datapack enable "file/x"`.

### REG-SAVE-002: No data-pack registry ids on disk (short form)

The save refers to entries of data-pack registries by key, never by numeric id.
This covers biome palettes in chunk sections, dimension keys, and entity variant
fields. A world therefore stays valid when packs that shift the ids
(REG-ENTRY-004) are added or removed.

Provenance: `source_suggested`. Confidence: medium. Experiment: create a world
and save it. Add a pack with `worldgen/biome/aaa.json`, which shifts every
vanilla biome id, and restart. Existing chunks keep their biomes (check with
`/locate biome` and F3).

---

## 11. Open questions

Each question lists the requirement it affects and an experiment that decides it.

- **OQ-01: Duplicate object keys in JSON** (REG-ENTRY-002, REG-TAG-001). **Closed** by REG-ENTRY-008: the last value wins and the file is not malformed. The answer is `source_suggested`; this experiment confirms it.
  - **Experiment:** a damage type file with `"exhaustion": 0.1` and then `"exhaustion": 5.0`. Start, then capture the registry data packet with the negotiated set empty. Expected: `5.0`. A startup failure or `0.1` would contradict REG-ENTRY-008.
- **OQ-02: Cycles with only optional edges** (REG-TAG-007). Open for the game's behaviour; REG-TAG-007 fixes a local rule meanwhile.
  - **Question:** with `#t:a = [x, {"id":"#t:b","required":false}]` and `#t:b = [y, {"id":"#t:a","required":false}]`, which tag includes the other, and is the choice stable across restarts and tag names?
  - **Experiment:** repeat with renamed tags (`t:a`/`t:b`, `t:b`/`t:a`, `u:q`/`v:q`), with a three-tag cycle, and with the tags of the cycle defined in different packs, over several restarts, capturing update tags each time. If the results vary, conformance only requires "exactly one includes the other", and the local rule of REG-TAG-007 stays. If they follow a fixed rule, that rule replaces the local one.
- **OQ-03: Schema-invalid tag file and tag existence** (REG-TAG-003, REG-TAG-004). Answered as `source_suggested`: the tag exists, empty. Kept until the experiment confirms it.
  - **Question:** when the only file for `#t:x` is valid JSON but invalid as a tag file, for example `{"values": [3]}`, does `#t:x` exist as an empty tag or not at all?
  - **Experiment:** `/execute if block ~ ~ ~ #t:x` reports either "false" or "unknown tag", and a capture of update tags shows whether `t:x` appears with zero members. Compare with a file that is syntactically invalid JSON (expected: unknown tag, absent from update tags).
- **OQ-04: Reload removes a tag that a stage 1 entry references** (REG-RELOAD-001, REG-RELOAD-003, REG-NET-007). Open for the client; REG-RELOAD-003 fixes the server's minimum.
  - **Question:** after `/reload` removes `#minecraft:exclusive_set/armor`, does a newly joining vanilla client, which receives an update tags packet without that tag, fail to load the enchantments that reference it?
  - **Experiment:** start with the tag, then hide it with a filter pack and `/reload`. Join with a new client, and capture the update tags packet and the client outcome. Also, before joining, combine two armor enchantments on an anvil on the server to see whether the old exclusivity still applies.
- **OQ-05: Unknown fields in entries** (REG-ENTRY-003). **Closed**: unknown fields are ignored in every data-pack registry, as the game ignores them. The experiment under REG-ENTRY-003 confirms it per decoder family.
- **OQ-06: Order of registry data packets** (REG-NET-004, REG-NET-007).
  - **Question:** does a vanilla client accept the synced registries in a different packet order, or split across several packets?
  - **Experiment:** a scripted server sends the 32 packets in reverse order, and also splits `minecraft:worldgen/biome` into two packets. Observe whether the client joins and whether biome ids match. Whatever the result, a conforming client accepts both (REG-NET-007), and a conforming server sends the order of REG-NET-004 with one packet per registry.
- **OQ-07: Filter matching details** (REG-PACK-004).
  - **Question:** confirm unanchored matching, and confirm that a namespace pattern and a path pattern from different block entries combine.
  - **Experiment:** use only non-vanilla namespaces, so that no vanilla registry can become empty. The stack is `[vanilla, file/a, file/b]`. `file/a` provides `t:damage_type/x`, `u:damage_type/x` and `u:damage_type/y`. `file/b` has the filter `[{"namespace":"^t$","path":"^zzz$"},{"namespace":"^u$","path":"^damage_type/x\\.json$"}]`. Under either reading `u:x` is hidden and `u:y` is kept. If the patterns combine, `t:x` is hidden too (its namespace matches the first pattern and its path the second); if they do not, `t:x` is kept. Separately, filter `{"path":"black"}` and check whether `cat_variant/all_black.json` is hidden (expected: yes, since `black` occurs in it), which confirms unanchored matching.
- **OQ-08: Reference cycles between entries** (REG-REF-002).
  - **Question:** is an entry that references itself, directly or through other entries of the same registry (for example density functions), rejected at load, or only when it is used?
  - **Experiment:** a pack with `worldgen/density_function/t/a.json` = `"t:t/b"` and `t/b.json` = `"t:t/a"`, and a second pack with the self-reference `t/a.json` = `"t:t/a"`. Start the server, then create a world whose noise settings reference `t:t/a`.
