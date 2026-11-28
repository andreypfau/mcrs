# GUI sprites of the client: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states how the vanilla client reads the scaling of a GUI sprite from the resource
pack, and how it draws a GUI sprite into a rectangle. It also states how the client draws the
enchantment glint over an item icon. The tooltip (`client-tooltip.md`), the
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
- **Item icon**: the picture of an item stack in an area of 16 × 16 GUI units, for example in
  a slot of a container screen or of the hotbar.
- **Glint**: the enchantment glint, a moving pattern that the client adds to the colour of an
  item icon. Other documents state which stack has the glint.
- **Glint texture**: the file `assets/minecraft/textures/misc/enchanted_glint_item.png` of the
  resource pack. In the vanilla pack it is 128 × 128 pixels, RGB.
- **Glint space**: the coordinates `(gx, gy)` on the glint texture. The value 1 is the full width
  or the full height of the texture, so a step of 1 / 128 is one pixel of the vanilla image.
- **Glint speed** `σ` and **glint strength** `s`: the two accessibility options of the client
  with the keys `glintSpeed` and `glintStrength` in `options.txt`. Each is a number from 0 to 1.
  The default of `σ` is 0.5. The default of `s` is 0.75.
- **Item atlas** and **block atlas**: the atlases `minecraft:textures/atlas/items.png` and
  `minecraft:textures/atlas/blocks.png`. An atlas coordinate `(u, v)` goes from 0 to 1, with `u`
  to the right and `v` down.
- **Special glint item**: an item in the item tag `#minecraft:compasses`, or `minecraft:clock`.
- **Projection point** `p`: the point from which the client computes the glint of one pixel of an
  item icon (GSPR-GLINT-003, GSPR-GLINT-004).
- **Glint point**: the point `G(p)` in glint space at which the client samples the glint texture
  for one pixel (GSPR-GLINT-002).

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
| `GSPR-GLINT` | the enchantment glint over an item icon |
| `GSPR-DEV` | intentional deviations |

### 0.5 Scope

In scope: the `gui` section of the sprite metadata, the three scaling types, the missing
sprite, and the glint over an item icon in the GUI. The `texture` section of the metadata of the
glint texture is in scope (GSPR-GLINT-001).

Out of scope: the animation section of the sprite metadata, the `texture` section of each other
sprite, the layout of
the atlas, and the tint colour of a draw. The glint of an item in the world, in the hand and on
armour is out of scope. The colour of an item icon before the glint is out of scope.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

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

Provenance: `source_suggested`. Kind: layout. Confidence: high.

---

## 3. Draw

### GSPR-DRAW-001: `stretch`

- **Behavior:** the full sprite stretches over the target, with the nearest pixel.
- **Edge cases:** a target with a width or a height of 0 draws nothing.

Provenance: `source_suggested`. Kind: layout. Confidence: high.

### GSPR-DRAW-002: `tile`

- **Inputs:** a `tile` scaling with `width` `tw` and `height` `th`, and a target.
- **Behavior:**
  1. The full sprite is one tile of `tw` × `th` GUI units.
  2. The tiles fill the target from its top left corner, in rows and columns.
  3. A tile at the right or the bottom edge that does not fit is cut. It shows only the part of
     the sprite from its top left corner that fits.
- **Postconditions:** the tiles are not scaled.

Provenance: `source_suggested`. Kind: layout. Confidence: high.

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
pack. Kind: layout. Confidence: medium.

---

## 4. The glint over an item icon

### GSPR-GLINT-001: The glint texture and its sampling

- **Observer:** a player: the look of an item icon with the glint.
- **Inputs:** the glint texture and its metadata file `enchanted_glint_item.png.mcmeta`.
- **Behavior:**
  1. The client samples the glint texture with the filter that the field `texture.blur` of the
     metadata selects: linear when it is true, nearest when it is false or absent.
  2. The field `texture.clamp` selects the wrap. When it is false or absent, the texture repeats
     on both axes. When it is true, the client clamps each axis to the edge.
  3. The glint texture has no mipmap levels.
- **Postconditions:** the vanilla metadata has `blur` true and no `clamp`. Thus the vanilla glint
  uses linear filter, and the texture repeats on both axes.

Provenance: `source_suggested_black_box_confirmed`
(`assets/minecraft/textures/misc/enchanted_glint_item.png.mcmeta` of the client jar). Kind:
layout. Confidence: medium.

