# Pause and options menus of the client: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| Packets | CONFIGURATION, serverbound: `client_information` 0. PLAY, serverbound: `chunk_batch_received` 11, `client_information` 14. PLAY, clientbound: `chunk_batch_finished` 11, `chunk_batch_start` 12, `forget_level_chunk` 38, `level_chunk_with_light` 46, `login` 50, `set_chunk_cache_center` 96, `set_chunk_cache_radius` 97, `set_simulation_distance` 113 |

Source of these values: `assets/minecraft/version.json`, and `assets/mcrs/reports/packets.json`
for the packet ids.

This document states the pause screen and the options screens of the vanilla client. It also
states the mcrs subset of each screen: the widgets that mcrs shows, and their positions. It
states each option that mcrs applies (section 4), except Sensitivity (`client-controls.md`,
CTRL-VAL-001), and the packets that Render Distance and
Simulation Distance cause. It uses the terms of `client-widget.md` and `client-list.md` and cites
their rules for each widget, layout and background. The document does not prescribe an
architecture.

## 0. Conventions

### 0.1 Terms

- The terms of `client-widget.md`, section 0.1, apply with their words: **GUI unit**, **W**,
  **H**, **in a world**, **screen**, **parent screen**, **in-game screen**, **widget**,
  **bounds**, **visible**, **active**, **hovered**, **focused**, **activation**, **activation
  key**, **click sound**, **focus navigation**, **tooltip of a widget**, **last input device**,
  **tab order group**, **caption**, **change mode**, **layout element**, **layout**, **cell**,
  **padding**, **alignment** and **gap**.
- The terms of `client-list.md`, section 0.1, apply with their words: **list**, **row**,
  **option list**, **section label row**, **widget list** and **Ctrl**.
- **Option**: one value of the options of the user, for example the field of view. A widget of
  an options screen shows one option and changes it.
- **Options file**: the file in which the client keeps the options between two starts. Its form
  is out of scope.
- **Stop of the client**: the end of the client on a request of the player. The player closes
  the window, or activates a button that closes the client (MENU-PAUSE-006, step 5). A crash and
  a kill of the process are not a stop of the client.
- **Options screen**: the screen with the title "Options" (section 2).
- **Option sub-screen**: a screen that the options screen or an option sub-screen opens, and
  that shows an option list (section 3). For example, "Mouse Settings" opens from "Controls"
  (`client-controls.md`, CTRL-MOUSE-001).
- **Pending value**: a value of an option slider that waits and is not stored yet
  (`client-widget.md`, WID-SLD-007, step 2).
- **Local world**: a world that the client runs in its own process. A singleplayer world is a
  local world, also when it is open to LAN. A world on a server that the client does not run is
  not a local world.
- **Window pixels** (`pw`, `ph`): the width and the height of the framebuffer of the window, in
  pixels. On a display with a pixel density above 1, they are larger than the window size in
  points.
- **GUI scale** (`s`): the number of window pixels in one GUI unit. It is an integer ≥ 1
  (MENU-SCALE-001).
- **mcrs subset**: the widgets of a vanilla screen that mcrs shows. A screen of mcrs shows only
  the widgets whose function mcrs has. The title of each rule for an mcrs subset ends with
  "(mcrs subset)".
- **Host**: the player whose client runs a local world.
- **Server view distance** (`V`): the largest view distance that a server gives to a player, in
  columns (MENU-DIST-004).
- **Render distance of the client** (`r`): the radius in columns in which the client draws the
  world (MENU-DIST-001).

### 0.2 Provenance, confidence and observer

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". The language keys and their English texts are in
`assets/minecraft/lang/en_us.json` of the client jar. The sprites are in
`assets/minecraft/textures/gui/sprites/` of the client jar. The choice of a key or a sprite for
a widget comes from the reference. No experiment of section 8 ran.

The observer of each rule is a player in normal play who sees the screen, unless the rule names
another observer.

### 0.3 Arithmetic

The arithmetic of `client-widget.md`, section 0.3, applies: `a / b`, `a % b`, `trunc`, `floor`,
`round`, `clamp`, `lerp` and the even split. `ceil(v)` rounds the real `v` up. `sign(v)` is −1,
0 or 1.

Each example uses W = 427 and H = 240, as in the setup of `client-widget.md`, section 14.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `MENU-PAUSE` | the pause screen |
| `MENU-OPT` | the options screen |
| `MENU-SUB` | the common rules of an option sub-screen |
| `MENU-VID` | the "Video Settings" screen |
| `MENU-ACC` | the "Accessibility Settings" screen |
| `MENU-VAL` | the options of mcrs: widget, values, caption, tooltip and effect |
| `MENU-DIST` | the effect of Render Distance and Simulation Distance: the client, the packets and the server |
| `MENU-DISP` | the frame limit, VSync, the fullscreen window and the key "Toggle Fullscreen" |
| `MENU-SCALE` | the GUI scale: its computation, its values and the Ctrl wheel |
| `MENU-SAVE` | the moments when the client writes the options file, and a write that fails |
| `MENU-PROP` | the properties of each screen that `client-widget.md` leaves open |
| `MENU-EXP` | experiments |
| `MENU-DEV` | intentional deviations |

### 0.5 Scope

In scope:

- the pause screen with its menu: the key that opens it, the close, the title and the buttons;
- the options screen;
- the option sub-screens "Video Settings" and "Accessibility Settings", for the order of their
  rows;
- the options of mcrs (section 4);
- the packets that a change of Render Distance or Simulation Distance causes, on a server and
  in a local world;
- the key "Toggle Fullscreen";
- the moment when the client writes the options file, and what a write that fails leaves in it;
- the points that `client-widget.md`, section 16, items 5 and 6, hand to this document.

Out of scope:

- whether the game pauses while the pause screen is open;
- the pause screen with no menu (F3 + Escape), and the pause on a loss of the window focus;
- the title screen;
- the form of the options file;
- the other vanilla options. This document names them only where an order needs them;
- the frame limits of "Reduce FPS When", and the frame limit outside a world;
- "Exclusive Fullscreen", "Exclusive Fullscreen Mode" and the video modes of a monitor;
- the colour of the fog, and the environmental fog;
- the size of a batch and the rate of the chunk send, and the change of the view when a player
  moves to another column;
- the use of the simulation distance in the client;
- the screens that the other buttons open, for example the statistics screen;
- the narrator, the restart message of "Video Settings" and the check for a draft player
  report.

### 0.6 Constants

| Value | Data | Rule |
|-------|------|------|
| Pause screen: title top | 40 | MENU-PAUSE-002 |
| Pause screen: full button, half button | 204 × 20, 98 × 20 | MENU-PAUSE-003 |
| Pause screen: default padding `pl`, `pt`, `pr`, `pb` | 4, 4, 4, 0 | MENU-PAUSE-003 |
| Pause screen: top padding of the first cell | 50 | MENU-PAUSE-003 |
| Pause screen: alignment of the grid in the screen | (0.5, 0.25) | MENU-PAUSE-003 |
| Pause screen: icon button, gap of the icon row | 20 × 20, 4 | MENU-PAUSE-003 |
| Options screen: header height `hh` | 61 | MENU-OPT-002 |
| Options screen: gap of the header column and of its row | 8, 8 | MENU-OPT-002 |
| Options screen: padding `pl`, `pr`, `pb` of a grid cell | 4, 4, 4 | MENU-OPT-002 |
| FOV: range, default | [30, 110], 70 | MENU-VAL-001 |
| FOV Effects: range, default | [0, 1], 1.0 | MENU-VAL-002 |
| Brightness: range, default | [0, 1], 0.5 | MENU-VAL-003 |
| Smooth Lighting: default | ON | MENU-VAL-004 |
| Clouds: vanilla default | Fancy | MENU-VAL-005 |
| Cloud cell, height of a fancy cloud | 12 × 12 blocks, 4 blocks | MENU-VAL-005 |
| GUI Scale: default | 0 (Auto) | MENU-VAL-006 |
| Render Distance: vanilla range, default | [2, 32], or [2, 16] with a heap below 1 000 000 000 bytes; 12 | MENU-VAL-007 |
| Render Distance: mcrs range, default | [2, 96], 96 | MENU-VAL-007 |
| Simulation Distance: vanilla range, default | [5, 32], or [5, 16] with a heap below 1 000 000 000 bytes; 12 | MENU-VAL-008 |
| Simulation Distance: mcrs range | [5, 32] | MENU-VAL-008 |
| Max Framerate: values, default, value with no limit | 10 to 260 in steps of 10, 120, 260 | MENU-VAL-009 |
| VSync: vanilla default, mcrs default | ON, OFF | MENU-VAL-010 |
| Fullscreen: default | OFF | MENU-VAL-011 |
| Draw test of the client: margin | 1 column | MENU-DIST-001 |
| Render-distance fog: end, length | `16 · r` blocks, `clamp(16 · r / 10, 4, 64)` blocks | MENU-DIST-001 |
| View of a player on the server: margin | 2 columns | MENU-DIST-003 |
| Server view distance: vanilla clamp, mcrs clamp | [2, 32], [2, 96] | MENU-DIST-004 |
| "Toggle Fullscreen": default key | F11 | MENU-DISP-005 |
| GUI scale: smallest W and H for a scale above 1 | 320, 240 | MENU-SCALE-001 |

---

## 1. Pause screen

### MENU-PAUSE-001: Open the pause screen

- **Inputs:** a press of Escape in a world, with no screen open.
- **Behavior:**
  1. The pause screen with its menu opens (`client-widget.md`, WID-SCR-001). It has no parent
     screen.
  2. The key is always Escape. The player cannot bind the action to another key.
  3. While a screen is open, Escape goes to that screen (WID-FOC-003). It does not open the
     pause screen.
- **Postconditions:** the game releases the mouse pointer. No widget is focused, because the
  screen opens from the game (WID-FOC-002, postconditions).
- **Edge cases:** the pause screen with no menu, and the pause on a loss of the window focus,
  are out of scope.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: MENU-EXP-01.

### MENU-PAUSE-002: Title of the pause screen

- **Behavior:**
  1. The title is `menu.game`, "Game Menu". It is a one-line text widget (WID-TXT-001) of the
     size `tw` × 9.
  2. Its position is (`W / 2 − tw / 2`, 40), with the integer division.
  3. The title is not a child of the grid of MENU-PAUSE-003.
- **Postconditions:** a title of the width 52 at W = 427 starts at x = 187.

