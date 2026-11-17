# Title screen of the client: behavioural specification

| Field | Value |
|-------|-------|
| Packets | LOGIN, clientbound: `login_disconnect` 0, `key` 1, `login_finished` 2, `custom_query` 4. CONFIGURATION, clientbound: `disconnect` 2. PLAY, clientbound: `disconnect` 32, `game_event` 39, `login` 50 |

Source of the packet ids: `assets/mcrs/reports/packets.json`.

This document states the title screen of the vanilla client, its panorama and its fade-in. It
also states the screens that the client shows between the title screen and a world, in both
directions. For each screen it states the mcrs subset: the title screen, the entry into a world
from the world screens, and the return to the title screen. It uses the terms of
`client-widget.md` and `client-menu.md` and cites their rules. The screens "Select World" and
"Create New World" are in `client-world-screens.md`. The document does not prescribe
an architecture.

## 0. Conventions

### 0.1 Terms

- The terms of `client-widget.md`, section 0.1, apply with their words: **GUI unit**, **W**,
  **H**, **in a world**, **screen**, **parent screen**, **in-game screen**, **widget**,
  **bounds**, **visible**, **active**, **hovered**, **focused**, **activation**, **activation
  key**, **focus navigation**, **tooltip of a widget**, **last input device**, **tab order
  group**, **caption**, **alpha**, **text widget**, **layout element**, **layout**, **cell**,
  **padding**, **alignment** and **gap**.
- The terms of `client-menu.md`, section 0.1, apply with their words: **option**, **options
  file**, **stop of the client**, **options screen**, **local world**, **window pixels**, **GUI
  scale** and **mcrs subset**.
- **Title screen**: the screen of section 1. It is the screen that the client shows with no
  world and no other screen.
- **Panorama**: the moving picture of the world that the screens with no world show behind
  their widgets (section 3).
- **Spin** (`a`): the angle of the panorama about the vertical axis, in degrees.
- **Message screen**: a screen that shows one line of text in a box, and no widget
  (TITLE-LOAD-001).
- **Level loading screen**: the screen that shows "Loading terrain..." while the client waits
  for the world around the player (TITLE-LOAD-002).
- **Connect screen**: the screen that shows the progress of a connection to a server
  (TITLE-LOAD-005).
- **Disconnected screen**: the screen that shows why a connection ended (section 7).
- **Unrequested disconnect**: the end of the connection of the client to a world, when the
  player did not ask for it. For example, the server kicks the player, the server stops, or the
  connection times out.
- **Quick play**: a start of the vanilla client that names a world or a server, so that the
  client enters it with no press on the title screen. mcrs has no quick play.
- **Start screens**: the screens that the vanilla client shows after its start and before its
  first title screen: the loading overlay, the accessibility onboarding and the warning screens.
  They are out of scope.
- **mcrs notice**: the two lines of text that mcrs shows in place of the copyright text
  (TITLE-SCR-007).

### 0.2 Provenance, confidence and observer

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". The language keys and their English texts are in
`assets/minecraft/lang/en_us.json` of the client jar. The textures are in
`assets/minecraft/textures/` of the client jar, or in the asset index of the version where a
rule says so. The choice of a key or a texture for a widget comes from the reference. No
experiment ran.

A rule is `black_box_confirmed` for an English text, a texture size or a resource location only
where the pinned artefact itself is the evidence. The rule names that artefact.

The observer of each rule is a player in normal play who sees the screen, unless the rule names
another observer.

### 0.3 Arithmetic

The arithmetic of `client-widget.md`, section 0.3, applies: `a / b`, `a % b`, `trunc`, `floor`,
`round`, `clamp`, `lerp` and the even split. `ceil(v)` rounds the real `v` up. `wrap(v)` adds or
subtracts a multiple of 360 to the angle `v`, so that the result is in [−180, 180).

`tw` is the width of a text in GUI units, as `client-font.md` states. Each example uses W = 427
and H = 240, with the window of 854 × 480 pixels and the GUI scale 2. A centred text with the
centre `cx` starts at `x = cx − tw / 2`.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `TITLE-SCR` | the title screen: widgets, logo, texts, draw order and keys |
| `TITLE-FADE` | the fade-in of the title screen |
| `TITLE-PAN` | the panorama, and the screens that draw it |
| `TITLE-START` | the first screen after the start of the client |
| `TITLE-LOAD` | the screens from a press of Singleplayer or a join to the first frame of the world |
| `TITLE-LEAVE` | the screens from the last button of the pause screen to the title screen |
| `TITLE-DISC` | an unrequested disconnect and the disconnected screen |
| `TITLE-QUIT` | the button "Quit Game" |
| `TITLE-PROP` | the properties of each screen that `client-widget.md` leaves open |
| `TITLE-DEV` | intentional deviations |

### 0.5 Scope

In scope:

- the title screen: its widgets, the logo, the version text, the copyright text, the mcrs
  notice, the splash text as a widget, the draw order and the keys;
- the fade-in of the title screen;
- the panorama: its textures, its projection, its spin, and the screens that draw it;
- the message screens "Reading world data...", "Preparing Resources...", "Preparing for world
  creation..." and "Saving world";
- the level loading screen;
- the connect screen and the disconnected screen, for their texts and their order. The mcrs
  subset has only the disconnected screen of TITLE-DISC-003;
- the first screen after the start of the client;
- "Quit Game".

Out of scope:

- the loading overlay before the first screen;
- the start screens: the accessibility onboarding, the safety and the warning screens;
- the demo and the modified variants of the client, except the version text;
- Realms and its notifications, and the friends list;
- the content of the splash texts;
- the screen "Credits & Attribution";
- the screens "Select World" and "Create New World" (`client-world-screens.md`);
- quick play, and the arguments of a start of the client;
- the server list, "Add Server", "Edit Server" and "Direct Connection", except where a rule
  names the screen that a button opens;
- music and sound, the click sound, the narrator and the window title;
- the check for a draft player report;
- the work of the local server while it starts and stops, except what a player sees.

### 0.6 Constants

