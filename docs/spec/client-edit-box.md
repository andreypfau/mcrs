# Edit box of the client GUI: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`.

This document states the one-line edit box of the vanilla client GUI. It states how the box
draws and how the player changes its value with the keyboard and the mouse. It also states what
a use of the box can set and observe. Each screen with a one-line text input uses these rules. The input box
of the chat screen is one use (EDIT-OPT-002). The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **GUI unit**, **W**, **H**: as `client-chat.md`, section 0.1, defines them.
- **Mouse position** (`mx`, `my`): as `client-tooltip.md`, section 0.1, defines it.
- **Character**, **code unit**: as `command-parser.md`, section 0.1, defines them.
- **Screen**, **parent screen**, **widget**, **bounds**, **visible**, **active**, **hovered**,
  **focused**, **activation**, **click sound**, **focus navigation**, **tooltip of a widget**:
  as `client-widget.md`, section 0.1, defines them. This document states only what is specific
  to the edit box.
- **Edit box**: a widget that holds one line of text that the player can change. In this
  document, "the box" is an edit box.
- **Bounds** of the box: the top left corner (`x`, `y`), the `width` and the `height`.
- **Use**: one edit box of a screen, with the values of its options (EDIT-OPT-001).
- **Value**: the text of the box. The value has no line break.
- **Length**: the number of code units of the value.
- **Position**: a place before, between or after the characters of the value. The position `p`
  has `p` characters before it. Position 0 is the start. The **end** is the position after the
  last character.
- **`value[a..b]`**: the characters of the value from the position `a` to the position `b`.
- **Cursor**: the position where the next typed character goes.
- **Anchor**: the position where the selection starts. When the selection is empty, the anchor
  is equal to the cursor.
- **Selection**: the characters between the anchor and the cursor.
- **Edit mode**: the state of a focused box in which it accepts characters and shows the cursor
  (EDIT-FOCUS-001).
- **Editable**: an option of the use. The player can see the value of a box that is not
  editable, but cannot change it (EDIT-STATE-001).
- **Maximum length** `m`: the largest length that an edit of the player can give
  (EDIT-LEN-002).
- **Width limit**: an optional largest text width of the value (EDIT-LEN-003).
- **Text origin** (`tx`, `ty`): the top left point of the visible text (EDIT-DRAW-002).
- **Inner width** `I`: the largest width of the visible text (EDIT-DRAW-002).
- **First visible position** `f`: the position of the first character that the box shows
  (EDIT-SCROLL-001). The box keeps `f` between frames.
- **Visible text**, **last visible position** `l`: as EDIT-DRAW-003 states.
- **Head**: the head of a text for a width `w` is the longest start of the text with a text
  width ≤ `w`. A character is in the head only when its full width fits.
- **Edit modifier**: the Control key. On macOS, it is the Command key.
- **Shortcut**: a key that the player presses with the edit modifier held. The key is the key
  of the current keyboard layout, not the physical key position.
- **Hint**: an optional text component that shows when the value is empty (EDIT-HINT-001).
- **Suggestion**: an optional plain text that shows after the end of the value
  (EDIT-HINT-002).
- **Formatter**: an optional function of the use that gives the style of the value
  (EDIT-API-004).
- **Filter**: an optional function of the use that accepts or refuses a new value
  (EDIT-API-003).
- **Value notification**: the message that tells the use the new value (EDIT-API-001).
- **Focus time**: the wall-clock time in milliseconds when the box last got the focus.
- **Double click**: a press of a mouse button not more than 250 ms after an earlier press of the
  same button on the same screen. A widget of the screen must take the earlier press. The
  positions of the two presses are not compared.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". No experiment of this document is run (section 13).

### 0.3 Colours and text

A colour is a 32-bit ARGB value in hexadecimal. The font, the line height of 9, the text width
and the drop shadow are as `client-chat.md`, section 0.3, states. `width(t)` is the text width
of the text `t` in GUI units.

A fill is a rectangle of one colour with no shadow. The rectangle from (`x0`, `y0`) to
(`x1`, `y1`) covers each GUI unit (`u`, `v`) with `x0` ≤ `u` < `x1` and `y0` ≤ `v` < `y1`.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `EDIT-OPT` | the options of the box, and the uses of the game |
| `EDIT-DRAW` | the frame, the text origin, the visible text and the order of draw |
| `EDIT-CUR` | the cursor and its blink |
| `EDIT-SEL` | the selection highlight |
| `EDIT-SCROLL` | the horizontal scroll: the first visible position |
| `EDIT-LEN` | the allowed characters, the maximum length and the width limit |
| `EDIT-KEY` | the edit keys, the word positions, the clipboard and the capture of keys |
| `EDIT-PTR` | the mouse: click, double click, drag and pointer shape |
| `EDIT-FOCUS` | the focus and edit mode |
| `EDIT-STATE` | a box that is not editable, and a box that is not active |
| `EDIT-HINT` | the hint and the suggestion |
| `EDIT-API` | what a use sets and observes |
| `EDIT-DEV` | intentional deviations |

