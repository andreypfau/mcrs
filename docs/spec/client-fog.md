# Fog of the world in the client: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`.

This document states the fog that the client draws over the world when the eye of the camera is
in air: the environmental fog, the render-distance fog, the fog colour, and how each kind of
draw takes the fog. The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Tick**, **frame**, **partial tick** (`pt`) and **camera**: as `client-camera.md`, section
  0.1, defines them. The **camera position** and the **view direction** are those of
  CCAM-POSE-001 and CCAM-POSE-002.
- **Render distance** (`r`): the render distance of the client in chunks, MENU-DIST-001 step 1.
- **Attribute value** `A(name)`: the value in the frame of the environment attribute
  `minecraft:visual/<name>`, at the camera position (`environment-attributes.md`,
  ENVA-LAYER-007).
- **Rain level** (`R`) and **thunder level** (`T`): the two weather levels of
  ENVA-LAYER-005, for the partial tick of the frame. `T` is the thunder level multiplied by the
  rain level.
- **Camera medium**: air, water, lava or powder snow (CFOG-TYPE-001).
- **Fog parameters**: the seven values of a frame that the draws read: the fog colour `C`, the
  environmental start `es` and end `ee`, the render-distance start `rs` and end `re`, the sky
  end `se` and the cloud end `ce`. The distances are in blocks.
- **Fog colour** (`C`): the colour that the fog mixes into a draw (CFOG-COL-001, CFOG-COL-002).
- **Offset** of a point: the vector from the camera position to the point, on the world axes.
- **Spherical distance** (`ds`) of a point: the length of its offset (`dx`, `dy`, `dz`).
- **Cylindrical distance** (`dc`) of a point: `max(√(dx² + dz²), |dy|)`.
- **Ramp** `ramp(d, s, e)`: 0 when `d ≤ s`; else 1 when `d ≥ e`; else `(d − s) / (e − s)`.
- **Fog value** (`v`) of a point: the fraction of the fog colour in the pixel (CFOG-MIX-001).
- **Fog mix**: the change of a colour by the fog value. CFOG-MIX-001 step 4 states it.
- **Rain fog factor** (`q`): a value that the client keeps between frames (CFOG-ENV-002).
- **Image value**: a channel of a colour in `[0, 1]`, as the client writes it into the image. The
  byte of the channel in a screenshot is `round(255 · c)`.
- **Improved Transparency**: the video option `options.improvedTransparency`, "Improved
  Transparency" (`client-menu.md`, MENU-VAL-012). The default is off.
- **Main image**: the image of the frame that the player sees.
- **Sky image** and **cloud image**: two images of the size of the main image. The client
  draws them only with Improved Transparency on (CFOG-TIME-002, step 3). At each pixel, the
  sky image has a colour `sk`. The cloud image has a stored colour `cl`, a stored alpha `ca` and a
  depth.
- **Depth** of a point or of a pixel: its distance from the camera along the view axis. The
  projection of CCAM-PROJ-001 gives the depth of each image of the frame. A point with a larger
  depth is **farther**.
- **Over blend** of a colour `u` with the alpha `α` over an image: the pixel becomes
  `lerp(α, pixel, u)`.
- **Cloud Distance** (`c`): the video option `options.renderCloudsDistance`, in chunks, from 2 to
  128. The default is 128.
- `lerp(t, a, b)`: `a + t · (b − a)`. For a colour, `lerp` applies to each channel.
- `smoothstep(a, b, x)`: let `t = clamp((x − a) / (b − a), 0, 1)`. The result is `t² · (3 − 2t)`.
  The formula also applies when `a > b`.
- **Draw test**: the test of MENU-DIST-001 step 3 that selects the columns that the client
  draws.
- **Ring** (`k`) of a column: its distance from the column of the camera in columns, as the draw
  test measures it (CFOG-MCRS-004, step 1).
- **Drawn column**: a column that the client holds and whose sections with blocks have their
  meshes in the frame. CFOG-MCRS-004, the input "the drawn columns of the frame", states when a
  column becomes drawn and when it stops being drawn.
- **Drawn end** (`e`): the end of the render-distance fog in mcrs, a value that the client keeps
  between frames (CFOG-MCRS-004).
- `f32` is a 32-bit IEEE 754 value. `f64` is a 64-bit IEEE 754 value.

### 0.2 Provenance, confidence and observer

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". The observer of each rule is a player who looks at the
image, or a screenshot of the image. Each rule has a line `Milestone`. `1` means the first
release of this document. `later` means a rule that a later release implements.

A parity test of this document compares image values with a tolerance of `1/255`, and fog
values with a tolerance of `1e-4`.

### 0.3 Arithmetic

