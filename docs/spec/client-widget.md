# Screens and widgets of the client GUI: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`.

This document states the general model of a screen and its widgets in the vanilla client GUI.
It states the pointer input, the keys and the focus, the click sound, the tooltip of a widget,
the core widgets and the layout arithmetic. An implementer can build a menu of buttons,
cycle buttons, sliders, checkboxes and text from it. The document does not prescribe an
architecture.

## 0. Conventions

### 0.1 Terms

- **GUI unit**, **W**, **H**: as `client-chat.md`, section 0.1, defines them.
- **Mouse position**: as `client-tooltip.md`, section 0.1, defines it. It is a pair of integers.
- **Event position**: the point (`ex`, `ey`) of a press, a release, a drag or a wheel event.
  `ex` is `px · W / ww` as a real number, with no floor. `px` and `ww` are as for the mouse
  position. `ey` uses the heights in the same way.
- **Frame**: as `client-tooltip.md`, section 0.1, defines it.
- **In a world**: the client shows a level, in singleplayer or in multiplayer.
- **Screen**: the GUI state that covers the window and takes the input of the mouse and the
  keyboard. It draws a background and a list of widgets. At most one screen is open.
- **Parent screen**: the screen that a screen opens when it closes. A screen can have no parent
  screen.
- **In-game screen**: a screen that draws over the world with a gradient (WID-SCR-004). Each
  screen states if it is an in-game screen. `client-menu.md` lists them.
- **Widget**: a part of a screen that has bounds, draws itself and can take input.
- **Bounds**: the rectangle (`x`, `y`, `w`, `h`) of a widget in GUI units. The left edge `x`
  and the top edge `y` are in the rectangle. The right edge `x + w` and the bottom edge `y + h`
  are not in it.
- **Visible**: a widget that draws and can take input. A widget that is not visible does
  neither, and it holds its place in the layout.
- **Active**: a visible widget that accepts a press and the activation keys. An inactive widget
  draws in its inactive form.
- **Hovered**: the state of a widget under the mouse (WID-STATE-002).
- **Focused**: the state of the one widget of a screen, or of none, that receives the keys
  first (WID-FOC-001).
- **Highlighted**: the state that selects the highlighted sprite of a widget (WID-STATE-002).
- **Activation**: the action of a widget that a press of the left mouse button or an activation
  key starts. For example, the action of a button opens a screen.
- **Activation key**: the key Enter, Space or numpad Enter.
- **Click sound**: the sound of an activation (WID-SND-001).
- **Focus navigation**: the change of the focused widget by Tab, Shift+Tab or an arrow key
  (WID-FOC-004, WID-FOC-005).
- **Tooltip of a widget**: the tooltip that a widget asks for while it is hovered or focused
  (WID-TIP-001). It is a tooltip request as `client-tooltip.md` defines it.
- **Last input device**: one of the values none, mouse, Tab, arrow and activation
  (WID-IN-001). "Keyboard" means one of the values Tab, arrow and activation.
- **Tab order group**: an integer of a widget, default 0, that sorts the widgets for Tab
  (WID-FOC-004).
- **Caption**: the text component that a widget shows, for example the text of a button.
- **Alpha**: the opacity of a widget, from 0 to 1, default 1. It multiplies the alpha of the
  sprites and of the caption of the widget.
- **Label area**: the part of a widget in which its caption draws (WID-BTN-004).
- **Label scroll**: the slow move of a caption that is wider than its label area
  (WID-BTN-005).
- **Change mode**: a state of a focused slider in which Left and Right change the value
  (WID-SLD-003).
- **Text widget**: a one-line text widget (WID-TXT-001) or a multi-line text widget
  (WID-TXT-002).
- **Layout element**: a widget, a layout or a spacer. It has a position and a size.
- **Layout**: a layout element that places other layout elements, its children.
- **Cell**: the place of one child in a layout. It holds the padding and the alignment of the
  child.
- **Padding**: the four margins `pl`, `pt`, `pr` and `pb` (left, top, right, bottom) of a
  cell, in GUI units. Each is ≥ 0. The default is 0.
- **Alignment**: the two real numbers `ax` and `ay` of a cell, each from 0 to 1. 0 is the left
  or the top, 0.5 is the centre, and 1 is the right or the bottom.
- **Gap**: the empty space between two neighbour rows or two neighbour columns, in GUI units.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". Each experiment is in section 14. No experiment ran.

### 0.3 Colours, text and arithmetic

- A colour is a 32-bit ARGB value in hexadecimal. "White" is `FFFFFFFF`.
- A colour "times alpha" keeps the red, the green and the blue, and multiplies its alpha by the
  alpha of the widget.
- The font, the width of a text, the line height 9 and the drop shadow are as
  `client-chat.md`, section 0.3, states. To wrap a text at a width is as `client-tooltip.md`,
  section 0.3, states.
- A key is a physical key position, as `client-chat.md` states for key bindings.
- A sprite name with no namespace has the namespace `minecraft`. A sprite draws into a
  rectangle as `client-gui-sprite.md` states for its scaling.
- `a / b`, with the integers `a` and `b`, is the integer quotient, rounded toward zero.
  `a % b` is the remainder `a − (a / b)·b`.
- `trunc(v)` rounds the real `v` toward zero. `floor(v)` rounds it down. `round(v)` is
  `floor(v + 0.5)`.
- `clamp(v, lo, hi)` is `lo` when `v < lo`, else `min(v, hi)`.
- `lerp(t, p, q)` is `p + t·(q − p)`. In section 13 it uses 32-bit floating point. The other
  real values of this document are 64-bit floating point.
- **Even split** of the integer `n` into `d` ≥ 1 parts: let `q = n / d` and `r = n % d`.
  When `r` ≥ 0, part `i` (from 1 to `d`) is `q + 1` when `(i·r) / d > ((i − 1)·r) / d`, else
  `q`. When `r` < 0, each part is `q`. Example: 7 into 3 parts gives 2, 2, 3.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `WID-SCR` | screens: open, close, re-arrange, background, draw order |
| `WID-IN` | the last input device |
| `WID-PTR` | the press, the drag, the release and the wheel |
| `WID-FOC` | the focus, the keys and focus navigation |
| `WID-STATE` | the common state of a widget and its effect on the draw |
| `WID-SND` | the click sound |
| `WID-TIP` | the tooltip of a widget |
| `WID-BTN` | the button, its caption and the label scroll |
| `WID-ICON` | buttons with an icon, and buttons that are only a sprite |
| `WID-CYC` | the cycle button |
| `WID-SLD` | the slider |
| `WID-CHK` | the checkbox |
| `WID-TXT` | the one-line and the multi-line text widgets |
| `WID-LAY` | the layout arithmetic and the standard sizes |
| `WID-EXP` | experiments |
| `WID-DEV` | intentional deviations |

### 0.5 Scope

In scope:

- the model of a screen and its widgets;
- the pointer input, the keys and the focus;
- the common state of a widget, the click sound and the tooltip of a widget;
- the button, the icon button, the image button, the cycle button, the slider and the checkbox;
- the text widgets;
- the layouts: row, column, grid, frame, equal gaps, spacer, and header, content and footer.

Out of scope, with the document that states it:

- the one-line text input: `client-edit-box.md`;
- the scroll area, the scroll bar, the lists and the tab bar: `client-list.md`;
- the pause screen, the options screens and the other menus: `client-menu.md`;
- the narrator and the other accessibility features;
- the title screen and the panorama;
- the toasts and the HUD;
- a button that shows a load animation in place of its caption.

---

## 1. Screens

### WID-SCR-001: Open a screen

- **Inputs:** a screen `S`, the screen that is open now or none, W and H.
- **Behavior:**
  1. When no screen is open, set the last input device to none (WID-IN-001).
  2. Release each mouse key binding that is down. Release the mouse pointer from the game, so
     that it shows.
  3. When `S` opens for the first time, `S` builds its widgets for W and H. Then it sets the
     initial focus (WID-FOC-002).
  4. When `S` was open before, for example a parent screen, `S` re-arranges (WID-SCR-003).
  5. `S` replaces the screen that was open. `S` draws from the next frame (WID-SCR-007).
- **Postconditions:** at most one screen is open. A screen that opens from the game shows no
  highlighted widget until the player moves the mouse or uses a key.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-01.

