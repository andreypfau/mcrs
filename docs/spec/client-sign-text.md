# Sign text of the client: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`.

This document states how the vanilla client draws the text of a sign and of a hanging sign. The
board, the post and the chains are block models (`block-entity-models.md`, BEM-SHAPE-010). Only
the text is drawn by the client as a block entity. The document states which text shows, how the
four lines come from the text components, where each line is in the block, which colour and
which light each line has, and which glyphs the font gives. The document does not prescribe an
architecture.

## 0. Conventions

### 0.1 Terms

- **Family**: `standing` for the blocks `<wood>_sign` and `<wood>_wall_sign`. `hanging` for the
  blocks `<wood>_hanging_sign` and `<wood>_wall_hanging_sign`.
- **Mount**: the way a sign holds. A `<wood>_sign` has the mount `ground`. A `<wood>_wall_sign`
  and a `<wood>_wall_hanging_sign` have the mount `wall`. A `<wood>_hanging_sign` has the mount
  `ceiling`.
- **Side**: `front` or `back`. A sign has the text of both sides. The front side faces the
  direction of the sign (SIGN-PLACE-001).
- **Side text**: the data of one side of the block entity: four messages, an optional list of
  four filtered messages, a dye colour and a glowing flag. The data is in the fields
  `front_text` and `back_text` of the block entity (`crates/mcrs_minecraft_block_entity/src/sign_text.rs`).
  A side text with no colour has the dye colour `black`. A side text with no glowing flag is not
  glowing.
- **Message**: one text component of a side text.
- **Line**: the text that the client draws for one message (SIGN-LINE-001).
- **Text unit**: the unit of the text plane. It is one pixel of the font at the scale of the
  sign. The text plane is the plane in which the lines lie. +X is to the right and +Y is down, as
  a viewer of that side sees them.
- **Text matrix**: the 4 × 4 matrix that maps a point `(u, v, 0, 1)` of the text plane to block
  space. Block space is as `block-entity-models.md`, section 0.1: the block fills `[0, 1]` on
  each axis, +X is east, +Y is up and +Z is south.
- **Line height** `h` and **maximum line width** `m`: the numbers of SIGN-LAY-001.
- **Yaw of a direction**, **clockwise direction**, `T`, `S`, `Ry`: as `block-entity-models.md`,
  section 0.1.
- **Outline range**: the condition of SIGN-COL-003.
- **Filtering flag**: the state of the profanity filter of the account of the player. The
  account service of the game sets it. A client with no such account has the flag off.
- **Packed light**: an integer `block · 16 + sky · 1048576`, where `block` and `sky` are light
  levels from 0 to 15.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". Each rule has a Milestone line. `1` means the first release
of the sign text. `later` means a rule that a later release implements.

### 0.3 Artefacts cited

- **Sign artefact**: the file `crates/mcrs_minecraft_model/tests/fixtures/vanilla/sign_text.json`.
  The task `dumpSignText` of `tools/vanilla-oracle` writes it. The task runs the sign text
  renderers of the game against a recorder. The recorder keeps every line that a renderer
  submits. The artefact uses a stub for the font: each code point has the advance 3 text units
  (`stub_advance`), and the stub does not wrap a message. The artefact therefore states the
  placement, the colours and the light, and it does not state the glyph metrics of the font.
  Its rows have these `kind` values:

| `kind` | Content |
|--------|---------|
| `constants` | line heights, maximum line widths, the full light, the outline colour of black glowing text, the outline range |
| `dye_colors` | for each dye, the bright and the dark text colour |
| `transform` | the text matrix for each family, mount, state and side |
| `layout` | the position of each of four lines, for several line widths |
| `paint` | colour, outline colour and light, for each dye, glowing flag and outline range |
| `filtering` | the lines that show with the filtering flag off and on |
| `styles` | the style of each code point of lines that come from styled components |

- **Block model artefact**: as `block-entity-models.md`, BEM-SHAPE-010.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `SIGN-SIDE` | which sides draw |
| `SIGN-LINE` | the lines that come from the messages |
| `SIGN-LAY` | the layout of the lines on the text plane |
| `SIGN-PLACE` | the text matrices |
| `SIGN-COL` | text colour and outline |
| `SIGN-LIGHT` | the light of the text |
| `SIGN-FONT` | the font and the glyphs |
| `SIGN-TEST` | parity tests |
| `SIGN-DEV` | intentional deviations |

### 0.5 Scope

