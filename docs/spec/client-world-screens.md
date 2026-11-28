# Singleplayer world screens of the client: behavioural specification

This document states the singleplayer screens of the vanilla client: "Select World", "Create New
World" and the confirmation of a delete. It states the folders and the files that these screens
read and write, and what they give to the entry into a world. For each screen it states the mcrs
subset. The document uses the terms of `client-widget.md`, `client-list.md`, `client-edit-box.md`,
`client-menu.md` and `client-title-screen.md`, and cites their rules. It does not prescribe an
architecture.

## 0. Conventions

### 0.1 Terms

- The terms of `client-widget.md`, section 0.1, apply with their words: **GUI unit**, **W**,
  **H**, **in a world**, **screen**, **parent screen**, **widget**, **bounds**, **visible**,
  **active**, **hovered**, **focused**, **activation**, **activation key**, **focus
  navigation**, **tooltip of a widget**, **tab order group**, **caption**, **text widget**,
  **layout element**, **layout**, **cell**, **padding**, **alignment** and **gap**.
- The terms of `client-menu.md`, section 0.1, apply with their words: **local world** and
  **mcrs subset**.
- The terms of `client-title-screen.md`, section 0.1, apply with their words: **title screen**,
  **message screen**, **level loading screen** and **disconnected screen**.
- The terms of `client-list.md`, section 0.1, apply to a list, a row, the selected row and a
  menu tab bar.
- **Game directory**: the folder in which the client keeps its files. It holds the saves folder
  and the options file of `client-menu.md`.
- **Saves folder**: the folder that holds one folder for each world of the client.
- **World folder**: a folder in the saves folder that WORLD-DIR-001 accepts.
- **Locked world**: a world folder whose file `session.lock` another process holds
  (`persistence.md`, SAVE-LOCK-001).
- **Hardcore world**: a world whose `Data.difficulty_settings.hardcore` is 1.
- **Folder id**: the name of a world folder, with no path.
- **World name**: the `LevelName` of a world (WORLD-DIR-002). A player types it on "Create New
  World".
- **Summary**: the values that the client reads from a world folder to show its entry
  (WORLD-DIR-002).
- **Entry**: a row of the world list that shows one summary (section 3).
- **Unreadable world**: a world folder whose summary the client cannot read (WORLD-DIR-003).
- **Client version**: the version of `assets/minecraft/version.json`. Its world version is the
  field `world_version`, its version series `series_id`, its version name `name`, and it is
  stable when `stable` is true.
- **Incompatible world**: in mcrs, a world folder whose `DataVersion` is not the world version of
  the client version (WORLD-DIR-004).
- **Disabled entry**: an entry whose world the player cannot start from the list.
- **Selection key**: Enter, Space or keypad Enter. **Confirmation key**: Enter or keypad Enter.
- **World preset**: an entry of the registry `minecraft:worldgen/world_preset` of the data pack.

### 0.2 Provenance, confidence and observer

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". The language keys and their English texts are in
`assets/minecraft/lang/en_us.json` of the client jar. The sprites are in
`assets/minecraft/textures/gui/sprites/` of the client jar. The world preset tags are in
`data/minecraft/tags/worldgen/world_preset/` of the data pack. The choice of a key, a sprite or a
tag for a widget comes from the reference. No experiment ran.

A rule is `black_box_confirmed` for an English text, a sprite size or a tag only where the pinned
artefact itself is the evidence. The rule names that artefact.

The observer of each rule is a player in normal play who sees the screen, unless the rule names
another observer. The observer "the save file" is a later reader of the world folder: the vanilla
client, the vanilla server or mcrs.

### 0.3 Arithmetic

The arithmetic of `client-widget.md`, section 0.3, applies: `a / b`, `a % b`, `trunc`, `floor`,
`round` and `lerp`. `tw` is the width of a text in GUI units, as `client-font.md` states. Each
example uses W = 427 and H = 240, with the window of 854 × 480 pixels and the GUI scale 2.

`i32` and `i64` are the signed integers of 32 and 64 bits in two's complement. A wrap to `i32`
keeps the low 32 bits.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `WORLD-DIR` | the saves folder, the world folders, and the fields of a summary |
| `WORLD-LIST` | the screen "Select World": widgets, the load of the list, order, search, keys |
| `WORLD-ENTRY` | an entry of the world list: content, info line, overlay, input |
| `WORLD-PLAY` | the press of "Play Selected World" and the screens that follow it |
| `WORLD-DEL` | the delete of a world and its confirmation screen |
| `WORLD-EDIT` | the buttons "Edit" and "Re-Create" |
| `WORLD-CREATE` | the frame of "Create New World": tabs, footer, keys, close |
| `WORLD-GAME` | the tab "Game" |
| `WORLD-GEN` | the tab "World" |
| `WORLD-MORE` | the tab "More" |
| `WORLD-NAME` | the folder id that a world name gives |
| `WORLD-SEED` | the seed that the text of the seed box gives |
| `WORLD-WRITE` | what "Create New World" writes, and the next screen |
| `WORLD-PROP` | the properties of each screen that `client-widget.md` leaves open |
| `WORLD-DEV` | intentional deviations |

### 0.5 Scope

In scope:

- "Select World": its widgets, the world list, the entries, the search, the keys and the
  buttons;
- the saves folder, what makes a folder a world, and the fields that the list reads;
- what shows while the list loads, with no world, and for a world that the client cannot read;
- "Create New World": its tabs, its widgets, the folder id, the seed, and what it writes;
- the confirmation and the effect of a delete;
- the screens after "Play Selected World", one line for each warning or question screen;
- the names of "Edit" and "Re-Create" and the screens that they open.

Out of scope:

- Realms, and the upload of a world;
- the screen "Edit World", backups and "Optimize World";
- the screens of the game rules, the experiments and the data packs;
- the warning for a symbolic link;
- how and when the client writes the icon of a world;
- the multiplayer screens;
- the screens of the preset editors ("Customize");
- the recovery screen of an unreadable world;
- music and sound, the click sound and the narrator;
- the work of the local server while it starts, except what a player sees.

### 0.6 Constants

| Value | Data | Rule |
|-------|------|------|
| Select World: header height, footer height | 49, 60 | WORLD-LIST-001 |
| Select World: search box | 200 × 20, maximum length 32 | WORLD-LIST-001 |
| World list: row height, row width | 36, 270 | WORLD-LIST-001 |
| Footer grid: columns, column gap, row gap | 4, 8, 4 | WORLD-LIST-001 |
| Footer buttons: row 1, row 2 | 150 × 20 with a span of 2, 71 × 20 | WORLD-LIST-001 |
| Entry: icon, text offset, text lines | 32 × 32, 35, at +1, +12 and +21 | WORLD-ENTRY-001 |
| Entry: maximum text width | 231 | WORLD-ENTRY-001 |
| Entry: grey text, hover fill | `FF808080`, `A0909090` | WORLD-ENTRY-001, WORLD-ENTRY-003 |
| Entry: tooltip wrap width | 175 | WORLD-ENTRY-003 |
| Entry: date form in mcrs | `M/d/yy, h:mm AM`, U+0020 before `AM` or `PM` | WORLD-ENTRY-005 |
| Loading entry: frame time of the dots | 300 ms | WORLD-LIST-003 |
| Error screen: title, first reason line, line height, reason width | y 90, y 110, 9, W − 50 | WORLD-LIST-007 |
| Error screen: button, background | (W / 2 − 100, 140, 200, 20); `FF402020` to `FF501010` | WORLD-LIST-007 |
| Delete: attempts, gap between attempts | 5, 500 ms | WORLD-DEL-002 |
| Confirmation screen: gaps, button padding top, message width, rows | 8 and 4, 16, W − 50, 15 | WORLD-DEL-001 |
| Create New World: footer height, footer gap | 33, 8 | WORLD-CREATE-001 |
| Game tab: name box, other widgets, row gap | 208 × 20, 210 × 20, 8 | WORLD-GAME-001 |
| World tab: column gap, row gap, seed box, switch grid width | 10, 8, 308 × 20, 310 | WORLD-GEN-001 |
| World tab: switch | 44 × 20, row gap 4 | WORLD-GEN-001 |
| Folder id: reserved names | `COM`, `CLOCK$`, `CON`, `PRN`, `AUX`, `NUL`, `COM1` to `COM9`, `LPT1` to `LPT9` | WORLD-NAME-001 |
| Folder id: changed characters | `/` `\` `:` `*` `?` `"` `<` `>` `\|` `` ` `` `.`, U+0000, U+0009, U+000A, U+000C, U+000D | WORLD-NAME-001 |

---

## 1. Saves folder and summaries

### WORLD-DIR-001: World folders of the vanilla client

- **Observer:** a player, and the save file.
- **Inputs:** the saves folder `saves` in the game directory, and its content.
- **Behavior:**
  1. When the saves folder does not exist, the client makes it.
  2. When the client cannot make the saves folder, when the saves path exists and is not a
     folder, or when the client cannot open it to list it, the list does not load. The error
     screen of WORLD-LIST-007 opens.
  3. When the client cannot read the next name of the saves folder after it opened the folder,
     the vanilla client stops with a crash report.
  4. A world folder is a folder directly in the saves folder that holds a regular file
     `level.dat` or a regular file `level.dat_old`.
  5. The client ignores each other folder and each file of the saves folder. An entry whose kind
     the client cannot read, for example because of its permissions, is not a folder. The client
     ignores it with no message.
- **Postconditions:** a folder with only `region/` and no `level.dat` is not in the list.
- **Edge cases:** a world folder that is a symbolic link gets the warning of the symbolic links,
  which is out of scope.

Provenance: `source_suggested`; `black_box_confirmed` for the English text by the language file.
Kind: semantic. Confidence: medium.

### WORLD-DIR-002: Fields of a summary

- **Observer:** a player, and the save file.
- **Inputs:** a world folder.
- **Behavior:**
  1. The client reads only `level.dat`. It never reads `level.dat_old`. When `level.dat` does
     not exist, the world is an unreadable world (WORLD-DIR-003), also when `level.dat_old`
     exists. The file is a gzip NBT file. The client reads the compound `Data`.
  2. The client reads these values. A missing value takes its default.