### WID-SCR-002: Close a screen

- **Inputs:** the screen `S`, its parent screen `P` or none, and a close request. Escape makes
  a close request (WID-FOC-003). A widget of `S` can make one, for example a Done button.
- **Behavior:**
  1. Each screen states if Escape closes it. Most screens allow it. Escape on a screen that
     does not allow it goes to the focused widget, as other keys do.
  2. When `S` has a parent screen, `P` opens (WID-SCR-001).
  3. Else, in a world, `S` closes and the player returns to the game. The game takes the
     mouse pointer again. `client-chat.md` states when the chat screen opens again with its
     draft.
  4. Else, the title screen opens.
- **Postconditions:** a chain of screens closes one level for each Escape.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-01.

### WID-SCR-003: Re-arrange after a resize or a return

- **Inputs:** an open screen `S`, and one of these events:
  - the window size changes;
  - the GUI scale changes;
  - `S` opens again as a parent screen (WID-SCR-002).
- **Behavior:** `S` gets the new W and H. Then it re-arranges in one of two ways:
  - A screen that keeps its widgets keeps their values and the focus. It places the widgets
    again for W and H (section 13). The menus with a header, a content and a footer
    (WID-LAY-011) are of this kind.
  - Each other screen builds its widgets again. It removes all widgets, clears the focus,
    builds the widgets for W and H, and sets the initial focus again (WID-FOC-002).
- **Postconditions:** the last input device does not change. A value that a screen of the
  second kind keeps only in a widget returns to its start value.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-02.

### WID-SCR-004: Background of a screen

- **Inputs:** the screen, W, H, and whether the client is in a world.
- **Behavior:**
  1. An in-game screen fills (0, 0, W, H) with a vertical gradient. The top colour is
     `C0101010` and the bottom colour is `D0101010`.
  2. Each other screen draws these parts, in this order:
     1. with no world, the panorama (out of scope);
     2. the blur, when WID-SCR-005 asks for it;
     3. the menu background texture over (0, 0, W, H).
  3. The menu background texture is `minecraft:textures/gui/menu_background.png` with no world,
     and `minecraft:textures/gui/inworld_menu_background.png` in a world.
  4. The texture tiles from the point (0, 0). One tile is the full texture over 32 × 32 GUI
     units.
- **Postconditions:** in the vanilla resource pack, each pixel of the two textures is
  `40000000`. The texture darkens the content behind it by the alpha 64/255.
- **Edge cases:** a screen can draw its own background. `client-menu.md` lists such screens.

Provenance: `source_suggested`; `black_box_confirmed` for the pixel values of
`assets/minecraft/textures/gui/menu_background.png` and
`assets/minecraft/textures/gui/inworld_menu_background.png` of the client jar (16 × 16 pixels,
each `40000000`). Kind: layout. Confidence: medium. Experiment: WID-EXP-03.

### WID-SCR-005: Blur behind a screen

- **Inputs:** the option "Menu Background Blur"
  (`options.accessibility.menu_background_blurriness`). It is an integer from 0 to 10. The
  default is 5.
- **Behavior:**
  1. When the option is 0, there is no blur.
  2. When the option is ≥ 1, the client blurs all that it drew before the menu background
     texture, for example the world or the panorama. The texture and the widgets do not blur.
  3. The blur is the post effect `minecraft:blur` of the resource pack. It has three pairs of
     passes of the shader `minecraft:post/box_blur`: a horizontal pass, then a vertical pass.
  4. The radius of each pass is the option value, in pixels of the window. It does not change
     with the GUI scale.
- **Postconditions:** the shader `box_blur.fsh` of the resource pack states the samples and
  their weights.

Provenance: `source_suggested`; `black_box_confirmed` for the passes of
`assets/minecraft/post_effect/blur.json` and the samples of
`assets/minecraft/shaders/post/box_blur.fsh` of the client jar. Kind: layout. Confidence:
medium. Experiment: WID-EXP-03.

### WID-SCR-006: Separators of a screen with no list

- **Inputs:** a screen with a header, a content and a footer (WID-LAY-011) that shows no list.
  Each screen states if it draws separators. `client-menu.md` lists such screens. The screen
  gives the header height `hh`, the footer height `fh` and the separator height `sh`. The
  vanilla screens use `sh` = 2.
- **Behavior:**
  1. The header separator fills the rectangle (0, `hh − 2`, W, `sh`). Its texture is
     `minecraft:textures/gui/header_separator.png`, or
     `minecraft:textures/gui/inworld_header_separator.png` in a world.
  2. The footer separator fills the rectangle (0, `H − fh`, W, `sh`). Its texture is
     `minecraft:textures/gui/footer_separator.png`, or
     `minecraft:textures/gui/inworld_footer_separator.png` in a world.
  3. Each texture tiles across from x = 0. One tile is the full texture over 32 × `sh` GUI
     units.
- **Postconditions:** in the vanilla resource pack, the header separator is a row of
  `33FFFFFF` above a row of `BF000000`. The footer separator is a row of `BF000000` above a row
  of `33FFFFFF`.
- **Edge cases:** a list draws its own separators over its own bounds (`client-list.md`,
  LIST-BG-002). A screen that shows a list does not draw the separators of this rule. Thus each
  separator draws one time.

Provenance: `source_suggested`; `black_box_confirmed` for the pixels of the four textures of
the client jar (32 × 2 pixels each). Kind: layout. Confidence: medium. Experiment: WID-EXP-03.

### WID-SCR-007: Draw order of a screen

- **Behavior:** in each frame, the screen draws these parts, in this order:
  1. the background (WID-SCR-004);
  2. the visible widgets, in their order of addition to the screen;
  3. the tooltip of the frame (`client-tooltip.md`, TIP-REQ-001, with WID-TIP-005).
- **Postconditions:** a later widget draws over an earlier widget. The tooltip is above all.
- **Edge cases:** where two widgets overlap, the press goes to the earlier widget
  (WID-PTR-002). That is the widget below.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-04.

---

## 2. Input device and pointer

### WID-IN-001: Last input device

- **Behavior:** the client keeps one last input device for the GUI. These events set it:

| Event | New value |
|-------|-----------|
| A screen opens while no screen is open | none |
| Tab, with or without Shift | Tab |
| An arrow key | arrow |
| An activation key | activation |
| A press of a mouse button, or a wheel event | mouse |
| A move of the mouse more than 7 GUI units from the anchor | mouse |

  1. The anchor is the mouse position at the last change of the last input device. The
     distance is the straight distance between the two points.
  2. An arrow key and an activation key have no effect on the device while the focused widget
     takes text input (`client-edit-box.md`).
  3. A press and a wheel event set the device before the screen looks for a widget
     (WID-PTR-002).
- **Postconditions:** after a key moves the focus, no widget is hovered. A move of more than
  7 GUI units, a press or a wheel event makes a widget hovered again.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-05.

### WID-PTR-001: The mouse over a widget

- **Inputs:** a point (`px`, `py`), and a widget with the bounds (`x`, `y`, `w`, `h`).
- **Behavior:** the point is over the widget when all these conditions are true:
  - the widget is active;
  - the last input device is mouse;
  - `x ≤ px < x + w` and `y ≤ py < y + h`.
- **Postconditions:** an inactive widget is never under the point for a press or a wheel event.
  A press can reach an active widget below it.
- **Edge cases:** a widget with `w` = 0 or `h` = 0 has no point over it. For a widget with
  integer edges, the event position and the mouse position give the same result.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-04.

### WID-PTR-002: Press of a mouse button

- **Inputs:** a press of a mouse button at the event position, and the widgets of the screen
  in their order of addition.
- **Behavior:**
  1. Set the last input device to mouse.
  2. Find the first widget in the order of addition that the point is over (WID-PTR-001).
  3. When there is no such widget, the press has no effect. The focus does not change.
  4. The widget accepts the press when the button is the left button. Each widget of this
     document accepts only the left button.
  5. When the widget accepts the press, it plays the click sound, then it starts its
     activation (WID-SND-001). The slider is an exception (WID-SLD-002).
  6. Then the widget becomes the focused widget. The screen starts a drag with it.
  7. When the widget does not accept the press, nothing else occurs. The press does not go to
     a widget below.