### GSPR-GLINT-002: The transform into glint space

- **Observer:** a player: the scale, the angle and the motion of the glint stripes.
- **Inputs:** a projection point `p = (px, py)` (GSPR-GLINT-003 and GSPR-GLINT-004); the glint
  speed `σ`; a clock `t` of the client in milliseconds.
- **Behavior:**
  1. The clock `t` increases at the rate of real time. Its origin is not specified. It is not the
     game time, so the glint moves when the game is paused or the tick rate is frozen.
  2. In each frame, the client computes `k = floor(t · σ · 8)`. It then computes
     `a = (k mod 110000) / 110000` and `b = (k mod 30000) / 30000`.
  3. The point in glint space is `G(p) = R(8 · p) + (−a, b)`. `R` turns a point by 10°:
     `R(x, y) = (x · cos 10° − y · sin 10°, x · sin 10° + y · cos 10°)`.
  4. All item icons of one frame use the same `a` and `b`.
- **Postconditions:**
  - The offset `a` repeats after `110000 / (8 · σ)` ms, and `b` after `30000 / (8 · σ)` ms. At the
    default `σ = 0.5`, these are 27.5 s and 7.5 s, and the pattern repeats after 82.5 s. At
    `σ = 1`, they are 13.75 s and 3.75 s.
  - At `σ = 0`, the glint does not move.
  - At `σ = 0.5`, a point of the pattern moves on the glint texture by
    `(+0.012658, −0.137622)` each second, in the coordinates of `8 · p`. The examples of
    GSPR-GLINT-003 and GSPR-GLINT-004 give this motion in GUI units.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### GSPR-GLINT-003: The projection point of an icon that is not a special glint item

- **Observer:** a player: how much of the glint texture one item icon shows.
- **Inputs:** a pixel of the item icon, and the face of the item model that the pixel shows. The
  sprite of that face is in the item atlas or in the block atlas.
- **Behavior:**
  1. Let `(fx, fy)` be the point of the sprite that the pixel shows, as a fraction of the sprite:
     `(0, 0)` is the top left corner of the sprite, and `(1, 1)` is the bottom right corner.
  2. The projection point is `p = (o_x + 16 · fx / W, o_y + 16 · fy / H)`. `W` × `H` is the
     reference size of the atlas of the sprite:

     | Atlas of the sprite | `W` | `H` |
     |---------------------|-----|-----|
     | item atlas | 1024 | 512 |
     | block atlas | 2048 | 2048 |

  3. The origin `(o_x, o_y)` is a constant for each sprite. Its value is unspecified (see
     GSPR-DEV-001).
  4. The glint point of the pixel is `G(p)` (GSPR-GLINT-002).
- **Postconditions:**
  - All pixels of one sprite use one origin. Two slots with the same item show the same
    pattern at the same time. Two different items can show different parts of the pattern.
  - Each face of a block item has its own sprite, so each face shows its own part of the
    pattern.
- **Example:** a flat item with a sprite of 16 × 16 pixels in the item atlas.
  - Across the icon, `p` changes by `16 / 1024 = 0.015625`, so `8 · p` changes by 0.125. Down
    the icon, `p` changes by `16 / 512 = 0.03125`, so `8 · p` changes by 0.25.
  - Thus one icon shows an area of 16 × 32 pixels of the 128 × 128 glint texture, turned by
    10°. The pattern is compressed down the icon by a factor of 2, relative to across.
  - At `σ = 0.5`, a point of the pattern moves about 1.62 GUI units to the right and 8.81 GUI
    units up each second.
  - A face of a block item with a sprite of 16 × 16 pixels in the block atlas shows 8 × 8
    pixels of the glint texture.

Provenance: `source_suggested`; `black_box_confirmed` for `W` and `H` (GSPR-EXP-01). Kind:
deviation (GSPR-DEV-001). Confidence: medium.

### GSPR-GLINT-004: The projection point of a special glint item

- **Observer:** a player: the glint of a compass, a lodestone compass, a recovery compass or a
  clock in the GUI.
- **Inputs:** a pixel of the icon of a special glint item. The pixel shows the front face of the
  flat item model, at the point `(X, Y)` in GUI units from the top left corner of the icon.
