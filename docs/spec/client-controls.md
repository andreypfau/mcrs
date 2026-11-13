# Controls screens and mouse sensitivity of the client: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`.

This document states the "Controls" screen and the "Mouse Settings" screen of the vanilla client,
and the mcrs subset of each screen. It states the option Sensitivity, and the rule that turns a
mouse motion into a change of the yaw and the pitch of the local player. It uses the terms of
`client-menu.md`, `client-widget.md`, `client-list.md` and `client-camera.md`, and cites their
rules. The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- The terms of `client-menu.md`, section 0.1, apply with their words. Examples: **option**,
  **options file**, **stop of the client**, **options screen**, **option sub-screen**, **pending
  value** and **mcrs subset**.
- In this document, the "Controls" screen and the "Mouse Settings" screen are option sub-screens.
  "Mouse Settings" opens from "Controls", not from the options screen. The rules of
  `client-menu.md` for an option sub-screen apply to it.
- The terms of `client-widget.md`, section 0.1, and of `client-list.md`, section 0.1, apply as
  `client-menu.md` cites them.
- **Yaw**, **pitch**, **local player**, **frame**, **current value** and **previous value**: as
  `client-camera.md`, section 0.1, defines them. A positive pitch looks down. Yaw 0 faces +Z and
  yaw 90 faces −X (`client-entities.md`, section 0.1). Thus a larger yaw turns the player to its
  right.
- **Pointer capture**: the state in which the game has the mouse pointer. No screen is open, and
  the pointer does not show. WID-SCR-002, step 3, and MENU-PAUSE-004, step 2, start it.
  WID-SCR-001, step 2, ends it.
- **Mouse motion** (`dx`, `dy`): the relative motion of the mouse that the window system reports
  during the pointer capture. `dx` > 0 is a motion to the right. `dy` > 0 is a motion toward the
  player, which is down on the screen. The value is in motion units.
- **Motion unit**: the unit of the mouse motion that the window system reports. It is not a GUI
  unit, and it is not the change of the pointer position on a screen.
- **Sensitivity** (`s`): the option of CTRL-VAL-001, a real from 0 to 1.
- **Turn factor** (`g`): the factor that Sensitivity gives to the mouse motion (CTRL-TURN-002).

### 0.2 Provenance, confidence and observer

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". The language keys and their English texts are in
`assets/minecraft/lang/en_us.json` of the client jar. The width of a title comes from the glyphs
of `assets/minecraft/textures/font/ascii.png` of the client jar. No experiment of section 7 ran.

The observer of each rule is a player in normal play who sees the screen or the view, unless the
rule names another observer.

### 0.3 Arithmetic