- **Postconditions:** a press of the right button on a button plays no sound and does not
  change the focus.
- **Edge cases:** two presses of the same button on the same screen within 250 ms are a double
  click. The widget gets this flag with the second press. `client-list.md` uses it.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-04.

### WID-PTR-003: Drag and release

- **Inputs:** a move of the mouse with the left button down, or a release of a mouse button.
- **Behavior:**
  1. While the screen drags, each move with the left button down goes to the focused widget,
     with the event position. It goes to the widget also when the point is outside it.
  2. A release of the left button ends the drag. When the screen dragged, the release goes to
     the focused widget.
  3. A release of another button goes to no widget.
- **Postconditions:** of the widgets of this document, only the slider uses the drag and the
  release (WID-SLD-002).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-07.

### WID-PTR-004: Wheel

- **Inputs:** a wheel event with the amounts across and down, at the event position. The amount
  down is the wheel value `v` of `client-list.md`, SCRL-AREA-002, step 1, so each widget gets
  the same amount.
- **Behavior:**
  1. Set the last input device to mouse.
  2. The event goes to the first widget in the order of addition that the point is over
     (WID-PTR-001).
  3. The kind of the widget selects the effect. Of the widgets of this document, only the cycle
     button uses the wheel (WID-CYC-002).
- **Postconditions:** the wheel does not change the focus. A wheel event over no widget has no
  effect.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-06.

---

## 3. Keyboard and focus

### WID-FOC-001: The focused widget

- **Behavior:**
  1. A screen has at most one focused widget.
  2. A widget can take the focus by focus navigation when it is active and it is not focused.
  3. A press that a widget accepts gives the focus to that widget (WID-PTR-002).
  4. A move of the mouse does not change the focus.
- **Postconditions:** an inactive widget and a widget that is not visible never take the focus
  by focus navigation. A text widget is never focused, because it is not active.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-05.

### WID-FOC-002: Initial focus

- **Inputs:** a screen that builds its widgets (WID-SCR-001, WID-SCR-003), and the last input
  device.
- **Behavior:**
  1. A screen can name its initial widget. The screen focuses that widget when it can take the
     focus, with any last input device.
  2. Else, when the last input device is a keyboard value, the screen focuses the first widget
     of a forward Tab navigation with no focus (WID-FOC-004).
  3. Else, no widget is focused.
- **Postconditions:**
  - A screen that opens from the game has no focused widget, because WID-IN-001 sets none.
  - A screen that opens by an activation key on a button of another screen focuses its first
    widget.
  - A screen that opens by a mouse press has no focused widget.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-05.

### WID-FOC-003: The order in which a screen uses a key

- **Inputs:** a key press while a screen is open. WID-IN-001 applies first.
- **Behavior:**
  1. When the key is Escape and the screen allows a close by Escape, the screen closes
     (WID-SCR-002). Stop.
  2. The focused widget gets the key. When the widget uses the key, stop. For example, a button
     uses an activation key, and a slider in change mode uses Left and Right.
  3. Tab starts a forward Tab navigation. Shift+Tab starts a backward Tab navigation
     (WID-FOC-004).
  4. An arrow key starts an arrow navigation in its direction (WID-FOC-005).
  5. Other keys have no effect in this document.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-05.

### WID-FOC-004: Tab navigation

- **Inputs:** the direction (forward or backward), the widgets, and the focused widget `F` or
  none.
- **Behavior:**
  1. Sort the widgets by tab order group, the smallest first. In one group, keep the order of
     addition.
  2. Forward: examine the widgets after `F` in the sorted order. With no `F`, start at the
     first widget.
  3. Backward: examine the widgets before `F`, in the reverse order. With no `F`, start at the
     last widget.
  4. The first widget that can take the focus (WID-FOC-001) becomes the focused widget.
  5. When no such widget is found, clear the focus. Then do steps 2 to 4 again with no `F`.
- **Postconditions:**
  - Tab goes around all the widgets that can take the focus, in a loop.
  - With one widget that can take the focus, Tab keeps the focus on it.
  - With no widget that can take the focus, the focus clears.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-05.

### WID-FOC-005: Arrow navigation

- **Inputs:** the direction `D` (Up, Down, Left or Right), the widgets, and the focused widget
  `F` or none.
- **Behavior:**
  1. With `F`, let `S` be the strip of 1 GUI unit just outside `F` on the side `D`. On the
     other axis, `S` has the length of `F`. For Down, `S` is (`F.x`, `F.y + F.h`, `F.w`, 1).
  2. With no `F`, let `S` be the strip of 1 GUI unit just outside the screen on the side
     opposite `D`. On the other axis, `S` has the length of the screen. For Down, `S` is
     (0, −1, W, 1).
  3. The candidates are the widgets other than `F` that overlap `S` on the other axis, and
     that meet the test of the table. `C` is the bounds of a candidate, with the right edge
     `C.x + C.w` and the bottom edge `C.y + C.h`.

| `D` | Overlap on the other axis | Test |
|-----|---------------------------|------|
| Down | `C.x < S.x + S.w` and `S.x < C.x + C.w` | `C.y > S.y`, or `C.y = S.y` and `C.h` ≥ 2 |
| Up | the same as for Down | `C.y + C.h < S.y + 1`, or `C.y + C.h = S.y + 1` and `C.y < S.y` |
| Right | `C.y < S.y + S.h` and `S.y < C.y + C.h` | `C.x > S.x`, or `C.x = S.x` and `C.w` ≥ 2 |
| Left | the same as for Right | `C.x + C.w < S.x + 1`, or `C.x + C.w = S.x + 1` and `C.x < S.x` |

  4. Sort the candidates by the near edge, the nearest first. The near edge is `C.y` for Down,
     `C.y + C.h` for Up, `C.x` for Right and `C.x + C.w` for Left.
  5. On equal near edges, sort by `C.x`, the smallest first, for Up and Down. Sort by `C.y`,
     the smallest first, for Left and Right.
  6. The first candidate that can take the focus becomes the focused widget.
  7. When no candidate takes the focus, do the fallback of WID-FOC-006.
- **Postconditions:**
  - With no `F`, Down focuses the top widget, Up the bottom widget, Right the left widget, and
    Left the right widget. Equal near edges select by step 5.
  - Arrow navigation does not wrap. At the last widget in a direction, the focus does not
    change.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-05.

### WID-FOC-006: Fallback of the arrow navigation

- **Inputs:** the strip `S`, the direction `D` and the focused widget `F` or none, from
  WID-FOC-005.
- **Behavior:**
  1. Let the point `P` be on `S`, at the centre of `S` on the other axis:
     - the coordinate on the axis of `D` is `S.y` for Down and Up, and `S.x` for Right and
       Left;
     - the centre on the other axis is `(lo + hi) / 2`, with `lo` the first unit and
       `hi = lo + length − 1` the last unit of `S` on that axis.
  2. For each widget other than `F`, let the point `Q` be its near side and its centre:
     - the near coordinate is `C.y` for Down, `C.y + C.h − 1` for Up, `C.x` for Right and
       `C.x + C.w − 1` for Left;
     - the centre on the other axis is computed as in step 1, from the bounds `C`.
  3. The widget is a candidate when `Q` is after `P` in the direction `D` on the axis of `D`.
     "After" is greater for Down and Right, and less for Up and Left.
  4. Sort the candidates by the squared distance from `P` to `Q`, the smallest first. On equal
     distances, keep the order of addition.
  5. The first candidate that can take the focus becomes the focused widget. When there is
     none, the focus does not change.
- **Postconditions:** the fallback can select a widget that does not overlap `F` on the other
  axis. An example is a widget down and to the right of `F`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-05.

---

## 4. Common state

### WID-STATE-001: Visible and active

- **Behavior:**
  1. A widget that is not visible does not draw, is never hovered, takes no press, no wheel
     event and no focus.
  2. An inactive widget draws in its inactive form. It takes no press and no activation key.
     It can be hovered (WID-STATE-002), and its tooltip can show.
  3. A screen can change both states of a widget at any time.
- **Postconditions:** an inactive button shows its tooltip under the mouse. For example, a
  screen can explain with the tooltip why the button is inactive.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-04.

### WID-STATE-002: Hovered and highlighted