| Value | NBT path in `level.dat` | Type | Default | Use |
|-------|-------------------------|------|---------|-----|
| storage version | `Data.version` | int | none | 19133, or 19132 for a world that needs a manual conversion. Another value makes the world unreadable. |
| last played | `Data.LastPlayed` | long, ms since 1970-01-01 UTC | 0 | order, date |
| version name | `Data.Version.Name` | string | `""` | info line |
| world version | `Data.DataVersion`, `Data.Version.Id` | int | 0 | compatibility |
| version series | `Data.Version.Series` | string | `main` | compatibility |
| snapshot | `Data.Version.Snapshot` | byte | 0 | none on the list |
| world name | `Data.LevelName` | string | `""` | first line |
| game mode | `Data.GameType` | int: 0 survival, 1 creative, 2 adventure, 3 spectator. A value that is not a number reads as 0. Then each value outside 0 to 3, for example 7 or −1, reads as 0. | 0 | info line |
| hardcore | `Data.difficulty_settings.hardcore` | byte | 0 | info line |
| commands | `Data.allowCommands` | byte | 1 when the game mode after its read is 1, else 0. Thus `GameType` 7 gives 0. | info line |
| feature flags | `Data.enabled_features` | list of strings | `["minecraft:vanilla"]` | experimental |
| icon | the file `icon.png` of the world folder | PNG of 64 × 64 pixels | none | icon |

  3. An empty world name shows the folder id in its place.
  4. A world is experimental when its feature flags hold a flag that is not
     `minecraft:vanilla`.
  5. A world is locked when another process holds the file `session.lock` of the world folder.
     The test is that of `persistence.md`, SAVE-LOCK-001. When the client cannot test the lock,
     the world is not in the list.
  6. An icon that is not a regular file, not a PNG, or not 64 × 64 pixels shows as no icon.
- **Postconditions:** the client reads no other field for the list. A `level.dat` with more
  fields than this table is not unreadable.

Provenance: `source_suggested_black_box_confirmed` for steps 1 and 3, the storage version, the
game mode and the commands by `crates/mcrs_minecraft_save/tests/fixtures/vanilla/local_worlds.json`, `summaries`
(section 14, E3). `source_suggested` for the rest. Kind: semantic. Confidence: medium.

### WORLD-DIR-003: Unreadable world in the vanilla client

- **Inputs:** a world folder with no `level.dat`, or whose `level.dat` is not valid gzip NBT, has
  no `Data`, or has a storage version that is not 19132 or 19133.
- **Behavior:**
  1. The entry stays in the list. Its world name is the folder id.
  2. Its last played time is the modification time of `level.dat`, else of `level.dat_old`.
     With neither time, the date does not show.
  3. Its info line is `recover_world.warning`, "Failed to load world summary", in `FFFF0000`.
  4. "Play Selected World" shows `recover_world.button`, "Attempt to Recover". It opens the
     recovery screen, which is out of scope.
  5. "Edit" and "Re-Create" are not active for it.
- **Postconditions:** the player can delete an unreadable world.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language
file. `source_suggested_black_box_confirmed` for the inputs and steps 1 to 3 by
`crates/mcrs_minecraft_save/tests/fixtures/vanilla/local_worlds.json`, `summaries` (section 14, E3). Kind: semantic. Confidence: medium.

### WORLD-DIR-004: Saves folder and summaries (mcrs subset)

- **Observer:** a player, and the save file.
- **Inputs:** the game directory that the client application gives, and its content.
- **Behavior:**
  1. The client application gives the path of the game directory through the configuration of
     the client plugin. The saves folder is the folder `saves` in the game directory. The
     options file is in the same game directory.
  2. The default game directory is the data folder of the platform for mcrs. The client
     application computes it as the table below states. A setting of the client application
     can name another game directory.
  3. WORLD-DIR-001 applies, with the saves folder of step 1, except its step 3. When the client
     cannot read the next name of the saves folder, the list does not load, and the error screen
     of WORLD-LIST-007 opens (WORLD-DEV-014). The list shows no part of the folder.
  4. WORLD-DIR-002 applies, step 5 included. The test of the lock is that of
     `persistence.md`, SAVE-LOCK-002.
  5. The client ignores each field of `level.dat` that the table of WORLD-DIR-002 does not name.
     It ignores the compounds `Data.Player` and `Data.WorldGenSettings`.
  6. WORLD-DIR-003, steps 1 to 3, apply. "Play Selected World" is not active for an unreadable
     world (WORLD-LIST-006).
  7. A world whose world version is not the world version of the client version is an
     incompatible world. Its storage version 19132 also makes it incompatible.
  8. An experimental world is an incompatible world (WORLD-DEV-006).

| Platform | Default game directory |
|----------|------------------------|
| Linux and the other Unix systems | `$XDG_DATA_HOME/mcrs`; `$HOME/.local/share/mcrs` when `XDG_DATA_HOME` is not set or is not an absolute path |
| macOS | `$HOME/Library/Application Support/mcrs` |
| Windows | `mcrs` in the roaming application data folder (`%APPDATA%\mcrs`) |

- **Postconditions:**
  - A `level.dat` that the vanilla client of the pinned version writes is not unreadable in
    mcrs.
  - A world of another `DataVersion` shows in the list and does not start (WORLD-ENTRY-005).
    mcrs does not convert it and does not write to it.
- **Edge cases:** a world folder that is a symbolic link is a world folder. No warning shows.

Provenance: `design` for steps 1, 2 and 8. `design` for step 7: mcrs has no datafixer, and a
`DataVersion` mismatch is a loud error. `design` for step 4: mcrs has the lock of a world folder,
so the vanilla behaviour applies (`client-menu.md`, MENU-DEV-002). `source_suggested` for the rest. Kind: semantic. Confidence:
medium.

---

## 2. Select World

### WORLD-LIST-001: Widgets of the vanilla "Select World"

- **Inputs:** W and H.
- **Behavior:**
  1. The screen has the title `selectWorld.title`, "Select World". Its header is 49 high and its
     footer is 60 high (`client-widget.md`, WID-LAY-011).
  2. The header holds a column with the gap 4 (WID-LAY-006). Each cell has the alignment 0.5
     across. The column holds:
     - the title, as a one-line text widget;
     - the search box, an edit box of 200 × 20 (`client-edit-box.md`, EDIT-OPT-001). Its
       caption is `selectWorld.search`, "search for worlds". Its hint is
       `gui.selectWorld.search`, "Search...", in grey (`FFAAAAAA`) and italic (EDIT-HINT-001).
  3. The column is centred in the header: `y = 8`, and the search box is at
     `(trunc((W − 200) / 2), 21)`.
  4. The world list fills the content: x 0, width W, top 49, height `H − 109`. Its row height
     is 36 and its row width is 270 (`client-list.md`, LIST-GEO-001, LIST-GEO-002).
  5. The footer holds a grid with 4 columns, the column gap 8 and the row gap 4
     (WID-LAY-004, WID-LAY-005). Each cell has the alignment 0.5 across. The grid holds:
     - row 1: `selectWorld.select`, "Play Selected World", 150 × 20 with a span of 2;
       `selectWorld.create`, "Create New World", 150 × 20 with a span of 2;
     - row 2, each 71 × 20: `selectWorld.edit`, "Edit"; `selectWorld.delete`, "Delete";
       `selectWorld.recreate`, "Re-Create"; `gui.back`, "Back".
  6. The grid is 308 × 44. It is centred in the footer: its left edge is
     `gx = trunc((W − 308) / 2)`, row 1 is at `H − 52` and row 2 at `H − 28`.
  7. The tab order is group 0, in this order: the search box, the list, then the buttons of
     step 5 (WID-FOC-004).
- **Postconditions:** at W = 427 and H = 240:

| Widget | Bounds |
|--------|--------|
| search box | (113, 21, 200, 20) |
| world list | (0, 49, 427, 131); rows from x 78 to 348; row 0 from y 51 |
| Play Selected World | (59, 188, 150, 20) |
| Create New World | (217, 188, 150, 20) |
| Edit | (59, 212, 71, 20) |
| Delete | (138, 212, 71, 20) |
| Re-Create | (217, 212, 71, 20) |
| Back | (296, 212, 71, 20) |

- **Edge cases:** the title is at `x = 113 + trunc((200 − tw) / 2)` when `tw` ≤ 200.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language
file. Kind: layout. Confidence: medium.

### WORLD-LIST-002: Active state of the buttons

- **Inputs:** the selected entry, or none.
- **Behavior:**
  1. "Create New World" and "Back" are always active.
  2. With no selected entry, "Play Selected World", "Edit", "Delete" and "Re-Create" are not
     active.
  3. With a selected entry:
     - "Play Selected World" is active when the entry is not disabled (WORLD-ENTRY-002);
     - "Delete" is active;
     - "Edit" and "Re-Create" are active when the entry is not disabled and is not unreadable.
  4. The caption of "Play Selected World" is that of the selected entry: "Play Selected World",
     or "Attempt to Recover" for an unreadable world (WORLD-DIR-003).
  5. A change of the selection updates the buttons at once.
- **Postconditions:** a press on an inactive button does nothing (`client-widget.md`,
  WID-BTN-003).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### WORLD-LIST-003: Load of the list

- **Inputs:** the open or the rebuild of "Select World".
- **Behavior:**
  1. The client reads the summaries off the frame. The screen does not wait for them.
  2. Until they are ready, the list holds one loading entry:
     - the text `selectWorld.loading_list`, "Loading World List", in white, at
       `x = (W − tw) / 2` and `y = 53 + 11` for the first row;
     - below it, at `y + 9`, the dots in `FF808080`, centred in the same way. The dots change
       every 300 ms through these four texts: `O o o`, `o O o`, `o o O`, `o O o`.
  3. When the summaries are ready and there is at least one, the entries of WORLD-LIST-004
     replace the loading entry.
  4. When the summaries are ready and there is none, "Create New World" opens at once
     (section 7). Its close opens the title screen.
  5. A return to "Select World" from another screen of this document reads the summaries again.
- **Postconditions:** a player with no world never sees an empty list. A delete of the last
  world opens "Create New World".

Provenance: `source_suggested`; `black_box_confirmed` for the English text by the language file.
Kind: semantic for steps 1, 3, 4 and 5, layout for step 2. Confidence: medium.

### WORLD-LIST-004: Order and search

