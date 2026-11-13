# Pause and options menus of the client: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`.

This document states the pause screen and the options screens of the vanilla client. It also
states the mcrs subset of each screen: the widgets that mcrs shows, and their positions. It
states the six options that mcrs applies: FOV, FOV Effects, Brightness, Smooth Lighting, Clouds
and GUI Scale. It uses the terms of `client-widget.md` and `client-list.md` and cites their
rules for each widget, layout and background. The document does not prescribe an architecture.

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
- **Options screen**: the screen with the title "Options" (section 2).
- **Option sub-screen**: a screen that the options screen opens and that shows an option list
  (section 3).
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
| `MENU-VAL` | the six options: widget, values, caption, tooltip and effect |
| `MENU-SCALE` | the GUI scale: its computation, its values and the Ctrl wheel |
| `MENU-SAVE` | the moments when the client writes the options file |
| `MENU-PROP` | the properties of each screen that `client-widget.md` leaves open |
| `MENU-EXP` | experiments |
| `MENU-DEV` | intentional deviations |

### 0.5 Scope

In scope:

- the pause screen with its menu: the key that opens it, the close, the title and the buttons;
- the options screen;
- the option sub-screens "Video Settings" and "Accessibility Settings", for the order of their
  rows;
- the six options of mcrs;
- the moment when the client writes the options file;
- the points that `client-widget.md`, section 16, items 5 and 6, hand to this document.

Out of scope:

- whether the game pauses while the pause screen is open;
- the pause screen with no menu (F3 + Escape), and the pause on a loss of the window focus;
- the title screen;
- the form of the options file;
- the option "Render Distance";
- the other vanilla options. This document names them only where an order needs them;
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
  3. The grid holds only "Video Settings..." and "Accessibility Settings...", in that order.
     WID-LAY-005 puts them in row 0, at the columns 0 and 1.
  4. The footer holds "Done", 200 × 20. Escape and "Done" act as MENU-OPT-001 states.
- **Postconditions:** at W = 427 and H = 240, with a title not wider than 150:
  - The column is 150 × 37 at (138, 12). The FOV slider is at (138, 29).
  - The grid is 316 × 24. The top of the content is `min(61 + 30, 240 − 33 − 24)` = 91. The
    grid is at (55, 91).
  - "Video Settings..." is at (59, 91). "Accessibility Settings..." is at (217, 91).
  - "Done" is at (113, 214).

Provenance: `design` for the set of widgets, as MENU-PAUSE-006 states. `source_suggested` for
the positions. Kind: layout. Confidence: medium. Experiment: MENU-EXP-02.

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
| `options.video.display.header` "Display" | Exclusive Fullscreen Mode | Max Framerate, VSync, Reduce FPS When, **GUI Scale**, Fullscreen, Exclusive Fullscreen, **Brightness**, Graphics API; then "Prefer DXGI Present" on Windows, or "Menu/Dock Visibility" on macOS |
| `options.video.quality.header` "Quality & Performance" | Preset | Biome Blend, Render Distance, Chunk Builder, Simulation Distance, **Smooth Lighting**, **Clouds**, Particles, Mipmap Levels, Entity Shadows, Entity Distance, Menu Background Blur, Cloud Distance, See-Through Leaves, Improved Transparency, Texture Filtering, Anisotropic Filtering, Weather Effect Radius |
| `options.video.preferences.header` "Preferences" | none | Autosave Indicator, Show Vignette, Attack Indicator, Chunk Fade Time |

- **Postconditions:** "GUI Scale" is at the index 1 of the second row of "Display".
  "Brightness" is at the index 0 of the fourth row. "Smooth Lighting" and "Clouds" share the
  third row of "Quality & Performance".
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
- **Postconditions:** the list holds these four rows. At W = 427 and H = 240 with the scroll
  offset 0:

