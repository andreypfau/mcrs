# Scroll areas, lists and tab bars of the client GUI: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`.

This document states how the vanilla client shows a part of a tall content in a scroll area.
It states how the client draws and moves a scrollbar. It states how a list of rows looks, and
what a list does on a press or a key. It also states how a tab bar selects the content of a screen. Each list and
each menu with tabs uses these rules. The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **GUI unit**, **W**, **H**: as `client-chat.md`, section 0.1, defines them.
- **Font**, **width of a text**, **drop shadow**: as `client-chat.md`, section 0.3, defines
  them. One line of text is 9 GUI units high.
- **GUI sprite**, **draw of a GUI sprite**: as `client-gui-sprite.md` defines them.
- **Screen**, **widget**, **bounds**, **visible**, **active**, **hovered**, **focused**,
  **highlighted**, **click sound**: as `client-widget.md`, section 0.1, defines them.
- **Focus navigation**, **last input device**, **event position**, **in a world**, **label
  area**, **label scroll**: as `client-widget.md`, section 0.1, defines them.
- **Double click**: as `client-widget.md`, WID-PTR-002, defines it.
- **Mouse position**: as `client-tooltip.md`, section 0.1, defines it. It has no fraction.
- **Texture**: an image of the resource pack that is not in the GUI atlas. The name
  `minecraft:textures/gui/x.png` is the file `assets/minecraft/textures/gui/x.png`. A draw of a
  texture with a tile of `a` × `b` GUI units repeats the full image in each tile.
- **Ctrl**: the Control key. On macOS it is the Command key.
- **Reload overlay**: the overlay that covers the screen while the resource packs load.

Scroll areas:

- **Scroll area**: a widget that shows a part of a taller content and moves the content up
  and down. Its rectangle has the left edge `x`, the top `t`, the width `w` and the height `h`.
  Its bottom is `b = t + h`.
- **Content height** `C`: the height of all the content of a scroll area, in GUI units.
- **Scroll offset** `s`: how far the content is moved up, in GUI units. It can have a fraction.
- **Maximum offset** `M`: `max(0, C − h)`.
- **Scrollable**: a scroll area is scrollable when `M > 0`.
- **Wheel value** `v`: the vertical value of one wheel event (SCRL-AREA-002). It is positive
  for a turn away from the player.
- **Wheel step** `r`: the distance in GUI units that a wheel value of 1 moves the content.
- **Scrollbar**: the vertical bar of a scroll area. It has a track and a thumb.
- **Track**: the full height of the scrollbar.
- **Thumb**: the part of the scrollbar that shows the position of the visible part.
- **Scrollbar drag**: the state of a scroll area between a press on its scrollbar and the
  release of the button (SCRL-BAR-003).
- **Scroll panel**: a scroll area over a layout of widgets, with no rows (SCRL-PANEL-001).

Lists:

- **List**: a scroll area that shows rows in one column.
- **Row**: one item of a list. A row has a rectangle and an inner box (LIST-GEO-002).
- **Default row height**: the row height that a list gives a row when the screen states no
  other height.
- `RL`, `RR`, `RW`: the left edge, the right edge and the width of the rows of a list.
- **Selection list**: a list in which the player selects one row as a whole. The world list
  and the language list are selection lists.
- **Selected row**: the row of a selection list that the list shows as selected. A list has
  at most one.
- **Focused row**: the row that has the focus inside its list. A list has at most one.
- **Hovered row**: the row under the mouse (LIST-HOVER-001).
- **Widget list**: a list whose rows hold widgets. The option lists and the list of key binds
  are widget lists.
- **Option list**: the widget list of an option screen, with the row layout of LIST-OPT-001.
- **Section label row**: a row of an option list that shows a label for the rows below it
  (LIST-OPT-002).

Tab bars:

- **Tab**: a widget of a tab bar with a label. It selects a set of widgets of the screen.
- **Tab bar**: one row of tabs.
- **Current tab**: the tab whose widgets the screen shows. A tab bar has one.
- **Menu tab bar**: the tab bar at the top of a full screen, with the layout of
  TAB-MENU-001. The world creation screen and the statistics screen have one.
- **Tab content area**: the rectangle in which the screen shows the widgets of the current
  tab (TAB-AREA-001).
- **Focused tab**: the tab that has the focus inside its tab bar. A tab bar can have none.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". The pixel values of sprites and textures come from the
resource pack of the client jar of the version above. They are `black_box_confirmed`. None of
the experiments in this document has been run. Each one is a proposal.

### 0.3 Colours, rectangles and numbers

A colour is a 32-bit ARGB value in hexadecimal. A rectangle (`x0`, `y0`, `x1`, `y1`) has the
left top corner (`x0`, `y0`). It covers the GUI units from `x0` to `x1 − 1` and from `y0` to
`y1 − 1`. A fill of a rectangle draws its colour over each GUI unit of the rectangle.

`floor(a)` is the largest integer not above `a`. `trunc(a)` drops the fraction of `a`, toward
zero. `even(a)` is the smallest even integer that is not less than `a`. `clamp(a, lo, hi)` is
`lo` when `a < lo`, `hi` when `a > hi`, and `a` otherwise.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `SCRL-AREA` | the scroll offset, the mouse wheel, the clip and changes of the size |
| `SCRL-BAR` | the scrollbar, its thumb and the scrollbar drag |
| `SCRL-PANEL` | the scroll panel |
| `LIST-GEO` | the position of a list and of its rows |
| `LIST-BG` | the background, the separators and the order of draw of a list |
| `LIST-HOVER` | the hovered row |
| `LIST-SEL` | selection lists: draw, press, scroll to a row, keys and focus |
| `LIST-WID` | widget lists: press, hover and focus |
| `LIST-OPT` | the row layout of the option lists |
| `TAB-BAR` | the tab bar in general |
| `TAB-MENU` | the layout and the look of the menu tab bar |
| `TAB-AREA` | the tab content area and the change of the current tab |
| `TAB-CLICK` | a press on a tab and the sound |
| `TAB-KEY` | the keys of a tab bar |
| `TAB-FOCUS` | focus navigation into, inside and out of a tab bar |