In scope: the text of the blocks of the four sign families, for the 13 woods of
`block-entity-models.md`, BEM-SHAPE-010.

Out of scope: the sign edit screen, the click and dye actions of a player, the data of the block
entity on the wire, and the crack overlay of a block that a player breaks. A sign text has no
motion. The distance at which the client stops to draw block entities is also out of scope.

---

## 1. Which text shows

### SIGN-SIDE-001: Both sides draw

- **Inputs:** a block entity of a sign, with its front text and its back text.
- **Behavior:**
  1. The client draws the front text on the front side and the back text on the back side.
  2. The client draws both sides in every frame in which it draws the sign. The position of the
     viewer does not select a side.
  3. The text of a side lies in front of the board of that side (SIGN-PLACE-001). The board hides
     it from a viewer behind the board.
  4. A side text with four empty messages draws nothing.
- **Postconditions:** a viewer in front of a side sees the text of that side. A viewer behind
  it sees the text of the other side.

Provenance: `source_suggested_black_box_confirmed` (the rows `transform` of the sign artefact:
the matrix of the back side is the matrix of the front side turned by 180° about the vertical
axis through the centre of the board). Kind: semantic. Confidence: high. Milestone: 1.
Experiment: SIGN-EXP-001.

---

## 2. Lines

### SIGN-LINE-001: One line for each message

- **Inputs:** a side text, and the filtering flag.
- **Behavior:**
  1. When the filtering flag is on, the four messages are the filtered messages of the side text.
     When the flag is off, they are the messages of the side text.
  2. A side text with no filtered messages in its data has filtered messages that equal its
     messages.
  3. The client wraps each message at the maximum line width `m` of the family (SIGN-LAY-001).
     The wrap is the wrap of `client-tooltip.md`, section 0.3.
  4. The line is the first row of the wrap. When the wrap has no row, the line is empty.
     The client does not draw the other rows.
  5. The style of each code point of a line is the style of that code point after the
     inheritance of the message (`client-tooltip.md`, TIP-HOVER-002, edge cases). A style can
     set a colour, bold, italic, underlined, strikethrough, obfuscated and a font.
  6. A translatable message shows its translation. When the language has no such key, the line
     shows the key.
- **Postconditions:** a sign draws at most four lines, in message order. A message that is
  wider than `m` shows only its first row.

Provenance: `source_suggested_black_box_confirmed` (the rows `filtering` and `styles` of the sign
artefact: the filtering flag selects one of two lists; the styles of the code points follow the
styles of the components; a key with no translation shows as the key). Item 3 and item 4 are
`source_suggested`. Kind: semantic. Confidence: high for items 1, 2, 5 and 6, medium for items 3
and 4. Milestone: 1. Experiment: SIGN-EXP-002.

---

## 3. Layout

### SIGN-LAY-001: Line height and maximum line width

- **Inputs:** the family of the sign.
- **Behavior:** the family selects two numbers, in text units:

| Family | Line height `h` | Maximum line width `m` |
|--------|-----------------|------------------------|
| `standing` | 10 | 90 |
| `hanging` | 9 | 60 |

- **Postconditions:** none.

Provenance: `black_box_confirmed` (the row `constants` of the sign artefact). Kind: layout.
Confidence: high. Milestone: 1.

### SIGN-LAY-002: Position of a line

- **Inputs:** the line number `i` from 0 to 3, the line height `h`, and the width `w` of the
  line in text units. `w` is the integer width that the font gives to the line (SIGN-FONT-001).
- **Behavior:**
  1. The line starts at the point `(x, y)` of the text plane. The point is the top left corner
     of the first glyph.
  2. `x` is `−(w div 2)`. The division is an integer division that rounds toward zero.
  3. `y` is `i · h − (4 · h) div 2`. The division is an integer division.
  4. The client draws a line even when its width is 0. Such a line shows nothing.
- **Postconditions:**
  - For `h` = 10, the four lines start at `y` = −20, −10, 0 and 10.
  - For `h` = 9, the four lines start at `y` = −18, −9, 0 and 9.
  - A line of width 3 starts at `x` = −1. A line of width 9 starts at `x` = −4.
  - A line is centred on `u` = 0. When `w` is odd, the centre of the line is a half text unit to
    the right of `u` = 0.

Provenance: `source_suggested_black_box_confirmed` (the rows `layout` of the sign artefact, for
the stub widths 0, 3, 6, 9, 12, 15, 18 and 21). Kind: layout. Confidence: high. Milestone: 1.
Experiment: SIGN-EXP-003.

