# Tooltips of the client GUI: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

Source of these values: `assets/minecraft/version.json`.

This document states how the vanilla client draws a tooltip at the mouse in a screen. It also
states what a hover event of a text style shows. The screens decide when a tooltip shows and
what text it holds. `client-chat.md`, CHAT-TIP-001, states this for the chat screen. The
document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **GUI unit**, **W**, **H**: as `client-chat.md`, section 0.1, defines them.
- **Mouse position**: the point (`mx`, `my`) of the mouse in GUI units. `mx` is
  `floor(px · W / ww)`, where `px` is the mouse x in window pixels and `ww` is the window width
  in pixels. `my` uses the heights in the same way.
- **Frame**: one draw of the current screen.
- **Tooltip**: a box of text lines that a screen draws near the mouse position, above all
  other content of the screen.
- **Tooltip request**: the lines, the mouse position and the optional style that a part of the
  screen gives during a frame to show a tooltip (TIP-REQ-001).
- **Line**: one row of text of a tooltip. A line is one text component that does not wrap
  again.
- **Content box**: the rectangle that the lines of a tooltip cover, without the background.
  It has the top left corner (`x`, `y`), the width `w` and the height `h`.
- **Tooltip style**: an optional resource location that selects the sprites of the background
  (TIP-BG-001).
- **Hover event**: the `hover_event` of a text style. Its kinds are `show_text`, `show_item`
  and `show_entity`.
- **Hover area**: the rectangle of a glyph in which the mouse position selects the style of
  that glyph (TIP-HOVER-002).

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents".

### 0.3 Colours and text

A colour is a 32-bit ARGB value in hexadecimal. The font, the width of a text and the wrap of a
text component are as `client-chat.md`, section 0.3, states. To wrap a text component at a
width means to split it into lines no wider than that width:

- A line break (U+000A) in the text also starts a new line. The line break is not in a line.
- When a line ends at a space (U+0020), the space is not in a line.
- A line that continues a wrapped row does not start with a space. This is not the same as the
  chat log, which adds one U+0020 to such a line (`client-chat.md`, CHAT-LOG-001).
- An empty text gives no line.
- A text that ends with a line break gives one more empty line at the end.

To use a text component as one line means to keep it whole, with no wrap. A line break in it
is then a glyph of the line, as TIP-BOX-002 states.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `TIP-REQ` | tooltip requests, and which request draws |
| `TIP-POS` | the position of the content box |
| `TIP-BOX` | the size of the content box and the lines in it |
| `TIP-BG` | the background and the outline |
| `TIP-HOVER` | hover events of text styles |
| `TIP-DEV` | intentional deviations |

### 0.5 Scope

In scope: the tooltip of a request with text lines, its position, size, background and order of
draw, and the three kinds of hover event. `client-gui-sprite.md` states the scaling of the
background sprites, for all GUI sprites.

Out of scope: the lines of an item tooltip, the image of an item tooltip (for example the
contents of a bundle), the tooltip of a widget with its delay and its positioners, and the
narration of tooltips.

---

## 1. Requests

### TIP-REQ-001: One tooltip in each frame

- **Behavior:**
  1. At the start of a frame, the screen has no tooltip request.
  2. A part of the screen can make a tooltip request while the screen draws.
  3. A request with no lines has no effect. A request with one empty line is not a request
     with no lines: it draws a tooltip with `w` = 0.
  4. When the screen already has a request, a new request has no effect. The first request
     of the frame stays.
  5. After the screen draws all its content, it makes the request of the hovered text style
     (TIP-HOVER-001). Step 4 applies to it.
  6. Then the screen draws the tooltip of the request, if there is one. The tooltip is above
     all other content of the screen.
  7. At the end of the frame, the screen discards the request.
- **Postconditions:**
  - A frame draws zero tooltips or one tooltip.
  - A tooltip shows in the first frame in which its source asks for it. There is no delay
    and no fade.
  - A tooltip follows the mouse position in each frame.
  - A hover event shows only when no other part of the screen made a request in that frame.
- **Edge cases:** a tooltip is a function of the mouse position only. A key never makes a
  tooltip request by itself.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: in the open chat,
put the mouse on a suggestion row with a tooltip, above a chat line with a `show_text` hover
event. Only the tooltip of the suggestion shows.

---

## 2. Position and size

### TIP-BOX-001: Size of the content box

- **Inputs:** the lines `L0` to `Ln−1` of a request, with `n` ≥ 1, and the flag "gap after the
  first line". Only an item tooltip sets the flag (TIP-HOVER-001).
- **Behavior:**
  1. `w` is the largest font width of the lines.
  2. Each line has a height of 10.
  3. Without the flag, or when `n` = 1, `h` = `10·n − 2`.
  4. With the flag and `n` ≥ 2, `h` = `10·n`.
- **Postconditions:** a tooltip with one line has `h` = 8.

Provenance: `source_suggested`. Kind: layout. Confidence: high. Experiment: see TIP-BG-001.

### TIP-POS-001: Position of the content box