- **Inputs:** the mouse position (`mx`, `my`), the last input device, the bounds, and the clip
  rectangle of the draw. A scroll area sets a clip rectangle (`client-list.md`). Else the clip
  rectangle is the screen.
- **Behavior:**
  1. A visible widget is hovered in a frame when all these conditions are true:
     - the last input device is mouse;
     - (`mx`, `my`) is in the bounds (WID-PTR-001, with the integer point);
     - (`mx`, `my`) is in the clip rectangle.
  2. A widget is highlighted in a frame in these cases:
     - when the last input device is mouse: the widget is hovered;
     - when the last input device is a keyboard value: the widget is focused;
     - when the last input device is none: never.
- **Postconditions:**
  - The focus shows as the highlighted sprite of the focused widget, only after a key.
  - Two overlapped widgets can both be hovered in one frame.
  - Active or not, the state "hovered" is the same. Only the sprite and the pointer differ.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-05.

### WID-STATE-003: Colour of a caption

- **Inputs:** the caption of a button, an icon button, a cycle button or a slider, its active
  state, and the alpha of the widget.
- **Behavior:**
  1. An active caption draws in white times alpha, with the drop shadow.
  2. An inactive caption draws in `FFA0A0A0` times alpha, with the drop shadow.
  3. A colour of the style of the caption, or of a part of it, has priority over the colours of
     steps 1 and 2.
- **Postconditions:** a caption of plain text is white when active and grey when inactive.
- **Edge cases:** the checkbox and the text widgets do not use step 2 (WID-CHK-002,
  WID-TXT-001).

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-04.

### WID-STATE-004: Shape of the mouse pointer

- **Behavior:** a hovered button, icon button, image button, cycle button, checkbox or slider
  sets the shape of the mouse pointer for the frame:

| State | Shape |
|-------|-------|
| active | a hand |
| active slider, while the screen drags it | a horizontal resize arrow |
| inactive | the "not allowed" shape |

- **Postconditions:** with no such hovered widget, the pointer has its normal shape. A text
  widget does not change the shape.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-04.

---

## 5. Click sound

### WID-SND-001: The click sound

- **Behavior:**
  1. The click sound is the sound event `minecraft:ui.button.click` in the sound source UI.
     The volume is 0.25 and the pitch is 1.0. It has no position in the world.
  2. The click sound plays at these moments:

| Widget | Moment of the click sound |
|--------|---------------------------|
| button, icon button, image button | an accepted press (WID-PTR-002), before the activation; an activation key while focused and active, before the activation |
| cycle button | as a button; not on a wheel event |
| checkbox | as a button |
| slider | the release of the left button that ends a drag of the slider (WID-SLD-002); not on the press, and not on a key |
| text widget | never |

- **Postconditions:** each accepted press plays one click sound, also the second press of a
  double click.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-06,
WID-EXP-07.

---

## 6. Tooltip of a widget

### WID-TIP-001: When the tooltip of a widget shows

- **Inputs:** a visible widget with a tooltip text, its delay `d` in milliseconds (default 0),
  its hovered and focused states, and the last input device.
- **Behavior:**
  1. The widget wants its tooltip in a frame when one of these conditions is true:
     - the widget is hovered;
     - the widget is focused and the last input device is a keyboard value.
  2. The time of the start is the clock of the client, in milliseconds, in the first frame in
     which the widget wants its tooltip.
  3. In a frame in which the widget wants its tooltip, and more than `d` ms passed since the
     start, the widget makes a tooltip request.
  4. When the widget does not want its tooltip in a frame, the next start is a new start.
  5. A widget with no tooltip text makes no request.
- **Postconditions:** with `d` = 0, the tooltip shows from the second frame of the hover, when
  that frame is at least 1 ms later. A screen can give a widget a delay, for example 500 ms.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-08.

### WID-TIP-002: Lines of the tooltip of a widget

- **Inputs:** the tooltip text, a text component.
- **Behavior:**
  1. The lines are the tooltip text, wrapped at the width 170.
  2. The lines change when the language of the client changes.
  3. The content box and the background are as `client-tooltip.md`, TIP-BOX-001, TIP-BOX-002,
     TIP-BG-001 and TIP-BG-002, state. The tooltip of a widget has no gap after the first line.
- **Postconditions:** no line of the tooltip of a widget is wider than 170, except a single
  word that is wider.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-08.

### WID-TIP-003: Position at the mouse

- **Inputs:** the hovered widget with the bounds (`bx`, `by`, `bw`, `bh`), the mouse position
  (`mx`, `my`), W and H. The content box has the width `w` and the height `h`.
- **Behavior:**
  1. Set `x` = `mx + 12`. If `x + w > W − 5`, set `x` = `max(mx − 12 − w, 9)`.
  2. Set `y` = `my + 3`.
  3. Let `off(m, e)` = `round((bh − 3) + (k / bh)·(5 − (bh − 3)))`, with
     `k = min(|m − e|, bh)`. The division `k / bh` is a real division.
  4. If `by + 2·bh + h + 6 ≤ H − 5`, set `y` = `y + off(y, by)`.
  5. Else set `y` = `y − (h + 6 + off(y, by + bh))`.
- **Postconditions:**
  - The content box is at (`x`, `y`). It is below the mouse when there is space below the
    widget, else above the mouse.
  - Example: `bh` = 20 and the mouse is on the top row of the widget. The content box starts
    18 GUI units below the mouse (3 + 15). On the bottom row, it starts 8 below (3 + 5).
- **Edge cases:** the test of step 4 does not depend on the mouse, only on the widget. Example:
  `bh` = 20, `my` = `by` + 10: `y` = `my + 3`, `k` = 13, `off` = `round(17 − 12·13/20)` = 9.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-08.

### WID-TIP-004: Position below or above a focused widget

- **Inputs:** a widget that is focused and not hovered, with the last input device a keyboard
  value. `w`, `h`, W, H and the bounds as in WID-TIP-003.
- **Behavior:**
  1. Set `x` = `bx + 3` and `y` = `by + bh + 4`.
  2. If `y + h + 3 > H`, set `y` = `by − h − 4`.
  3. If `x + w > W`, set `x` = `max(bx + bw − w − 3, 4)`.
- **Postconditions:** the content box starts 4 GUI units below the widget, or ends 4 GUI units
  above it. Its left edge is 3 GUI units right of the left edge of the widget.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-08.

### WID-TIP-005: Tooltip of a widget and other tooltip requests

- **Behavior:**
  1. A widget makes its request while it draws, in the order of WID-SCR-007.
  2. A request of a focused widget replaces a request that the frame already has. This is an
     exception to `client-tooltip.md`, TIP-REQ-001, step 4.
  3. A request of a widget that is not focused obeys TIP-REQ-001, step 4: the first request
     stays.
  4. The tooltip of a widget uses the position of WID-TIP-003 or WID-TIP-004, not TIP-POS-001.
- **Postconditions:** while the player moves the focus with keys and the mouse is on another
  widget, the tooltip of the focused widget shows. A key can make a tooltip request through
  the focus.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-08.

---

## 7. Button

### WID-BTN-001: Size of a button

- **Behavior:** a screen gives each button its bounds. These are the standard sizes:

| Name | Width | Height |
|------|-------|--------|
| default | 150 | 20 |
| small | 120 | 20 |
| big | 200 | 20 |

- **Postconditions:** a button with no stated size is 150 × 20.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-04.

### WID-BTN-002: Sprites of a button

- **Behavior:** the sprite of a button fills its bounds, times alpha:

| Active | Highlighted | Sprite |
|--------|-------------|--------|
| yes | no | `widget/button` |
| yes | yes | `widget/button_highlighted` |
| no | yes or no | `widget/button_disabled` |

- **Postconditions:** in the vanilla resource pack, each of the three sprites is 200 × 20 with
  the scaling `nine_slice`. The border is 3 for `widget/button` and
  `widget/button_highlighted`, and 1 for `widget/button_disabled`.

Provenance: `source_suggested_black_box_confirmed`: the sprites and their `.mcmeta` files are
in `assets/minecraft/textures/gui/sprites/widget/` of the client jar. Kind: layout.
Confidence: medium. Experiment: WID-EXP-04.

### WID-BTN-003: Activation of a button