| Value | Data | Rule |
|-------|------|------|
| Title screen: top of the first button row `y0` | `H / 4 + 48` | TITLE-SCR-001 |
| Title screen: full button, half button, icon button | 200 × 20, 98 × 20, 20 × 20 | TITLE-SCR-001 |
| Title screen: row spacing, gap of the icon row | 24, 4 | TITLE-SCR-001 |
| Logo: position, size, texture region | (W / 2 − 128, 30), 256 × 44, v from 0 to 44 / 64 | TITLE-SCR-002 |
| Edition: position, size, texture region | (W / 2 − 64, 67), 128 × 14, v from 0 to 14 / 16 | TITLE-SCR-002 |
| Logo variant chance | 0.0001 for each title screen | TITLE-SCR-002 |
| Version text, copyright text: margin | 2 from the side, 10 from the bottom | TITLE-SCR-003 |
| mcrs notice: margin, line height, gap to the version text, smallest W | 2 from the right, 9, 4, 320 | TITLE-SCR-007 |
| Splash: centre, angle, colour | (W / 2 + 123, 69), −20°, `FFFF00` | TITLE-SCR-004 |
| Fade: length, invisible part | 2000 ms, the first 1000 ms | TITLE-FADE-001 |
| Panorama: vertical field of view, pitch | 85°, 10° down | TITLE-PAN-002 |
| Panorama: spin step, frame gap limit | 0.1° for each 50 ms at speed 1, 7 × 50 ms | TITLE-PAN-003 |
| Panorama Scroll Speed: range, default | [0, 1], 1.0 | TITLE-PAN-003 |
| Message screen: padding, box height | 12, 33 | TITLE-LOAD-001 |
| Level loading screen: text top with no map | `H / 2 − 50` | TITLE-LOAD-002 |
| Level loading screen: progress bar, smoothing | 200 × 2, 0.2 for each tick | TITLE-LOAD-002 |
| Level loading screen: timeout, close delay of a new world | 30 s, 500 ms | TITLE-LOAD-003 |
| Disconnected screen: text padding, button padding, reason width | 10, 2, W − 50 | TITLE-DISC-002 |
| Missing texture of a panorama face | 16 × 16, black and magenta | TITLE-PAN-001 |

---

## 1. Title screen

### TITLE-SCR-001: Widgets of the vanilla title screen

- **Inputs:** W, H, and whether multiplayer is allowed for the account.
- **Behavior:**
  1. Let `y0 = H / 4 + 48` and `cx = W / 2`. The screen adds these widgets in this order. The
     order is also the tab order, in the group 0 (WID-FOC-004).

| # | Widget | Caption or sprite | Bounds | Action |
|---|--------|-------------------|--------|--------|
| 1 | button | `menu.singleplayer` "Singleplayer" | (cx − 100, y0, 200, 20) | opens "Select World". The title screen is its parent screen |
| 2 | button | `menu.multiplayer` "Multiplayer" | (cx − 100, y0 + 24, 200, 20) | opens the server list, or the safety screen before the first use |
| 3 | button | `menu.online` "Minecraft Realms" | (cx − 100, y0 + 48, 200, 20) | opens the Realms screen |
| 4 | icon button | sprite `friends/friends`, tooltip `gui.friends.open` "Friends" | (cx − 34, y0 + 72, 20, 20) | opens the friends list |
| 5 | icon button | icon `icon/language`, caption `options.language` "Language...", tooltip `options.language.tooltip` "Change Language" | (cx − 10, y0 + 72, 20, 20) | opens the language screen. The title screen is its parent screen |
| 6 | icon button | icon `icon/accessibility`, caption `options.accessibility` "Accessibility Settings...", tooltip `options.accessibility.tooltip` "Accessibility Settings" | (cx + 14, y0 + 72, 20, 20) | opens "Accessibility Settings". The title screen is its parent screen |
| 7 | button | `menu.options` "Options..." | (cx − 100, y0 + 96, 98, 20) | opens the options screen. The title screen is its parent screen |
| 8 | button | `menu.quit` "Quit Game" | (cx + 2, y0 + 96, 98, 20) | TITLE-QUIT-001 |
| 9 | text button | `title.credits` "Copyright Mojang AB. Do not distribute!" | (W − tw − 2, H − 10, tw, 10) | opens "Credits & Attribution". The title screen is its parent screen |

  2. The icon row holds `n` = 3 icon buttons with the gap 4. Button `i`, from 1 to `n`, is at
     `x = cx − (20 · n + 4 · (n − 1)) / 2 + 24 · (i − 1)`.
  3. The icons 5 and 6 are 15 × 15. Each draws as WID-ICON-001 states. The caption of an icon
     button does not show.
  4. When multiplayer is not allowed, "Multiplayer" and "Minecraft Realms" are not active.
     Their tooltip is the reason: `title.multiplayer.disabled`, or the key of a suspended
     account or of a name that must change.
  5. The text button 9 draws its text in white, with the alpha `ceil(α · 255)` of TITLE-FADE-001
     and a shadow. The text has an underline while the button is hovered or focused.
- **Postconditions:** at W = 427 and H = 240, `y0` = 108:
  - "Singleplayer" is at (113, 108). "Multiplayer" is at (113, 132). "Minecraft Realms" is at
    (113, 156).
  - The icon buttons are at (179, 180), (203, 180) and (227, 180).
  - "Options..." is at (113, 204). "Quit Game" is at (215, 204).
  - The copyright text is 196 wide. Its button is at (229, 230, 196, 10).
- **Edge cases:**
  - A demo client and a client in a development setup show other buttons. They are out of
    scope.
  - "Friends" is not active in a demo or offline. It shows a badge for a friend request. Both
    are out of scope.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language
file, and for the icon size by the sprite files. Kind: layout. Confidence: medium.

### TITLE-SCR-002: Logo and edition

- **Inputs:** W, and the alpha `α_logo` of TITLE-FADE-001.
- **Behavior:**
  1. The screen draws `minecraft:textures/gui/title/minecraft.png` at (W / 2 − 128, 30), with
     the size 256 × 44. It takes the whole width of the texture and the top 44 / 64 of its
     height.
  2. Each title screen picks the logo one time, when it is made. With the chance 0.0001, it uses
     `minecraft:textures/gui/title/minceraft.png` in place of step 1, with the same region.
  3. The screen draws `minecraft:textures/gui/title/edition.png` at (W / 2 − 64, 67), with the
     size 128 × 14. It takes the whole width of the texture and the top 14 / 16 of its height.
  4. The logo and the edition draw in white with the alpha `α_logo`.
- **Postconditions:** at W = 427, the logo is at (85, 30) and the edition is at (149, 67). The
  bottom 7 rows of the logo and the top of the edition overlap.

Provenance: `source_suggested`; `black_box_confirmed` for the texture sizes by the jar:
`minecraft.png` and `minceraft.png` are 1024 × 256, `edition.png` is 512 × 64. Kind: layout.
Confidence: medium.

### TITLE-SCR-003: Version text