- **Inputs:** the summaries, and the value `q` of the search box.
- **Behavior:**
  1. The entries are in the order of the last played time, the newest first. For an equal time,
     the folder ids are in the ascending order of their UTF-16 code units.
  2. An entry shows when the lower case of `q` is a part of the lower case of the world name,
     or a part of the lower case of the folder id. The lower case is the Unicode lower case with
     no rule of a locale. An empty `q` shows each entry.
  3. Each change of `q` applies step 2 at once. The order of step 1 stays.
  4. When the selected entry still shows after a change, it stays selected. Else no entry is
     selected.
  5. When no entry shows, the list is empty. No text shows in it.
  6. The search box keeps its value when the screen is built again, for example after a resize.
- **Postconditions:** `q = "WORLD"` shows a world with the name "My world" and a world with the
  folder id `world_2`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### WORLD-LIST-005: Keys, focus and close of "Select World"

- **Inputs:** a key, a press or the open of the screen.
- **Behavior:**
  1. The initial focus is the search box (`client-widget.md`, WID-FOC-002).
  2. The keys of the list are those of `client-list.md`, LIST-SEL-004 and LIST-SEL-005. A
     selection key on a focused entry acts as WORLD-ENTRY-004, step 3.
  3. Escape and "Back" open the parent screen, which is the title screen.
  4. "Create New World" opens section 7. Its close opens "Select World" again and reads the
     summaries again.
- **Postconditions:** a player can type into the search box at once after the screen opens.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### WORLD-LIST-006: Select World (mcrs subset)

- **Inputs:** W and H.
- **Behavior:**
  1. "Singleplayer" on the title screen opens "Select World". The title screen is its parent
     screen.
  2. The screen has these widgets of WORLD-LIST-001: the title, the search box, the world list,
     "Play Selected World", "Create New World", "Delete" and "Back". "Edit" and "Re-Create" do
     not show (WORLD-EDIT-003).
  3. The footer grid keeps 4 columns of 71. "Edit" and "Re-Create" leave empty cells of
     71 × 20. Thus each widget of step 2 has the bounds of WORLD-LIST-001.
  4. WORLD-LIST-002 applies to the widgets of step 2, with one change: "Play Selected World" is
     not active for an unreadable world, and its caption stays "Play Selected World".
  5. WORLD-LIST-003, WORLD-LIST-004, WORLD-LIST-005 and WORLD-LIST-007 apply.
  6. The search box draws its hint "Search..." in grey and not in italic (WORLD-DEV-011).
- **Postconditions:** at W = 427 and H = 240, "Delete" is at (138, 212) and "Back" is at
  (296, 212). The cells at (59, 212) and (217, 212) are empty.

Provenance: `design` for step 1. `design` for steps 2 to 4 (`client-menu.md`, MENU-DEV-002): mcrs has no "Edit
World" screen, no re-create and no recovery screen. `design` for the empty cells of step 3:
a widget that is not in the subset leaves its cell empty, and the
other widgets keep their vanilla bounds. `source_suggested` for the rest. Kind: layout.
Confidence: medium.

### WORLD-LIST-007: Error screen of a saves folder that the client cannot read

- **Inputs:** a load of the list that fails as WORLD-DIR-001, step 2, or WORLD-DIR-004, step 3,
  states.
- **Behavior:**
  1. An error screen replaces "Select World". Its title is `selectWorld.unable_to_load`,
     "Unable to load worlds". Its reason is `selectWorld.load_folder_access`, "Unable to read or
     access the folder where game worlds are saved!".
  2. The title has its centre at (W / 2, 90). It is white, with a shadow.
  3. The reason breaks into lines with the maximum width W − 50 (WID-TXT-002). Each line is
     centred at W / 2 and is white. The first line is at y 110, and each next line is 9 lower.
  4. The screen has one button, `gui.cancel`, "Cancel", at (W / 2 − 100, 140, 200, 20). It opens
     a title screen with no fade (`client-title-screen.md`, TITLE-SCR-005, step 6).
  5. The background is a vertical gradient from `FF402020` at the top to `FF501010` at the
     bottom. No panorama, no blur and no menu background texture show.
  6. Escape does nothing. A resize builds the widgets again.
- **Postconditions:**
  - A player whose saves path is a file sees "Unable to load worlds", not an empty list. No
    "Create New World" opens.
  - At W = 427 and H = 240, "Cancel" is at (113, 140).

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language
file. Kind: layout for steps 2, 3 and 5, semantic for the rest. Confidence: medium.

---

## 3. Entries of the world list

### WORLD-ENTRY-001: Content of an entry

- **Inputs:** a summary, and the inner box of its row at (`cx`, `cy`) (`client-list.md`,
  LIST-GEO-002, step 6).
- **Behavior:**
  1. The icon fills (`cx`, `cy`, 32, 32). With no valid icon, it is
     `minecraft:textures/misc/unknown_server.png`.
  2. The texts start at `tx = cx + 35`. Each text is a one-line text widget
     (`client-widget.md`, WID-TXT-001):
     - the world name at `cy + 1`, in white;
     - at `cy + 12`, in `FF808080`: the folder id, then ` (`, the date, and `)`. With no last
       played time, only the folder id shows;
     - the info line at `cy + 21` (WORLD-ENTRY-002).
  3. The date is the last played time in the time zone of the system, in the short date and
     time form of the locale of the system. For example, en-US gives `11/14/26, 3:07 PM`.
  4. Each text has the maximum width 231. A wider text shows its start, then `...`, within
     231. Its text widget then has a tooltip with the full text (WID-TIP-001).
- **Postconditions:** at W = 427, the first row has its inner box at (80, 53). Its icon is at
  (80, 53). Its texts start at x 115, at y 54, 65 and 74.

Provenance: `source_suggested`; `black_box_confirmed` for the texture
`assets/minecraft/textures/misc/unknown_server.png` of the client jar. Kind: layout. Confidence:
medium.

### WORLD-ENTRY-002: Info line and disabled entries

- **Inputs:** a summary that is not unreadable.
- **Behavior:**
  1. The info line is the first text of this list that applies:
     1. a locked world: `selectWorld.locked`, "Locked by another running instance of
        Minecraft.", in `FFFF5555`;
     2. a storage version 19132: `selectWorld.conversion`, "Must be converted!", in
        `FFFF5555`;
     3. a version series that is not the version series of the client version:
        `selectWorld.incompatible.info`, "Incompatible version: %s", with the version name, in
        `FFFF5555`;
     4. else the parts of step 2.
  2. The parts, joined with `, `:
     - the game mode: `gameMode.survival` "Survival Mode", `gameMode.creative` "Creative Mode",
       `gameMode.adventure` "Adventure Mode" or `gameMode.spectator` "Spectator Mode". A
       hardcore world shows `gameMode.hardcore`, "Hardcore Mode", in `FFFF0000`;
     - when commands are on, `selectWorld.commands`, "Commands";
     - when the world is experimental, `selectWorld.experimental`, "Experimental", in
       `FFFFFF55`;
     - `selectWorld.version`, "Version:", a space, and the version name.
  3. An empty version name shows `selectWorld.versionUnknown`, "unknown".
  4. The version name is italic when the world needs a backup question (WORLD-PLAY-001, step
     3). It is in `FFFF5555` when the world version is newer than the world version of the
     client version.
  5. A part with no colour of its own is in `FF808080`.
  6. A disabled entry is a locked world, a world with the storage version 19132, or a world of
     another version series.
- **Postconditions:** a survival world of the pinned version with commands off shows "Survival
  Mode, Version: 26.4 Snapshot 7".

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language
file. Kind: layout. Confidence: medium.

### WORLD-ENTRY-003: Overlay of the icon

- **Inputs:** the hovered row (`client-list.md`, LIST-HOVER-001), and whether the mouse is in
  the icon square (`cx` ≤ x < `cx + 32`, `cy` ≤ y < `cy + 32`).
- **Behavior:**
  1. Only the hovered row draws an overlay. It fills the icon square with `A0909090`.
  2. It then draws 32 × 32 sprites over the icon. With the mouse in the icon square, each sprite
     has the suffix `_highlighted`. The sprites are in `minecraft:world_list/`:
     - an unreadable world: `error`, then `marked_join`;
     - a locked world: `error`, and in the square the tooltip `selectWorld.locked`;
     - a storage version 19132: `error`, and in the square the tooltip
       `selectWorld.conversion.tooltip`;
     - another version series: `error`, and in the square the tooltip
       `selectWorld.incompatible.tooltip`;
     - a world that needs a backup question: `marked_join`. A newer world version adds `error`
       and the two lines `selectWorld.tooltip.fromNewerVersion1` and `…2` in `FFFF5555`. Else
       a client of a version that is not stable adds `warning` and the two lines
       `selectWorld.tooltip.snapshot1` and `…2` in `FFFFAA00`;
     - each other world: `join`.
  3. The tooltips of the locked, the conversion and the incompatible world are in `FFFF5555`
     and wrap at 175 GUI units. Each tooltip shows at the mouse (WID-TIP-003).
  4. Over the icon of a world that the player can start, the pointer is the hand.
- **Postconditions:** with the mouse on the icon of a world of the pinned version, the player
  sees `world_list/join_highlighted`: a press there starts the world.

Provenance: `source_suggested`; `black_box_confirmed` for the sprites in
`assets/minecraft/textures/gui/sprites/world_list/` of the client jar (each 32 × 32 pixels) and
for the English texts by the language file. Kind: layout. Confidence: medium.

### WORLD-ENTRY-004: Input on an entry

- **Inputs:** a press of the left button on an entry, or a selection key on a focused entry.
- **Behavior:**
  1. A press selects the entry (`client-list.md`, LIST-SEL-002).
  2. When the entry is not disabled, a double click on the entry, or a press in its icon square,
     starts the world as "Play Selected World" does (WORLD-PLAY-001).
  3. When the entry is not disabled, a selection key on the focused entry starts the world in
     the same way.
  4. A press of another button on an entry does nothing.
- **Postconditions:** one press on the icon of a world starts it. No double click is needed.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### WORLD-ENTRY-005: Entries (mcrs subset)