---

## 4. Placement

### SIGN-PLACE-001: Text matrices

- **Inputs:** the family, the mount, the side, and the state of the block: `rotation` from 0 to 15
  for the mounts `ground` and `ceiling`, `facing` for the mount `wall`.
- **Behavior:**
  1. The angle `A` is `22.5° · rotation` for the mounts `ground` and `ceiling`. The angle `A` is
     the yaw of `facing` for the mount `wall`.
  2. The scale `k` is `0.010416667` for the family `standing` and `0.0140625` for the family
     `hanging`.
  3. For the family `standing`, let `W` be `T(0, −0.3125, −0.4375)` for the mount `wall` and the
     identity for the mount `ground`. The text matrix of the front side is
     `T(0.5, 0.5, 0.5) · Ry(−A) · W · T(0, 0.33333334, 0.046666667) · S(k, −k, k)`.
  4. For the family `hanging`, the text matrix of the front side is
     `T(0.5, 0.9375, 0.5) · Ry(−A) · T(0, −0.3125, 0) · T(0, −0.32, 0.073) · S(k, −k, k)`.
  5. The matrix of the back side has the extra factor `Ry(180°)` before the last translation,
     that is before `T(0, 0.33333334, 0.046666667)` or `T(0, −0.32, 0.073)`.
  6. The `attached` property of a `hanging` sign does not change the matrix.
  7. The text plane of a side is in front of that side. Its distance from the plane through the
     centre of the board is `0.046666667` block for `standing` and `0.073` block for `hanging`.
- **Postconditions:**
  - A `standing` sign with `rotation` 0 has the front matrix with the rows
    `(k, 0, 0, 0.5)`, `(0, −k, 0, 0.8333334)`, `(0, 0, k, 0.5466667)`. The front side faces south.
  - A `standing` wall sign with `facing` `south` has the front translation
    `(0.5, 0.5208334, 0.10916667)`.
  - A `hanging` wall sign with `facing` `south` has the front translation `(0.5, 0.305, 0.573)`.
  - A `hanging` sign with `rotation` 4 has the front translation `(0.427, 0.305, 0.5)`. Its
    front side faces west.
  - Every matrix agrees with the row `transform` of the sign artefact within `1e-6`.

Provenance: `source_suggested_black_box_confirmed` (the 80 rows `transform` of the sign artefact:
16 `ground` states, 4 `wall` states for `standing`, and 16 `ceiling` states, 4 `wall` states for
`hanging`, each with two sides). Item 6 is `source_suggested`. Kind: layout. Confidence: high.
Milestone: 1. Experiment: SIGN-EXP-004.

---

## 5. Colour

### SIGN-COL-001: Colours of the dyes

- **Inputs:** a dye colour of a side text.
- **Behavior:** each dye has a bright colour and a dark colour. The dark colour is the bright
  colour with each of the red, green and blue channels multiplied by `0.4` and rounded down.
  The table lists the ARGB values:

| Id | Dye | Bright | Dark |
|----|-----|--------|------|
| 0 | white | `0xFFFFFFFF` | `0xFF666666` |
| 1 | orange | `0xFFFF681F` | `0xFF66290C` |
| 2 | magenta | `0xFFFF00FF` | `0xFF660066` |
| 3 | light_blue | `0xFF9AC0CD` | `0xFF3D4C52` |
| 4 | yellow | `0xFFFFFF00` | `0xFF666600` |
| 5 | lime | `0xFFBFFF00` | `0xFF4C6600` |
| 6 | pink | `0xFFFF69B4` | `0xFF662A48` |
| 7 | gray | `0xFF808080` | `0xFF333333` |
| 8 | light_gray | `0xFFD3D3D3` | `0xFF545454` |
| 9 | cyan | `0xFF00FFFF` | `0xFF006666` |
| 10 | purple | `0xFFA020F0` | `0xFF400C60` |
| 11 | blue | `0xFF0000FF` | `0xFF000066` |
| 12 | brown | `0xFF8B4513` | `0xFF371B07` |
| 13 | green | `0xFF00FF00` | `0xFF006600` |
| 14 | red | `0xFFFF0000` | `0xFF660000` |
| 15 | black | `0xFF000000` | `0xFF000000` |

- **Postconditions:** these colours are not the colours of the dye items or of the banner tint
  (`block-entity-models.md`, BEM-BAKE-005).

