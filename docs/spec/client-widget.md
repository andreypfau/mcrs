# Screens and widgets of the client GUI: behavioural specification

| Field | Value |
|-------|-------|
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
- **In a world**: the client shows a world, in singleplayer or in multiplayer.
- **Screen**: the GUI state that covers the window and takes the input of the mouse and the
  keyboard. It draws a background and a list of widgets. At most one screen is open.
- **Parent screen**: the screen that a screen opens when it closes. A screen can have no parent
  screen.
- **In-game screen**: a screen that draws the gradient of WID-SCR-004, step 2, in place of the
  panorama, the blur and the menu background texture. WID-SCR-008 lists the in-game screens.
- **Default background**: the background of WID-SCR-004, step 3.
- **HUD**: the parts of the GUI that the client draws over the world when no screen is open,
  for example the hotbar, the health and the chat lines.
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
- **Multi-line edit box**: a widget that holds text with line breaks that the player can
  change (section 14). In section 14, "the box" is a multi-line edit box.
- **Value**, **length**, **position**, **`value[a..b]`**, **cursor**, **anchor**,
  **selection**, **head**, **edit modifier**, **shortcut**, **focus time**, **double click**,
  **character**, **code unit**: as `client-edit-box.md`, section 0.1, defines them. The value
  of a multi-line edit box can hold U+000A. Its length is in code units.
- **Display line**: a part of the value of the box that draws as one row (WID-MLE-002). Display
  line `i`, from 0, begins at the position `begin(i)` and ends at `end(i)`. `n` is the number
  of display lines.
- **Character limit** `m`, **line limit** `k`: the optional limits of the box (WID-MLE-001).
- **Scroll offset** `s`, **content height** `C`, **maximum offset** `M`: as `client-list.md`
  defines them.
- **Test box**: in an experiment, the multi-line edit box of a dialog `text` input with
  `multiline`, `width` 108 and `max_length` 100 (`dialog.md`, DLG-CTL-005). The experiment
  states the other fields. Its wrap width is 100.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". No experiment ran.

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
| `WID-MLE` | the multi-line edit box |
| `WID-DEV` | the intentional deviations |

### 0.5 Scope

In scope:

- the model of a screen and its widgets;
- the pointer input, the keys and the focus;
- the common state of a widget, the click sound and the tooltip of a widget;
- the button, the icon button, the image button, the cycle button, the slider and the checkbox;
- the text widgets;
- the layouts: row, column, grid, frame, equal gaps, spacer, and header, content and footer;
- the multi-line edit box.

Out of scope, with the document that states it:

- the one-line text input: `client-edit-box.md`;
- the scroll area, the scroll bar, the lists and the tab bar: `client-list.md`;
- the pause screen, the options screens and the other menus: `client-menu.md`;
- the narrator and the other accessibility features;
- the text of an input method that is not yet committed, in each edit box;
- the title screen and the panorama: `client-title-screen.md`;
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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### WID-SCR-004: Background of a screen

- **Inputs:** the screen, W, H, and whether the client is in a world.
- **Behavior:**
  1. The kind of the screen selects its background. WID-SCR-008 states the background of each
     screen. Whether the client is in a world changes only the parts of step 3 and the texture
     of step 4.
  2. An in-game screen fills (0, 0, W, H) with a vertical gradient. The top colour is
     `C0101010` and the bottom colour is `D0101010`. It draws no panorama, no blur and no menu
     background texture.
  3. Each screen that is not an in-game screen and has no own background in WID-SCR-008 draws
     the default background. It has these parts, in this order:
     1. with no world, the panorama (`client-title-screen.md`, TITLE-PAN-002);
     2. the blur, when WID-SCR-005 asks for it;
     3. the menu background texture over (0, 0, W, H).
  4. The menu background texture is `minecraft:textures/gui/menu_background.png` with no world,
     and `minecraft:textures/gui/inworld_menu_background.png` in a world.
  5. The texture tiles from the point (0, 0). One tile is the full texture over 32 × 32 GUI
     units.
  6. In a world, the client draws the world and then the HUD before the background of the
     screen. Each background of this rule and of WID-SCR-008 draws over them. The exception is
     the level loading screen: the HUD does not draw (WID-SCR-008).
- **Postconditions:**
  - in the vanilla resource pack, each pixel of the two textures is `40000000`. The texture
    darkens the content behind it by the alpha 64/255;
  - in a world, the default background shows the world and the HUD blurred and darkened. With
    no world, it shows the panorama blurred and darkened;
  - with the blur radius 0, the default background shows the world, the HUD or the panorama
    sharp and darkened.

Provenance: `source_suggested`; `black_box_confirmed` for the pixel values of
`assets/minecraft/textures/gui/menu_background.png` and
`assets/minecraft/textures/gui/inworld_menu_background.png` of the client jar (16 × 16 pixels,
each `40000000`). Kind: layout. Confidence: medium.

### WID-SCR-005: Blur behind a screen

- **Inputs:** the blur radius `r`, an integer from 0 to 10. In the game, `r` is the option
  "Menu Background Blur" (`options.accessibility.menu_background_blurriness`). Its default is 5.
  The image `P` is the content that the client drew before the background of the screen, in
  pixels of the window. `P` has the size of the window framebuffer. In a world, `P` holds the
  world and the HUD. With no world, `P` holds the panorama.
- **Behavior:**
  1. When `r` is 0, there is no blur.
  2. When `r` is 1 or more, the client blurs `P`. The parts of the background that the screen
     draws, for example the menu background texture, do not blur. The widgets do not blur.
  3. The blur is six passes, in this order: horizontal, vertical, horizontal, vertical,
     horizontal, vertical. Each pass reads the result of the previous pass. The first pass reads
     `P`. The result of the sixth pass replaces `P`.
  4. Each pass uses the same radius `r`. The radius is in pixels of the window. It does not
     change with the GUI scale or the size of the GUI.
  5. One pass sets each pixel `x` to the mean of the `2r + 1` pixels from `x − r` to `x + r`
     along its direction. Each of these pixels has the weight `1 / (2r + 1)`. The pass does this
     for each of the four channels, alpha included.
  6. A pixel outside the image has the value of the nearest pixel on the edge of the image, in
     the same row or column. Thus a pass near the edge counts the edge pixel more than one time.
- **Postconditions:**
  - one pass is a box filter of width `2r + 1`. Three passes along one axis give the
    convolution of three such boxes. Its width is `6r + 1` pixels, and its weights sum to 1. For
    `r` = 5, one box is 11 pixels and the result of the three passes is 31 pixels wide;
  - the horizontal and the vertical passes act on separate axes. Thus the full blur is the
    product of the two results of one axis;
  - an image of one colour does not change.