Provenance: `source_suggested`; `black_box_confirmed` for the English text. Kind: layout.
Confidence: medium. Experiment: MENU-EXP-01.

### MENU-PAUSE-003: Grid of the vanilla pause screen

- **Inputs:** W and H, and the state of the client that the column "Shows when" names.
- **Behavior:**
  1. The buttons are the children of a grid that the screen fills by rows (WID-LAY-005) with
     `n` = 2.
  2. The default cell of the grid has the padding `pl` = 4, `pt` = 4, `pr` = 4, `pb` = 0 and
     the alignment (0, 0).
  3. The screen adds the children of the table below in its order. It skips a child whose
     condition is false. "Span 2" is the column span 2.
  4. The screen arranges the grid (WID-LAY-004). Then it aligns the grid in (0, 0, W, H) with
     the alignment (0.5, 0.25) (WID-LAY-003).
  5. The widgets go to the screen in the order of the table. This is also the order of the Tab
     navigation (WID-FOC-004).

| # | Widget | Key and English | Size | Span and cell | Shows when | Action |
|---|--------|-----------------|------|---------------|------------|--------|
| 1 | button | `menu.returnToGame` "Back to Game" | 204 × 20 | span 2, `pt` = 50 | always | MENU-PAUSE-004 |
| 2 | button | `gui.advancements` "Advancements" | 98 × 20 | span 1 | always | opens the advancements screen |
| 3 | button | `gui.stats` "Statistics" | 98 × 20 | span 1 | always | opens the statistics screen |
| 4 | icon row (step 6) | none | 92 × 20 | span 2, `ax` = 0.5 | always | none |
| 5 | button | step 7 | 204 × 20 | span 2 | step 7 | opens a dialog or the server links |
| 6 | button | `menu.options` "Options..." | 98 × 20 | span 1 | in a world | opens the options screen (MENU-OPT-001) |
| 7 | button | `options.worldOptions.button` "World Options..." | 98 × 20 | span 1 | in a world | opens the world options screen |
| 8 | button | MENU-PAUSE-005 | 204 × 20 | span 2 | always | MENU-PAUSE-005 |

  6. The icon row is a row (WID-LAY-006) with the gap 4. It holds four icon only buttons of
     20 × 20 (WID-ICON-001). The caption of each one is also its tooltip. In order:
     - "Friends", `gui.friends.open`, with the icon `friends/friends` (16 × 16). It is inactive
       in a demo world.
     - "Other Players", `menu.other_players`, with the icon `pause_menu/other_players`
       (16 × 16). It is active only on a server that is not a local world, or in a local world
       that is open to LAN. When it is inactive, its tooltip is
       `menu.other_players.no_players`, "This world is not currently joinable by other
       players".
     - "Report Bugs", `menu.reportBugs`, with the icon `pause_menu/bug` (13 × 13). It opens a
       confirm screen for a web link.
     - "Give Feedback", `menu.sendFeedback`, with the icon `pause_menu/feedback` (20 × 20). It
       opens a confirm screen for a web link.
  7. Child 5 shows when the server gives dialogs for the pause screen, or else when the server
     gives server links. With one dialog, its caption is the title of the dialog. With more
     dialogs, it is `menu.custom_options`, "Custom Options...". With only server links, it is
     `menu.server_links`, "Server Links...". With dialogs, the tooltip is
     `menu.custom_options.tooltip`.
- **Postconditions:** in a world with no child 5, at W = 427 and H = 240:
  - The column widths are 106 and 106. The row heights are 70, 24, 24, 24 and 24. The grid is
    212 × 166 at (107, 18).
  - "Back to Game" is at (111, 68). "Advancements" is at (111, 92). "Statistics" is at
    (217, 92).
  - The icon buttons are at x = 167, 191, 215 and 239, with y = 116.
  - "Options..." is at (111, 140). "World Options..." is at (217, 140). Child 8 is at
    (111, 164).

Provenance: `source_suggested`; `black_box_confirmed` for the English texts and for the sizes
of the four icon sprites. Kind: layout. Confidence: medium. Experiment: MENU-EXP-01.

### MENU-PAUSE-004: Back to Game and the close

- **Inputs:** an activation of "Back to Game", or Escape while the pause screen is open.
- **Behavior:**
  1. The pause screen closes. It has no parent screen, so the player returns to the game
     (WID-SCR-002, step 3).
  2. The game takes the mouse pointer again.
- **Postconditions:** Escape on the options screen opens the pause screen again (WID-SCR-002,
  step 2). The pause screen then builds its widgets again (WID-SCR-003, the second kind). It
  does the same after a resize and after a change of the GUI scale.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: MENU-EXP-01.

### MENU-PAUSE-005: Last button of the vanilla pause screen

- **Inputs:** whether the world is a local world.
- **Behavior:**
  1. In a local world, the caption is `menu.returnToMenu`, "Save and Quit to Title". Else it is
     `menu.disconnect`, "Disconnect".
  2. An activation makes the button inactive. Then the client leaves the world.
  3. After a local world, the client shows the title screen. After a server, it shows the list
     of multiplayer servers.
- **Edge cases:** the check for a draft player report before step 2 is out of scope.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts. Kind: semantic.
Confidence: medium. Experiment: MENU-EXP-01.

### MENU-PAUSE-006: Pause screen (mcrs subset)

- **Inputs:** W, H, and whether the world is a local world. mcrs is always in a world. By
  default the mcrs client starts its own local server, so the world is a local world. The
  client can also join a server that it does not run.
- **Behavior:**
  1. The screen keeps the title of MENU-PAUSE-002, and the grid, the cells and the alignment of
     MENU-PAUSE-003.
  2. The grid holds only the children 1, 6 and 8 of the table of MENU-PAUSE-003, in that order:
     "Back to Game", "Options..." and the last button.
  3. "Back to Game" and Escape act as MENU-PAUSE-004 states. "Options..." opens the options
     screen (MENU-OPT-001).
  4. The caption of the last button keeps the condition of MENU-PAUSE-005, step 1: "Save and
     Quit to Title" in a local world, else "Disconnect".
  5. An activation of the last button makes it inactive. Then the client closes, as when the
     player closes the window. This is the same for both captions.
- **Postconditions:** at W = 427 and H = 240:
  - WID-LAY-005 puts "Back to Game" in row 0, "Options..." in row 1 at column 0, and the last
    button in row 2. The right half of row 1 is empty.
  - The column widths are 106 and 106. The row heights are 70, 24 and 24. The grid is
    212 × 118 at (107, 30).
  - "Back to Game" is at (111, 80, 204, 20). "Options..." is at (111, 104, 98, 20). The last
    button is at (111, 128, 204, 20).

Provenance: `design` for the set of widgets and the action of the last button. The owner
decided that a screen shows only the widgets whose function mcrs has. The owner also decided
that the last button closes the client. `source_suggested` for the positions. Kind: layout for the positions,
semantic for the actions. Confidence: medium. Experiment: MENU-EXP-01.

---

## 2. Options screen

### MENU-OPT-001: Open and close the options screen

- **Inputs:** an activation of "Options..." on the pause screen, then Escape or an activation of
  "Done".
- **Behavior:**
  1. "Options..." opens the options screen. The pause screen is its parent screen.
  2. Escape and "Done" close the options screen. The parent screen opens (WID-SCR-002,
     step 2).
  3. The options screen keeps its widgets when it re-arranges (WID-SCR-003, the first kind).
  4. A button of the grid opens an option sub-screen. The options screen is its parent screen.
- **Postconditions:** the client writes the options file each time that the options screen is
  removed (MENU-SAVE-001).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: MENU-EXP-02.

### MENU-OPT-002: Layout of the vanilla options screen

- **Inputs:** W and H.
- **Behavior:**
  1. The screen is a header, a content and a footer (WID-LAY-011) with `hh` = 61, and the
     default `fh` = 33 and `cm` = 30.
  2. The header holds a column (WID-LAY-006) with the gap 8. Its first child is the title
     `options.title`, "Options", a one-line text widget. Its cell has `ax` = 0.5.
  3. The second child of the column is a row with the gap 8. It holds the FOV slider
     (MENU-VAL-001), 150 × 20, then the button `options.online`, "Online...", 150 × 20.
  4. The content holds a grid that the screen fills by rows (WID-LAY-005) with `n` = 2. Its
     default cell has the padding `pl` = 4, `pr` = 4, `pb` = 4 and `ax` = 0.5.
  5. The grid holds these buttons of 150 × 20, in order. Each one opens its screen, with the
     options screen as the parent screen:

| # | Key | English |
|---|-----|---------|
| 1 | `options.skinCustomisation` | "Skin Customization..." |
| 2 | `options.sounds` | "Music & Sounds..." |
| 3 | `options.video` | "Video Settings..." |
| 4 | `options.controls` | "Controls..." |
| 5 | `options.language` | "Language..." |
| 6 | `options.chat` | "Chat Settings..." |
| 7 | `options.resourcepack` | "Resource Packs..." |
| 8 | `options.accessibility` | "Accessibility Settings..." |
| 9 | `options.telemetry` | "Telemetry Data..." |
| 10 | `options.credits_and_attribution` | "Credits & Attribution..." |

  6. Button 9 is inactive when telemetry is not allowed. Its tooltip is then
     `options.telemetry.disabled`, "Telemetry is disabled."
  7. The footer holds the button `gui.done`, "Done", 200 × 20.
  8. The widgets go to the screen in this order: the header, the content, the footer.
- **Postconditions:** at W = 427 and H = 240:
  - The column is 308 × 37 at (59, 12). The title starts at x = `59 + trunc(0.5·(308 − tw))`,
    y = 12.
  - The FOV slider is at (59, 29). "Online..." is at (217, 29).
  - The grid is 316 × 120 at (55, 87). Its buttons are at x = 59 and x = 217, and at
    y = 87, 111, 135, 159 and 183.
  - "Done" is at (113, 214).

Provenance: `source_suggested`; `black_box_confirmed` for the English texts. Kind: layout.
Confidence: medium. Experiment: MENU-EXP-02.

### MENU-OPT-003: Options screen (mcrs subset)

- **Inputs:** W and H.
- **Behavior:**
  1. The screen keeps the parts, the column, the row, the grid and the cells of MENU-OPT-002.
  2. The row of the header holds only the FOV slider.
  3. The grid holds only "Video Settings...", "Controls..." and "Accessibility Settings...", in
     that order. WID-LAY-005 puts the first two in row 0, at the columns 0 and 1. It puts
     "Accessibility Settings..." in row 1, at the column 0.
  4. "Controls..." opens the "Controls" screen of `client-controls.md` (CTRL-SCR-001). The
     options screen is its parent screen.
  5. The footer holds "Done", 200 × 20. Escape and "Done" act as MENU-OPT-001 states.
