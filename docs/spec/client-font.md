# The client font: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`.

This document states how the vanilla client turns a code point into a glyph. It states the font
definition files, the providers that make glyphs, the order in which the client searches them,
the advance and the box of a glyph, the missing glyph, the bold and shadow offsets, and the width
of a text. It does not state the shapes of the glyphs: these are the pixels of the texture files
and of the Unifont data. The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **GUI unit**: as `client-chat.md`, section 0.1, defines it.
- **Font**: a named set of glyphs. A font has an ID, for example `minecraft:default`. The
  `font` field of a text style selects it. A style with no `font` field selects
  `minecraft:default`.
- **Font definition file**: a JSON file `font/<path>.json` of a pack. It defines the font
  `<namespace>:<path>`. It has the list `providers`.
- **Provider**: one entry of `providers`. It has a `type` and an optional `filter`. Its kinds are
  `bitmap`, `space`, `unihex`, `reference` and `ttf`.
- **Glyph source**: a provider that can give a glyph for a code point. A `reference` provider is
  not a glyph source. It is replaced by the glyph sources of another font (FONT-PROV-001).
- **Glyph**: what a glyph source gives for a code point. A glyph has an advance, a bold offset, a
  shadow offset and, optionally, an image with a box.
- **Advance**: the distance, in GUI units, from the pen position of a glyph to the pen position
  of the next glyph.
- **Pen position**: the point (`gx`, `gy`) where a glyph starts. `gy` is the top of the line.
  The line has the height 9 (`client-chat.md`, section 0.3).
- **Box**: the rectangle (`left`, `right`, `top`, `bottom`) of the image of a glyph, in GUI
  units from the pen position.
- **Search order**: the list of the glyph sources of a font, in the order in which the client
  asks them for a code point (FONT-ORD-001).
- **Missing glyph**: the glyph that the client uses when no glyph source of the font has the
  code point (FONT-GLY-001).
- **Font option**: a switch that a player controls. `uniform` is the option "Force Unicode
  Font". `jp` is the option "Japanese Glyph Variants". A set of options is the set of the
  switches that are on.
- **Cell**: a rectangle of a texture file that holds one glyph of a `bitmap` provider.
- **Ink**: a pixel of a cell whose alpha value is not 0.
- **Dump**: the file `crates/mcrs_minecraft_client_gui/tests/fixtures/vanilla/font.json`. The
  task `dumpFont` of `tools/vanilla-oracle` writes it from the game. The sections below name its
  parts. `configs[0]` is the font `minecraft:default` with no option. `configs[1]` is the same
  font with `uniform`. `configs[2]` is the same font with `jp`. `configs[3]` is the font
  `minecraft:uniform` with no option.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". A rule that cites the dump has the label
`source_suggested_black_box_confirmed` for the part that the dump shows. The other parts of the
rule stay `source_suggested`, and the rule names them.

### 0.3 Numbers

A box, an advance and an offset are in GUI units. The client computes them in 32-bit floating
point numbers. Every advance and offset in the vanilla fonts is a multiple of 0.5, so the sums
in this document are exact.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `FONT-DEF` | font definition files and where the vanilla files come from |
| `FONT-PROV` | the kinds of provider and their fields |
| `FONT-ORD` | the search order and the choice of a glyph |
| `FONT-GLY` | the missing glyph and the box of a glyph |
| `FONT-DRAW` | the offsets of bold, shadow and italic |
| `FONT-WID` | the width of a text |
| `FONT-SET` | the code points of the vanilla default font |
| `FONT-DEV` | intentional deviations |

### 0.5 Scope

In scope: the loading of font definition files, the glyph sources that the vanilla fonts use,
the choice of a glyph, the advance and the box of a glyph, and the width of a text.

Out of scope:

- The pixels of the glyphs. They are data of the texture files and of the Unifont archives.
- The `ttf` provider. The vanilla fonts do not use it (FONT-PROV-009).
- The obfuscated style, which draws a random glyph of the same width.
- The right-to-left shaping of text.
- The wrap of a text component. `client-chat.md`, section 7, lists it.
- The colour of a shadow and the lines of underline and strikethrough.