- **Inputs:** a summary of WORLD-DIR-004.
- **Behavior:**
  1. WORLD-ENTRY-001 and WORLD-ENTRY-004 apply, with these exceptions (WORLD-DEV-010):
     - the date has the form of step 2;
     - the icon is always `minecraft:textures/misc/unknown_server.png`. The client does not
       read `icon.png`;
     - a text that is wider than 231 shows its start and `...`, and it has no tooltip;
     - a press in the icon square only selects the entry, as a press on another part of the
       entry does. A double click and a selection key start the world;
     - WORLD-ENTRY-003 does not apply. The hovered row draws no overlay, no sprite and no
       tooltip over the icon, and the pointer does not change.
  2. The date has the en-US short form `M/d/yy, h:mm AM` for each locale of the system, in the
     time zone of the system:
     - the month, the day and the hour have no leading zero. The hour is 1 to 12;
     - the year has two digits, and the minute has two digits;
     - one U+0020 space comes before `AM` or `PM`. Midnight is `12:00 AM`, and noon is
       `12:00 PM`.
  3. WORLD-ENTRY-002 applies with this order of the info line:
     1. a locked world: `selectWorld.locked`, "Locked by another running instance of
        Minecraft.", in `FFFF5555`;
     2. an incompatible world: `selectWorld.incompatible.info`, with the version name, in
        `FFFF5555`;
     3. else the parts of WORLD-ENTRY-002, step 2. A hardcore world shows "Hardcore Mode" in
        `FFFF0000`.
  4. The disabled entries are the locked worlds, the incompatible worlds and the unreadable
     worlds. A hardcore world is not disabled.
  5. No world needs a backup question, because each world that starts has the world version of
     the client version.
- **Postconditions:**
  - A world of 1.21.4 shows "Incompatible version: 1.21.4" in red, and "Play Selected World" is
    not active for it. "Delete" is active for it.
  - A world that a vanilla client plays shows "Locked by another running instance of
    Minecraft." in red, and "Play Selected World" is not active for it.
  - A world that a player last played at 2026-11-14 15:07 in the time zone of the system shows
    `11/14/26, 3:07 PM` in each locale. A player in a time zone other than UTC sees the local
    time, not the time in UTC.
  - A world with `GameType` 7 shows "Survival Mode" (WORLD-DIR-002).

Provenance: `design` for step 2 (WORLD-DEV-007). `design` for step 1 (WORLD-DEV-010). `design` for steps 3 to 5 (WORLD-DIR-004, steps 4, 7 and 8). The locked world
follows the vanilla rule, because mcrs has the lock (`client-menu.md`, MENU-DEV-002). A hardcore
world starts (WORLD-DEV-008). `source_suggested` for the rest.
Kind: layout. Confidence: medium.

---

## 4. Play Selected World

### WORLD-PLAY-001: From the press to the world in the vanilla client

- **Inputs:** an activation of "Play Selected World" with a selected entry that is not
  disabled, or an input of WORLD-ENTRY-004, steps 2 and 3.
- **Behavior:**
  1. The message screen "Reading world data..." shows (`client-title-screen.md`,
     TITLE-LOAD-001).
  2. A world that the client now cannot read opens the recovery screen (out of scope).
  3. A world that needs a backup opens a question screen. Its title is
     `selectWorld.backupQuestion.<status>` and its text `selectWorld.backupWarning.<status>`.
     The status is one of these:
     - `file_fixing_required`: the world needs an upgrade of its files;
     - `snapshot`: the client is a snapshot and the world version is older;
     - `downgrade`: the world version is newer. The title is in `FFDF5050`.
  4. A world whose generator settings are an old customized world, or are experimental, opens
     the question `selectWorld.backupQuestion.customized` or
     `selectWorld.backupQuestion.experimental`.
  5. With less free disk space than the client needs, the question
     `selectWorld.warning.lowDiskSpace.title` opens, with "Continue" and "Back".
  6. Then the entry acts as `client-title-screen.md`, TITLE-LOAD-004, steps 2 to 4.
  7. "Back" or a cancel on a screen of steps 2 to 5 opens "Select World" again and reads the
     summaries again.
- **Postconditions:** a world of the pinned version that is not experimental shows no question.

Provenance: `source_suggested`; `black_box_confirmed` for the keys by the language file. Kind:
semantic. Confidence: medium.

### WORLD-PLAY-002: Play Selected World (mcrs subset)

- **Inputs:** the same inputs as WORLD-PLAY-001, and a world folder that is not disabled.
- **Behavior:**
  1. The message screen "Reading world data..." replaces "Select World" at the activation,
     before the client handles the next input. Thus no second activation and no key reaches
     "Select World" after the first activation.
  2. No screen of WORLD-PLAY-001, steps 2 to 5, shows. None of them is in the subset.
  3. The client starts its local server on the selected world folder. Then
     `client-title-screen.md`, TITLE-LOAD-006, steps 3 and 4, apply.
  4. The player enters the world in the game mode of `Data.GameType` when the world has no data
     for that player.
  5. When the local server cannot start, the disconnected screen of TITLE-DISC-003 shows the
     reason, with the title of TITLE-DISC-004 for a local world.
  6. When the local server cannot start because another process holds the world
     (`persistence.md`, SAVE-LOCK-002), the reason is `selectWorld.locked`, "Locked by another
     running instance of Minecraft.". This occurs when another process takes the world after
     the list read the summaries.
- **Postconditions:** the player sees "Reading world data...", then "Loading terrain...", then
  the world of that folder.

Provenance: `design` for step 2: WORLD-DIR-004 disables each world that would need a question,
and the low disk space check is not in the subset. `design` for step 6 (WORLD-DEV-009). `source_suggested` for the rest. Kind: semantic.
Confidence: medium.

---

## 5. Delete

### WORLD-DEL-001: Confirmation screen of a delete

- **Inputs:** an activation of "Delete" with a selected entry, W and H.
- **Behavior:**
  1. The confirmation screen opens. It holds a column with the gap 8, centred in the screen
     (`client-widget.md`, WID-LAY-003). Each cell has the alignment 0.5 across.
  2. The column holds:
     - the title `selectWorld.deleteQuestion`, "Are you sure you want to delete this world?",
       as a one-line text widget in white;
     - the message `selectWorld.deleteWarning`, "'%s' will be lost forever! (A long time!)",
       with the world name. It is a centred multi-line text widget (WID-TXT-002) with the
       maximum width `W − 50` and at most 15 rows;
     - a row with the gap 4 of two buttons of 150 × 20, each cell with the padding top 16:
       `selectWorld.deleteButton`, "Delete", then `gui.cancel`, "Cancel".
  3. Escape acts as "Cancel".
- **Postconditions:** at W = 427 and H = 240, with a message of one row, the column is 304 × 70
  at (61, 85). The title is at y 85, the message at y 102, "Delete" at (61, 135) and "Cancel"
  at (215, 135).
- **Edge cases:** the world name in the message is the folder id when the world name is empty
  or the world is unreadable.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language
file. Kind: layout. Confidence: medium.

### WORLD-DEL-002: Effect of a delete in the vanilla client

- **Observer:** a player, and the save file.
- **Inputs:** the answer on the confirmation screen.
- **Behavior:**
  1. "Cancel" and Escape open "Select World" again. Nothing is deleted.
  2. "Delete" first takes the lock of the world folder (`persistence.md`, SAVE-LOCK-001). When
     another process holds it, nothing is deleted, no attempt follows, and the toast of step 4
     shows at once.
  3. Then "Delete" removes the world folder with each file and folder in it. The client
     releases the lock and removes the file `session.lock` last, then the world folder.
  4. When a removal fails, the client tries again, up to 5 attempts, with 500 ms between them.
  5. After the last failed attempt, the toast `selectWorld.delete_failure`, "Failed to delete
     world", shows with the folder id.
  6. Then "Select World" opens again and reads the summaries again (WORLD-LIST-003).
- **Postconditions:** the entry is gone from the list. A delete of the last world opens "Create
  New World" (WORLD-LIST-003, step 4). A delete of a locked world removes no file.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language
file. `source_suggested_black_box_confirmed` for steps 2 and 3 by
`crates/mcrs_minecraft_save/tests/fixtures/vanilla/local_worlds.json`, `delete` (section 14, E4). Kind: semantic. Confidence: medium.

### WORLD-DEL-003: Delete (mcrs subset)

- **Observer:** a player, and the save file.
- **Inputs:** an activation of "Delete", and the answer on the confirmation screen.
- **Behavior:**
  1. WORLD-DEL-001 applies.
  2. WORLD-DEL-002, steps 1, 2 and 4, apply. Step 3 has this order in mcrs (WORLD-DEV-015):
     1. With the lock held, the client removes each file and folder of the world folder except
        `session.lock`.
     2. With the lock still held, the client removes `session.lock`.
     3. The client releases the lock.
     4. The client removes the world folder only when it is empty. It never removes a file that
        another process made in the folder after step 2.
     Each of the removals of substeps 1, 2 and 4 has the attempts of WORLD-DEL-002, step 4. The
     client takes and releases the lock as `persistence.md`, SAVE-LOCK-002, states. The removal
     runs off the frame. The window continues to draw.
  3. While the removal runs, "Select World" shows with the loading entry of WORLD-LIST-003. When
     the removal ends, the list reads the summaries again.
  4. No toast shows. A world folder that the removal did not remove shows in the list again.
- **Postconditions:**
  - A delete of a large world does not stop the frames of the client.
  - A delete of a world that another process holds removes no file. The entry shows again as a
    locked world.
  - A delete never removes a `session.lock` that another process holds, and never removes a file
    that another process made after substep 2.
- **Edge cases:**
  - Another process opens the world folder between substeps 2 and 4 of step 2 and makes a new
    `session.lock`. Then the last removal fails after its attempts, and the folder stays on the
    disk with the files of that process. No toast shows. A folder with no `level.dat` is not in
    the list (WORLD-DIR-001), so the entry does not show again.
  - Limit: a process that opened the old `session.lock` before substep 2 and tries its lock after
    substep 3 holds a lock on a removed file, and a third process can then make and lock a new
    `session.lock` in the same folder. Each client tries the lock at once after it opens the
    file, so the window is a few system calls.

Provenance: `design` for steps 2 to 4 and the edge case: the client never waits on the disk in a
frame, mcrs has no toast, and the order of step 2 removes no lock file that another process
holds (WORLD-DEV-015). `source_suggested` for the rest. Kind: semantic. Confidence: medium.

---

## 6. Edit and Re-Create

### WORLD-EDIT-001: Edit in the vanilla client

- **Inputs:** an activation of "Edit" with a selected entry.
- **Behavior:**
  1. The message screen "Reading world data..." shows.
  2. The screen `selectWorld.edit.title`, "Edit World", opens. It is out of scope.
  3. When the client cannot get the world folder, the toast `selectWorld.access_failure`,
     "Failed to access world", shows and the list reads the summaries again.
  4. The close of "Edit World" opens "Select World" again and reads the summaries again.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language