### 0.5 Scope

In scope:

- the scroll area with its wheel, its clip and its scrollbar;
- the scroll panel;
- the list with its rows, its background and its separators;
- the selection list and the widget list;
- the row layout of the option lists;
- the tab bar and the menu tab bar, with the tab content area.

Out of scope:

- the rows of each screen of the game. Examples are the world list, the server list, the key
  binds, the language list, the resource pack lists and the statistics lists;
- what a double click or Enter on a row of a screen does;
- the pause screen and the option screens as screens (`client-menu.md`);
- the chat log and its scroll (`client-chat.md`, CHAT-LOG-004);
- the edit box (`client-edit-box.md`) and the multi-line text area;
- the narrator and all narration text;
- the creative inventory tabs and the advancement tabs;
- the tab style of the social interactions screen and the friends screens;
- the scroll panels with their own scrollbar look in container screens, for example the
  stonecutter, the loom, the villager trade screen and the enchantment screen;
- the move of rows by the player, as in the resource pack lists;
- the footer separator and the footer of a screen with a menu tab bar.

### 0.6 Constants

| Constant | Value | Rule |
|----------|-------|------|
| Width of the scrollbar | 6 | SCRL-BAR-001 |
| Minimum height of the thumb | 32 | SCRL-BAR-002 |
| Wheel step of a list | `floor(default row height / 2)` | SCRL-AREA-002 |
| Wheel step of a scroll panel | 10 | SCRL-AREA-002 |
| Space between the rows and the scrollbar | 8 (from `RR` to the scrollbar) | SCRL-BAR-001 |
| Space above the first row and below the last row | 2 | LIST-GEO-002 |
| Inset of the inner box of a row | 2 | LIST-GEO-002 |
| Default width of the rows | 220 | LIST-GEO-002 |
| Height of a separator | 2 | LIST-BG-002 |
| Rows of an option list: height, width, wheel step | 25, 310, 12 | LIST-OPT-001 |
| Height of the menu tab bar | 24 | TAB-MENU-001 |
| Maximum width of the menu tab bar | 400, minus 28 | TAB-MENU-001 |

---

## 1. Scroll area

### SCRL-AREA-001: The scroll offset

- **Inputs:** the height `h` of a scroll area, its content height `C`, and a new value `s'`
  for the scroll offset.
- **Behavior:**
  1. Each change of the scroll offset sets `s = clamp(s', 0, M)`.
  2. The content draws `floor(s)` GUI units above its position at `s = 0`.
- **Postconditions:** `0 ≤ s ≤ M`. At `s = 0` the top of the content shows. At `s = M` the
  bottom of the content shows at the bottom of the area.
- **Edge cases:** when `C ≤ h`, `M = 0` and `s = 0`. The area is not scrollable.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open the video
settings at GUI scale 2 in a window of 854 × 480 pixels. Turn the wheel down by 50 notches.
Take a screenshot. The last option row shows at the bottom of the list. Turn the wheel down
by 5 more notches. The next screenshot is the same.

### SCRL-AREA-002: The mouse wheel

- **Inputs:** one wheel event with its vertical value `y` from the platform, the options
  "Discrete Scrolling" and "Scroll Sensitivity", and the event position.
- **Behavior:**
  1. The wheel value is `v = y` when "Discrete Scrolling" is off. It is `v = sign(y)` when the
     option is on. Then `v` is multiplied by the value of "Scroll Sensitivity". The default of
     that value is 1.0.
  2. When the reload overlay shows, the event has no effect. Otherwise the screen gives the
     event to a widget as `client-widget.md`, WID-PTR-004, states.
  3. When that widget is a scroll area, the scroll offset changes to `s − v · r`
     (SCRL-AREA-001). A turn away from the player shows content that is higher up.
  4. The scroll area keeps the event. A widget inside the area, for example a cycle button in
     a row, does not get it.
- **Constants:** the wheel step `r` of a list is `floor(d / 2)`, where `d` is the default row
  height. The wheel step of a scroll panel is 10.
- **Postconditions:** with the default options, one notch of the wheel moves an option list by
  12 GUI units. It moves a list with rows of 36 by 18 GUI units.
- **Edge cases:** a scroll area that is not scrollable also keeps the event. Its content does
  not move.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open the video
settings with "Discrete Scrolling" on. Take a screenshot. Turn the wheel down by one notch and
take a second screenshot. The option rows are 12 GUI units higher in the second screenshot.

### SCRL-AREA-003: The clip

- **Inputs:** a scroll area with the rectangle (`x`, `t`, `x + w`, `b`), and its rows or
  widgets.
- **Behavior:**
  1. The rows and the widgets of the area draw only inside the clip rectangle. The clip
     rectangle is the rectangle of the area.
  2. The part of a row or a widget outside the clip rectangle does not show. A row that is
     partly in the area shows its visible part.
  3. The separators (LIST-BG-002) and the scrollbar (SCRL-BAR-001) draw after the clip ends.
     They are not cut.
  4. The clip rectangle is the clip rectangle of `client-widget.md`, WID-STATE-002, for each
     widget of the area.
- **Postconditions:** a row never covers the header or the footer of the screen. A widget of
  the area is hovered only inside the area.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: open the world
list with more worlds than fit. Turn the wheel until a row is half under the header. Take a
screenshot. The row shows below the header separator only.

### SCRL-AREA-004: A change of the content or of the size

- **Inputs:** a scroll area and a change of `h` or `C`.
- **Behavior:**
  1. After each change of `h` or of `C`, the scroll offset becomes `clamp(s, 0, M)`.
     Otherwise it does not change.
  2. When the screen adds rows at the end of a list, `s` does not change.
  3. When the screen adds a row above the first row, or removes the first row, the scroll
     offset changes so that `M − s` stays the same. The view keeps its distance from the
     bottom of the content.
- **Postconditions:** the scroll offset is valid after each change.
- **Edge cases:** when `C` becomes less than `h`, `s` becomes 0.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open the video
settings in a window of 854 × 480 pixels at GUI scale 2. Scroll to the bottom. Change the
window height to 600 pixels. Take a screenshot. The last option row is at the bottom of the
list, and no empty space shows below it.

