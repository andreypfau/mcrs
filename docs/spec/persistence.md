# Persistence

This document states what the server keeps in a save and how it reads that save. It does not state
an upgrade path from another world version. The vanilla game must be able to open a save that the
server writes. Thus each rule about a file format states the layout that the game reads and writes.
The label `design` marks a rule that states a choice of mcrs where the game states nothing.

## Terms

- **Save directory**: the directory of one world.
- **Dimension directory**: `dimensions/<namespace>/<path>` below the save directory. The namespace
  and the path are the two parts of the dimension key.
- **Column**: one 16 by 16 chunk column.
- **Region**: the 32 by 32 group of columns that one `r.<x>.<z>.mca` file holds.
- **Stored column**: the form of a column while it is cold or on disk.
- **Dirty column**: a stored column that the data layer holds and the region file does not hold yet.
- **Data layer**: the part of the server that holds stored columns and writes them to region files.
- **Saved data file**: a gzip NBT file below `data/`, with the layout of SAVE-DATA-001.
- **Unknown field**: a field in a save file that the field set of this document does not list.

## SAVE-FILE-001: File layout

A save uses these paths:

```text
<save>/session.lock
<save>/level.dat
<save>/data/minecraft/weather.dat
<save>/data/minecraft/game_rules.dat
<save>/data/minecraft/world_clocks.dat
<save>/data/minecraft/world_gen_settings.dat
<save>/data/mcrs/birth_clock.dat
<save>/players/data/<uuid>.dat
<save>/dimensions/<namespace>/<path>/region/r.<x>.<z>.mca
<save>/quarantine/<namespace>/<path>/r.<x>.<z>/attempt-<n>/
```

`level.dat`, the saved data files and the player files are gzip-compressed NBT. A region file uses
the Anvil sector format (SAVE-REGION-001). The file name of a player is the UUID in the lowercase
form with hyphens.

The vanilla game uses these paths too, except `data/mcrs/` and `quarantine/`. The game ignores
`data/mcrs/birth_clock.dat`, because the game has no saved data with that name. The game creates
the files that the server does not write, for example `players/stats/`, `players/advancements/`
and the directories `entities` and `poi` of a dimension. The server writes none of these
(SAVE-LIMITS-001). `session.lock` is the lock of the save directory (SAVE-LOCK-001).

A dimension key whose path is empty or leaves the dimension directory is refused. The server does
not map it to a path outside the save.

Provenance: `source_suggested`. Kind: file format. Confidence: high.

## SAVE-LEVEL-001: The layout of `level.dat`

The root compound of `level.dat` has one key, the compound `Data`. The table lists all 18 keys of
`Data`. The column "Written" states when the server writes a key. The game reads each key with the
default of the table, so a key that is missing does not stop the game. The game reads no other key
at this version.

| Key | Type | Default when absent | Written | Meaning |
| --- | --- | --- | --- | --- |
| `DataVersion` | int | none | always | The world version. `5127` for this game version. |
| `Version` | compound | no version | always | `Name` (string), `Id` (int), `Snapshot` (byte), `Series` (string). |
| `version_history` | list of int | empty | always | The world versions that opened the world. |
| `LevelName` | string | empty string | always | The name of the world. |
| `Time` | long | 0 | always | The game time in ticks. |
| `LastPlayed` | long | 0 | always | The time of the last save, in milliseconds since the epoch. |
| `GameType` | int | 0 | always | The default game mode: 0 survival, 1 creative, 2 adventure, 3 spectator. |
| `spawn` | compound | the overworld, `0 0 0`, yaw 0, pitch 0 | always | `dimension` (string), `pos` (int array of three), `yaw` (float, -180 through 180), `pitch` (float, -90 through 90). |
| `difficulty_settings` | compound | `normal`, not hardcore, not locked | always | `difficulty` (string), `hardcore` (byte), `locked` (byte). All three are required. |
| `allowCommands` | byte | true when `GameType` is 1, else false | always | Whether cheats are on. |
| `initialized` | byte | true | always | Whether the world finished its first start. |
| `version` | int | 0 | always | The format number. The game accepts `19132` and `19133`, and the server writes `19133`. |
| `DataPacks` | compound | `Enabled` is `["vanilla"]`, `Disabled` is empty | always | `Enabled` and `Disabled`: lists of pack ids. Both are required inside the compound. |
| `enabled_features` | list of string | `["minecraft:vanilla"]` | always | The ids of the enabled feature flags. |
| `WasModded` | byte | false | always | Whether a modification ran on the world. |
| `ServerBrands` | list of string | empty | always | The brands of the servers that opened the world. |
| `removed_features` | list of string | empty | only when not empty | The ids of the feature flags that the world removed. |
| `singleplayer_uuid` | int array of four | none | only in a single-player world | The UUID of the owner of the world. |

The server appends the current world version to `version_history` when the list is empty or its last
entry differs. Older world versions kept the player and the world generation settings in `Data`, in
the keys `Player` and `WorldGenSettings`. The game skips both keys when it lists worlds, and it does
not read them. The server treats both keys as unknown fields.

The time of day, the weather, the game rules and the world generation settings are not in
`level.dat`. They are in saved data files (SAVE-DATA-001).

Provenance: `source_suggested`. Kind: file format. Confidence: high.

## SAVE-DATA-001: The layout of a saved data file

The root compound of a saved data file has the int `DataVersion` and the compound `data`. The
compound `data` holds the content. The key of `data` has a lowercase first letter. The key `Data` of
`level.dat` has an uppercase first letter, and it holds `DataVersion` inside it. This difference is
a trap. The server writes both keys of the root in every saved data file.

The server reads and writes these files. The tables list every key of `data`.

`data/minecraft/weather.dat`. All five keys are required. If one is missing, the game logs an error
and uses the default weather.

| Key | Type | Meaning |
| --- | --- | --- |
| `clear_weather_time` | int | The ticks until the clear weather ends. |
| `rain_time` | int | The ticks until the rain starts or stops. |
| `thunder_time` | int | The ticks until the thunder starts or stops. |
| `raining` | byte | Whether it rains. |
| `thundering` | byte | Whether it thunders. |

`data/minecraft/game_rules.dat`. The compound `data` has one key for each game rule that the file
sets. The key is the id of the game rule, for example `minecraft:advance_time`. The value is a byte
for a boolean rule and an int for an integer rule. A game rule that the file does not set has its
default value. The server writes every game rule that has a value. A key that is not the id of a
game rule that this build knows is an unknown field.

`data/minecraft/world_clocks.dat`. The compound `data` has one compound for each clock. The key is
the id of the clock, for example `minecraft:overworld`.

| Key of a clock | Type | Default when absent | Meaning |
| --- | --- | --- | --- |
| `total_ticks` | long | required | The ticks that the clock counted. |
| `partial_tick` | float | 0 | The fraction of a tick. |
| `rate` | float, more than 0 | 1 | The speed of the clock. |
| `paused` | byte | false | Whether the clock stands still. |

`data/minecraft/world_gen_settings.dat`.

| Key | Type | Default when absent | Meaning |
| --- | --- | --- | --- |
| `seed` | long | required | The world seed. |
| `generate_structures` | byte | true | Whether the world generates structures. |
| `bonus_chest` | byte | false | Whether a new world has a bonus chest. |
| `legacy_custom_options` | string | none | The generator options of a very old world. |
| `dimensions` | compound | required | One compound for each dimension key. |

A compound in `dimensions` has the two required keys `type` and `generator`. The key `type` is the
id of a dimension type. The key `generator` is a compound in the data pack format of a chunk
generator: its string `type` and the keys of that type.

`data/mcrs/birth_clock.dat`. Only the server uses this file. The compound `data` has the required
long `tick`, the value of the birth clock. The game does not read the file.

If a file is missing, the game uses the default content. If `world_gen_settings.dat` is missing or
unreadable, the game uses a random seed. Thus a damaged seed file changes the terrain that the game
generates.

The game also writes other saved data files, for example the scoreboard, the raids and the world
border. The server does not read or write them (SAVE-LIMITS-001).

Provenance: `source_suggested` for the files of the game, `design` for `birth_clock.dat`. Kind: file
format. Confidence: high.

## SAVE-FILE-002: Atomic replacement and unknown fields

`level.dat`, the saved data files and the player files are written to a temporary file in the same
directory. The server flushes the temporary file to the disk with `fsync`. The server then renames
the temporary file over the target. The server then flushes the directory. A crash leaves the old
file or the new file, never a partial file.

An unknown field in `level.dat`, in a saved data file, in a column record or in a player file is a
loud error. The server reports the file and the field name, and it does not load that file. The
server keeps no raw bag of unknown fields, and it does not write an unknown field back. This is
decision #388. Two exceptions exist: the compounds of block entities (SAVE-CHUNK-001) and the
entity save compounds in a player file (SAVE-PLAYER-001). Issue #738 removes both. The known field
set of each file is in SAVE-LEVEL-001, SAVE-DATA-001, SAVE-CHUNK-001 and SAVE-PLAYER-001.

A file that exists and does not read back is not replaced with defaults. In the list `attributes` of
a player file, only an unknown key is such a failure; the other rows of ATR-SAV-002 of `attributes.md`
give their stated results. A save directory with no `level.dat` is a new world (SAVE-NEW-003).

Provenance: `design`. Kind: durability. Confidence: high. The reason: a field that the server
drops in silence loses the data of the player at the next save. The game also keeps a backup file
with the suffix `_old`. The server does not.

## SAVE-LOCK-001: The lock of a save directory in the game