### 0.5 Scope

In scope:

- the options and the draw of the box, the cursor, the selection and the horizontal scroll;
- the edit keys, the clipboard, the mouse, the focus and edit mode;
- the hint, the suggestion, and what a use observes. `client-gui-sprite.md` states how a sprite draws.

Out of scope: multi-line text input, the IME and the narrator, and the command suggestions of
the chat. The position and the size of a box in its screen belong to the specification of that
screen.

### 0.6 Relation to `client-chat.md`

The input box of the chat screen is a use of the edit box (EDIT-OPT-002). These parts of
`client-chat.md` cite this document:

- CHAT-BOX-001 cites it for the refused characters, the form and the blink of the caret, and
  the part of the input that the box shows. The rectangle of the input box, the fill
  `80000000` and the value of the maximum length are specific to the chat.
- CHAT-BOX-002 cites it for the edit keys.

CHAT-DEV-002 counts the maximum length of the chat in characters. This document counts code
units (section 15).

---

## 1. Options

### EDIT-OPT-001: Options of the box

- **Inputs:** the options that a use sets.

| Option | Default | Rule |
|--------|---------|------|
| bounds | the size 150 × 20 | EDIT-DRAW-001 |
| maximum length | 32 | EDIT-LEN-002 |
| width limit | none | EDIT-LEN-003 |
| border | on | EDIT-DRAW-001 |
| centred | off | EDIT-DRAW-002 |
| text shadow | on | EDIT-DRAW-003 |
| text colour | `FFE0E0E0` | EDIT-DRAW-003 |
| colour when not editable | `FF707070` | EDIT-STATE-001 |
| editable | on | EDIT-STATE-001 |
| can lose the focus | on | EDIT-FOCUS-003 |
| invert of the selected text | on | EDIT-SEL-001 |
| hint | none | EDIT-HINT-001 |
| suggestion | none | EDIT-HINT-002 |
| formatter | none | EDIT-API-004 |
| filter | none | EDIT-API-003 |

- **Behavior:** a use can change each option after it makes the box. EDIT-API-002 states the
  result of a change of the value and of the maximum length.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. The filter is a `design`
option (EDIT-API-003). Experiment: open Multiplayer, then Add Server. Type `abc` into the box
Server Name. Take a screenshot at a GUI scale of 1. The glyph pixels are `FFE0E0E0`.

### EDIT-OPT-002: Uses of the game

- **Behavior:** these uses of the game set options that differ from the defaults:

| Use | Options |
|-----|---------|
| input box of the chat screen | no border; maximum length 256; cannot lose the focus; a formatter that gives the colours of CCMD-HL-001 (`client-commands.md`); a value notification to the chat screen. The bounds are in CHAT-BOX-001. |
| name of the anvil | size 103 × 12; no border; maximum length 50; text colour `FFFFFFFF`, also when not editable; invert off; editable only while the first input slot holds an item |
| search of the creative inventory | no border; maximum length 50; text colour `FFFFFFFF`; invert off; cannot lose the focus while the search tab shows |
| line of the sign edit screen | no border; centred; text shadow off; width limit equal to the maximum line width of the sign; a formatter |

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: put an item into
the first input slot of an anvil. Paste a text of 60 characters `a` into the name. The name
holds 50 characters.

---

## 2. Draw

### EDIT-DRAW-001: Frame of the box

- **Inputs:** the bounds, the border option, the state of the box, and the last input of the
  player.
- **Behavior:**
  1. Without the border, the box draws no frame. The use can draw its own background, for
     example the fill of the chat screen.
  2. With the border, the box draws one sprite over its bounds, as `client-gui-sprite.md`
     states.
  3. The sprite is `minecraft:widget/text_field_highlighted` when one of these is true:
     - the box is in edit mode;
     - the last input of the player came from the mouse, and the box is hovered;
     - the last input of the player came from the keyboard, and the box is focused.
  4. Otherwise the sprite is `minecraft:widget/text_field`.
  5. A box that is not active uses the same two sprites and the same conditions.
- **Postconditions:** with the vanilla resource pack, the frame is an outline of 1 GUI unit
  around a black interior `FF000000`. The outline is `FFA0A0A0`, or `FFFFFFFF` when the frame
  is highlighted. Both sprites are opaque.
- **Artefacts:** `assets/minecraft/textures/gui/sprites/widget/text_field.png` and
  `assets/minecraft/textures/gui/sprites/widget/text_field_highlighted.png` of the client jar.
  The `.mcmeta` file of each has the scaling `nine_slice` with the width 200, the height 20 and
  the border 1.

Provenance: `source_suggested_black_box_confirmed`; the sprite names, the pixels and the scaling
are `black_box_confirmed` by the artefacts. Kind: layout. Confidence: medium. Experiment: open
Multiplayer, then Add Server. Put the mouse outside the boxes and take a screenshot. The box
Server Name is in edit mode and has the outline `FFFFFFFF`. The box Server Address has the
outline `FFA0A0A0`.