- **Edge cases:**
  - the game reads `2r + 1` pixels with `r + 1` samples. Each sample at the offset
    `−r + 0.5 + 2k`, for `k` from 0 to `r − 1`, falls between two pixels. The linear filter of
    the texture gives the mean of the two pixels. The last sample is at the offset `r`, on the
    centre of one pixel, with the weight 1/2. The sum is divided by `r + 0.5`. The result is the
    mean of step 5. An implementation can read the `2r + 1` pixels in a different way, if the
    result is the same;
  - a pass of the vanilla post effect has its own radius value 0. A value of 0.5 or more replaces
    `r` for that pass, rounded to the nearest integer. A resource pack can set it. The vanilla
    pack does not.
- **mcrs:** `r` is the option "Menu Background Blur" of `client-menu.md`, MENU-VAL-013. No
  graphics preset sets `r` (WID-DEV-001).

Provenance: `black_box_confirmed` for the six passes, their order and their directions, by
`assets/minecraft/post_effect/blur.json` of the client jar. `black_box_confirmed` for the sample
offsets, the weights, the division and the linear filter of each input, by
`assets/minecraft/shaders/post/box_blur.fsh` and `blur.json` of the client jar. The mean of step
5 is the arithmetic of these samples. `source_suggested` for the option as the source of `r`, for
the content and the size of `P` and for the edge of step 6. `design` for the absence of a
graphics preset in mcrs. Kind: layout. Confidence: medium.

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
the client jar (32 × 2 pixels each). Kind: layout. Confidence: medium.

### WID-SCR-007: Draw order of a screen

- **Behavior:** in each frame, the screen draws these parts, in this order:
  1. the background (WID-SCR-004);
  2. the visible widgets, in their order of addition to the screen;
  3. the tooltip of the frame (`client-tooltip.md`, TIP-REQ-001, with WID-TIP-005).
- **Postconditions:** a later widget draws over an earlier widget. The tooltip is above all.
- **Edge cases:** where two widgets overlap, the press goes to the earlier widget
  (WID-PTR-002). That is the widget below.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### WID-SCR-008: Background of each screen

- **Inputs:** the open screen, its state, and whether the client is in a world.
- **Behavior:** each screen draws the background of its row. "Default" is the default
  background and "in-game" is the background of an in-game screen (section 0.1). A dash shows
  that the screen does not open in that state.

| Screen | No world | In a world | Rule |
|--------|----------|------------|------|
| Title screen | the panorama and its overlay; no blur, no menu background texture | — | `client-title-screen.md`, TITLE-SCR-005 |
| Options screen, "Video Settings", "Accessibility Settings", "Controls", "Mouse Settings", "Key Binds" and each other options screen | default | default | `client-menu.md`, MENU-PROP-001; `client-controls.md` |
| Select World, confirmation of a delete | default | — | `client-world-screens.md` |
| Create New World | default, and the tab header texture over the header | — | `client-list.md`, TAB-AREA-001, step 6 |
| Error screen of the saves folder | a vertical gradient from `FF402020` to `FF501010`; no panorama, no blur, no menu background texture | — | `client-world-screens.md`, WORLD-LIST-007 |
| Connect screen, disconnected screen | default | — | `client-title-screen.md`, TITLE-PAN-004, step 5 |
| Message screen | the panorama, the blur, `menu_background.png` | the panorama, the blur, `inworld_menu_background.png`; the panorama covers the world | TITLE-PAN-004, step 3 |
| Level loading screen, not for a portal | as the message screen | as the message screen; the HUD does not draw | TITLE-PAN-004, step 4 |
| Pause screen with its menu | — | default | `client-menu.md`, MENU-PAUSE-001 |
| Pause screen with no menu | — | no background; the world and the HUD show unchanged | `client-menu.md`, MENU-PAUSE-008 |
| Death screen, and the confirmation of its title screen button | — | a vertical gradient from `60500000` to `A0803030`; no blur, no menu background texture | `player-death.md`, PLD-SCR-004 |
| Chat screen | — | no background; the world and the HUD show unchanged | `client-chat.md` |
| Game mode switcher | — | its own texture only | `client-game-mode-switcher.md`, CGMS-LAY-001 |
| Each container screen, for example the inventory, the creative inventory and a chest | — | in-game, then the window image of the screen | `client-container-screen.md`, `client-inventory.md` |
| Sign edit, book view, book edit, book sign, and the edit screens of a command block, a structure block, a jigsaw block and a test block | — | in-game, then the image of the screen | — |
| Server dialog | default | default | `dialog.md` |

- **Edge cases:**
  - the death gradient has the alpha `60` at the top and `A0` at the bottom. Thus the world
    is more red and more dark at the bottom of the window;
  - a level loading screen for a nether portal or an end portal draws an image of the portal
    over the full window. It is out of scope;
  - the screen of the end poem and the credits is out of scope.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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
     takes text input (`client-edit-box.md`, WID-MLE-011).
  3. A press and a wheel event set the device before the screen looks for a widget
     (WID-PTR-002).
- **Postconditions:** after a key moves the focus, no widget is hovered. A move of more than
  7 GUI units, a press or a wheel event makes a widget hovered again.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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
     activation (WID-SND-001). The slider and the multi-line edit box are exceptions
     (WID-SLD-002, WID-MLE-012).
  6. Then the widget becomes the focused widget. The screen starts a drag with it.
  7. When the widget does not accept the press, nothing else occurs. The press does not go to
     a widget below.
- **Postconditions:** a press of the right button on a button plays no sound and does not
  change the focus.
- **Edge cases:** two presses of the same button on the same screen within 250 ms are a double
  click. The widget gets this flag with the second press. `client-list.md` uses it.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### WID-PTR-003: Drag and release

- **Inputs:** a move of the mouse with the left button down, or a release of a mouse button.
- **Behavior:**
  1. While the screen drags, each move with the left button down goes to the focused widget,
     with the event position. It goes to the widget also when the point is outside it.
  2. A release of the left button ends the drag. When the screen dragged, the release goes to
     the focused widget.
  3. A release of another button goes to no widget.
- **Postconditions:** of the widgets of this document, only the slider and the multi-line edit
  box use the drag and the release (WID-SLD-002, WID-MLE-013).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### WID-PTR-004: Wheel

- **Inputs:** a wheel event with the amounts across and down, at the event position. The amount
  down is the wheel value `v` of `client-list.md`, SCRL-AREA-002, step 1, so each widget gets
  the same amount.
- **Behavior:**
  1. Set the last input device to mouse.
  2. The event goes to the first widget in the order of addition that the point is over
     (WID-PTR-001).
  3. The kind of the widget selects the effect. Of the widgets of this document, only the cycle
     button and the multi-line edit box use the wheel (WID-CYC-002, WID-MLE-016).