file. Kind: semantic. Confidence: medium.

### WORLD-EDIT-002: Re-Create in the vanilla client

- **Inputs:** an activation of "Re-Create" with a selected entry.
- **Behavior:**
  1. The message screen "Reading world data..." shows.
  2. "Create New World" opens with the values of the world: the world name, the game mode
     (hardcore, survival or creative), the difficulty, the commands, the game rules, the seed,
     the world type that matches its generator, and its data packs.
  3. A world of the old customized type first opens the question
     `selectWorld.recreate.customized.title` with `gui.proceed`, "Proceed", and `gui.cancel`.
  4. A failure opens the alert `selectWorld.recreate.error.title`, "An error occurred!", with
     `selectWorld.recreate.error.text`.
  5. The close of "Create New World" opens "Select World" again.
- **Postconditions:** "Create New World" then makes a new world folder. The old world stays.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language
file. Kind: semantic. Confidence: medium.

### WORLD-EDIT-003: Edit and Re-Create (mcrs subset)

- **Behavior:** "Edit" and "Re-Create" are not in the subset. They do not show
  (WORLD-LIST-006, step 3).

Provenance: `design` (`client-menu.md`, MENU-DEV-002). mcrs has no "Edit World" screen. A
re-create needs the world type of a saved world, and a save keeps only its dimensions
(`persistence.md`, SAVE-NEW-001). Kind: layout. Confidence: medium.

---

## 7. Create New World

### WORLD-CREATE-001: Frame of the vanilla "Create New World"

- **Inputs:** W, H, and the screen that opened it.
- **Behavior:**
  1. Before the screen opens, the message screen `createWorld.preparing`, "Preparing for world
     creation...", shows while the client prepares the data pack.
  2. The screen has the title `selectWorld.create`, "Create New World". The title draws nowhere
     on the screen: the tab bar fills the header, and no text widget shows the title. The screen
     has a menu tab bar
     (`client-list.md`, TAB-MENU-001) with three tabs: `createWorld.tab.game.title`, "Game";
     `createWorld.tab.world.title`, "World"; `createWorld.tab.more.title`, "More". The first
     tab is current at the open.
  3. The footer is 33 high. It holds a row with the gap 8 of two buttons of 150 × 20:
     `selectWorld.create`, "Create New World", then `gui.cancel`, "Cancel". The row is centred:
     at `(trunc((W − 308) / 2), H − 26)`.
  4. The tab content area is `(0, 24, W, H − 33 − 24)` (TAB-AREA-001).
  5. The screen draws the texture of TAB-AREA-001, step 6, over the header. It draws the
     texture `minecraft:textures/gui/footer_separator.png` at `y = H − 35`, over the full width
     with the height 2.
  6. The tab order: the tab bar and the widgets of the current tab are in group 0. The footer
     buttons are in group 1.
  7. The initial focus is the name box of the "Game" tab.
  8. The tab bar gets a key first (TAB-KEY-001). Then the focused widget gets it. A confirmation
     key that no widget uses acts as "Create New World".
  9. Escape and "Cancel" close the screen:
     - opened from "Select World", it opens "Select World" again and reads the summaries again;
     - opened by WORLD-LIST-003, step 4, it opens the title screen.
  10. A resize builds the widgets again with the same values.
- **Postconditions:** at W = 427 and H = 240, "Create New World" is at (59, 214) and "Cancel"
  at (217, 214). The tabs start at x 28, 152 and 276.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language
file. Kind: layout for steps 2 to 5, semantic for the rest. Confidence: medium.

### WORLD-CREATE-002: Frame of "Create New World" (mcrs subset)

- **Inputs:** W, H, and the screen that opened it.
- **Behavior:**
  1. Before the screen opens, the client reads the world types of WORLD-GEN-003 from the data
     pack. Until they are ready, the message screen "Preparing for world creation..." shows
     (`client-title-screen.md`, TITLE-LOAD-001). When they are ready, "Create New World" opens.
  2. When the client cannot read the world types, "Create New World" does not open. The
     disconnected screen shows the reason, with the title of `client-title-screen.md`,
     TITLE-DISC-004, for a local world. Its button opens a title screen with no fade.
  3. The tab bar has two tabs: "Game" and "World". The tab "More" is not in the subset
     (WORLD-MORE-002).
  4. WORLD-CREATE-001, steps 2 to 10, apply to these tabs. Thus no title widget shows.
- **Postconditions:** at W = 427, each tab is 186 wide. The tabs start at x 28 and 214
  (TAB-MENU-001 with `n = 2`).

Provenance: `design` for step 3 (`client-menu.md`, MENU-DEV-002). Section 15, item 1, asks
whether the empty cells of WORLD-LIST-006 apply to the tab bar. `source_suggested` for step 1.
The number of frames of the message screen is `unspecified`: when the world types are ready at
the activation, the message screen can show for no frame. `design` for step 2: the vanilla
client reads the data pack of its jar and stops with a crash report when that read fails, which
a player does not meet in normal play. `source_suggested` for the rest. Kind: layout for steps 3
and 4, semantic for the rest. Confidence: medium.

### WORLD-GAME-001: Widgets of the vanilla "Game" tab

- **Inputs:** W, H, and the values of the screen.
- **Behavior:**
  1. The tab holds a grid of one column with the row gap 8. It holds, in this order:
     1. an element with a label (`client-widget.md`, WID-LAY-010), with the alignment 0.5
        across. The label is `selectWorld.enterName`, "World Name". The element is the name box,
        208 × 20, with the maximum length 32. Its value starts as `selectWorld.newWorld`, "New
        World", in the language of the client. Its tooltip is `selectWorld.targetFolder`, "Save
        folder: %s", with the folder id of WORLD-NAME-001 in italic;
     2. the cycle button "Game Mode" (`selectWorld.gameMode`), 210 × 20 (WORLD-GAME-002);
     3. the cycle button "Difficulty" (`options.difficulty`), 210 × 20. Its values are
        "Peaceful", "Easy", "Normal" and "Hard" (`options.difficulty.<name>`). It starts at
        "Normal". Its tooltip is `options.difficulty.<name>.info` of the value;
     4. the on/off cycle button `selectWorld.allowCommands`, "Allow Cheats", 210 × 20, with the
        tooltip `selectWorld.allowCommands.info`, "Commands like /gamemode, /experience";
     5. when the client version is not stable, the button `selectWorld.experiments`,
        "Experiments", 210 × 20. It opens the experiments screen.
  2. The captions of the cycle buttons have the form "name and value" (WID-CYC-001).
  3. The tab places the grid as TAB-AREA-001, step 3, states.
- **Postconditions:** with a client version that is not stable, the grid is 210 × 145. At
  W = 427 and H = 240 it is at (108, 30):

| Widget | Bounds |
|--------|--------|
| label "World Name" | at (109, 30) |
| name box | (109, 43, 208, 20) |
| Game Mode | (108, 71, 210, 20) |
| Difficulty | (108, 99, 210, 20) |
| Allow Cheats | (108, 127, 210, 20) |
| Experiments | (108, 155, 210, 20) |

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language file,
and for "not stable" by the field `stable` of `assets/minecraft/version.json`. Kind: layout.
Confidence: medium.

### WORLD-GAME-002: Game modes and the locks

- **Inputs:** the value of "Game Mode", and the world type.
- **Behavior:**
  1. The values of "Game Mode" are, in this order: `selectWorld.gameMode.survival` "Survival",
     `selectWorld.gameMode.hardcore` "Hardcore", `selectWorld.gameMode.creative` "Creative".
     It starts at "Survival". Its tooltip is `selectWorld.gameMode.<value>.info`.
  2. Each value sets or locks these widgets:

| Game Mode | Game type written | Difficulty | Allow Cheats | Bonus Chest |
|-----------|-------------------|------------|--------------|-------------|
| Survival | 0 | free | free, starts OFF | free |
| Hardcore | 0, with `hardcore` 1 | shows "Hard", not active | shows OFF, not active | shows OFF, not active |
| Creative | 1 | free | free, starts ON | free |

  3. "Allow Cheats" follows the game mode until the player changes it. After a change by the
     player, it keeps the value of the player.
  4. A change away from "Hardcore" shows again the values that the player chose before.
  5. With the world type "Debug Mode", "Game Mode" shows `selectWorld.gameMode.spectator`,
     "Spectator", and is not active. "Allow Cheats" shows ON and is not active.
- **Postconditions:** a player who selects "Hardcore" and presses "Create New World" gets a
  world with the difficulty hard, commands off and no bonus chest.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language
file. Kind: semantic. Confidence: medium.

### WORLD-GAME-003: "Game" tab (mcrs subset)

- **Inputs:** W and H.
- **Behavior:**
  1. The tab holds the name box with its label, and "Game Mode". "Difficulty", "Allow Cheats"
     and "Experiments" are not in the subset.
  2. A widget that is not in the subset leaves an empty cell of its own size in the grid. Thus
     the widgets of step 1 have the bounds of WORLD-GAME-001.
  3. The values of "Game Mode" are those of WORLD-GAME-002, step 1: "Survival", "Hardcore" and
     "Creative", in this order. It starts at "Survival". Its tooltip is that of WORLD-GAME-002,
     step 1.
  4. Each value gives the values of the new world that WORLD-WRITE-002, step 3, states. "Hardcore"
     gives the values of WORLD-GAME-002, step 2: the difficulty hard, commands off and no bonus
     chest.
  5. The name box acts as WORLD-GAME-001, step 1.1, except its tooltip. The name box has no
     tooltip "Save folder: %s" (WORLD-DEV-012). The client computes the folder id of
     WORLD-NAME-001 only at the create (WORLD-WRITE-002, step 3).
- **Postconditions:** at W = 427 and H = 240 the grid is at (108, 30). The label is at (109, 30),
  the name box at (109, 43, 208, 20), and "Game Mode" at (108, 71, 210, 20). The cells at
  (108, 99), (108, 127) and (108, 155) are empty.

Provenance: `design` for step 1 (`client-menu.md`, MENU-DEV-002): mcrs has no difficulty, the
host of a local world is always an operator, and mcrs has no experiments. `design` for the
empty cells of step 2. `design` for steps 3 and 4: "Hardcore" is in the subset, and the default
is "Survival". "Hardcore" is a stated exception to MENU-DEV-002, because mcrs does not have the
rules of hardcore (WORLD-DEV-008). `source_suggested` for the rest. Kind: layout. Confidence: medium.