The arithmetic of `client-menu.md`, section 0.3, applies. `x³` is `x · x · x`. Each example uses
W = 427 and H = 240.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CTRL-SCR` | the "Controls" screen |
| `CTRL-MOUSE` | the "Mouse Settings" screen |
| `CTRL-VAL` | the option Sensitivity |
| `CTRL-TURN` | the turn of the local player by the mouse motion |
| `CTRL-SAVE` | the moments when these screens write the options file |
| `CTRL-PROP` | the properties of these screens that `client-widget.md` leaves open |
| `CTRL-EXP` | experiments |
| `CTRL-DEV` | intentional deviations |

### 0.5 Scope

In scope:

- the "Controls" screen and the "Mouse Settings" screen, and their mcrs subsets;
- the option Sensitivity;
- the turn of the local player by the mouse motion, in first person.

Out of scope:

- the key binds and the "Key Binds" screen;
- the options Sneak, Sprint, Attack/Destroy and Use Item/Place Block (hold or toggle), Auto-Jump,
  Sprint Window, Quit Shortcuts, Right-Click Emulation and Operator Items Tab. This document names
  them only where an order needs them;
- the options Scroll Sensitivity, Discrete Scrolling, Invert Mouse X, Invert Mouse Y and Allow
  Cursor Changes. This document names them only where an order needs them;
- raw input and touchscreen mode;
- the turn while the player looks through a spyglass, and the cinematic camera;
- the turn of a vehicle by its passenger;
- the movement packets that carry the yaw and the pitch to the server.

### 0.6 Constants

| Value | Data | Rule |
|-------|------|------|
| Sensitivity: range, default | [0, 1], 0.5 | CTRL-VAL-001 |
| Sensitivity: factor of the caption | 200 | CTRL-VAL-001 |
| Turn: factor and offset of `s` | 0.6, 0.2 | CTRL-TURN-002 |
| Turn: factor of `f³` | 8 | CTRL-TURN-002 |
| Turn: degrees for each motion unit at `g` = 1 | 0.15 | CTRL-TURN-002 |
| Pitch: range | [−90, 90] | CTRL-TURN-002 |
| Title width of "Controls", of "Mouse Settings" | 43, 74 | CTRL-SCR-003, CTRL-MOUSE-003 |

---

## 1. "Controls" screen

### CTRL-SCR-001: Open and close the "Controls" screen

- **Inputs:** an activation of the button `options.controls`, "Controls...", then Escape or an
  activation of "Done". In vanilla, the button is on the options screen (MENU-OPT-002, button 4)
  and on "Accessibility Settings" (MENU-ACC-001, step 2). In mcrs, `client-menu.md` states the
  screens that show the button.
- **Behavior:**
  1. "Controls" opens. The screen that has the button is its parent screen.
  2. Escape and "Done" close "Controls" as MENU-SUB-002 states. The parent screen opens.
  3. The button "Mouse Settings..." opens "Mouse Settings" (CTRL-MOUSE-001). "Controls" is its
     parent screen.
  4. "Controls" keeps its widgets when it re-arranges (WID-SCR-003, the first kind). This is also
     true when it opens again after "Mouse Settings" closes.
- **Postconditions:** the client writes the options file each time that "Controls" is removed
  (CTRL-SAVE-001).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CTRL-EXP-01.

### CTRL-SCR-002: Rows of the vanilla "Controls" screen

- **Inputs:** W, H, and the platform of the client: macOS or another.
- **Behavior:**
  1. The title is `controls.title`, "Controls". The screen has the form of MENU-SUB-001, with the
     footer "Done", 200 × 20.
  2. The first row of the list holds two buttons of 150 × 20: `options.mouse_settings`, "Mouse
     Settings...", at the index 0, and `controls.keybinds`, "Key Binds...", at the index 1.
     "Key Binds..." opens the "Key Binds" screen, which is out of scope.
  3. A flat list of options fills the next rows, two in each row, as MENU-VID-001, step 3,
     states. In order:

| # | Key | English | Shows when |
|---|-----|---------|------------|
| 1 | `key.sneak` | "Sneak" | always |
| 2 | `key.sprint` | "Sprint" | always |
| 3 | `key.attack` | "Attack/Destroy" | always |
| 4 | `key.use` | "Use Item/Place Block" | always |
| 5 | `options.autoJump` | "Auto-Jump" | always |
| 6 | `options.sprintWindow` | "Sprint Window" | always |
| 7 | `options.quitShortcuts` | "Quit Shortcuts" | always |
| 8 | `options.ctrlClickEmulatesRightClick` | "Right-Click Emulation" | on macOS |
| 9 | `options.operatorItemsTab` | "Operator Items Tab" | always |

- **Postconditions:** at W = 427 and H = 240, with the scroll offset 0:
  - The widgets of row `r` are at y = `37 + 25 · r`, at x = 58 for the index 0 and x = 218 for
    the index 1. "Mouse Settings..." is at (58, 37). "Key Binds..." is at (218, 37).
  - On macOS, the list has six rows. "Operator Items Tab" is alone in row 5, at (58, 162). On
    another platform, the list has five rows. "Operator Items Tab" is at (218, 137).
  - The content height is 154 on macOS and 129 on another platform. The list is 174 high, so it
    does not scroll.
  - The title is at (192, 12). "Done" is at (113, 214).

Provenance: `source_suggested`; `black_box_confirmed` for the English texts. Kind: layout.
Confidence: medium. Experiment: CTRL-EXP-01.

### CTRL-SCR-003: "Controls" screen (mcrs subset)

- **Inputs:** W and H.
- **Behavior:**
  1. The screen keeps the title, the form and the footer of CTRL-SCR-002.
  2. The list holds one row: "Mouse Settings...", 150 × 20, alone at the index 0.
  3. Escape, "Done" and "Mouse Settings..." act as CTRL-SCR-001 states.
  4. The widgets go to the screen in this order: the title, the list, "Done" (MENU-SUB-001,
     step 5). This is also the order of the Tab navigation (WID-FOC-004).
- **Postconditions:** at W = 427 and H = 240:

| Widget | Bounds (x, y, w, h) |
|--------|---------------------|
| title "Controls" | (192, 12, 43, 9) |
| "Mouse Settings..." | (58, 37, 150, 20) |
| "Done" | (113, 214, 200, 20) |

  The row of "Mouse Settings..." has the top 35 and the height 25. The content height is 29, so
  the list does not scroll.

Provenance: `design` for the set of widgets. The owner decided on 2026-11-13 that a screen shows
only the widgets whose function mcrs has (`client-menu.md`, MENU-DEV-002), and that the mcrs
subset of "Controls" holds only "Mouse Settings...". `source_suggested` for the positions;
`black_box_confirmed` for the title width. Kind: layout. Confidence: medium. Experiment:
CTRL-EXP-01.

---

## 2. "Mouse Settings" screen

### CTRL-MOUSE-001: Open and close the "Mouse Settings" screen

- **Inputs:** an activation of "Mouse Settings..." on "Controls", then Escape or an activation of
  "Done".
- **Behavior:**
  1. "Mouse Settings" opens. "Controls" is its parent screen.
  2. Escape and "Done" close "Mouse Settings" as MENU-SUB-002 states. "Controls" opens again.
  3. "Mouse Settings" keeps its widgets when it re-arranges (WID-SCR-003, the first kind).
- **Postconditions:** the client writes the options file each time that "Mouse Settings" is
  removed (CTRL-SAVE-001). A chain of two Escape keys from "Mouse Settings" opens "Controls", then
  the screen that opened "Controls".

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CTRL-EXP-02.

### CTRL-MOUSE-002: Rows of the vanilla "Mouse Settings" screen

- **Inputs:** W and H.
- **Behavior:**
  1. The title is `options.mouse_settings.title`, "Mouse Settings". The screen has the form of
     MENU-SUB-001, with the footer "Done", 200 × 20.
  2. A flat list of options fills the rows, two in each row, as MENU-VID-001, step 3, states. In
     order:

| # | Key | English |
|---|-----|---------|
| 1 | `options.sensitivity` | "Sensitivity" (CTRL-VAL-001) |
| 2 | `options.mouseWheelSensitivity` | "Scroll Sensitivity" |
| 3 | `options.discrete_mouse_scroll` | "Discrete Scrolling" |
| 4 | `options.invertMouseX` | "Invert Mouse X" |
| 5 | `options.invertMouseY` | "Invert Mouse Y" |
| 6 | `options.allowCursorChanges` | "Allow Cursor Changes" |

- **Postconditions:**
  - At W = 427 and H = 240, the list has three rows. Their widgets are at y = 37, 62 and 87, and
    at x = 58 and x = 218. Sensitivity is at (58, 37).
  - The title is at (176, 12). "Done" is at (113, 214).
  - The screen has no row for raw input and no row for touchscreen mode.

Provenance: `source_suggested`; `black_box_confirmed` for the English texts. Kind: layout.
Confidence: medium. Experiment: CTRL-EXP-02.

### CTRL-MOUSE-003: "Mouse Settings" screen (mcrs subset)

- **Inputs:** W and H.
- **Behavior:**
  1. The screen keeps the title, the form and the footer of CTRL-MOUSE-002.
  2. The list holds one row: Sensitivity (CTRL-VAL-001), 150 × 20, alone at the index 0.
  3. Escape and "Done" act as CTRL-MOUSE-001 states.
  4. The widgets go to the screen in this order: the title, the list, "Done". This is also the
     order of the Tab navigation.
- **Postconditions:** at W = 427 and H = 240:

| Widget | Bounds (x, y, w, h) |
|--------|---------------------|
| title "Mouse Settings" | (176, 12, 74, 9) |
| Sensitivity | (58, 37, 150, 20) |
| handle of Sensitivity at the default 0.5 | (129, 37, 8, 20) |
| "Done" | (113, 214, 200, 20) |

  The content height is 29, so the list does not scroll.

Provenance: `design` for the set of widgets. The owner decided on 2026-11-13 that the mcrs subset
of "Mouse Settings" holds only Sensitivity. `source_suggested` for the positions;
`black_box_confirmed` for the title width. Kind: layout. Confidence: medium. Experiment:
CTRL-EXP-02.

---

## 3. Sensitivity

### CTRL-VAL-001: Sensitivity

- **Behavior:**
  1. The widget is a slider whose value `s` is its position, from 0 to 1. The default is 0.5.
  2. In change mode, Left and Right move the position by `1 / (w − 8)` (WID-SLD-003, step 3). For
     the width 150, the step is 1/142.
  3. When `s` is exactly 0, the caption is `options.generic_value`, `"%s: %s"`, with
     `options.sensitivity`, "Sensitivity", and `options.sensitivity.min`, "*yawn*".
  4. When `s` is exactly 1, the caption is `options.generic_value` with "Sensitivity" and
     `options.sensitivity.max`, "HYPERSPEED!!!".
  5. Else the caption is `options.percent_value`, `"%s: %s%%"`, with "Sensitivity" and
     `trunc(200 · s)`.
  6. The slider has no tooltip.
  7. The option applies at once (WID-SLD-007, step 1). The next turn uses the new value
     (CTRL-TURN-002).
- **Postconditions:**
  - The default shows "Sensitivity: 100%", with the handle at the centre.
  - The handle at the left end shows "Sensitivity: *yawn*". The handle at the right end shows
    "Sensitivity: HYPERSPEED!!!".
  - One key step right of the left end shows "Sensitivity: 1%". One key step left of the right
    end shows "Sensitivity: 198%".
  - A position above 0 and below 0.005 shows "Sensitivity: 0%".
- **Edge cases:** the change of the slider does not write the options file at once
  (CTRL-SAVE-001).

Provenance: `source_suggested`; `black_box_confirmed` for the English texts. Kind: semantic.
Confidence: medium. Experiment: CTRL-EXP-03.

---

## 4. Turn of the player

### CTRL-TURN-001: When a mouse motion turns the player

- **Inputs:** the mouse motion events of a frame, the pointer capture, the focus of the window,
  and the local player.
- **Behavior:**
  1. During the pointer capture, while the window has the focus, the client adds `dx` and `dy` of
     each mouse motion event to two sums.
  2. In each frame with the pointer capture and a local player, the client applies one turn with
     the two sums (CTRL-TURN-002). This occurs before the client places the camera
     (`client-camera.md`, CCAM-TIME-002, step 1).
  3. The sums become 0 at the end of each frame, also when no turn occurs.
  4. While a screen is open, no pointer capture exists. A motion of the mouse goes to the screen
     (WID-PTR-001). It does not turn the player.
  5. While the window does not have the focus, the client does not add a mouse motion to the sums.
  6. The client does not scale the mouse motion by the GUI scale or by the window size.
- **Postconditions:**
  - A mouse motion while a screen is open does not turn the player after the screen closes.
  - The view turns in the frame of the mouse motion (CCAM-POSE-002, postconditions).
  - Two mouse motion events of +100 and −100 on `dy` in one frame give one turn of 0. At the pitch
    89, the pitch stays 89. One turn for each event gives another pitch, because of the clamp.
- **Edge cases:** the vanilla client takes its window input from SDL 3 (`lwjgl-sdl` in the
  library list of the version manifest). Its mouse motion is the relative motion of SDL 3 in the
  relative mouse mode. Section 9, item 1, asks whether mcrs has the same motion unit.

Provenance: `source_suggested`; `black_box_confirmed` for the library list of the version
manifest of `26.4-snapshot-7`. Kind: semantic. Confidence: medium. Experiment: CTRL-EXP-04.

### CTRL-TURN-002: Change of the yaw and the pitch

- **Inputs:** the sums `dx` and `dy` of a frame (CTRL-TURN-001), Sensitivity `s`, the current yaw
  `y` and pitch `p`, and the previous yaw `y'` and pitch `p'` of the local player.
- **Behavior:**
  1. `f` = `0.6 · s + 0.2`.
  2. `g` = `8 · f³`.
  3. The change of the yaw is `Δy` = `0.15 · g · dx` degrees. The change of the pitch is
     `Δp` = `0.15 · g · dy` degrees. The client computes each change as an `f32`.
  4. `y` becomes `y + Δy`, and `y'` becomes `y' + Δy`. The yaw has no clamp and no wrap.
     `client-camera.md`, CCAM-POSE-002, step 3, states the interpolation for a client that keeps
     the yaw in [−180, 180).
  5. `p` becomes `clamp(p + Δp, −90, 90)`. `p'` becomes `clamp(p' + Δp, −90, 90)`.
  6. The yaw and the pitch are `f32` values.
- **Postconditions:**
  - These values of `s` give these turns for each motion unit, on each axis:

| `s` | Caption | `g` | Degrees for each motion unit |
|-----|---------|-----|------------------------------|
| 0 | "Sensitivity: *yawn*" | 0.064 | 0.0096 |
| 0.5 | "Sensitivity: 100%" | 1 | 0.15 |
| 1 | "Sensitivity: HYPERSPEED!!!" | 4.096 | 0.6144 |

  - At the default Sensitivity 0.5, one motion unit turns the player by 0.15 degree on each axis.
  - A mouse motion to the right makes the yaw larger, and the player turns to its right. A player
    with the yaw 0 faces +Z, and a motion to the right turns it toward −X.
  - A mouse motion toward the player makes the pitch larger, and the player looks down.
  - A motion of 1000 units toward the player at `s` = 0.5 from the pitch 0 gives the pitch 90.
  - The previous pitch has the same clamp, so the camera does not interpolate past ±90
    (CCAM-POSE-002, step 3).
- **Edge cases:**
  - These steps are the vanilla behaviour with Invert Mouse X and Invert Mouse Y off, which are
    their defaults. The spyglass and the cinematic camera change the turn. They are out of scope.
  - In the reference, the constants 0.6 and 0.2 are 32-bit reals in a 64-bit computation. Thus
    `g` at `s` = 0.5 is 1.0000001. No observer sees the difference. A parity test compares a turn
    with a relative tolerance of `1e-6`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CTRL-EXP-04.

---

## 5. Options file

### CTRL-SAVE-001: When these screens write the options file

- **Observer:** the options file, as the client reads it at the next start.
- **Behavior:** MENU-SAVE-001, step 2, applies to "Controls" and to "Mouse Settings". Thus the
  client writes the options file at these moments:
  1. "Controls" opens: the screen that opened it is removed.
  2. "Mouse Settings" opens: "Controls" is removed.
  3. "Mouse Settings" closes: it is removed, and "Controls" opens again.
  4. "Controls" closes: it is removed, and its parent screen opens.
- **Postconditions:**
  - A change of Sensitivity writes nothing at once. The write of step 3 includes it.
  - At a stop of the client with "Controls" or "Mouse Settings" open, MENU-SAVE-002 applies.
    mcrs acts as MENU-DEV-005 states.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CTRL-EXP-03.

---

## 6. Screen properties

### CTRL-PROP-001: Properties of the screens of this document

- **Behavior:** these are the properties of the columns of MENU-PROP-001:

| Screen | In-game screen | Own background | Separators | Re-arrange (WID-SCR-003) | Tab order group | Sliders that wait |
|--------|----------------|----------------|------------|--------------------------|-----------------|-------------------|
| "Controls" | no | no | of the list (LIST-BG-002) | keeps its widgets | 0 | none |
| "Mouse Settings" | no | no | of the list (LIST-BG-002) | keeps its widgets | 0 | none |

- **Postconditions:**
  - Each screen draws the background of WID-SCR-004, step 2. The screen draws no separators of
    WID-SCR-006.
  - No screen names an initial widget (WID-FOC-002, step 1).
  - In vanilla, Sprint Window, Sensitivity and Scroll Sensitivity apply at once. No slider of
    these screens waits 600 ms.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: CTRL-EXP-02.

---

## 7. Experiments

None of these experiments ran. No game window started for this document. Each experiment uses
the setup of `client-widget.md`, section 14: window pixels of 854 × 480 and the GUI scale 2.
"Record the file time" means: record the time of the last change of `options.txt` in the game
directory.

The input agent of `tools/vanilla-client` sends no GUI input and no mouse motion, and its window
never takes the focus. Each experiment of this section needs an agent that sends these inputs to
a window with the focus, or a person at the window.

### CTRL-EXP-01: "Controls"

1. In a world, open the options screen. Record the file time.
2. Press "Controls...". Record the file time, the screenshot, the title and the bounds of each
   widget.
3. Press Tab three times. Record the focused widget after each key.
4. Press Escape. Record the screen that shows and the file time.
5. Open "Accessibility Settings...", then "Controls...". Press "Done". Record the screen that
   shows.
6. Do steps 2 and 4 again on macOS and on another platform. Record the rows of the list.

Decides: CTRL-SCR-001, CTRL-SCR-002, and the positions of CTRL-SCR-003.

### CTRL-EXP-02: "Mouse Settings"

1. Open "Controls", then "Mouse Settings...". Record the file time, the screenshot, the title and
   the bounds of each widget.
2. Record the GUI rows of pixels at y = 31, 32 and at y = 207, 208.
3. Change the window size. Record the bounds of each widget and the focused widget.
4. Press Escape. Record the screen that shows and the file time.
5. Press Escape. Record the screen that shows.

Decides: CTRL-MOUSE-001, CTRL-MOUSE-002, CTRL-PROP-001, and the positions of CTRL-MOUSE-003.

### CTRL-EXP-03: Sensitivity

1. Open "Mouse Settings". Record the caption of Sensitivity and the position of its handle.
2. Put the mouse on the slider for 2 seconds. Record that no tooltip shows.
3. Drag the handle to the left end, to the centre and to the right end. Record the caption and
   the file time after each drag.
4. Drag the handle to the right end. Press Left one time. Record the caption.
5. Drag the handle to the left end. Press Right one time. Record the caption.
6. Press "Done". Record the file time and the value of the option in `options.txt`.

Decides: CTRL-VAL-001 and CTRL-SAVE-001.

### CTRL-EXP-04: The turn

1. In a world, with no screen open, set the yaw 0 and the pitch 0.
2. For Sensitivity 0, 0.5 and 1, send a mouse motion of 100 units to the right in one frame.
   Record the yaw and the pitch after the frame.
3. For the same values, send a mouse motion of 100 units toward the player. Record the pitch.
4. With Sensitivity 0.5, send a mouse motion of 1000 units toward the player. Record the pitch.
5. Set the pitch 89. In one frame, send a mouse motion of +100 on `dy`, then one of −100. Record
   the pitch.
6. Open the pause screen. Send a mouse motion of 500 units to the right. Press Escape. Record
   the yaw in the next 10 frames.
7. Remove the focus from the window. Send a mouse motion of 500 units. Give the focus back.
   Record the yaw.

Decides: CTRL-TURN-001 and CTRL-TURN-002. The expected yaw changes of step 2 are 0.96, 15 and
61.44 degrees.

### CTRL-EXP-05: The motion unit

1. Use a device that moves the mouse by a known number of device counts, on macOS, on Windows and
   on Linux.
2. On macOS, use a display with a pixel density of 1 and a display with a pixel density of 2.
3. For two values of the pointer speed of the system, move the mouse by 1000 counts to the right
   in the vanilla client, with Sensitivity 0.5. Record the change of the yaw.
4. Do step 3 again in the mcrs client.
5. Compare the changes of the yaw of the two clients for each platform, display and pointer
   speed.

Decides: item 1 of section 9.

---

## 8. Intentional deviations

### CTRL-DEV-001: The screens show only the widgets of mcrs

- **Rule:** CTRL-SCR-003, CTRL-MOUSE-003.
- **Reference:** each screen shows all the widgets of CTRL-SCR-002 and CTRL-MOUSE-002.
- **Visible to the client:** yes. "Controls" shows only "Mouse Settings...". "Mouse Settings"
  shows only Sensitivity. mcrs has no invert option: its turn is the vanilla turn with the
  invert options off.
- **Test:** a differential test compares the bounds of each widget of mcrs with the rules of the
  mcrs subset. It does not compare them with a vanilla capture.

---

## 9. Unresolved

1. The vanilla client takes the relative motion of SDL 3 (CTRL-TURN-001, edge cases). mcrs takes
   the motion that its window library gives. These units can differ on a platform: in the
   pointer acceleration of the system, or with a pixel density above 1. When CTRL-EXP-05 finds a
   difference, must mcrs convert its motion to the unit of the vanilla client?