- **Inputs:** an accepted press (WID-PTR-002), or an activation key while the button is focused
  and active.
- **Behavior:**
  1. The click sound plays.
  2. Then the action of the button runs. The action knows whether Shift is down.
- **Postconditions:** a key other than an activation key does nothing on a button. An
  inactive button takes no key.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-06.

### WID-BTN-004: Caption of a button

- **Inputs:** the bounds (`x`, `y`, `w`, `h`), the caption with the font width `tw`.
- **Behavior:**
  1. The label area goes from `left` = `x + 2` to `right` = `x + w − 2`, and from `top` = `y`
     to `bottom` = `y + h`. Its width is `aw` = `right − left`.
  2. The top of the text is `(top + bottom − 9) / 2 + 1`.
  3. When `tw ≤ aw`, let `c` = `clamp((left + right) / 2, left + tw / 2, right − tw / 2)`. The
     text starts at `c − tw / 2`.
  4. When `tw > aw`, the caption moves as WID-BTN-005 states.
  5. The colour is as WID-STATE-003 states.
- **Postconditions:** a button of 150 × 20 at (0, 0) with a caption of width 40 draws the text
  at (55, 6).

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-04.

### WID-BTN-005: Label scroll

- **Inputs:** a label area (`left`, `top`, `right`, `bottom`) of width `aw`, a caption of
  width `tw > aw`, and the clock of the client in seconds `t`.
- **Behavior:**
  1. Let `m` = `tw − aw`, and the period `p` = `max(m · 0.5, 3.0)` seconds.
  2. Let `a` = `sin(π/2 · cos(2π · t / p)) / 2 + 0.5`.
  3. The text starts at `left − trunc(a · m)`, at the top of WID-BTN-004, step 2.
  4. The text draws only inside the label area. The part outside is cut.
- **Postconditions:**
  - With `a` = 0 the start of the text shows. With `a` = 1 the end of the text shows.
  - The text moves from one end to the other and back in each period, slowly at each end.
  - Each label scroll of the client uses the same clock, so two captions move together.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-09.

---

## 8. Icon and image buttons

### WID-ICON-001: Icon button

- **Inputs:** a button with a caption and an icon sprite of `iw` × `ih` GUI units. The button
  has one of these three forms:
  - icon only;
  - text and icon at the right;
  - text and icon after the text.
- **Behavior:**
  1. The background is the sprite of a button (WID-BTN-002). The icon draws over it, times
     alpha. The icon sprite is the same in each state.
  2. Icon only: the icon draws at (`x + w / 2 − iw / 2`, `y + h / 2 − ih / 2`). The caption
     does not draw.
  3. The two text forms: the label area goes from `x + 2` to `x + w − iw − 4`. The centre `c`
     of WID-BTN-004, step 3, starts from `x + w / 2` and not from the centre of the area. The
     label scroll of WID-BTN-005 applies in that area.
  4. Text and icon at the right: the icon draws at (`x + w − iw − 2`, `y + h / 2 − ih / 2`).
  5. Text and icon after the text: when `tw < aw + 2`, the icon draws at
     `x + w / 2 + tw / 2 + 4`. Else it draws as in step 4. The top is as in step 4.
- **Postconditions:** an icon only button of 20 × 20 with an icon of 15 × 15 draws the icon at
  (`x + 3`, `y + 3`).
- **Edge cases:** the vanilla resource pack has the icons `icon/language` and
  `icon/accessibility`, each 15 × 15. `client-menu.md` states the buttons that use them.

Provenance: `source_suggested`; `black_box_confirmed` for the size of the two icons in
`assets/minecraft/textures/gui/sprites/icon/` of the client jar. Kind: layout. Confidence:
medium. Experiment: WID-EXP-10.

### WID-ICON-002: Image button

- **Inputs:** a button with four sprites: active, active highlighted, inactive, inactive
  highlighted. It has no caption that draws.
- **Behavior:**
  1. The sprite of the state fills the bounds, times alpha, with the scaling of that sprite.
  2. The activation and the click sound are as for a button (WID-BTN-003).
- **Postconditions:** a screen that gives the same sprite for both inactive states shows no
  highlight on an inactive image button.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-10.

---

## 9. Cycle button

### WID-CYC-001: Values and caption of a cycle button

- **Inputs:** a name `N`, a list of values with an index, a text for each value, and one of
  three caption forms.
- **Behavior:**
  1. The caption form "name and value" shows `options.generic_value` with `N` and the text of
     the value. In English that is `"%s: %s"`.
  2. The caption form "value" shows only the text of the value.
  3. The caption form "hidden" draws no caption.
  4. A Boolean cycle button has the values true, then false. Their texts are `options.on`
     ("ON") and `options.off` ("OFF"), unless the screen gives other texts.
  5. The default size is 150 × 20. The sprites and the caption position are as for a button
     (WID-BTN-002, WID-BTN-004).
- **Postconditions:** a Boolean cycle button with the name "Cape" shows `Cape: ON` in English.

Provenance: `source_suggested_black_box_confirmed`: the keys and the English texts are in
`assets/minecraft/lang/en_us.json` of the client jar. Kind: layout. Confidence: medium.
Experiment: WID-EXP-06.

### WID-CYC-002: Input of a cycle button

- **Inputs:** an activation (an accepted press or an activation key), the state of Shift, or a
  wheel event with the amount down `s`.
- **Behavior:**
  1. An activation with no Shift selects the next value: the index becomes
     `(index + 1) mod n`. The click sound plays first.
  2. An activation with Shift selects the previous value: `(index − 1) mod n`, a value from
     0 to `n − 1`. The click sound plays first.
  3. A wheel event with `s > 0` (the wheel up) selects the previous value. A wheel event with
     `s < 0` selects the next value. A wheel event plays no sound.
  4. After each change, the caption and the tooltip show the new value. The screen gets the
     new value at once.
- **Postconditions:** a press of the right button has no effect (WID-PTR-002).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-06.

### WID-CYC-003: Tooltip and sprite of a value

- **Behavior:**
  1. A screen can give a tooltip text for each value. The tooltip of the cycle button is the
     tooltip text of its value, or none.
  2. A screen can give a sprite for each value. That sprite fills the bounds in place of the
     sprite of a button.
- **Postconditions:** the tooltip changes in the same frame as the value.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-06.

---

## 10. Slider

### WID-SLD-001: Draw of a slider

- **Inputs:** the bounds (`x`, `y`, `w`, `h`), the position `v` from 0 to 1, the state, and
  the change mode.
- **Behavior:**
  1. The bar fills the bounds, times alpha. The bar sprite is `widget/slider_highlighted` when
     the slider is active, focused, not in change mode, and the last input device is a
     keyboard value. Else it is `widget/slider`.
  2. The handle is 8 GUI units wide and `h` high, at (`x + trunc(v · (w − 8))`, `y`).
  3. The handle sprite is `widget/slider_handle_highlighted` when the slider is active and one
     of these conditions is true. Else it is `widget/slider_handle`.
     - the slider is hovered;
     - the slider is in change mode and the last input device is a keyboard value.
  4. The caption draws as for a button (WID-BTN-004, WID-STATE-003).
- **Postconditions:**
  - A slider focused by keys and not in change mode shows a highlighted bar.
  - In change mode it shows a highlighted handle.
- **Edge cases:** in the vanilla resource pack, `widget/slider` and
  `widget/slider_highlighted` are 200 × 20, `nine_slice` with the border 1.
  `widget/slider_handle` and `widget/slider_handle_highlighted` are 8 × 20, `nine_slice` with
  the border left 2, top 2, right 2 and bottom 3.

Provenance: `source_suggested_black_box_confirmed`: the sprites and their `.mcmeta` files are
in `assets/minecraft/textures/gui/sprites/widget/` of the client jar. Kind: layout.
Confidence: medium. Experiment: WID-EXP-07.

### WID-SLD-002: Mouse on a slider

- **Inputs:** a press, a drag and a release of the left button, at the event position `ex`.
- **Behavior:**
  1. An accepted press sets the position to `clamp((ex − (x + 4)) / (w − 8), 0, 1)`. The
     press plays no sound. The slider is focused and the screen drags it (WID-PTR-002).
  2. Each drag move sets the position in the same way, also outside the bounds.
  3. The release of the left button ends the drag and plays the click sound. The sound plays
     also when the mouse is outside the slider.
  4. Each change of the position is a change of the value (WID-SLD-004).