### WORLD-GEN-001: Widgets of the vanilla "World" tab

- **Inputs:** W, H, and the values of the screen.
- **Behavior:**
  1. The tab holds a grid of two columns with the column gap 10 and the row gap 8:
     - row 0: the cycle button `selectWorld.mapType`, "World Type", 150 × 20 (WORLD-GEN-002);
       then the button `selectWorld.customizeType`, "Customize", 150 × 20;
     - row 1, with a span of 2: an element with a label. The label is
       `selectWorld.enterSeed`, "Seed for the world generator". The element is the seed box,
       308 × 20, with the maximum length 32 and the hint `selectWorld.seedInfo`, "Leave blank
       for a random seed";
     - row 2, with a span of 2: a switch grid of the width 310.
  2. The switch grid is a grid with the row gap 4. Its row 0 holds two spacers of the widths
     266 and 44. Each next row holds a label with the alignment (0, 0.5), and a Boolean cycle
     button of 44 × 20 with the caption form "value" in the second column. The rows are:
     - `selectWorld.mapFeatures`, "Generate Structures", with the tooltip
       `selectWorld.mapFeatures.info`, "Villages, Shipwrecks, etc.". It starts ON. It is not
       active for "Debug Mode";
     - `selectWorld.bonusItems`, "Bonus Chest". It starts OFF. It is not active for
       "Hardcore" or "Debug Mode".
  3. "Customize" is active when the world type has a preset editor: "Superflat" and "Single
     Biome". It opens that editor, which is out of scope.
  4. The seed box starts empty.
  5. The tab places the grid as TAB-AREA-001, step 3, states.
- **Postconditions:** the grid is 310 × 117. At W = 427 and H = 240 it is at (58, 35):

| Widget | Bounds |
|--------|--------|
| World Type | (58, 35, 150, 20) |
| Customize | (218, 35, 150, 20) |
| label of the seed | at (58, 63) |
| seed box | (58, 76, 308, 20) |
| label "Generate Structures" | at (58, 114) |
| its switch | (324, 108, 44, 20) |
| label "Bonus Chest" | at (58, 138) |
| its switch | (324, 132, 44, 20) |

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language
file. Kind: layout. Confidence: medium.

### WORLD-GEN-002: World types

- **Observer:** a player, and a data pack.
- **Inputs:** the world presets and their tags in the data pack.
- **Behavior:**
  1. The values of "World Type" are the world presets of the tag `minecraft:normal`, in the
     order of the tag. When that tag is empty or missing, the values are all world presets.
  2. While Alt is down, the values are those of the tag `minecraft:extended`. When that tag
     is empty or missing, they are the values of step 1.
  3. The text of a value is the language key `generator.<namespace>.<path>` of the preset, for
     example `generator.minecraft.normal`, "Default".
  4. The value starts at `minecraft:normal`.
  5. The value `minecraft:amplified` has the tooltip `generator.minecraft.amplified.info`.
- **Postconditions:** with the vanilla data pack, the values are "Default", "Superflat", "Large
  Biomes", "AMPLIFIED" and "Single Biome". With Alt down, "Debug Mode" follows them.

Provenance: `source_suggested`; `black_box_confirmed` for the tags
`data/minecraft/tags/worldgen/world_preset/normal.json` and `extended.json` of the data pack,
and for the English texts by the language file. Kind: semantic. Confidence: medium.

### WORLD-GEN-003: "World" tab (mcrs subset)

- **Inputs:** W, H, and the data pack.
- **Behavior:**
  1. The tab holds "World Type" and the seed box with its label. "Customize", "Generate
     Structures" and "Bonus Chest" are not in the subset.
  2. A widget that is not in the subset leaves an empty cell of its own size in the grid. Thus
     the widgets of step 1 have the bounds of WORLD-GEN-001.
  3. WORLD-GEN-002 applies, except step 2. Alt does not change the values.
- **Postconditions:** at W = 427 and H = 240, "World Type" is at (58, 35) and the seed box at
  (58, 76). A data pack that adds a preset to the tag `minecraft:normal` adds a value.

Provenance: `design` for step 1 (`client-menu.md`, MENU-DEV-002): mcrs has no preset editor, and
its world generation reads neither switch. `design` for step 3: the debug world type is not a
world that a player expects. `design` for the empty cells of step 2.
`source_suggested` for the rest. Kind: layout. Confidence: medium.

### WORLD-MORE-001: The vanilla "More" tab

- **Behavior:** the tab holds a grid of one column with the row gap 8, with three buttons of
  210 × 20: `selectWorld.gameRules`, "Game Rules"; `selectWorld.experiments`, "Experiments";
  `selectWorld.dataPacks`, "Data Packs". Each opens a screen that is out of scope.
- **Postconditions:** at W = 427 and H = 240 the grid is at (108, 41). The buttons are at y 41,
  69 and 97.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language
file. Kind: layout. Confidence: medium.

### WORLD-MORE-002: "More" tab (mcrs subset)

- **Behavior:** the tab is not in the subset. The screens that its buttons open are out of scope.

Provenance: `design` (`client-menu.md`, MENU-DEV-002). Kind: layout. Confidence: medium.

---

## 8. Folder id and seed

### WORLD-NAME-001: Folder id of a world name

- **Observer:** a player, in the tooltip of the name box, and the save file.
- **Inputs:** the value of the name box, and the folders of the saves folder.
- **Behavior:**
  1. Remove from both ends of the value each character with a code of U+0020 or less. When the
     result is empty, use the English text of `selectWorld.newWorld` in the language of the
     client.
  2. Replace each character of the table of section 0.6, "changed characters", with `_`.
  3. When the result is a reserved name of section 0.6, with no regard to case, put `_` before
     it and after it.
  4. Let `b` be the result. For `n` = 0, 1, 2 and so on, the candidate is `b` for `n` = 0, and
     `b (n)` for each other `n`: `b`, a space, `(`, the decimal `n`, `)`.
  5. The folder id is the first candidate that the file system can make as a new folder in the
     saves folder. The client makes it as a test and removes it again.
  6. When the file system refuses a candidate for a reason other than an existing folder, steps 4
     and 5 run one time again with `b` = `World`. They run again also when `b` is `World`. When
     that second run also meets such a refusal, the client has no folder id. The vanilla client
     then stops with a crash report.
  7. The client computes the folder id again at each change of the name box.
- **Postconditions:**

| Name box | Folders that exist | Folder id |
|----------|--------------------|-----------|
| `New World` | none | `New World` |
| `New World` | `New World` | `New World (1)` |
| `  ` | `New World`, `New World (1)` | `New World (2)` |
| `a.b:c` | none | `a_b_c` |
| `con` | none | `_con_` |
| `Map (1)` | `Map (1)` | `Map (1) (1)` |

- **Edge cases:** the test of step 5 is the test of the file system. On a file system with no
  regard to case, `new world` is taken when `New World` exists.

Provenance: `source_suggested_black_box_confirmed` for steps 1 to 5 by
`crates/mcrs_minecraft_save/tests/fixtures/vanilla/local_worlds.json`, `folder_ids` (section 14, E1). `source_suggested` for
steps 6 and 7. Kind: semantic. Confidence: medium.

### WORLD-SEED-001: Seed of the text of the seed box

- **Observer:** a player, in the world that the seed gives, and the save file.
- **Inputs:** the text `t` of the seed box.
- **Behavior:**
  1. Remove from both ends of `t` each character with a code of U+0020 or less.
  2. When the result is empty, the seed is a random `i64`. The client chooses it at the open of
     the screen, and again at each change of the seed box that gives an empty result in step 1.
     For example, a change from ` ` to `  ` chooses a new seed.
  3. When the result is an optional `+` or `-` followed by one or more decimal digits, and its
     value fits in `i64`, the seed is that value. A decimal digit is each character of the
     Unicode category Nd, with its digit value.
  4. Else the seed is the hash of the result, sign-extended to `i64`. The hash starts at 0. For
     each UTF-16 code unit `c` of the result, in order, it becomes `31 · hash + c`, with a wrap
     to `i32`.
- **Postconditions:**

| Text | Seed |
|------|------|
| `12345` | 12345 |
| ` -7 ` | −7 |
| `abc` | 96354 |
| `9223372036854775808` | the hash of the text, because the value does not fit in `i64` |
| `0x10` | the hash of the text |
| `+5` | 5 |
| `-` | 45, the hash of the text |
| `١٢٣` | 123 |
| U+00A0 followed by `12` | the hash of the text, because step 1 does not remove U+00A0 |
| `-9223372036854775808` | −9223372036854775808 |

Provenance: `source_suggested_black_box_confirmed` for steps 1, 3 and 4 and for the empty
result of step 2 by `crates/mcrs_minecraft_save/tests/fixtures/vanilla/local_worlds.json`, `seeds` (section 14, E2).
`source_suggested` for the time of the random choice in step 2. Kind: semantic. Confidence:
medium.

---

## 9. What "Create New World" writes

### WORLD-WRITE-001: Create in the vanilla client

- **Observer:** a player, and the save file.
- **Inputs:** an activation of "Create New World", or a confirmation key of WORLD-CREATE-001,
  step 8.
- **Behavior:**
  1. A world type whose settings are not stable, or a re-created world, first opens the question
     `selectWorld.warning.experimental.title` or `selectWorld.warning.deprecated.title`.
  2. The message screen "Preparing for world creation..." shows.
  3. The client makes the world folder with the folder id of WORLD-NAME-001.
  4. The world gets these values:
     - `LevelName`: the value of the name box after step 1 of WORLD-NAME-001, with no default.
       An empty value gives an empty name;
     - `GameType`, `difficulty_settings` and `allowCommands`: the values of WORLD-GAME-002, with
       `locked` 0;
     - the game rules, the data packs and the feature flags of the "More" tab;
     - in the world generation settings: the seed of WORLD-SEED-001, the dimensions of the world
       type, `generate_structures` and `bonus_chest`.
  5. The world opens as `client-title-screen.md`, TITLE-LOAD-004, steps 2 to 4, states. It is a
     world that the player created in this session (TITLE-LOAD-003).
  6. When the client cannot make the world folder, or cannot copy the data packs into it, no
     world opens. The toast `pack.copyFailure`, "Failed to copy packs", shows with the folder id.
     Then the parent screen opens, as "Cancel" opens it (WORLD-CREATE-001, step 9).