- **Postconditions:** at W = 427 and H = 240, with a title not wider than 150:
  - The column is 150 × 37 at (138, 12). The FOV slider is at (138, 29).
  - The grid is 316 × 48. The top of the content is `min(61 + 30, 240 − 33 − 48)` = 91. The
    grid is at (55, 91).
  - "Video Settings..." is at (59, 91). "Controls..." is at (217, 91). "Accessibility
    Settings..." is at (59, 115). The right half of row 1 is empty.
  - "Done" is at (113, 214).

Provenance: `design` for the set of widgets, as MENU-PAUSE-006 states. The owner decided on
2026-11-13 that "Controls..." is in the set. `source_suggested` for the positions. Kind: layout.
Confidence: medium. Experiment: MENU-EXP-02.

---

## 3. Option sub-screens

### MENU-SUB-001: Form of an option sub-screen

- **Inputs:** the title of the screen, its rows, W and H.
- **Behavior:**
  1. The screen is a header, a content and a footer (WID-LAY-011) with the default `hh` = 33
     and `fh` = 33.
  2. The header holds the title, a one-line text widget.
  3. The content is an option list (LIST-OPT-001) in the rectangle of LIST-GEO-001:
     `t` = 33 and `h` = `H − 66`.
  4. The footer holds "Done", 200 × 20, unless the screen states another footer.
  5. The widgets go to the screen in this order: the title, the list, the footer.
  6. The list draws its background (LIST-BG-001) and its separators (LIST-BG-002). The screen
     draws no separators of WID-SCR-006.
- **Postconditions:** at W = 427 and H = 240, the title starts at (`trunc(0.5·(427 − tw))`, 12)
  and "Done" is at (113, 214). The widgets of a row start at x = 58 and x = 218
  (LIST-OPT-001).

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: MENU-EXP-03.

### MENU-SUB-002: Close an option sub-screen

- **Inputs:** Escape, or an activation of "Done".
- **Behavior:**
  1. Each slider of the screen stores its pending value at once (WID-SLD-007, step 2).
  2. Then the parent screen opens (WID-SCR-002, step 2).
- **Postconditions:**
  - The sub-screen keeps its widgets when it re-arranges (WID-SCR-003, the first kind). The
    list gets its new rectangle (LIST-GEO-001, step 3).
  - The client writes the options file each time that the sub-screen is removed
    (MENU-SAVE-001).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: MENU-EXP-03.

### MENU-VID-001: Rows of the vanilla "Video Settings" screen

- **Inputs:** the platform of the client: Windows, macOS or another.
- **Behavior:**
  1. The title is `options.videoTitle`, "Video Settings". The screen has the form of
     MENU-SUB-001.
  2. The list holds three sections. Each section has a section label row (LIST-OPT-002), then
     an optional row with one wide widget, then a flat list of options.
  3. The flat list of a section fills rows of two widgets, in order. The first option of a
     pair is at the index 0 of its row. A last option with no pair is alone at the index 0.
  4. The sections, in order:

| Section label | Wide row | Flat list of options, in order |
|---------------|----------|--------------------------------|
| `options.video.display.header` "Display" | Exclusive Fullscreen Mode | **Max Framerate**, **VSync**, Reduce FPS When, **GUI Scale**, **Fullscreen**, Exclusive Fullscreen, **Brightness**, Graphics API; then "Prefer DXGI Present" on Windows, or "Menu/Dock Visibility" on macOS |
| `options.video.quality.header` "Quality & Performance" | Preset | Biome Blend, **Render Distance**, Chunk Builder, **Simulation Distance**, **Smooth Lighting**, **Clouds**, Particles, Mipmap Levels, Entity Shadows, Entity Distance, Menu Background Blur, Cloud Distance, See-Through Leaves, Improved Transparency, Texture Filtering, Anisotropic Filtering, Weather Effect Radius |
| `options.video.preferences.header` "Preferences" | none | Autosave Indicator, Show Vignette, Attack Indicator, Chunk Fade Time |

- **Postconditions:**
  - "Max Framerate" and "VSync" share the first row of "Display". "GUI Scale" is at the index 1
    of the second row. "Fullscreen" is at the index 0 of the third row. "Brightness" is at the
    index 0 of the fourth row.
  - "Render Distance" is at the index 1 of the first row of "Quality & Performance".
    "Simulation Distance" is at the index 1 of the second row. "Smooth Lighting" and "Clouds"
    share the third row.
- **Edge cases:** the Ctrl wheel on this screen changes the GUI scale (MENU-SCALE-003).

Provenance: `source_suggested`; `black_box_confirmed` for the English texts. Kind: layout.
Confidence: medium. Experiment: MENU-EXP-03.

### MENU-VID-002: "Video Settings" screen (mcrs subset)

- **Inputs:** W and H.
- **Behavior:**
  1. The screen keeps the title, the form and the order of MENU-VID-001.
  2. The flat list of each section holds only the options of mcrs, in the vanilla order. Then
     step 3 of MENU-VID-001 makes the rows.
  3. A wide row shows only when its option is an option of mcrs. No wide row shows.
  4. A section label row shows only when its section holds an option of mcrs.
  5. The Ctrl wheel acts as MENU-SCALE-003 states.
- **Postconditions:** the list holds these seven rows. At W = 427 and H = 240 with the scroll
  offset 0:

| Row | Content | Row top | Height | Widgets |
|-----|---------|---------|--------|---------|
| 0 | label "Display" | 35 | 18 | label box top 37 |
| 1 | "Max Framerate", "VSync" | 53 | 25 | (58, 55) and (218, 55) |
| 2 | "GUI Scale", "Fullscreen" | 78 | 25 | (58, 80) and (218, 80) |
| 3 | "Brightness" | 103 | 25 | (58, 105) |
| 4 | label "Quality & Performance" | 128 | 36 | label box top 148 |
| 5 | "Render Distance", "Simulation Distance" | 164 | 25 | (58, 166) and (218, 166) |
| 6 | "Smooth Lighting", "Clouds" | 189 | 25 | (58, 191) and (218, 191) |

  - The content height is 183. The list is 174 high, so it scrolls with `M` = 9
    (`client-list.md`, SCRL-AREA-001).
  - At the scroll offset 0, the bottom of row 6 is below the bottom of the list at y = 207.
  - The scrollbar shows at x = 376 (SCRL-BAR-001). Its thumb is 165 high (SCRL-BAR-002).

Provenance: `design` for the set of rows, as MENU-PAUSE-006 states. `source_suggested` for the
positions. Kind: layout. Confidence: medium. Experiment: MENU-EXP-03.

### MENU-ACC-001: Rows of the vanilla "Accessibility Settings" screen

- **Behavior:**
  1. The title is `options.accessibility.title`, "Accessibility Settings". The screen has the
     form of MENU-SUB-001, but with another footer (step 4).
  2. The first row holds "Narrator", then the button `options.controls`, "Controls...",
     150 × 20. The button opens the "Controls" screen (`client-controls.md`, CTRL-SCR-001), with
     "Accessibility Settings" as its parent screen.
  3. A flat list of options fills the next rows, two in each row, as MENU-VID-001, step 3,
     states. In order: Closed Captions, High Contrast, Menu Background Blur, Text Background
     Opacity, Text Background, Chat Text Opacity, Line Spacing, Chat Delay, Notification Time,
     View Bobbing, Distortion Effects, **FOV Effects**, Darkness Pulsing, Damage Tilt, Glint
     Speed, Glint Strength, Hide Sky Flashes, Monochrome Logo, Panorama Scroll Speed, Hide
     Splash Texts, Narrator Hotkey, Rotate with Minecarts, High Contrast Block Outlines.
  4. The footer holds a row (WID-LAY-006) with the gap 8: the button
     `options.accessibility.link`, "Accessibility Guide", 150 × 20, then "Done", 150 × 20.
     "Accessibility Guide" opens a confirm screen for a web link.
- **Postconditions:** "FOV Effects" is at the index 1 of the seventh row. At W = 427 and
  H = 240, "Accessibility Guide" is at (59, 214) and "Done" is at (217, 214).

Provenance: `source_suggested`; `black_box_confirmed` for the English texts. Kind: layout.
Confidence: medium. Experiment: MENU-EXP-04.

### MENU-ACC-002: "Accessibility Settings" screen (mcrs subset)

- **Behavior:**
  1. The screen keeps the title, the form and the order of MENU-ACC-001.
  2. The list holds two rows. Row 0 holds "Controls..." alone, at the index 0. "Narrator" does
     not show. Row 1 holds "FOV Effects" alone, at the index 0.
  3. "Controls..." keeps the caption, the size and the action of MENU-ACC-001, step 2.
  4. The row of the footer (WID-LAY-006) holds only "Done". "Done" keeps its vanilla size,
     150 × 20. The footer frame places the row with its one child (WID-LAY-011). Escape and
     "Done" act as MENU-SUB-002 states.
- **Postconditions:** at W = 427 and H = 240:
  - "Controls..." is at (58, 37). "FOV Effects" is at (58, 62). "Done" is at (138, 214).
  - The content height is 54, so the list does not scroll.

Provenance: `design` for the set of widgets, as MENU-PAUSE-006 states. "Controls..." is in the
set because mcrs has the "Controls" screen (MENU-DEV-002). `source_suggested` for the
positions. Kind: layout. Confidence: medium. Experiment: MENU-EXP-04.

---

## 4. The options of mcrs

The options of mcrs are on these screens:

| Option | Screen | Rule |
|--------|--------|------|
| FOV | the options screen, in the header | MENU-VAL-001 |
| FOV Effects | "Accessibility Settings" | MENU-VAL-002 |
| Brightness | "Video Settings", section "Display" | MENU-VAL-003 |
| Smooth Lighting | "Video Settings", section "Quality & Performance" | MENU-VAL-004 |
| Clouds | "Video Settings", section "Quality & Performance" | MENU-VAL-005 |
| GUI Scale | "Video Settings", section "Display" | MENU-VAL-006 |
| Render Distance | "Video Settings", section "Quality & Performance" | MENU-VAL-007 |
| Simulation Distance | "Video Settings", section "Quality & Performance" | MENU-VAL-008 |
| Max Framerate | "Video Settings", section "Display" | MENU-VAL-009 |
| VSync | "Video Settings", section "Display" | MENU-VAL-010 |
| Fullscreen | "Video Settings", section "Display" | MENU-VAL-011 |