---

## 1. Font definition files

### FONT-DEF-001: Files and the stack of a font

- **Inputs:** the resource packs of the client, from the lowest to the highest priority.
- **Behavior:**
  1. A file `assets/<namespace>/font/<path>.json` of a pack defines the font `<namespace>:<path>`.
     The path can have a `/`, for example `include/space`.
  2. The client reads the file of every pack that has it. The files of one font are the stack of
     that font.
  3. A file is a JSON object with the field `providers`, a list of providers.
  4. When a file is not valid JSON, or one provider in it has a field that is not valid, the
     client ignores that whole file and writes a warning. The files of the other packs for the
     same font still count.
  5. When a provider is valid but the client cannot read its data (a missing texture, an archive
     that is not readable, a line of Unifont data that is not valid), the font does not load.
     The client writes a warning.
  6. The glyph sources of the stack are in the search order of FONT-ORD-001.
- **Postconditions:** a font that does not load acts as a font with no definition. A font that
  references it does not load either (FONT-PROV-001, step 4). The client cannot start when
  `minecraft:default` does not load.
- **Edge cases:** a style that names a font with no definition draws every code point as the
  missing glyph.

Provenance: `source_suggested`. Kind: client agreement. Confidence: high. Experiment: add a
pack with a `font/default.json` that is not valid JSON. The text keeps the vanilla glyphs, and
the log has a warning.

### FONT-DEF-002: Where the vanilla files come from

- **Behavior:**
  1. The client jar holds `font/default.json`, `font/uniform.json`, `font/alt.json`,
     `font/illageralt.json`, `font/include/space.json`, `font/include/default.json` and the
     texture files `font/ascii.png`, `font/accented.png`, `font/nonlatin_european.png`,
     `font/ascii_sga.png` and `font/asciillager.png`.
  2. The jar also holds a file `font/include/unifont.json` with an empty list `providers`. It
     is a placeholder.
  3. The asset index of the game version holds the real `font/include/unifont.json`, the
     archives `font/unifont.zip`, `font/unifont_jp.zip` and `font/unifont_pua.zip`, and
     `font/include/unifont_pua.json`. The client has two pack layers: the jar, and above it
     the asset index. The file of the index layer is the one that provides glyph sources, so
     the stack of `include/unifont` has two files (FONT-DEF-001) and only the second one
     has providers.
  4. A client that reads only the jar has no glyph for the code points that only the Unifont
     data has.
- **Postconditions:** the dump shows the effect. In `configs[0]`, the provider with index 4 is
  a `unihex` provider from `minecraft:font/unifont.zip` with 114432 glyphs.

Provenance: `source_suggested_black_box_confirmed`. Kind: client agreement. Confidence: high.
Evidence: `crates/mcrs_minecraft_client_jar/src/font_hint.json` gives the size 29 for the file
`include/unifont.json` of the jar; the dump `configs[0].providers`. Experiment: none needed.

### FONT-DEF-003: The vanilla definitions

- **Behavior:** the vanilla files define these fonts:

| Font | Providers, in file order |
|------|--------------------------|
| `minecraft:include/space` | `space` with the advances U+0020 → 4 and U+200C → 0 |
| `minecraft:include/default` | `bitmap` `font/nonlatin_european.png` (height 8, ascent 7, 67 rows of 16 characters); `bitmap` `font/accented.png` (height 12, ascent 10, 75 rows of 16); `bitmap` `font/ascii.png` (height 8, ascent 7, 16 rows of 16) |
| `minecraft:include/unifont` | `unihex` `font/unifont_jp.zip` with filter `jp` true and two size overrides; `unihex` `font/unifont.zip` with nine size overrides |
| `minecraft:default` | `reference` `include/space`; `reference` `include/default` with filter `uniform` false; `reference` `include/unifont` |
| `minecraft:uniform` | `reference` `include/space`; `reference` `include/unifont` |

  The size overrides are those of FONT-PROV-008. `minecraft:alt` and `minecraft:illageralt`
  are fonts of the enchanting table and of the illager runes. They use `space` and one
  `bitmap` provider each, with the files `ascii_sga.png` and `asciillager.png`. The font
  `include/unifont_pua` has one `unihex` provider. No vanilla font references it.