- **Inputs:** H, the version name, and the alpha `α` of TITLE-FADE-001.
- **Behavior:**
  1. The text is "Minecraft " and then the version name of `assets/minecraft/version.json`.
  2. A demo client adds " Demo". A client whose brand is not the vanilla brand adds
     `menu.modded`, " (Modded)".
  3. The text draws at (2, H − 10) in white with the alpha `ceil(α · 255)` and a shadow.
  4. The text is not a widget. A click on it does nothing.
- **Postconditions:** for example, the version name "26.4 Snapshot 7" gives the text "Minecraft
  26.4 Snapshot 7", 131 wide, at (2, 230).

Provenance: `source_suggested`; `black_box_confirmed` for " (Modded)" by the language file.
Kind: layout. Confidence: medium.

### TITLE-SCR-004: Splash text

- **Inputs:** the option "Hide Splash Texts" (`options.hideSplashTexts`), the splash text of the
  screen, the time `t` in milliseconds, and the alpha `α` of TITLE-FADE-001.
- **Behavior:**
  1. When "Hide Splash Texts" is ON, no splash shows.
  2. Else the screen picks one splash text when it is made. A re-arrange keeps it. The choice of
     the text is out of scope.
  3. The text has its centre at (W / 2 + 123, 69). It turns by −20°, so that its right end goes
     up.
  4. Its scale is `(1.8 − |sin(2π · (t % 1000) / 1000)| · 0.1) · 100 / (tw + 32)`. Thus it
     pulses one time each 500 ms.
  5. It draws in the colour `FFFF00` with the alpha `α` and a shadow. Its top left is at
     (−tw / 2, −8) about the centre, before the turn and the scale.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### TITLE-SCR-005: Draw order, keys and re-arrange

- **Behavior:**
  1. The title screen draws, in order: the panorama (TITLE-PAN-002) with its overlay, the
     widgets, the logo and the edition, the splash, then the version text.
  2. The title screen draws no blur (WID-SCR-005) and no menu background texture.
  3. Escape does not close the title screen. It goes to the focused widget (WID-SCR-002,
     step 1).
  4. The title screen names no initial widget. WID-FOC-002 applies.
  5. After a resize, or when a screen of the title screen closes, the title screen builds its
     widgets again (WID-SCR-003, the second kind). It keeps the splash and the logo of
     TITLE-SCR-002, step 2.
  6. With no world and no parent screen, a screen that closes opens a new title screen
     (WID-SCR-002, step 4). That title screen has no fade (TITLE-FADE-001).

Provenance: `source_suggested`. Kind: layout for the order, semantic for the keys. Confidence:
medium.

### TITLE-SCR-006: Title screen (mcrs subset)

- **Inputs:** W and H.
- **Behavior:**
  1. The screen adds only these widgets of TITLE-SCR-001, in this order: "Singleplayer", the
     icon button "Accessibility Settings", "Options..." and "Quit Game". This is the tab order.
  2. "Singleplayer" is at (W / 2 − 100, y0, 200, 20). It opens the screen "Select World" of
     `client-world-screens.md`. The title screen is its parent screen.
  3. The icon row is at `y0 + 24`. It holds the icon button "Accessibility Settings" alone, so
     `n` = 1 in TITLE-SCR-001, step 2. It opens the screen of `client-menu.md`, MENU-ACC-002.
     The title screen is its parent screen.
  4. "Options..." is at (W / 2 − 100, y0 + 48, 98, 20). It opens the options screen of
     `client-menu.md`, MENU-OPT-003. The title screen is its parent screen.
  5. "Quit Game" is at (W / 2 + 2, y0 + 48, 98, 20). It acts as TITLE-QUIT-001 states.
  6. The screen draws the logo and the edition of TITLE-SCR-002 and the vanilla version text of
     TITLE-SCR-003, with no suffix.
  7. "Multiplayer", "Minecraft Realms", "Friends", "Language..." and the splash text do not
     show. The copyright text does not show. The mcrs notice of TITLE-SCR-007 shows in its
     place.
  8. The draw order, the keys and the re-arrange are those of TITLE-SCR-005.
  9. The client picks the logo of TITLE-SCR-002, step 2, one time for each start of the
     client, not for each title screen (TITLE-DEV-008).
- **Postconditions:** at W = 427 and H = 240:
  - "Singleplayer" is at (113, 108).
  - "Accessibility Settings" is at (203, 132). Its icon is at (206, 135).
  - "Options..." is at (113, 156). "Quit Game" is at (215, 156).
  - The version text is at (2, 230).
- **Edge cases:** Escape on "Select World", on "Accessibility Settings" or on the options screen
  opens the title screen again (WID-SCR-002, step 2).

Provenance: `design` for the set of widgets: "Multiplayer", Realms, the splash, "Language..."
and the copyright text do not show, and the version text is the vanilla text. "Accessibility Settings" is in the set because
mcrs has the screen of MENU-ACC-002 (`client-menu.md`, MENU-DEV-002). "Friends" is not in the
set because mcrs has no friends list. `design` for the rows of step 3 to step 5: the rows keep
the spacing 24 with no empty row. `source_suggested` for the sizes and the x positions. Kind:
layout. Confidence: medium.

### TITLE-SCR-007: mcrs notice (mcrs subset)

- **Inputs:** W, H, the width `tv` of the version text (TITLE-SCR-003), and the alpha `α` of
  TITLE-FADE-001.
- **Behavior:**
  1. The notice has two parts, in this order from top to bottom:
     - "mcrs is licensed under MIT OR Apache-2.0.";
     - "Not an official Minecraft product. Not approved by or associated with Mojang or
       Microsoft."
  2. The texts are plain English. They are not language keys, and no language changes them.
  3. Each part starts on a new line. A part breaks into lines only at a space. "MIT OR
     Apache-2.0." stays on one line.
  4. No line is wider than `L = 320 − 2 − 4 − (2 + tv)`. 320 is the smallest W
     (`client-menu.md`, MENU-SCALE-001), and 4 is the gap to the version text. Each line holds
     as many words as `L` allows.
  5. The lines do not change with W. Line `i` of `n`, from 0 at the top, has its top at
     `H − 10 − 9 · (n − 1 − i)`. Thus the last line has its top at `H − 10`, as the copyright
     text.
  6. Each line is right aligned: it starts at `W − 2 − wi`, with `wi` its width.
  7. The lines draw in white with the alpha `ceil(α · 255)` and a shadow, where TITLE-SCR-005,
     step 1, draws the widgets.
  8. The notice is not a widget. A click on it does nothing, and Tab does not focus it.
- **Postconditions:** for the version text of the example of TITLE-SCR-003, `tv` = 131 and
  `L` = 181. The notice has five
  lines. At H = 240:

| Line | Text | Width | Bounds at W = 427 | Bounds at W = 320 |
|------|------|-------|-------------------|-------------------|
| 0 | "mcrs is licensed under" | 115 | (310, 194, 115, 9) | (203, 194, 115, 9) |
| 1 | "MIT OR Apache-2.0." | 94 | (331, 203, 94, 9) | (224, 203, 94, 9) |
| 2 | "Not an official Minecraft product." | 168 | (257, 212, 168, 9) | (150, 212, 168, 9) |
| 3 | "Not approved by or associated with" | 180 | (245, 221, 180, 9) | (138, 221, 180, 9) |
| 4 | "Mojang or Microsoft." | 105 | (320, 230, 105, 9) | (213, 230, 105, 9) |

  - At W = 320, each line starts at x ≥ 138. The version text ends at x = 133, so no line
    overlaps it.
  - The top line is at y = 194. The lowest button of TITLE-SCR-006 ends at y = 176, so the
    notice overlaps no widget.
- **Edge cases:** at the GUI scale 1, a window narrower than 320 window pixels gives W < 320.
  The lines can then overlap the version text, as the vanilla copyright text does.

Provenance: `design` for the texts, their order, the right alignment, the bottom margin and the
condition of step 4. `design` for the gap 4 and the break of step 3. The widths come from
`client-font.md`. Kind: layout. Confidence: high. Deviation: TITLE-DEV-001.

---

## 2. Fade-in

### TITLE-FADE-001: Fade-in of the vanilla title screen

- **Inputs:** how the title screen opened, and the time `e` in milliseconds since the first
  frame that the title screen drew.
- **Behavior:**
  1. A title screen fades in only when it is the first title screen after the start screens,
     and the start is not a quick play. Each other title screen has `α` = 1 from its first
     frame.
  2. During the fade, `α = clamp((e − 1000) / 1000, 0, 1)`. The fade ends at the first frame
     with `e` > 2000.
  3. The widgets, the copyright text, the splash and the version text draw with the alpha `α`.
     The tooltip of a widget does not fade. It draws at full strength also while `α` < 1.
  4. The logo and the edition draw with `α_logo = α`. After the accessibility onboarding screen,
     `α_logo` = 1. That screen is out of scope.
  5. The panorama does not fade. It shows at full strength from the first frame.
  6. The widgets take input during the fade, also while `α` = 0.
- **Postconditions:** at 60 frames each second, the widgets are invisible for 1 s. They then
  become visible during 1 s.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### TITLE-FADE-002: Fade-in (mcrs subset)

- **Behavior:**
  1. mcrs has no start screens and no quick play. The first title screen after each start of
     the client fades in, as TITLE-FADE-001 states (TITLE-START-002).
  2. `α_logo = α`, because mcrs has no accessibility onboarding screen.
  3. A title screen after a world, after a disconnected screen, or after a close of TITLE-SCR-005,
     step 6, has no fade.
  4. The widgets take input during the fade, as TITLE-FADE-001, step 6, states.
  5. Each draw in front of the panorama takes the alpha `α`: the widgets, the mcrs notice, the
     version text, the logo and the edition. The tooltip of a widget also takes `α`
     (TITLE-DEV-009).
  6. The panorama and its overlay do not fade, as TITLE-FADE-001, step 5, states.

Provenance: `source_suggested`; `design` for steps 2 and 5. Steps 4 and 6 are the rules of the
game: a player who knows the screen can click before the widgets show.
Kind: layout. Confidence: medium.

---

## 3. Panorama

### TITLE-PAN-001: Textures of the panorama

- **Observer:** a player, and an author of a resource pack.
- **Inputs:** the resource packs.
- **Behavior:**
  1. The six faces are `minecraft:textures/gui/title/background/panorama_0.png` to
     `panorama_5.png`. The overlay is
     `minecraft:textures/gui/title/background/panorama_overlay.png`.
  2. The faces draw with linear filtering.
  3. When the faces do not all have the same size, each face shows the missing texture: a
     checker of 16 × 16 pixels in black and magenta.
  4. The overlay stretches over (0, 0, W, H) in white, after the faces.
- **Postconditions:**
  - In the client jar, each face and the overlay is a placeholder of 1 × 1 pixel. The asset
    index of the version holds the real files, and they replace the files of the jar.
  - In the asset index, each face is 1024 × 1024 RGB. The overlay is 1 × 1 RGBA `00FFFFFF`,
    which is fully transparent. Thus the vanilla overlay changes no pixel.

Provenance: `source_suggested` for steps 2 to 4; `black_box_confirmed` for the files and their
sizes by the client jar and by the asset index of the version. Kind: layout. Confidence:
medium for steps 2 to 4, high for the files.

### TITLE-PAN-002: Projection of the panorama

- **Inputs:** the window pixels `pw` and `ph`, and the spin `a`.
- **Behavior:**
  1. The client draws the six faces as a cube around a camera at the centre of the cube.
  2. The projection is a perspective with the vertical field of view 85° and the aspect
     `pw / ph`. This quotient is the real quotient, not the integer quotient of section 0.3. The
     GUI scale does not change the picture.
  3. The camera looks 10° down from the horizontal. Its yaw is `−a`: an increase of `a` turns
     the view from `panorama_0` toward `panorama_1`.
  4. At `a` = 0 the faces are:

| Face | Direction | Edge that meets `panorama_0` |
|------|-----------|------------------------------|
| `panorama_0` | ahead | none |
| `panorama_1` | right | its left edge meets the right edge of `panorama_0` |
| `panorama_2` | behind | its left edge meets the right edge of `panorama_1` |
| `panorama_3` | left | its right edge meets the left edge of `panorama_0` |
| `panorama_4` | up | its bottom edge meets the top edge of `panorama_0` |
| `panorama_5` | down | its top edge meets the bottom edge of `panorama_0` |

  5. The four side faces are upright and not mirrored, as a player at the centre of the cube
     sees them.
- **Postconditions:** while `a` grows, the picture moves from the right to the left of the
  window.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### TITLE-PAN-003: Spin of the panorama

- **Inputs:** the real time since the previous frame `dt` in milliseconds, and the option
  "Panorama Scroll Speed" `k`.
- **Behavior:**
  1. `a` is 0 at the start of the client.
  2. In each frame that draws the panorama, let `d = dt / 50`. When `d` > 7, `d` = 0.5.
  3. Then `a = wrap(a + 0.1 · d · k)`.
  4. A frame that does not draw the panorama does not change `a`. Thus the panorama keeps its
     angle while a world shows, and it goes on from that angle after the world.
  5. All the screens share one `a`.