| Row | Content | Row top | Height | Widgets |
|-----|---------|---------|--------|---------|
| 0 | label "Display" | 35 | 18 | label box top 37 |
| 1 | "GUI Scale", "Brightness" | 53 | 25 | (58, 55) and (218, 55) |
| 2 | label "Quality & Performance" | 78 | 36 | label box top 98 |
| 3 | "Smooth Lighting", "Clouds" | 114 | 25 | (58, 116) and (218, 116) |

  The content height is 108. The list is 174 high, so it does not scroll.

Provenance: `design` for the set of rows, as MENU-PAUSE-006 states. `source_suggested` for the
positions. Kind: layout. Confidence: medium. Experiment: MENU-EXP-03.

### MENU-ACC-001: Rows of the vanilla "Accessibility Settings" screen

- **Behavior:**
  1. The title is `options.accessibility.title`, "Accessibility Settings". The screen has the
     form of MENU-SUB-001, but with another footer (step 4).
  2. The first row holds "Narrator", then the button "Controls...".
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
  2. The list holds one row: "FOV Effects" alone, at the index 0.
  3. The row of the footer (WID-LAY-006) holds only "Done". "Done" keeps its vanilla size,
     150 × 20. The footer frame places the row with its one child (WID-LAY-011). Escape and
     "Done" act as MENU-SUB-002 states.
- **Postconditions:** at W = 427 and H = 240, "FOV Effects" is at (58, 37) and "Done" is at
  (138, 214).

Provenance: `design` for the set of widgets, as MENU-PAUSE-006 states. `source_suggested` for
the positions. Kind: layout. Confidence: medium. Experiment: MENU-EXP-04.

---

## 4. The six options

The six options of mcrs are on these screens:

| Option | Screen | Rule |
|--------|--------|------|
| FOV | the options screen, in the header | MENU-VAL-001 |
| FOV Effects | "Accessibility Settings" | MENU-VAL-002 |
| Brightness | "Video Settings", section "Display" | MENU-VAL-003 |
| Smooth Lighting | "Video Settings", section "Quality & Performance" | MENU-VAL-004 |
| Clouds | "Video Settings", section "Quality & Performance" | MENU-VAL-005 |
| GUI Scale | "Video Settings", section "Display" | MENU-VAL-006 |

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
     Lighting, Clouds and GUI Scale are such buttons;
  2. each time that the options screen or an option sub-screen is removed: it closes, or
     another screen replaces it. For example, the options screen is removed when a sub-screen
     opens.
- **Postconditions:**
  - A change of a slider writes nothing at once. The write of step 2 includes it.
  - A change of the GUI scale by the Ctrl wheel (MENU-SCALE-003) writes nothing at once. The
    write of step 2 includes it.
  - The close of the pause screen writes nothing.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: MENU-EXP-08.

---

## 7. Screen properties

### MENU-PROP-001: Properties of the screens of this document

- **Behavior:** these are the properties that `client-widget.md`, section 16, items 5 and 6,
  leave open:

| Screen | In-game screen | Own background | Separators | Re-arrange (WID-SCR-003) | Tab order group | Sliders that wait |
|--------|----------------|----------------|------------|--------------------------|-----------------|-------------------|
| Pause screen | no | no | none | builds its widgets again | 0 | none |
| Options screen | no | no | none | keeps its widgets | 0 | none |
| "Video Settings" | no | no | of the list (LIST-BG-002) | keeps its widgets | 0 | vanilla: Biome Blend, Render Distance, Simulation Distance; mcrs: none |
| "Accessibility Settings" | no | no | of the list (LIST-BG-002) | keeps its widgets | 0 | none |

- **Postconditions:**
  - Each screen of this document draws the background of WID-SCR-004, step 2. In a world, that
    is the blur of WID-SCR-005 and then the texture `inworld_menu_background.png`.
  - The pause screen and the options screen draw no separators of WID-SCR-006.
  - No screen of this document names an initial widget (WID-FOC-002, step 1).
  - None of the six options waits 600 ms. Each one applies at once.

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

1. Open "Accessibility Settings...". Record the row of "FOV Effects" and the footer bounds.
2. Press "Done". Record the screen that shows.

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

---

## 10. Unresolved

No point is open.