Provenance: `source_suggested_black_box_confirmed`. Kind: client agreement. Confidence: high.
Evidence: the dump `configs[*].providers`, which lists the providers that remain after
FONT-ORD-001. Experiment: none needed.

---

## 2. Providers

### FONT-PROV-001: The reference provider

- **Inputs:** a provider `{"type": "reference", "id": "<font id>", "filter": ...}`.
- **Behavior:**
  1. The provider is replaced, in place, by the glyph sources of the font `id`, in the order in
     which that font lists them.
  2. Each replaced glyph source gets the filter of the reference merged into its own filter
     (FONT-PROV-002).
  3. The font `id` loads before the font that references it.
  4. When the font `id` does not exist, fails to load, or is part of a reference cycle, the
     referencing font does not load. The client writes a warning.
- **Postconditions:** the effective list of the font `minecraft:default` is the list of
  `include/space`, then the list of `include/default`, then the list of `include/unifont`.

Provenance: `source_suggested`. Kind: client agreement. Confidence: high. Experiment: create a
font `example:a` that references `example:b` and `example:b` that references `example:a`. Use
`example:a` in a text style. Every code point draws as the missing glyph, and the log has a
warning.

### FONT-PROV-002: The filter of a provider

- **Inputs:** the field `filter` of a provider: an object that maps an option name to a
  Boolean. The names are `uniform` and `jp`. The field is optional. A missing filter passes
  always. An unknown name makes the provider not valid.
- **Behavior:**
  1. The provider is active when, for every entry, "the option is on" equals the value of the
     entry.
  2. When a `reference` has a filter, each entry of that filter replaces the entry with the
     same name in the filter of a glyph source of the referenced font.
  3. The client checks the filters again each time a player changes a font option. No reload
     of the resource packs is needed.
- **Postconditions:** with no option on, a provider with `"uniform": false` is active and a
  provider with `"jp": true` is not. With `uniform` on, the providers of `include/default` are
  not active.

Provenance: `source_suggested_black_box_confirmed`. Kind: client agreement. Confidence: high.
Evidence: the dump. `configs[0]` has the bitmap providers and the one Unifont provider.
`configs[1]` has only the Unifont provider and `space`. `configs[2]` has the extra provider
`unifont_jp.zip` before `unifont.zip`. `configs[3]` has the same providers as `configs[1]`.
Experiment: for step 3, change "Force Unicode Font" in the options with the chat open. The
letters change at once.

### FONT-PROV-003: The space provider

- **Inputs:** `{"type": "space", "advances": {"<character>": <number>, ...}}`.
- **Behavior:**
  1. Each key is a string of exactly one code point. Each value is a number, which can have a
     fraction and can be 0 or negative.
  2. For each key, the provider gives a glyph with that advance and no image.
  3. The bold offset and the shadow offset of the glyph are 1.
- **Postconditions:** in the dump, U+0020 has the advance 4 and the bold advance 5. U+200C has
  the advance 0 and the bold advance 1.

Provenance: `source_suggested_black_box_confirmed`. Kind: client agreement. Confidence: high.
Evidence: the dump `configs[0].glyphs` for U+0020 and U+200C. The strings `a U+200C b` have
the width 12. The parts about a fraction and a negative number are `source_suggested`.
Experiment: define `{"A": 2.5}` in a space provider that is first in `minecraft:default`. The
text `AA` has the width 5.

### FONT-PROV-004: The bitmap provider: fields

- **Inputs:** `{"type": "bitmap", "file": <id>, "height": <int>, "ascent": <int>, "chars":
  [<string>, ...]}`.
- **Behavior:**
  1. `file` is a resource ID. The texture file is `textures/` plus the path of the ID, for
     example `minecraft:font/ascii.png` is `assets/minecraft/textures/font/ascii.png`.
  2. `height` is optional and has the default 8.
  3. `ascent` is required. It must not be greater than `height`. It can be negative.
  4. `chars` is a non-empty list of rows. Each row is a string. All rows have the same number
     of code points, counted as code points and not as UTF-16 units. A row that has another
     length makes the provider not valid.
  5. The code point U+0000 in a row means "no glyph in this cell".
  6. When a code point is in two cells of one provider, the later cell in reading order (row by
     row, left to right) gives the glyph.
  7. The texture must have an alpha channel or a luminance channel. A texture with only colour
     channels, or a texture that the client cannot read, makes the provider fail to load
     (FONT-PROV-001, step 4).