---

## 2. Scrollbar

### SCRL-BAR-001: The scrollbar

- **Inputs:** a scroll area.
- **Behavior:**
  1. The scrollbar shows only when the area is scrollable.
  2. The scrollbar is 6 GUI units wide. Its left edge `SX` is:
     - `RR + 8` for a list;
     - `x + w − 6` for a scroll panel.
  3. The track draws the GUI sprite `minecraft:widget/scroller_background` into the rectangle
     (`SX`, `t`, `SX + 6`, `b`).
  4. Then the thumb draws the GUI sprite `minecraft:widget/scroller` into the rectangle
     (`SX`, `y0`, `SX + 6`, `y0 + th`). SCRL-BAR-002 gives `y0` and `th`.
  5. While the mouse position is on the scrollbar, the mouse pointer is a hand. During a
     scrollbar drag, it is a vertical resize arrow. The test of SCRL-BAR-003, step 1, with the
     mouse position decides "on the scrollbar".
- **Data of the vanilla resource pack:** both sprites have the scaling `nine_slice` with the
  width 6, the height 32 and the border 1.
  - `widget/scroller_background` is `FF000000` in each pixel.
  - `widget/scroller` is `FFC0C0C0`, with the right column and the bottom row in `FF808080`.
- **Postconditions:** a scrollable list at W = 427 with rows of width 220 has its scrollbar at
  `SX = 331`.
- **Edge cases:** when the area is not scrollable, no track and no thumb draw.

Provenance: `source_suggested`; `black_box_confirmed` for the sprite data. Kind: layout.
Confidence: medium. Experiment: open the video settings at W = 427 and H = 240. Take a
screenshot. The scrollbar covers x from 376 to 381. The track is black and the thumb is light
gray with a dark gray right column and bottom row.

### SCRL-BAR-002: The size and the position of the thumb

- **Inputs:** `t`, `h`, `C`, `M` and `s` of a scrollable area.
- **Behavior:**
  1. The thumb height is `th = clamp(floor(h · h / C), 32, max(h − 8, 32))`.
  2. The thumb top is `y0 = max(t, t + floor(floor(s) · (h − th) / M))`.
- **Postconditions:** at `s = 0` the thumb touches the top of the track. At `s = M` the thumb
  touches the bottom of the track.
- **Example:** `t = 33`, `h = 168`, `C = 340`. Then `th = 83` and `M = 172`. At `s = 86`,
  `y0 = 33 + floor(86 · 85 / 172) = 75`.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: open the video
settings at W = 427 and H = 240. Turn the wheel down by one notch. Take a screenshot. Measure
the thumb top and height. Compare them with the formula and the list rectangle.

### SCRL-BAR-003: A press on the scrollbar, and the scrollbar drag

- **Inputs:** a press, the moves of the mouse and the release of the left button.
- **Behavior:**
  1. A press of the left button at the event position (`ex`, `ey`) starts a scrollbar drag
     when all these are true:
     - the area is scrollable;
     - `SX ≤ ex ≤ SX + 6`;
     - `t ≤ ey < b`.
  2. A press on the track starts a scrollbar drag in the same way as a press on the thumb. The
     thumb does not jump to the mouse.
  3. The press gives the focus of the screen to the scroll area (`client-widget.md`,
     WID-PTR-002). It plays no click sound. For a selection list, the focus changes the outline
     of the selected row (LIST-SEL-001).
  4. During a scrollbar drag, each move of the mouse with the left button down changes `s`
     (WID-PTR-003):
     - when `ey < t`, `s` becomes 0;
     - when `ey > b`, `s` becomes `M`;
     - otherwise `s` becomes `s + dy · max(1, M) / max(1, h − th)`. `dy` is the vertical
       move of the event position since the last move, in GUI units.
  5. The release of the left button ends the scrollbar drag.
- **Postconditions:** during a scrollbar drag, the thumb follows the mouse. The scrollbar drag
  continues when the mouse leaves the scrollbar to the left or to the right.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open the video
settings at W = 427 and H = 240. Press the left button on the track below the thumb. Take a
screenshot: the list does not move. Move the mouse down by 20 GUI units and take a screenshot.
The thumb moved down by 20 GUI units.

---

## 3. Scroll panel

### SCRL-PANEL-001: Size and draw of a scroll panel

- **Inputs:**
  - a layout of widgets with the width `cw` and the height `C`;
  - a minimum width `wmin`, with the default 0;
  - a maximum height `hmax`;
  - a vertical alignment `a` from 0 to 1, with the default 0.
- **Behavior:**
  1. The width of the panel is `w = max(cw, wmin) + 20`. The left edge of the layout is at
     `x + 10`. At the right, 4 GUI units of space separate the layout from the scrollbar.
  2. The screen gives the height `h` of the panel. It is not more than `hmax`. The screens of
     the game use `min(C, hmax)` as a rule.
  3. The content height is the height of the layout, with no space above or below it.
  4. The top of the layout is at `t + round(a · (h − C)) − floor(s)`.
  5. The widgets of the layout draw inside the rectangle of the panel. A widget draws only
     when its bottom is not above `t` and its top is not below `b`.
  6. The scrollbar draws after the widgets (SCRL-BAR-001).
- **Postconditions:** the scroll panel has no background and no separators.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: show a dialog
from a server with a body taller than the screen. Take a screenshot. The scrollbar is at the
right edge of the body, 4 GUI units from the widest widget.

### SCRL-PANEL-002: Scroll to a focused widget of a scroll panel

- **Inputs:** a widget of the panel that gets the focus, with its top `wt` and its bottom
  `wb`, and the last input device.
- **Behavior:** when the last input device is a keyboard value:
  1. When `wt < t`, `s` becomes `s + (wt − t) − r`.
  2. Otherwise, when `wb > b`, `s` becomes `s + (wb − b) + r`.
  3. `r` is the wheel step of the panel. SCRL-AREA-001 clamps the result.