- **Postconditions:** at `k` = 1, the panorama turns 2° each second. One full turn takes 180 s.
- **Edge cases:** the accessibility onboarding screen holds the spin. The opening of a title
  screen ends that hold. Both are out of scope.

The option "Panorama Scroll Speed" is `options.accessibility.panorama_speed`. Its range is
[0, 1] and its default is 1.0. The caption shows the value as a percent (`options.percent_value`).
It is on the vanilla "Accessibility Settings" screen.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language
file. Kind: layout. Confidence: medium.

### TITLE-PAN-004: Screens that draw the panorama

- **Inputs:** the open screen, and whether the client has a world.
- **Behavior:**
  1. With no world, each screen that is not an in-game screen draws the panorama, then the blur
     of WID-SCR-005, then the menu background texture (WID-SCR-004, step 2).
  2. The title screen draws only the panorama and its overlay (TITLE-SCR-005, step 2).
  3. The message screens of TITLE-LOAD-001 draw the panorama, the blur and the menu background
     texture, also when the client has a world.
  4. The level loading screen of TITLE-LOAD-002 draws the panorama, the blur and the menu
     background texture, also after the world exists. A level loading screen for a portal is
     out of scope.
  5. The connect screen and the disconnected screen have no world, so step 1 applies.
- **Postconditions:** `menu_background.png` and `inworld_menu_background.png` are the same
  image (WID-SCR-004). Thus a screen with no world darkens the blurred panorama as a screen in a
  world darkens the blurred world.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### TITLE-PAN-005: Panorama (mcrs subset)

- **Behavior:**
  1. The panorama keeps TITLE-PAN-001 to TITLE-PAN-004.
  2. mcrs has no option "Panorama Scroll Speed". `k` is always 1.
  3. The faces and the overlay come from the asset index of the pinned version, laid over the
     jar, as for the fonts. The 1 × 1 placeholders of the jar do not show.
  4. When the faces do not all have the same size, no panorama draws. The background of the
     title screen stays black, and the client writes a warning to its log (TITLE-DEV-007).

Provenance: `design` for step 2, as `client-menu.md`, MENU-ACC-002, states. `design` for step 4:
mcrs has no resource packs, so only a broken asset set has faces of different sizes.
`source_suggested` for the rest. Kind: layout. Confidence: medium.

---

## 4. Start of the client

### TITLE-START-001: First screen of the vanilla client

- **Inputs:** a start of the client, with or without a quick play.
- **Behavior:**
  1. With no quick play, the client shows the start screens, then the title screen with its
     fade (TITLE-FADE-001).
  2. A quick play enters its world or its server with no title screen first. Its details are out
     of scope.
- **Postconditions:** after a quick play, the title screen that shows later has no fade.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### TITLE-START-002: First screen of the mcrs client

- **Inputs:** a start of the client.
- **Behavior:**
  1. Each start of the client opens the title screen with its fade (TITLE-FADE-002).
  2. No start enters a world first. The player enters a world only from the screens of
     `client-world-screens.md`.

Provenance: `design`. Kind: semantic. Confidence: high.

---

## 5. From a press to the world

### TITLE-LOAD-001: Message screen

- **Inputs:** a text, W and H.
- **Behavior:**
  1. The box is `(tw + 24) × 33`. Its top left is at `(W / 2 − (tw + 24) / 2, H / 2 − 4)`.
  2. The box is black, from `x + 1`. Its outline is `FFA0A0A0`.
  3. The text draws in white at `(x + 12, y + 12)`.
  4. The screen has no widget. Escape does nothing.
  5. The background is that of TITLE-PAN-004, step 3.
- **Postconditions:** the texts are `selectWorld.data_read`, "Reading world data...", and
  `menu.savingLevel`, "Saving world". At W = 427 and H = 240, "Saving world" is 63 wide. Its box
  is 87 × 33 at (170, 116), and the text is at (182, 128).

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language
file. The box is 4 units below the centre, as in the game. Kind: layout. Confidence: medium.

### TITLE-LOAD-002: Level loading screen

- **Inputs:** W, H, whether the world is a local world, the progress `p_s` that the server
  reports, and the status of each chunk near the spawn of a local world.
- **Behavior:**
  1. The text is `multiplayer.downloadingTerrain`, "Loading terrain...". It is centred at
     W / 2, in white.
  2. For a local world, the screen draws a map of the chunk statuses. The map is a square of
     2 × 2 GUI units for each column, with no gap. It has the side `2r + 1` columns and its
     centre at the centre of the screen. Each square has the colour of its chunk status. `r` is
     6 plus the radius in columns that the generation of a full chunk needs around it.
  3. With the map, the top of the text is at `H / 2 − 2r − 27`. With no map, it is at
     `H / 2 − 50`.
  4. When the server reports a progress, a bar of 200 × 2 shows at `(W / 2 − 100, top + 12)`.
     Its background is `FF000000`. Its fill is `FF00FF00` and `round(p · 200)` wide.
  5. `p` starts at 0. Each client tick, `p = p + (p_s − p) · 0.2`.
  6. Escape does nothing. The background is that of TITLE-PAN-004, step 4.
- **Postconditions:** with no map at W = 427 and H = 240, the text is 84 wide at (171, 70).

The colours of the map, as RGB:

| Status | Colour | Status | Colour |
|--------|--------|--------|--------|
| empty | `545454` | features | `21C600` |
| structure_starts | `999999` | initialize_light | `CCCCCC` |
| structure_references | `5F6191` | light | `FFE0A0` |
| noise_biomes | `80B252` | spawn | `F25D60` |
| biomes | `226622` | full | `FFFFFF` |
| terrain | `303572` | | |

Provenance: `source_suggested`; `black_box_confirmed` for the English text by the language
file. Kind: layout. Confidence: medium.

### TITLE-LOAD-003: Close of the level loading screen

- **Observer:** a player who waits for the world.
- **Inputs:** the packets that the client receives, the chunks that the client has drawn, and
  the time.
- **Behavior:** the screen closes when all these conditions are true, in this order:
  1. The client has received the PLAY `login` packet.
  2. After it, the client has received `game_event` with the event 13 (level chunks load
     start).
  3. One of these conditions is true:
     - the client can draw the section of the camera;
     - the camera is above or below the build height;
     - the player is a spectator;
     - the player is dead;
     - 30 s have passed since the `login` packet.
  4. A delay has passed: 500 ms for a world that the player created in this session, else 0.
- **Postconditions:**
  - The screen closes and the world shows with no screen open (WID-SCR-001). The last input
    device becomes none.
  - The player never sees a frame of the world with a hole at the camera, unless the timeout of
    step 3 occurs.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. `dimension-change.md`,