- **Postconditions:** the grid of `chars` has `R` rows and `C` columns. The cell of the code
  point in row `r` and column `c` is the cell (`c`, `r`) of the texture, counted from 0.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: add a
pack with a bitmap provider with `ascent` 9 and `height` 8. The client ignores the file and
writes a warning.

### FONT-PROV-005: The bitmap provider: cells and advance

- **Inputs:** a `bitmap` provider and its texture of `W` × `H` pixels, with `C` columns and `R`
  rows in `chars`.
- **Behavior:**
  1. The cell width `cw` is `W / C` and the cell height `ch` is `H / R`. Both are integer
     divisions that drop the remainder.
  2. The scale `s` is `height / ch`, as a 32-bit floating point number.
  3. The ink width `iw` of a cell is the number of the columns up to and including the
     rightmost column that has ink in the cell, counted from the left edge of the cell. A cell
     with no ink has `iw` = 0.
  4. The advance of the glyph is `floor(0.5 + iw · s) + 1`.
  5. The bold offset and the shadow offset of the glyph are 1.
  6. The glyph has an image: the cell, drawn with the box of FONT-GLY-002.
- **Postconditions:**
  - For `s` = 1, the advance is `iw + 1`. A cell with no ink gives the advance 1.
  - The advance is always an integer.
  - Leading blank columns count: the advance does not depend on the left side of the ink.
- **Edge cases:** a texture whose size is not a multiple of the grid loses the remainder at
  the right and at the bottom.

Provenance: `source_suggested_black_box_confirmed`. Kind: client agreement. Confidence: high
for the advance, medium for the edge case. Evidence: the dump `configs[0].bitmap_advances` has
the advance of every glyph of the three bitmap providers (indexes 1, 2 and 3). An
implementation must give these values from the texture files. Experiment: for the edge case,
make a texture 3 pixels wider than the grid. The glyphs stay the same.

### FONT-PROV-006: The unihex provider: fields

- **Inputs:** `{"type": "unihex", "hex_file": <id>, "size_overrides": [...]}`.
- **Behavior:**
  1. `hex_file` is a resource ID. The file is read as is, with no `textures/` prefix. It is a
     ZIP archive.
  2. `size_overrides` is optional and is empty by default.
  3. The client reads every entry of the archive whose name ends with `.hex`, in archive order.
     The other entries are ignored.
  4. A glyph that two lines define gets the data of the later line.
- **Postconditions:** the glyph sources of the provider are the code points that the lines of
  the `.hex` entries define.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: add a
pack with a `unihex` provider that names a file that is not a ZIP. The font fails to load.

### FONT-PROV-007: Unifont data and the bounds of a glyph

- **Inputs:** the lines of a `.hex` entry.
- **Behavior:**
  1. A line is `<code point>:<data>` and ends with a line feed. The code point has 4, 5 or 6
     hexadecimal digits. The digits are the characters 0 to 9 and A to F. Lower case letters
     are not valid.
  2. The data has 32, 64, 96 or 128 hexadecimal digits. These are 16 rows of 8, 16, 24 or 32
     pixels. The first row is the top row. The most significant bit of a row is the leftmost
     pixel.
  3. A line that does not fit these rules makes the provider fail to load.
  4. The columns of a glyph are numbered from 0 at the left. The bounds `left` and `right` of
     a glyph are the leftmost and the rightmost column that has a set pixel in any of the 16
     rows.
  5. A glyph with no set pixel has `left` = 0 and `right` = the number of columns of its row
     (8, 16, 24 or 32). It is one column wider than the data.
  6. The width `w` of the glyph is `right − left + 1` columns.
  7. The advance of the glyph is `w / 2 + 1`, with an integer division that drops the
     remainder. The bold offset and the shadow offset are 0.5.
  8. The image of the glyph has `w` columns and 16 rows. It starts at the column `left`. The
     box is that of FONT-GLY-003.