- **Postconditions:** the wheel does not change the focus. A wheel event over no widget has no
  effect.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### WID-PTR-005: The mouse outside the window

- **Observer:** a player: the hovered widget, a tooltip and the figure of the inventory screen
  after the mouse pointer leaves the window.
- **Inputs:** the moves of the mouse pointer that the platform reports, and the down state of
  the mouse buttons.
- **Behavior:**
  1. The client keeps the point (`px`, `py`) of the last move that the platform reported. The
     mouse position and the event position come from that point. The client has no state "no
     mouse position", and the exit of the pointer from the window does not clear the point.
  2. While a mouse button is down, the platform reports each move, also outside the window.
     `px` and `py` can then be negative, or more than the width or the height of the window.
     The mouse position uses the floor of the term "Mouse position" also for a negative value.
  3. While no mouse button is down, the last move before the exit is at a point in the window.
     That point is the point where the pointer left, moved to the nearest pixel of the window,
     or the last point of the pointer in the window. The platform selects which.
  4. Each user of the mouse position in a frame uses the point of item 1. Examples are the
     hovered state (WID-STATE-002), the tooltip of a widget (section 6), the slot under the
     mouse and the figure (`client-player-render.md`, CPR-FIG-003).
  5. When the pointer comes back into the window, its first move sets the point again.
- **Postconditions:**
  - While a button is down and the pointer is outside the window, the mouse position is outside
    the screen. No widget is hovered, and no tooltip of a widget shows.
  - While no button is down and the pointer is outside the window, the hovered widget and its
    tooltip are those of the last mouse position. They stay until the pointer comes back.

Provenance: `source_suggested`. Kind: semantic. Item 3 has the kind unspecified: a parity test
must not compare a frame after the pointer left the window with no button down. Confidence:
medium.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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
| multi-line edit box | never (WID-MLE-012) |

- **Postconditions:** each accepted press of a widget that has a click sound plays one click
  sound, also the second press of a double click.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### WID-TIP-002: Lines of the tooltip of a widget

- **Inputs:** the tooltip text, a text component.
- **Behavior:**
  1. The lines are the tooltip text, wrapped at the width 170.
  2. The lines change when the language of the client changes.
  3. The content box and the background are as `client-tooltip.md`, TIP-BOX-001, TIP-BOX-002,
     TIP-BG-001 and TIP-BG-002, state. The tooltip of a widget has no gap after the first line.
- **Postconditions:** no line of the tooltip of a widget is wider than 170, except a single
  word that is wider.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### WID-TIP-004: Position below or above a focused widget

- **Inputs:** a widget that is focused and not hovered, with the last input device a keyboard
  value. `w`, `h`, W, H and the bounds as in WID-TIP-003.
- **Behavior:**
  1. Set `x` = `bx + 3` and `y` = `by + bh + 4`.
  2. If `y + h + 3 > H`, set `y` = `by − h − 4`.
  3. If `x + w > W`, set `x` = `max(bx + bw − w − 3, 4)`.
- **Postconditions:** the content box starts 4 GUI units below the widget, or ends 4 GUI units
  above it. Its left edge is 3 GUI units right of the left edge of the widget.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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
Confidence: medium.

### WID-BTN-003: Activation of a button

- **Inputs:** an accepted press (WID-PTR-002), or an activation key while the button is focused
  and active.
- **Behavior:**
  1. The click sound plays.
  2. Then the action of the button runs. The action knows whether Shift is down.
- **Postconditions:** a key other than an activation key does nothing on a button. An
  inactive button takes no key.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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
medium.

### WID-ICON-002: Image button

- **Inputs:** a button with four sprites: active, active highlighted, inactive, inactive
  highlighted. It has no caption that draws.
- **Behavior:**
  1. The sprite of the state fills the bounds, times alpha, with the scaling of that sprite.
  2. The activation and the click sound are as for a button (WID-BTN-003).
- **Postconditions:** a screen that gives the same sprite for both inactive states shows no
  highlight on an inactive image button.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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
- **Edge cases:** a cycle button can use a second list of values while Alt is down. The vanilla
  "World Type" uses it (`client-world-screens.md`, WORLD-GEN-002), and so does the mode button
  of the structure block screen. No screen of mcrs uses it (WORLD-GEN-003), so this document
  does not state it.

Provenance: `source_suggested_black_box_confirmed`: the keys and the English texts are in
`assets/minecraft/lang/en_us.json` of the client jar. Kind: layout. Confidence: medium.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### WID-CYC-003: Tooltip and sprite of a value

- **Behavior:**
  1. A screen can give a tooltip text for each value. The tooltip of the cycle button is the
     tooltip text of its value, or none.
  2. A screen can give a sprite for each value. That sprite fills the bounds in place of the
     sprite of a button.
- **Postconditions:** the tooltip changes in the same frame as the value.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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
Confidence: medium.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### WID-SLD-004: Value and caption of a slider

- **Behavior:**
  1. A new position is clamped to [0, 1].
  2. When the clamped position differs from the old position, the slider applies the value.
     When it is the same, nothing applies.
  3. In both cases the caption updates from the value.
- **Postconditions:** a drag past the end does not apply the value again.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### WID-SLD-006: Option slider over a list of values

- **Inputs:** an option with a list of `k` values in order, and the index `i` of its value.
  An option with a real value from 0 to 1 uses the position as the value.
- **Behavior:**
  1. The value of a position `v` is the index `floor(v' · k)`, clamped to [0, `k − 1`], with
     `v'` as in WID-SLD-005.
  2. The position of the index `i` is `i / (k − 1)` as a real division, 0 for the first value
     and 1 for the last value.
- **Postconditions:** the first and the last value put the handle at the ends of the bar.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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
layout. Confidence: medium.

### WID-CHK-003: Toggle and tooltip of a checkbox

- **Behavior:**
  1. An activation plays the click sound, then inverts the selected state. The screen gets the
     new state at once.
  2. A screen can give a checkbox a tooltip text. The checkbox keeps it only when its label,
     wrapped at the width of the checkbox, has more than 2 lines. Else the checkbox has no
     tooltip.
  3. When the label has more than 2 lines, the second line ends with `...` (WID-TXT-002).
- **Postconditions:** a checkbox with a cut label shows the given tooltip text under the mouse.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

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

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### WID-LAY-003: Align an element in a rectangle

- **Inputs:** a layout element of size (`w`, `h`), a rectangle (`rx`, `ry`, `rw`, `rh`), and an
  alignment (`ax`, `ay`). To centre is the alignment (0.5, 0.5).
- **Behavior:**
  1. `x` = `rx + trunc(lerp(ax, 0, rw − w))`.
  2. `y` = `ry + trunc(lerp(ay, 0, rh − h))`.