Provenance: `black_box_confirmed` (the rows `dye_colors` of the sign artefact). Kind: layout.
Confidence: high. Milestone: 1.

### SIGN-COL-002: Colour and outline of a line

- **Inputs:** the dye colour and the glowing flag of the side text, and the outline range
  (SIGN-COL-003).
- **Behavior:**
  1. When the side text is not glowing, the default colour of the line is the dark colour of the
     dye. The line has no outline.
  2. When the side text is glowing, the default colour of the line is the bright colour of the dye.
  3. When the side text is glowing and the dye is `black`, the outline colour is `0xFFF0EBCC`.
     This does not depend on the outline range.
  4. When the side text is glowing, the dye is not `black` and the outline range holds, the
     outline colour is the dark colour of the dye.
  5. When the side text is glowing, the dye is not `black` and the outline range does not hold,
     the line has no outline.
  6. A code point whose style has a colour uses that colour instead of the default colour. The
     outline colour does not change.
  7. The client draws the line with no drop shadow and with no background.
  8. An outline is a copy of the line in the outline colour, at each of the eight offsets of one
     text unit around the line. The line in its colours is on top of the copies.
- **Postconditions:** a black sign with glowing text shows black text with a pale outline in
  every case.

Provenance: `source_suggested_black_box_confirmed` (the 64 rows `paint` of the sign artefact,
for items 1 to 5 and 7; the rows `styles`, for the style colour of item 6). Items 6 and 8 are
`source_suggested`. Kind: layout. Confidence: high for items 1 to 5 and 7, medium for items 6
and 8. Milestone: 1. Experiment: SIGN-EXP-005.

### SIGN-COL-003: The outline range

- **Inputs:** the block position of the sign, the camera entity of the client, the local player.
- **Behavior:** the outline range holds in these two cases:
  - the local player looks through a spyglass in the first person view;
  - the camera entity exists, and the squared distance from the camera entity to the centre of
    the block is less than 256.
- **Postconditions:** a glowing sign at 16 blocks or more shows no outline, if its dye is not
  `black`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: 1. Experiment:
SIGN-EXP-006.

---

## 6. Light

### SIGN-LIGHT-001: Light of a line

- **Inputs:** the glowing flag of the side text, and the packed light of the block position of
  the sign.
- **Behavior:**
  1. When the side text is glowing, the packed light of the line is 15728880 (`0x00F000F0`:
     block light 15, sky light 15).
  2. When the side text is not glowing, the packed light of the line is the packed light of the
     block position of the sign. The light is the block light and the sky light of that position,
     as `block-entity-models.md`, BEM-BAKE-004, item 1, states.
  3. The client draws the line with a depth offset toward the viewer. The line does not
     z-fight with the board.
- **Postconditions:** glowing text has the same brightness in a dark cave and in daylight.

Provenance: `source_suggested_black_box_confirmed` (the rows `paint` of the sign artefact, with
the packed light 3145808 as input; the row `constants`, `full_bright_light`). Item 3 is
`source_suggested`. Kind: semantic. Confidence: high. Milestone: 1. Experiment: SIGN-EXP-007.

---

## 7. Font

### SIGN-FONT-001: Font and width

- **Inputs:** a message with its styles.
- **Behavior:**
  1. A code point with no `font` in its style uses the default font of the game. A code point
     with a `font` in its style uses that font.
  2. The width `w` of a line is the sum of the advances of its code points, as an integer. Bold
     and the other styles change the advance as the font states.
  3. The wrap of SIGN-LINE-001 and the width `w` of SIGN-LAY-002 use the same widths.
- **Postconditions:** none.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: 1. The font does
not have a specification yet (`client-chat.md`, section 7).

### SIGN-FONT-002: Code points that the font lacks

- **Inputs:** a code point in a message.
- **Behavior:**
  1. The default font has three sources, in this order: the space glyphs, the bitmap glyphs of
     the game, and the Unifont glyphs (`font/default.json` of the client jar).
  2. A code point that none of the sources has draws as the missing glyph. The missing glyph
     is a hollow rectangle with a width of 5 texels and a height of 8 texels.
  3. The missing glyph has the colour and the style of the line like any other glyph.
  4. A line break (U+000A) in a message ends the wrap row (`client-tooltip.md`, section 0.3). It
     never reaches a line.
- **Postconditions:** the client draws every code point of a line. It never skips one.

Provenance: `source_suggested`. Kind: layout. Confidence: medium for items 1 and 3, low for
item 2. Milestone: 1. Experiment: SIGN-EXP-008.