- **Postconditions:** in the dump, `configs[0].unihex_widths` lists the pixel widths of all
  114432 glyphs of `unifont.zip`. They are from 1 to 17, and only two glyphs have 17. The
  width 17 occurs only for a blank glyph of 16 columns (step 5). U+3000 is the first one, and
  its advance is 9. The advance of every glyph of the dump with a Unifont image is
  `⌊w / 2⌋ + 1`, and its box ends at `w / 2`.

Provenance: `source_suggested_black_box_confirmed`. Kind: client agreement. Confidence: high for
steps 5 to 7, medium for steps 1 to 4 and 8. Evidence: the dump `glyphs` (for example U+00A0:
width 9, advance 5; U+1F600: width 15, advance 8) and `unihex_widths`. Experiment: add a
`.hex` line `10000:` followed by 32 zeros to an archive and draw U+10000. The advance is 5.

### FONT-PROV-008: Size overrides

- **Inputs:** `size_overrides`, a list of objects `{"from": <char>, "to": <char>, "left": <int>,
  "right": <int>}`. `from` and `to` are strings of one code point. `from` must be less than
  `to`; otherwise the provider is not valid.
- **Behavior:**
  1. The client handles the overrides in list order.
  2. For each code point from `from` to `to` that the data defines and no earlier override
     took, the glyph gets `left` and `right` from the override. The bounds of FONT-PROV-007,
     step 4, are not used for it.
  3. A code point that the data does not define gets no glyph.
- **Postconditions:** in the vanilla `unifont.zip`, the ranges U+3001 to U+30FF, U+3200 to
  U+9FFF, U+1100 to U+11FF, U+3130 to U+318F, U+A960 to U+A97F, U+D7B0 to U+D7FF, U+F900 to
  U+FAFF and U+FF01 to U+FF5E use `left` 0 and `right` 15. They have the width 16 and the advance
  9. The range U+AC00 to U+D7AF uses `left` 1 and `right` 15. It has the width 15 and the
  advance 8. `unifont_jp.zip` uses `left` 0 and `right` 15 for U+3200 to U+9FFF and for
  U+F900 to U+FAFF.

Provenance: `source_suggested_black_box_confirmed`. Kind: client agreement. Confidence: high.
Evidence: the dump `configs[0].glyphs`: U+4E00 has the width 16 and the advance 9, U+AC00 has
the width 15 and the advance 8. Experiment: define an override `{"from": "A", "to": "B",
"left": 0, "right": 3}` on a Unifont provider that comes first. The glyph U+0041 has the advance
3.

### FONT-PROV-009: The ttf provider

- **Behavior:** the format has a provider `ttf` that reads a TrueType font. The vanilla fonts
  of this version do not use it. This document does not state it.

Provenance: `unresolved`. Kind: unspecified. Confidence: n/a.

---

## 3. Search order and choice of a glyph

### FONT-ORD-001: Search order

- **Inputs:** the stack of a font (FONT-DEF-001), the set of font options.
- **Behavior:**
  1. Take the files of the stack from the highest pack to the lowest.
  2. In each file, take the providers in file order. Replace each `reference` by the glyph
     sources of the referenced font, in the search order of that font.
  3. Remove each glyph source whose filter does not pass (FONT-PROV-002).
  4. Add the fallback source at the end. It has the missing glyph for every code point.
- **Postconditions:** the search order of `minecraft:default` with no option is: the space
  provider, `nonlatin_european.png`, `accented.png`, `ascii.png`, `unifont.zip`, the fallback
  (the dump `configs[0].providers`, with `fallback_index` 5). With `jp` on, the provider
  `unifont_jp.zip` is before `unifont.zip`.

Provenance: `source_suggested_black_box_confirmed`. Kind: client agreement. Confidence: high.
Evidence: the dump `configs[*].providers`. The part about several packs is `source_suggested`.
Experiment: add a pack with `font/default.json` that has one `bitmap` provider for the letter
`A`. The letter `A` shows from the pack. The letter `B` is as before.