The fog parameters, the fog colour and the rain fog factor are `f32`. A colour channel of an
attribute is its byte divided by 255 (ENVA-TYPE-002). The fog mix is linear in the image values.
It is not a mix in linear light.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CFOG-TYPE` | the camera medium |
| `CFOG-TIME` | the fog parameters of a frame and the order of the frame |
| `CFOG-ENV` | the environmental fog and the rain fog factor |
| `CFOG-COL` | the fog colour |
| `CFOG-MIX` | the fog value of a point and the fog mix |
| `CFOG-DRAW` | the draws of the world and the fog |
| `CFOG-SKY` | the sky disc, the other sky elements and the sky occluder |
| `CFOG-CLOUD` | the clouds |
| `CFOG-MCRS` | the subset of mcrs |
| `CFOG-TEST` | parity tests |
| `CFOG-EXP` | experiments |
| `CFOG-DEV` | intentional deviations |

### 0.5 Scope

In scope: a world with the camera medium air, and a camera entity with no effect. The
render-distance fog is in MENU-DIST-001, steps 5 and 6. This document refers to it and does not
state it again. In mcrs, the render-distance fog ends at the drawn end `e` (CFOG-MCRS-004).

Out of scope, one line for each:

- The fog when the camera medium is water, lava or powder snow.
- The fog of the effects blindness and darkness, and the brighter fog of night vision.
- The fog of the panorama and of the menus. The GUI takes no fog (CFOG-TIME-001).
- The fog and the darker world of a boss bar.
- The fade of a new chunk section, the option `options.chunkFade`.
- The sky of the skybox `end`.
- The geometry of the sun, the moon, the stars and the sunrise fan, and the shape of the clouds.

Improved Transparency also changes these results outside the fog. Each line states what mcrs
does in milestone 1:

- The translucent layers of a pixel show in the order of their depth (CFOG-TIME-002, step 3).
  The layers are the translucent terrain, the translucent entities and items, the particles,
  the shadows, the name tags and the text in the world, the block outline, the crack of a block,
  the rain and the snow, the world border and the clouds. mcrs: a fixed order (CFOG-MCRS-002,
  item 1; CFOG-DEV-002).
- The clouds show behind water and the other translucent blocks. With the option off, they do
  not: the tooltip `options.graphics.fancy.tooltip` states it. mcrs: yes (CFOG-MCRS-002,
  item 1).
- Each change of the option builds each chunk mesh again. mcrs: no (`client-menu.md`,
  MENU-VAL-012, MENU-DEV-013).
- The preset `options.graphics.fabulous`, "Fabulous!", of the video settings sets the option to
  on. The presets "Fast" and "Fancy" set it to off. A change of the option sets the preset to
  "Custom". mcrs: no, because mcrs has no preset (`client-menu.md`, MENU-VID-002, step 3).
- The debug key `key.debug.improvedTransparency`, "Toggle Improved Transparency", changes the
  option. mcrs: no. That key is out of scope.

### 0.6 Artefacts cited

- The shaders of the client jar: `assets/minecraft/shaders/include/fog.glsl`,
  `assets/minecraft/shaders/core/terrain.fsh`, `assets/minecraft/shaders/core/sky.fsh`,
  `assets/minecraft/shaders/core/sky_occluder.fsh`, `assets/minecraft/shaders/core/clouds.fsh`
  and the other files of `assets/minecraft/shaders/core/` and
  `assets/minecraft/shaders/include/` that a rule names.
- The language file of the client jar: `assets/minecraft/lang/en_us.json`.
- The data pack of the client jar: `data/minecraft/dimension_type/overworld.json`,
  `data/minecraft/timeline/day.json` and the biome files `data/minecraft/worldgen/biome/*.json`.
- The frame artefact of `client-frame-capture.md`:
  `crates/mcrs_minecraft_client/tests/fixtures/vanilla/frames/`. All of its scenes are in the
  flat world in the plains biome, at the time 6000, in clear weather, with `r = 5` and the
  field of view 70°. The background frames have only terrain and sky.

---

## 1. The camera medium

### CFOG-TYPE-001: Camera medium

- **Observer:** a player: the kind of fog, and the field of view of CCAM-FOV-005, step 3.
- **Inputs:** the camera position `P` of the frame; the forward, up and left unit vectors `F`,
  `U` and `L` of the camera; the FOV option `O` (CCAM-OPT-001); the width and the height of the
  image; the blocks and the fluids of the world.
- **Behavior:** in `f64`:
  1. Let `h = 0.05 · tan(O / 2)`, with `O` in degrees. Let `w = h · width / height`.
  2. Let `N = 0.05 · F`. Make five points in this order: `P + N`, `P + N + h·U + w·L` (top
     left), `P + N + h·U − w·L` (top right), `P + N − h·U + w·L` (bottom left) and
     `P + N − h·U − w·L` (bottom right).
  3. For each point, find the block cell that contains it.
  4. When the cell holds a fluid, let `H` be the surface height of that fluid in the cell:
     - 1 when the fluid is a source and the bottom face of the block above the cell is sturdy;
     - else 1 when the cell above holds the same fluid;
     - else the amount of the fluid divided by 9.
  5. When the cell holds a fluid and the `y` of the point is at or below the cell bottom plus
     `H`, the medium is that fluid, water or lava. Stop.
  6. When the cell holds no fluid and the block is `minecraft:powder_snow`, the medium is powder
     snow. Stop.
  7. Else go to the next point. When no point gives a medium, the medium is air.
- **Postconditions:**
  - The rules of this document apply when the medium is air.
  - CCAM-FOV-005 step 3 applies when the medium is water or lava.
  - The near plane of this test uses `O`, not the field of view of the frame.
- **Edge cases:** a point in a cell of powder snow stops the test before a later point in water.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CFOG-EXP-07.
Milestone: later.

---

## 2. The frame

### CFOG-TIME-001: Fog parameters of a frame

- **Observer:** a player.
- **Inputs:** a frame of the world.
- **Behavior:**
  1. After the client places the camera of the frame (CCAM-TIME-002, steps 2 and 3), it
     calculates the fog parameters of the frame one time.
  2. The values of steps 3 to 7 use the attribute values, `R`, `T` and `r` of this frame.
  3. `C`: CFOG-COL-001, then CFOG-COL-002.
  4. `es` and `ee`: CFOG-ENV-001.
  5. `rs` and `re`: MENU-DIST-001, step 6. mcrs: CFOG-MCRS-004, step 5 (CFOG-DEV-005).
  6. `se = min(16 · r, A(sky_fog_end_distance))`.
  7. `ce = min(16 · c, A(cloud_fog_end_distance))`.
  8. Each draw of the world in the frame reads these values. The hand and the item in the hand
     read them too.
  9. The GUI, the HUD and the screens take no fog.
- **Postconditions:** a change of an attribute value changes the fog in the frame that reads
  it. The fog is the same for each draw of one frame.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Milestone: 1 for steps 1 to 9,
with `q = 0` in step 4.

### CFOG-TIME-002: Clear colour and order of the frame

- **Observer:** a player: the pixels that no draw covers, and the pixels with translucent
  layers.
- **Inputs:** `C` of the frame; the option Improved Transparency.
- **Behavior:**
  1. The client clears the image to `C`, with the alpha 1, before it draws the sky.
  2. With Improved Transparency off, the client draws in this order: the sky (CFOG-SKY-001 to
     CFOG-SKY-003), the opaque terrain and the opaque draws, the translucent draws and the
     translucent terrain, the clouds (CFOG-CLOUD-001), the rain and the snow.
  3. With Improved Transparency on, the client draws the frame in these steps:
     1. It clears the sky image to `C`. It draws the sky into the sky image: CFOG-SKY-001, then
        CFOG-SKY-002, then CFOG-SKY-003. When the client draws no sky (CFOG-SKY-001, step 1),
        the sky image keeps the colour `C`.
     2. It clears the cloud image to `cl = (0, 0, 0)` and `ca = 0`, with the depth at the far
        end. It draws the clouds of CFOG-CLOUD-001 into the cloud image, with a depth test among
        the cloud faces. A cloud pixel with the colour `u` and the alpha `α` sets
        `cl = α · u + (1 − α) · cl` and `ca = α + (1 − α) · ca`. The depth of the cloud image is
        the depth of the nearest cloud face. When the client draws no clouds
        (CFOG-CLOUD-001, step 1), the cloud image keeps `cl = 0` and `ca = 0`.
     3. It clears the main image to `C`. It draws the opaque terrain and the opaque draws into
        the main image. The opaque terrain takes the fog of CFOG-DRAW-002.
     4. It copies the sky image into each pixel of the main image that no opaque draw covers.
     5. It composites the translucent layers into the main image: the translucent terrain with
        the fog of CFOG-DRAW-002, the translucent draws, the rain and the snow, and the cloud
        image. The cloud image is one layer at its depth, with the colour `cl / ca` and the alpha
        `ca`. A layer is hidden where an opaque face is nearer than the layer.
     6. The client composites the layers of a pixel in the order of their depth. The order of
        the draws has no effect. This document does not state that method.
- **Postconditions:**
  - A pixel that no draw covers has the colour `C`. In the overworld at noon in the plains with
    `r = 5`, that colour is `(176, 205, 255)`.
  - With Improved Transparency on, a pixel with one translucent layer with the alpha
    `0.01 ≤ α ≤ 0.98` has the over blend of that layer. A cloud pixel with no other
    translucent layer thus shows `(1 − ca) · m + cl`, where `m` is the pixel of the main image
    after step 3.4. mcrs blends the translucent terrain in linear light (CFOG-DEV-004).
  - With Improved Transparency on, a translucent layer with `α > 0.98` replaces the colour of
    the pixel, when it is the only layer of the pixel.
- **Edge cases:** a pixel of the cloud image with `ca = 0` has no cloud. The cloud image gives
  no layer there.

Provenance: `black_box_confirmed` for the first postcondition (frame artefact,
`pig_idle.front.background.png`, the pixels at the horizon, where the last terrain meets the
sky); `source_suggested` for the order, and for the second and third postconditions
(`assets/minecraft/shaders/core/oit_composite.fsh`;
`assets/minecraft/shaders/include/oit_common.glsl`, the limit `0.02` of the total
transmittance; `assets/minecraft/shaders/include/oit.glsl`, the limit `0.01` of the alpha).
`assets/minecraft/shaders/core/blit_clouds.fsh` divides the stored colour by the stored alpha,
and writes the depth of the cloud image. Kind: semantic. Confidence: high for steps 1 and 2,
medium for step 3. Experiment: CFOG-EXP-01, CFOG-EXP-08.
Milestone: 1 for steps 1 to 3. In step 3, mcrs composites the layers in a fixed order
(CFOG-MCRS-002, item 1; CFOG-DEV-002). The owner decided on 2026-11-13 that Improved
Transparency has its effect from the first release.

---

## 3. The environmental fog

### CFOG-ENV-001: Environmental start and end

- **Observer:** a player: the amount of fog on near and far blocks.
- **Inputs:** `A(fog_start_distance)` = `S`, `A(fog_end_distance)` = `E`, and `q`.
- **Behavior:** in `f32`:
  1. `es = S − 160 · q`.
  2. `ee = max(min(96, E), E − 256 · q)`.
- **Postconditions:**
  - With `q = 0`: `es = S` and `ee = E`. In the overworld, `S = 0` and `E = 1024`
    (`data/minecraft/dimension_type/overworld.json` sets neither, so the defaults of
    ENVA-TYPE-006 apply).
  - With `q = 1` in the overworld: `es = −160` and `ee = 768`. A point at the camera then has
    the fog value `160/928 = 0.1724`.
  - In `minecraft:the_nether`, `S = 10` and `E = 96`.
- **Edge cases:** when `E ≤ 96`, the rain does not lower `ee`. The rain never raises `ee` above
  `E`.

Provenance: `source_suggested_black_box_confirmed`. The frame artefact is consistent with
`es = 0` and `ee = 1024` in clear weather: in `pig_idle.front.background.png` and
`pig_walk.step4.background.png`, the removal of the fog of CFOG-MIX-001 from the grass gives
one colour, about `(84.5, 110.3, 51.8)`, at 6 to 50 blocks. Kind: semantic. Confidence:
high for `q = 0`, medium for `q > 0`. Experiment: CFOG-EXP-04. Milestone: 1 for `q = 0`, with
the limit of 512 blocks of CFOG-MCRS-002, item 3 (CFOG-DEV-003). `later` for `q > 0`.

### CFOG-ENV-002: Rain fog factor

- **Observer:** a player: the fog grows over about one second when rain starts, and it goes
  away under a roof.
- **Inputs:** `R`; the sky light `L` of the camera cell, from 0 to 15, as the light engine stores
  it; the biome of the camera cell; the time `Δ` since the previous frame, in ticks of the
  current tick rate (`f32`): the milliseconds since that frame divided by the milliseconds of
  one tick.
- **Behavior:** in each frame, before CFOG-ENV-001:
  1. Let `s = clamp((L − 8) / 7, 0, 1)`.
  2. Let `b = 1` when the field `has_precipitation` of the biome is `true`, and `b = 0.5` when it
     is `false`.
  3. Let `g = R · s · b`.
  4. `q = q + (g − q) · min(1, 0.2 · Δ)`.
- **Postconditions:**
  - `q` is 0 when the client starts.
  - When `g` does not change, `q` comes near `g` with a time constant of 5 ticks. After 20 ticks
    of short frames, `q` is within 2 % of `g`.
  - In a desert in rain, `g = 0.5 · R`. In a cell with a sky light of 8 or less, `g = 0`.

Provenance: `source_suggested`; `design` for the `min` of step 4 (CFOG-DEV-001). Kind: semantic.
Confidence: medium. Experiment: CFOG-EXP-04. Milestone: later.

---

## 4. The fog colour

### CFOG-COL-001: Base colour

- **Observer:** a player: the colour of the far terrain, of the horizon and of the clear image.
- **Inputs:** `A(fog_color)` = `F0`; `A(sky_color)` = `S0`; `A(sunrise_sunset_color)` = `U`,
  with the alpha `U.a`; `A(sun_angle)` = `θ`, in degrees; `A(sky_fog_end_distance)` = `Es`;
  `r`; the view direction `f` of the camera, a unit vector; `R` and `T`.
- **Behavior:** in `f32`:
  1. Let `c = F0`.
  2. When `r ≥ 4`:
     1. Let `x = −1` when `sin(θ) > 0`. Else let `x = 1`.
     2. Let `g = x · f.x`, where `f.x` is the east part of `f`.
     3. When `g > 0` and `U.a > 0`, `c = lerp(g · U.a, c, U.rgb)`.
  3. Let `s = S0`. When `R > 0`, multiply the red and the green of `s` by `1 − 0.5 · R`, and its
     blue by `1 − 0.4 · R`. Clamp each channel to `[0, 1]`.
  4. When `T > 0`, multiply each channel of `s` by `1 − 0.5 · T`. Clamp each channel to
     `[0, 1]`.
  5. Let `n = min(Es / 16, r)`. Let `t = clamp(n / 32, 0, 1)`. Let `p = lerp(t, 0.25, 1)`.
  6. Let `k = 1 − p^0.25`. The power is in `f64`, and the client rounds it to `f32`.
  7. The base colour is `lerp(k, c, s)`.
- **Postconditions:**
  - With `Es = 512`, `k` depends only on `r`:

    | `r` | `p` | `k` |
    |-----|-----|-----|
    | 2 | 0.296875 | 0.26185209 |
    | 4 | 0.34375 | 0.23429644 |
    | 5 | 0.3671875 | 0.22156569 |
    | 8 | 0.4375 | 0.18671173 |
    | 12 | 0.53125 | 0.14626175 |
    | 16 | 0.625 | 0.11086029 |
    | 32 or more | 1 | 0 |

  - At `θ` with `sin(θ) > 0`, the sun is in the west, so the sunset colour shows when the
    camera looks west. At sunrise it shows when the camera looks east.
  - The sky colour of step 3 and step 4 is darker than the sky disc in rain. The sky disc uses
    `S0` (CFOG-SKY-001).
- **Edge cases:** with `r < 4`, the sunrise colour has no effect on the fog colour.

Provenance: `black_box_confirmed` for steps 1 and 5 to 7 at `r = 5` with no sunrise colour: the
frame artefact shows `(176, 205, 255)`, and the rule gives `(176.05, 205.14, 255)`.
`source_suggested` for steps 2 to 4. In rain, the weather layer of ENVA-LAYER-005 already moves
`S0` toward gray, and steps 3 and 4 make the sky term dark a second time. The owner decided on
2026-11-13 to keep steps 3 and 4, as the game does. CFOG-TEST-004 states the result. Kind:
semantic. Confidence: high for steps 1 and 5 to 7, medium for steps 2 to 4. Experiment:
CFOG-EXP-01, CFOG-EXP-02, CFOG-EXP-04, CFOG-EXP-05. Milestone: 1.

### CFOG-COL-002: Void darkness and alpha

- **Observer:** a player: the fog goes black near the bottom of the world.
- **Inputs:** the base colour of CFOG-COL-001; the `y` of the camera position; the `min_y` of
  the dimension type; the field `flat` of the spawn information (`dimension-change.md`).
- **Behavior:** in `f32`:
  1. Let `o = 1` when `flat` is true. Else let `o = 32`.
  2. Let `a = clamp((o + min_y − y) / o, 0, 1)`.
  3. When `a > 0`, multiply each channel of the base colour by `(1 − a)²`.
  4. The alpha of `C` is 1.
- **Postconditions:** in the overworld of a normal world, `min_y = −64`. The fog colour starts
  to go dark below `y = −32`, and it is black at `y = −64` and below. At `y = −50`,
  `a = 0.5625` and the factor is `0.19140625`. In a flat world, the dark part is the last block
  above `min_y`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CFOG-EXP-06.
Milestone: later.

---

## 5. The fog value

### CFOG-MIX-001: Fog value of a point and fog mix

- **Observer:** a player: the fog on each draw of section 6.
- **Inputs:** a point of a draw, its offset, and the fog parameters.
- **Behavior:**
  1. `ds` and `dc` are the distances of the point. MENU-DIST-001, step 5, also states `dc`.
  2. Let `ve = ramp(ds, es, ee)`. Let `vr = ramp(dc, rs, re)`.
  3. `v = max(ve, vr)`.
  4. The fog mix of a colour with the channels `rgb` and the alpha `α` gives
     `lerp(v, rgb, C.rgb)` and the alpha `α`.
- **Postconditions:**
  - The environmental fog uses the spherical distance. The render-distance fog uses the
    cylindrical distance.
  - The distances are those of the point itself (CFOG-MCRS-003). The vanilla client computes
    them at the vertices of faces of one block, and interpolates them. For a ramp with the length
    `L > 0`, `ee − es` or `re − rs`, the difference of its value is at most the difference of the
    distance divided by `L`; it is less where a distance is outside the ramp. The fog value, the
    larger of the two ramps, differs by no more than the larger of their two differences. A ramp
    with `L = 0` is a step: on a face that the step crosses, the fog value can differ by up to 1.
    For a face of one block and ramps with `L > 0`:
    - `ds` differs by at most `1 / (4 · d)` blocks, where `d` is the smallest `ds` of the face.
      Thus `ve` differs by less than `1/255` when `d · (ee − es) ≥ 64`.
    - `dc` is the larger of `|dy|` and `√(dx² + dz²)`:
      - Where `|dy|` is the larger at each point of the face, `dc = |dy|`, and the interpolation
        gives the same value. `vr` does not differ.
      - Where `√(dx² + dz²)` is the larger at each point of the face, `dc` differs by at most
        `1 / (4 · h)` blocks, where `h` is the smallest `√(dx² + dz²)` of the face. Thus `vr`
        differs by less than `1/255` when `h · (re − rs) ≥ 64`.
      - Where the face crosses `|dy| = √(dx² + dz²)`, `dc` differs by up to about `1/2` block at
        each distance, and `vr` by up to `1 / (2 · (re − rs))`.
    - `v` differs by less than `1/255` when `ve` and `vr` both do. The condition of one ramp
      does not bound the other ramp. Example with `ve = ramp(ds, 0, 1024)` and
      `vr = ramp(dc, 28, 32)`, the ramps of the vanilla data in the overworld at `r = 2`: a
      horizontal face at `dy = 28.25`, with `dx` from 27.75 to 28.75 and `dz` from −0.5 to 0.5,
      has `d · (ee − es) > 64`, and it crosses `|dy| = √(dx² + dz²)`. At its centre, `v = 0.0625`
      for the point itself and about `0.1255` with the interpolated distances.
    - With the ramps of the vanilla data, `ve` meets its condition: in the overworld
      `ee − es = 1024`, so `d ≥ 1/16`; in the nether `ee − es = 86`, so `d ≥ 0.75`. `vr` meets
      its condition on each face that does not cross `|dy| = √(dx² + dz²)`: `re − rs ≥ 4`, and
      `vr` differs only on a face where `dc` goes above `rs`. With `r ≥ 2`, `rs ≥ 28`, so such
      a face has `h > 26`.
    - Outside these conditions, for example a data pack with `es = 0` and `ee = 1`, the pixel of
      the vanilla client can differ by far more than `1/255`. The pixel of mcrs is the value of
      the point itself. That difference is intended (CFOG-DEV-006).

    CFOG-SKY-001 states the one draw where mcrs keeps the interpolation.
- **Edge cases:** when `es ≥ ee` or `rs ≥ re`, the ramp is a step at the start: 0 at or before
  the start, 1 after it.
- **Note:** in mcrs, `ve = 0` for each point when `A(fog_end_distance) > 512` (CFOG-MCRS-002,
  item 3; CFOG-DEV-003). Then `v = vr`.

Provenance: `source_suggested_black_box_confirmed` (`assets/minecraft/shaders/include/fog.glsl`,
the functions of the fog value and of the fog mix; frame artefact, as CFOG-ENV-001). Kind:
semantic. Confidence: high. Experiment: CFOG-EXP-01, CFOG-EXP-02. Milestone: 1.

---

## 6. The draws of the world

### CFOG-DRAW-001: Terrain

- **Observer:** a player: the blocks of the world.
- **Inputs:** a pixel of a block face of any terrain layer: solid, cutout or translucent.
- **Behavior:**
  1. With Improved Transparency off, the pixel takes the fog mix of CFOG-MIX-001 step 4.
  2. With Improved Transparency on, CFOG-DRAW-002 applies.
- **Postconditions:** with Improved Transparency off, at `dc ≥ re` the terrain has the colour
  `C`, as the sky at the horizon (CFOG-SKY-001) and the clear colour (CFOG-TIME-002). With the
  option on, it has the colour of the sky and the clouds behind it (CFOG-DRAW-002).

Provenance: `source_suggested_black_box_confirmed` (`assets/minecraft/shaders/core/terrain.fsh`;
frame artefact). Kind: semantic. Confidence: high. Milestone: 1.

### CFOG-DRAW-002: Terrain with Improved Transparency

- **Observer:** a player: with the option on, the far terrain fades into the sky behind it, not
  into `C`.
- **Inputs:** a pixel of a block face of any terrain layer: solid, cutout or translucent. Its
  colour `rgb` and its alpha `α` before the fog, and its depth. `ve` and `vr` of CFOG-MIX-001.
  At the same pixel: the colour `sk` of the sky image, and `cl`, `ca` and the depth of the cloud
  image (CFOG-TIME-002, step 3).
- **Behavior:**
  1. Let `v = max(ve, vr)`. Let `b = max(smoothstep(0.9, 1, ve), vr)`.
  2. Let `h = 1` when the depth of the cloud image is equal to the depth of the pixel or
     larger. Else let `h = 0`.
  3. Let `Q = cl / ca` when `ca > 0`. Else let `Q = cl`.
  4. The background colour is `K = lerp(ca · h, sk, Q)`.
  5. Let `Z = lerp(b, C, K)`. The colour of the pixel is `lerp(v, rgb, Z)`. Its alpha stays
     `α`.
- **Postconditions:**
  - `K = (1 − ca · h) · sk + h · cl`. Where the cloud image has no cloud, `K = sk`. A cloud nearer
    than the face does not change `K`.
  - When `b = 0`, `Z = C`, and the pixel is the fog mix of CFOG-MIX-001 step 4. Thus the option
    changes the fog only where `vr > 0` or `ve > 0.9`.
  - At the end of the render distance, `v = b = 1`, and the pixel has the colour `K`: the sky
    and the clouds behind it. This is also true at sunrise and with the sun or the moon behind
    the pixel.
  - CFOG-TEST-005 gives numbers.
- **Edge cases:**
  - With no sky (CFOG-SKY-001, step 1) and no clouds, `K = C`. The pixel is then the pixel of
    CFOG-DRAW-001, step 1.
  - A translucent face takes this rule. CFOG-TIME-002, step 3.5, then composites it with its
    alpha `α`.
  - The draws of CFOG-DRAW-003 do not take this rule.
- **Note:** in mcrs, `ve = 0` when `A(fog_end_distance) > 512` (CFOG-DEV-003). Then
  `smoothstep(0.9, 1, ve) = 0`, `b = vr` and `v = vr`.

Provenance: `source_suggested_black_box_confirmed`: `assets/minecraft/shaders/core/terrain.fsh`,
the variant with improved fog, has steps 1 to 5 and the constant `0.9`. The tooltip
`options.improvedTransparency.oitWithImprovedFog.tooltip` of `assets/minecraft/lang/en_us.json`
states that the option blends the sky and the clouds with the render-distance fog.
`source_suggested` for the set of draws that take the rule. Kind: semantic. Confidence: medium.
Experiment: CFOG-EXP-08. Milestone: 1. The owner decided on 2026-11-13 that Improved
Transparency has its effect from the first release.

### CFOG-DRAW-003: Other draws of the world

- **Observer:** a player.
- **Inputs:** a pixel of a draw of the world that is not terrain, not the sky and not a cloud.
- **Behavior:** the draw takes the fog as the table states. "Mix" is the fog mix of
  CFOG-MIX-001 step 4.

  | Draw | Fog |
  |------|-----|
  | entities, block entities, item entities, `minecraft:falling_block` | mix |
  | particles, the rain and the snow | mix |
  | the block outline and the crack of a block that the player breaks | mix |
  | the hand and the item in the hand | mix |
  | the shadow of an entity, a lead | mix |
  | text in the world: a name tag, the text of a sign | mix |
  | the part of a name tag that shows through blocks | none |
  | the enchantment glint | the colour of the glint times `1 − v` |
  | the bolt of lightning | its colour and its alpha times `1 − v` |
  | the beam of a beacon | mix, with `ds = dc =` the depth of the pixel along the view axis |
  | the sun, the moon, the stars, the sunrise fan | none (CFOG-SKY-002) |

- **Postconditions:**
  - In clear weather in the overworld, a draw at 0.5 blocks has `v < 0.001`. In full rain
    (`q = 1`), the hand has `v ≈ 0.173`.
  - Improved Transparency does not change the fog of these draws. With the option on, the
    translucent draws of the table composite as CFOG-TIME-002, steps 3.5 and 3.6, state.

Provenance: `source_suggested_black_box_confirmed` (`assets/minecraft/shaders/core/entity.fsh`,
`block.fsh`, `particle.fsh`, `rendertype_lines.fsh`, `text.fsh`, `glint.fsh`,
`rendertype_lightning.fsh` and `rendertype_beacon_beam.fsh`); `source_suggested` for the
assignment of each draw to its shader. Kind: semantic. Confidence: medium. Experiment:
CFOG-EXP-03. Milestone: 1 for the entities, the block entities, the particles and the block
outline. `later` for the other rows.

---

## 7. The sky

### CFOG-SKY-001: Sky disc

- **Observer:** a player: the colour of the sky, from the zenith to the horizon.
- **Inputs:** the skybox of the dimension type; `S0 = A(sky_color)`; `C` and `se` of the frame.
- **Behavior:**
  1. The client draws the sky only when the skybox is `overworld`. With the skybox `none`, the
     clear colour shows (CFOG-TIME-002).
  2. The sky disc is a fan of 8 triangles. Its points are offsets from the camera position, on
     the world axes:
     - the centre `(0, 16, 0)`;
     - the ring points `(512 · cos i, 16, 512 · sin i)` for `i` = −180°, −135°, −90°, −45°, 0°,
       45°, 90°, 135° and 180°.
  3. The distance of the centre is 16 blocks. The distance of each ring point is
     `√(512² + 16²) = 512.25` blocks.
  4. The distance `d` of a pixel is the linear interpolation of these vertex distances over its
     triangle, linear in world space.
  5. The colour of the pixel is `lerp(min(1, d / se), S0, C)`, with the alpha 1. The disc is
     opaque.
- **Postconditions:**
  - Let `λ` be the horizontal fraction from the centre to the edge of the octagon, in the
    direction of the pixel. Then `d = 16 + 496.25 · λ`.
  - At the zenith, the fog value is `16 / se`: `0.0833` with `r = 12`, and `0.2` with `r = 5`.
  - A view ray that misses the disc shows the colour of the image under it: the clear colour
    `C`. The disc ends at the elevation `atan(16 / 512) = 1.79°` toward a ring point, and
    `atan(16 / (512 · cos 22.5°)) = 1.94°` at the middle of an edge.
  - With `A(sky_fog_end_distance) ≤ 512`, the edge of the disc has the colour `C`, so the
    disc meets the clear colour with no seam.
- **Edge cases:** with `A(sky_fog_end_distance) > 512.25` and `16 · r > 512.25`, the edge of the
  disc is not fully `C`. The image then shows a seam at the edge.

Provenance: `black_box_confirmed` (frame artefact: in the background frames of `pig_idle`,
`pig_walk` and `pig_lineup`, at the yaws −180°, 135° and 90°, each sampled sky pixel is within
`1/255` of the rule. With an exact distance for each pixel, the error is 8/255 to 10/255).
`assets/minecraft/shaders/core/sky.fsh` gives step 5. Kind: semantic. Confidence: high.
Experiment: CFOG-EXP-01. Milestone: 1.

### CFOG-SKY-002: The other sky elements

- **Observer:** a player.
- **Inputs:** a frame with the skybox `overworld`.
- **Behavior:**
  1. After the sky disc, the client draws the sunrise fan when `U.a > 0.001`, then the sun, the
     moon and the stars.
  2. None of them takes the fog.
- **Postconditions:** a sun at the horizon is not fogged. The sky occluder (CFOG-SKY-003)
  covers it below the horizon.

Provenance: `source_suggested_black_box_confirmed` (`assets/minecraft/shaders/core/stars.fsh`,
`position_tex.fsh` and `position_color.fsh` have no fog). Kind: semantic. Confidence: medium.
Milestone: 1.

### CFOG-SKY-003: Sky occluder

- **Observer:** a player: the colour of the sky below the horizon.
- **Inputs:** `A(has_sky_occluder)`; `C`; the eye `y` of the local player for the partial tick;
  the `min_y` of the dimension type; the view ray of each pixel.
- **Behavior:**
  1. When the skybox is `overworld` and `A(has_sky_occluder)` is true, the client draws the
     occluder after the elements of CFOG-SKY-002.
  2. The occluder covers the full image. Let `e` be the elevation of the view ray of a pixel, in
     degrees.
  3. The alpha is `smoothstep(a0, a1, e)`. When the eye `y` is below `min_y`,
     `(a0, a1) = (15, 10)`. Else, with the camera medium air, `(a0, a1) = (−1, −8)`. The angles
     for the camera medium water are out of scope.
  4. The colour of the pixel is `lerp(alpha, colour below, C)`.
- **Postconditions:**
  - With the eye `y` at or above `min_y`: above an elevation of −1°, the occluder has no effect.
    Below −8°, the sky has the colour `C`.
  - With the eye `y` below `min_y`: above an elevation of 15°, the occluder has no effect. Below
    10°, the sky has the colour `C`. Thus at the elevation 0° the alpha is 1.
  - The terrain covers the occluder.

Provenance: `source_suggested_black_box_confirmed`
(`assets/minecraft/shaders/core/sky_occluder.fsh`; `"has_sky_occluder": true` in
`data/minecraft/dimension_type/overworld.json`). Kind: semantic. Confidence: medium.
Experiment: CFOG-EXP-05. Milestone: 1.

---

## 8. The clouds

### CFOG-CLOUD-001: Fog of the clouds

- **Observer:** a player: far clouds fade out.
- **Inputs:** a pixel of a cloud face; `ce` of the frame; `A(cloud_color)`.
- **Behavior:**
  1. The client draws the clouds when the option Clouds is not Off and the alpha of
     `A(cloud_color)` is above 0.
  2. Let `ds` be the spherical distance of the cloud point.
  3. The alpha of the cloud pixel is multiplied by `1 − ramp(ds, 0, ce)`.
  4. The colour of the cloud does not take the fog colour. The environmental fog and the
     render-distance fog do not apply to the clouds.
- **Postconditions:**
  - With the default Cloud Distance of 128 chunks and the overworld, `ce = 2048` blocks. A cloud
    at 1024 blocks has half its alpha. In the nether, the alpha of the cloud colour is 0, so the
    client draws no clouds.
  - Improved Transparency does not change this rule. With the option on, the client draws the
    clouds into the cloud image (CFOG-TIME-002, step 3.2).

Provenance: `source_suggested_black_box_confirmed` (`assets/minecraft/shaders/core/clouds.fsh`;
`assets/minecraft/shaders/core/clouds.vsh` uses the spherical distance). Kind: semantic.
Confidence: high. Experiment: CFOG-EXP-03. Milestone: 1.

---

## 9. Constants

| Constant | Value | Unit | Rule |
|----------|-------|------|------|
| rain offset of the environmental start | −160 | blocks | CFOG-ENV-001 |
| rain offset of the environmental end | −256 | blocks | CFOG-ENV-001 |
| smallest end that rain gives | 96 | blocks | CFOG-ENV-001 |
| rate of the rain fog factor | 0.2 | per tick | CFOG-ENV-002 |
| sky light without rain fog | 8 | light level | CFOG-ENV-002 |
| sky light span of the rain fog | 7 | light levels | CFOG-ENV-002 |
| factor of a biome with no precipitation | 0.5 | — | CFOG-ENV-002 |
| smallest `r` for the sunrise colour | 4 | chunks | CFOG-COL-001 |
| rain darkness of the sky term | 0.5, 0.5, 0.4 | per channel | CFOG-COL-001 |
| thunder darkness of the sky term | 0.5 | — | CFOG-COL-001 |
| sky term: blocks per chunk, chunks, low end, exponent | 16, 32, 0.25, 0.25 | — | CFOG-COL-001 |
| void onset, normal world and flat world | 32, 1 | blocks | CFOG-COL-002 |
| span of the render-distance fog | `clamp(16 · r / 10, 4, 64)` | blocks | MENU-DIST-001 |
| background blend start | 0.9 | fog value | CFOG-DRAW-002 |
| largest environmental end that mcrs draws | 512 | blocks | CFOG-MCRS-002 |
| sky disc height and radius | 16, 512 | blocks | CFOG-SKY-001 |
| sky disc angle step | 45 | degrees | CFOG-SKY-001 |
| sunrise fan limit | 0.001 | alpha | CFOG-SKY-002 |
| occluder angles, normal | −1, −8 | degrees | CFOG-SKY-003 |
| occluder angles, eye below `min_y` | 15, 10 | degrees | CFOG-SKY-003 |
| blocks per chunk of the cloud end | 16 | blocks | CFOG-TIME-001 |
| blocks per ring of the drawn target | 16 | blocks | CFOG-MCRS-004 |
| rate of the drawn end | 0.1 | per tick | CFOG-MCRS-004 |
| snap of the drawn end to the target | 0.01 | blocks | CFOG-MCRS-004 |
| span of the render-distance fog in mcrs | `clamp(e / 10, 4, 64)` | blocks | CFOG-MCRS-004 |

---

## 10. Parity tests

Each test is in the overworld in the plains biome, with the camera at `y = 80` and the sky light
15 at the camera cell. The camera looks north. `A(sky_fog_end_distance) = 512`, `c = 128`. The
values are those of the game. In the overworld, `A(fog_end_distance) = 1024 > 512`, so mcrs has no
environmental fog (CFOG-DEV-003). A line that starts with "mcrs:" gives the value of mcrs where it
is not that of the game. Each other value is the same in mcrs. In mcrs, each test has each column
of the draw test drawn, and `e = 16 · r` (CFOG-MCRS-004; CFOG-DEV-005).

### CFOG-TEST-001: Noon in clear weather

- **Inputs:** time 6000, `R = T = 0`, `q = 0`, `r = 12`.
- **Postconditions:**
  - `F0 = #c0d8ff = (0.752941, 0.847059, 1)`, `S0 = #78a7ff = (0.470588, 0.654902, 1)`,
    `U.a = 0`.
  - `k = 0.14626175`. `C = (0.711644, 0.818954, 1)`, the bytes `(181, 209, 255)`.
  - `es = 0`, `ee = 1024`, `rs = 172.8`, `re = 192`, `se = 192`, `ce = 2048`. mcrs: the fog
    parameters have no environmental ramp.
  - The offset `(100, −20, 50)`: `ds = 113.578`, `dc = 111.803`, `v = 0.110916`. mcrs: `v = 0`.
  - The offset `(180, 0, 0)`: `ve = 0.175781`, `vr = 0.375`, `v = 0.375`. mcrs: `ve = 0`,
    `v = 0.375`.
  - The offset `(150, −100, 0)`: `ds = 180.278`, `dc = 150`, `v = 0.176052`. mcrs: `v = 0`.
  - At the zenith, the sky pixel is `lerp(0.083333, S0, C)`.

### CFOG-TEST-002: Noon with `r = 5`

- **Inputs:** as CFOG-TEST-001, with `r = 5`.
- **Postconditions:**
  - `k = 0.22156569`. `C = (176.05, 205.14, 255) / 255`, the bytes `(176, 205, 255)`.
    `rs = 72`, `re = 80`, `se = 80`. `es` and `ee` are those of CFOG-TEST-001. mcrs: the fog
    parameters have no environmental ramp. The sky disc does not use `ve`, so the sky pixel
    below is the same in mcrs.
  - In `pig_lineup.front.background.png`, the camera looks north with the pitch 15° and the
    field of view 70°. The view ray of the pixel (427, 5) has the elevation 19.4°. It meets the
    disc at the horizontal distance 45.5 blocks, so `λ = 0.0888`, `d = 60.1` and `v = 0.751`.
    The rule gives `(162.1, 195.7, 255)`. The frame has `(162, 196, 255)`.

### CFOG-TEST-003: Midnight in clear weather

- **Inputs:** time 18000, `R = T = 0`, `q = 0`, `r = 12`.
- **Postconditions:** the timeline `day` multiplies `F0` by `#0f0f16` and `S0` by `#000000`, and
  `U.a = 0`. `F0 = (0.044291, 0.049827, 0.086275)`, `S0 = (0, 0, 0)`.
  `C = (0.037813, 0.042539, 0.073656)`, the bytes `(10, 11, 19)`. The fog parameters other than
  `C`, and the fog values of the offsets, are those of CFOG-TEST-001, in the game and in mcrs.

### CFOG-TEST-004: Noon in rain

- **Inputs:** time 6000, `R = 1`, `T = 0`, `q = 1` (the limit of CFOG-ENV-002 after a long
  time in the rain), `r = 12`.
- **Postconditions:**
  - The weather layer gives `F0 = (0.374994, 0.421869, 0.6)` and
    `S0 = (0.404553, 0.450631, 0.536906)`.
  - Step 3 of CFOG-COL-001 gives `s = (0.202276, 0.225316, 0.322144)`.
  - `C = (0.349732, 0.393120, 0.559360)`, the bytes `(89, 100, 143)`.
  - `es = −160`, `ee = 768`. `rs`, `re`, `se` and `ce` are those of CFOG-TEST-001.
  - The offset `(0, 0, 10)`: `v = 0.183190`. The offset `(100, −20, 50)`: `v = 0.294804`. The
    offset `(180, 0, 0)`: `ve = 0.366379`, `vr = 0.375`, `v = 0.375`.
  - mcrs: `es`, `ee` and the fog values of this test need the rain fog of a later release
    (section 13, item 4).

### CFOG-TEST-005: Noon with Improved Transparency

- **Inputs:** as CFOG-TEST-001, with Improved Transparency on. A terrain face has the colour
  `rgb = (0.4, 0.5, 0.1)` before the fog. Where a case states a cloud, the cloud image at the
  pixel has `cl = (0.6, 0.6, 0.6)` and `ca = 0.6`.
- **Postconditions:**
  - The offset `(0, 30, −180)` has the elevation 9.462°. Its view ray meets the sky disc 96
    blocks north of the camera, toward the ring point `(0, 16, −512)`. Thus `λ = 0.1875`,
    `d = 109.046875`, and the fog value of the sky is `0.567952`.
  - `sk = lerp(0.567952, S0, C) = (0.607496, 0.748076, 1)`. The sky image holds the bytes
    `(155, 191, 255)`. The values below use these bytes.
  - At that offset, `ds = 182.483`, `dc = 180`, `ve = 0.178206`, `vr = 0.375`, and
    `v = b = 0.375`. mcrs: `ve = 0`, and `v = b = 0.375`. `vr` gives the larger value, so the
    pixels of the next three cases are the same in mcrs.
  - With no cloud: `K = (0.607843, 0.749020, 1)`, `Z = (0.672719, 0.792729, 1)`, and the pixel
    is `(0.502270, 0.609773, 0.4375)`. With the option off, the pixel is
    `(0.516867, 0.619608, 0.4375)`.
  - With the cloud behind the face (`h = 1`): `K = (0.843137, 0.899608, 1)`,
    `Z = (0.760954, 0.849199, 1)`, and the pixel is `(0.535358, 0.630950, 0.4375)`.
  - With the cloud in front of the face (`h = 0`), the pixel is that with no cloud.
  - The offset `(0, 32, −192)` is on the same view ray, with `dc = 192` and `v = b = 1`. With no
    cloud, the pixel is `K`, the bytes `(155, 191, 255)`. With the option off, it is `C`, the
    bytes `(181, 209, 255)`. With the cloud behind the face, it is the bytes `(215, 229, 255)`.
    mcrs: the same, because `vr = 1`.
  - In rain as CFOG-TEST-004, the offset `(0, 0, 10)` has `ve = 0.183190` and `vr = 0`. Thus
    `b = 0`, and the pixel is the fog mix of CFOG-MIX-001 step 4, as with the option off. mcrs:
    this case needs the rain fog (milestone `later`; section 13, item 4).
  - The face is opaque, and no blended draw covers it. Thus CFOG-DEV-004 does not change these
    pixels.

---

## 11. Experiments

Each experiment uses the scene format of `client-frame-capture.md` and the task
`captureClientFrames` of `tools/vanilla-oracle`, unless it states another tool. The capture must
be of `26.4-snapshot-7`.

### CFOG-EXP-01: The clear frame at `r = 5` (run)

- **Steps:** read the background frames of the frame artefact. For each sky pixel, compute the
  view ray from the record of the frame. Compare the pixel with CFOG-SKY-001 and CFOG-COL-001.
  For the grass at 6 to 50 blocks, remove the fog of CFOG-MIX-001 from each pixel.
- **Result:** run on 2026-11-13. The clear colour is `(176, 205, 255)`. Each sampled sky pixel
  matches CFOG-SKY-001 within `1/255` (CFOG-TEST-002). The grass gives one colour, about
  `(84.5, 110.3, 51.8)`, after the removal.

### CFOG-EXP-02: The fog colour and the render distance (not run)

- **Steps:** a scene with the time 6000, clear weather and `tick freeze`. Capture the same
  pose, the pitch 0, at `r` = 2, 4, 8, 12 and 16.
- **Record:** the pixel just above the horizon. It must be `C` of CFOG-COL-001 for each `r`.

### CFOG-EXP-03: Draws in the fog (not run)

- **Steps:** at `r = 5`, put a pig, a chest, a torch with particles and a sign at 40, 70 and 78
  blocks from the camera. Capture them with the block outline on a block at 4 blocks.
- **Record:** each pixel against its background frame. The fog value of each pixel must be that
  of CFOG-MIX-001.

### CFOG-EXP-04: Rain (not run)

- **Steps:** a scene without `tick freeze`. Run `weather rain`. Capture at 1, 5, 10, 20 and 60
  ticks after the rain level reaches 1. Then capture under a stone roof after 40 ticks.
- **Record:** `C` and the fog on a block at 10 blocks. After 60 ticks, they must be those of
  CFOG-TEST-004. Under the roof, `es` and `ee` must go back to 0 and 1024.

### CFOG-EXP-05: Sunrise and below the horizon (not run)

- **Steps:** capture at the time 12732, looking west and looking east, with the pitch 0 and the
  pitch −20, at `r = 12` and at `r = 2`.
- **Record:** looking west, `C` takes the sunset colour; looking east it does not. With `r = 2`
  it does not. Below −8°, the pixels with no terrain must be `C`.

### CFOG-EXP-06: Void darkness (not run)

- **Steps:** in a normal world, a spectator at `y = −50` and at `y = −70`, at noon.
- **Record:** `C` must be the base colour times `0.19140625` at `y = −50`, and black at
  `y = −70`.

### CFOG-EXP-07: Camera medium (not run)

- **Steps:** use `tools/vanilla-client` and `tools/mcrs_minecraft_client_recorder`. Lower the
  camera into still water in steps of 0.01 blocks, and into a cell of powder snow above water.
- **Record:** the first frame with the water fog and with the field of view of CCAM-FOV-005 step
  3. The surface test must be that of CFOG-TYPE-001.

### CFOG-EXP-08: Improved Transparency (not run)

- **Steps:**
  1. Make a scene with `r = 12` and the clouds on (`renderClouds` `"true"`). With `fill`, put a
     wall of stone with its south face 186 blocks north of the camera. The wall is 160 blocks
     wide and goes from the ground to 40 blocks above the camera. The camera looks north, 10°
     above the horizon.
  2. Capture three frames of the same pose: the option `improvedTransparency` `false` with the
     wall; `true` with the wall; and `true` with no wall.
  3. Make a scene at the time 12732 with flat terrain. The camera looks west. Capture it with
     the option `false` and `true`.
  4. Add a wall of `minecraft:white_stained_glass` 10 blocks north of the camera to the scene
     of step 1. Capture it with the option `false` and `true`.
- **Record:**
  1. For each pixel of the south face in the frames of step 2, compute `v` and `b` from its
     view ray. Get `rgb` of the face from the frame with the option off and from CFOG-MIX-001.
  2. Each pixel of the frame with the option on must be that of CFOG-DRAW-002 within `2/255`.
     `K` is the pixel of the frame with no wall.
  3. Where `dc ≥ 192`, each pixel of the face must be the pixel of the frame with no wall.
  4. In step 3, the terrain at the end of the render distance must have the colour of the
     sunset sky behind it with the option on, and `C` with the option off.
  5. In step 4, the clouds must show through the glass with the option on. Record whether they
     show with the option off.

---

## 12. Intentional deviations

### CFOG-DEV-001: Overshoot of the rain fog factor

- **Rule:** CFOG-ENV-002, step 4.
- **Reference:** the step is `q = q + (g − q) · 0.2 · Δ`, with no limit. A frame of more than 5
  ticks, 250 ms at 20 ticks a second, moves `q` past `g`. When `g = 0`, `q` then becomes
  negative, and `es` goes above `S`. A frame of more than 10 ticks moves `q` farther from `g`
  than it was before the frame.
- **Visible to the client:** yes. After a long frame, for example when the rain stops, the
  fog jumps and then settles.
- **Test:** a differential test skips the frames after a frame of more than 5 ticks.

### CFOG-DEV-002: mcrs composites the translucent layers in a fixed order

- **Rule:** CFOG-MCRS-002, item 1. This entry records what mcrs does less than CFOG-TIME-002,
  step 3.6, in milestone 1.
- **Reference:** with Improved Transparency on, the client composites the translucent layers of
  each pixel in the order of their depth. The order of the draws has no effect.
- **Visible to the client:** yes, where two or more translucent layers cover one pixel. For
  example, mcrs shows the faces of the translucent terrain in its own order of the draws. When
  the player is above the clouds, a translucent face behind a cloud shows in front of the cloud.
- **Test:** with Improved Transparency on, a differential test compares only the pixels with at
  most one translucent layer. The cloud image counts as one layer.

### CFOG-DEV-003: mcrs draws no environmental fog where it ends beyond 512 blocks

- **Rule:** CFOG-MCRS-002, item 3. The owner decided on 2026-11-13 that mcrs draws no
  environmental haze in clear weather, at any render distance. The render distance of mcrs goes
  to 96 chunks, 1536 blocks (`client-menu.md`, the range of Render Distance). With the end at
  1024 blocks, each point beyond 64 chunks is all fog, and nearer terrain loses more contrast
  than the game shows at its largest render distance, 32 chunks. 512 blocks is 16 blocks times
  32 chunks. A dimension with a dense fog keeps it: the nether has `S = 10` and `E = 96`.
- **Reference:** the environmental fog applies in each dimension. In the overworld,
  `ve = ramp(ds, 0, 1024)`.
- **Visible to the client:** yes. In the overworld, near and middle terrain has less fog in
  mcrs. With `r = 12`, the offset `(100, −20, 50)` has `v = 0` in mcrs and `v = 0.110916` in the
  game (CFOG-TEST-001). At and near the end of the render distance, `vr` gives the larger value,
  and the two are the same.
- **Test:** a differential test in a dimension with `A(fog_end_distance) > 512` compares the fog
  values of mcrs with `vr` alone. Section 10 gives the values of mcrs on the lines "mcrs:".

### CFOG-DEV-004: mcrs blends the blended draws in linear light

- **Rule:** the over blend (section 0.1) where a draw is blended into the image: the translucent
  terrain with each value of Improved Transparency (CFOG-TIME-002, steps 2 and 3.5, and its
  second postcondition), the clouds with Improved Transparency off, the sunrise fan and the
  stars (CFOG-SKY-002), and the sky occluder (CFOG-SKY-003, step 4).
- **Reference:** the over blend is on image values.
- **mcrs:** the image is an sRGB target. A blended draw converts its colour to linear light, and
  the target blends there. These results do not change, because mcrs computes them in image
  values: the opaque draws, each fog mix (CFOG-MIX-001, CFOG-DRAW-002), the sky disc
  (CFOG-SKY-001), and the cloud layer with Improved Transparency on (CFOG-MCRS-002, item 1.1).
  The deviation goes away when mcrs draws the world into a target that holds image values and
  converts it one time at the end. That is a change of the renderer for a later release.
- **Visible to the client:** yes. A white layer with the alpha 0.5 over black gives about 0.735
  in mcrs, and 0.5 in the game.
- **Test:** a differential test compares the pixels that no blended draw covers with the
  tolerance of section 0.2. For a pixel that a blended draw covers, the test states its own
  tolerance.

### CFOG-DEV-005: mcrs ends the render-distance fog at the edge of the drawn columns

- **Rule:** CFOG-MCRS-004. The owner decided on 2026-11-13 that the render-distance fog follows
  the drawn columns. The render distance of mcrs goes to 96 chunks (`client-menu.md`, the range
  of Render Distance). At such distances the columns arrive over many seconds. The fog at the
  edge of the drawn columns hides their arrival.
- **Reference:** the render-distance fog ends at `16 · r` in each frame, also while columns that
  pass the draw test are not drawn. A column that is not drawn shows as a hole in the terrain,
  with the sky or the clear colour behind it.
- **Visible to the client:** yes, while a column that passes the draw test is not drawn: after
  a join, after a change of the dimension, after a larger `r`, and while the camera moves into
  new columns. In mcrs the fog then ends nearer than `16 · r`.
- **Test:** a differential test compares frames only after each column that passes the draw
  test is drawn and `e = 16 · r`.

### CFOG-DEV-006: mcrs computes the fog distances of each pixel

- **Rule:** CFOG-MCRS-003.
- **Reference:** the client computes `ds` and `dc` at the vertices of each face and interpolates
  them over the face. The interpolated distance is larger than the distance of the point.
- **Visible to the client:** yes, outside the conditions of CFOG-MIX-001, the second
  postcondition. With a short ramp from a data pack, the vanilla client shows too much fog on a
  near face: with `es = 0` and `ee = 1`, a face of one block at 0.1 blocks in front of the
  camera has `ds = 0.1` at its centre and about 0.714 at its corners. With the vanilla data, the
  render-distance fog shows a difference of up to `1 / (2 · L)` on a face that crosses
  `|dy| = √(dx² + dz²)`.
- **Test:** a differential test compares only the pixels of faces that meet the conditions of
  CFOG-MIX-001, the second postcondition.

---

## 13. Unresolved

1. CFOG-ENV-002 and CFOG-COL-001: `R` and `T`, and how they change, belong to a weather
   specification that does not exist yet (`environment-attributes.md`, open question 3).
2. CFOG-ENV-002: this document does not state if `q` keeps its value when the client goes to
   another world or dimension.
3. CFOG-DEV-002: the owner decides if mcrs composites the translucent layers in the order of
   their depth, as CFOG-TIME-002 step 3.6 states. The options are:
   - (a) a later release adds such a method for Improved Transparency on, and CFOG-DEV-002 then
     goes away;
   - (b) mcrs keeps the fixed order of CFOG-MCRS-002, item 1, and CFOG-DEV-002 stays.

   Recommendation: (a), in its own issue after mcrs draws the particles and the rain and the
   snow. Until then, the translucent terrain and the clouds are the only translucent layers of
   mcrs, and the method has little to sort.
4. CFOG-DEV-003 and the rain fog (CFOG-ENV-002, milestone `later`): in the overworld at `q = 0`,
   mcrs has no environmental fog. In the game, `q > 0` gives `ve = ramp(ds, S − 160 · q,
   E − 256 · q)`. When mcrs takes that ramp for each `q > 0`, the fog jumps when rain starts:
   at `ds = 100`, `ve` goes from 0 to about 0.098. The fog of mcrs must change with no jump in
   `q`. The release that builds the rain fog decides how. The options are:
   - (a) when `E > 512`, `ve = q · ramp(ds, es, ee)`, with `es` and `ee` of CFOG-ENV-001. At
     `q = 0` it gives 0. At `q = 1` it gives the fog of the game, so CFOG-TEST-004 applies to
     mcrs;
   - (b) mcrs tests `ee > 512` in place of `E > 512`. In the overworld, `ee ≥ 768`, so mcrs then
     has no rain fog in the overworld;
   - (c) when `E > 512`, `ve = max(0, ramp(ds, es, ee) − ramp(ds, S, E))`: the fog that the rain
     adds. At `q = 1` it is less than the fog of the game.

   Recommendation: (a). It is continuous, it keeps the fog of the game in full rain, and it
   needs no new constant. The same question applies when a data pack moves `E` across 512, for
   example with a timeline: the fog of mcrs then jumps at 512.
5. CFOG-MCRS-004: a column that passes the draw test and never becomes drawn keeps the fog at 16
   times its ring. A server that does not send each column of the draw test causes this, for
   example a server for a minigame with a small map. A vanilla server sends each such column
   (`client-menu.md`, MENU-DIST-003, the margin of 2 columns). The options are:
   - (a) no tolerance, as in this release;
   - (b) a column that the client does not hold does not count for `m` when no column arrived in
     the last 40 ticks. A column that the client holds and that is not drawn counts;
   - (c) a column does not count for `m` after it passed the draw test for 200 ticks and is
     still not drawn.

   Recommendation: (b). It keeps the fog over the columns that arrive, and it frees the fog when
   the server sends no more columns. A column that the client holds always becomes drawn, so (b)
   needs no limit for the meshes.
6. CFOG-MCRS-004: when the camera goes into the next column, the columns that come into the draw
   test can lower `e` to `16 · (r − 1)` or less until their meshes are built. The client already
   holds these columns (item 5). The owner decides if the client must build the mesh of a column
   that it holds before that column passes the draw test, so that the fog end does not move when
   the camera walks. Recommendation: yes, for the columns of the next ring outside the draw test.
7. CFOG-MCRS-004: mcrs counts the mesh of a section as drawn in the frame from the moment the
   client places it for the frame. When the client places many meshes at once, the upload of
   their geometry and of their draw records to the GPU can come some frames later. A section can
   also stay out of the draw list when its stream has no room. In those frames the fog end can
   move past a column that the image does not show yet. The options are:
   - (a) a column becomes drawn only when the draw records of its sections are applied for the
     frame;
   - (b) mcrs keeps this limit, because the fog end moves with a time constant of 10 ticks and
     the upload usually comes first.

   Recommendation: (a), in a later release.

---

## 14. The subset of mcrs

### CFOG-MCRS-001: What mcrs drew before the first fog

This is a record of the state of mcrs before the release of CFOG-MCRS-002, not a rule. It does
not state what mcrs draws now. Each item is in the past, at the base of CFOG-MCRS-002.

- The image is cleared to `A(fog_color)`, with no step of CFOG-COL-001 or CFOG-COL-002.
- The sky disc is a fan at the height 16 with the radius `A(sky_fog_end_distance)`, not 512. Its
  fog value is the vertex distance divided by `A(sky_fog_end_distance)`, not by `se`. It mixes
  toward `A(fog_color)`, not toward `C`.
- When the camera `y` is below 63, a black disc 4 blocks below the camera covers the lower sky.
  `26.4-snapshot-7` has no such disc. It has the sky occluder of CFOG-SKY-003.
- The clouds fade over `A(cloud_fog_end_distance)`, with no limit from Cloud Distance.
- The terrain, the entities, the particles and the block outline take no fog. MENU-DIST-001
  steps 5 to 7 are not implemented.
- mcrs has no option Improved Transparency.

### CFOG-MCRS-002: The first fog of mcrs

- **Observer:** a player.
- **Behavior:** the first release of the fog in mcrs implements the rules with the milestone 1:
  1. CFOG-TIME-001 with `q = 0`, and CFOG-TIME-002 steps 1 to 3. In place of step 3.6, mcrs
     composites the layers of step 3.5 in this fixed order, each with the over blend:
     1. the cloud image, with the colour `cl / ca` and the alpha `ca`, in each pixel with `ca > 0`
        where no opaque face is nearer than the depth of the cloud image;
     2. then the translucent terrain, with the fog of CFOG-DRAW-002, in the order of the draws
        of mcrs. The cloud image does not hide it.
  2. CFOG-COL-001, with steps 3 and 4;
  3. CFOG-ENV-001 with `q = 0`, and CFOG-MIX-001, with this limit (CFOG-DEV-003). Let
     `E = A(fog_end_distance)` of the frame:
     1. When `E > 512`, `ve = 0` for each point of each draw, and the fog parameters of the
        frame have no environmental ramp. The fog value is `v = vr`. How the fog parameters
        express this has no observer.
     2. When `E ≤ 512`, CFOG-ENV-001 applies with no change. In the nether, `es = 10` and
        `ee = 96`.
     3. The limit 512 is 16 blocks times 32 chunks, the largest render distance of the game. It
        does not depend on `r`.
  4. MENU-DIST-001 steps 5 to 7, with the start and the end of CFOG-MCRS-004 (CFOG-DEV-005);
  5. CFOG-DRAW-001 steps 1 and 2, CFOG-DRAW-002, and the rows of CFOG-DRAW-003 with the
     milestone 1;
  6. CFOG-SKY-001 to CFOG-SKY-003, in place of the black disc of CFOG-MCRS-001;
  7. CFOG-CLOUD-001;
  8. the option Improved Transparency of `client-menu.md`, MENU-VAL-012.
- **Postconditions:**
  - With Improved Transparency off, the terrain at the end of the render distance, the sky at
    the horizon, the sky below the horizon and the clear image have the same colour `C`.
  - With the option on, the terrain at the end of the render distance has the colour of the sky
    and the clouds behind it (CFOG-DRAW-002). The sky and the clear image keep the colours of
    the option off.
  - With the option on, the clouds show behind the translucent terrain. Where a pixel has one
    translucent layer with the alpha from 0.01 to 0.98, the image is that of the game
    (CFOG-TIME-002, the second postcondition), with the over blend in linear light of
    CFOG-DEV-004.
  - In the overworld, a point nearer than `rs` has no fog (item 3).
  - While a column that passes the draw test is not drawn, the render-distance fog ends at the
    edge of the drawn columns (item 4).

Provenance: `design`. These rules give the render-distance fog with a colour that meets the
sky at the horizon. The limit of item 3 is CFOG-DEV-003, and the blend of the blended draws is
CFOG-DEV-004. The rain fog and the void darkness come in a later release. Improved
Transparency is in this release, because the owner decided on 2026-11-13 that the option has
its effect from the first release. It changes the colour of the far terrain only in some
scenes: where the sky behind the terrain is not `C`. Examples are terrain above the horizon,
the sunrise and the sunset, and clouds behind the terrain. The fixed order of item 1 is
CFOG-DEV-002. The fog end of item 4 is CFOG-DEV-005. Kind: semantic. Confidence: high.
Milestone: 1.

### CFOG-MCRS-003: Fog distances for each pixel

- **Observer:** a player.
- **Behavior:** mcrs computes `ds` and `dc` of a terrain pixel, an entity pixel and a particle
  pixel from the world position of the pixel. It does not interpolate them from the vertices.
- **Postconditions:** a terrain face longer than one block gets the fog of each of its points.

Provenance: `design`. The result is that of the vanilla client within `1/255` only under the
conditions of CFOG-MIX-001, the second postcondition: a face far enough from the camera for the
length of its ramp. With the ramps of the vanilla data, these conditions hold for most pixels.
Outside them, mcrs gives the value of the point itself and the vanilla client the interpolated
value (CFOG-DEV-006). A face of mcrs can be longer than one block. The sky disc keeps the
interpolation of CFOG-SKY-001, because the image shows it. Kind: semantic. Confidence: high.
Milestone: 1.

### CFOG-MCRS-004: The render-distance fog follows the drawn columns

- **Observer:** a player: while the columns load, the terrain ends in the fog at the edge of the
  drawn columns. The player sees no hole at the place of a column that is not drawn.
- **Inputs:**
  - `r` (MENU-DIST-001, step 1);
  - the column (`cx`, `cz`) of the camera of the frame;
  - the drawn columns of the frame:
    - A column becomes drawn when the client holds it and the mesh of each of its sections that
      has blocks is drawn in the frame. A column whose sections have nothing to draw is drawn.
    - A drawn column stops being drawn when the client lets the column go, or when the mesh of
      one of its sections leaves the frame with no mesh in its place.
    - A drawn column stays drawn while the client builds the mesh of a section again with the
      old mesh still in the frame. It also stays drawn when a section that had no blocks gets
      its first blocks.
    - In this release, a mesh is drawn in the frame from the moment the client places it for
      the frame, also when its upload to the GPU comes in a later frame or its section stays out
      of the draw list for lack of room. The fog end can then move past a column that the image
      does not show yet (section 13, item 7);
  - the drawn end `e` of the previous frame. `e` is 0 when the client joins a world and when it
    changes the dimension;
  - the time `Δ` since the previous frame, in ticks, as CFOG-ENV-002 defines it.
- **Behavior:** in each frame, before CFOG-TIME-001 step 5. `e` is `f32`.
  1. The ring of the column (`x`, `z`) is the largest integer `k` with
     `k² ≤ max(0, |x − cx| − 1)² + max(0, |z − cz| − 1)²`. Thus
     `k = floor(√(max(0, |x − cx| − 1)² + max(0, |z − cz| − 1)²))`. The draw test uses the same
     sum, so a column passes the draw test when `k < r`, and only then.
  2. Let `m` be the smallest ring of a column that passes the draw test and is not a drawn
     column. The target is `t = 16 · m` blocks. When each column that passes the draw test is a
     drawn column, `t = 16 · r`.
  3. When `t ≤ e`, `e = t`. Else `e = e + (t − e) · min(1, 0.1 · Δ)`.
  4. When `t − e < 0.01` after step 3, `e = t`.
  5. The render-distance fog of the frame ends at `re = e`. It starts at
     `rs = e − clamp(e / 10, 4, 64)`. These values replace the end and the start of
     MENU-DIST-001 step 6. The fog distance of step 5, the ramp of step 6 and the larger value
     of step 7 do not change.
  6. The fog colour (CFOG-COL-001), the sky end `se` and the cloud end `ce` (CFOG-TIME-001) use
     `r`, not `e`.
- **Postconditions:**
  - After step 4, `e ≤ t ≤ 16 · r`. A column that passes the draw test has `k ≤ r − 1`. Thus
    `t ≤ 16 · (r − 1)` while such a column is not drawn.
  - A point at the horizontal distance `h < t` from the camera is in a drawn column. Reason:
    each column between the column of the camera and the column of the point is 16 blocks wide.
    Thus `16 · max(0, |x − cx| − 1) ≤ |dx|`, and the same for `z`. The ring of the column of the
    point is then at most `floor(h / 16)`, which is below `m` and below `r`.
  - Each point with a fog value `vr < 1` is in a drawn column, because `h ≤ dc < re = e ≤ t`.
  - When each column that passes the draw test is drawn and stays drawn, `t − e` falls by the
    factor `1 − 0.1 · Δ` in each frame. This is a time constant of 10 ticks, 0.5 s at 20 ticks a
    second. From `e = 0` with `r = 12` and frames of 1 tick, `e = 192` after 94 frames. With
    `r = 96`, `e = 1536` after 114 frames. From then on, the fog is that of MENU-DIST-001 step 6.
  - When the client joins a world, `e = 0`. The world comes out of the fog from the camera
    outward.
  - When a column of the ring `k` with `16 · k < e` stops being drawn, `e` falls to `16 · k` or
    less in the same frame. When `r` falls, `e` falls to `16 · r` or less in the same frame.
  - Example: `r = 12`. Each column of the rings 0 to 4 is drawn, and one column of the ring 5 is
    not drawn. Then `t = 80`. From `e = 0` with `Δ = 1`, the next values of `e` are 8, 15.2 and
    21.68. From `e = 192`, when that column stops being drawn, `e = 80` in the same frame. With
    `e = 80`, `rs = 72` and `re = 80`.
- **Edge cases:**
  - With no drawn column, `t = 0` and `e = 0`. Then `rs = −4` and `re = 0`, and each point is in
    the fog (CFOG-MIX-001, the ramp at `dc ≥ re`).
  - A block change does not make a column stop being drawn. The client builds the mesh of the
    section again, and the old mesh stays in the frame until the new mesh replaces it. When the
    change puts the first blocks into a section with no blocks, the column also stays drawn
    while the client builds the first mesh of that section. Else one block change in the ring 0
    sets `e = 0`.
  - A section whose mesh leaves the frame with no mesh in its place makes its column stop being
    drawn, also while the client builds a new mesh for it.
  - A column that passes the draw test and never becomes drawn keeps `t` at 16 times its ring.
    This release has no tolerance for it (section 13, item 5).
  - When the camera goes into the next column, the columns that come into the draw test lower
    `t` until they are drawn (section 13, item 6).
  - The terrain (CFOG-DRAW-001, CFOG-DRAW-002), the entities and the other draws of
    CFOG-DRAW-003 read `rs` and `re` of step 5. With Improved Transparency on, the terrain thus
    fades into the sky image at `e`.

Provenance: `design`. The owner decided on 2026-11-13 that the render-distance fog follows the
drawn columns (CFOG-DEV-005). Kind: semantic. Confidence: high. Milestone: 1.