The file `<save>/session.lock` marks a save directory that a process has open. While the game has
the world open, it holds an exclusive lock of the operating system on the whole file. The lock is
the file lock of the Java platform: a POSIX record lock (`fcntl`) on Linux and macOS, and a lock of
`LockFileEx` on Windows. The game does not see a lock of another kind, for example a `flock` lock
on Linux.

To take the lock, the game does these steps:

1. It makes the save directory when the directory does not exist.
2. It opens `session.lock` for write. It makes the file when the file does not exist.
3. It writes the three bytes `E2 98 83` at offset 0, with no truncation. The bytes are the
   character U+2603 in UTF-8. It flushes the file to the disk.
4. It tries the lock with no wait. When another process holds the lock, the attempt fails at once.

The content has no meaning. The game never reads it.

| Process | The game takes the lock | The game releases the lock |
| --- | --- | --- |
| vanilla client | at "Play Selected World", before it reads `level.dat` | when the local server stops, after its last save |
| vanilla client | at "Create New World", when it makes the world folder. `session.lock` is the first file of the folder. | when the local server stops, after its last save |
| vanilla client | at "Delete", before it removes a file | just before it removes `session.lock`, the last file |
| vanilla client | at "Edit", before it reads `level.dat` | when "Edit World" closes |
| vanilla server | at its start, before it reads `level.dat` | at the end of its stop, after its last save |

A release keeps the file. Only a delete of the world removes it. When the process ends in another
way, for example in a crash, the operating system releases the lock. Thus each save directory that
the game opened holds a `session.lock` of 3 bytes.

The world list tests the lock (`client-world-screens.md`, WORLD-DIR-002, step 5). The test opens
the file for write, with no create, and tries the lock. A file that does not exist is not locked. A
file that the process cannot open for write is locked. Another error removes the world from the
list. The test does not change the file, and it releases the lock that it got at once.

A second process observes a save directory that another process holds:

- The vanilla world list shows the entry as a locked world (`client-world-screens.md`,
  WORLD-ENTRY-002). The player cannot start it from the list.
- When a vanilla client cannot take the lock at "Play Selected World", the toast
  `selectWorld.access_failure`, "Failed to access world", shows and the title screen opens.
- A "Delete" of the vanilla client removes no file. The toast `selectWorld.delete_failure`,
  "Failed to delete world", shows.
- The vanilla server does not start. Its log shows a fatal error that names the path of
  `session.lock`. It changes no other file of the save directory.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by
`assets/minecraft/lang/en_us.json` of the client jar. `source_suggested_black_box_confirmed` for
the content, the release that keeps the file, the conflict between two processes and the test of
the lock by `crates/mcrs_minecraft_save/tests/fixtures/vanilla/local_worlds.json`, `lock` (section Experiments, E1 to E4). Kind: file format for the file, its content
and the kind of lock; semantic for the rest. Confidence: medium.

## SAVE-LOCK-002: The lock of a save directory in mcrs

1. The server takes the lock of SAVE-LOCK-001 when it opens a save directory, before it reads or
   writes another file of it. It writes the content of SAVE-LOCK-001 and tries the lock with no
   wait. When the host gives the server a lock that the host holds on that save directory, the
   server keeps that lock and takes no second lock. Steps 4 and 5 then apply to the given lock.
2. When another process holds the lock, the server does not start. It writes no other file of the
   save directory. The start returns an error of its own type. A caller can tell this error from
   another I/O error, and the error names the save directory.
3. The lock is an exclusive lock on the whole of `session.lock`, which the game sees as locked,
   and which sees the lock of the game. The kind for each system is:
   - Linux: an open file description lock (`F_OFD_SETLK`). It conflicts with the POSIX record
     lock of the game, and the game does not see a `flock` lock on Linux;
   - macOS: a `flock` lock. On macOS, a `flock` lock and a POSIX record lock of another process
     conflict;
   - Windows: a `LockFileEx` lock, the same kind as the lock of the game.
   Thus the game sees the save directory as locked while mcrs holds it, and mcrs sees it as
   locked while the game holds it.
4. The lock belongs to the open file of step 1, not to the process. While a process of mcrs holds
   the lock, a close of another handle of `session.lock` does not release it, and a test of the
   lock in that process reports the save directory as locked. A POSIX record lock does not have
   these properties, so mcrs does not use it for its own lock.
5. The server holds the lock until the shutdown of SAVE-SHUTDOWN-001 has made its last write. Then
   it releases the lock. It does not remove `session.lock`.
6. A server with no save directory takes no lock.
7. The world list of the mcrs client tests the lock as SAVE-LOCK-001 states.
8. A delete of the mcrs client takes the lock before it removes a file. When another process holds
   the lock, the delete removes no file. The delete removes `session.lock` while it still holds
   the lock, then releases the lock, and then removes the empty folder
   (`client-world-screens.md`, WORLD-DEL-003, step 2).
9. When the local server of the client does not start because of the error of step 2, the client
   shows the screen of a failed start (`client-title-screen.md`, TITLE-LOAD-006, edge cases).
   The reason is `selectWorld.locked`, "Locked by another running instance of Minecraft."
   (`client-world-screens.md`, WORLD-PLAY-002, step 6).
10. A create of the mcrs client takes the lock of the new folder at once after it makes the
    folder, before it writes another file (`client-world-screens.md`, WORLD-WRITE-002, step 3).
    The client holds the lock while it writes the files. Then it gives the lock to its local
    server, as step 1 states. Thus the lock has no gap from the create to the stop of the server.
    When the create fails, the client releases the lock.