- **Postconditions:** this alignment uses `trunc` on both axes, unlike WID-LAY-002. Example: to
  centre 316 × 120 in 427 × 240 gives (55, 60).

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### WID-LAY-007: Frame

- **Inputs:** a minimum width `mw` and a minimum height `mh` (default 0), and children.
- **Behavior:**
  1. The width of the frame is the largest of `mw` and the cell widths. The height is the
     largest of `mh` and the cell heights.
  2. Place each child by WID-LAY-002, from the position of the frame, in the full size of the
     frame.
- **Postconditions:** the children of a frame overlap. With the default alignment, each child
  is at the centre of the frame.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### WID-LAY-009: Spacer

- **Behavior:** a spacer is a layout element with a fixed width and a fixed height. It does not
  draw and takes no input. A spacer of a width has the height 0. A spacer of a height has the
  width 0.
- **Postconditions:** in a row, a spacer of the width 10 adds 10 plus one gap to the width.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### WID-LAY-010: Element with a label

- **Behavior:** an element with a label is a column with the gap 4. Its first child is a
  one-line text widget with the label. Its second child is the element. The default cell
  applies to both.
- **Postconditions:** the top of the element is 9 + 4 = 13 GUI units below the top of the
  label.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

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

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

---

## 14. Multi-line edit box

The experiments of this section use the test box (section 0.1). In the default font, `a`, `b`
and `W` are 6 GUI units wide, `i` is 2 wide and U+0020 is 4 wide.

### WID-MLE-001: Options of the box

- **Inputs:** a use of the box sets these options:

| Option | Default |
|--------|---------|
| bounds (`x`, `y`, `w`, `h`) | none |
| placeholder: a text component | empty |
| text colour | `FFE0E0E0` |
| cursor colour | `FFD0D0D0` |
| text shadow | on |
| frame | on |
| counter | on |
| character limit `m` | none |
| line limit `k`, an integer ≥ 0 | none |
| value notification | none |

- **Behavior:**
  1. The inner left is `ix = x + 4`. The inner top is `iy = y + 4`.
  2. The wrap width is `ww = w − 8`. A wrap width less than 1 counts as 1.
  3. The use gets the value notification after each change of the value
     (`client-edit-box.md`, EDIT-API-001).
- **Postconditions:** a dialog sets the options as `dialog.md`, DLG-CTL-005 and DLG-CTL-007,
  state.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: show the test box
with no `height`. The box is 108 × 44, and the text starts 4 GUI units right of the left edge.

### WID-MLE-002: Wrap into display lines

- **Inputs:** the value and `ww`. The advance of a character is that of `client-font.md`,
  FONT-WID-001, step 3.
- **Behavior:**
  1. The first display line begins at position 0.
  2. From the begin `b` of a display line, examine the characters in order. Keep the sum of
     their advances, and the position of the last U+0020 of the line.
  3. A U+000A ends the line before it. The next line begins after it.
  4. Another character adds its advance to the sum. A U+0020 is the last U+0020 also when it
     is the current character.
  5. The line breaks when the sum is more than `ww`. An earlier character of the line must
     have an advance that is not 0. At a break:
     - when the line has a last U+0020, the line ends before it, and the next line begins
       after it;
     - else the line ends before the current character, and the next line begins at it.
  6. At the end of the value, the current line ends at the end.
  7. When the value ends with U+000A, one more display line follows. It begins and ends at the
     end.
- **Postconditions:**
  - An empty value has one display line, from 0 to 0. `n` ≥ 1.
  - The U+0020 at a break is in no display line. Each other U+0020 stays in its line.
  - When a line breaks inside a word, its end is the begin of the next line.
- **Edge cases:** when the break of step 5 is at a U+0020 that is the last character, no line
  follows. The end of the value is then in no display line (WID-MLE-003).
- **Example:** with `ww` = 100, `aaaaaaaaaaaaaaa bb` (15 `a`) has the lines `aaaaaaaaaaaaaaa` and
  `bb`. 20 `a` have a line of 16 `a` and a line of 4 `a`.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: in the test box,
type 15 `a`, a space and `bb`. The second row shows `bb` at the left edge. Clear the box and type
20 `a`. The first row holds 16 `a`.

### WID-MLE-003: The line of a position

- **Inputs:** a position `p` and the display lines.
- **Behavior:**
  1. The line of `p` is the first display line `i` with `begin(i) ≤ p ≤ end(i)`.
  2. When no display line holds `p`, the line of `p` is the last display line.
- **Postconditions:**
  - A position at a break inside a word is in the upper of the two lines. The cursor shows at
    the end of that line.
  - Home on a line that begins inside a word puts the cursor at the end of the line above. A
    second Home goes to the begin of that line.
- **Edge cases:** step 2 applies only to the end of a value that the edge case of
  WID-MLE-002 breaks. Step 2 is `design` (WID-DEV-011).

Provenance: `source_suggested`; step 2 is `design`. Kind: semantic. Confidence: medium.
Experiment: in the test box, type 20 `a`. Press Home. The cursor shows at the end of the first
row. Press Home again. The cursor shows at the start of the first row.

### WID-MLE-004: Allowed characters and the insert

- **Inputs:** the value, the selection, a text `t` to insert, and `m`. The length is in code
  units.
- **Behavior:**
  1. Remove from `t` each character below U+0020 except U+000A. Remove each U+007F and U+00A7.
     Do not put a character in the place of a removed one.
  2. With `m`, let `free = m − length + (the length of the selected text)`. Keep the longest
     start of `t` with at most `free` code units. The kept text never ends inside a character.
  3. The new value is the value with the kept text in place of the selection.
  4. The line limit tests the new value (WID-MLE-005). When it refuses the new value, the
     value, the cursor and the anchor do not change.
  5. Else the value becomes the new value. The cursor and the anchor go to the end of the kept
     text.
- **Postconditions:**
  - A typed character, Enter, keypad Enter and a paste are inserts.
  - After an edit of the player, the length is never more than `m`.
  - A pasted U+000D U+000A becomes U+000A. A pasted tab is removed.
  - A typed character over a selection in a full box replaces the selection.
- **Edge cases:** a character outside the Basic Multilingual Plane counts two code units. When
  one code unit is free, such a character does not go in.

Provenance: `source_suggested`; the part of step 2 that counts the selection and the last
sentence of step 2 are `design` (WID-DEV-003, WID-DEV-004). Kind: semantic. Confidence: medium.
Experiment: show the test box with `max_length` 5. Paste `a`, a tab, `b`, U+000D U+000A, `c`, `§`
and `xyz123`. The value is `ab`, U+000A, `cx`, and the counter shows `5/5`.