### EDIT-DRAW-002: Text origin and inner width

- **Inputs:** the bounds, the border option, the centred option, the visible text.
- **Behavior:**
  1. With the border: `tx` = `x + 4`, `ty` = `y + floor((height − 8) / 2)`, and
     `I` = `width − 8`.
  2. Without the border: `tx` = `x`, `ty` = `y`, and `I` = `width`.
  3. When the centred option is on, `tx` = `x + floor((width − width(visible text)) / 2)`.
     Step 1 or step 2 gives `ty` and `I`.
- **Example:** a box with the border at (10, 20) with the size 150 × 20 has `tx` = 14,
  `ty` = 26 and `I` = 142.
- **Postconditions:** without the border, the text starts at the top left corner of the
  bounds.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: at a GUI scale of
1, open Add Server, type `W` and take a screenshot. The glyph starts 4 GUI units right of the
left column of the outline.

### EDIT-DRAW-003: Visible text

- **Inputs:** the value, `f`, `I`, the text colour, the colour when not editable, the text
  shadow option, the formatter.
- **Behavior:**
  1. The visible text is the head of `value[f..end]` for the width `I`.
  2. `l` = `f` + the number of characters of the visible text.
  3. The box draws the visible text as one line from (`tx`, `ty`).
  4. The colour is the text colour when the box is editable. Otherwise it is the colour when
     not editable.
  5. With a formatter, each character draws in the style that the formatter gives
     (EDIT-API-004). A colour of that style replaces the colour of step 4.
  6. The text has a drop shadow when the text shadow option is on.
- **Postconditions:**
  - The visible text is never wider than `I`. The box does not cut a glyph.
  - The box shows no character before `f`.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: open the chat and
type 100 characters `W`. Take a screenshot. No glyph at the right edge of the input box is cut.

### EDIT-DRAW-004: Order of draw

- **Behavior:** the box draws its parts in this order:
  1. the frame (EDIT-DRAW-001);
  2. the visible text (EDIT-DRAW-003);
  3. the hint (EDIT-HINT-001);
  4. the suggestion (EDIT-HINT-002);
  5. the selection highlight (EDIT-SEL-001);
  6. the cursor (EDIT-CUR-001).
- **Postconditions:** a later part covers an earlier part where they overlap. The cursor shows
  above the selection highlight.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: see EDIT-SEL-001.

---

## 3. Cursor

### EDIT-CUR-001: Form and position of the cursor

- **Inputs:** the cursor, the length, `m`, the visible text, `f`, `l`, `tx`, `ty`.
- **Behavior:**
  1. When the cursor is at the end and the length < `m`, the cursor is the text `_`.
  2. Otherwise the cursor is a bar.
  3. For the bar, `cx` = `tx + width(value[f..cursor])`. The bar is a fill from
     (`cx`, `ty − 1`) to (`cx + 1`, `ty + 10`).
  4. For the `_`, `cx` = `tx + width(visible text) + 1` when the visible text is not empty.
     Otherwise `cx` = `tx`. The `_` draws as text at (`cx`, `ty`), with a drop shadow when the
     text shadow option is on.
  5. The cursor has the colour that EDIT-DRAW-003, step 4, gives. The formatter does not change
     it.
  6. The cursor shows only when all of these are true:
     - the box is in edit mode;
     - the blink phase is on (EDIT-CUR-002);
     - `f` ≤ cursor ≤ `l`.
- **Postconditions:**
  - The bar is 1 GUI unit wide and 11 GUI units high. It starts 1 GUI unit above the text.
  - A full box (length = `m`) shows a bar at the end, not `_`.
  - A box that is not editable also shows the cursor in edit mode.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: at a GUI scale of
1, open the chat, type `ab` and press Left. Take a screenshot in the on phase. A bar of
`FFE0E0E0` covers x = `4 + width(a)` from y = H − 13 to y = H − 3.

### EDIT-CUR-002: Blink of the cursor

- **Inputs:** the focus time, the wall-clock time `now` in milliseconds.
- **Behavior:**
  1. The blink phase is on when `floor((now − focus time) / 300)` is even.
  2. The focus time changes only when the box gets the focus. An edit or a cursor move does not
     change it.
- **Postconditions:** the cursor shows for 300 ms, then it hides for 300 ms. It shows at once
  when the box gets the focus.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: record the screen
for 2 s after the chat opens. Type a character at 450 ms. The cursor shows from 0 ms to 300 ms
and from 600 ms to 900 ms.

---

## 4. Selection

### EDIT-SEL-001: Selection highlight