Provenance: `design` for the typed error of step 2 and for the screen of step 9. `design` for the
hand-over of steps 1 and 10: no other process can take a new world between the create and the
start. `source_suggested` for the claim that the vanilla client does the same: it takes the lock
when it makes the folder and gives the same lock to its local server (SAVE-NEW-001, step 1). No
tool can drive the create of the vanilla client yet, so no experiment confirms it (issue #973).
`source_suggested` for the content of step 1 and for the lock of the game that step 3 must meet.
`design` for the kind of lock of mcrs in steps 3 and 4. On macOS, two processes show that a
`flock` lock and a POSIX record lock conflict in both directions. In one process, a second handle
sees a `flock` lock as held. Kind: file format for steps 1 and 3; semantic for the rest.
Confidence: medium.

## SAVE-NEW-001: A new world in the game

"Create New World" of the vanilla client makes a world folder. `client-world-screens.md`,
WORLD-WRITE-001, step 4, and WORLD-GAME-002, step 2, state the values of the widgets. This rule
adds the order of the writes, and the keys and files that those rules do not state.

1. The client takes the lock of the new folder (SAVE-LOCK-001).
2. Before the local server starts, the client writes `level.dat` one time, with the keys of the
   column "Before the start" of the table below.
3. At the first start of a world whose `initialized` is 0, the local server chooses the world
   spawn. It places the bonus chest when `bonus_chest` is 1. Then it sets `initialized` to 1.
4. The first save of the local server writes `level.dat` again, with the keys of the column
   "After the first save". The previous `level.dat` becomes `level.dat_old`.
5. The first save also writes the saved data files of SAVE-DATA-001 (`weather.dat`,
   `game_rules.dat`, `world_clocks.dat` and `world_gen_settings.dat`), the player file of the
   host, the region files, and the other files of the game (SAVE-LIMITS-001).

| Key of `level.dat` | Before the start | After the first save |
| --- | --- | --- |
| `LevelName` | the name box (WORLD-WRITE-001) | the same |
| `GameType` | "Game Mode" (WORLD-GAME-002) | the same |
| `difficulty_settings` | `difficulty` of "Difficulty", `hard` for "Hardcore"; `hardcore` 1 for "Hardcore", else 0; `locked` 0 | the same |
| `allowCommands` | "Allow Cheats", 0 for "Hardcore" | the same |
| `initialized` | 0 | 1 |
| `spawn` | the default of SAVE-LEVEL-001: the overworld, `0 0 0`, yaw 0, pitch 0 | the spawn that the server chose |
| `Time` | 0 | the game time of the save |
| `LastPlayed` | the time of the write | the time of the save |
| `DataVersion`, `Version` | the game version of this document | the same |
| `version_history` | `[5127]` | the same |
| `version` | 19133 | the same |
| `DataPacks`, `enabled_features` | the data packs and the feature flags of the tab "More" | the same |
| `singleplayer_uuid` | absent | the UUID of the host |

In this version, `level.dat` holds no world generation settings. They are in
`data/minecraft/world_gen_settings.dat` (SAVE-DATA-001), and the first save writes that file.
Before the first save, the folder has no `world_gen_settings.dat`.

| Key of `world_gen_settings.dat` | Value |
| --- | --- |
| `seed` | the seed of the seed box (WORLD-SEED-001) |
| `generate_structures` | "Generate Structures" |
| `bonus_chest` | "Bonus Chest"; 0 for "Hardcore" |
| `dimensions` | the dimensions of the world preset of "World Type", as the data pack defines them |
| `legacy_custom_options` | absent |

The save keeps no id of the world preset. A reader knows the world type of a saved world only when
its `dimensions` equal the dimensions of one world preset.

Provenance: `source_suggested`. Kind: file format. Confidence: medium.

## SAVE-NEW-002: The first start of a world in mcrs

A world has its first start when its `initialized` is 0. A new world of SAVE-NEW-003 also has its
first start.

1. At the first start, before the first player joins, the server sets `spawn` and sets
   `initialized` to 1. The spawn is the default of SAVE-LEVEL-001: the overworld, `0 0 0`, yaw 0
   and pitch 0 (SAVE-DEV-002). A join then places a new player as SAVE-PLAYER-001, "Default
   position", states for `S` at `0 0 0`.
2. Each write of `level.dat` after the first start writes `initialized` 1.
3. In a local world, the client plugin gives the UUID of the host to the server. The server then
   writes that UUID as `singleplayer_uuid` at each write of `level.dat`. The player file of the
   host is `players/data/<uuid>.dat` of that UUID (SAVE-PLAYER-001).
4. A server with no host writes the `singleplayer_uuid` that it read, or no key when it read none.
5. The server keeps the keys that the client wrote (`client-world-screens.md`, WORLD-WRITE-002,
   step 3). `GameType` is the default game mode of the world. A player with no player file
   enters the world in that game mode.

- **Postconditions:** after the first save of a world that the mcrs client created, the save
  directory holds these files, and no `level.dat_old`:

```text
<save>/session.lock
<save>/level.dat
<save>/data/minecraft/weather.dat
<save>/data/minecraft/game_rules.dat
<save>/data/minecraft/world_clocks.dat
<save>/data/minecraft/world_gen_settings.dat
<save>/data/mcrs/birth_clock.dat
<save>/players/data/<uuid of the host>.dat
<save>/dimensions/<namespace>/<path>/region/r.<x>.<z>.mca
```

Provenance: `design` for step 1 and for the absence of `level.dat_old` (SAVE-FILE-002).
`source_suggested` for steps 2 to 5: the game writes `initialized` 1 after the first start and
the UUID of the host at each save of a local world. Kind: file format. Confidence: medium.

## SAVE-NEW-003: A save directory with missing files

The server opens a save directory by the state of `level.dat` and `world_gen_settings.dat`:

SAVE-LEVEL-002 applies first when `level.dat` is missing and `level.dat_old` exists. The table
then uses the `level.dat` that SAVE-LEVEL-002 restores.

| `level.dat` | `world_gen_settings.dat` | The server |
| --- | --- | --- |
| present | present | opens the world. |
| present | missing | does not start. The start returns an error that names the missing file (SAVE-DEV-001). |
| missing | present | makes a new world. It reads the seed and the dimensions from the file. |
| missing | missing | makes a new world. It first writes `world_gen_settings.dat`. |

For a new world, the server writes these files before its first tick:

1. When `world_gen_settings.dat` is missing, the server writes it with the seed of its plugin
   configuration, the dimensions that the plugin gives or the dimensions of the world preset of
   the plugin, `generate_structures` 1 and `bonus_chest` 0. A configuration with no seed gives a
   random `i64`.
2. Then the server writes `level.dat` with these keys:
   - `LevelName`: the folder id of the save directory;
   - `GameType`: the default game mode of the plugin configuration;
   - `difficulty_settings`: `difficulty` `normal`, `hardcore` 0, `locked` 0;
   - `allowCommands` 0;
   - `initialized` 0. The first start of SAVE-NEW-002 then sets the spawn;
   - `Time` 0, `LastPlayed`, and the version keys of SAVE-LEVEL-001.

The order makes a stop between the two writes harmless: the next start finds a new world with its
seed, not a world with a missing file.

The default game mode of the plugin configuration applies only to a new world. A world with a
`level.dat` uses its `GameType`.

Provenance: `design`. The reason for the refusal: a world with a `level.dat` and no seed is a
damaged world, and a random seed changes its terrain at each column that the world generates
again. `source_suggested` for the keys of a new world: the vanilla server writes the name of the
world folder, the game mode of its configuration and commands off. Kind: semantic. Confidence:
medium.

## SAVE-LEVEL-002: The backup `level.dat_old`

The game writes `level.dat` with a backup: the previous `level.dat` becomes `level.dat_old`
(SAVE-NEW-001, step 4). A crash of the game between the two renames can leave a save directory
with `level.dat_old` and no `level.dat`. The server opens such a save directory as the vanilla
server does:

1. When `level.dat` does not exist and `level.dat_old` is a regular file, the server applies the
   check of `Data.DataVersion` of SAVE-VERSION-002, step 2, to `level.dat_old`. When the check
   passes, it reads `level.dat_old` as SAVE-LEVEL-001 states. The check and the read occur before
   the server renames a file.
2. When the check and the read succeed, the server renames `level.dat_old` to `level.dat`. Then
   it opens the world from that file, as SAVE-NEW-003, rows 1 and 2, state.
3. When the read fails, the server does not start. The start returns an error that names
   `level.dat_old`. The server changes no file of the save directory.
4. When the check fails, the server does not start. The start returns the error of
   SAVE-VERSION-002, step 2. The server changes no file of the save directory: `level.dat_old`
   keeps its name (SAVE-DEV-005).
5. A `level.dat` that exists and does not read is a loud error (SAVE-FILE-002). The server does
   not read `level.dat_old` in its place (SAVE-DEV-003).

- **Postconditions:** after a start on a save directory with only `level.dat_old`, the save
  directory holds `level.dat` with the content of the backup, and no `level.dat_old`. The world
  keeps its name, its time and its spawn. The server never makes a new world over the backup.

The world list of the client does not read `level.dat_old`. Such a folder shows as an unreadable
world (`client-world-screens.md`, WORLD-DIR-002, step 1).

Provenance: `source_suggested_black_box_confirmed` for the read and the rename of steps 1 and 2
and the postconditions by
`crates/mcrs_minecraft_save/tests/fixtures/vanilla/local_worlds.json`, `backup` (section Experiments, E5).
`design` for the order of step 1 and for step 4: the game renames `level.dat_old` when the read
succeeds, before it reads the version (SAVE-DEV-005). `source_suggested` for step 3. `design` for
step 5. Kind: semantic.
Confidence: medium.

## SAVE-VERSION-002: The version of `level.dat`

1. The server reads `Data.DataVersion` of `level.dat` before it reads another file of the world.
   When only `level.dat_old` exists, the server reads it from that file before it reads or
   renames another file (SAVE-LEVEL-002, step 1).
2. When `Data.DataVersion` is missing or is not `5127`, the server does not start. The start
   returns an error of its own type. The error names the save directory, the `DataVersion` that
   it found and `5127`. A caller can tell this error from another failed start.
3. The server changes no file of the save directory for such a world, except the content of
   `session.lock` of SAVE-LOCK-002, step 1. A `level.dat_old` keeps its name. The server releases
   the lock that it took.
4. A local world of the client shows the screen of a failed start with that error
   (`client-title-screen.md`, TITLE-LOAD-006, edge cases). The world list disables such a world
   before this (`client-world-screens.md`, WORLD-DIR-004, step 7), so a player meets the error
   only when the file changes after the list read it.

Provenance: `design` (SAVE-DEV-004). Kind: version policy. Confidence: high. The reason: mcrs
has no datafixer, and a `DataVersion` mismatch is a loud error.

## SAVE-VERSION-001: Exact version, no datafixer

A column whose `DataVersion` is not `5127` does not load. A region that fails to open, fails to
decode a column, or holds a column of the wrong version is quarantined (SAVE-QUARANTINE-001). The
world then generates the column again. The server does not run a datafixer. The server never converts a
column of another version. The quarantine copy keeps the original bytes before a replacement
column is written.

A saved column whose status is below `minecraft:full` reads as a missing column. The server does
not quarantine the region for it. The world can generate the column. The server does not write over
the saved bytes of that column until the world produces a full column at that position. A flush
that writes other columns of the region keeps the record of that column unchanged.

The game runs a datafixer for a file of an older version. Thus the server writes `DataVersion`
`5127` in each file that has the field. The game then reads the file with no conversion.

Provenance: `design`. Kind: version policy. Confidence: high. The reason for the partial column
rule: a vanilla save has such columns at the edge of the generated area. A quarantine of the whole
region at each load makes copies on disk and helps no one.

## SAVE-REGION-001: Region format

A region file has two 4096-byte header sectors. The first sector holds one 32-bit entry for each of
the 1024 columns. The high 24 bits are the first sector of the record. The low 8 bits are the length
of the record in sectors. The second sector holds one timestamp for each column, in seconds since the
epoch. A record is `[length: i32][compression: u8][payload]`. The `length` counts the compression
byte and the payload. Each record starts at a sector border.

The writer uses zlib (compression `2`). The reader accepts gzip (`1`), zlib (`2`), uncompressed
(`3`) and LZ4 block (`4`). The reader rejects compression `127` and every other id. The game accepts
`127` as a custom format, so the reader is stricter than the game.

A record of 256 sectors or more goes to the file `c.<x>.<z>.mcc` beside the region, where `x` and `z`
are the column coordinates. The region entry then has one sector. The compression byte of that
record has the flag `128`, and the record holds only the length and the compression byte. The `.mcc`
file holds the compressed payload. The reader reads external records and the quarantine copies them.

The region covers the columns `32 * region_x` through `32 * region_x + 31`, and the same range on the
z axis. The slot of a column is `(x & 31) + (z & 31) * 32`. The arithmetic uses floor division, so
negative coordinates follow the same rule.

Provenance: `source_suggested`. Kind: file format. Confidence: high.

## SAVE-REGION-002: Streaming and replacement

The server keeps only the two header sectors of a region in memory. It reads a record when a column
is requested. The data layer keeps at most four region files open.

A flush groups all dirty columns by region. The server writes each affected region once. A record
that is not in the flush set is copied unchanged from the source region, so a flush does not lose
other columns. The server writes the new region to a temporary file and renames it over the target.
If a write or the rename fails, the server keeps the dirty columns and writes each of them again
in the next flush. One layer owns this retry. A failed region does not stop the write of the other regions.

A column with an external payload follows this order. The server writes each new `.mcc` file to a
temporary file and renames it over the old `.mcc` file. Then the server renames the new region
file over the target. The format has no way to commit both in one step. If the region rename fails,
the old record still refers to the `.mcc` file, and that file holds the new payload. A read in
this state returns the new payload of the same column, or, when its decode fails, follows
SAVE-QUARANTINE-001. The column stays dirty, and the next flush repairs the state.

Provenance: `design`. Kind: durability. Confidence: high.

## SAVE-CHUNK-001: The layout of a column record

The payload of a record is one NBT compound. Every key of the record has a type in this rule. The
reader fails the load of a key that this rule does not list (SAVE-FILE-002). The writer writes a
key again with the value that the reader read. One exception exists in a column record:
the compounds in `block_entities` (see "Block entities" below).

The game writes the root keys of the first table, and it reads each of them with the default of the
table. The server reads a column only when its `status` is `minecraft:full` (SAVE-VERSION-001). A key
that the game writes only for a status below `minecraft:full` is still in the table, because the
format has it.

| Root key | Type | Default when absent | Written |
| --- | --- | --- | --- |
| `DataVersion` | int | required by the server (SAVE-VERSION-001) | always |
| `xPos`, `zPos` | int | 0 | always |
| `yPos` | int | the game does not read it | always |
| `status` | string | none: the column reads as missing | always |
| `LastUpdate` | long | 0 | always |
| `InhabitedTime` | long | 0 | always |
| `isLightOn` | byte | false | only when true |
| `sections` | list of compound | empty | always |
| `block_entities` | list of compound | empty | always |
| `entities` | list of entity save compounds | empty | only for a status below `minecraft:full` |
| `Heightmaps` | compound | empty | always |
| `block_ticks`, `fluid_ticks` | list of compound | empty | always |
| `PostProcessing` | list of list of short | empty | always |
| `structures` | compound | empty | always |
| `UpgradeData` | compound | empty | only when it holds data |
| `blending_data` | compound | none | only when the column blends with old terrain |
| `retrogen` | compound | none | only when the column needs a retrogen |

A record whose `xPos` and `zPos` do not equal the column position of its slot in the region is an
unreadable column (SAVE-QUARANTINE-001). Provenance: `design`. Reason: a record at the wrong slot
must not land at another position.

A record whose `entities` list is not empty is an unreadable column (SAVE-QUARANTINE-001). An empty
or absent list reads as no entity. Provenance: `design`. Reason: an entity save compound has no typed
layout in the server (SAVE-LIMITS-001), and the server must not drop the entities of a column in
silence.

The key `yPos` is the lowest section index of the dimension, for example `-4`. The key `status` has
a lowercase first letter. The key `isLightOn` is false or missing when the game must light the
column again. An entry of `entities` has the layout of an entity save compound. The game writes
`entities` only while a column is below full. A full column keeps its entities in the `entities`
directory of the dimension (SAVE-LIMITS-001).

A section compound has these keys:

| Section key | Type | Default when absent | Written |
| --- | --- | --- | --- |
| `Y` | byte | 0 | always |
| `block_states` | compound | one palette entry `minecraft:air` | when the section has blocks |
| `biomes` | compound | one palette entry `minecraft:plains` | when the section has blocks |
| `noise_biomes` | compound | none | only for the statuses `minecraft:noise_biomes`, `minecraft:biomes` and `minecraft:terrain` |
| `BlockLight` | byte array of 2048 | none | only when the light is not empty |
| `SkyLight` | byte array of 2048 | none | only when the light is not empty |

A section compound can hold only `Y` and light. This is the case for a section whose `Y` is outside
the section range of the dimension. The game writes no empty section compound.

The compounds `block_states`, `biomes` and `noise_biomes` have the list `palette` and the long array
`data`. The key `palette` is required. The key `data` is missing when the palette has one entry.
A palette entry of `block_states` is a string or a compound. The string is a block id, and it
stands for the default state of that block. The compound has the string `id` and the compound
`properties` with string values. The key `id` is required. The reader accepts both forms. A missing
`properties` gives the default state of the block.

The writer writes the string when the state is the default state of its block. Else the writer
writes the compound, with all properties of the block in `properties`. World version 5006 renamed
`Name` to `id` and `Properties` to `properties`. A palette compound with `Name` or `Properties`
comes from an older world version, and SAVE-VERSION-001 rejects the column.

A palette entry of `biomes` and of `noise_biomes` is a biome id.

The array `data` holds the index of the palette entry for each cell, packed with no value across two
longs. A block state container has 4096 cells in the order `y * 256 + z * 16 + x`. A biome container
has 4096 cells of one block, in the order `y * 256 + z * 16 + x`. A noise biome container has 64
cells of 4 by 4 by 4 blocks, in the order `y * 16 + z * 4 + x`. `section-biomes.md`, BIO-SEC-001,
states the two biome containers. Let `n` be the palette size. The
`data` field is missing when `n` is 1. Else the bits for each cell are `ceil(log2(n))`. The minimum
is 4 for block states. Biome containers and noise biome containers have no minimum. The game rejects a
`data` array whose length does not match the bits that follow from `n`.

`Heightmaps` has one long array for each heightmap type that the column carries. The keys are
`WORLD_SURFACE_WG`, `WORLD_SURFACE`, `OCEAN_FLOOR_WG`, `OCEAN_FLOOR`, `MOTION_BLOCKING` and
`MOTION_BLOCKING_NO_LEAVES`. A full column has `WORLD_SURFACE`, `OCEAN_FLOOR`, `MOTION_BLOCKING`
and `MOTION_BLOCKING_NO_LEAVES`, and no other key. The game never writes a `_WG` key in a full
column. A column below full has the two `_WG` keys. From the status `minecraft:terrain` it can
also have the four other keys. The reader accepts the six keys in every column. For a full column
the server ignores a `_WG` key, and the writer does not write it. A full column that does not have
one of the four keys, or has an array of the wrong length, is a load error (SAVE-QUARANTINE-001).
Provenance of this sentence: `design`, after decision #388; the server does not compute a missing
heightmap again. A heightmap is a
long array of 256 values, with the index `x + z * 16`. The value is the lowest `y` above the highest
matching block, minus the lowest `y` of the dimension. The bits for each value are
`ceil(log2(height + 1))`, where `height` is the height of the dimension. The packing is the same as
the packing of `data`.

`PostProcessing` has one list of shorts for each section of the dimension. An empty list means that
the section has no block to process. A short packs the position of a block in the section.

`structures` has the compounds `starts` and `References` (SAVE-COLUMN-003).

`UpgradeData` has these keys:

| Key | Type | Default when absent | Written |
| --- | --- | --- | --- |
| `Indices` | compound of int arrays | empty | only when one index is not empty |
| `Sides` | byte | 0 | always |
| `neighbor_block_ticks` | list of scheduled ticks | empty | only when not empty |
| `neighbor_fluid_ticks` | list of scheduled ticks | empty | only when not empty |

The keys of `Indices` are section numbers as decimal strings. Each value is an int array. A
scheduled tick has the layout of SAVE-TICK-001. `Sides` is a bit set of the eight horizontal
directions.

`blending_data` has the int `min_section` and the int `max_section`, both required. It has the
optional list `heights` of 16 floats.

`retrogen` has these keys:

| Key | Type | Default when absent |
| --- | --- | --- |
| `target_status` | string, not `minecraft:empty` | required |
| `statuses_to_rerun` | list of string, none `minecraft:empty` | required |
| `has_below_zero_retrogen` | byte | false |
| `missing_bedrock` | long array, a bit set | none |

Block entities: a compound in `block_entities` has the string `id`, the ints `x`, `y` and `z` (absolute
block coordinates), the optional compound `components`, and the keys of its block entity type. The
reader keeps the whole compound as written. The reader checks only `id`, `x`, `y` and `z`. This is
the one exception to the typed layout, and it has this reason. A typed codec for each block entity
type is a separate body of work. Issue #738 adds typed block entities. Until then the compound of a block entity stays opaque and the writer writes it as it
read it. The keys of a column other than the contents of these compounds are all typed.

Provenance: `source_suggested` for the layout, `design` for the exception of the block entities.
Kind: file format. Confidence: high.

## SAVE-TICK-001: Scheduled ticks

A scheduled tick of a block or a fluid is an NBT compound in `block_ticks` or `fluid_ticks`:

- `i`: the registry id of the block or the fluid, for example `minecraft:water`;
- `x`, `y`, `z`: the absolute block position, as ints;
- `t`: the delay in ticks, as an int. It is the trigger time minus the game time of the save;
- `p`: the priority, as an int from -3 (extremely high) through 3 (extremely low). 0 is normal.

All six keys are required, and the game writes all six. The game limits a `p` outside -3 through 3
to the nearest end of that range. The game ignores a key that is not in this list.

On load, the trigger time is the game time of the load plus `t`. Thus a tick that waits for 10 ticks
when the column is saved waits for 10 ticks after the column loads. The game time of the save is
`LastUpdate`. A tick whose position is not inside the column is dropped on load. Two ticks with the
same block and the same position are one tick.

The game reads each entry alone. It logs an entry with a missing key, a key of the wrong type or an
unknown block or fluid id. It skips that entry and keeps the other entries. The server does not
copy this. An entry with a missing key, a key of the wrong type, an unknown id or a key that this
rule does not list fails the load of the column (SAVE-FILE-002).

Provenance: `source_suggested` for the layout, `design` for the load error. Kind: file format.
Confidence: high for the layout, medium for the skip of the game.

## SAVE-COLUMN-001: Stored column

A stored column carries:

- the column position and `min_section_y`;
- one entry for each section: block states, biomes, block light and sky light;
- the chunk status, which is `minecraft:full` when a live column is saved;
- `is_light_on`, `inhabited_time` and `last_update`;
- the heightmaps that the column carries, each as a typed array;
- block entities: typed block entities, and block entities whose compound stays as written (the
  exception of SAVE-CHUNK-001);
- block ticks and fluid ticks, each with its delay and its priority;
- structure starts and references, with typed pieces;
- the other keys of SAVE-CHUNK-001: `PostProcessing`, `UpgradeData`, `blending_data` and `retrogen`.

The live save path writes no light and no `isLightOn` key. A dimension that loads such a column
lights it again from the blocks. The data layer can carry light, and the region round-trip tests
cover it.

Provenance: `design`. Kind: semantic. Confidence: high. SAVE-CHUNK-001 states the layout.

## SAVE-COLUMN-002: What a cold write keeps

A column that the server unloads is written when at least one of these is true:

- at least one section came from the data layer or changed since it landed;
- a scheduled block or fluid tick changed since the column landed;
- every section was generated and the light is final.

A column that came back from the data layer unchanged is not written again. A generated column whose
light never settled is not written. The next load generates it again with settled light. The
scheduled ticks leave the simulation queues on every cold path, also when the column is not written.

A change to a block entity after its section landed marks the section dirty. A block entity that is
created in the same tick as the section lands does not mark the section, because the section is
already new. A save-all request writes every loaded column that this server owns, whether or not it
is marked.

Every `6000` host ticks, the dimension writes the loaded columns that have a changed section or
changed scheduled ticks. The write copies their scheduled ticks and does not take them out of the
queues, so the world keeps running. It clears the section marks of the columns that it writes. A
save-all still writes every loaded column and takes the ticks out. It runs at shutdown only, when
the dimension stops, so this is safe.

Provenance: `design`. Kind: semantic. Confidence: high.

## SAVE-COLUMN-003: Structure state

A stored column has a map of structure starts and a map of structure references. Both maps use the
structure id as the key. A start carries the claim count, the start column and the NBT of every
piece. A reference entry carries the start columns whose structure reaches into this column.

The NBT layout is:

```text
structures: {
  starts: {
    "<structure id>": {
      ChunkX: int
      ChunkZ: int
      references: int
      Children: [piece NBT, ...]
    }
  }
  References: {
    "<structure id>": long array of packed start columns
  }
}
```

A start has the ints `ChunkX`, `ChunkZ` and `references` and the list `Children` of piece
compounds. The game writes all four keys for each start. The game reads a missing `ChunkX`,
`ChunkZ` or `references` as 0, and a missing `Children` as an empty list. A start with no readable
piece is dropped, and an empty `Children` has no readable piece. A `References` entry is a long array. The
game writes `starts` and `References` in every column, also when they are empty.

A start column is packed as `(x as u32 as i64) | ((z as i64) << 32)`. A `References` entry is not
written when it is empty.

The field `references` of a start is the number of claims. A lookup that claims a start increases
it. The lookups of an exploration map claim a structure. Thus a start with `references` of 1 or more is
not a target of a new exploration map. A start that the generator creates has `references` 0. The
count is not the number of columns that hold a reference. The server keeps the value that it reads
and writes it back.

A column holds a reference to a start when two conditions are true. The horizontal bounding box of
the start touches the area of the column. The start column is at most 8 columns away on each axis. The generator writes
the start into its own column and the reference into each column that it touches. The start column
holds a reference to itself when the box touches it.

Each piece compound has these keys. The keys of the first table are in every piece:

| Key | Type | Default when absent |
| --- | --- | --- |
| `id` | string, a piece type id in lowercase | required |
| `BB` | int array of six: `minX`, `minY`, `minZ`, `maxX`, `maxY`, `maxZ` | required |
| `O` | int, the horizontal direction (0 south, 1 west, 2 north, 3 east), or -1 for none | 0 |
| `GD` | int, the depth of generation | 0 |

The reader types every other key of a piece by the table of its piece type. A piece with an `id`
that no table lists, or with a key outside its table, fails the load (SAVE-FILE-002). The game skips
such a piece. The game also skips a piece that it cannot read, and it drops a start that has no
readable piece.

The key `D` is a byte: 0 south, 1 west, 2 north, 3 east. The writer writes a byte. The game reads a
number of any type, and it uses the absolute value of the remainder of the division by 4.

The enumeration values are strings in upper case: a rotation is `NONE`, `CLOCKWISE_90`,
`CLOCKWISE_180` or `COUNTERCLOCKWISE_90`. A mirror is `NONE`, `LEFT_RIGHT` or `FRONT_BACK`.

| Piece type ids | Keys after the common keys |
| --- | --- |
| `mscorridor` | `hr`, `sc`, `hps` (byte, false), `Num` (int, 0), `MST` (int, 0) |
| `mscrossing` | `tf` (byte, false), `D` (byte, 0 through 3, default south), `MST` (int, 0) |
| `msroom` | `Entrances` (list of `BB` arrays, empty), `MST` (int, 0) |
| `msstairs` | `MST` (int, 0) |
| `nebcr`, `nebs`, `neccs`, `nectb`, `nece`, `nescsc`, `nesc`, `necsr`, `nerc`, `nesr`, `nestart` | none |
| `nebef` | `Seed` (int, 0) |
| `nesclt`, `nescrt` | `Chest` (byte, false) |
| `nemt` | `Mob` (byte, false) |
| `shcc` | `EntryDoor`, `Chest` (byte, false) |
| `shfc` | `EntryDoor`, `Steps` (int, 0) |
| `sh5c` | `EntryDoor`, `leftLow`, `leftHigh`, `rightLow`, `rightHigh` (byte, false) |
| `shlt`, `shrt`, `shph`, `shssd` | `EntryDoor` |
| `shli` | `EntryDoor`, `Tall` (byte, false) |
| `shpr` | `EntryDoor`, `Mob` (byte, false) |
| `shrc` | `EntryDoor`, `Type` (int, 0) |
| `shsd`, `shstart` | `EntryDoor`, `Source` (byte, false) |
| `shs` | `EntryDoor`, `Left`, `Right` (byte, false) |
| `tejp` | `Width`, `Height`, `Depth`, `HPos` (int, 0), `placedMainChest`, `placedHiddenChest`, `placedTrap1`, `placedTrap2` (byte, false) |
| `tedp` | `Width`, `Height`, `Depth`, `HPos` (int, 0), `hasPlacedChest0` through `hasPlacedChest3` (byte, false) |
| `tesh` | `Width`, `Height`, `Depth`, `HPos` (int, 0), `Witch`, `Cat` (byte, false) |
| `btp` | none |
| `omb`, `omcr`, `omdxr`, `omdxyr`, `omdyr`, `omdyzr`, `omdzr`, `omentry`, `ompenthouse`, `omsimple`, `omsimplet`, `omwr` | none in the game. `omb` can carry `mcrs:rooms` and `mcrs:children`. |
| `iglu`, `nefos` | `TPX`, `TPY`, `TPZ` (int, 0), `Template` (string, empty), `Rot` (rotation, required) |
| `shipwreck` | `TPX`, `TPY`, `TPZ`, `Template`, `Rot` as `iglu`, `isBeached`, `height_adjusted` (byte, false) |
| `orp` | `TPX`, `TPY`, `TPZ`, `Template`, `Rot` as `iglu`, `Integrity` (float, 0), `BiomeType` (`WARM` or `COLD`, required), `IsLarge` (byte, false) |
| `ecp` | `TPX`, `TPY`, `TPZ`, `Template`, `Rot` as `iglu`, `OW` (byte, false) |
| `wmp` | `TPX`, `TPY`, `TPZ`, `Template`, `Rot` as `iglu`, `Mi` (mirror, required) |
| `rupo` | `TPX`, `TPY`, `TPZ` (int, 0), `Template` (string, empty), `Rotation` and `Mirror` (required), `VerticalPlacement` (required), `Properties` (required) |
| `jigsaw` | `PosX`, `PosY`, `PosZ` (int), `ground_level_delta` (int), `pool_element` (compound), `rotation` (required), `junctions` (list, empty), `liquid_settings` (string) |

The game writes `TPX`, `TPY`, `TPZ` and `Template` in each piece that has them. It reads a missing
key as the default of the table.

`EntryDoor` is `OPENING`, `WOOD_DOOR`, `GRATES` or `IRON_DOOR`, and it is required. The piece type ids
carry the namespace `minecraft:` in the file. The ids `omb` through `omwr` name the monument
pieces. The piece `tesh` is a swamp hut, `tedp` a desert pyramid and `tejp` a jungle temple.

The compound `Properties` of `rupo` has the bytes `cold`, `air_pocket`, `overgrown`, `vines` and
`replace_with_blackstone`, and the float `mossiness`. All six keys are required. `VerticalPlacement`
is `on_land_surface`, `partly_buried`, `on_ocean_floor`, `in_mountain`, `underground` or `in_nether`.

A junction of `jigsaw` is a compound with the ints `source_x`, `source_ground_y`, `source_z` and
`delta_y` (default 0), and the string `dest_proj` (`terrain_matching` or `rigid`). The string
`liquid_settings` is `apply_waterlogging` or `ignore_waterlogging`. The game omits it when the
value is the default of the structure. The compound `pool_element` has the string `element_type`
and the keys of that type in the data pack format of a template pool element: `single_pool_element`,
`legacy_single_pool_element`, `list_pool_element`, `feature_pool_element` and `empty_pool_element`.
The key `processors` follows SAVE-POOL-001, the key `feature` follows SAVE-POOL-002, and the use
of both after a load follows SAVE-POOL-003. These rules apply also to each element in the list
`elements` of a `list_pool_element`.

A start with no pieces, an id that is not a resource location, and a reference more than 8 columns
from the column are dropped. The rest of the column loads.

A mineshaft corridor piece has the booleans `hr`, `sc` and `hps`, and the int `Num`. The field `hps`
is true after the piece placed its cave spider spawner. The spawner is a typed `MobSpawner` block
entity of its column. It is saved with the block entities of that column. The game does not place a
second spawner when `hps` is true.

The vanilla monument piece does not store its room graph. The game builds the graph again from the
world seed and the start column when it loads the start. The server adds two optional keys to the
compound of the piece `omb`: `mcrs:rooms` and `mcrs:children`. The game ignores both keys. They are
missing in a vanilla save.

`mcrs:rooms` is a list of compounds, one for each room of the graph. A room has the int `index`,
the byte array `openings` of six values and the int array `connections` of six values. The
six values follow the order down, up, north, south, west, east. A value of `openings` is 1 for an
open face. A value of `connections` is the index of the room past that face, or -1 for none.

`mcrs:children` is a list of compounds, one for each room piece. A child has the string `kind`,
the int array `bounds` of six values like `BB`, the int `room` and the int `design`. `kind` is
`entry`, `core`, `double_x`, `double_xy`, `double_y`, `double_yz`, `double_z`, `simple`,
`simple_top`, `wing` or `penthouse`. The key `room` is present for each kind except `wing` and
`penthouse`. The key `design` is present for `simple` and `wing`.

When a saved monument start has no `mcrs:rooms`, the server builds the room graph from the world
seed one time. It does this at the first load of that start. It writes `mcrs:rooms` at the next
save of the column that holds the start. The built graph must equal the graph that generation of the
same start produces.

Provenance: `source_suggested` for the layout and the claim count, `design` for `mcrs:rooms`, for
`mcrs:children` and for the rebuild of the room graph. The reason for the rebuild: a vanilla save
must give the same monument as generation gives, and the save has no room data. Kind: file format and semantic. Confidence: high.

## SAVE-POOL-001: The processor list of a saved pool element

The key `processors` of a `single_pool_element` or a `legacy_single_pool_element` in a saved piece
has one of these three forms:

| Form | Example | Meaning |
| --- | --- | --- |
| A string, a registry id | `"minecraft:mossify_10_percent"` | The entry of `worldgen/processor_list` with that id |
| A compound with the list `processors` | `{processors: []}` | An inline processor list |
| A list | `[]` | An inline processor list |

Each entry of an inline list is a processor compound in the data pack form of a processor list.
The server reads each of the three forms. An inline list is not a load error.

The server writes the form that the pool element of the data pack has. For a registry id, it
writes that id. For an inline list in the data pack, it writes the compound with the list
`processors`. It writes this compound also when the data pack has the list form. The game does the
same. Example: the pools of `abandoned_camp` have the inline list `{"processors": []}`. The game
writes `processors: {processors: []}` into the `pool_element` of each piece of these pools.

A piece that the server reads from a save and writes again keeps its value. An id stays the same
id. The entries of an inline list stay the same, in the same order. An inline list that the save
holds in the list form is written in the compound form. The game reads both forms.

Provenance: `source_suggested_black_box_confirmed` for the compound form of an empty inline list,
which the pieces of `minecraft:abandoned_camp_forest` in
`crates/mcrs_minecraft_worldgen_generator/src/tests/fixtures/structure_pieces.bin` show.
`source_suggested` for the read of the list form, for a list with entries and for the write of a
piece that the server read. Kind: file format. Confidence: high.

## SAVE-POOL-002: The placed feature of a saved pool element

The key `feature` of a `feature_pool_element` in a saved piece has one of these two forms:

| Form | Meaning |
| --- | --- |
| A string, a registry id | The entry of `worldgen/placed_feature` with that id |
| A compound | An inline placed feature in its data pack form, with the keys `feature` and `placement` |

The key `feature` of an inline placed feature is a registry id of a configured feature or an inline
configured feature. The server reads both forms of the placed feature. An inline placed feature is
not a load error.

The server writes the form that the pool element of the data pack has, as SAVE-POOL-001 states for
a processor list. A piece that the server reads from a save and writes again keeps its value. The
pools of the vanilla data pack name each placed feature by registry id. An inline placed
feature comes only from a data pack that adds it.

Provenance: `source_suggested`. Kind: file format. Confidence: high.

## SAVE-POOL-003: The use of a saved pool element after a load

A saved pool element is complete. The server does not search the pools of the data pack for an
element that is equal to it. A saved element with no equal element in the current pools is not an
error. The piece uses the saved element when it places blocks in a column after the load. Until the
server implements this rule, a saved element with no equal element in the loaded pools fails the
column load.

- A registry id names the entry of the current registries. When a data pack changes that entry
  between two runs, the piece uses the new content.
- An inline processor list or an inline placed feature is the value of the save. A change of the
  pool in the data pack does not change it.

A registry id that names no entry of the current registries is a load error of the column. The
error names the key and the id. An inline value that does not read as a processor list or a placed
feature in the data pack form is a load error of the column, and the error names the key. The game
skips such a piece and keeps the other pieces of the start (SAVE-COLUMN-003).

Provenance: `source_suggested` for the use of the saved value. `design` for the load error, by
decision #388 (SAVE-FILE-002). Kind: semantic. Confidence: high.

## SAVE-COLUMN-004: Write order

A cold column is put into the data layer while its sections still hold their blocks. The data layer
keeps it until a flush. A read sees the dirty column before the column on disk. Thus a column that
cooled and loads again in the same run does not read the older region record.

A save-all request is serviced after the heightmaps are updated, at the end of the tick of the
dimension. It snapshots every loaded column that this server owns, without light. It takes the
scheduled ticks out of the simulation, sets `last_update` to the current game time, and puts each
column into the data layer. The host then flushes the data layer of every dimension and waits for
the answers. The region rewrite happens in that flush.

The server asks for a save-all one time during the shutdown. Every 6000 host ticks, the host
autosave tick, the periodic write of SAVE-COLUMN-002 runs instead, and the world keeps running. The
first tick of a world does not count as an autosave tick.

Provenance: `design`. Kind: semantic. Confidence: high.

## SAVE-QUARANTINE-001: Quarantine

A region is quarantined in one of three cases. It cannot be opened. A column in it cannot be decoded.
A column has the wrong `DataVersion`. A column with a status below `minecraft:full` does not cause a
quarantine (SAVE-VERSION-001). The server copies the whole region to
`<save>/quarantine/<namespace>/<path>/r.<x>.<z>/attempt-<n>/`. It copies each external `.mcc` file
that the records use. The file `report.txt` holds the reason. The attempt number grows until an
unused directory is found.

If the copy succeeds, the column reads as missing and the world can generate a replacement. If the
copy fails, the read returns an unreadable answer and the column is never written over. A region that
does not exist reads as missing with no quarantine. A region is quarantined at most one time in a
run. After a successful replacement write, the server clears the in-memory quarantine mark.

If the original region cannot be opened at all, the replacement starts a new region file after the
copy. If it can be opened, the replacement rewrites the damaged record and keeps the other records.

Provenance: `design`. Kind: durability. Confidence: high.

## SAVE-SHUTDOWN-001: Shutdown

A shutdown request does these steps:

1. It marks the server for shutdown. It disconnects each connection, also the connections that
   arrive later.
2. It asks the dimension coordinator for a save-all.
3. It waits until no connection and no player that departs is left, and until the data layer has
   flushed every dimension.
4. It writes `level.dat`, the weather, the game rules and the world clocks. Then it releases the
   lock of the save directory (SAVE-LOCK-002) and exits.

The chunk save happens before the write of the host state. A player that departs writes its player
file before the shutdown completes. The server waits for each player that departs. The embedder handles a second signal. The server then exits
at once with no other save, and the operating system releases the lock.

The exit status is success only when every write of steps 2 to 4 and every player file succeeded.
A write error is logged, does not block the exit, and makes the exit status an error. Provenance:
`design`. Reason: an operator must see a lost save in the exit status, not only in the log.

Provenance: `design`. Kind: semantic. Confidence: high.

## SAVE-PLAYER-001: Player data

A player file is `players/data/<uuid>.dat`. Its root compound is the player. The reader fails the
load of a root key that the tables of this rule do not list (SAVE-FILE-002). The game reads each
key with the default of the table, so a key that is missing does not stop the game.

Every key has a type. A key that mcrs has no behaviour for is still typed. The server reads such a
key and writes it back with the value that it read. Thus the vanilla game gets back each value
that it saved.

Notation: "3 doubles" is a list of three doubles. "UUID" is an int array of four. "Position" is an
int array of three. "Text" is a text component (`text-component.md`). "Entity save compound" is a
compound with the string `id` of an entity type, the entity keys and the living entity keys below,
and the keys of that entity type. The keys of an entity type are not in this document.

### Entity keys

| Key | Type | Default when absent | Written |
| --- | --- | --- | --- |
| `Pos` | 3 doubles | 0, 0, 0; a join with no `Pos` uses the position of the paragraph "Default position" | always |
| `Motion` | 3 doubles | 0, 0, 0 | always |
| `Rotation` | list of two floats: yaw, pitch | 0, 0; a join with no `Rotation` uses the yaw and pitch of `spawn` | always |
| `fall_distance` | double | 0 | always |
| `Fire` | short | 0 | always |
| `Air` | short | 300 | always |
| `OnGround` | byte | false | always |
| `Invulnerable` | byte | false | always |
| `PortalCooldown` | int | 0 | always |
| `invulnerable_time` | int | 0 | only when more than 0 |
| `UUID` | UUID | the UUID of the profile | always |
| `CustomName` | Text | none | only when set |
| `CustomNameVisible` | byte | false | only when true |
| `Silent` | byte | false | only when true |
| `NoGravity` | byte | false | only when true |
| `Glowing` | byte | false | only when true |
| `TicksFrozen` | int | 0 | only when more than 0 |
| `HasVisualFire` | byte | false | only when true |
| `Tags` | list of string, at most 1024 | empty | only when not empty |
| `data` | compound, custom data | empty | only when not empty |
| `Passengers` | list of entity save compounds | empty | only when the entity has passengers |
| `Team` | string | none | never: the game reads it and does not write it |

The game limits `Pos` to 30000512 on the x and z axes and to 20000000 on the y axis. It sets a
`Motion` value to 0 when its absolute value is more than 10. A `Pos` or `Rotation` that is not
finite is an error. For a player the game replaces `UUID` with the UUID of the profile.

Default position: a join reads `Dimension`, `Pos` and `Rotation` of the player file one by one.
Each missing key takes its own default. A missing `Dimension` gives the dimension of `spawn`. When
the server does not have that dimension, the join uses the overworld. A missing `Rotation` gives
the yaw and pitch of `spawn`. A missing `Pos` gives the result of the spawn placement
(`player-death.md`, PLD-RSP-005) in the dimension that the join uses. The start block `S` is the
`S` of PLD-RSP-004, with the world border of that dimension (SAVE-DEV-006). Thus a player with no
player file gets the position of a respawn with no respawn point. The x and z of the position are
those of a block plus 0.5, and the y is a whole number.

With the default `spawn` of SAVE-LEVEL-001, `S` is `0 0 0` in the overworld. The default game
mode of the world then gives these positions:

- survival or creative: a surface position at most `respawn_radius` columns from x 0 and z 0. When
  no candidate is valid, it is the height fix of `0 0 0`;
- adventure: the height fix of `0 0 0`;
- spectator: `0.5 0 0.5`.

The join finds the position before the player enters the play phase. The server keeps the
connection in the configuration phase until the spawn placement ends. The server loads or
generates the columns that the spawn placement reads (PLD-RSP-007). The first position that the client receives is
the result. The server sends no other position before it.

### Living entity keys

| Key | Type | Default when absent | Written |
| --- | --- | --- | --- |
| `Health` | float | the maximum health | always |
| `HurtTime`, `DeathTime` | short | 0 | always |
| `AbsorptionAmount` | float | 0 | always |
| `attributes` | list of attribute compounds | empty | always, when the player has attribute instances |
| `active_effects` | list of effect compounds | empty | only when not empty |
| `FallFlying` | byte | false | always |
| `sleeping_pos` | Position | none | only when the player sleeps |
| `Brain` | compound | no memory | always |
| `last_hurt_by_player` | UUID | none | only when set |
| `last_hurt_by_player_memory_time` | int | 0 | with `last_hurt_by_player` |
| `last_hurt_by_mob` | UUID | none | only when set |
| `ticks_since_last_hurt_by_mob` | int | 0 | with `last_hurt_by_mob` |
| `equipment` | compound | empty | when a slot is not empty (see below) |
| `locator_bar_icon` | compound | the default icon | only when it differs from the default |
| `current_explosion_impact_pos` | 3 doubles | none | only when set |
| `current_impulse_context_reset_grace_time` | int | 0 | always |

An attribute compound has the string `id` (required), the double `base` (default 0) and the list
`modifiers` (default empty). A modifier is a compound with the string `id`, the double `amount` and
the string `operation`. All three are required. `operation` is `add_value`, `add_multiplied_base`
or `add_multiplied_total`. ATR-SAV-002 of `attributes.md` states the load of a compound with an
unknown attribute, an unknown key, a bad field, or two modifiers with one `id`.

The key `attributes` is written always for a player that has attribute instances. The list is empty
when no instance differs from the entity type (ATR-SAV-001 of `attributes.md`). A host without the
attribute plugin gives a player no attribute instances and writes no key `attributes`. A file that
holds the list keeps it: the load gives the player the instances. Provenance `design`: the engine
does not force attributes on a host. The owner has not confirmed this decision.

An effect compound has the string `id` (required), the ubyte `amplifier` (default 0), the int
`duration` (default 0), the byte `ambient` (default false) and the byte `show_particles` (default
true). It has the byte `show_icon` (default: the value of `show_particles`). It has the optional
compound `hidden_effect`, with the same keys except `id`. The effect of `hidden_effect` has the
`id` of its parent.

The compound `Brain` has the compound `memories`. A player has no memory type, so `memories` is
empty. The key of an entry is a memory type id.

The compound `locator_bar_icon` has the string `style` (default `minecraft:default`) and the
optional int `color`, an RGB value.

### Player keys

| Key | Type | Default when absent | Written |
| --- | --- | --- | --- |
| `DataVersion` | int | none | always. `5127`. |
| `Dimension` | string | the dimension of `spawn` | always |
| `playerGameType` | int | the default game mode of the world | always |
| `previousPlayerGameType` | int | none | only when set |
| `abilities` | compound | the defaults below | always |
| `foodLevel`, `foodTickTimer` | int | 20, 0 | always |
| `foodSaturationLevel` | float | 5 | always |
| `foodExhaustionLevel` | float | 0 | always |
| `XpLevel`, `XpTotal`, `Score` | int | 0 | always |
| `XpP` | float | 0 | always |
| `XpSeed` | int | a random int, when 0 or missing | always |
| `Inventory` | list of slot stacks | empty | always |
| `SelectedItemSlot` | int | 0 | always |
| `EnderItems` | list of slot stacks | empty | always |
| `SleepTimer` | short | 0 | always |
| `LastDeathLocation` | compound | none | only when set |
| `respawn` | compound | none | only when set |
| `seenCredits` | byte | false | always |
| `spawn_extra_particles_on_fall` | byte | false | always |
| `raid_omen_position` | Position | none | only when set |
| `entered_nether_pos` | 3 doubles | none | only when set |
| `last_explosion_impact_pos` | 3 doubles | none | only when set |
| `warden_spawn_tracker` | compound | all fields 0 | always |
| `recipeBook` | compound | empty book, all settings false | always |
| `post_effects` | list of string | empty | always |
| `ShoulderEntityLeft` | entity save compound | none | only when not empty |
| `ShoulderEntityRight` | entity save compound | none | only when not empty |
| `RootVehicle` | compound | none | only when the player rides a vehicle |
| `ender_pearls` | list of compound | empty | only when not empty |

The compound `abilities` has seven keys. The game writes all seven keys. The reader uses the
default of the table for each missing key.

| Key | Type | Default when absent |
| --- | --- | --- |
| `invulnerable` | byte | false |
| `flying` | byte | false |
| `mayfly` | byte | false |
| `instabuild` | byte | false |
| `mayBuild` | byte | true |
| `flySpeed` | float | 0.05 |
| `walkSpeed` | float | 0.1 |

When mcrs has no behaviour that changes an ability, the server writes back the value that it read.

The compound `LastDeathLocation` has the string `dimension` and the Position `pos`. Both are
required. The compound `respawn` has the same two keys. It has the floats `yaw` (-180 through 180)
and `pitch` (-90 through 90), both required, and the byte `forced` (default false).

The compound `warden_spawn_tracker` has the non-negative ints `ticks_since_last_warning`,
`warning_level` and `cooldown_ticks`. The default of each is 0. The game writes all three.

The compound `recipeBook` has the list of string `recipes` and the list of string `toBeDisplayed`.
Both are required, and each entry is a recipe id. It has eight bytes with the default false:
`isGuiOpen`, `isFilteringCraftable`, `isFurnaceGuiOpen`, `isFurnaceFilteringCraftable`,
`isBlastingFurnaceGuiOpen`, `isBlastingFurnaceFilteringCraftable`, `isSmokerGuiOpen` and
`isSmokerFilteringCraftable`. The game writes all ten keys. On load the game drops each recipe id
that no recipe has.

An entity save compound is the second exception to the typed layout (SAVE-FILE-002), for the same
reason as block entities: its keys depend on the entity type, and no rule states them. The
reader checks only the string `id`, keeps the whole compound as written, and the writer writes it
as it read it. This applies to `ShoulderEntityLeft`, `ShoulderEntityRight`, `RootVehicle.Entity`,
each entry of `ender_pearls` and each entry of `Passengers` inside `RootVehicle.Entity`. An entry
of `Passengers` is an entity save compound, and it can hold its own `Passengers` at any depth. All
these entries stay opaque. Issue #738 adds typed entity layouts. Provenance: `design`.

The compound `RootVehicle` has the UUID `Attach` and the entity save compound `Entity`. `Entity` is
the root vehicle. `Attach` is the UUID of the entity that the player rides. It is the root vehicle
or one of its passengers. An entry of `ender_pearls` is an entity save compound with the extra
string `ender_pearl_dimension`, the dimension of the pearl. The game drops an entry that has no
dimension.

A slot stack is an item stack with the ubyte `Slot` (default 0). An item stack is a compound with
the string `id`, the int `count` from 1 through 99 (default 1) and the optional compound
`components`. The keys of `components` follow `item-stack.md`. The list holds only the slots that
are not empty. In `Inventory`, the slots 0 through 8 are the hotbar. The slots 9 through 35 are the
main inventory. The game drops a stack with a `Slot` outside 0 through 35 from `Inventory`. In
`EnderItems`, `Slot` is 0 through 26.

The armor, the off hand and the other equipment are not in `Inventory`. They are in the compound
`equipment`. It has the keys `mainhand`, `offhand`, `feet`, `legs`, `chest`, `head`, `body` and
`saddle`. Each key holds an item stack with no `Slot`. A key is missing when its slot is empty. A
player file can carry `offhand`, `feet`, `legs`, `chest`, `head`, `body` and `saddle`. The game
never writes `mainhand` for a player, because the main hand is the selected slot of `Inventory`.
The server reads `mainhand` without an error, and it ignores the stack. The game writes `equipment` when a slot is not empty or when the selected slot holds
a stack. In the second case the compound can be empty.

The registry of components decides which components the server persists. A component that is not
registered is not written. A component whose codec fails is not written, and the error is logged.

The server snapshots a player on the autosave tick and writes the snapshot on the IO task pool. A
player that leaves is snapshotted before the departure. The shutdown flushes the save queue before
the exit. A player file that is missing gives the default player. An unreadable player file gives the default
player, and a later save does not overwrite that file. The game writes over an unreadable file.

Provenance: `source_suggested` for the layout, `design` for the snapshot and the unreadable file.
`source_suggested_black_box_confirmed` for the position of a player with no player file:
experiments PLD-E1 to PLD-E5 and PLD-E7 of `player-death.md`, section 13. `source_suggested` for
the wait in the configuration phase. Kind: file format and semantic. Confidence: high.

## SAVE-OWNERSHIP-001: Ownership and eviction

A dimension has one data layer. The cells of a divided dimension share it. A column has one owner
cell, which the region partition selects. Only the owner writes the column. A cell that does not own
a column can ask the owner or the data layer for it. It can keep a copy for its view.

A hot column lives in a cell. When the server unloads it, the owner writes it to the data layer and
the cell releases its sections. A later load takes the column from the data layer before the world
generator runs. The data layer keeps a dirty column until a flush makes it durable. The number of
open region files is bounded. A file that a flush does not use is released.

Provenance: `design`. Kind: architecture. Confidence: high.

## SAVE-LIMITS-001: Not covered

The server does not persist the activity mask of a chunk. It does not write the entities of a
column, the points of interest (the `poi` directory), or the statistics and advancements of a
player. It does not write the other saved data files of the game, for example the scoreboard, the raids and
the world border.
Thus the game starts such a world with no saved entity and with no saved scoreboard. The server
does not read the entities of a column either: a column record with a non-empty `entities` list is
unreadable (SAVE-CHUNK-001). A save from
another world version is rejected and not upgraded. The server does not run a datafixer. The order
in which a full region cache releases files is not part of the contract.

The server neither reads nor writes the `poi` directory (see U3).

Provenance: `design`. Kind: scope. Confidence: high. Test: none; this is a stated limit.

## Experiments

The task `dumpLocalWorlds` of `tools/vanilla-oracle` runs E1, E3, E4 and E5 and the game part of
E2 with the game version of `assets/minecraft/version.json`. It writes
`crates/mcrs_minecraft_save/tests/fixtures/vanilla/local_worlds.json` (fixture `local_worlds` of `tools/fixtures.json`). The
second process of the task is a Java process of the game. The capture ran on macOS. The `fcntl`
and `flock` attempts of E2 ran by hand with a script, and the fixture does not hold them.

| Experiment | Rules | Steps | Result |
|------------|-------|-------|--------|
| E1 | SAVE-LOCK-001 | Let the game take the lock of a new directory. Read `session.lock`. Release the lock and read the file again. | The file holds `E2 98 83` before and after the release. The release keeps the file. |
| E2 | SAVE-LOCK-001, SAVE-LOCK-002 step 3 | Let the game hold the lock. In a second process, test the lock as the game does. By hand, also try a POSIX record lock and a `flock` lock with no wait. Then hold the lock of the game in the second process, and let the game test the lock and take it. Release it and let the game take it. By hand, also hold each kind in the second process. | The second process sees the lock, and its attempts fail at once. The game tests the directory as locked for each kind, and its attempt to take the lock fails. After the release, the game takes the lock. |
| E3 | SAVE-LOCK-001 | Let the game test a directory with no `session.lock`, and a directory whose `session.lock` the process cannot open for write. | The first directory is not locked. The second directory is locked. |
| E4 | SAVE-LOCK-001 | Let the game hold the lock. In the same process, read `session.lock` through a second handle and close it. Then try the lock from a second process. | The second process sees no lock. The close of the second handle released the POSIX record lock of the game (SAVE-LOCK-002, step 4). |
| E5 | SAVE-LEVEL-002 | Make a save directory with only `level.dat_old`. Let the game open it and read the world data. | The game reads the values of `level.dat_old`. Then the directory holds `level.dat` and `session.lock`, and no `level.dat_old`. |

## Unresolved

- **U3.** The game probably builds the points of interest again from the blocks that it loads when
  the `poi` directory is missing. No experiment confirms it. No rule exists yet. SAVE-LIMITS-001 states that the server
  neither reads nor writes `poi`.

- **U4.** The game chooses the spawn of a new world at its first start (SAVE-NEW-001, step 3).
  No rule states this choice. The game selects a start column from the field `spawn_target` of
  the noise settings and the climate noises of the world. Then it tests the columns of the chunks
  around that column with items 1 to 3 of `player-death.md`, PLD-RSP-005. mcrs keeps the default
  spawn until a rule states the choice (SAVE-NEW-002, step 1, and SAVE-DEV-002).

## Intentional deviations

### SAVE-DEV-001: A world with no seed does not open

- **Rule:** SAVE-NEW-003, row 2.
- **Reference:** a save directory with `level.dat` and no `world_gen_settings.dat` opens with a
  random seed (SAVE-DATA-001).
- **Reason:** a world with a `level.dat` and no seed is a damaged world. A random seed changes its
  terrain at each column that the world generates again.
- **Visible to the client:** yes. The server does not start, and a local world shows the screen
  of a failed start. The game opens the same save, and new columns have other terrain.
- **Bug reference:** `none`.
- **Test:** a differential test uses no save with this damage.

### SAVE-DEV-002: The first start keeps the default spawn

- **Rule:** SAVE-NEW-002, step 1.
- **Reference:** at the first start, the game chooses a spawn on the surface of a column that
  the climate selects, and it places the bonus chest when `bonus_chest` is 1.
- **Reason:** no rule states the choice of the game yet. Item U4 of the section "Unresolved" holds
  the open point.
- **Visible to the client:** yes. A new player of a new world enters near x 0 and z 0 of the
  overworld, at the position that SAVE-PLAYER-001, paragraph "Default position", gives for
  `S` at `0 0 0`. In the game, the player enters near the spawn that the game chose. mcrs places
  no bonus chest.
- **Bug reference:** `none`.
- **Test:** a differential test does not compare the spawn of a new world.

### SAVE-DEV-003: A damaged `level.dat` does not fall back to the backup

- **Rule:** SAVE-LEVEL-002, step 5.
- **Reference:** when `level.dat` does not read, the vanilla server reads `level.dat_old`. It
  renames the damaged `level.dat` to `level.dat_corrupted_<date and time>` and renames
  `level.dat_old` to `level.dat`. Then it opens the world from the backup.
- **Reason:** the server never reads a damaged file in silence (SAVE-FILE-002). The backup can be
  older than the regions, so a silent open can lose the data of the player. The server writes
  `level.dat` atomically and keeps no backup, so it does not make such a file itself.
- **Visible to the client:** only for a save directory whose `level.dat` is damaged. The server
  does not start, and the error names `level.dat`.
- **Bug reference:** `none`.
- **Test:** a differential test uses no save with a damaged `level.dat`.

### SAVE-DEV-004: A `level.dat` of another version does not open

- **Rule:** SAVE-VERSION-002.
- **Reference:** the vanilla server converts a `level.dat` of an older version with its
  datafixer and opens the world. It opens a world of a newer version of the same version series
  with no conversion. It refuses a world of another version series.
- **Reason:** mcrs has no datafixer, and a `DataVersion` mismatch is a loud error. A world of a
  newer version can hold data that this version cannot read, and a save can lose it.
- **Visible to the client:** yes, for a world of another version. The server does not start, and
  the error names the two versions.
- **Bug reference:** `none`.
- **Test:** a differential test uses only saves of `DataVersion` `5127`.

### SAVE-DEV-005: A backup of another version keeps its name

- **Rule:** SAVE-LEVEL-002, steps 1 and 4.
- **Reference:** when `level.dat` is missing and `level.dat_old` reads, the vanilla server renames
  `level.dat_old` to `level.dat` before it reads the version. A world of a version that it cannot
  open then does not open, and the save directory keeps the renamed file.
- **Reason:** mcrs does not open a world of another version (SAVE-DEV-004). A world that does not
  open keeps its files as they are, so the game of the version of the world can open its backup
  as before.
- **Visible to the client:** only for a save directory with `level.dat_old`, no `level.dat`, and a
  `DataVersion` that is not `5127`. The save directory keeps `level.dat_old`.
- **Bug reference:** `none`.
- **Test:** a differential test uses only saves of `DataVersion` `5127`.

### SAVE-DEV-006: A join outside the world border uses the border centre

- **Rule:** SAVE-PLAYER-001, paragraph "Default position".
- **Reference:** a join with no `Pos` uses `spawn.pos` as `S` also when `spawn.pos` is outside the
  world border. In survival, the search radius is then 1, and the player enters outside the
  border. A respawn uses the centre of the border in that case (`player-death.md`, PLD-RSP-004). Experiment PLD-E7
  of `player-death.md` shows the join outside the border.
- **Reason:** a player expects to enter the world inside the border, at the same place as a
  respawn. One spawn placement with one `S` serves the join and the respawn.
- **Visible to the client:** only when `spawn.pos` is outside the world border. A new player
  enters near the centre of the border, not near `spawn.pos`.
- **Bug reference:** `none`.
- **Test:** a differential test does not compare a join with `spawn.pos` outside the world border.