The rules MENU-DIST-001 to MENU-DIST-005 state the effect of Render Distance and Simulation
Distance. The rules MENU-DISP-001 to MENU-DISP-005 state the effect of Max Framerate, VSync and
Fullscreen. `client-controls.md`, CTRL-VAL-001, states the option Sensitivity.

### MENU-VAL-001: FOV

- **Behavior:**
  1. The widget is an option slider over the integer range [30, 110] (WID-SLD-005). The step
     of a key in change mode is 1. The default is 70.
  2. The caption is `options.generic_value` with `options.fov`, "FOV", and a value text. The
     value text of 70 is `options.fov.min`, "Normal". The value text of 110 is
     `options.fov.max`, "Quake Pro". The value text of each other value is its number.
  3. The slider has no tooltip.
  4. The option applies at once (WID-SLD-007, step 1).
  5. The value is the FOV option `O` of `client-camera.md` (CCAM-OPT-001, CCAM-FOV-005).
- **Postconditions:** the captions are "FOV: Normal", "FOV: 90" and "FOV: Quake Pro". The
  field of view of the world changes in the next frame after each change.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts. Kind: semantic.
Confidence: medium. Experiment: MENU-EXP-05.

### MENU-VAL-002: FOV Effects

- **Behavior:**
  1. The widget is a slider with the position `q` from 0 to 1. The value is `k` = `q²`. The
     position of the value `k` is `√k`. The default value is 1.0.
  2. In change mode, Left and Right move the position by `1 / (w − 8)` (WID-SLD-003, step 3).
  3. When `k` is exactly 0, the caption is `options.generic_value` with
     `options.fovEffectScale`, "FOV Effects", and `options.off`, "OFF".
  4. Else the caption is `options.percent_value`, `"%s: %s%%"`, with "FOV Effects" and
     `trunc(k · 100)`.
  5. The tooltip is `options.fovEffectScale.tooltip`, "Controls how much the field of view can
     change with gameplay effects."
  6. The option applies at once (WID-SLD-007, step 1).
  7. The value is the effect scale `k` of `client-camera.md` (CCAM-FOV-001, step 6, and
     CCAM-FOV-005).
- **Postconditions:**
  - The handle at the right end shows "FOV Effects: 100%". The handle at the centre shows
    "FOV Effects: 25%". The handle at the left end shows "FOV Effects: OFF".
  - A position above 0 and below 0.1 shows "FOV Effects: 0%".

Provenance: `source_suggested`; `black_box_confirmed` for the English texts. Kind: semantic.
Confidence: medium. Experiment: MENU-EXP-05.

### MENU-VAL-003: Brightness

- **Behavior:**
  1. The widget is a slider whose value `b` is its position, from 0 to 1. The default is 0.5.
  2. In change mode, Left and Right move the position by `1 / (w − 8)` (WID-SLD-003, step 3).
  3. Let `p` = `trunc(b · 100)`. The caption is `options.generic_value` with
     `options.gamma`, "Brightness", and a value text:
     - `p` = 0: `options.gamma.min`, "Moody";
     - `p` = 50: `options.gamma.default`, "Default";
     - `p` = 100: `options.gamma.max`, "Bright";
     - else the number `p`, with no percent sign.
  4. The slider has no tooltip.
  5. The option applies at once (WID-SLD-007, step 1).
  6. The light map uses `b`. For each colour `c` of the light map, after the clamp to [0, 1],
     let `m` be the largest of its three parts. When `m` > 0, let
     `n` = `c · (1 − (1 − m)⁴) / m`. When `m` = 0, let `n` = `c`. The final colour is
     `lerp(b, c, n)`.
- **Postconditions:**
  - The captions are "Brightness: Moody", "Brightness: 75" and "Brightness: Bright".
  - With `b` = 0 the light map does not change. A higher `b` makes the dark places of the world
    brighter. The full light stays the same.
- **Edge cases:** the Darkness effect lowers `b` for the light map. That is out of scope.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts and for the curve
of step 6 in `assets/minecraft/shaders/core/lightmap.fsh` of the client jar. Kind: semantic.
Confidence: medium. Experiment: MENU-EXP-05.

### MENU-VAL-004: Smooth Lighting

- **Behavior:**
  1. The widget is a Boolean cycle button (WID-CYC-001, step 4), with the caption form "name
     and value". Its name is `options.ao`, "Smooth Lighting". The default is ON.
  2. The button has no tooltip.
  3. Each change applies at once. Each chunk mesh builds again. `block-smooth-light.md`,
     BSL-MODE-001, then selects the smooth path or the flat path.
  4. Each change writes the options file (MENU-SAVE-001).
- **Postconditions:** the captions are "Smooth Lighting: ON" and "Smooth Lighting: OFF". After
  the chunk meshes build again, the blocks show the smooth light or the flat light.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts. Kind: semantic.
Confidence: medium. Experiment: MENU-EXP-06.

### MENU-VAL-005: Clouds

- **Behavior:**
  1. The widget is a cycle button with the caption form "name and value". Its name is
     `options.renderClouds`, "Clouds".
  2. In vanilla, the values in order are OFF (`options.off`), Fast (`options.clouds.fast`) and
     Fancy (`options.clouds.fancy`). The default is Fancy.
  3. The look of each vanilla value:
     - OFF: no clouds draw.
     - Fast: each cloud cell is one flat horizontal face, with no height.
     - Fancy: each cloud cell is a box with its top, bottom and side faces. The box is 4 blocks
       high.
     - A cloud cell is 12 × 12 blocks. The cloud texture is
       `assets/minecraft/textures/environment/clouds.png` of the client jar.
  4. In mcrs, the button has two values in order: OFF, then the vanilla value whose look mcrs
     draws. The implementer selects the value that matches the one kind of clouds of mcrs.
     The default is that second value. Its text is the text of that vanilla value.
  5. The button has no tooltip.
  6. Each change applies at once and writes the options file (MENU-SAVE-001).
- **Postconditions:** in vanilla, the captions are "Clouds: OFF", "Clouds: Fast" and "Clouds:
  Fancy". In mcrs, two activations bring the caption back to its start.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts and the texture.
`design` for step 4. Kind: semantic. Confidence: medium. Experiment: MENU-EXP-06.

### MENU-VAL-006: GUI Scale

- **Behavior:**
  1. The widget is a cycle button with the caption form "name and value". Its name is
     `options.guiScale`, "GUI Scale".
  2. The values are 0, then 1 to `max` (MENU-SCALE-002). The value text of 0 is
     `options.guiScale.auto`, "Auto". The value text of each other value is its number.
  3. The default is 0.
  4. The button has no tooltip.
  5. Each change applies at once (MENU-SCALE-001) and writes the options file
     (MENU-SAVE-001).
- **Postconditions:** the captions are "GUI Scale: Auto", "GUI Scale: 1", and so on.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts. Kind: semantic.
Confidence: medium. Experiment: MENU-EXP-07.

### MENU-VAL-007: Render Distance

- **Behavior:**
  1. The widget is an option slider over the integer range [2, `hi`] (WID-SLD-005). The step
     of a key in change mode is 1.
  2. In vanilla, `hi` is 32 when the Java runtime of the client has a maximum heap of at least
     1 000 000 000 bytes. Else `hi` is 16. The default is 12.
  3. In mcrs, the range is [2, 96] and the default is 96 (MENU-DEV-008).
  4. The caption is `options.generic_value` with `options.renderDistance`, "Render Distance",
     and the value text `options.chunks`, "%s Chunks", with the number.
  5. The slider has no tooltip.
  6. The slider waits (WID-SLD-007, step 2).
  7. The stored value is the option `o` of MENU-DIST-001 to MENU-DIST-004.
- **Postconditions:** the captions are "Render Distance: 2 Chunks" and "Render Distance: 12
  Chunks". In mcrs, the right end shows "Render Distance: 96 Chunks".

Provenance: `source_suggested`; `black_box_confirmed` for the English texts. `design` for step
3: the owner decided the range and the default of mcrs on 2026-11-13. Kind: semantic.
Confidence: medium. Experiment: MENU-EXP-11.

### MENU-VAL-008: Simulation Distance

- **Behavior:**
  1. The widget is an option slider over the integer range [5, `hi`] (WID-SLD-005). The step
     of a key in change mode is 1. The default is 12.
  2. In vanilla, `hi` is 32 or 16, with the condition of MENU-VAL-007, step 2.
  3. In mcrs, `hi` is 32 (section 10, item 1).
  4. The caption is `options.generic_value` with `options.simulationDistance`, "Simulation
     Distance", and the value text `options.chunks`, "%s Chunks", with the number.
  5. The slider has no tooltip.
  6. The slider waits (WID-SLD-007, step 2).
  7. The stored value is the option `m` of MENU-DIST-005.
- **Postconditions:** the captions are "Simulation Distance: 5 Chunks" and "Simulation
  Distance: 12 Chunks".

Provenance: `source_suggested`; `black_box_confirmed` for the English texts. `unresolved` for
step 3. Kind: semantic. Confidence: medium. Experiment: MENU-EXP-12.

### MENU-VAL-009: Max Framerate

- **Behavior:**
  1. The widget is an option slider over the integer range [1, 26] (WID-SLD-005). The value of
     the option is 10 times the integer of the slider: 10, 20, …, 260.
  2. The step of a key in change mode is 1 on the slider. Thus it is 10 on the value.
  3. The default is 120.
  4. The caption is `options.generic_value` with `options.framerateLimit`, "Max Framerate",
     and a value text:
     - 260: `options.framerateLimit.max`, "Unlimited";
     - else `options.framerate`, "%s FPS", with the value.
  5. The slider has no tooltip.
  6. The option applies at once (WID-SLD-007, step 1).
  7. The value is the frame limit `L` of MENU-DISP-001.
- **Postconditions:** the captions are "Max Framerate: 10 FPS", "Max Framerate: 120 FPS" and
  "Max Framerate: Unlimited".

Provenance: `source_suggested`; `black_box_confirmed` for the English texts. Kind: semantic.
Confidence: medium. Experiment: MENU-EXP-13.

### MENU-VAL-010: VSync