DIM-CHG-007, uses the same event 13.

### TITLE-LOAD-004: Entry into a local world in the vanilla client

- **Inputs:** an activation of "Play Selected World" on "Select World", or of "Create New
  World" on "Create New World" (`client-world-screens.md`).
- **Behavior:**
  1. After "Play Selected World", the message screen "Reading world data..."
     (`selectWorld.data_read`) shows while the client reads the world. Then "Preparing
     Resources..." (`selectWorld.resource_load`) shows while the client loads its data packs.
  2. After "Create New World", the message screen "Preparing for world creation..."
     (`createWorld.preparing`) shows while the client makes the world folder.
  3. The level loading screen shows, with the map, while the local server prepares the spawn.
  4. The client connects to its local server. The level loading screen stays.
  5. The level loading screen closes as TITLE-LOAD-003 states. After "Create New World", the
     close delay is 500 ms.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language
file. Kind: semantic. Confidence: medium.

### TITLE-LOAD-005: Join of a server in the vanilla client

- **Inputs:** a join of a server, its parent screen, and the LOGIN packets.
- **Behavior:**
  1. The connect screen opens. Its status text is centred at (W / 2, H / 2 − 50). Its button
     `gui.cancel`, "Cancel", is at (W / 2 − 100, H / 4 + 132, 200, 20). Escape does nothing.
  2. The status starts as `connect.connecting`, "Connecting to the server...".
  3. At the LOGIN `key` packet it becomes `connect.authorizing`, "Logging in...". After the
     client sends its key, it becomes `connect.encrypting`, "Encrypting...".
  4. At a LOGIN `custom_query` it becomes `connect.negotiating`, "Negotiating...". At
     `login_finished` it becomes `connect.joining`, "Joining world...".
  5. The connect screen stays during the configuration.
  6. At the PLAY `login` packet, the level loading screen opens with no map. It closes as
     TITLE-LOAD-003 states.
  7. "Cancel" ends the connection and opens the parent screen.
  8. A failure before the PLAY `login` packet opens the disconnected screen. Its title is
     `connect.failed`, "Failed to connect to the server", or "Failed to Quick Play" for a quick
     play. Its parent screen is the parent of the connect screen.
- **Postconditions:** at W = 427 and H = 240, "Cancel" is at (113, 192). "Connecting to the
  server..." is 134 wide at (146, 70).

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language
file. Kind: semantic for the order, layout for the positions. Confidence: medium.

### TITLE-LOAD-006: Entry into a local world (mcrs subset)

- **Inputs:** an activation of "Play Selected World" or "Create New World"
  (`client-world-screens.md`).
- **Behavior:**
  1. After "Play Selected World", the message screen "Reading world data..." shows. After
     "Create New World", "Preparing for world creation..." shows. No message screen
     "Preparing Resources..." shows (TITLE-DEV-011).
  2. The client starts its local server on the world.
  3. When the client starts the connection to that server, the level loading screen opens. It
     has no map and no progress bar, so its text is at `H / 2 − 50`.
  4. The level loading screen closes as TITLE-LOAD-003 states, with one difference in step 3.
     The mcrs client does not test whether the player is dead (TITLE-DEV-010). Step 3 is true
     only when one of these conditions is true:
     - the client can draw the section of the camera;
     - the camera is above or below the build height;
     - the player is a spectator;
     - 30 s have passed since the `login` packet.
- **Postconditions:** the player sees the message screens, then "Loading terrain...", then the
  world. Each screen shows for at least one frame.
- **Edge cases:** when the local server cannot start, the client shows the disconnected screen
  of TITLE-DISC-003 with the reason of the failure and the title of TITLE-DISC-004
  (TITLE-DEV-013).

Provenance: `source_suggested`; `design` for step 1 (TITLE-DEV-011), step 3 (TITLE-DEV-003) and
the test of a dead player in step 4 (TITLE-DEV-010). Kind: semantic.
Confidence: medium.

---

## 6. From the pause screen to the title screen

### TITLE-LEAVE-001: Last button of the vanilla pause screen

- **Inputs:** an activation of the last button of the pause screen (`client-menu.md`,
  MENU-PAUSE-005), and whether the world is a local world.
- **Behavior:**
  1. The client closes its connection. No PLAY packet goes to the server for this.
  2. In a local world, the message screen "Saving world" shows. The client draws frames until
     the local server has stopped and saved the world.
  3. On a server, a screen with only the background shows for one frame.
  4. After a local world, a title screen opens with no fade.
  5. After a server, the server list opens. A title screen is its parent screen. After a Realms
     world, the Realms list opens.
- **Postconditions:** the client has no world. The panorama goes on from its angle before the
  world (TITLE-PAN-003, step 4).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### TITLE-LEAVE-002: Return to the title screen (mcrs subset)

- **Inputs:** an activation of the last button of the pause screen (`client-menu.md`,
  MENU-PAUSE-006, step 5).
- **Behavior:**
  1. The client does steps 1 and 2 of TITLE-LEAVE-001.
  2. Then a title screen opens with no fade, for both captions of the last button. On a server,
     no frame with only the background shows before it (TITLE-DEV-012).
- **Postconditions:**
  - In a local world, the world folder holds the world as the server saved it.
  - The options file is not written (`client-menu.md`, MENU-SAVE-002).

Provenance: `source_suggested` for step 1; `design` for step 2, because mcrs has no server
list (TITLE-DEV-004) and the frame between the two screens has no use for a player
(TITLE-DEV-012). Kind: semantic. Confidence: medium.

---

## 7. Unrequested disconnect

### TITLE-DISC-001: Unrequested disconnect in the vanilla client

- **Inputs:** an unrequested disconnect, its reason, and the kind of the world.
- **Behavior:**
  1. The reason is the text of the `disconnect` packet of the state of the connection, or a text
     of the client. Examples: `disconnect.timeout`, "Timed out"; `disconnect.endOfStream`, "End
     of stream".
  2. In a local world, the message screen "Saving world" shows first, as TITLE-LEAVE-001,
     step 2, states.
  3. Then the disconnected screen opens. Its title is `disconnect.lost`, "Connection Lost".
  4. Its one button is `gui.toMenu`, "Back to Server List". After a Realms world it is
     `gui.back`, "Back". When multiplayer is not allowed, it is `gui.toTitle`, "Back to Title
     Screen", and it opens a new title screen.
  5. The button opens the parent screen. The parent is the server list, with a title screen as
     its parent, when the session joined a server. Else it is a title screen.
  6. Escape does nothing.