### WID-MLE-005: Line limit

- **Inputs:** a new value of an edit and the line limit `k`.
- **Behavior:**
  1. The line count of a value is the number of its display lines (WID-MLE-002). The empty
     value has the line count 0.
  2. With `k`, the box refuses each new value with a line count more than `k`.
  3. The test applies to each edit of the player: an insert, a delete and a cut.
- **Postconditions:**
  - A value that ends with U+000A counts its last empty line.
  - A long line that wraps counts each of its display lines.
  - A paste that passes `k` has no effect. The box does not cut it.
  - With `k` = 0, the box holds only the empty value.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: show the test box
with `max_lines` 2. Type `a`, Enter and `b`. Press Enter. The value stays `a`, U+000A, `b`. Type
`c`. The value is `a`, U+000A, `bc`.

### WID-MLE-006: Set by the use

- **Inputs:** a text `t` that the use sets, and a flag "allow overflow" of the use, default off.
- **Behavior:**
  1. With `m`, cut `t` to its longest start with at most `m` code units. The cut never ends
     inside a character.
  2. When the flag is off and the line count of the result is more than `k`, the value does not
     change. Stop.
  3. Else the value becomes the result. The cursor and the anchor go to the end.
  4. The box scrolls to the cursor (WID-MLE-015).
- **Postconditions:** the set does not remove characters. A `§` in a set value changes the style
  of the draw, as for a plain string (`client-font.md`, FONT-WID-001).
- **Edge cases:** an initial value that passes `k` leaves a new box empty (`dialog.md`,
  DLG-CTL-005).

Provenance: `source_suggested`; the last sentence of step 1 is `design` (WID-DEV-003). Kind:
semantic. Confidence: medium. Experiment: show the test box with `max_lines` 1 and the
`initial` `a`, U+000A, `b`. The box is empty.

### WID-MLE-007: Edit keys

- **Inputs:** a key press or a key repeat that the screen gives to the focused box. The
  modifiers Shift, Alt and the edit modifier.
- **Behavior:**
  1. The box acts on these keys:

| Key | Effect |
|-----|--------|
| Backspace | When the selection is not empty, delete it. Else delete the character before the cursor. With the edit modifier, delete from the backward word position to the cursor. |
| Delete | When the selection is not empty, delete it. Else delete the character after the cursor. With the edit modifier, delete from the cursor to the forward word position. |
| Enter, keypad Enter | Insert U+000A (WID-MLE-004). |
| Left | Move the cursor to the position before it. With the edit modifier, move it to the backward word position. |
| Right | Move the cursor to the position after it. With the edit modifier, move it to the forward word position. |
| Up, Down | Move the cursor one display line (WID-MLE-008). With the edit modifier, nothing changes. |
| Home | Move the cursor to the begin of its line (WID-MLE-003). With the edit modifier, move it to 0. |
| End | Move the cursor to the end of its line. With the edit modifier, move it to the end. |
| Page Up | Move the cursor to 0. |
| Page Down | Move the cursor to the end. |
| shortcut A | Select all: the cursor goes to the end and the anchor goes to 0. |
| shortcut C, X, V | Copy, cut, paste (WID-MLE-010). |

  2. A shortcut acts only when Shift and Alt are not held.
  3. A move starts at the cursor, also when the selection is not empty. A move never goes
     before 0 or past the end.
  4. Without Shift, a move also puts the anchor at the new cursor. With Shift, the anchor
     stays.
  5. A delete is an edit. The line limit tests it (WID-MLE-005).
  6. After each move and each edit, the box scrolls to the cursor (WID-MLE-015). Select all
     does not scroll.
  7. The box uses each key of the table, also when nothing changes. It does not use another
     key. For example, Tab and Escape go to the screen (WID-FOC-003).
- **Postconditions:**
  - An arrow key never starts an arrow navigation while the box is focused. Only Tab moves
    the focus out of the box.
  - Left and Right cross a line break as one character.
- **Example:** in `foo bar baz` with the cursor at the end, Ctrl+Left puts the cursor before
  `baz`. Then Ctrl+Backspace gives `foo baz`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the test box,
type `ab`, Enter and `cd`. Press Home, then Shift+Page Up. Press Ctrl+C, Page Down and Ctrl+V.
The value is `ab`, U+000A, `cdab`, U+000A.

### WID-MLE-008: Up and Down

- **Inputs:** the cursor, its line `L` (WID-MLE-003), and the key Up or Down.
- **Behavior:**
  1. Let `x0 = width(value[begin(L)..cursor]) + 2`.
  2. The target line `T` is `L − 1` for Up and `L + 1` for Down.
  3. When `T` is not a display line, nothing changes.
  4. Else the cursor goes to `begin(T)` plus the number of characters of the head of
     `value[begin(T)..end(T)]` for the width `x0`.
- **Postconditions:**
  - The box keeps no column. Each move starts from the current cursor.
  - A move to a short line puts the cursor at the end of that line.
- **Example:** in `abcdef`, U+000A, `ab`, U+000A, `abcdef` with the cursor at 5, Down gives 9.
  A second Down gives 12, not 15.

Provenance: `source_suggested`; step 3 is `design` (WID-DEV-007). Kind: semantic. Confidence:
medium. Experiment: type the value of the example in the test box. Press Ctrl+Home, then Right
five times. Press Down two times and type `x`. The last line is `abxcdef`.

### WID-MLE-009: Word positions

- **Inputs:** the value and a position `p`.
- **Behavior:**
  1. A separator is one of these characters: U+0009 to U+000D, U+001C to U+001F, and each
     character of the Unicode categories Zs, Zl and Zp except U+00A0, U+2007 and U+202F.
  2. The backward word position from `p`: move back one position while the character before
     the current position is a separator. Then move back while it is not a separator.
  3. The forward word position from `p`: move forward one position while the character after
     the current position is not a separator. Then move forward while it is a separator.
- **Postconditions:**
  - A line break separates words. Ctrl+Left at the begin of a line goes to the previous line.
  - Punctuation does not separate words.
  - This is not the set of the one-line edit box (`client-edit-box.md`, EDIT-KEY-002).
- **Example:** in `foo  bar baz`, the forward word position from 0 is 5. Ctrl+Delete at 5 gives
  `foo  baz`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the test box,
type `foo  bar baz`. Press Ctrl+Home, Ctrl+Right and Ctrl+Delete. The value is `foo  baz`. Press
Enter, then Ctrl+Left. The cursor is before `foo`.

### WID-MLE-010: Clipboard

- **Behavior:**
  1. Copy: when the selection is not empty, the system clipboard gets the selected text, with
     its line breaks.
  2. Cut: copy as step 1. Then delete the selection as an edit (WID-MLE-005).
  3. Paste: insert the clipboard text (WID-MLE-004).
  4. When the selection is empty, a copy and a cut do not change the clipboard and do not
     change the value.