- **Postconditions:** a widget that the keyboard focuses shows with a margin of `r` to the edge
  of the panel, when the content lets it.
- **Edge cases:** when the last input device is mouse or none, the panel does not scroll.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: on a server,
show a dialog with 20 `boolean` inputs in a window of 854 × 480 pixels at GUI scale 2. Press
Tab until the focus reaches an input below the visible part. Take a screenshot. The input
shows with 10 GUI units between it and the bottom of the body.

---

## 4. Lists

### LIST-GEO-001: The position and the size of a list

- **Inputs:** the screen, with its header height `hh` and its footer height `fh`
  (`client-widget.md`, WID-LAY-011).
- **Behavior:**
  1. A list of a screen with a header and a footer covers the content part of the screen:
     `x = 0`, `w = W`, `t = hh` and `h = H − hh − fh`. The content top margin of WID-LAY-011
     does not apply to the list.
  2. A screen can give a list another rectangle.
  3. When W or H changes, the screen re-arranges (`client-widget.md`, WID-SCR-003). A screen
     that keeps its list gives it the new rectangle.
     The list keeps its scroll offset and places its rows again (LIST-GEO-002). When it has a
     selected row, it scrolls to that row (LIST-SEL-003). Then SCRL-AREA-004 applies.
- **Postconditions:** the list does not overlap the header or the footer.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: open the
language list at W = 427 and H = 240. Select a row near the bottom. Change the window height
from 480 to 360 pixels. Take a screenshot. The selected row is fully visible.

### LIST-GEO-002: The rows of a list

- **Inputs:** a list with the rows `0` to `n − 1`, the height of each row, and the width `RW` of
  the rows.
- **Behavior:**
  1. `RW` is 220 unless the kind of list states another width. An option list has 310.
  2. `RL = x + floor(w / 2) − floor(RW / 2)`. `RR = RL + RW`.
  3. Each row has its own height. A row has the default row height of the list unless the
     screen gives another height.
  4. The top of row 0 is `t + 2 − floor(s)`. The top of each next row is the bottom of the
     row before it. The rows have no space between them.
  5. The rectangle of a row is (`RL`, top, `RR`, top + height).
  6. The inner box of a row is its rectangle with 2 GUI units less at each side. A row draws
     its content in the inner box.
  7. `C` is the sum of the row heights plus 4.
- **Postconditions:** at `s = 0` there are 2 GUI units of space above the first row. At `s = M`
  there are 2 GUI units of space below the last row.
- **Edge cases:** a list has no header row of its own. A section label row is a row
  (LIST-OPT-002).
- **Example:** W = 427 and `RW = 220` give `RL = 103` and `RR = 323`.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: open the world
list at W = 427 with one world. Take a screenshot. The selected row outline covers x from 103
to 322, and its top is 2 GUI units below the header separator.

### LIST-BG-001: The background of a list

- **Inputs:** a list, its scroll offset, and whether the client is in a world.
- **Behavior:**
  1. When the client is not in a world, the background is the texture
     `minecraft:textures/gui/menu_list_background.png`. In a world, it is
     `minecraft:textures/gui/inworld_menu_list_background.png`.
  2. The background fills the rectangle of the list. The image fills a tile of 32 × 32 GUI
     units, whatever its size in pixels.
  3. The GUI unit (`px`, `py`) shows the point of the image at the fractions
     `((px + w) mod 32) / 32` across and `((py + h + floor(s)) mod 32) / 32` down.
  4. The background draws before the rows.
- **Data of the vanilla resource pack:** both images are 16 × 16 pixels of `70000000`.
- **Postconditions:** the background moves up with the rows when `s` increases.

Provenance: `source_suggested`; `black_box_confirmed` for the image data. Kind: layout.
Confidence: medium. Experiment: make a resource pack with a striped
`menu_list_background.png`. Open the world list with more worlds than fit. Turn the wheel by
one notch. The stripes move with the rows.

### LIST-BG-002: The separators of a list

- **Inputs:** a list and whether the client is in a world.
- **Behavior:**
  1. The header separator draws the texture `minecraft:textures/gui/header_separator.png`
     into the rectangle (`x`, `t − 2`, `x + w`, `t`). In a world, the texture is
     `minecraft:textures/gui/inworld_header_separator.png`.
  2. The footer separator draws the texture `minecraft:textures/gui/footer_separator.png`
     into the rectangle (`x`, `b`, `x + w`, `b + 2`). In a world, the texture is
     `minecraft:textures/gui/inworld_footer_separator.png`.
  3. Each image fills a tile of 32 × 2 GUI units. The tiles repeat across from `x`.
  4. The list draws its separators one time in each frame, in the order of LIST-BG-003. A
     screen that shows a list does not draw the separators of `client-widget.md`, WID-SCR-006.
- **Data of the vanilla resource pack:** each image is 32 × 2 pixels.
  - Both header separators have row 0 `33FFFFFF` and row 1 `BF000000`.
  - Both footer separators have row 0 `BF000000` and row 1 `33FFFFFF`.
- **Postconditions:** with the vanilla resource pack, the GUI row `t − 2` is light, `t − 1`
  is dark, `b` is dark and `b + 1` is light.
- **Edge cases:** a screen can show a list with no separators. The lists of the statistics
  screen have none.

Provenance: `source_suggested`; `black_box_confirmed` for the image data. Kind: layout.
Confidence: medium. Experiment: open the world list from the title screen. Take a screenshot.
Read the two GUI rows above `t` and the two GUI rows from `b`.

### LIST-BG-003: The order of draw of a list

- **Behavior:** a list draws in this order:
  1. the background (LIST-BG-001);
  2. the rows, from the first to the last, inside the clip (SCRL-AREA-003);
  3. the separators (LIST-BG-002);
  4. the scrollbar (SCRL-BAR-001).
- **Postconditions:** a row draws only when its bottom is not above `t` and its top is not
  below `b`. The separators draw over the rows.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: see
SCRL-AREA-003. The separator line shows over the half row.

### LIST-HOVER-001: The hovered row