- **Postconditions:** the folder holds `level.dat` and
  `data/minecraft/world_gen_settings.dat` after the first save of the local server.

Provenance: `source_suggested`; `black_box_confirmed` for the keys by the language file. Kind:
semantic. Confidence: medium.

### WORLD-WRITE-002: Create (mcrs subset)

- **Observer:** a player, and the save file.
- **Inputs:** the same inputs as WORLD-WRITE-001.
- **Behavior:**
  1. No question of WORLD-WRITE-001, step 1, shows. Each value of "World Type" of the subset is a
     stable world preset.
  2. The message screen "Preparing for world creation..." replaces "Create New World" at the
     activation, before the client handles the next input. Thus no second activation and no
     confirmation key reaches "Create New World", and the client makes one world for one
     activation.
  3. Before the local server starts, the client makes the world folder and writes its files in
     this order:
     - The client makes the folder with the folder id of WORLD-NAME-001 as a new folder, and it
       takes the lock of the folder at once (`persistence.md`, SAVE-LOCK-002, step 10). When a
       folder with that id exists at this time, the client writes nothing in it, and step 5
       applies.
     - The client writes `data/minecraft/world_gen_settings.dat` with the seed of
       WORLD-SEED-001, the `dimensions` of the world preset of "World Type" as the data pack
       defines them, `generate_structures` 1 and `bonus_chest` 0.
     - The client writes `level.dat` last. It holds `Data.LevelName` (as WORLD-WRITE-001, step 4),
       the values of the table below for the value of "Game Mode", `Data.initialized` 0,
       `Data.DataVersion` and `Data.Version` (`Id`, `Name`, `Series` and `Snapshot`) from the
       fields `world_version`, `name`, `series_id` and `stable` of
       `assets/minecraft/version.json`, `Data.version` 19133 and `Data.LastPlayed`.
  4. The client gives the lock to its local server, and the world opens as WORLD-PLAY-002, steps 3
     to 5, state. No other process can take the lock between the create and the start. It is a
     world that the player created in this session (TITLE-LOAD-003). The local server then
     completes the world as `persistence.md`, SAVE-NEW-002, states.
  5. When the client has no folder id (WORLD-NAME-001, step 6), cannot make the folder, cannot
     take the lock or cannot write a file, no world opens. The client releases the lock that it
     holds, and it removes no file. The disconnected screen of `client-title-screen.md`,
     TITLE-DISC-003, shows the reason, with the title of TITLE-DISC-004 for a local world. Its
     button opens a title screen with no fade (WORLD-DEV-013).

| Game Mode | `GameType` | `difficulty_settings` | `allowCommands` |
|-----------|------------|-----------------------|-----------------|
| Survival | 0 | `difficulty` `normal`, `hardcore` 0, `locked` 0 | 1 |
| Hardcore | 0 | `difficulty` `hard`, `hardcore` 1, `locked` 0 | 0 |
| Creative | 1 | `difficulty` `normal`, `hardcore` 0, `locked` 0 | 1 |

- **Postconditions:**
  - A stop of the client during step 3 leaves no `level.dat` with no `world_gen_settings.dat`.
    A folder with no `level.dat` is not a world folder (WORLD-DIR-001). The next start of the
    folder as a server makes a new world (`persistence.md`, SAVE-NEW-003):
    - after a stop that follows the write of `world_gen_settings.dat`, with the seed of that file
      (row "missing, present");
    - after a stop that comes before that write, the folder has no `world_gen_settings.dat`. The
      server uses the seed of its configuration, or a random seed (row "missing, missing",
      step 1).
  - While the client makes the world, another process sees the folder as a locked folder or
    does not see it as a world folder. Thus another client cannot delete or open it.
  - The new world is in "Select World" the next time it opens. Its info line shows the game
    mode, "Commands" for a world that is not a hardcore world, and the version name of the
    client version.
  - The vanilla client of the pinned version can open the folder.