- **Inputs:** the mouse position (`mx`, `my`) of the request, `w` and `h` (TIP-BOX-001), W and H.
- **Behavior:**
  1. Set `x` = `mx + 12` and `y` = `my − 12`.
  2. If `x + w` > W, set `x` = `max(x − 24 − w, 4)`.
  3. If `y + h + 3` > H, set `y` = `H − h − 3`.
- **Postconditions:**
  - Normally the content box starts 12 GUI units right of the mouse and 12 GUI units above
    it.
  - At the right edge, the content box ends 12 GUI units left of the mouse, but its left edge
    is not less than 4.
  - The position has no limit at the top. When `my` < 12, the top of the tooltip is outside
    the screen.
- **Edge cases:** step 2 tests `w` without the background. The background can go up to 4 GUI
  units past the right edge of the screen.

Provenance: `source_suggested`. Kind: layout. Confidence: high. Experiment: at a GUI scale of 2,
put the mouse at `mx` = 100, `my` = 100 on a one-line tooltip. Take a screenshot. The text starts
at x = 112, y = 88.

### TIP-BOX-002: Lines in the content box

- **Behavior:**
  1. Line `i`, from 0, has its top at `y + 10·i`. With the flag "gap after the first line",
     each line after line 0 is 2 GUI units lower.
  2. Each line starts at `x`. A line does not start with a space of indent.
  3. The text draws with a drop shadow. A part of the text with no colour in its style draws
     in `FFFFFFFF`. A part with a colour in its style draws in that colour.
  4. A line break (U+000A) inside one line does not start a new line. The font draws it as the
     glyph of the code point U+000A, the same as any other code point. With the vanilla fonts,
     this is the Unifont glyph of U+000A, a dotted box with the letters `LF`.
  5. The lines of a tooltip do not answer the mouse. They make no hover request and change
     no pointer.
- **Postconditions:** the lines keep the styles of their text component, for example bold,
  italic and colours.

Provenance: `source_suggested`. Kind: layout. Confidence: high. Experiment: see TIP-BG-001.

---

## 3. Background

### TIP-BG-001: Background and outline

- **Inputs:** the content box (`x`, `y`, `w`, `h`) and the tooltip style `st` of the request.
- **Behavior:**
  1. The background rectangle has the left edge `x − 12`, the top `y − 12`, the width `w + 24`
     and the height `h + 24`.
  2. Select two GUI sprites:
     - When `st` is absent: `minecraft:tooltip/background` and `minecraft:tooltip/frame`.
     - When `st` is `ns:p`: `ns:tooltip/p_background` and `ns:tooltip/p_frame`.
  3. Draw the background sprite into the background rectangle, then the outline sprite into the
     same rectangle. Each sprite draws with its own scaling (TIP-BG-002).
  4. When the resource pack has no sprite for a name, that name draws the missing sprite
     (`client-gui-sprite.md`, GSPR-MISS-001), stretched over the background rectangle. The
     tooltip still draws its lines.
  5. Draw the lines (TIP-BOX-002) above the two sprites.
- **Postconditions:** with the vanilla resource pack and no tooltip style, the tooltip has these
  parts. Each rectangle is (left, top, right, bottom), and the right and the bottom edges are not
  in the rectangle.

| Part | Rectangle | Colour |
|------|-----------|--------|
| background, middle | (`x − 4`, `y − 3`, `x + w + 4`, `y + h + 3`) | `F0100010` |
| background, top row | (`x − 3`, `y − 4`, `x + w + 3`, `y − 3`) | `F0100010` |
| background, bottom row | (`x − 3`, `y + h + 3`, `x + w + 3`, `y + h + 4`) | `F0100010` |
| outline, top | (`x − 2`, `y − 3`, `x + w + 2`, `y − 2`) | `505000FF` |
| outline, bottom | (`x − 2`, `y + h + 2`, `x + w + 2`, `y + h + 3`) | `5028007F` |
| outline, left | (`x − 3`, `y − 2`, `x − 2`, `y + h + 2`) | gradient, top `505000FF`, bottom `50280081` |
| outline, right | (`x + w + 2`, `y − 2`, `x + w + 3`, `y + h + 2`) | gradient, top `505000FF`, bottom `50280081` |

  The four corner pixels of the background and of the outline are transparent. The gradient
  comes from the sprite, which stretches 80 texels over the height `h + 4` (TIP-BG-002).

Provenance: `source_suggested`, and `black_box_confirmed` for the texels of the vanilla sprites.
Kind: layout. Confidence: high. Experiment: at a GUI scale of 1, show the tooltip
`chat.tag.system_single_player` in the open chat. Take a screenshot. Compare each pixel of the
background with the table.

### TIP-BG-002: Scaling of the background sprites

- **Behavior:**
  - The background and the outline sprites are GUI sprites. They draw by the rules of
    `client-gui-sprite.md`. The scaling of each sprite comes from its sprite metadata
    (GSPR-META-001) and selects the draw: `stretch` (GSPR-DRAW-001), `tile` (GSPR-DRAW-002) or
    `nine_slice` (GSPR-DRAW-003).
  - Each scaling type is valid for a tooltip sprite. The border of `nine_slice` can be one
    integer or one value for each side.
  - A sprite with an error in its sprite metadata draws as the missing sprite (GSPR-META-002).
    The error does not stop the load of the resource pack.