- **Inputs:** the mouse position, once in each frame before the list draws.
- **Behavior:**
  1. When the reload overlay shows, the list has no hovered row.
  2. When the mouse position is in the rectangle of the list, the hovered row is the row
     whose rectangle contains the mouse position.
  3. Otherwise the list has no hovered row.
- **Postconditions:** the space left of `RL` and right of `RR` has no row. A row of a screen
  can use the hovered state to draw its content, for example an icon.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: see
LIST-SEL-001.

---

## 5. Selection lists

### LIST-SEL-001: The selected row and the hovered row

- **Inputs:** a selection list, its selected row, its hovered row, the focus of the list and
  the last input device.
- **Behavior:** before the content of a row draws:
  1. When the row is the selected row, fill its rectangle with the outline colour `O`. Then
     fill the rectangle with 1 GUI unit less at each side with `FF000000`. `O` is `FFFFFFFF`
     when the list has the focus, and `FF808080` when it does not.
  2. Otherwise, when the row is the hovered row and the last input device is mouse, do the same
     with `O = FF808080`.
  3. Then the content of the row draws.
- **Postconditions:** a selected row has an outline of 1 GUI unit around a black field. A
  widget list draws no selected row and no hovered row.
- **Example:** a selected row at top 40 with height 36, `RL = 103` and `RR = 323` fills
  (103, 40, 323, 76) and then (104, 41, 322, 75).

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: open the world
list with two worlds. Press on the first world. Move the mouse onto the second world. Take a
screenshot. The first row has a white outline and the second row has a gray outline. Press
Tab until the focus leaves the list. The outline of the first row becomes gray.

### LIST-SEL-002: A press on a row, and the double click

- **Inputs:** a press of a mouse button at the event position.
- **Behavior:**
  1. The screen gives the press to the list as `client-widget.md`, WID-PTR-002, states. The
     event position must be in the rectangle of the list.
  2. The list gives the press to the row whose rectangle contains the event position.
  3. On a press of the left button on a row:
     - the row becomes the focused row and the selected row;
     - the list gets the focus of the screen;
     - LIST-SEL-003 applies.
  4. The list plays no click sound. A row of a screen can play one.
  5. A press can be a double click (WID-PTR-002). The row gets the press with this state. The
     screen decides what a double click on a row does.
- **Postconditions:** a press in the list that hits no row does not change the selected row.
- **Edge cases:** the double click has no test of the distance. U-2 records this.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open the
world list with one world. Press twice on the row with 100 ms between the presses. The world
opens. Repeat with 400 ms between the presses. The world does not open.

### LIST-SEL-003: Scroll to a row

- **Inputs:** a row that becomes the selected row of a list, and the last input device. In a
  widget list, the row that becomes the focused row.
- **Behavior:**
  1. The list scrolls to the row when one of these is true:
     - the last input device is a keyboard value;
     - the top of the inner box of the row is above `t`;
     - the bottom of the inner box of the row is below `b`.
  2. To scroll to the row, compute `mt = ` row top `− t − 2`. When `mt < 0`, `s` becomes
     `s + mt`.
  3. Then compute `mb = b − ` row bottom `− 2`. When `mb < 0`, `s` becomes `s − mb`.
  4. SCRL-AREA-001 clamps each change.
- **Postconditions:** after a scroll, the row shows with at least 2 GUI units to each edge of
  the list, when it fits.
- **Edge cases:** a mouse press on a row whose inner box is fully visible does not scroll the
  list.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open the
language list. Press on a row that is half under the footer separator. Take a screenshot. The
row shows fully, with 2 GUI units between its bottom and the footer separator.

### LIST-SEL-004: Keys in a selection list

- **Inputs:** a key press while the list has the focus.
- **Behavior:**
  1. The focused widget gets the key first (`client-widget.md`, WID-FOC-003). The list gives it
     to its focused row. The row of a screen decides what the key does, for example Enter on a
     world.
  2. The list itself uses no key. Page Up, Page Down, Home and End do nothing in a list.
  3. When the row does not use the key, Tab and the arrow keys start focus navigation
     (WID-FOC-003, LIST-SEL-005).
- **Postconditions:** the keys of a list move the selection one row at a time.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open the
language list. Press on a row. Press Page Down. Take a screenshot. The selected row did not
change.

### LIST-SEL-005: Focus navigation in a selection list

- **Inputs:** a focus navigation event: Tab, Shift+Tab or an arrow key.
- **Behavior:**
  1. A list with no rows never takes the focus.
  2. For Tab navigation (`client-widget.md`, WID-FOC-004), the list is one widget.
  3. When the list gets the focus from another widget:
     - with a selected row, that row becomes the focused row;
     - with no selected row, Tab, Down, Left and Right give the first row, and Shift+Tab and Up
       give the last row.
  4. The focused row becomes the selected row, and LIST-SEL-003 applies.
  5. While the list has the focus, Down gives the next row and Up gives the previous row.
  6. While the list has the focus, the list clears its focused row and its selected row on
     each of these: Down on the last row, Up on the first row, Left, and Right. Then the
     screen does an arrow navigation from the bounds of the list (WID-FOC-005, WID-FOC-006).
     U-1 records this.
  7. While the list has the focus, Tab and Shift+Tab move the focus to the next and the
     previous widget of WID-FOC-004. The selected row stays.
- **Postconditions:** when the list loses the focus by a press on another widget or by Tab,
  the selected row stays selected. Its outline becomes `FF808080` (LIST-SEL-001).
- **Edge cases:** the arrow navigation of step 6 can find no widget. Then the list keeps the
  focus of the screen, with no focused row and no selected row. The next Down gives the first
  row, and the next Up gives the last row.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open the
world list with three worlds. Press on the second world. Press Down. Take a screenshot. The
third world is selected with a white outline. Press Up three times. Take a screenshot. No row
is selected, and the focus is on the search box above the list.

### LIST-SEL-006: A change of the rows

- **Inputs:** the screen removes rows from a selection list or replaces all its rows.
- **Behavior:**
  1. When the screen removes the selected row, the list has no selected row.
  2. When the screen replaces all the rows, the list has no selected row.
  3. SCRL-AREA-004 applies to the new content height.