- **Inputs:** the anchor, the cursor, `f`, `l`, `tx`, `ty`, the bounds, the invert option.
- **Behavior:**
  1. The highlight draws only when the anchor and the cursor are different.
  2. Let `a` be the anchor, limited to the range from `f` to `l`.
     `hx` = `tx + width(value[f..a]) − 1`.
  3. `cx` is the x of the cursor in EDIT-CUR-001 when `f` ≤ cursor ≤ `l`. When the cursor is
     after `l`, `cx` = `x + width`. When the cursor is before `f`, `cx` = `tx`.
  4. When `hx` or `cx` is more than `x + width`, set it to `x + width`.
  5. The highlight covers the rectangle from (`min(cx, hx)`, `ty − 1`) to
     (`max(cx, hx)`, `ty + 10`).
  6. In the highlight, each pixel with the colour (`R`, `G`, `B`) changes:
     - when the invert option is on, it becomes (`255 − R`, `255 − G`, 255);
     - when the invert option is off, it becomes (`R`, `G`, 255).
- **Postconditions:** the highlight changes the colours of the text and of the background below
  it. The selected text has no other colour.
- **Example:** on the black interior of the frame, with the default text colour:

| Pixel before | Invert on | Invert off |
|--------------|-----------|------------|
| background `FF000000` | `FFFFFFFF` | `FF0000FF` |
| text `FFE0E0E0` | `FF1F1FFF` | `FFE0E0FF` |
| text shadow `FF383838` | `FFC7C7FF` | `FF3838FF` |

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: at a GUI scale of
1, open Add Server, type `abc` and press Shift+Left two times. Take a screenshot in the off
phase of the blink. The highlight is white with glyphs of `FF1F1FFF`. It covers the columns from
`tx + width(a)` to `tx + width(abc) − 2`.

---

## 5. Horizontal scroll

### EDIT-SCROLL-001: First visible position

- **Inputs:** the value, `f`, `l`, `I`, and the target `t`. The target is the cursor.
- **Behavior:** after each change of the value or the cursor, by the player or by the use:
  1. If `f` > end, set `f` = end.
  2. When one of these is true, set `f` to the smallest position such that
     `width(value[f..t])` ≤ `I`:
     - `t` > `l`;
     - `t` < `f`;
     - `t` = `f` and `f` > 0.
  3. Otherwise `f` does not change.
  4. After select all (EDIT-KEY-001), `f` = 0.
- **Postconditions:**
  - A move of the cursor inside the view does not move the view.
  - When the cursor goes past the right edge, the view moves so that the cursor is at the right
    edge.
  - When the cursor goes to or past the left edge, the view moves so that the cursor is at the
    right edge. The player then sees the text before the cursor.
  - When the player deletes with Backspace at the end, `f` stays until the cursor reaches `f`.
    Then the view moves left.
  - After select all, the cursor at the end can be outside the view.
- **Edge cases:** when the character before `t` is wider than `I`, step 2 gives `f` = `t`.

Provenance: `design`. Kind: layout. Confidence: medium. Reason: the view moves by width, so the
cursor stays in the view for each font. The game moves the view by a count of characters
(EDIT-DEV-001). For a value whose characters have one width, the two give the same `f`.
Experiment: at a GUI scale of 4 with a window 852 pixels wide (W = 213), open the chat. Type 40
characters `a`. The box shows 34 characters `a`. Press Home: the box shows the first 34
characters.

---

## 6. Characters and length

### EDIT-LEN-001: Allowed characters

- **Behavior:**
  1. The box refuses these characters: each character below U+0020, U+007F and U+00A7.
  2. A typed character that the box refuses has no effect.
  3. A paste (EDIT-KEY-003) removes each refused character from the clipboard text before the
     insert. The box removes a line break or a tab. It does not put a space in its place.
- **Postconditions:** after an edit of the player, the value holds no refused character.
- **Example:** a paste of `a§b`, U+000A, `c` inserts `abc`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: copy two lines
`a` and `b` to the clipboard. Open the chat and press Ctrl+V. The input is `ab`.

### EDIT-LEN-002: Insert and maximum length

- **Inputs:** the value, the selection, the text `s` to insert, the maximum length `m`.
- **Behavior:**
  1. Let `free` = `m − length + (the length of the selected text)`, in code units.
  2. When `free` ≤ 0, the value does not change.
  3. Otherwise the box keeps the longest start of `s` with at most `free` code units. The kept
     text never ends inside a character.
  4. The filter and the width limit test the new value (EDIT-API-003, EDIT-LEN-003). A refused
     insert leaves the box as it was.
  5. The kept text replaces the selection. The cursor and the anchor go to the end of the kept
     text.
- **Postconditions:**
  - After an edit of the player, the length is never more than `m`.
  - A character outside the Basic Multilingual Plane counts two code units. The input box of
    the chat holds 128 such characters.
  - A typed character that does not fit has no effect.
  - A vanilla server refuses an anvil name of more than 50 code units. The count of the box
    agrees with it.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: put an item into
the first input slot of an anvil. Paste 30 characters U+1F600 into the empty name. The name
holds 25 characters.

### EDIT-LEN-003: Width limit

- **Inputs:** the width limit `wl` of the use, the value, the new value of an insert.
- **Behavior:** when the use sets `wl`, the box refuses an insert when both of these are true:
  - `width(new value)` > `wl`;
  - `width(new value)` > `width(value)`.