- **Postconditions:** the vanilla sprites are 100 × 100 GUI units with the scaling
  `nine_slice`. `tooltip/background` has the border 9 and repeats its inner parts.
  `tooltip/frame` has the border 10 and `stretch_inner` true. A tooltip is never smaller than
  24 × 24, so the borders are never reduced.

Provenance: `source_suggested`; `black_box_confirmed` for the `.mcmeta` values of the vanilla
pack. Kind: layout. Confidence: high. Experiment: in a resource pack, give `tooltip/background`
the scaling `stretch`. Show a tooltip. The full sprite stretches over the background
rectangle, so the transparent margin and the cut corners grow with the tooltip.

---

## 4. Hover events

### TIP-HOVER-001: What a hover event shows

- **Inputs:** the hovered text style of the frame (TIP-HOVER-002), the mouse position, W, and
  the option "Advanced Tooltips".
- **Behavior:** when the style has a hover event, the screen makes a request at the mouse
  position after it draws its content (TIP-REQ-001, step 5):

| Kind | Lines | Gap after the first line | Tooltip style |
|------|-------|--------------------------|---------------|
| `show_text` | the text, wrapped at `max(floor(W / 2), 200)` | no | none |
| `show_item` | the item tooltip of the stack | yes | the `tooltip_style` component of the stack |
| `show_entity` | only when "Advanced Tooltips" is on: the name, the type line and the UUID | no | none |

  - The lines of `show_entity` are, in this order:
    1. the name of the entity, only when the hover event has a name;
    2. the translation `gui.entity_tooltip.type`. Its one argument is the translation of the
       key `entity.<namespace>.<path>` of the entity type, with each `/` in the path replaced
       by `.`;
    3. the UUID as literal text, in lower-case hexadecimal, in the form `8-4-4-4-12`.
  - The lines of `show_entity` do not wrap.
  - When "Advanced Tooltips" is off, `show_entity` makes no request.
- **Postconditions:**
  - A `show_text` text with a line break shows on two or more lines.
  - An empty `show_text` text gives no line, so it makes no request (TIP-REQ-001, step 3).
- **Example:** a `show_entity` hover event with no name, the type `minecraft:pig` and the UUID
  `0-0-0-0-1` shows these lines in `en_us`: `Type: Pig` and
  `00000000-0000-0000-0000-000000000001`.

| Key | `en_us` text |
|-----|--------------|
| `gui.entity_tooltip.type` | `Type: %s` |
| `entity.minecraft.pig` | `Pig` |

"Advanced Tooltips" is the option that F3 + H changes. It is off by default.

Provenance: `source_suggested`. Kind: semantic; layout for the wrap width. Confidence: medium.
Experiment: on a GUI 427 units wide, send a `system_chat` with a `show_text` of 300 GUI units
of text. Open the chat and put the mouse on it. The tooltip wraps at 213 GUI units.

### TIP-HOVER-002: The hovered text style

- **Behavior:**
  1. Some text in a screen answers the mouse. The chat log of the open chat is one
     (`client-chat.md`, CHAT-LOG-007). Other text does not answer the mouse.
  2. For each glyph of such text, test the mouse position against the hover area of the glyph,
     in the coordinates of the text. A point is in a rectangle when
     `left ≤ px < right` and `top ≤ py < bottom`.
  3. When the point is in the hover area, and the style of the glyph has a hover event, that
     style becomes the hovered text style.
  4. When two glyphs match in one frame, the glyph that draws last sets the hovered text style.
- **Hover area of a glyph:** the pen position of the glyph is (`gx`, `gy`), with `gy` at the top
  of the line.
  - The left edge is the left edge of the drawn glyph. The right edge is `gx` plus the advance
    of the glyph. The top and the bottom are those of the drawn glyph, and the bottom includes
    the drop shadow.
  - A glyph that draws nothing, for example a space, has the area from `gx` to `gx` + advance,
    and from `gy` to `gy + 9`.
- **Postconditions:** with the default font, the hover area of a glyph is from `gx` to
  `gx` + advance, and from `gy` to `gy + 9`.
- **Edge cases:** the style of a glyph is its style after the inheritance of the text
  component. A child with no hover event of its own has the hover event of its parent.

Provenance: `source_suggested`. Kind: semantic; layout for the hover area. Confidence: medium.
Experiment: send a `system_chat` with the text `a b` and a `show_text` hover event. Open the
chat and put the mouse on the space. The tooltip shows.

---

## 5. Intentional deviations

No deviation is recorded.

---

## 6. Open points

- The lines and the image of an item tooltip need a separate specification. TIP-HOVER-001 for
  `show_item` depends on it.
- The tooltip of a widget, for example a button, uses the same background with a delay and
  other positions. A separate rule set can state it.
