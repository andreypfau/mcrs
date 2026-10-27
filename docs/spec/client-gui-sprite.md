# GUI sprites of the client: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

Source of these values: `assets/minecraft/version.json`.

This document states how the vanilla client reads the scaling of a GUI sprite from the resource
pack, and how it draws a GUI sprite into a rectangle. The tooltip (`client-tooltip.md`), the
hotbar and each other part of the GUI use these rules. The document does not prescribe an
architecture.

## 0. Conventions

### 0.1 Terms

- **GUI unit**: as `client-chat.md`, section 0.1, defines it.
- **GUI sprite**: an image of the GUI atlas. The name `ns:p` of a GUI sprite is the file
  `assets/ns/textures/gui/sprites/p.png` of the resource pack.
- **Sprite metadata**: the file with the same path as the image and the extra extension
  `.mcmeta`. It is JSON.
- **Scaling**: the value of the field `gui.scaling` of the sprite metadata (GSPR-META-001).
- **Target**: the rectangle of `PW` × `PH` GUI units into which a screen draws a GUI sprite.
- **Missing sprite**: the sprite that the client uses for a name that has no loaded GUI sprite
  (GSPR-MISS-001).

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents".

### 0.3 Colours

A colour is a 32-bit ARGB value in hexadecimal.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `GSPR-META` | the sprite metadata and its errors |
| `GSPR-MISS` | the missing sprite |
| `GSPR-DRAW` | how each scaling draws into a target |
| `GSPR-DEV` | intentional deviations |

### 0.5 Scope

In scope: the `gui` section of the sprite metadata, the three scaling types, and the missing
sprite.

Out of scope: the animation and the `texture` sections of the sprite metadata, the layout of
the atlas, and the tint colour of a draw.

---

## 1. Sprite metadata

### GSPR-META-001: The scaling of a GUI sprite

- **Inputs:** the sprite metadata of a GUI sprite. The file is optional.
- **Behavior:**
  1. When the file is absent, or it has no `gui` object, or `gui` has no `scaling`, the scaling
     is `stretch`.
  2. `scaling` is an object with the field `type`. The type selects the other fields:

| `type` | Field | JSON type | Constraint | Default |
|--------|-------|-----------|------------|---------|
| `stretch` | none | | | |
| `tile` | `width` | integer | > 0 | required |
| `tile` | `height` | integer | > 0 | required |
| `nine_slice` | `width` | integer | > 0 | required |
| `nine_slice` | `height` | integer | > 0 | required |
| `nine_slice` | `border` | integer or object | see below | required |
| `nine_slice` | `stretch_inner` | Boolean | | `false` |

  3. The `border` of `nine_slice` has one of two forms:
     - an integer `b` > 0, which gives `b` for the left, the top, the right and the bottom;
     - an object with the four integer fields `left`, `top`, `right` and `bottom`, each ≥ 0.
       All four fields are required.
  4. A `nine_slice` scaling must have `left + right < width` and `top + bottom < height`.
  5. The client ignores a field that the table does not name.
- **Postconditions:** the `width` and the `height` of `tile` and `nine_slice` are the size of
  the sprite in GUI units. The size of the image in pixels does not change them.
- **Example:**

```json
{ "gui": { "scaling": { "type": "nine_slice", "width": 200, "height": 20,
  "border": { "left": 20, "top": 4, "right": 20, "bottom": 4 } } } }
```

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: in a resource pack,
give `tooltip/frame` the border `{"left": 10, "top": 10, "right": 10, "bottom": 10}`. The
tooltip looks the same as with the border `10`.

### GSPR-META-002: Errors in the sprite metadata

- **Behavior:** these errors stop the load of one GUI sprite:
  - the sprite metadata is not valid JSON;
  - the `gui` section breaks a rule of GSPR-META-001, for example an unknown `type`, a missing
    required field, a value out of its range, or a border as wide as the sprite;
  - the image cannot be read.