- **Postconditions:**
  - The box never refuses a delete.
  - A paste that does not fit has no effect. The box does not cut it.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: on a sign, type
`W` until the line is full. The next `W` has no effect. Paste a text that is wider than the
line. The line does not change.

---

## 7. Keys

### EDIT-KEY-001: Edit keys

- **Inputs:** a key press or a key repeat that the screen gives to the box. The box is active,
  focused and in edit mode. The modifiers Shift, Alt and the edit modifier.
- **Behavior:**
  1. The box acts on these keys:

| Key | Effect |
|-----|--------|
| Backspace | When the selection is not empty, delete it. Otherwise delete the character before the cursor. With the edit modifier, delete from the backward word position to the cursor. |
| Delete | When the selection is not empty, delete it. Otherwise delete the character after the cursor. With the edit modifier, delete from the cursor to the forward word position. |
| Left | Move the cursor to the position before it. With the edit modifier, move it to the backward word position. |
| Right | Move the cursor to the position after it. With the edit modifier, move it to the forward word position. |
| Home | Move the cursor to position 0. |
| End | Move the cursor to the end. |
| shortcut A | Select all: the cursor goes to the end and the anchor goes to 0. |
| shortcut C | Copy (EDIT-KEY-003). |
| shortcut X | Cut (EDIT-KEY-003). |
| shortcut V | Paste (EDIT-KEY-003). |

  2. A shortcut acts only when Shift and Alt are not held.
  3. A move starts at the cursor, also when the selection is not empty. A move never goes
     before 0 or past the end.
  4. Without Shift, a move also puts the anchor at the new cursor. The selection is then
     empty.
  5. With Shift, a move keeps the anchor.
  6. When the box is not editable, Backspace and Delete change nothing (EDIT-STATE-001).
  7. The box does not use any other key, for example Up, Down, Tab, Enter, keypad Enter and
     Escape. The screen gets these keys (`client-widget.md`).
- **Postconditions:**
  - A held key repeats as the system repeats it.
  - Escape closes the screen also while the box is focused (`client-chat.md`,
    CHAT-KEY-001).
- **Example:** the value is `foo bar baz` with the cursor at the end. Ctrl+Left puts the cursor
  before `baz`. Then Ctrl+Backspace gives `foo baz`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: do the steps of
the example in the chat. Take a screenshot after each key.

### EDIT-KEY-002: Word positions

- **Inputs:** the value and a position `p`. Only U+0020 separates words.
- **Behavior:**
  1. The forward word position from `p`:
     1. Start at the first position at or after `p` that has U+0020 after it. When there is
        none, the result is the end.
     2. Move forward one position while the character after the current position is U+0020.
     3. The result is the current position.
  2. The backward word position from `p`:
     1. Start at `p`. Move back one position while the character before the current position
        is U+0020.
     2. Then move back one position while the character before the current position is not
        U+0020.
     3. The result is the current position. It is never less than 0.
- **Example:** in `foo  bar` (two spaces):

| `p` | Forward | Backward |
|-----|---------|----------|
| 0 | 5 | 0 |
| 1 | 5 | 0 |
| 5 | 8 | 0 |
| 8 | 8 | 5 |

- **Postconditions:** a punctuation character or another space character does not separate
  words.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the chat,
type `a,b c`. Press Ctrl+Left. The cursor is before `c`. Press Ctrl+Left again. The cursor is
at the start.

### EDIT-KEY-003: Clipboard

- **Behavior:**
  1. Copy: when the selection is not empty, the system clipboard gets the selected text.
  2. Cut: copy as step 1. Then, when the box is editable, delete the selection.
  3. Paste: when the box is editable, insert the clipboard text as EDIT-LEN-001 and
     EDIT-LEN-002 state. When the box is not editable, nothing changes.
- **Postconditions:** a copy never changes the value.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the chat,
type `abc`, press Shift+Left and Ctrl+X. The input is `ab`. Press Ctrl+V two times. The input
is `abcc`.

### EDIT-KEY-004: Copy and cut with an empty selection

- **Behavior:** when the selection is empty, a copy and a cut do not change the clipboard and
  do not change the value.

Provenance: `design`. Kind: semantic. Confidence: medium. Reason: a player does not expect to
lose the clipboard text when the shortcut finds no selection (EDIT-DEV-002). Experiment: see
section 13, experiment 4.

### EDIT-KEY-005: Capture of the keyboard

- **Inputs:** the box is active, focused, editable and in edit mode.
- **Behavior:**
  1. While all four conditions are true, the screen and the game do not act on a key binding.
     Examples: `key.inventory`, `key.hotbar.1` to `key.hotbar.9`, `key.socialInteractions`.
  2. Escape, `key.fullscreen` and `key.screenshot` still act.
- **Postconditions:** in the search of the creative inventory, a typed `e` goes into the value.
  The inventory stays open.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open the
creative inventory on the search tab and type `e`. The inventory stays open and the search
value is `e`.

---

## 8. Mouse

### EDIT-PTR-001: Click