Provenance: `design` for steps 1, 3 and 5. Step 5 has no toast because mcrs has no toasts
(WORLD-DEV-013). `source_suggested` for step 2. `allowCommands` is 1 because the host of a local
world in mcrs is always an operator (WORLD-DEV-005). The row "Hardcore" is the vanilla row of
WORLD-GAME-002 (WORLD-DEV-008). `initialized` is 0 because
a missing `initialized` reads as 1, and the server then does not complete a new world. The server
reads the seed and the dimensions from `world_gen_settings.dat` (`persistence.md`, SAVE-NEW-003),
so the client writes that file. `design` for the order of step 3: the game writes only `level.dat`
before the start, and an interrupted create of the game opens later with a random seed
(`persistence.md`, SAVE-DEV-001). `design` for the hand-over of the lock in step 4
(`persistence.md`, SAVE-LOCK-002, step 10). The game probably holds the lock in the same way
(`persistence.md`, SAVE-NEW-001, step 1), but no experiment confirms it (issue #973).
`source_suggested` for the rest. Kind: semantic.
Confidence: medium.

---

## 10. Screen properties

### WORLD-PROP-001: Properties of the screens of this document

- **Behavior:** these are the properties that `client-widget.md`, section 16, items 4 and 5,
  leave open. No screen of this document is shown in a world, so no screen pauses a game.

| Screen | In-game screen | Background | Escape closes it | Re-arrange (WID-SCR-003) | Initial focus |
|--------|----------------|------------|------------------|--------------------------|---------------|
| Select World | no | `client-title-screen.md`, TITLE-PAN-004, step 1; the list draws its own background (LIST-BG-001 to LIST-BG-003) | yes, to the title screen | builds its widgets again; keeps the search value and the summaries | search box |
| Confirmation of a delete | no | TITLE-PAN-004, step 1 | yes, as "Cancel" | builds its widgets again | none |
| Create New World | no | TAB-AREA-001, step 6, and the footer line of WORLD-CREATE-001, step 5 | yes, as "Cancel" | builds its widgets again with the same values | name box |
| Message screens | no | TITLE-LOAD-001 | no | builds its widgets again | none |

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

---

## 11. Pointers to other documents

The other documents state these points of this document:

1. `client-title-screen.md`, TITLE-SCR-006, step 2: "Singleplayer" opens "Select World"
   (WORLD-LIST-006).
2. `client-title-screen.md`, TITLE-LOAD-006: the entry starts from "Play Selected World" or
   "Create New World" (WORLD-PLAY-002, WORLD-WRITE-002). Its edge case names the screen of a
   failed start. That screen also shows a world that another process holds (WORLD-PLAY-002,
   step 6).
3. `client-list.md`, LIST-GEO-002: the world list has the row width 270, so its rows run from
   x 78 to 348 at W = 427.
4. `client-widget.md`, WID-CYC-001: the vanilla "World Type" uses a second list of values while
   Alt is down (WORLD-GEN-002). No screen of mcrs uses it (WORLD-GEN-003).
5. `persistence.md`, SAVE-LOCK-001 and SAVE-LOCK-002: the lock of a world folder.
   `persistence.md`, SAVE-NEW-001 to SAVE-NEW-003: the files of a new world.
6. `client-menu.md`, section 0.1, "Options file": the options file is in the game directory
   (WORLD-DIR-004, step 1).
7. `client-title-screen.md`, TITLE-DISC-004: the title of the disconnected screen after a failed
   start of a local world (WORLD-CREATE-002, WORLD-PLAY-002, WORLD-WRITE-002).

---

## 12. Intentional deviations

### WORLD-DEV-001: A world of another DataVersion does not start

- **Rule:** WORLD-DIR-004, step 7, and WORLD-ENTRY-005. `persistence.md`, SAVE-VERSION-002,
  states the start of such a world.
- **Reference:** a world of the same version series starts. An older world gets the backup
  question and an upgrade. A newer world gets the backup question of a downgrade.
- **Reason:** mcrs has no datafixer, and a `DataVersion` mismatch is a loud error. A world of
  another version that mcrs opens and saves can lose data.
- **Visible to the client:** yes. Such a world shows "Incompatible version: %s" in red, and "Play
  Selected World" is not active.
- **Bug reference:** `none`.
- **Test:** a differential test uses only worlds of the pinned `DataVersion`.

### WORLD-DEV-002: The screens show only the widgets of mcrs

- **Rule:** WORLD-LIST-006, WORLD-GAME-003, WORLD-GEN-003, WORLD-MORE-002, WORLD-CREATE-002.
- **Reference:** the screens show all the widgets of WORLD-LIST-001, WORLD-GAME-001,
  WORLD-GEN-001 and WORLD-MORE-001.
- **Reason:** the screens and the options behind the other widgets are not in the subset.
- **Visible to the client:** yes. "Edit", "Re-Create", "Difficulty", "Allow Cheats",
  "Experiments", "Customize", the two switches and the tab "More" do not show. Their cells stay
  empty, and the other widgets keep their vanilla bounds.
- **Bug reference:** `none`.
- **Test:** a differential test compares the bounds of each widget of mcrs with the rules of the
  mcrs subset.

### WORLD-DEV-003: An unreadable world has no recovery

- **Rule:** WORLD-DIR-004, step 6, and WORLD-LIST-006, step 4.
- **Reference:** "Play Selected World" shows "Attempt to Recover" and opens the recovery screen.
- **Reason:** mcrs has no recovery screen (section 0.5).
- **Visible to the client:** yes, for an unreadable world only. The button is not active.
- **Bug reference:** `none`.
- **Test:** a differential test does not compare an unreadable world.

### WORLD-DEV-004: A delete does not stop the frames

- **Rule:** WORLD-DEL-003.
- **Reference:** the delete runs in the frame. The window stops to draw until the folder is gone
  or the last attempt fails. A failure shows a toast.
- **Reason:** the client never waits on the disk in a frame, and mcrs has no toasts.
- **Visible to the client:** yes. The loading entry shows during the delete, and no toast shows.
- **Bug reference:** `none`.
- **Test:** a differential test compares the list after the delete, not the frames during it.

### WORLD-DEV-005: A new world has commands on

- **Rule:** WORLD-WRITE-002, step 3.
- **Reference:** `allowCommands` follows "Allow Cheats": OFF for "Survival" unless the player
  changes it.
- **Reason:** the host of a local world in mcrs is always an operator, and the subset has no switch
  "Allow Cheats".
- **Visible to the client:** yes. Each new world that is not a hardcore world shows "Commands"
  in its info line.
- **Bug reference:** `none`.
- **Test:** a differential test does not compare `allowCommands` of a new world.

### WORLD-DEV-006: An experimental world does not start

- **Rule:** WORLD-DIR-004, step 8.
- **Reference:** the entry shows "Experimental" in yellow, and the world starts after a
  question.
- **Reason:** mcrs has no experimental feature, so it cannot run such a world as the game does.
- **Visible to the client:** yes, for a world with a feature flag that is not
  `minecraft:vanilla`. The entry shows "Incompatible version: %s", and "Play Selected World"
  is not active. mcrs has no experimental feature.
- **Bug reference:** `none`.
- **Test:** a differential test uses no experimental world.

### WORLD-DEV-007: The date of an entry has one form

- **Rule:** WORLD-ENTRY-005, step 2.
- **Reference:** the date has the short form of the locale of the system. The runtime of the
  game puts U+202F before "AM" or "PM" in the English form.
- **Reason:** the mcrs client has no date forms of the locales. One form with a normal space gives
  the same text on each system.
- **Visible to the client:** yes. A player whose system locale is not en-US sees the en-US
  form. Each player sees a normal space before "AM" or "PM".
- **Bug reference:** `none`.
- **Test:** a differential test runs with the locale en-US and compares the date with U+202F
  and U+0020 as equal.

### WORLD-DEV-008: A hardcore world has no hardcore rules

- **Rule:** WORLD-GAME-003, steps 3 and 4, WORLD-WRITE-002, step 3, and WORLD-ENTRY-005, steps 3
  and 4. "Hardcore" is in the subset as a value, and mcrs does not have its rules. This is a stated exception to `client-menu.md`, MENU-DEV-002.
- **Reference:** in a hardcore world, a death ends the world for the player, the difficulty
  stays hard and locked in play, and the hearts have the hardcore form.
- **Reason:** the subset keeps the value "Hardcore" so that a world keeps its vanilla files. mcrs
  does not have the rules of a hardcore world yet.
- **Visible to the client:** yes, in play. mcrs stores `hardcore` 1, `difficulty` `hard` and
  `allowCommands` 0, and the list shows "Hardcore Mode". These values have no effect in
  play: a death acts as in a world that is not hardcore, the difficulty has no effect, the hearts
  have the normal form, and the host stays an operator.
- **Bug reference:** `none`.
- **Test:** a differential test compares the files and the list for a hardcore world. It does
  not compare the play in a hardcore world.

### WORLD-DEV-009: A world that another process took shows the screen of a failed start

- **Rule:** WORLD-PLAY-002, step 6.
- **Reference:** when the client cannot take the lock at "Play Selected World", the toast
  `selectWorld.access_failure`, "Failed to access world", shows, and the title screen opens.
- **Reason:** mcrs has no toasts. The disconnected screen shows the same failure with its reason.
- **Visible to the client:** yes, only when another process takes the world after the list read
  the summaries. The disconnected screen shows the title "Failed to access world", as the vanilla
  toast does, and the reason "Locked by another running instance of Minecraft.".
- **Bug reference:** `none`.
- **Test:** a differential test does not compare this race.

### WORLD-DEV-010: An entry has no overlay and no icon of its own

- **Rule:** WORLD-ENTRY-005, step 1.
- **Reference:** an entry shows the `icon.png` of its world. The hovered row draws the overlay
  of WORLD-ENTRY-003 with its sprites and tooltips, and the pointer is the hand over the icon of
  a world that can start. A press in the icon square starts the world. A cut text has a tooltip
  with the full text.
- **Reason:** the mcrs client reads no `icon.png` and has no overlay of an entry.
- **Visible to the client:** yes. Each entry shows the icon of an unknown server. The icon square
  shows no sprite and no tooltip, and one press on it only selects the entry. A cut world name
  shows no tooltip.
- **Bug reference:** `none`.
- **Test:** a differential test compares the texts of an entry and their bounds. It does not
  compare the icon square, the tooltips or a press on the icon.

### WORLD-DEV-011: The hint of the search box is not italic

- **Rule:** WORLD-LIST-006, step 6.
- **Reference:** the hint "Search..." is grey and italic (`client-edit-box.md`, EDIT-HINT-001).
- **Reason:** the mcrs client draws no italic text (`client-font.md`, FONT-DRAW-002).
- **Visible to the client:** yes, while the search box is empty. The mcrs client draws no italic
  text (`client-font.md`, FONT-DRAW-002).
- **Bug reference:** `none`.
- **Test:** a differential test compares the text and the colour of the hint, not its shear.

### WORLD-DEV-012: The name box has no "Save folder" tooltip

- **Rule:** WORLD-GAME-003, step 5.
- **Reference:** the name box has the tooltip "Save folder: %s" with the folder id of
  WORLD-NAME-001 in italic. The client computes it again at each change of the name box.
- **Reason:** without the tooltip, the client computes the folder id only at the create
  (WORLD-GAME-003, step 5), and it does not test the saves folder at each change of the name box.
- **Visible to the client:** yes, while the mouse is on the name box or the box has the focus.
  No tooltip shows. The folder id needs a test of the saves folder at each change of the name.
- **Bug reference:** `none`.
- **Test:** a differential test does not compare the tooltip of the name box. It compares the
  folder id of the world that the create makes.

### WORLD-DEV-013: A create that fails shows the screen of a failed start

- **Rule:** WORLD-WRITE-002, step 5.
- **Reference:** the toast "Failed to copy packs" shows with the folder id, and the parent screen
  of "Create New World" opens (WORLD-WRITE-001, step 6).
- **Reason:** mcrs has no toasts. The disconnected screen shows the reason of the failure.
- **Visible to the client:** yes, only when the client cannot make the world folder or write its
  files. The disconnected screen shows the title "Failed to access world" and the reason. Its
  button opens the title screen. mcrs has no toasts.
- **Bug reference:** `none`.
- **Test:** a differential test does not compare a create that fails.

### WORLD-DEV-014: A saves folder that stops to list shows the error screen

- **Rule:** WORLD-DIR-004, step 3.
- **Reference:** when the client cannot read the next name of the saves folder after it opened
  the folder, the client stops with a crash report (WORLD-DIR-001, step 3).
- **Reason:** a crash of the client is a defect for mcrs. A part of the list can hide a world
  that exists, and the player can then make a second world with the same name.
- **Visible to the client:** only when the file system fails while the client lists the saves
  folder. The player sees "Unable to load worlds" and can go back to the title screen.
- **Bug reference:** `none`.
- **Test:** a differential test does not compare a saves folder that fails to list.

### WORLD-DEV-015: A delete removes `session.lock` while it holds the lock

- **Rule:** WORLD-DEL-003, step 2.
- **Reference:** the client removes the files of the world folder with the lock held. Then it
  releases the lock, and after the release it removes `session.lock` and the world folder
  (WORLD-DEL-002, step 3).
- **Reason:** on Linux and macOS, a second process can take the lock in the gap between the
  release and the removal of `session.lock`. The removal then removes the lock file that the
  second process holds. A third process then makes a new `session.lock` and takes a second
  lock on the same world, and two processes write the same world. When the client removes
  `session.lock` with the lock held, a process that opens the folder later makes a new
  `session.lock`, and the last removal fails because the folder is not empty.
- **Visible to the client:** only when another process opens the world while the client
  deletes it. Then the folder stays on the disk with the files of that process (WORLD-DEL-003,
  edge cases).
- **Bug reference:** `none`.
- **Test:** after the removal of `session.lock` and before the last removal, a second process
  makes a new `session.lock` in the folder and takes its lock. The delete fails, and the folder
  and the new `session.lock` stay, with the lock of the second process.

---

## 14. Experiments

The task `dumpLocalWorlds` of `tools/vanilla-oracle` runs each experiment with the game version
of `assets/minecraft/version.json`, with no client screen. It writes
`crates/mcrs_minecraft_save/tests/fixtures/vanilla/local_worlds.json` (fixture `local_worlds` of `tools/fixtures.json`). The
capture ran on macOS.

| Experiment | Rules | Steps | Result |
|------------|-------|-------|--------|
| E1 | WORLD-NAME-001 | For each row of the table of WORLD-NAME-001, and for `COM1`, `Map (0)`, `World` and `` x/y\z*?"<>\|` ``, make the folders that exist and ask the game for the folder id. | Each row of the table gives its folder id. `COM1` gives `_COM1_`, `Map (0)` and `World` stay, and the last name gives `x_y_z_______`. The fixture holds no name that differs only in case, because the test is that of the file system. |
| E2 | WORLD-SEED-001 | Ask the game for the seed of each text of the table of WORLD-SEED-001, and of the empty text, two spaces, `+5`, `-`, `١٢٣`, U+00A0 followed by `12`, and `-9223372036854775808`. | The table holds. The empty text and two spaces give no seed. The other results are in the table. |
| E3 | WORLD-DIR-001, WORLD-DIR-002, WORLD-DIR-003 | Make a saves folder with these entries: worlds with `GameType` 1, 7, −1 and none; a world with only `level.dat_old`; a world with storage version 19134; a world with storage version 19132; a world with an empty `LevelName`; a folder with only `region/`; a regular file. Ask the game for the list. | The folder with only `region/` and the regular file are not in the list. `GameType` 1 gives creative with commands. 7, −1 and none give survival with no commands. The world with only `level.dat_old` and the world with storage version 19134 are unreadable worlds, with the info "Failed to load world summary" and the modification time of their file. The world with 19132 is disabled with "Must be converted!". The empty name shows the folder id. |
| E4 | WORLD-DEL-002, WORLD-DEL-003 | Hold the lock of a world in a second process. Ask the game to delete the world. Release the lock and ask again. | The first delete fails on the lock and removes no file. The second delete removes the world folder with `session.lock`. |

## 15. Unresolved

1. A widget that is not in the subset leaves its cell empty (WORLD-LIST-006, WORLD-GAME-003,
   WORLD-GEN-003). Does this also apply to the menu tab
   bar of "Create New World", which has no tab "More" (WORLD-CREATE-002)? The options:
   - two tabs of 186 that fill the bar, as WORLD-CREATE-002 states. The tabs start at x 28
     and 214 at W = 427;
   - the three vanilla places of 124, with the third place empty. The tabs start at x 28 and
     152.

   Recommendation: two tabs that fill the bar. The tab bar is one widget, not a grid of cells,
   and an empty third of the bar looks like a tab that failed to draw. A question to the owner.