- **Postconditions:** the selected row is always a row of the list.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open the world
list with two worlds. Select a world and type a search text that hides it. Take a screenshot.
No row is selected.

---

## 6. Widget lists

### LIST-WID-001: The mouse in a widget list

- **Inputs:** a widget list, the event position or the mouse position, and a press or a frame.
- **Behavior:**
  1. A row of a widget list holds widgets in an order. The row of a screen places them.
  2. The screen gives the press to the list as `client-widget.md`, WID-PTR-002, states. The
     event position must be in the rectangle of the list. The list gives the press to the row
     whose rectangle contains the event position.
  3. The row gives the press to its first widget whose bounds contain the event position.
     WID-PTR-002 applies to that widget. When the widget accepts the press, its row becomes the
     focused row, and the list gets the focus of the screen. LIST-SEL-003 applies to the row.
  4. A widget of a row is hovered as WID-STATE-002 states. The clip rectangle is the rectangle
     of the list (SCRL-AREA-003).
  5. A widget of a row that does not draw (LIST-BG-003) is not hovered, and its tooltip does
     not show. Otherwise its tooltip follows WID-TIP-001.
  6. The wheel over a widget of a row moves the list (SCRL-AREA-002). The widget does not get
     the wheel.
- **Postconditions:** a part of a widget outside the list cannot get a press and shows no
  tooltip.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open the video
settings. Turn the wheel until a button is half under the footer separator. Move the mouse
onto the hidden half of that button. Take a screenshot. The button is not highlighted and
shows no tooltip.

### LIST-WID-002: Focus navigation in a widget list

- **Inputs:** a focus navigation event while the list has the focus or gets it.
- **Behavior:**
  1. A list with no rows never takes the focus. A row with no widgets never takes the focus.
     A widget that cannot take the focus (`client-widget.md`, WID-FOC-001) is skipped.
  2. Tab navigation (WID-FOC-004) visits the rows in order, and the widgets of each row in
     order. After the last widget, the focus leaves the list to the next widget of
     WID-FOC-004. Shift+Tab visits them in the reverse order.
  3. With a focused widget at the index `c` in its row, from 0:
     - Right and Left move the focus to the next or the previous widget of the row that can
       take the focus.
     - At the end of the row, the row does an arrow navigation over its own widgets only
       (WID-FOC-005, WID-FOC-006). When it finds none, the screen does an arrow navigation
       from the bounds of the list.
     - Down and Up go to the next or the previous row that has widgets. In a row of `n`
       widgets, the focus goes to the widget at the index `min(c, n − 1)`.
     - When that widget cannot take the focus, the list tries the row after it in the same
       direction. When no row remains, the screen does an arrow navigation from the bounds of
       the list.
  4. With no focused row, an arrow key gives the focus as follows:
     - Down: the first widget of the first row that has widgets;
     - Up: the first widget of the last row that has widgets;
     - Right: the first widget of the first row that has widgets;
     - Left: the last widget of the first row that has widgets.
     When that widget cannot take the focus, the list continues as step 3 states, down for
     Right and Left.
  5. When a row becomes the focused row, LIST-SEL-003 applies.
- **Postconditions:** Up and Down keep the column of the focus where the rows let them. A row
  of a widget list does not move the focus by the position of its widgets for Up and Down.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open the video
settings. Press on the left widget of a row with two widgets. Press Right, then Down. Take a
screenshot. When the next row has two widgets, its right widget has the focus. When it has one
widget, that widget has the focus.

### LIST-OPT-001: The rows of an option list

- **Inputs:** an option list of a screen.
- **Behavior:**
  1. The default row height is 25. `RW` is 310. The wheel step is 12.
  2. A row holds one wide widget or one or two widgets of their own width. The width of a wide
     widget is 310. An option widget that is not wide is 150 wide.
  3. The widget at the index `i` of a row has its left edge at `floor(W / 2) − 155 + 160 · i`.
     Its top is the row top plus 2.
  4. Each widget keeps its own height. A button is 20 high (`client-widget.md`, WID-BTN-001).
- **Postconditions:** two widgets of 150 in one row have 10 GUI units between them.
- **Example:** at W = 427, the two widgets of a row cover x from 58 to 207 and from 218 to
  367. `RL = 58`, `RR = 368` and `SX = 376`.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: open the video
settings at W = 427. Take a screenshot. The buttons of the first two columns start at x 58
and x 218.

### LIST-OPT-002: The section label row

- **Inputs:** a label text of a screen, and whether the row is the first row of the list.
- **Behavior:**
  1. The row height is `p + 18`. `p` is 0 for the first row of the list and 18 for each other
     row.
  2. The row holds one label widget that can take the focus. The label box is the text width
     plus 8 wide, and 9 for each line plus 8 high.
  3. The left edge of the label box is `floor(W / 2) − floor(bw / 2)`, where `bw` is its width.
     Its top is the row top plus 2 plus `p`.
  4. The text is white `FFFFFFFF` with a drop shadow, centred in the box. Its top is 4 below
     the top of the box.
  5. While the label has the focus, fill (`lx + 1`, `ly`, `lx + bw`, `ly + bh`) with
     `FF000000`. Then draw an outline of 1 GUI unit in `FFFFFFFF` on the edge of the box. `lx`,
     `ly`, `bw` and `bh` are the left edge, the top, the width and the height of the box.
  6. Without the focus, nothing draws behind the text.
  7. A press on the label gives it the focus. The label plays no click sound.
- **Postconditions:** a section label row after other rows has 18 GUI units of space above
  its label.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: open the video
settings. Press on a section label. Take a screenshot. The label shows on a black box with a
white outline.

---

## 7. Tab bars

### TAB-BAR-001: The tab bar

- **Inputs:** the tabs that the screen gives, in order, and the current tab.
- **Behavior:**
  1. The tabs sit side by side in one row, from the left edge of the bar, in the order of the
     screen. They have no space between them.
  2. The screen gives the size of each tab, except in a menu tab bar (TAB-MENU-001).
  3. A tab can be inactive. An inactive tab never becomes the current tab by a press, a key or
     focus navigation.
  4. The mouse is on the bar only where it is on a tab.
  5. When the screen opens, the screen sets the first current tab. This makes no sound.