### FONT-ORD-002: The glyph of a code point

- **Inputs:** a font, its options and a code point.
- **Behavior:**
  1. Ask the glyph sources in the search order. The first source that has a glyph gives it.
  2. The fallback source has the missing glyph for every code point, so a glyph always exists.
  3. The client has two ways to read a font. The first way uses the first glyph found. The
     second way skips a glyph whose advance, or whose bold advance, is less than 0 or greater
     than 32. It then uses the next glyph that is not skipped. If there is none, it uses the
     missing glyph.
- **Postconditions:** the first source wins even when a later source has a better shape. In
  the dump, U+00C9 has the glyph of `accented.png` (index 2), U+0404 has the glyph of
  `nonlatin_european.png` (index 1), and U+0041 has the glyph of `ascii.png` (index 3).
  The dump `configs[0].winners` lists, for each code point of the basic plane, the index of the
  source that wins.
- **Edge cases:** no glyph of the vanilla default font has an advance outside the limits of
  step 3: the lowest advance is 1 and the highest bold advance is 11 (the dump `ranges`).
  Step 3 changes no result for the vanilla fonts.

Provenance: `source_suggested_black_box_confirmed`. Kind: client agreement. Confidence: high
for steps 1 and 2, low for step 3. Evidence: the dump `winners` and `glyphs`. Step 3 is
`source_suggested`. Experiment: see section 9, the first item.

---

## 4. The missing glyph and the box of a glyph

### FONT-GLY-001: The missing glyph

- **Behavior:**
  1. The missing glyph is an image of 5 × 8 pixels. A pixel is set at the border and not set
     inside:

     ```
     #####
     #...#
     #...#
     #...#
     #...#
     #...#
     #...#
     #####
     ```

  2. Its advance is 6. Its bold offset and shadow offset are 1.
  3. Its box is `left` 0, `right` 5, `top` 0, `bottom` 8.
  4. A code point that no source has gets this glyph. In the dump, U+D800, U+E000 and
     U+F0000 get it.
- **Postconditions:** the text `U+E000` has the width 6.

Provenance: `source_suggested_black_box_confirmed`. Kind: layout. Confidence: high. Evidence:
the dump `special_glyphs.missing` and `configs[0].glyphs`. Experiment: none needed.

### FONT-GLY-002: The box of a bitmap glyph

- **Inputs:** a glyph of a `bitmap` provider with `height`, `ascent`, and a cell of `cw` × `ch`
  pixels (FONT-PROV-005).
- **Behavior:**
  1. `left` = 0.
  2. `right` = `cw · s`, with `s = height / ch`.
  3. `top` = `7 − ascent`.
  4. `bottom` = `top + ch · s`, which is `top + height`.
  5. The box is relative to the pen position (`gx`, `gy`), with `gy` the top of the line.
- **Postconditions:** for `ascii.png` and `nonlatin_european.png` (height 8, ascent 7, cell 8
  × 8) the box is 0, 8, 0, 8. For `accented.png` (height 12, ascent 10, cell 9 × 12) the box
  is 0, 9, −3, 9. The glyph draws 3 units above the top of the line, so the accent fits.

Provenance: `source_suggested_black_box_confirmed`. Kind: layout. Confidence: high. Evidence:
the dump `configs[0].glyphs`: `pixels` and `box` of U+0041, U+0404 and U+00C9. Experiment:
none needed.

### FONT-GLY-003: The box of a Unifont glyph

- **Behavior:**
  1. `left` = 0. The leading blank columns are not drawn, so the first set column is at `gx`.
  2. `right` = `w / 2`. This is a fraction when `w` is odd.
  3. `top` = 0 and `bottom` = 8.
- **Postconditions:** a glyph with `w` = 15 has the box 0, 7.5, 0, 8 and the advance 8, so the
  image ends 0.5 before the next pen position.

Provenance: `source_suggested_black_box_confirmed`. Kind: layout. Confidence: high for the
numbers, medium for the crop of the leading blank columns. Evidence: the dump `glyphs`:
U+AC00 has the box 0, 7.5, 0, 8 and the image width 15. The crop of the columns is
`source_suggested`. Experiment: draw U+0021 next to the letter `A`. The exclamation mark starts
at the pen position, with no gap on its left.