- **Behavior:**
  1. The widget is a Boolean cycle button (WID-CYC-001, step 4), with the caption form "name
     and value". Its name is `options.vsync`, "VSync". In vanilla, the default is ON.
  2. In mcrs, the default is OFF (MENU-DEV-010).
  3. The button has no tooltip.
  4. Each change applies from the next frame (MENU-DISP-002).
  5. Each change writes the options file (MENU-SAVE-001).
- **Postconditions:** the captions are "VSync: ON" and "VSync: OFF".

Provenance: `source_suggested`; `black_box_confirmed` for the English texts. `design` for step
2: the owner decided the default of mcrs on 2026-11-13. Kind: semantic. Confidence: medium.
Experiment: MENU-EXP-13.

### MENU-VAL-011: Fullscreen

- **Behavior:**
  1. The widget is a Boolean cycle button (WID-CYC-001, step 4), with the caption form "name
     and value". Its name is `options.fullscreen`, "Fullscreen". The default is OFF.
  2. The button has no tooltip.
  3. Each change applies at the next frame (MENU-DISP-003, MENU-DISP-004).
  4. Each change writes the options file (MENU-SAVE-001).
  5. The key "Toggle Fullscreen" also changes the option (MENU-DISP-005).
- **Postconditions:** the captions are "Fullscreen: ON" and "Fullscreen: OFF".

Provenance: `source_suggested`; `black_box_confirmed` for the English texts. Kind: semantic.
Confidence: medium. Experiment: MENU-EXP-14.

### MENU-DIST-001: The render distance of the client

- **Inputs:** the stored value `o` of Render Distance, and the server radius `v`. `v` is the
  chunk radius of `login`, or the radius of the last `set_chunk_cache_radius` after it.
- **Behavior:**
  1. When `v` > 0, the render distance of the client is `r = min(o, v)`. Else `r = o`.
  2. A change of `o` or of `v` changes `r` from the next frame.
  3. Let (`cx`, `cz`) be the column of the camera. The client draws the sections of the column
     (`x`, `z`) only when `max(0, |x − cx| − 1)² + max(0, |z − cz| − 1)² < r²`.
  4. The far plane uses `r` (`client-camera.md`, CCAM-PROJ-001).
  5. The render-distance fog uses `r`. Let (`dx`, `dy`, `dz`) be the offset from the camera to
     a point. The fog distance of the point is `max(√(dx² + dz²), |dy|)`.
  6. The fog value is 0 at the start or nearer, 1 at the end or farther, and linear between.
     The end is `16 · r` blocks. The start is `16 · r − clamp(16 · r / 10, 4, 64)` blocks.
  7. The fog of a point is the larger of this value and the value of the environmental fog.
  8. The client keeps each column that it got until `forget_level_chunk` for that column
     arrives. A change of `r` removes no column.
- **Postconditions:**
  - With `o` = 12 on a server with `v` = 10, `r` = 10.
  - A lower `o` makes the drawn world smaller in the next frame. A higher `o` shows more columns
    only after the server sends them (MENU-DIST-003).
  - With `r` = 12, the fog starts at 172.8 blocks and ends at 192 blocks. With `r` = 2, it
    starts at 28 blocks and ends at 32 blocks.
- **Edge cases:** the client can drop a column outside the square of the radius `max(2, v) + 3`
  around the column of the last `set_chunk_cache_center`. A vanilla server sends no column
  outside that square.

Provenance: `source_suggested`; `black_box_confirmed` for the form of the fog of steps 5 to 7 in
`assets/minecraft/shaders/include/fog.glsl` of the client jar. Kind: semantic. Confidence:
medium. Experiment: MENU-EXP-11.

### MENU-DIST-002: The view distance that the client sends

- **Observer:** the server, on the wire.
- **Inputs:** the stored value `o` of Render Distance, and the moments of MENU-SAVE-001 and
  MENU-SAVE-002.
- **Behavior:**
  1. In the configuration state, the client sends `client_information` (configuration,
     serverbound 0) one time, at the start of the state.
  2. In the play state, at each moment of MENU-SAVE-001 and MENU-SAVE-002, the client makes its
     client information from the current options. This occurs also when the write of the
     options file fails.
  3. When that information differs from the last `client_information` that the client sent in
     the play state, the client sends `client_information` (play, serverbound 14). The first
     moment in the play state always sends it.
  4. The field "view distance" holds `o`, not `r`. The other fields are out of scope.
  5. The client sends `client_information` at no other moment.
- **Postconditions:**
  - A move of the Render Distance slider sends nothing. "Done" sends one `client_information`
    with the new value, because the close stores the pending value first (MENU-SUB-002).
  - A press of a cycle button sends the stored value of Render Distance when it differs from
    the value that the client sent last. A pending value is not stored, so it does not go.
  - In mcrs, `o` can be up to 96. A vanilla server clamps it (MENU-DIST-003).

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: MENU-EXP-11.

### MENU-DIST-003: The view of a player on the server

- **Observer:** the client of the player, on the wire.
- **Inputs:** the view distance `q` of the last `client_information` of the player, the server
  view distance `V` (MENU-DIST-004), and the column (`cx`, `cz`) of the player.
- **Behavior:**
  1. The view distance of the player is `d = clamp(q, 2, V)`.
  2. The view of the player holds the column (`x`, `z`) when
     `max(0, |x − cx| − 2)² + max(0, |z − cz| − 2)² < d²`.
  3. When `client_information` arrives, the server stores `q`. At its next tick, the server
     computes `d` again.
  4. When `d` changes, the server first sends `forget_level_chunk` for each column that left
     the view and that the client got.
  5. A column that left the view before the server sent it gets no packet.
  6. Then the server sends the columns that entered the view, in the batches of the chunk send:
     `chunk_batch_start`, one `level_chunk_with_light` for each column, then
     `chunk_batch_finished`. The client answers each batch with `chunk_batch_received`.
  7. The columns nearest to the player go first.
  8. A change of `q` sends no `set_chunk_cache_radius`.
- **Postconditions:**
  - With `V` = 10, a change of `q` from 20 to 30 sends nothing.
  - With `V` = 10, a change of `q` from 12 to 6 sends `forget_level_chunk` for each column of
    the view of `d` = 10 that is not in the view of `d` = 6.
- **Edge cases:**
  - The load tickets of the player use `V`, not `d` (`block-simulation.md`, SIM-TICK-005).
  - The tracking range of entities uses `d` (`entity-tracking.md`, TRK-RNG-001). The server
    tests it at the times of TRK-RNG-003.
  - A dimension change keeps `d` (`dimension-change.md`, DIM-CHG-007).

Provenance: `source_suggested`. Kind: wire for the packets, semantic for the order of the
columns. Confidence: medium. Experiment: MENU-EXP-11.

### MENU-DIST-004: Render Distance in a local world

- **Observer:** the client of each player of a local world, on the wire.
- **Inputs:** the stored value `o` of Render Distance of the host. A pending value is not an
  input.
- **Behavior:**
  1. At the end of each tick that it runs, the server of a local world compares `max(2, o)`
     with its server radius `v`.
  2. When they differ, the server sets `v` to `max(2, o)`. It sends `set_chunk_cache_radius`
     with `v` to each player: the host and each player that joined over LAN.
  3. Then the server view distance becomes `V = clamp(v, 2, 32)`. The server computes the view
     of each player again at once (MENU-DIST-003).
  4. The `login` of a player carries the current `v` as its chunk radius.
  5. While the server of a local world runs no tick, a change of `o` waits. The pause of a
     local world is out of scope (section 0.5).
  6. A server that the client does not run takes `V` from its own configuration. A vanilla
     dedicated server clamps its property `view-distance` to [2, 32].
- **Postconditions:**
  - A player that joined over LAN has `d = clamp(q, 2, V)` with its own `q`. Its client draws
    with `r = min(o', v)`, where `o'` is its own option (MENU-DIST-001).
  - When the host lowers `o`, its client draws the smaller world in the next frame. The server
    lowers `V` at the end of its next tick.
  - When the host raises `o`, the host keeps `d = clamp(q, 2, V)` with its old `q`. The new
    columns arrive only after the next `client_information` of the host (MENU-DIST-002), for
    example at "Done".
- **Edge cases:** in mcrs, `V = clamp(v, 2, 96)` (MENU-DEV-008).

Provenance: `source_suggested`. `design` for the edge case: the server of mcrs accepts the range
of Render Distance of mcrs (MENU-VAL-007, step 3). Kind: wire. Confidence: medium. Experiment:
MENU-EXP-12.

### MENU-DIST-005: Simulation Distance in a local world

- **Observer:** the client of each player of a local world, on the wire, and a player who sees
  the blocks and the entities of the world.
- **Inputs:** the stored value `m` of Simulation Distance of the host.
- **Behavior:**
  1. At the end of each tick that it runs, the server of a local world compares `max(2, m)`
     with the value that it took last.
  2. At the first tick that runs, and when the two values differ, the server takes `max(2, m)`
     as its simulation distance `S`. It sends `set_simulation_distance` with `S` to each player.
  3. `S` is the simulation distance of `block-simulation.md`, SIM-TICK-005, in each dimension.
  4. The `login` of a player that joins over LAN carries the current `S`.
  5. On a server that the client does not run, the option has no effect. `client_information`
     has no field for it.
- **Postconditions:** with `m` = 5, the simulation level of a column at the Chebyshev distance
  `d` from a player is `26 + d` (SIM-TICK-005).
- **Edge cases:** the client keeps the value of `login` and of the last
  `set_simulation_distance`. The use of that value in the client is out of scope.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: MENU-EXP-12.

### MENU-DISP-001: The frame limit

- **Inputs:** the value `L` of Max Framerate, in a world.
- **Behavior:**
  1. When `L` < 260, the client waits after the present of each frame. The wait ends when
     `1 / L` seconds passed since the end of the previous wait.
  2. When `L` = 260, the client does not wait.
  3. An open screen, the pause screen and a loss of the window focus do not change `L`.
  4. VSync (MENU-DISP-002) and `L` both apply.
- **Postconditions:**
  - With `L` = 60 and VSync OFF, the client draws 60 frames or fewer in each second.
  - With VSync ON, the frame rate is not above the lower of `L` and the refresh rate of the
    monitor.
- **Edge cases:** the frame limits of "Reduce FPS When" for a minimised window and for an
  inactive player are out of scope.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: MENU-EXP-13.

### MENU-DISP-002: VSync and the present

- **Inputs:** the value of VSync.
- **Behavior:**
  1. With VSync ON, each present waits for the vertical blank of the monitor. This is the
     present mode FIFO of the graphics API.
  2. With VSync OFF, a present does not wait for the vertical blank. The present mode is
     Immediate when the surface supports it, else Mailbox, else FIFO.
  3. A change applies from the next frame.