- **Postconditions:** a press at the centre of the handle keeps the position. The ends of the
  slider are 4 GUI units inside its bounds.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-07.

### WID-SLD-003: Keys on a slider

- **Inputs:** a key while the slider is focused, the change mode, and the last input device.
- **Behavior:**
  1. When the slider takes the focus, the change mode becomes on if the last input device is
     mouse or Tab. Else the change mode keeps its value. When the slider loses the focus, the
     change mode becomes off.
  2. An activation key turns the change mode on or off. It plays no sound and does not change
     the value.
  3. In change mode, Left subtracts `1 / (w − 8)` from the position, and Right adds it. The
     key does not move the focus.
  4. When the change mode is off, Left and Right do arrow navigation (WID-FOC-005). Up and Down
     always do arrow navigation.
- **Postconditions:** a slider that the player reaches with an arrow key does not take Left
  and Right until the player presses an activation key.
- **Edge cases:** an option slider replaces step 3 with one step of its value (WID-SLD-005).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-07.

### WID-SLD-004: Value and caption of a slider

- **Behavior:**
  1. A new position is clamped to [0, 1].
  2. When the clamped position differs from the old position, the slider applies the value.
     When it is the same, nothing applies.
  3. In both cases the caption updates from the value.
- **Postconditions:** a drag past the end does not apply the value again.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-07.

### WID-SLD-005: Option slider over an integer range

- **Inputs:** an option with the integer range [`lo`, `hi`], `lo < hi`, and its value `n`.
- **Behavior:**
  1. The position of `n` is 0 when `n = lo`, 1 when `n = hi`, else
     `(n + 0.5 − lo) / (hi + 1 − lo)`.
  2. The value of a position `v` is `floor(lo + v' · (hi + 1 − lo))`. `v'` is `v`, except
     when `v ≥ 1`: then `v'` is 0.99999 as a 32-bit real (0.99998998641968).
  3. In change mode, Left sets the value `n − 1` and Right sets `n + 1`. The position becomes
     the position of that value. Then the value of WID-SLD-004 applies with the clamp.
  4. The caption and the tooltip are the text that the option gives for its value.
- **Postconditions:** each value has an equal part of the bar. The handle of the value 70 in
  the range [30, 110] is at the position 0.5.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-07.

### WID-SLD-006: Option slider over a list of values

- **Inputs:** an option with a list of `k` values in order, and the index `i` of its value.
  An option with a real value from 0 to 1 uses the position as the value.
- **Behavior:**
  1. The value of a position `v` is the index `floor(v' · k)`, clamped to [0, `k − 1`], with
     `v'` as in WID-SLD-005.
  2. The position of the index `i` is `i / (k − 1)` as a real division, 0 for the first value
     and 1 for the last value.
- **Postconditions:** the first and the last value put the handle at the ends of the bar.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-07.

### WID-SLD-007: When an option slider applies its value

- **Inputs:** an option slider. The option states if it applies at once. Most options do.
- **Behavior:**
  1. An option that applies at once stores the value at each change. At the release of the
     drag, the handle moves to the position of the stored value.
  2. Another option waits. It stores the value in the first frame at least 600 ms after the
     last change. Then the handle moves to the position of the stored value.
  3. The caption shows the new value at once in both cases.
- **Postconditions:** on an integer slider, the handle jumps to the centre of the part of its
  value when the drag ends.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-07.
In mcrs, the option Render Distance applies at once (`client-menu.md`, MENU-VAL-007, step 6).
`client-menu.md`, MENU-DEV-014, states that deviation and when the client sends the value.

---

## 11. The checkbox

### WID-CHK-001: Size of a checkbox

- **Inputs:** the label, a text component with the font width `tw`, and an optional maximum
  width `mw`.
- **Behavior:**
  1. The box is 17 × 17. That is the line height 9 plus 8.
  2. The width is `17 + 4 + tw`. With `mw`, it is `min(17 + 4 + tw, mw)`.
  3. The label is a multi-line text widget (WID-TXT-002) with at most 2 rows. It wraps at the
     width of the checkbox.
  4. The height is `max(17, 9 · lines)`, with `lines` the number of label lines.
- **Postconditions:** a checkbox with a short label is `21 + tw` wide and 17 high.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-11.

### WID-CHK-002: Draw of a checkbox

- **Behavior:**
  1. The box sprite draws at (`x`, `y`) with the size 17 × 17, times alpha:

| Selected | Highlighted | Sprite |
|----------|-------------|--------|
| no | no | `widget/checkbox` |
| no | yes | `widget/checkbox_highlighted` |
| yes | no | `widget/checkbox_selected` |
| yes | yes | `widget/checkbox_selected_highlighted` |

  2. The active state does not change the sprite.
  3. The label draws at (`x + 21`, `y + 17 / 2 − lh / 2`), with `lh` the height of the label.
  4. The label is white times alpha, with the drop shadow. An inactive checkbox does not make
     the label grey.
- **Postconditions:** a label of one line draws at `y + 4`. A label of two lines draws at
  `y − 1`.
- **Edge cases:** in the vanilla resource pack, the four sprites are 20 × 20 pixels with the
  scaling `stretch`. They stretch to 17 × 17.

Provenance: `source_suggested_black_box_confirmed`: the four sprites are in
`assets/minecraft/textures/gui/sprites/widget/` of the client jar, with no `.mcmeta`. Kind:
layout. Confidence: medium. Experiment: WID-EXP-11.

### WID-CHK-003: Toggle and tooltip of a checkbox

- **Behavior:**
  1. An activation plays the click sound, then inverts the selected state. The screen gets the
     new state at once.
  2. A screen can give a checkbox a tooltip text. The checkbox keeps it only when its label,
     wrapped at the width of the checkbox, has more than 2 lines. Else the checkbox has no
     tooltip.
  3. When the label has more than 2 lines, the second line ends with `...` (WID-TXT-002).
- **Postconditions:** a checkbox with a cut label shows the given tooltip text under the mouse.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: WID-EXP-11.

---

## 12. Text widgets

### WID-TXT-001: One-line text widget

- **Inputs:** a text component `t` with the font width `tw`. Optional: the bounds that a screen
  gives, a maximum width `mw` > 0, and the overflow mode "cut" (the default) or "scroll".
- **Behavior:**
  1. The default size is `tw` × 9.
  2. With `mw`, the width is `min(mw, tw)`.
  3. When the screen changes `t`, the width becomes the width of the new text. Step 2 still
     applies.
  4. The text starts at (`x`, `y + (h − 9) / 2`). It is white times alpha, with the drop
     shadow. A colour of the style of `t` has priority.
  5. Let the limit be `mw`, or `w` when there is no `mw`. When `tw` is more than the limit,
     the overflow mode applies:
     - "cut": draw the longest start of `t` that is not wider than the limit minus the width of
       `...`, then `...`;
     - "scroll": the label scroll of WID-BTN-005, with the label area from `x + 2` to
       `x + w − 2` and from `y` to `y + h`.
  6. The widget is not active. While it is hovered, the hover events of its text styles show
     (`client-tooltip.md`, TIP-HOVER-001).
- **Postconditions:** a text widget aligns only through its cell (WID-LAY-002). The text always
  starts at the left edge of the widget.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-12.

### WID-TXT-002: Multi-line text widget

- **Inputs:** a text component `t`, an optional maximum width `mw` (default: no limit), an
  optional maximum of rows `mr`, and a flag "centred".
- **Behavior:**
  1. Wrap `t` at `mw` into lines.
  2. With `mr`, keep the first `mr` lines. When lines are dropped, cut the last kept line to
     the longest start that is not wider than its width minus the width of `...`. Then add
     `...` with the style of `t`.
  3. The width is the largest line width. The height is 9 times the number of lines.
  4. Line `i`, from 0, draws at the top `y + 9·i`.
  5. With "centred", line `i` starts at `x + width / 2 − wi / 2`, with `wi` its width. Else it
     starts at `x`.
  6. The colour and the hover events are as WID-TXT-001 states. The widget is not active.