- **Edge cases:** the buttons "Report to Server" and "Open Report Directory" can also show.
  They are out of scope.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts by the language
file. Kind: semantic. Confidence: medium.

### TITLE-DISC-002: Layout of the disconnected screen

- **Inputs:** W, H, the title, the reason and the button.
- **Behavior:**
  1. The screen is a column (WID-LAY-006) of the title, the reason and the button.
  2. The default cell has the padding 10 on each side and `ax` = 0.5.
  3. The title is a one-line text widget (WID-TXT-001). The reason is a multi-line text widget
     (WID-TXT-002) with the maximum width W − 50 and centred lines. The reason keeps the
     colours and the styles of its text component. A part with no colour is white.
  4. The cell of each button has the padding 2 on each side.
  5. The column has its centre at the centre of the screen (WID-LAY-003).
  6. The background is that of TITLE-PAN-004, step 5.
  7. The reason has no maximum number of lines. When the column is taller than H, it stays
     centred, the parts above y = 0 and below y = H do not show, and the screen does not scroll.
- **Postconditions:** at W = 427 and H = 240, with a one-line reason: the title is at y = 89,
  the reason is at y = 118, and a button of 200 × 20 is at (113, 139).
- **Edge cases:** at H = 240 the lower edge of the button leaves the screen from a reason of 19
  lines. Escape does not close the screen.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### TITLE-DISC-003: Unrequested disconnect (mcrs subset)

- **Behavior:**
  1. TITLE-DISC-001, steps 1 and 2, and TITLE-DISC-002 apply. The title is that of
     TITLE-DISC-004. The reason draws in white with no style (TITLE-DEV-014).
  2. The one button is `gui.toTitle`, "Back to Title Screen". It opens a title screen with no
     fade.
  3. The parent of the disconnected screen is a title screen, for a local world and for a
     server.
  4. The buttons "Report to Server" and "Open Report Directory" do not show.
- **Postconditions:** at W = 427 and H = 240, with a one-line reason, "Back to Title Screen" is
  at (113, 139).

Provenance: `design` for step 2 to step 4, because mcrs has no server list
(TITLE-DEV-005). `source_suggested` for the layout. Kind: semantic. Confidence: medium.

### TITLE-DISC-004: Title of the disconnected screen (mcrs subset)

- **Inputs:** the end that opens the disconnected screen.
- **Behavior:** the title of the disconnected screen is:
  1. `disconnect.lost`, "Connection Lost", when the connection ends after the PLAY `login`
     packet;
  2. `connect.failed`, "Failed to connect to the server", when a join of a server fails before
     the PLAY `login` packet, as TITLE-LOAD-005, step 8, states;
  3. `selectWorld.access_failure`, "Failed to access world", when a local world does not
     start. This includes a world that is locked, a world folder that the client cannot make,
     read or write, the world types that the client cannot read, and a local server that
     cannot start (`client-world-screens.md`, WORLD-CREATE-002, WORLD-PLAY-002 and
     WORLD-WRITE-002).
- **Postconditions:**
  - A player who presses "Play Selected World" on a world that another process holds sees
    "Failed to access world", not "Connection Lost".
  - A player who leaves a local world through a timeout of its connection sees "Connection
    Lost".

Provenance: `source_suggested` for steps 1 and 2. `design` for step 3: the vanilla client uses
this text for a world that it cannot open, and it stops with a crash report for the other
failures of a local start (TITLE-DEV-013). `black_box_confirmed` for the English texts by the
language file. Kind: semantic. Confidence: medium.

---

## 8. Quit Game

### TITLE-QUIT-001: Quit Game

- **Inputs:** an activation of "Quit Game" on the title screen.
- **Behavior:**
  1. The activation is a stop of the client (`client-menu.md`, section 0.1).
  2. The check for a draft player report before step 1 is out of scope.
- **Postconditions:** the title screen is not an options screen or an option sub-screen. Thus
  the stop writes no options file (`client-menu.md`, MENU-SAVE-002).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.
mcrs keeps this rule.

---

## 9. Screen properties

### TITLE-PROP-001: Properties of the screens of this document

- **Behavior:** these are the properties that `client-widget.md`, section 16, items 4 and 5,
  leave open:

| Screen | In-game screen | Background | Escape closes it | Re-arrange (WID-SCR-003) | Tab order group |
|--------|----------------|------------|------------------|--------------------------|-----------------|
| Title screen | no | panorama only (TITLE-SCR-005) | no | builds its widgets again | 0 |
| Message screen | no | TITLE-PAN-004, step 3 | no | builds its widgets again | none |
| Level loading screen | no | TITLE-PAN-004, step 4 | no | builds its widgets again | none |
| Connect screen | no | TITLE-PAN-004, step 1 | no | builds its widgets again | 0 |
| Disconnected screen | no | TITLE-PAN-004, step 1 | no | builds its widgets again | 0 |

- **Postconditions:** no screen of this document names an initial widget (WID-FOC-002, step 1).
  No screen of this document draws the separators of WID-SCR-006.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

---

## 11. Intentional deviations

### TITLE-DEV-001: The mcrs notice replaces the copyright text

- **Rule:** TITLE-SCR-006, step 7, and TITLE-SCR-007.
- **Reference:** the text button "Copyright Mojang AB. Do not distribute!" at the bottom right.
  It opens "Credits & Attribution" (TITLE-SCR-001, widget 9).
- **Reason:** mcrs is not the game of Mojang, so the copyright text of the game does not apply to
  it. mcrs has no screen "Credits & Attribution".
- **Visible to the client:** yes. Five lines of plain text show in place of one line. A click
  there opens no screen, and Tab does not reach it.
- **Bug reference:** `none`.
- **Test:** a differential test does not compare the bottom right of the title screen. A
  conformance test of mcrs compares the bounds of TITLE-SCR-007 at W = 427 and W = 320.

### TITLE-DEV-002: The title screen shows only the widgets of mcrs

- **Rule:** TITLE-SCR-006.
- **Reference:** the title screen shows the widgets of TITLE-SCR-001 and the splash.
- **Reason:** mcrs has no multiplayer screens, no Realms, no friends list and no language screen.
  The splash texts are not in the subset.
- **Visible to the client:** yes. The screen has four widgets, and the rows move up.
- **Bug reference:** `none`.
- **Test:** a differential test compares the bounds of each widget of mcrs with TITLE-SCR-006.
  It does not compare them with a vanilla capture.

### TITLE-DEV-003: The level loading screen has no map and no bar

- **Rule:** TITLE-LOAD-006, step 3.
- **Reference:** for a local world, the screen shows the map of the chunk statuses and the text
  above it (TITLE-LOAD-002).