- **Postconditions:**
  - With VSync ON and Max Framerate "Unlimited", the frame rate is not above the refresh rate
    of the monitor.
  - With VSync OFF on a surface that supports only FIFO, the result is the same as with VSync
    ON.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: MENU-EXP-13.

### MENU-DISP-003: Fullscreen in vanilla

- **Inputs:** the value of Fullscreen, and the vanilla option "Exclusive Fullscreen".
- **Behavior:**
  1. A change applies at the next frame.
  2. ON with "Exclusive Fullscreen" OFF, the default: the window becomes a borderless window
     over the bounds of the monitor that holds the window.
  3. ON with "Exclusive Fullscreen" ON: the window takes an exclusive video mode of that
     monitor. This case is out of scope.
  4. OFF: the window gets back the position and the size that it had before fullscreen.
  5. When the switch fails, the option takes the real state of the window. The client then
     writes the options file.
  6. When the system changes the fullscreen state of the window, the option takes the new
     state. The client writes the options file. The Fullscreen button of an open "Video
     Settings" shows the new value.
- **Postconditions:** the window pixels change. MENU-SCALE-001, step 4, computes the GUI scale
  again, and the open screen re-arranges.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: MENU-EXP-14.

### MENU-DISP-004: Fullscreen (mcrs subset)

- **Behavior:**
  1. ON: the window becomes a borderless window that covers the primary monitor of the system.
  2. mcrs has no exclusive video mode and no list of video modes. "Exclusive Fullscreen" and
     "Exclusive Fullscreen Mode" are not options of mcrs.
  3. Steps 1, 4, 5 and 6 of MENU-DISP-003 apply.
- **Postconditions:** for a window on a second monitor, ON moves the window to the primary
  monitor. OFF puts it back at its old position on the second monitor.

Provenance: `design`: the owner decided on 2026-11-13 that fullscreen in mcrs is a borderless
window on the primary monitor. `source_suggested` for step 3. Kind: semantic. Confidence:
medium. Experiment: MENU-EXP-14. mcrs deviates: MENU-DEV-009.

### MENU-DISP-005: The key "Toggle Fullscreen"

- **Inputs:** a press of the key of `key.fullscreen`, "Toggle Fullscreen". The default key is
  F11. The player can bind the action to another key or to a mouse button.
- **Behavior:**
  1. A press inverts the option Fullscreen. A repeat of a held key does nothing.
  2. While the key of `key.debug.modifier`, "Debug Modifier Key", is down, the press does
     nothing. The default of that key is F3.
  3. The key acts in a world with no screen and with each screen of this document open. It
     also acts while an edit box captures the keyboard (`client-edit-box.md`, EDIT-KEY-005).
  4. The new value applies as MENU-DISP-003 and MENU-DISP-004 state.
  5. The client writes the options file (MENU-SAVE-001, step 3).
  6. The Fullscreen button of an open "Video Settings" shows the new value.
- **Postconditions:** two presses put the window back at its old position and size.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts. Kind: semantic.
Confidence: medium. Experiment: MENU-EXP-14.

---

## 5. GUI scale

### MENU-SCALE-001: Computation of the GUI scale

- **Inputs:** the window pixels `pw` and `ph`, and the value `g` of the option GUI Scale.
- **Behavior:**
  1. Let `s` = 1.
  2. While all of these are true, add 1 to `s`:
     - `s` ≠ `g`;
     - `s < pw` and `s < ph`;
     - `pw / (s + 1)` ≥ 320 and `ph / (s + 1)` ≥ 240, with the integer division.
  3. `s` is the GUI scale. W = `ceil(pw / s)` and H = `ceil(ph / s)`.
  4. The client computes `s` again after each change of `g` and after each change of the
     window size. Then the open screen re-arranges with the new W and H (WID-SCR-003).
- **Postconditions:**
  - With `g` = 0, `s` is the largest scale that fits. "Auto" means this scale.
  - With `g` ≥ 1, `s` = `min(g, ` the scale of "Auto"`)`.
  - Window pixels of 854 × 480 give the scale 2 for "Auto", with W = 427 and H = 240.
    Window pixels of 1920 × 1080 give the scale 4.
- **Edge cases:** on a screen that keeps its widgets, the widgets keep their values and the
  focus. After a change by the GUI Scale button, the list of "Video Settings" keeps its scroll
  offset (LIST-GEO-001, step 3).

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: MENU-EXP-07.

### MENU-SCALE-002: Values of the GUI Scale button

- **Inputs:** the stored value `v` of the option GUI Scale, the window pixels at the moment of
  each activation of the button, and the state of Shift.
- **Behavior:**
  1. At each activation, let `max` be the scale of "Auto" for the window pixels of that moment
     (MENU-SCALE-001 with `g` = 0).
  2. The values of the button are 0, 1, …, `max`, in that order. The list follows the window
     size of the moment of the activation, not the window size when the screen built the
     button.
  3. An activation with no Shift selects `v + 1` when `v + 1` ≤ `max`, else 0.
  4. An activation with Shift selects `max` when `v` = 0 or `v` > `max`, else `v − 1`.
  5. The option stores the selected value. The caption shows the stored value.
- **Postconditions:**
  - At 854 × 480 window pixels, the values are Auto, 1 and 2. An activation of "GUI Scale: 2"
    selects "GUI Scale: Auto" (WID-CYC-002, step 1).
  - The caption always agrees with the stored value.
- **Edge cases:** after the window becomes smaller while "Video Settings" is open, the next
  activation uses the new `max`. A stored value above `max` gives the scale of "Auto"
  (MENU-SCALE-001).

Provenance: `design` for step 2: the caption must agree with the stored value after a change
of the window size (MENU-DEV-004). `source_suggested` for the other steps. Kind: semantic.
Confidence: medium. Experiment: MENU-EXP-07.

### MENU-SCALE-003: Ctrl wheel on "Video Settings"

- **Inputs:** a wheel event with the vertical value `y`, while Ctrl is down and "Video Settings"
  is open.
- **Behavior:**
  1. Let `max` be the scale of "Auto" for the current window pixels. Let `old` be the value of
     the option GUI Scale, or `max + 1` when that value is 0.
  2. Let `new` = `old + sign(y)`.
  3. When 1 ≤ `new` ≤ `max`, the option gets `new`, and the GUI Scale button shows `new`. The
     GUI scale applies at once (MENU-SCALE-001). The scroll offset of the list becomes 0.
  4. Else nothing changes.
  5. While Ctrl is down, the wheel never scrolls the list and never goes to a widget.
- **Postconditions:**
  - The wheel turned away from the player makes the GUI larger. From "Auto", the wheel turned
    toward the player selects `max`. The wheel never selects "Auto".
  - This change does not write the options file at once (MENU-SAVE-001).
- **Edge cases:** the wheel with no Ctrl scrolls the list (`client-list.md`, SCRL-AREA-002). The
  GUI Scale button does not get it.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: MENU-EXP-07.

---

## 6. Options file

### MENU-SAVE-001: When the client writes the options file

- **Observer:** the options file, as the client reads it at the next start.
- **Behavior:** the client writes the options file at these moments:
  1. after each change of the value of an option cycle button by an activation. Smooth
     Lighting, Clouds, GUI Scale, VSync and Fullscreen are such buttons;
  2. each time that the options screen or an option sub-screen is removed: it closes, or
     another screen replaces it. For example, the options screen is removed when a sub-screen
     opens;
  3. after each change of Fullscreen by the key "Toggle Fullscreen", by a switch of the window
     that fails, or by the system (MENU-DISP-003, MENU-DISP-005).
- **Postconditions:**
  - Each moment of this rule also sends `client_information` when its content changed
    (MENU-DIST-002).
  - A change of a slider writes nothing at once. The write of step 2 includes it.
  - A change of the GUI scale by the Ctrl wheel (MENU-SCALE-003) writes nothing at once. The
    write of step 2 includes it.
  - The close of the pause screen writes nothing.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: MENU-EXP-08.

### MENU-SAVE-002: Stop of the client

- **Observer:** the options file, as the client reads it at the next start.
- **Inputs:** a stop of the client, and the screen that is open at that moment.
- **Behavior:**
  1. At a stop of the client, the client removes the screen that is open.
  2. When that screen is the options screen or an option sub-screen, MENU-SAVE-001, step 2,
     writes the options file.
  3. The stop of the client does not write the options file at another moment.
- **Postconditions:**
  - The write of step 2 occurs also when no option changed. The client does not compare the
    options with the content of the file.
  - With the options screen or an option sub-screen open, the file gets each change that waits
    for MENU-SAVE-001, step 2. Examples: a change of a slider, and a change by the Ctrl wheel.
  - With no such screen open, the stop writes nothing. The pause screen is not such a screen.
    Thus the last button of the pause screen (MENU-PAUSE-006, step 5) writes nothing.
  - After a stop of the client, the file holds the last value of each option of section 4.
    MENU-SAVE-003 states the exception.
- **Edge cases:** the client removes only the screen that is open. When an option sub-screen is
  open, the options screen was removed when the sub-screen opened. Thus a stop writes the file
  one time at most.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: MENU-EXP-09.
mcrs deviates: MENU-DEV-005.

### MENU-SAVE-003: A write that fails

- **Observer:** the player, and the options file as the client reads it at the next start.
- **Inputs:** a write of the options file (MENU-SAVE-001, MENU-SAVE-002) that the client cannot
  complete. For example, the file is read-only, its directory does not exist, or the disk is
  full.
- **Behavior:**
  1. The client does not try the write again. No timer and no later attempt exists for it.
  2. The options in memory keep their new values. Each change applies as section 4 states, also
     when its write fails.
  3. The client shows no message. Each screen acts as after a write that succeeds. For example,
     "Done" opens the parent screen.
  4. The next write occurs at the next moment of MENU-SAVE-001 or MENU-SAVE-002. Each write
     stores the value that each option has in memory at that moment.
- **Postconditions:**
  - When a later write succeeds, the file holds the value that each option has in memory at that
    moment. For the option of the failed write, this is the value of that change, or a later
    value when the option changed again.
  - When no later write succeeds, the next start gets the values that the file holds. These are
    the old values when the client could not open the file. They are the result of MENU-SAVE-004
    when the write failed after the open, and that result can hold the change.
  - When the client cannot open the file, the file keeps the content that it had before the
    write. A read-only file is an example.
  - When the write fails after the client opened the file, the file holds the result of
    MENU-SAVE-004. A full disk is an example.