- **Postconditions:** an empty text has no line and the height 0.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-12.

---

## 13. Layout

### WID-LAY-001: Layout elements and cells

- **Behavior:**
  1. A layout element has a position (`x`, `y`) and a size (`w`, `h`) in GUI units.
  2. A layout arranges its children. A layout first arranges each child that is a layout. Then
     it places its children and computes its own size.
  3. The size of a cell is the size of its child plus its padding:
     `w + pl + pr` × `h + pt + pb`.
  4. The default cell has the padding 0 and the alignment (0, 0). In a frame, the default
     alignment is (0.5, 0.5).
  5. A widget that is not visible keeps its cell.
  6. When a layout moves after it arranged its children, each child moves by the same amount.
- **Postconditions:** a screen can arrange a layout, then align it in the screen
  (WID-LAY-003). The children follow.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-13.

### WID-LAY-002: Placement of a child in its cell

- **Inputs:** a child of size (`w`, `h`), and its cell with the padding (`pl`, `pt`, `pr`,
  `pb`) and the alignment (`ax`, `ay`). The layout gives a start (`sx`, `sy`) and an available
  size (`aw`, `ah`).
- **Behavior:**
  1. `x` = `sx + trunc(lerp(ax, pl, aw − w − pr))`.
  2. `y` = `sy + round(lerp(ay, pt, ah − h − pb))`.
- **Postconditions:** with an odd free space and the alignment 0.5, `x` rounds down and `y`
  rounds up. Example: `aw − w` = 13 gives the offset 6 across; `ah − h` = 13 gives the offset 7
  down.
- **Edge cases:** when the child is larger than the available size, the offset is negative.
  `trunc` and `round` then round toward 0 and up.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-13.

### WID-LAY-003: Align an element in a rectangle

- **Inputs:** a layout element of size (`w`, `h`), a rectangle (`rx`, `ry`, `rw`, `rh`), and an
  alignment (`ax`, `ay`). To centre is the alignment (0.5, 0.5).
- **Behavior:**
  1. `x` = `rx + trunc(lerp(ax, 0, rw − w))`.
  2. `y` = `ry + trunc(lerp(ay, 0, rh − h))`.
- **Postconditions:** this alignment uses `trunc` on both axes, unlike WID-LAY-002. Example: to
  centre 316 × 120 in 427 × 240 gives (55, 60).

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-13.

### WID-LAY-004: Grid

- **Inputs:** the position (`gx`, `gy`), the row gap `gr`, the column gap `gc` (default 0 each),
  and children. Each child has a row `r`, a column `c`, a row span `rs` ≥ 1, a column span
  `cs` ≥ 1, and a cell.
- **Behavior:**
  1. The grid has `R` rows and `C` columns. `R − 1` is the largest `r + rs − 1` of the
     children. `C − 1` is the largest `c + cs − 1`.
  2. For each child, split its cell height minus `(rs − 1)·gr` into `rs` parts (even split).
     Part `i` goes to the row `r + i − 1`.
  3. The height of a row is the largest part in that row, or 0.
  4. The width of a column is computed as in steps 2 and 3, with the cell width, `cs` and `gc`.
  5. The top of row 0 is 0. The top of row `i` is the top of row `i − 1`, plus its height,
     plus `gr`. The left of each column is computed in the same way with `gc`.
  6. The available width of a child is the sum of the widths of its columns plus
     `gc·(cs − 1)`. The available height is computed in the same way.
  7. Place each child by WID-LAY-002, from (`gx` + the left of `c`, `gy` + the top of `r`),
     in its available size.
  8. The width of the grid is the left of column `C − 1` plus its width. The height is the top
     of row `R − 1` plus its height.
- **Postconditions:** a grid with no child has the size 0 × 0. A row with no child adds only
  `gr` to the height.
- **Example:** two columns of buttons of 150 × 20, each cell with the padding left 4, right 4,
  bottom 4, and `ax` = 0.5. The gaps are 0. Each column is 158 wide and each row 24 high. The buttons of a row
  are at the offsets 4 and 162. Five rows give the size 316 × 120.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-13.

### WID-LAY-005: Add children in rows

- **Inputs:** a grid, a number of columns `n` ≥ 1, and children that a screen adds one by one.
  Each child has a column span `s` (default 1). The index `j` starts at 0.
- **Behavior:**
  1. The child goes to the row `j / n` and the column `j % n`, with the row span 1.
  2. When `j % n + s > n`, the child goes to the next row at column 0. First, `j` becomes the
     next multiple of `n` that is ≥ `j`.
  3. After each child, `j` increases by `s`.
- **Postconditions:** with `n` = 2, a first child with `s` = 2 fills row 0. The next two
  children fill row 1.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-13.

### WID-LAY-006: Row and column

- **Inputs:** an orientation (row or column), a gap `g`, and children in the order of
  addition.
- **Behavior:**
  1. A row is a grid (WID-LAY-004) with one row. Its children go to the columns 0, 1, 2, and
     so on, in the order of addition. The column gap is `g`.
  2. A column is a grid with one column. Its children go to the rows 0, 1, 2, and so on. The
     row gap is `g`.
- **Postconditions:** a row of two cells of widths 150 with `g` = 8 is 308 wide. Its height is
  the largest cell height.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-13.

### WID-LAY-007: Frame

- **Inputs:** a minimum width `mw` and a minimum height `mh` (default 0), and children.
- **Behavior:**
  1. The width of the frame is the largest of `mw` and the cell widths. The height is the
     largest of `mh` and the cell heights.
  2. Place each child by WID-LAY-002, from the position of the frame, in the full size of the
     frame.
- **Postconditions:** the children of a frame overlap. With the default alignment, each child
  is at the centre of the frame.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-13.

### WID-LAY-008: Equal gaps

- **Inputs:** an orientation (row or column), a fixed length `L` on that axis, a size `M` on
  the other axis, and `n` children.
- **Behavior:**
  1. Let `free` be `L` minus the sum of the cell lengths on the axis.
  2. With `n` ≥ 2, split `free` into `n − 1` gaps (even split).
  3. The first cell starts at the start of the layout. Each next cell starts at the end of the
     previous cell plus the next gap.
  4. On the axis, a child starts at the start of its cell plus its first padding (`pl` or
     `pt`). Its alignment on the axis has no effect.
  5. On the other axis, the size is the largest of `M` and the cell sizes. Place each child by
     WID-LAY-002 in that size.
  6. The layout keeps `L` on its axis. Its size on the other axis is the value of step 5.
- **Postconditions:** when `free` ≥ 0, the last cell ends at `L`.
- **Edge cases:** when `free` < 0, the cells overlap.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-13.

### WID-LAY-009: Spacer

- **Behavior:** a spacer is a layout element with a fixed width and a fixed height. It does not
  draw and takes no input. A spacer of a width has the height 0. A spacer of a height has the
  width 0.
- **Postconditions:** in a row, a spacer of the width 10 adds 10 plus one gap to the width.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-13.

### WID-LAY-010: Element with a label

- **Behavior:** an element with a label is a column with the gap 4. Its first child is a
  one-line text widget with the label. Its second child is the element. The default cell
  applies to both.
- **Postconditions:** the top of the element is 9 + 4 = 13 GUI units below the top of the
  label.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-13.

### WID-LAY-011: Header, content and footer

- **Inputs:** W and H, the header height `hh` (default 33), the footer height `fh` (default
  33), and the content top margin `cm` (default 30). Each of the three parts has children.
- **Behavior:**
  1. The header is a frame (WID-LAY-007) with the minimum size W × `hh`, at (0, 0).
  2. The footer is a frame with the minimum size W × `fh`. Its top is at `H − fh`.
  3. The content is a frame with the minimum width W and the minimum height 0. Let `ch` be its
     height.
  4. The top of the content is `min(hh + cm, H − fh − ch)`. Its left is 0.
  5. In the three frames, the default alignment of a child is (0.5, 0.5).
  6. The title of such a screen is a one-line text widget in the header.
- **Postconditions:**
  - The title is at (`trunc(0.5·(W − tw))`, 12) for the default header, with `tw` its width.
  - A button of 200 × 20 alone in the default footer is at (`trunc(0.5·(W − 200))`,
    `H − 26`).
  - At W = 427 and H = 240 with the default heights, a content child of 316 × 120 is at
    (55, 63). With `hh` = 61, it is at (55, 87).