- **Result:** the client reports the error with the name of the sprite. It does not load that
  sprite, so its name resolves to the missing sprite (GSPR-MISS-001). The other sprites and the
  rest of the resource pack load.
- **Postconditions:** an error in one sprite metadata never stops the load of the resource pack.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: in a resource pack,
give `tooltip/background` the border 50 with the width 100. Open the chat and show a tag
tooltip. The background is the missing sprite, and the log has an error for
`minecraft:tooltip/background`.

---

## 2. The missing sprite

### GSPR-MISS-001: The missing sprite

- **Behavior:**
  1. A draw of a GUI sprite name that is not in the atlas draws the missing sprite.
  2. The missing sprite is 16 × 16 pixels. The pixel (`px`, `py`) is `FFF800F8` when exactly
     one of `px < 8` and `py < 8` is true. Otherwise it is `FF000000`.
  3. The missing sprite has the scaling `stretch`.
- **Postconditions:** the missing sprite is opaque. Its top left and bottom right quarters are
  black, and the other two quarters are magenta.

Provenance: `source_suggested`. Kind: layout. Confidence: high. Experiment: give an item the
`tooltip_style` component `example:none`, with no such sprites in any pack. Put the mouse on
the item in the inventory. The tooltip background is a stretched black and magenta pattern of
four squares.

---

## 3. Draw

### GSPR-DRAW-001: `stretch`

- **Behavior:** the full sprite stretches over the target, with the nearest pixel.
- **Edge cases:** a target with a width or a height of 0 draws nothing.

Provenance: `source_suggested`. Kind: layout. Confidence: high. Experiment: see GSPR-MISS-001.

### GSPR-DRAW-002: `tile`

- **Inputs:** a `tile` scaling with `width` `tw` and `height` `th`, and a target.
- **Behavior:**
  1. The full sprite is one tile of `tw` × `th` GUI units.
  2. The tiles fill the target from its top left corner, in rows and columns.
  3. A tile at the right or the bottom edge that does not fit is cut. It shows only the part of
     the sprite from its top left corner that fits.
- **Postconditions:** the tiles are not scaled.

Provenance: `source_suggested`. Kind: layout. Confidence: high. Experiment: draw a sprite with
`tile` 16 × 16 into a target of 40 × 16. Three tiles show, and the third shows the left half of
the sprite.

### GSPR-DRAW-003: `nine_slice`

- **Inputs:** a `nine_slice` scaling with `width` `sw`, `height` `sh`, the borders `l`, `t`, `r`
  and `b`, and the flag `stretch_inner`. A target of `PW` × `PH` GUI units.
- **Behavior:**
  1. Reduce the borders to the size of the target: `l' = min(l, floor(PW / 2))`,
     `r' = min(r, floor(PW / 2))`, `t' = min(t, floor(PH / 2))`, `b' = min(b, floor(PH / 2))`.
  2. The sprite has nine parts, cut at `l'` and `sw − r'` across and at `t'` and `sh − b'` down.
     The target has nine parts, cut at `l'` and `PW − r'` across and at `t'` and `PH − b'` down.
  3. Each corner part of the sprite draws into the same corner part of the target, with no
     scale.
  4. Each edge part and the centre part of the sprite fill the matching part of the target:
     - when `stretch_inner` is true, the part stretches over its target part, with the nearest
       pixel;
     - otherwise, the part repeats as a tile (GSPR-DRAW-002) from the top left corner of its
       target part.
  5. A part of the target with a width or a height of 0 draws nothing.
- **Postconditions:** when `PW` = `sw` and `PH` = `sh`, the sprite draws with no change.

Provenance: `source_suggested`; `black_box_confirmed` for the `.mcmeta` values of the vanilla
pack. Kind: layout. Confidence: medium. Experiment: show a tooltip with 20 lines
(`client-tooltip.md`, TIP-BG-001). Take a screenshot. The left outline changes colour from the
top to the bottom, because the sprite `tooltip/frame` stretches.

---

## 4. Intentional deviations

No deviation is recorded.