- **Postconditions:** a copy never changes the value. A paste of more lines than the line limit
  allows has no effect.

Provenance: `source_suggested`; step 4 is `design` (WID-DEV-009). Kind: semantic. Confidence:
medium. Experiment: show the test box with `max_lines` 2. Copy the three lines `x`, `y` and `z`
to the clipboard. Type `a` and press Ctrl+V. The value stays `a`.

### WID-MLE-011: Focus, typed characters and the keyboard

- **Behavior:**
  1. The box has no edit mode. While it is visible and focused, it takes typed characters.
  2. A typed character goes in as an insert (WID-MLE-004).
  3. While the box is visible and focused, it captures the keyboard as `client-edit-box.md`,
     EDIT-KEY-005, states.
  4. While the box is visible and focused, an arrow key and an activation key do not change
     the last input device (WID-IN-001, step 2).
  5. The focus time is the time when the box last got the focus (WID-MLE-020).
- **Postconditions:** a box that gets the focus by Tab takes typed characters at once.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: show a dialog
with a button and the test box. Press Tab until the box is focused. Type `e`. The value is `e`.

### WID-MLE-012: Pointer area and press

- **Inputs:** a press of a mouse button at the event position (`ex`, `ey`), the Shift key.
- **Behavior:**
  1. For a press and a wheel event, the box is under the point when WID-PTR-001 is true with
     the width `w + 6`. The extra column of 6 is the place of the scrollbar.
  2. The box takes only a press of the left button.
  3. The press is on the scrollbar when the box is scrollable (WID-MLE-015),
     `x + w ≤ ex ≤ x + w + 6` and `y ≤ ey < y + h`. Such a press starts a scrollbar drag
     (WID-MLE-017). The cursor and the anchor do not change.
  4. Else, on a press that is not a double click, compute the click position:
     - the line is `L = clamp(floor((ey − iy + s) / 9), 0, n − 1)`;
     - let `d = floor(ex − ix)`. The click position is `begin(L)` plus the number of
       characters of the head of `value[begin(L)..end(L)]` for the width `d`. When `d` < 0,
       it is `begin(L)`.
  5. Without Shift, the cursor and the anchor go to the click position. With Shift, the cursor
     goes there and the anchor stays.
  6. A double click selects a word (WID-MLE-014).
  7. The box gets the focus. It plays no click sound.
- **Postconditions:**
  - A click on the right half of a character puts the cursor before that character.
  - A click right of a line puts the cursor at the end of that line.
  - A click in the column right of a box with no scrollbar moves the cursor to the end of a
    line.

Provenance: `source_suggested`; the last sentence of step 3 is `design` (WID-DEV-008). Kind:
semantic. Confidence: medium. Experiment: at a GUI scale of 1, type `abc` in the test box. Click
at x = `ix + 10`, on the right half of `b`. Type `x`. The value is `axbc`.

### WID-MLE-013: Drag

- **Behavior:**
  1. While the screen drags the box, each drag event moves the cursor to the click position
     of WID-MLE-012, step 4. The anchor stays. A scrollbar drag is an exception.
  2. The box scrolls to the cursor after each drag event (WID-MLE-015).
- **Postconditions:** a drag above or below the box moves the cursor to a line out of view.
  The box then scrolls, and each further drag event can scroll further.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the test box,
type 8 lines `a`. Press Ctrl+End. Press the left button on the last row and drag above the box.
The view scrolls to the top, and the selection grows to position 0.

### WID-MLE-014: Double click

- **Inputs:** a double click with the left button, and the click position `p` of the second
  press (WID-MLE-012, step 4).
- **Behavior:**
  1. Let `c` be the character after `p`. When `c` exists and is not a separator
     (WID-MLE-009), the word is the longest run of non-separators that holds `c`.
  2. Else the word begins at the backward word position from `p`. It ends at the first
     separator after its begin, or at the end.
  3. The anchor goes to the begin of the word. The cursor goes to its end.
- **Postconditions:**
  - The selection holds the word, with no separator after it.
  - Shift does not change the result.
- **Example:** a double click on `b` in `foo bar` selects `bar`. A double click on the spaces
  of `foo  bar` selects `foo`.

Provenance: `design` (WID-DEV-006). Kind: deviation. Confidence: medium.

### WID-MLE-015: Scroll offset and scroll to the cursor

- **Inputs:** the scroll offset `s`, `h`, `n`, the cursor and its line `Lc`.
- **Behavior:**
  1. The content height is `C = 9·n + 8`. `M` and the clamp of `s` are as `client-list.md`,
     SCRL-AREA-001, states. The box is scrollable when `M` > 0.
  2. To scroll to the cursor: let `f = clamp(floor(s / 9), 0, n − 1)`. When
     cursor ≤ `begin(f)`, `s` becomes `9·Lc`.
  3. Else let `g = clamp(floor((s + h) / 9) − 1, 0, n − 1)`. When cursor > `end(g)`, `s`
     becomes `9·Lc + 17 − h`.
  4. Else `s` does not change.
  5. After each change of the value, `M` changes, and `s` is clamped (`client-list.md`,
     SCRL-AREA-004).
- **Postconditions:** after a move or an edit, the line of the cursor shows in the box.
- **Example:** in the test box with no `height` (`h` = 44), the fifth line starts after the
  fourth Enter. Then `C` = 53, and `s` becomes 9.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the test box,
type `a` and Enter four times. The text moves up 9 GUI units. Press Ctrl+Home. The text moves
down 9 GUI units.

### WID-MLE-016: Wheel

- **Inputs:** a wheel event with the wheel value `v` (`client-list.md`, SCRL-AREA-002) that the
  screen gives to the box.
- **Behavior:**
  1. `s` becomes `s − v · 4` (SCRL-AREA-001).
  2. The box keeps the event, also when it is not scrollable.
- **Postconditions:** the wheel does not move the cursor.
- **Edge cases:** a container that holds the box can take the event first. `dialog.md`,
  DLG-CTL-008, states the dialog.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open a screen
that holds the box outside a scroll area, for example the comment box of a chat report. Type
lines until the box scrolls, and press Ctrl+Home. Turn the wheel one notch toward the player. The
text moves up 4 GUI units.

### WID-MLE-017: Scrollbar

- **Behavior:**
  1. The scrollbar shows only when the box is scrollable. It draws as `client-list.md`,
     SCRL-BAR-001 and SCRL-BAR-002, state, with `SX = x + w`, `t = y` and `b = y + h`.
  2. The scrollbar is outside the bounds of the box, right of it.
  3. A press on the scrollbar starts the scrollbar drag of SCRL-BAR-003 (WID-MLE-012,
     step 3).
  4. The pointer shape over the scrollbar is that of SCRL-BAR-001, step 5.