---

## 8. Parity tests

### SIGN-TEST-001: Text against the sign artefact

- **Inputs:** the sign artefact.
- **Behavior:**
  1. For each row `transform`, compute the text matrix of SIGN-PLACE-001 and compare it with
     the row within `1e-6`.
  2. For each row `layout`, give each message the width of the row and compare `x` and `y` of
     each line.
  3. For each row `paint`, compute the colour, the outline colour and the packed light of
     SIGN-COL-002 and SIGN-LIGHT-001 for the dye, the glowing flag and the outline range of the
     row. Compare them with the row.
  4. For each row `dye_colors`, compare the bright and the dark colour.
  5. For each row `filtering`, give the side text of that row the filtering flag of the row, and
     compare the shown lines.
- **Postconditions:** the test fails when one value differs.

Provenance: `source_suggested_black_box_confirmed` (the sign artefact). Kind: semantic.
Confidence: high. Milestone: 1.

### SIGN-TEST-002: Screenshot of the signs

- **Inputs:** a world with one sign of each family and mount, in daylight, with the texts
  `Hello`, `Wide wide wide wide wide wide wide` and an empty line on one side, and a glowing
  text of a coloured dye on the other side.
- **Behavior:**
  1. Take a screenshot in the reference and in the client, at the same camera position and the
     same field of view, once from each side.
  2. Compare the two images.
- **Postconditions:** the outline of each text agrees within one pixel at 1920 × 1080. The
  colour of the text agrees within 3 of 255 in each channel.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Milestone: 1.

---

## 9. Intentional deviations

No deviation is recorded.

---

## 10. Unresolved

1. SIGN-LINE-001, items 3 and 4: the wrap of a message. The artefact does not wrap. The rule
   takes the wrap of `client-tooltip.md`. Open: the break of a word that is wider than `m`, and a
   first glyph that is wider than `m`.
2. SIGN-FONT-001, item 2: the width of a line as an integer. Open: whether the width of a line
   is rounded up or down when the advances are not whole numbers (bold text, scaled glyphs).
3. SIGN-FONT-002, item 2: the advance of the missing glyph. The width and the height are 5 and 8
   texels. The advance is probably 6, and no artefact shows it.
4. SIGN-COL-002, item 8: the exact offsets and the draw order of the outline copies.
5. SIGN-LIGHT-001: whether the line has the shade of a face. The rule states only the light.
6. SIGN-SIDE-001, item 3: the text of a side shows through the board only in the case of
   a viewer behind the board. The text planes are near the board, and no artefact shows the depth
   test.
7. The distance at which a sign stops to draw its text. The rule set has none.

---

## 11. Experiments

None of these experiments has been run. Each one is a proposal.

- **SIGN-EXP-001:** put an oak sign. Write the text `FRONT` on the front side and `BACK` on the
  back side. Look at the sign from both sides. Each viewer sees only its own text.
- **SIGN-EXP-002:** write `0123456789 0123456789 0123456789` on one line of a sign. Only the
  first row of the wrap shows. Repeat with a hanging sign, where the row is shorter. Turn on the
  profanity filter of the account in a server that stores filtered messages. The filtered text
  shows.
- **SIGN-EXP-003:** write `i`, `ii`, `iii` and `iiii` on the four lines. Take a screenshot with
  the camera in front of the sign. Each line is centred, with the shift to the right that
  SIGN-LAY-002 states for an odd width.
- **SIGN-EXP-004:** put 16 standing signs with `rotation` 0 to 15, 4 wall signs, 16 hanging signs
  and 4 wall hanging signs. Take a screenshot of each from the front and from the back. The text
  stays on the side where the dump places it.
- **SIGN-EXP-005:** apply the 16 dyes to signs, once with glow ink and once with no glow ink.
  Sample the centre of a thick glyph of each. Compare it with SIGN-COL-001.
- **SIGN-EXP-006:** put a glowing red sign. Walk from 20 blocks to 10 blocks. The outline shows
  when the distance is less than 16 blocks. Look at the same sign from 20 blocks through a
  spyglass. The outline shows.
- **SIGN-EXP-007:** put a glowing sign and a normal sign in a dark room. Only the glowing text
  stays bright. Put the normal sign next to a torch. Its text is brighter.
- **SIGN-EXP-008:** write the code point U+E000 (private use) on a sign. Zoom in on a screenshot.
  The missing glyph shows. Measure its size and the distance to the next glyph.