---

## 5. Bold, shadow and italic

### FONT-DRAW-001: Offsets of bold and shadow

- **Inputs:** a glyph with an image, the pen position (`gx`, `gy`), the bold offset `b` and the
  shadow offset `d` of the glyph (FONT-PROV-003, 005, 007).
- **Behavior:**
  1. The advance of a bold glyph is its advance plus `b`.
  2. A bold glyph draws its image two times: at (`gx`, `gy`) and at (`gx + b`, `gy`). Each
     time the box is wider by 0.1 on each side and taller by 0.1 on the top and the bottom.
  3. A glyph with a shadow first draws the shadow, below the glyph. The shadow is the image at
     (`gx + d`, `gy + d`). A bold glyph has two shadow images: at (`gx + d`, `gy + d`) and at
     (`gx + b + d`, `gy + d`).
  4. For a glyph with no image, for example a space, steps 2 and 3 draw nothing. Step 1 holds.
- **Postconditions:** the vanilla values are:

| Source | Bold offset | Shadow offset | Bold advance |
|--------|-------------|---------------|--------------|
| `bitmap`, `space`, missing glyph | 1 | 1 | advance + 1 |
| `unihex` | 0.5 | 0.5 | advance + 0.5 |

Provenance: `source_suggested_black_box_confirmed`. Kind: layout. Confidence: high for the
table and step 1, medium for steps 2 and 3. Evidence: the dump `glyphs`: `bold_offset`,
`shadow_offset` and `advance_bold`. Steps 2 and 3 are `source_suggested`. Experiment: draw
bold `日` with a shadow at GUI scale 4. The shadow is 2 pixels right and down. The bold strike
is 2 pixels to the right.

### FONT-DRAW-002: Italic

- **Behavior:**
  1. An italic glyph shears its box. The top edge moves right by `1 − 0.25 · top` and the
     bottom edge moves right by `1 − 0.25 · bottom`, where `top` and `bottom` are the values
     of the box (FONT-GLY-002, 003).
  2. The advance does not change.
- **Postconditions:** for a box with `top` 0 and `bottom` 8, the top edge moves right by 1 and
  the bottom edge moves left by 1.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: draw italic `I`
at GUI scale 4 and measure the slant of the stem.

---

## 6. The width of a text

### FONT-WID-001: Width of a text

- **Inputs:** a text with its styles, and the fonts of the styles.
- **Behavior:**
  1. Split the text into code points. In a plain string, a `§` and the formatting character
     after it change the style. They are not code points of the width. A lone surrogate
     counts as U+FFFD.
  2. For each code point, take the glyph in the font of its style (FONT-ORD-002).
  3. The advance of a code point is the advance of the glyph, or the bold advance when the
     style is bold.
  4. The exact width is the sum of the advances, in text order, as a 32-bit floating point
     number.
  5. The width of the text is the exact width rounded up to an integer.
- **Postconditions:**
  - The rounding happens one time for the whole text. It does not happen for each part or for
    each glyph.
  - An empty text has the width 0.
  - A line feed, a tab and the other control characters are code points with a glyph. Their
    glyph is that of the Unifont. The code points U+0000 to U+001F have the advance 9. Only
    U+0020 and U+200C come from the space provider.
- **Examples:** these values are from the dump `configs[0].strings`.

| Text | Exact width | Width |
|------|-------------|-------|
| `Hello, World!` | 59 | 59 |
| `Привет, мир!` | 62 | 62 |
| `日本語` | 27 | 27 |
| `a U+200C b` | 12 | 12 |
| `a U+D800 b` | 20 | 20 |
| `§lBold` | 25 | 25 |
| bold `日` | 9.5 | 10 |
| bold `日本` | 19 | 19 |
| bold `日本語` | 28.5 | 29 |
| `ab` plain, `日` bold, `本` plain | 30.5 | 31 |
| `日` bold three times | 28.5 | 29 |