- **Behavior:**
  1. The projection point is `p = (o_x + X / 1024, o_y + Y / 1024)`. The origin `(o_x, o_y)`
     is the same for all pixels of the icon. Its value is unspecified.
  2. The position of the sprite in its atlas does not change `p`.
  3. The glint point of the pixel is `G(p)` (GSPR-GLINT-002).
  4. For a face of another direction, for example the face of a model from a resource pack, the
     projection is unspecified.
- **Postconditions:** one icon of 16 × 16 GUI units shows an area of 16 × 16 pixels of the glint
  texture, turned by 10°. At `σ = 0.5`, a point of the pattern moves about 1.62 GUI units to the
  right and 17.62 GUI units up each second.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### GSPR-GLINT-005: The blend of the glint with the icon colour

- **Observer:** a player: the colour of the stripes over the item.
- **Inputs:** a pixel of the item icon with the colour `c = (r, g, b)` and the alpha `α`. The
  colour includes the texture, the tint and the shade of the item. Each channel goes from 0 to 1.
  The glint strength `s`. The sample `q = (qr, qg, qb)` of the glint texture at the glint point
  of the pixel (GSPR-GLINT-001).
- **Behavior:**
  1. A pixel with `α < 0.1` is not drawn, and it gets no glint.
  2. For each channel `x` of the colour, the new value is `min(1, x + (s · q_x)²)`. The client
     adds the glint after the shade of the item, so the glint has no shade.
  3. The new alpha is `max(α, s)`.
  4. The arithmetic uses the 8-bit values of the textures and of the target divided by 255.
     The client does not convert a colour to linear light.
  5. The fog does not change the glint in the GUI.
- **Postconditions:**
  - The glint never makes a pixel darker.
  - At `s = 0.75`, a white glint pixel adds 0.5625 to each channel. A glint value of 128 / 255
    adds about 0.1417.
  - At `s = 0`, the glint adds nothing, and the alpha does not change.
  - At the default `s`, each pixel with `α ≥ 0.1` has an alpha of at least 0.75 after the
    glint. A pixel with `α < 0.1` is not drawn (step 1).

Provenance: `source_suggested_black_box_confirmed` (`assets/minecraft/shaders/core/item.vsh`
and `item.fsh` of the client jar). Kind: layout. Confidence: medium.

### GSPR-GLINT-006: The resolution and the update of the glint

- **Observer:** a player: the smoothness and the motion of the stripes.
- **Inputs:** the GUI scale `n`.
- **Behavior:**
  1. The client computes the glint for each pixel of the screen that the icon covers. An icon of
     16 GUI units has `16 · n` pixels across, and each pixel has its own projection point.
  2. The client draws the glint again in each frame.
- **Postconditions:**
  - The stripes are finer than the pixels of the item sprite.
  - At `n = 2`, the icon of the example of GSPR-GLINT-003 is 32 × 32 pixels. It shows half a
    pixel of the glint texture for each screen pixel across, and one for each pixel down.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

---

## 5. Experiments

### GSPR-EXP-01: The size of the atlases

- **Steps:** start the vanilla client with the vanilla resource pack and the default options.
  Read the line of the log that the client writes when it creates each atlas.
- **Result:** the item atlas is 1024 × 512 pixels. The block atlas is 2048 × 2048 pixels.

---

## 6. Intentional deviations

### GSPR-DEV-001: The glint scale does not follow the size of the atlas

- **Rule:** GSPR-GLINT-003.
- **Reference:** the projection point is the true atlas coordinate of the sprite point. The
  client packs the sprites of the resource pack into the atlas, and the size of the atlas
  follows the sprites. A resource pack that adds sprites can make the atlas larger. The glint
  stripes then become coarser. The position of the sprite in the atlas gives the origin
  `(o_x, o_y)`.
- **Reason:** the scale and the origin are accidents of the packing. mcrs keeps the look of the
  vanilla pack without the packing algorithm of the reference. The rule uses the fraction of the
  sprite, so a resource pack with sprites of a higher resolution keeps the same scale.
- **Visible to the client:** with the vanilla resource pack, only the part of the pattern that
  each item shows at a given time is different. With a resource pack that changes the size of
  an atlas, the scale of the stripes is different.
- **Bug reference:** none.
- **Test:** a differential test compares the scale, the angle, the motion and the blend of the
  glint with the vanilla resource pack. It does not compare the origin of a sprite.