- **Inputs:** a press of a mouse button, the mouse position, the Shift key.
- **Behavior:**
  1. The box takes only a press of the left button. The box must be active, and the mouse
     position must be in its bounds.
  2. Let `d` = `min(mx − tx, I)`. When `d` < 0, the click position is `f`. Otherwise the click
     position is `f` + the number of characters of the head of the visible text for the width
     `d`.
  3. Without Shift, the cursor and the anchor go to the click position. With Shift, the cursor
     goes to the click position and the anchor stays.
  4. The box gets the focus and enters edit mode.
  5. The box plays no click sound.
- **Postconditions:**
  - A click on the right half of a character puts the cursor before that character.
  - A click on the screen outside each widget does not remove the focus from the box.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: at a GUI scale
of 1, open Add Server and type `abc`. Click at x = `tx + width(ab) − 2`, on the right half of
`b`. Type `x`. The value is `axbc`.

### EDIT-PTR-002: Double click

- **Behavior:** on a double click with the left button on the box:
  1. The box computes the click position `p` as EDIT-PTR-001, step 2, states.
  2. The anchor goes to the backward word position from `p` (EDIT-KEY-002).
  3. The cursor goes to the forward word position from `p`.
- **Postconditions:**
  - The selection holds the word and the spaces after it.
  - Shift does not change the result.
- **Example:** a double click on `foo` in `foo  bar` selects `foo  `, with the cursor before
  `bar`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the chat,
type `foo  bar`. Double-click on the first `o`. Press Ctrl+C, then End and Ctrl+V. The input is
`foo  barfoo  `.

### EDIT-PTR-003: Drag

- **Behavior:**
  1. The box takes a press of the left button (EDIT-PTR-001). While the player holds the
     button, each move of the mouse gives a drag event to the box.
  2. On each drag event, the cursor goes to the click position of EDIT-PTR-001, step 2, for
     the current mouse position. The anchor stays.
  3. EDIT-SCROLL-001 applies after each drag event.
- **Postconditions:**
  - A drag to the left of `tx` moves the cursor to `f`. When `f` > 0, each drag event then
    moves the view to the left.
  - A drag to the right of `tx + I` moves the cursor to `l`. The view does not move
    (section 15).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the chat,
type 200 characters `a`. Press Home. Press the left button on the input and move the mouse to
the right edge of the window. The selection ends at `l`, and the view does not move.

### EDIT-PTR-004: Pointer shape

- **Behavior:** while the box is visible and hovered:
  - the pointer is the text pointer (I-beam) when the box is editable;
  - the pointer is the "not allowed" pointer when the box is not editable.
- **Postconditions:** a box that is not active shows the same pointers.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open an anvil
with an empty first input slot. Put the mouse on the name. The pointer is "not allowed". Put an
item into the slot. The pointer is the I-beam.

---

## 9. Focus and edit mode

### EDIT-FOCUS-001: Edit mode

- **Behavior:**
  1. The box enters edit mode when it gets the focus in one of these ways:
     - a click (EDIT-PTR-001);
     - focus navigation with Tab;
     - the screen gives it the focus, for example when the screen opens.
  2. Focus navigation with an arrow key gives the box the focus without edit mode.
  3. When the box loses the focus, it leaves edit mode. The next focus follows step 1 or
     step 2.
- **Postconditions:** a box that is not focused is not in edit mode.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open Add Server.
Press Tab. Type `a`. The box Server Address holds `a`.

### EDIT-FOCUS-002: Focus without edit mode

- **Behavior:** while the box is focused and not in edit mode:
  1. Activation with Enter, Space or keypad Enter starts edit mode. The box uses the key.
  2. The box does not use other keys. The arrow keys move the focus to the next widget.
  3. The box refuses typed characters.
  4. The cursor does not show. The hint shows when the value is empty (EDIT-HINT-001).
  5. A click starts edit mode (EDIT-PTR-001).
- **Postconditions:** the player can move the focus over a box with the arrow keys and not type
  into it.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: see section 13,
experiment 1.

### EDIT-FOCUS-003: A box that cannot lose the focus

- **Behavior:** when the option "can lose the focus" is off:
  1. The box ignores a request to remove its focus.
  2. When another widget gets the focus of the screen, the box keeps its cursor, its
     highlighted frame and edit mode.
- **Postconditions:** the input box of the chat stays in edit mode while the chat screen is
  open.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open the chat and
press Tab with an empty input. Type `a`. The input is `a`.

---

## 10. Not editable and not active

### EDIT-STATE-001: A box that is not editable

- **Behavior:**
  1. The text draws in the colour when not editable. A use can set that colour. The anvil name
     keeps `FFFFFFFF` (EDIT-OPT-002).
  2. The box refuses typed characters. It does not use them, so the screen gets them.
  3. Cursor moves, selection, select all and copy act as in an editable box.
  4. Cut copies and does not delete. Paste does nothing.
  5. The box uses Backspace and Delete, and the value does not change.
  6. The cursor shows and blinks in edit mode.
  7. The pointer is "not allowed" (EDIT-PTR-004).
  8. The box does not capture the keyboard (EDIT-KEY-005).