Provenance: `source_suggested_black_box_confirmed`. Kind: layout. Confidence: high for steps 2
to 5, medium for step 1. Evidence: the dump `configs[0].strings`; each row has `sum` (the exact
width), `width`, and for plain strings `string_width`, the width of the same text as a string.
Experiment: for step 1, put `§l` in a sign line and compare the width with a bold style.

---

## 7. The code points of the vanilla default font

### FONT-SET-001: Which code points have a glyph

- **Behavior:** with no option, the vanilla default font gives each code point of the basic
  plane to these sources:

| Source | Code points (count, basic plane) |
|--------|----------------------------------|
| space (index 0) | U+0020, U+200C |
| `ascii.png` (index 3) | 165, among them U+0021 to U+007E and U+00A3, U+00F7 |
| `accented.png` (index 2) | 1189 |
| `nonlatin_european.png` (index 1) | 1018 |
| Unifont (index 4) | 54712 |
| the missing glyph (index 5) | 8450 |

  The count of the basic plane is the sum of the table: 65536.
  1. The Unifont gives the glyph to the code points U+0000 to U+001F, to U+007F to U+00A0
     (the dump shows U+00A0 with the advance 5), and to every other code point of the basic
     plane that no earlier source has and the Unifont data has.
  2. The code points from U+D800 to U+F8FF (the surrogates and the private use area) and
     U+FFFE and U+FFFF have the missing glyph.
  3. The Unifont data also has glyphs in the planes 1 and above. The dump `glyphs` shows that
     U+1F600, U+20000 and U+1D400 have a glyph, and that U+F0000 and U+10FFFF do not.
  4. A code point of the private use area never has a glyph in the vanilla fonts, because no
     vanilla font references `include/unifont_pua`.
- **Postconditions:** the Cyrillic letters U+0400 to U+045F, including `Ё` (U+0401), `ё`
  (U+0451), `Ї` (U+0407), `Є` (U+0404), `І` (U+0406) and `Ґ` (U+0490), have a bitmap glyph.
  The letters `Й` (U+0419) and `й` (U+0439) come from `accented.png`. The Greek letters
  U+0391 to U+03C9 come from the two bitmap files `nonlatin_european.png` and
  `accented.png`. The CJK ideographs, the Hangul syllables
  and the kana come from the Unifont, with the advance 8 or 9.

Provenance: `source_suggested_black_box_confirmed`. Kind: layout. Confidence: high. Evidence:
the dump `configs[0].winners`, `ranges` and `glyphs`. Experiment: none needed.

### FONT-SET-002: The options change the set

- **Behavior:**
  1. With `uniform` on, or for the font `minecraft:uniform`, the search order is the space
     provider, `unifont.zip`, the fallback. Every Latin, Cyrillic and Greek letter then has
     a Unifont glyph. In the dump, `A`, `É`, `Ё`, `Й` and `α` have the advance 4, and a
     bold one has 4.5.
  2. With `jp` on, `unifont_jp.zip` comes before `unifont.zip` and gives 9813 glyphs, from
     U+3303 to U+3106C.
- **Postconditions:** the dump `configs[1]` and `configs[3]` have the same providers.

Provenance: `source_suggested_black_box_confirmed`. Kind: layout. Confidence: high. Evidence:
the dump `configs[1..3]`. Experiment: none needed.

---

## 8. Intentional deviations

No deviation is recorded.

---

## 9. Unresolved

- FONT-ORD-002, step 3: the client has a second way to read a font that skips a glyph with an
  advance outside 0 to 32. This document does not say which texts use it. The vanilla fonts
  are not affected.
- FONT-PROV-009: the `ttf` provider.
- The colour of the shadow: its value for each channel, and how the client rounds it.
- The rules for the lines of underline and strikethrough.
- The obfuscated style. It replaces a glyph with a random glyph of the same width, and uses a
  random number source of the client. A parity test cannot compare it.
- The right-to-left shaping of text and the order of the glyphs of right-to-left text.
- The exact text of the warnings that the client writes (FONT-DEF-001, FONT-PROV-001).

## 10. Experiments

Each experiment is in the rule that it tests. None has been run by hand. The dump is the output
of the game for the rules that cite it.