- **Edge cases:** the log of the client is not an observer of this rule. The reference writes an
  error line to its log when it cannot open the file. It writes no line when the write fails
  after the open. An implementation can write a line for each write that fails.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: MENU-EXP-10.
mcrs deviates from step 1: MENU-DEV-006.

### MENU-SAVE-004: A write that stops before its end

- **Observer:** the options file, as the client reads it at the next start.
- **Inputs:** a write of the options file that stops before its end. For example, the process
  ends, the computer loses power, or the disk becomes full.
- **Behavior:**
  1. The client writes the new content into the options file itself. It uses no other file: no
     temporary file, no copy of the old content and no rename.
  2. When the write starts, the file loses its old content. Then the new content goes into the
     file from its first line to its last line.
  3. The client does not wait until the disk holds the content.
- **Postconditions:**
  - After a write that stops before its end, the file is empty or holds the first part of the
    new content. The old content is lost, and no other file holds it.
  - The last line in the file can be cut. Its value can then differ from the value in memory.
  - At the next start, the client reads the options that the file holds. Each other option has
    its default value.
  - Because of step 3, a loss of power soon after a write can also leave the file empty or cut.
- **Edge cases:** a write that cannot open the file does not change the file (MENU-SAVE-003).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium for steps 1 and 2, low for the
content after a loss of power. Experiment: MENU-EXP-10. mcrs deviates: MENU-DEV-007.

---

## 7. Screen properties

### MENU-PROP-001: Properties of the screens of this document

- **Behavior:** these are the properties that `client-widget.md`, section 16, items 5 and 6,
  leave open:

| Screen | In-game screen | Own background | Separators | Re-arrange (WID-SCR-003) | Tab order group | Sliders that wait |
|--------|----------------|----------------|------------|--------------------------|-----------------|-------------------|
| Pause screen | no | no | none | builds its widgets again | 0 | none |
| Options screen | no | no | none | keeps its widgets | 0 | none |
| "Video Settings" | no | no | of the list (LIST-BG-002) | keeps its widgets | 0 | vanilla: Biome Blend, Render Distance, Simulation Distance; mcrs: Render Distance, Simulation Distance |
| "Accessibility Settings" | no | no | of the list (LIST-BG-002) | keeps its widgets | 0 | none |

- **Postconditions:**
  - Each screen of this document draws the background of WID-SCR-004, step 2. In a world, that
    is the blur of WID-SCR-005 and then the texture `inworld_menu_background.png`.
  - The pause screen and the options screen draw no separators of WID-SCR-006.
  - No screen of this document names an initial widget (WID-FOC-002, step 1).
  - Of the options of mcrs, only Render Distance and Simulation Distance wait 600 ms
    (WID-SLD-007, step 2). Each other option applies at once.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: MENU-EXP-03.

---

## 8. Experiments

None of these experiments ran. No game window started for this document. Each experiment uses
the setup of `client-widget.md`, section 14. That setup starts the vanilla client of the pinned
version with the input agent of `tools/vanilla-client`. It sets window pixels of 854 × 480 and
the GUI scale 2. `tools/mcrs_minecraft_client_recorder` records each step. "Record the file
time" means: record the time of the last change of `options.txt` in the game directory.

### MENU-EXP-01: Pause screen

1. In a singleplayer world, press Escape. Record the screenshot and the focused widget.
2. Record the bounds of the title and of each button. Compare them with MENU-PAUSE-002 and
   MENU-PAUSE-003.
3. Put the mouse on each icon button. Record its tooltip.
4. Press Tab six times. Record the focused widget after each key.
5. Press Escape. Record that the game takes the mouse pointer again.
6. Join a multiplayer server. Open the pause screen. Record the caption of the last button and
   the state of "Other Players".
7. Press the last button. Record the screen that shows.

Decides: MENU-PAUSE-001 to MENU-PAUSE-005, and the positions of MENU-PAUSE-006.

### MENU-EXP-02: Options screen

1. Open the pause screen, then "Options...". Record the bounds of each widget.
2. Compare them with MENU-OPT-002.
3. Press Escape. Record that the pause screen shows.
4. Open "Options..." again and press "Done". Record that the pause screen shows.

Decides: MENU-OPT-001, MENU-OPT-002, and the positions of MENU-OPT-003.

### MENU-EXP-03: "Video Settings"

1. Open "Video Settings...". Record the screenshot, the title and the bounds of each row.
2. Record the GUI rows of pixels at y = 31, 32 and at y = 207, 208.
3. Change the GUI scale with the window open. Record the scroll offset and the focused widget.
4. Press Escape. Record the screen that shows.

Decides: MENU-SUB-001, MENU-SUB-002, MENU-VID-001, MENU-PROP-001, and the positions of
MENU-VID-002.

### MENU-EXP-04: "Accessibility Settings"

1. Open "Accessibility Settings...". Record the first row, the row of "FOV Effects" and the
   footer bounds.
2. Press "Controls..." in the first row. Record the screen that shows. Press Escape. Record the
   screen that shows.
3. Press "Done". Record the screen that shows.

Decides: MENU-ACC-001, and the positions of MENU-ACC-002.

### MENU-EXP-05: The sliders

1. Drag the FOV slider to each end and to the centre. Record the caption and the field of view.
2. Set "FOV Effects" to the left end, the centre, and one key step right of the left end.
   Record the caption.
3. Set "Brightness" to 0, 0.5, 0.75 and 1. Record the caption.
4. In a dark place of a world, record a screenshot at each value of step 3.

Decides: MENU-VAL-001, MENU-VAL-002, MENU-VAL-003.

### MENU-EXP-06: Smooth Lighting and Clouds

1. Look at a block in a corner with light from one side. Press "Smooth Lighting". Record a
   screenshot after each press.
2. Press "Clouds" three times. Record the caption and a screenshot of the sky after each press.
3. Record the file time after each press.

Decides: MENU-VAL-004, MENU-VAL-005.

### MENU-EXP-07: GUI scale

1. Use the window pixels 854 × 480, 1280 × 720 and 1920 × 1080. For each, open
   "Video Settings". Press "GUI Scale" until it shows "Auto" again. Record each caption and W.
2. Hold Ctrl and turn the wheel one notch in each direction from "Auto", 1 and `max`. Record the
   caption, W, the scroll offset and the file time.

Decides: MENU-VAL-006, MENU-SCALE-001, MENU-SCALE-003, and steps 1, 3, 4 and 5 of
MENU-SCALE-002.

### MENU-EXP-08: Options file

1. Open "Options...". Move the FOV slider. Record the file time.
2. Open "Video Settings...". Record the file time.
3. Move the "Brightness" slider. Record the file time. Press "Done". Record the file time.
4. Press "Clouds". Record the file time.
5. Open the pause screen and press Escape. Record the file time.

Decides: MENU-SAVE-001.

### MENU-EXP-09: Stop of the client

1. In a world, open "Options...". Move the FOV slider. Record the file time.
2. Close the window. Record the file time and the FOV value in `options.txt`.
3. Start the client. In a world, open "Video Settings...". Record the file time.
4. Close the window with no change. Record the file time.
5. Start the client. In a world, with no screen open, record the file time.
6. Close the window. Record the file time.
7. Start the client. Open "Video Settings...". Hold Ctrl and turn the wheel one notch.
8. Close the window. Record the GUI scale value in `options.txt`.

Decides: MENU-SAVE-002.

### MENU-EXP-10: A write that fails or stops before its end

1. Make `options.txt` read-only. In a world, open "Video Settings...". Press "Clouds".
2. Record the caption, a screenshot of the sky, the new log lines and the content of the file.
3. Press "Done". Record the screen that shows and the file time.
4. Make the file writable. Open "Video Settings..." and press "Done". Record the Clouds value in
   the file.
5. Make the file read-only. Open "Video Settings...", press "Clouds" and press "Done".
6. Close the window. Make the file writable. Start the client. Record the caption of "Clouds".
7. Start the client under a system call tracer. Press "Clouds".
8. Record each call that opens, truncates, writes, renames or deletes a file of the game
   directory during the press.
9. Repeat step 7 with a tracer that ends the process at the first write call on `options.txt`.
   Record the size and the content of the file.
10. Start the client. Record the value of each option of section 4.

Decides: MENU-SAVE-003 and MENU-SAVE-004.

### MENU-EXP-11: Render Distance on a server

Start the vanilla server with `tools/vanilla-server`, with `view-distance=10` and a flat world.
The client joins through the proxy of `tools/mcrs_minecraft_client_recorder`. The proxy records
each packet with its time.

1. Join the server. Record the chunk radius of `login` and the view distance of each
   `client_information`.
2. Open "Video Settings". Record the caption at the default and at each end of Render Distance
   and of Simulation Distance.
3. Drag Render Distance to 6. Wait 1 second. Record the packets and a screenshot.
4. Press "Done". Record `client_information` and the packets of the next 5 seconds.
5. Set Render Distance to 20 and press "Done". Record the packets of the next 10 seconds.
6. Set Render Distance to 30 and press "Done". Record each packet of the next 10 seconds.
7. Open "Video Settings". Drag Render Distance to 8, and press "Clouds" within 300 ms. Record
   each `client_information`.
8. Wait 1 second and press "Clouds" again. Record each `client_information`.
9. In clear weather, set Render Distance to 2, then to 10. Record a screenshot toward the
   horizon at each value. Measure the distances where the fog starts and ends.

Decides: MENU-VAL-007, MENU-DIST-001, MENU-DIST-002, MENU-DIST-003.

### MENU-EXP-12: A local world and LAN

1. Open a new singleplayer world with the default options. Open it to LAN.
2. Start a second vanilla client with `tools/vanilla-client`. Join the LAN world through the
   proxy. Record the chunk radius and the simulation distance of its `login`.
3. On the host, open "Video Settings" and drag Render Distance to 6. Record the time of each
   `set_chunk_cache_radius` at the second client.
4. On the host, drag Render Distance to 20. Do not press "Done". After 5 seconds, record a
   screenshot of the host and the packets of the second client.
5. Press "Done" on the host. After 10 seconds, record a screenshot of the host.
6. On the host, drag Simulation Distance to 5. Record each `set_simulation_distance` at the
   second client.
7. Open a singleplayer world that is not open to LAN. Open the pause screen and change Render
   Distance.
8. Return to the game. Record the new log lines of the game and their times.

Decides: MENU-VAL-008, MENU-DIST-004, MENU-DIST-005.