- **Postconditions:** only the use can change the value of a box that is not editable
  (EDIT-API-002).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open an anvil
with an empty first input slot. Click the name and press `e`. The anvil screen closes.

### EDIT-STATE-002: A box that is not active

- **Behavior:**
  1. The box takes no click, no key and no character.
  2. Focus navigation does not give it the focus.
  3. The frame uses the same sprites and the same conditions as an active box
     (EDIT-DRAW-001). Thus the frame is highlighted when the mouse is on it.
  4. The pointer follows EDIT-PTR-004.

Provenance: `source_suggested`. Kind: semantic. Confidence: low. Experiment: see section 13,
experiment 3.

---

## 11. Hint and suggestion

### EDIT-HINT-001: Hint

- **Behavior:**
  1. The hint shows when all of these are true:
     - the use set a hint;
     - the value is empty;
     - the box is not in edit mode.
  2. The hint draws at (`tx`, `ty`) with a drop shadow. The text shadow option does not change
     this.
  3. The hint has the colour of its own style. A hint with no colour draws in `FF555555`.
  4. The box does not cut the hint to `I`.
- **Postconditions:**
  - A box in edit mode shows no hint.
  - A long hint can draw past the bounds.
- **Example:** the search of the recipe book has the hint `gui.recipebook.search_hint`
  ("Search...") in `FFAAAAAA` and italic.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: open the inventory
and the recipe book. Click outside the search. Take a screenshot. The search shows "Search..."
in gray italic.

### EDIT-HINT-002: Suggestion

- **Behavior:**
  1. The suggestion shows when the use set a suggestion and the cursor has the form `_`
     (EDIT-CUR-001, step 1).
  2. `sx` = `tx + width(visible text)` when the visible text is not empty. Otherwise `sx` =
     `tx − 1`.
  3. The suggestion draws as text at (`sx`, `ty`) in `FF808080`. It has a drop shadow when the
     text shadow option is on.
  4. The box does not cut the suggestion to `I`.
- **Postconditions:** the suggestion starts 1 GUI unit left of the `_`. The `_` covers the first
  column of the suggestion.
- **Example:** the chat shows the ghost text of a command as a suggestion (`client-commands.md`,
  CCMD-LIST-003).

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: at a GUI scale of
1, open the chat and type `/gam`. The ghost text `emode` starts at x = `4 + width(/gam)`.

---

## 12. What a use sets and observes

### EDIT-API-001: Value notification

- **Behavior:**
  1. The box sends a value notification when an edit of the player changes the value: an
     insert, a delete, a cut or a paste.
  2. The box sends a value notification after each set of the value by the use, and when a
     lower maximum length cuts the value (EDIT-API-002).
  3. The notification holds the value after the change. When the use receives it, the cursor,
     the anchor and `f` of the box also have their new values.
  4. The use receives the notification before the box takes the next input event.
  5. A cursor move, a change of the selection, a change of the focus and a copy send no
     notification.
  6. The box has no commit event. Enter goes to the screen (EDIT-KEY-001).
- **Postconditions:** an edit that does not change the value sends no notification.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: open the chat
and type `/`. The command suggestions show at once. Thus the chat screen received the
notification of the insert.

### EDIT-API-002: Set by the use

- **Behavior:**
  1. The use can set the value. The box keeps the longest start of the new value with at most
     `m` code units. The kept text never ends inside a character.
  2. After a set of the value, the cursor and the anchor go to the end. EDIT-SCROLL-001
     applies.
  3. A set of the value does not apply EDIT-LEN-001, EDIT-LEN-003 or the filter.
  4. The use can set the maximum length. When the value is longer, the box cuts it as step 1
     states. A cursor or an anchor after the new end moves to the end.
  5. The use can set the cursor, the selection and each option of EDIT-OPT-001.
- **Postconditions:** the chat screen puts an earlier input from the history into the box with a
  set of the value (`client-chat.md`).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: send `abc` in
the chat. Open the chat and press Up. The input is `abc`, with the cursor at the end.

### EDIT-API-003: Filter

- **Behavior:**
  1. A use can give a filter. The filter takes the new value of an insert and accepts or
     refuses it.
  2. The box asks the filter after the cut of EDIT-LEN-002, step 3. The width limit
     (EDIT-LEN-003) is a filter that the box supplies.
  3. A refused insert leaves the value, the cursor, the anchor and `f` as they were. The box
     sends no notification.
  4. The filter tests a typed character and a paste. It does not test a delete, a cut or a set
     by the use.

Provenance: `design`. Kind: semantic. Confidence: medium. Reason: the width limit is the only
filter of the game. A general filter lets a use refuse a value with the same result for the
player, for example in a field for numbers. Experiment: none at the game; EDIT-LEN-003 has the
experiment of the width limit.

### EDIT-API-004: Formatter