- **Postconditions:** each tab bar has one current tab.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: see
TAB-MENU-001.

### TAB-MENU-001: The layout of a menu tab bar

- **Inputs:** W and the number of tabs `n`.
- **Behavior:**
  1. The bar is at the top of the screen. Its top is 0 and its height is 24.
  2. The width of all tabs is `TBW = min(400, W) − 28`.
  3. Each tab is `TW = even(floor(TBW / n))` wide and 24 high.
  4. The first tab has its left edge at `X0 = even(floor((W − TBW) / 2))`. Tab `i` has its left
     edge at `X0 + i · TW`.
  5. When the window size changes, the bar uses the new W.
  6. The header of the screen is 24 high.
- **Postconditions:** the tabs touch each other. All edges of the tabs are at even x values.
- **Examples:**

| W | n | `TBW` | `TW` | Left edges | Right edge of the last tab |
|---|---|-------|------|------------|-----------------------------|
| 427 | 3 | 372 | 124 | 28, 152, 276 | 400 |
| 320 | 3 | 292 | 98 | 14, 112, 210 | 308 |

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: open the world
creation screen at GUI scale 2 in a window of 854 × 480 pixels. Take a screenshot. The three
tabs start at x 28, 152 and 276, and each is 124 wide.

### TAB-MENU-002: The sprites of the tabs

- **Inputs:** a tab of a menu tab bar, whether it is the current tab, and whether it is
  highlighted (`client-widget.md`, WID-STATE-002).
- **Behavior:** the tab draws one GUI sprite into its rectangle:

| Current tab | Highlighted | GUI sprite |
|-------------|-------------|------------|
| no | no | `minecraft:widget/tab` |
| no | yes | `minecraft:widget/tab_highlighted` |
| yes | no | `minecraft:widget/tab_selected` |
| yes | yes | `minecraft:widget/tab_selected_highlighted` |

- **Data of the vanilla resource pack:** the four sprites have the scaling `nine_slice` with
  the width 130 and the height 24. The border is left 2, top 2, right 2 and bottom 0, and
  `stretch_inner` is false.
  - `widget/tab`: the rows 0 to 3 are transparent. From row 4 a frame of `BF000000` outside
    and `33FFFFFF` inside surrounds a fill of `DB000000`. Row 22 is `33FFFFFF` and row 23 is
    `BF000000` over the full width.
  - `widget/tab_selected`: the frame starts at row 0 and the centre is transparent. In the
    rows 22 and 23 only the two columns at each side have colour, so the tab is open at the
    bottom.
  - The highlighted sprites have `FFFFFFFF` in place of the inner `33FFFFFF` frame.
- **Postconditions:** a tab that is not current is 4 GUI units lower at its top than the
  current tab.

Provenance: `source_suggested`; `black_box_confirmed` for the sprite data. Kind: layout.
Confidence: medium. Experiment: open the world creation screen. Move the mouse onto the
second tab. Take a screenshot. The second tab has a white inner frame. The first tab is open at
the bottom.

### TAB-MENU-003: The background and the line of the current tab

- **Inputs:** the current tab of a menu tab bar at the left edge `tx`, with the width `TW`, and
  its label.
- **Behavior:** after its sprite, the current tab draws:
  1. the texture `minecraft:textures/gui/menu_background.png` into the rectangle
     (`tx + 2`, 2, `tx + TW − 2`, 24), with a tile of 32 × 32 GUI units from the point
     (`tx + 2`, 2). It uses this texture also in a world. U-3 records this.
  2. a line of 1 GUI unit at y = 22. Its width is `uw = min(lw, TW − 4)`, where `lw` is the width
     of the label. Its left edge is `tx + floor((TW − uw) / 2)`. Its colour is `FFFFFFFF` when
     the tab is active and `FFA0A0A0` when it is inactive.
- **Data of the vanilla resource pack:** `menu_background.png` is 16 × 16 pixels of `40000000`.
- **Postconditions:** a line the width of the label shows under the label of the current tab.

Provenance: `source_suggested`; `black_box_confirmed` for the image data. Kind: layout.
Confidence: medium. Experiment: open the world creation screen at W = 427. Take a screenshot.
Under the label "Game", a white line of the label width shows at y = 22.

### TAB-MENU-004: The label of a tab

- **Inputs:**
  - a tab at the left edge `tx` with the width `TW`;
  - its label;
  - whether it is the current tab, and whether it is active.
- **Behavior:**
  1. The label area of the tab has `left = tx + 1` and `right = tx + TW − 1`. Its top is 0 for
     the current tab and 3 for another tab. Its bottom is 24.
  2. The label draws in this label area as `client-widget.md`, WID-BTN-004, steps 2 to 4,
     states. A label wider than the label area moves by the label scroll (WID-BTN-005).
  3. The colour is as WID-STATE-003 states: white for an active tab and `FFA0A0A0` for an
     inactive tab, with the drop shadow.
  4. The label shows no tooltip and no click of a text style.
- **Postconditions:** the text top is 8 for the current tab and 10 for another tab. The label
  of the current tab is 2 GUI units higher than the labels of the other tabs.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: open the world
creation screen at W = 427. Take a screenshot. Measure the text top of the current tab and of
the other tabs.

### TAB-MENU-005: The line under a menu tab bar

- **Inputs:** a menu tab bar.
- **Behavior:**
  1. The bar draws the texture `minecraft:textures/gui/header_separator.png` at y = 22, with
     the height 2, in two parts:
     - from x = 0 to the left edge of the first tab;
     - from the right edge of the last tab to W.
  2. Each part repeats a tile of 32 × 2 GUI units from its own left edge. The bar uses this
     texture also in a world. U-3 records this.
  3. The bar draws the line first, then the tabs from left to right.
- **Postconditions:** with the vanilla resource pack, the line continues through the bottom
  rows of each tab that is not current. It is open under the current tab.