- **Postconditions:** a box of width `w` uses `w + 6` GUI units across.

Provenance: `source_suggested`; `black_box_confirmed` for the sprite names. Kind: layout.
Confidence: medium. Artefact: the client jar holds `widget/scroller` and
`widget/scroller_background`. Experiment: in the test box, type 6 lines. A track of 6 × 44
draws right of the box, and its thumb is 32 high.

### WID-MLE-018: Frame, clip and draw order

- **Behavior:**
  1. With the frame option, the frame draws the GUI sprite `widget/text_field_highlighted`
     into the bounds while the box is highlighted (WID-STATE-002). Else it draws
     `widget/text_field`. The active state does not change the sprite.
  2. The clip rectangle is from (`x + 1`, `y + 1`) to (`x + w − 1`, `y + h − 1`). Inside
     it, these draw in order:
     - the text or the placeholder (WID-MLE-019);
     - the cursor (WID-MLE-020);
     - the selection highlight (WID-MLE-021).
  3. The content inside the clip draws `s` GUI units above its position at `s = 0`.
  4. Then the scrollbar draws (WID-MLE-017). Then the counter draws (WID-MLE-022). Neither is
     clipped.
- **Postconditions:**
  - After a click, the last input device is mouse, so the frame is highlighted only while the
    mouse is on the box. Typing does not change this.
  - After a Tab, the frame is highlighted while the box is focused.
- **Edge cases:** when `s` is not an integer, the rounding of the content to pixels is of the
  kind unspecified. A parity test must not compare such a frame.

Provenance: `source_suggested`; `black_box_confirmed` for the sprite names. Kind: layout.
Confidence: medium. Artefact: the client jar holds both sprites. Experiment: show the test box.
Click on it and move the mouse off it. The frame is `widget/text_field`. Press Tab until the box
is focused. The frame is `widget/text_field_highlighted`.

### WID-MLE-019: Text and placeholder

- **Behavior:**
  1. When the value is empty and the box is not focused, the placeholder shows. It wraps at
     `ww` (section 0.3) and starts at (`ix`, `iy`). Its colour is `CCE0E0E0`, with the drop
     shadow.
  2. Else display line `i`, from 0, draws as a plain string at (`ix`, `iy + 9·i`), with
     `s = 0`. It has the text colour, and the drop shadow when the text shadow option is on.
  3. A U+0020 at a break and a U+000A do not draw.
- **Postconditions:** a focused empty box shows no placeholder.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: in the test box,
type `a`, Enter and `a`. The second `a` is 9 GUI units below the first.

### WID-MLE-020: Cursor

- **Inputs:** the cursor, its line `Lc`, the focus time, the wall-clock time `now`.
- **Behavior:**
  1. The cursor shows only when the box is focused and `floor((now − focus time) / 300)` is
     even.
  2. Let `top = iy + 9·Lc`, with `s = 0`.
  3. When the cursor is before the end, it is a bar. Let
     `cx = ix + width(value[begin(Lc)..cursor])`. The bar is a fill from (`cx`, `top − 1`) to
     (`cx + 1`, `top + 10`) in the cursor colour.
  4. When the cursor is at the end, it is the text `_` in the cursor colour. It draws at
     (`ix + width(value[begin(Lc)..end(Lc)])`, `top`), with the drop shadow when the text
     shadow option is on.
- **Postconditions:**
  - The `_` touches the last character. It has no gap of 1, as the one-line edit box has.
  - A full box shows `_` at the end, not a bar.
  - When the value ends with U+000A, the `_` shows at `ix` on the last empty line.
- **Edge cases:** the focus time changes only when the box gets the focus. An edit does not
  restart the blink.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: at a GUI scale of
1, type `ab` in the test box and press Left. Take a screenshot in the on phase. A bar of
`FFD0D0D0` covers x = `ix + 6` from y = `iy − 1` to y = `iy + 9`.

### WID-MLE-021: Selection highlight

- **Inputs:** the selection from `sb` to `se` (`sb` < `se`), the display lines.
- **Behavior:**
  1. For each display line `i` with `sb ≤ end(i)` and `begin(i) ≤ se`, a highlight draws.
  2. Its left edge is `ix + width(value[begin(i)..max(sb, begin(i))])`.
  3. Its right edge is `ix + width(value[begin(i)..se])` when `se ≤ end(i)`. Else it is
     `x + w`.
  4. It covers the rows from `iy + 9·i` to `iy + 9·i + 9`, with `s = 0`.
  5. In the highlight, the colours change as `client-edit-box.md`, EDIT-SEL-001, step 6,
     states with the invert option on.
- **Postconditions:** a line that the selection passes is highlighted to the right edge of the
  box. The highlight is 9 high, not 11 as in the one-line edit box.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: at a GUI scale of
1, type `ab`, Enter and `cd` in the test box. Press Shift+Up. Take a screenshot. The first row is
inverted from x = `ix + 12` to `x + w − 1`. The second row is inverted from `ix` to `ix + 12`.

### WID-MLE-022: Counter

- **Behavior:**
  1. The counter shows only when the counter option is on and the use sets `m`.
  2. Its text is the translation `gui.multiLineEditBox.character_limit` with the length in
     code units and `m`.
  3. It draws at (`x + w − width(counter)`, `y + h + 4`) in `FFA0A0A0`, with the drop shadow.
- **Postconditions:** the counter is right-aligned to the right edge of the box, below it. It is
  outside the bounds.

Provenance: `source_suggested`; `black_box_confirmed` for the translation. Kind: layout.
Confidence: medium. Artefact: the `en_us` language file of the client jar gives
`gui.multiLineEditBox.character_limit` the text `%s/%s`. Experiment: show the test box with
`max_length` 100. Paste U+1F600. The counter shows `2/100`.

### WID-MLE-023: Pointer shape

- **Behavior:**
  1. While the box is hovered, the pointer is the text pointer (I-beam).
  2. Over the scrollbar, the pointer is that of WID-MLE-017, step 4.

Provenance: `source_suggested`; step 1 for an empty box that is not focused is `design`
(WID-DEV-010). Kind: semantic. Confidence: medium. Experiment: type `a` in the test box and put
the mouse on it. The pointer is the I-beam.

---

## 15. Intentional deviations

### WID-DEV-001: No graphics preset sets the blur radius

- **Rule:** WID-SCR-005, the paragraph "mcrs". `design`.
- **Reference:** the radius follows "Menu Background Blur", from 0 to 10. A graphics preset can
  also set the value, for example to 2. A change of the option sets the preset to "Custom".