- **Edge cases:**
  - When the content is high, its top can be above `hh`. There is no limit at the top.
  - A header child higher than `hh` makes the header frame higher. The content still uses `hh`.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-14.

### WID-LAY-012: Standard sizes

- **Behavior:** these values are the standard sizes of the vanilla menus, in GUI units:

| Value | Size |
|-------|------|
| button height | 20 |
| default, small and big button width | 150, 120, 200 |
| width of the Done button of a footer | 200 |
| standard gap between buttons | 8 |
| gap of an element with a label | 4 |
| header and footer height | 33 |
| content top margin | 30 |
| margin of a label area in a button | 2 |
| width of a slider handle | 8 |
| box of a checkbox, gap to its label | 17, 4 |
| wrap width of the tooltip of a widget | 170 |
| line height | 9 |

- **Postconditions:** `client-menu.md` states the sizes of each menu, with these values.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: WID-EXP-14.

---

## 14. Experiments

None of these experiments ran. The owner works on the only machine with the game, so no game
window started for this document. Each experiment uses this setup:

1. Start the vanilla client of the pinned version with the input agent of `tools/vanilla-client`.
2. Set the window to 854 × 480 pixels and the GUI scale to 2. Then W = 427 and H = 240.
3. Record each step with `tools/mcrs_minecraft_client_recorder`: the screenshot, and the sound
   events with their volume and pitch.
4. Compare with a capture of the same version only (`README.md`, section "Experiments").

### WID-EXP-01: Open and close

1. In a singleplayer world, press Escape. Record the screen and its focused widget.
2. Open "Options...", then press Escape. Record the screen.
3. Press Escape again. Record that the game takes the mouse pointer again.
4. On the title screen, open "Options...", then press Escape. Record that the title screen
   shows.

Decides: WID-SCR-001, WID-SCR-002.

### WID-EXP-02: Resize

1. Open the "Music & Sound Options" screen. Press Tab two times. Record the focused widget.
2. Change the GUI scale to 3 with the window open, then back to 2. Record the focused widget.
3. Do steps 1 and 2 on the pause screen. Record the focused widget.

Decides: WID-SCR-003.

### WID-EXP-03: Background

1. In a world, open the pause screen with the blur option at 0. Record a screenshot.
2. Do step 1 with the blur option at 5 and at 10, and at the GUI scale 1 and 4.
3. Open a screen of `client-menu.md` that draws separators and shows no list. Record the rows
   of pixels at y = 31, 32, 33 and at y = H − 33, H − 32 in GUI units.
4. Do step 3 on the "Video Settings" screen, which shows a list. Record that each separator
   row has the same pixels as in step 3.
5. Open the inventory. Record the gradient at the top and the bottom rows.

Decides: WID-SCR-004, WID-SCR-005, WID-SCR-006.

### WID-EXP-04: Press, bounds and state

1. Open "Options...". Move the mouse to the left edge of the "Done" button, then one GUI unit
   to the left. Record the sprite and the pointer shape at each point.
2. Press the right button on "Done". Record no sound and no change of screen.
3. In a world with telemetry off, put the mouse on the inactive "Telemetry Data..." button.
   Record the grey caption, the sprite, the pointer shape and the tooltip.
4. Record the position of the caption of each button.

Decides: WID-PTR-001, WID-PTR-002, WID-STATE-001, WID-STATE-003, WID-STATE-004, WID-BTN-001,
WID-BTN-002, WID-BTN-004, WID-SCR-007.

### WID-EXP-05: Focus navigation

1. From the title screen, open "Options..." with Tab and Enter. Record the focused widget.
2. Press Tab ten times, then Shift+Tab two times. Record the focused widget after each key.
3. Press each arrow key from three start widgets. Record the focused widget after each key.
4. Move the mouse 5 GUI units, then 8 GUI units. Record when the highlight follows the mouse.
5. Open "Options..." with the mouse. Press Down. Record the focused widget.

Decides: WID-IN-001, WID-FOC-001 to WID-FOC-006, WID-STATE-002.

### WID-EXP-06: Button and cycle button

1. Open "Skin Customization". Press the left button on "Cape: ON". Record the caption and the
   sound.
2. Press the left button with Shift. Record the caption.
3. Turn the wheel up one step and down one step on the button. Record the caption and no
   sound.
4. Focus a button with Tab, press Enter and Space. Record the sound before the screen change.

Decides: WID-BTN-003, WID-CYC-001, WID-CYC-002, WID-CYC-003, WID-SND-001, WID-PTR-004.

### WID-EXP-07: Slider

1. Open "Options...". Record the FOV slider at the value 70: the bar, the handle at
   `x + 71`, and the caption.
2. Press the left button 10 GUI units right of the handle. Record no sound and the caption.
3. Drag to the right past the end, then release outside the slider. Record the sound at the
   release and the snap of the handle.
4. Focus the slider with Tab. Press Right two times. Record the value after each key.
5. Press Enter, then Right. Record the focus change and the sprites.
6. Reach the slider with an arrow key. Press Right. Record the focus change.

Decides: WID-SLD-001 to WID-SLD-007, WID-PTR-003.

### WID-EXP-08: Tooltip of a widget

1. Put the mouse on a button with a tooltip at its top edge, its centre and its bottom edge.
   Record the position of the content box.
2. Do step 1 on a button near the bottom of the screen.
3. Focus the button with Tab, with the mouse on another widget. Record the position.
4. Record the frame in which the tooltip shows first, and the line breaks of a long tooltip.

Decides: WID-TIP-001 to WID-TIP-005.

### WID-EXP-09: Label scroll

1. Set the language to one with long captions, or use a resource pack with a long caption for
   "Done".
2. Record the screenshots over 20 seconds at 10 frames in each second.
3. Compare the start of the text with WID-BTN-005.

Decides: WID-BTN-005.

### WID-EXP-10: Icon buttons

1. On the title screen, record the language button and the accessibility button.
2. Record the position of each icon and its sprite in each state.

Decides: WID-ICON-001, WID-ICON-002.

### WID-EXP-11: The checkbox

1. Open a screen with a checkbox, for example the screen to create a world.
2. Record the box, the label position and the sprite in the four states.
3. Use a resource pack that makes the label long. Record the lines and the tooltip.

Decides: WID-CHK-001 to WID-CHK-003.

### WID-EXP-12: Text widgets

1. Record the title of the pause screen and of "Options...".
2. Use a resource pack that makes a title longer than its widget. Record the cut and `...`.

Decides: WID-TXT-001, WID-TXT-002.

### WID-EXP-13: Layout arithmetic

1. Open "Options...". Record the bounds of each button of the grid and of the header.
2. Compare the bounds with WID-LAY-004 and WID-LAY-011. Expect these values:
   - the grid of 316 × 120 at (55, 87);
   - the buttons of the grid at x = 59 and x = 217;
   - the FOV slider at (59, 29).
3. Open the pause screen. Record the bounds of each button.

Decides: WID-LAY-001 to WID-LAY-010.

### WID-EXP-14: Header and footer

1. Open "Music & Sound Options". Record the title position and the "Done" button at
   (113, 214, 200, 20).
2. Do it at H = 200 with a window of 854 × 400 pixels. Record the top of the content.

Decides: WID-LAY-011, WID-LAY-012.

---

## 15. Intentional deviations

No deviation is recorded.

---

## 16. Unresolved

1. A checkbox keeps a given tooltip text only when its label needs more than 2 lines
   (WID-CHK-003). Is that the intent, or a defect that the owner wants to correct?
2. The label of a checkbox wraps at the full width of the checkbox, but it starts 21 GUI units
   right of the box (WID-CHK-001). The label can go 21 GUI units past the bounds. Is this a
   defect?
3. An inactive checkbox has the same sprite and a white label (WID-CHK-002). Is that the
   intent?
4. A cycle button can use a second list of values while Alt is down. This document does not
   state it. Does a menu of mcrs need it?
5. The list of in-game screens, of screens with their own background, and of option sliders
   that wait 600 ms belongs to `client-menu.md`.
6. The tab order groups that the vanilla screens use are not stated. Each widget of this
   document uses the group 0.