- **Reason:** the mcrs client gets no chunk statuses from its local server. Item 1 of section 12
  holds the question to the owner.
- **Visible to the client:** yes. The text is at `H / 2 − 50` and no map shows.
- **Bug reference:** `none`.
- **Test:** a differential test compares the screen with the vanilla form for a server.

### TITLE-DEV-004: The last button returns to the title screen after a server

- **Rule:** TITLE-LEAVE-002, step 2.
- **Reference:** after a server, the server list opens (TITLE-LEAVE-001, step 5).
- **Reason:** mcrs has no server list.
- **Visible to the client:** yes, after "Disconnect" only. mcrs has no server list.
- **Bug reference:** `none`.
- **Test:** a differential test does not compare the screen after "Disconnect".

### TITLE-DEV-005: The disconnected screen leads to the title screen

- **Rule:** TITLE-DISC-003.
- **Reference:** the button is "Back to Server List" and opens the server list, or a title
  screen after a local world (TITLE-DISC-001).
- **Reason:** mcrs has no server list.
- **Visible to the client:** yes. The caption is "Back to Title Screen".
- **Bug reference:** `none`.
- **Test:** a differential test compares the title, the reason and the bounds. It does not
  compare the caption of the button.

### TITLE-DEV-006: The panorama turns at the default speed

- **Rule:** TITLE-PAN-005, step 2.
- **Reference:** the speed follows "Panorama Scroll Speed".
- **Reason:** mcrs has no option "Panorama Scroll Speed" (`client-menu.md`, MENU-ACC-002).
- **Visible to the client:** only for a player who changed that option in the vanilla client.
- **Bug reference:** `none`.
- **Test:** a differential test sets the vanilla option to 1.

### TITLE-DEV-007: Faces of different sizes show no panorama

- **Rule:** TITLE-PAN-005, step 4.
- **Reference:** each face shows the missing texture (TITLE-PAN-001, step 3).
- **Reason:** mcrs has no resource packs, so only a broken asset set has faces of different sizes.
  The warning in the log names that defect, and a checker picture helps no player.
- **Visible to the client:** only with a broken asset set. mcrs has no resource packs, and the
  faces of the asset index have one size.
- **Bug reference:** `none`.
- **Test:** none. The vanilla asset set has faces of one size.

### TITLE-DEV-008: The rare logo is picked once for each start

- **Rule:** TITLE-SCR-006, step 9.
- **Reference:** each title screen picks the logo when the client makes it (TITLE-SCR-002,
  step 2).
- **Reason:** with the chance 0.0001, a player sees no difference in practice.
- **Visible to the client:** in effect no. A player sees the other logo with the chance 0.0001
  in each start, in place of each title screen.
- **Bug reference:** `none`.
- **Test:** a differential test does not compare the logo texture.

### TITLE-DEV-009: A tooltip fades with the title screen

- **Rule:** TITLE-FADE-002, step 5.
- **Reference:** the tooltip of a widget draws at full strength during the fade
  (TITLE-FADE-001, step 3).
- **Reason:** the mcrs client draws all of the title screen in front of the panorama with one
  alpha. A tooltip at full strength over a widget that does not show yet points to nothing.
- **Visible to the client:** only when the player holds the mouse on a widget with a tooltip
  during the first 2 s of the first title screen.
- **Bug reference:** `none`.
- **Test:** a differential test does not compare the tooltips during the fade.

### TITLE-DEV-010: The level loading screen does not test a dead player

- **Rule:** TITLE-LOAD-006, step 4.
- **Reference:** the screen closes when the player is dead, also when the client cannot draw
  the section of the camera (TITLE-LOAD-003, step 3).
- **Reason:** the mcrs client has no death screen. The vanilla client closes the screen early for a
  dead player because its death screen covers the world. Without that screen, the player sees the
  hole at the camera.
- **Visible to the client:** only when the player joins a world as a dead player and the chunks
  are slow. The screen then closes at the timeout of 30 s.
- **Bug reference:** `none`.
- **Test:** a differential test does not join a world as a dead player.

### TITLE-DEV-011: No "Preparing Resources..." screen

- **Rule:** TITLE-LOAD-006, step 1.
- **Reference:** after "Reading world data...", "Preparing Resources..." shows while the client
  loads the data packs of the world (TITLE-LOAD-004, step 1).
- **Reason:** the mcrs client loads no data pack of a world. The local server reads the data packs.
- **Visible to the client:** yes, for a short time. The mcrs client loads no data pack of a
  world. The local server reads the data packs.
- **Bug reference:** `none`.
- **Test:** a differential test compares the order of the message screens and ignores
  "Preparing Resources...".

### TITLE-DEV-012: No frame with only the background after a server

- **Rule:** TITLE-LEAVE-002, step 2.
- **Reference:** a screen with only the background shows for one frame before the next screen
  (TITLE-LEAVE-001, step 3).
- **Reason:** the frame has no text and no widget, so it has no use for a player.
- **Visible to the client:** no. The screen shows for one frame and has no text or widget.
- **Bug reference:** `none`.
- **Test:** a differential test does not compare the frames between the pause screen and the
  next screen.

### TITLE-DEV-013: A local start that fails shows the disconnected screen

- **Rule:** TITLE-DISC-004, step 3, and TITLE-LOAD-006, edge case.
- **Reference:** for a world that another process holds, a toast shows "Failed to access world"
  and the title screen opens (`client-world-screens.md`, WORLD-DEV-009). For the other failures of a local start, the client stops with a
  crash report.
- **Reason:** mcrs has no toasts, and a crash of the client is a defect for mcrs.
- **Visible to the client:** only when a local world cannot start. The player sees the
  disconnected screen and can go back to the title screen. mcrs has no toasts, and a crash of
  the client is a defect for mcrs.
- **Bug reference:** `none`.
- **Test:** a differential test does not compare a local start that fails.

### TITLE-DEV-014: The reason of the disconnected screen has no style

- **Rule:** TITLE-DISC-003, step 1.
- **Reference:** the reason keeps the colours and the styles of its text component
  (TITLE-DISC-002, step 3).
- **Reason:** the mcrs client shows the reason as plain text. A reason with a colour or a style is
  rare, and its text stays complete without them.
- **Visible to the client:** only when the server sends a reason with a colour or a style. The
  reason of mcrs is plain text.
- **Bug reference:** `none`.
- **Test:** a differential test compares the plain text of the reason.

---

## 12. Unresolved

1. Must the level loading screen of mcrs show the map of the chunk statuses and the progress bar
   (TITLE-DEV-003)? A question to the owner.