- **Behavior:**
  1. A use can give one or more formatters, in order.
  2. A formatter takes a part of the value and the position of its first character in the
     value. It gives a styled text with the same characters, or it gives nothing.
  3. The box uses the result of the first formatter that gives a styled text. When no
     formatter gives one, the box draws the plain text (EDIT-DRAW-003).
  4. A formatter must give each character the same style in each part that holds it.
  5. The box can ask for the visible text in more than one part, for example the parts before
     and after the cursor.
- **Postconditions:** the split of step 5 does not change the image.

Provenance: `design`. Kind: semantic. Confidence: medium. Reason: the game asks the formatter
for the parts before and after the cursor. Step 4 makes this split invisible to the player.
Experiment: open the chat and type `/gamemode creative`. Move the cursor into the word
`gamemode`. The colours of the command do not change (CCMD-HL-001).

---

## 13. Experiments

No experiment of this document is run. The owner works on the machine of the author, so no
vanilla client or game window started. The input agent of `tools/vanilla-client` also sets only
the movement key mappings. It sends no GUI key, no character and no mouse input, and it takes no
screenshot. Each experiment of this document needs these inputs.

These experiments decide the points of section 15:

1. Activation that starts edit mode (EDIT-FOCUS-002):
   1. Open Multiplayer, then Add Server. The box Server Name is in edit mode.
   2. Press Down. The box Server Address gets the focus without edit mode.
   3. Press Space.
   4. Record the value of Server Address and whether the cursor shows.
   5. Repeat steps 1 to 4 with Enter in place of Space.
2. Drag past the right edge (EDIT-PTR-003):
   1. In the chat, type 200 characters `a` and press Home.
   2. Press the left button on the input, and move the mouse past the right edge in 10 steps.
   3. Record `f` and the cursor after each step.
3. A box that is not active (EDIT-STATE-002): no vanilla screen with such a box is known. A test
   mod puts a box that is not active into a screen.
   1. Put the mouse on the box.
   2. Take a screenshot and record the pointer shape.
4. Copy with an empty selection (EDIT-KEY-004, EDIT-DEV-002):
   1. Put the text `x` into the system clipboard.
   2. Open the chat, type `abc` and press Ctrl+C.
   3. Press Ctrl+V. Record the input.
5. Scroll with characters of different widths (EDIT-SCROLL-001, EDIT-DEV-001):
   1. At a GUI scale of 4 with a window 852 pixels wide, open the chat.
   2. Type 60 characters `i`, then 5 characters `W`.
   3. Take a screenshot. Record the visible text and the cursor.
   4. Press Shift+Ctrl+Left until the selection reaches the left edge. Record the cursor and
      `f`.

---

## 14. Intentional deviations

### EDIT-DEV-001: The view moves by width

- **Rule:** EDIT-SCROLL-001.
- **Reference:** the view moves by a count of characters:
  - When the cursor goes past `l`, `f` increases by the difference of the cursor and `l`.
  - When the cursor goes before `f`, `f` becomes the cursor.
  - When the cursor is at `f` > 0, `f` decreases by a count `k`. `k` is the number of
    characters of the longest end of the whole value that fits `I`.
  - A move with Shift applies this test one time. Other changes apply it two times.
- **Visible to the client:** yes, with characters of different widths and with Shift. Examples:
  - The end of the text can stop short of the right edge.
  - After a wide character that follows narrow characters, the cursor and the last character
    can stay outside the view.
  - Shift with Left past the left edge leaves the cursor at the left edge, not at the right
    edge.
- **Test:** a differential test compares `f` only for values whose characters have one width,
  for example only `a`. It does not compare a move with Shift past the left edge.

### EDIT-DEV-002: Copy with an empty selection

- **Rule:** EDIT-KEY-004.
- **Reference:** a copy or a cut with an empty selection puts an empty text into the system
  clipboard.
- **Visible to the client:** yes. After Ctrl+C with no selection, a paste inserts nothing.
- **Test:** a differential test does not compare the clipboard after a copy or a cut with an
  empty selection.

---

## 15. Unresolved

- CHAT-DEV-002 counts the maximum length of the chat in characters. This document counts code
  units, because a vanilla server counts code units (EDIT-LEN-002). The owner decides if the
  chat keeps its exception.
- A drag past the right edge does not move the view, but a drag past the left edge does
  (EDIT-PTR-003). Must a drag to the right also move the view?
- On macOS, the word moves and the word deletes use the Command key, not the Option key
  (EDIT-KEY-001). Keep this?
- Left and Right with a selection and no Shift start at the cursor. They do not go to the edge
  of the selection (EDIT-KEY-001, step 3). Keep this?
- After select all, the view shows the start, and the cursor at the end can be outside the view
  (EDIT-SCROLL-001, step 4). Keep this?
- A paste that is wider than the width limit has no effect. The box does not cut it to the
  limit (EDIT-LEN-003). Keep this?
- A box that is not active highlights its frame on hover and shows the I-beam
  (EDIT-STATE-002). Keep this?
- Does the Space or the Enter that starts edit mode also type a character (EDIT-FOCUS-002)?
  Section 13, experiment 1, decides it.