- **Reason:** mcrs has no graphics preset. Only the option sets the radius.
- **Visible to the client:** only for a player who chose a graphics preset in the vanilla client.
- **Bug reference:** `none`.
- **Test:** a differential test sets the vanilla option and the option of mcrs to one value
  from 0 to 10, and compares the blur of WID-SCR-005.

### WID-DEV-002: A negative mouse position

- **Rule:** WID-PTR-005, item 2.
- **Reference:** the reference converts the point to the mouse position with a cut toward 0, not
  with the floor. A point less than one GUI unit left of the window or above it gives 0, not −1.
- **Reason:** the floor gives each GUI unit the same width. A point outside the window then
  never hovers a widget at the edge 0.
- **Visible to the client:** yes, only while a button is down and the pointer is less than one
  GUI unit left of the window or above it. A widget at the left or top edge of the GUI stays
  hovered in the reference.
- **Bug reference:** `none`.
- **Test:** a differential test does not drag the pointer out of the window at the left or the
  top edge.

### WID-DEV-003: A position of the multi-line edit box is between characters

- **Rule:** WID-MLE-004, step 2; WID-MLE-006, step 1; WID-MLE-007 (Left, Right, Backspace,
  Delete).
- **Reference:** the box counts its positions in code units. Left from the end of `a` U+1F600
  puts the cursor between the two code units, and Backspace then leaves a lone low surrogate. An
  insert of U+1F600 with one free code unit inserts a lone high surrogate.
- **Reason:** a lone surrogate is not text. The value notification and the dialog action must
  get valid text.
- **Visible to the client:** yes, only with a character outside the Basic Multilingual Plane.
- **Bug reference:** `none`.
- **Test:** a differential test does not use a character outside the Basic Multilingual Plane
  with a move, a delete or the limit.

### WID-DEV-004: The free space of an insert counts the selection

- **Rule:** WID-MLE-004, step 2.
- **Reference:** the free space is `m − length`. In a full box, a typed character over a
  selection only deletes the selection. With `m` = 5, `x` typed over `bc` of `abcde` gives
  `ade`.
- **Reason:** the one-line edit box counts the selection (`client-edit-box.md`, EDIT-LEN-002).
  A player expects that a typed character replaces the selection.
- **Visible to the client:** yes, in a box that is full or almost full.
- **Bug reference:** `none`.
- **Test:** a differential test does not insert over a selection when the insert passes
  `m − length`.

### WID-DEV-005: A word position at the end of the value

- **Rule:** WID-MLE-009.
- **Reference:** a word position from the end starts one code unit before the end. Ctrl+Left at
  the end of `a b` goes to 0. Ctrl+Backspace at the end of `hello a` deletes the whole value.
- **Reason:** a word position from the end must be that of each other position. A player can
  lose text by one key.
- **Visible to the client:** yes, when the value ends with a word of one character after a
  separator.
- **Bug reference:** `none`.
- **Test:** a differential test does not press Ctrl+Left, Ctrl+Backspace or a double click with
  the cursor at the end.

### WID-DEV-006: A double click selects the word under the mouse

- **Rule:** WID-MLE-014.
- **Reference:** the second press does not move the cursor. The box selects the word before the
  cursor that the first press set. A double click on the first character of a word selects the
  word before it. In `foo bar`, a double click on `b` selects `foo`.
- **Reason:** a double click must select the word that the player clicks.
- **Visible to the client:** yes, on the first character of a word and on a separator.
- **Bug reference:** `none`.
- **Test:** a differential test double-clicks only inside a word, not on its first character.

### WID-DEV-007: Up on the first line and Down on the last line

- **Rule:** WID-MLE-008, step 3.
- **Reference:** the box uses the same line. Because of the 2 of step 1, the cursor can move to
  the right over narrow characters. In `abiiii` with the cursor at 2, each Up moves the cursor
  one `i` to the right.
- **Reason:** Up and Down must not move the cursor along the line.
- **Visible to the client:** yes, only next to characters of a width of 2 or less.
- **Bug reference:** `none`.
- **Test:** a differential test ignores the cursor after Up on the first line or Down on the last
  line.

### WID-DEV-008: A press on the scrollbar keeps the cursor

- **Rule:** WID-MLE-012, step 3.
- **Reference:** a press on the scrollbar also moves the cursor to the end of the line at the
  height of the press. Without Shift, it clears the selection.
- **Reason:** a player who moves the view with the scrollbar must not lose the selection.
- **Visible to the client:** yes, the cursor and the selection after a press on the scrollbar.
- **Bug reference:** `none`.
- **Test:** a differential test ignores the cursor and the selection after a press on the
  scrollbar.

### WID-DEV-009: Copy and cut with an empty selection in the multi-line edit box

- **Rule:** WID-MLE-010, step 4.
- **Reference:** the clipboard gets the empty text.
- **Reason:** the one-line edit box keeps the clipboard (`client-edit-box.md`, EDIT-DEV-002).
  One shortcut must have one effect.
- **Visible to the client:** yes, the clipboard after the shortcut.
- **Bug reference:** `none`.
- **Test:** a differential test does not press shortcut C or X with an empty selection.

### WID-DEV-010: The text pointer over an empty box

- **Rule:** WID-MLE-023, step 1.
- **Reference:** while the placeholder shows, the pointer has its normal shape.
- **Reason:** the box takes a press at each point, also when it is empty. The pointer must show
  this.
- **Visible to the client:** yes, the pointer over an empty box that is not focused.
- **Bug reference:** `none`.
- **Test:** a differential test ignores the pointer shape over an empty box.

### WID-DEV-011: The end of the value after a break at a final space

- **Rule:** WID-MLE-003, step 2.
- **Reference:** the end is in no line. The scroll to the cursor then scrolls to the top of the
  box, and Up stays on the last line.
- **Reason:** the cursor must show after a typed space. The view must not jump.
- **Visible to the client:** yes, when a typed space at the end breaks the last line.
- **Bug reference:** `none`.
- **Test:** a differential test does not end the value with a space that breaks the last line.

---

## 16. Unresolved

1. A checkbox keeps a given tooltip text only when its label needs more than 2 lines
   (WID-CHK-003). Is that the intent, or a defect that the owner wants to correct?
2. The label of a checkbox wraps at the full width of the checkbox, but it starts 21 GUI units
   right of the box (WID-CHK-001). The label can go 21 GUI units past the bounds. Is this a
   defect?
3. An inactive checkbox has the same sprite and a white label (WID-CHK-002). Is that the
   intent?
4. The list of option sliders that wait 600 ms belongs to `client-menu.md`. WID-SCR-008 lists
   the in-game screens and the screens with their own background.
5. The tab order groups that the vanilla screens use are not stated. Each widget of this
   document uses the group 0.