### MENU-EXP-13: Max Framerate and VSync

Keep the window open and not minimised. The debug screen shows the frame rate.

1. In a world with no screen open, set VSync to OFF.
2. Set Max Framerate to 30, 60, 120 and "Unlimited". At each value, record the frame rate for
   10 seconds.
3. Set Max Framerate to 60. Open the pause screen and record the frame rate. Move the focus to
   another window and record the frame rate.
4. Set VSync to ON and Max Framerate to "Unlimited". Record the frame rate and the refresh rate
   of the monitor.
5. Record the caption of Max Framerate at each end, and one key step from each end.
6. Record the file time after a move of Max Framerate, and after a press of "VSync".

Decides: MENU-VAL-009, MENU-VAL-010, MENU-DISP-001, MENU-DISP-002.

### MENU-EXP-14: Fullscreen and the key "Toggle Fullscreen"

1. Put a window of 854 × 480 at a known position on the primary monitor. Open "Video
   Settings".
2. Press "Fullscreen". Record the window bounds, the monitor bounds, the window pixels, the GUI
   scale, the caption and the file time.
3. Press "Fullscreen" again. Record the window bounds.
4. Close the screens. Press F11 with no screen open, then on the pause screen, then in the chat
   box. After each press, record the state of the window and the file time.
5. Open "Video Settings" and press F11. Record the caption of "Fullscreen".
6. Hold F3 and press F11. Record the state of the window.
7. Hold F11 for 2 seconds. Record the number of changes of the window state.
8. On macOS, press the green button of the window. Record the caption of "Fullscreen" and its
   value in `options.txt`.
9. Connect a second monitor and move the window to it. Press "Fullscreen". Record the monitor
   that the window covers.

Decides: MENU-VAL-011, MENU-DISP-003, MENU-DISP-005.

---

## 9. Intentional deviations

### MENU-DEV-001: The last button of the pause screen closes the client

- **Rule:** MENU-PAUSE-006, step 5.
- **Reference:** the client leaves the world. Then it shows the title screen or the list of
  multiplayer servers (MENU-PAUSE-005).
- **Visible to the client:** yes. The window closes in place of a menu. mcrs has no title
  screen.
- **Test:** a differential test does not compare the screen after the activation.

### MENU-DEV-002: The screens show only the widgets of mcrs

- **Rule:** MENU-PAUSE-006, MENU-OPT-003, MENU-VID-002, MENU-ACC-002.
- **Reference:** each screen shows all the widgets of MENU-PAUSE-003, MENU-OPT-002,
  MENU-VID-001 and MENU-ACC-001.
- **Visible to the client:** yes. The screens have fewer widgets, and the layout rules place the
  other widgets at other positions.
- **Test:** a differential test compares the bounds of each widget of mcrs with the rules of
  the mcrs subset. It does not compare them with a vanilla capture.

### MENU-DEV-003: The Clouds button has two values

- **Rule:** MENU-VAL-005, step 4.
- **Reference:** three values: OFF, Fast and Fancy.
- **Visible to the client:** yes. The caption shows only OFF and one vanilla value.
- **Test:** a differential test compares only those two values.

### MENU-DEV-004: The GUI Scale button follows the window size

- **Rule:** MENU-SCALE-002, step 2.
- **Reference:** the button gets its list of values one time, when the screen builds it. The
  list keeps the old `max` after the window becomes smaller while "Video Settings" is open.
  The player can then select a value above the new `max`. The client stores `max`, but the
  caption shows the selected value.
- **Visible to the client:** yes, only after the window becomes smaller while "Video Settings"
  is open. In mcrs the caption agrees with the stored value.
- **Test:** a differential test does not compare the GUI Scale caption after a change of the
  window size while "Video Settings" is open.

### MENU-DEV-005: The stop of the client writes only a change that waits

- **Rule:** MENU-SAVE-002.
- **Reference:** the rule. The stop writes the file when the options screen or an option
  sub-screen is open, also when no option changed. With another screen open or none, the stop
  writes nothing.
- **mcrs:**
  1. At a stop of the client, mcrs lets a write that still runs come to its end.
  2. Then mcrs writes the file when a change waits for its write, or when the last write that
     ended before the stop failed (MENU-DEV-006). A change of a slider that the player still
     holds is a change that waits. The screen that is open has no effect.
  3. When no change waits and no write that ended before the stop failed, the stop writes
     nothing. The removal of the options screen or of an option sub-screen at the stop does not
     write the file.
  4. When the write of step 1 fails, it does not count as a failed write for step 2. When the
     write of step 2 fails, mcrs does not try it again.
- **Reason:** when no write failed, the file already holds each change that does not wait. A
  write of the same values at the stop adds nothing for the next start.
- **Visible to the client:** no, in normal play. After a stop, the file holds the last value of
  each option, as in the reference. The time of the file can differ. MENU-DEV-006 states the
  result after a write that failed.
- **Test:** a differential test does not compare the time of the file after a stop
  (MENU-EXP-09). It compares the values in the file.

### MENU-DEV-006: A write that failed occurs again at the stop

- **Rule:** MENU-SAVE-003, step 1.
- **Reference:** the client does not try a write that failed again. A later write of
  MENU-SAVE-001 or MENU-SAVE-002 that succeeds stores the value that each option has at that
  moment. With no such write, the file holds what the postconditions of MENU-SAVE-003 state:
  the old content, or the result of MENU-SAVE-004.
- **mcrs:**
  1. mcrs keeps the fact that the last write failed.
  2. At the stop of the client, mcrs writes the file once more (MENU-DEV-005, step 2), with any
     screen open or none. A write that fails during the stop is not tried again (MENU-DEV-005,
     step 4).
  3. mcrs does not try the write again on a timer or on each frame.
  4. A later write that succeeds clears the fact of step 1.
  5. mcrs writes a warning to its log for each write that fails. The log is not an observer
     (MENU-SAVE-003, edge cases).
  6. Steps 2 to 4 of MENU-SAVE-003 apply to mcrs.
- **Reason:** without the write at the stop, the player loses a change that the screens showed
  as applied.
- **Visible to the client:** yes, after a write that failed, when the write at the stop
  succeeds. At the next start, mcrs has the last value of each option. The reference has the
  values that its file holds: the old values, or the result of MENU-SAVE-004, which can hold a
  part of the new values.
- **Test:** a differential test does not compare the options at the next start after a write
  that failed (MENU-EXP-10, steps 5 and 6).

### MENU-DEV-007: The options file holds the old or the new content whole

- **Rule:** MENU-SAVE-004.
- **Reference:** the client writes into the options file itself and does not wait for the disk.
  After a write that stops before its end, the file is empty or cut. At the next start, each
  option that the file does not hold gets its default value.
- **mcrs:**
  1. mcrs writes the new content to a second file beside the options file.
  2. mcrs waits until the disk holds the content of the second file.
  3. mcrs then renames the second file over the options file.
  - After a write that stops before its end, the options file holds the old content whole or
    the new content whole. The second file can stay with a part of the new content. The next
    write replaces it.
  - A write that fails at a step does not change the options file. Thus the last postcondition
    of MENU-SAVE-003 does not apply to mcrs.
- **Reason:** the native client of mcrs refuses to start on an options file that it cannot read.
  The game gives each missing option its default value.
- **Visible to the client:** yes, only after a crash, a kill of the process or a loss of power
  during a write. mcrs starts with the old or the new options. The reference can start with
  default values.
- **Test:** a differential test does not compare the file after a write that stops before its
  end (MENU-EXP-10, steps 7 to 10). Because of the rename, a read-only options file in a
  writable directory can let a write succeed. A test of MENU-SAVE-003 against mcrs makes the
  directory read-only in place of the file.

### MENU-DEV-008: The range and the default of Render Distance

- **Rule:** MENU-VAL-007, step 3, and the edge case of MENU-DIST-004.
- **Reference:** the range is [2, 32], or [2, 16] with a small heap of the Java runtime. The
  default is 12. Each vanilla server clamps its server view distance to [2, 32].
- **mcrs:**
  1. The range is [2, 96]. The default is 96.
  2. The server of mcrs clamps its server view distance to [2, 96]. In a local world, the view
     of a player can thus reach 96 columns.
  3. The largest Simulation Distance is 32 on each client (MENU-VAL-008, step 3).
- **Reason:** the owner decided the range and the default on 2026-11-13. mcrs does not run in a
  Java runtime, so no condition on a heap applies.
- **Visible to the client:** yes. The slider of mcrs goes to 96, and a new client starts with
  96. A vanilla server clamps the request of 96 to its own server view distance
  (MENU-DIST-003).
- **Test:** a differential test does not compare the range, the default, or a view with a
  distance above 32.

### MENU-DEV-009: Fullscreen covers the primary monitor

- **Rule:** MENU-DISP-004.
- **Reference:** the borderless window covers the monitor that holds the window. With
  "Exclusive Fullscreen" ON, the window takes an exclusive video mode.
- **mcrs:** the borderless window always covers the primary monitor. mcrs has no exclusive
  fullscreen.
- **Reason:** the owner decided this on 2026-11-13. `DECISIONS.md` records that the window
  system of mcrs did not find the monitor that holds the window.
- **Visible to the client:** yes, only with more than one monitor, when the window is not on the
  primary monitor.
- **Test:** a differential test uses one monitor, or it does not compare the monitor that the
  window covers.

### MENU-DEV-010: VSync is OFF by default

- **Rule:** MENU-VAL-010, step 2.
- **Reference:** the default of VSync is ON.
- **mcrs:** the default is OFF. A client with no options file, or with no value of VSync in
  it, starts with VSync OFF.
- **Reason:** the owner decided this on 2026-11-13.
- **Visible to the client:** yes, at the first start of the client. The frame rate is not bound
  to the refresh rate of the monitor, and the image can tear (MENU-DISP-002).
- **Test:** a differential test writes the value of VSync into the options file before it
  compares. It does not compare the default.

---

## 10. Unresolved

1. The largest Simulation Distance of mcrs. MENU-VAL-008 takes 32, the vanilla value for a
   client with a large heap. The owner decides if mcrs keeps 32, or takes another value, for
   example 96 as Render Distance.
2. The load tickets of a server view distance above 32. SIM-TICK-005 clamps the view distance
   `V` to 2–32 for the load level. With MENU-DEV-008, the server of mcrs has a `V` up to 96.
   The owner decides which clamp the load level of mcrs uses. `block-simulation.md` then
   states it.