Provenance: `source_suggested`; `black_box_confirmed` for the image data. Kind: layout.
Confidence: medium. Experiment: open the world creation screen at W = 427. Take a screenshot.
Read the GUI rows 22 and 23 from x = 0 to 426.

### TAB-AREA-001: The tab content area and a change of the current tab

- **Inputs:** a screen with a menu tab bar, its footer height `F`, and a change of the current
  tab.
- **Behavior:**
  1. The tab content area is (0, 24, W, H − F).
  2. When the current tab changes:
     - the widgets of the old tab leave the screen;
     - a focused widget that leaves the screen loses the focus;
     - the widgets of the new tab join the screen;
     - the new tab places its widgets in the tab content area.
  3. A tab with one layout places it at the left edge `0 + trunc(0.5 · (W − lw))` and the top
     `24 + trunc((H − F − 24 − lh) / 6)`. `lw` and `lh` are the width and the height of the
     layout.
  4. A tab can hold a list that fills the tab content area, as the statistics screen does.
  5. When the window size changes, the current tab places its widgets in the new tab content
     area.
  6. The screen draws the texture `minecraft:textures/gui/tab_header_background.png` over
     (0, 0, W, 24), with a tile of 16 × 16 GUI units. It draws its menu background
     (`client-widget.md`, WID-SCR-004) only from y = 24 down.
- **Data of the vanilla resource pack:** `tab_header_background.png` is 16 × 16 pixels of
  `00000000`.
- **Postconditions:** only the widgets of the current tab take a press, a key or the focus.

Provenance: `source_suggested`; `black_box_confirmed` for the image data. Kind: layout.
Confidence: medium. Experiment: open the world creation screen at W = 427 and H = 240. Press
on the second tab. Take a screenshot. The widgets of the first tab are gone, and the widgets of
the second tab show in the tab content area.

### TAB-CLICK-001: A press on a tab, and the sound

- **Inputs:** a press of the left button at the event position.
- **Behavior:**
  1. A press on an active tab makes it the current tab.
  2. A tab plays no press sound of its own.
  3. The current tab can change by a press, a key or focus navigation. Then the click sound
     plays (`client-widget.md`, WID-SND-001).
  4. When the current tab does not change, no sound plays. A press on the current tab makes no
     sound.
  5. After a press, the bar has the focus of the screen and has no focused tab. No tab is
     highlighted by the keyboard until a key moves the focus.
  6. An inactive tab does not use the press.
- **Postconditions:** each change of the current tab by the player plays one click sound.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open the world
creation screen and record the sounds. Press on the first tab, then on the second tab. The
record has one `minecraft:ui.button.click`, for the second press.

### TAB-KEY-001: The keys of a tab bar

- **Inputs:** a key press in a screen with a tab bar, and the state of Ctrl and Shift.
- **Behavior:**
  1. The screen gives each key to the tab bar first, before the steps of `client-widget.md`,
     WID-FOC-003.
  2. With Ctrl down, a digit key `d` from 0 to 9 gives the index `k = (d − 1) mod 10`, from 0
     to 9. The keys of the top row and of the keypad are the same. The tab at the index
     `clamp(k, 0, n − 1)` becomes the current tab when it is active. The bar uses the key in
     each case.
  3. With Ctrl down, Tab gives the next tab after the current tab, and Shift+Tab gives the tab
     before it. The order wraps at the ends. The bar skips inactive tabs and uses the key.
  4. When the bar has the focus, the focused tab moves to the target tab too.
  5. The click sound plays when the current tab changes (TAB-CLICK-001).
- **Postconditions:** Ctrl+1 gives the first tab. Ctrl+0 and Ctrl+9 give the last tab when
  the bar has fewer than 10 tabs. Ctrl+Tab does not move the focus of the screen.
- **Edge cases:** when no other tab is active, Ctrl+Tab keeps the current tab.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open the world
creation screen. Press on the world name box. Press Ctrl+3. Take a screenshot. The third tab
is current. Press Ctrl+Tab. The first tab is current.

### TAB-FOCUS-001: Focus navigation with a tab bar

- **Inputs:** a focus navigation event in a screen with a tab bar.
- **Behavior:**
  1. For Tab navigation (`client-widget.md`, WID-FOC-004), the bar is one widget.
  2. When the bar gets the focus by Tab, Shift+Tab or an arrow key, the current tab becomes
     the focused tab.
  3. While a tab has the focus, Left and Right move the focus to the next tab on that side
     that is active. That tab becomes the current tab at once, with the click sound.
  4. At the first or the last tab, the screen does an arrow navigation from the bounds of the
     bar (WID-FOC-005, WID-FOC-006).
  5. Tab and Shift+Tab move the focus from the bar to the next or the previous widget of
     WID-FOC-004.
  6. Down from the bar starts an arrow navigation from the bounds of the bar. As a rule, it
     finds a widget of the tab content area. Up from a widget at the top of the content gives
     the focus to the bar, and step 2 applies.
- **Postconditions:** in the world creation screen, Tab visits the bar, then the widgets of
  the current tab, then the buttons of the footer.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open the world
creation screen. Press Tab once. Take a screenshot: the first tab has a white inner frame.
Press Right. Take a screenshot: the second tab is current and has the white inner frame.

---

## 8. Intentional deviations

No deviation is recorded.

---

## 9. Unresolved

- **U-1:** in a focused selection list, four keys clear the selected row (LIST-SEL-005,
  step 6). They are Left, Right, Up on the first row and Down on the last row. In the world
  list, this makes the button that plays the selected world inactive. Is this a defect that
  mcrs corrects? Until the owner decides, the rule states the behaviour of the game.
- **U-2:** a double click has no test of the distance or of the row (LIST-SEL-002). Two quick
  presses on two different rows open the second row. Is this a defect that mcrs corrects?
- **U-3:** the menu tab bar uses `header_separator.png` and `menu_background.png` also in a
  world, for example in the statistics screen (TAB-MENU-003, TAB-MENU-005). The vanilla
  resource pack has the same pixels in the variants for a world, so only a resource pack shows
  the difference. Does mcrs use the variants for a world?
