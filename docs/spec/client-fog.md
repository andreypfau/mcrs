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
  Transparency". The default is off.
- **Cloud Distance** (`c`): the video option `options.renderCloudsDistance`, in chunks, from 2 to
  128. The default is 128.
- `lerp(t, a, b)`: `a + t · (b − a)`. For a colour, `lerp` applies to each channel.
- `smoothstep(a, b, x)`: let `t = clamp((x − a) / (b − a), 0, 1)`. The result is `t² · (3 − 2t)`.
  The formula also applies when `a > b`.
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
state it again.

Out of scope, one line for each:

- The fog when the camera medium is water, lava or powder snow.
- The fog of the effects blindness and darkness, and the brighter fog of night vision.
- The fog of the panorama and of the menus. The GUI takes no fog (CFOG-TIME-001).
- The fog and the darker world of a boss bar.
- The fade of a new chunk section, the option `options.chunkFade`.
- The sky of the skybox `end`.
- The geometry of the sun, the moon, the stars and the sunrise fan, and the shape of the clouds.

### 0.6 Artefacts cited

- The shaders of the client jar: `assets/minecraft/shaders/include/fog.glsl`,
  `assets/minecraft/shaders/core/terrain.fsh`, `assets/minecraft/shaders/core/sky.fsh`,
  `assets/minecraft/shaders/core/sky_occluder.fsh`, `assets/minecraft/shaders/core/clouds.fsh`
  and the other files of `assets/minecraft/shaders/core/` that a rule names.
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
  5. `rs` and `re`: MENU-DIST-001, step 6.
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

- **Observer:** a player: the pixels that no draw covers.
- **Inputs:** `C` of the frame.
- **Behavior:**
  1. The client clears the image to `C`, with the alpha 1, before it draws the sky.
  2. With Improved Transparency off, the client draws in this order: the sky (CFOG-SKY-001 to
     CFOG-SKY-003), the opaque terrain and the opaque draws, the translucent draws and the
     translucent terrain, the clouds (CFOG-CLOUD-001), the rain and the snow.
  3. With Improved Transparency on, the client draws the sky into a separate sky image, which
     it also clears to `C`, and the clouds into a separate cloud image with depth. It clears the
     main image to `C` and draws the opaque terrain and the opaque draws into it. It then copies
     the sky image into the pixels that no opaque draw covers. The translucent draws, the rain
     and the snow, and the cloud image come after.
- **Postconditions:** a pixel that no draw covers has the colour `C`. In the overworld at
  noon in the plains with `r = 5`, that colour is `(176, 205, 255)`.

Provenance: `black_box_confirmed` for the postcondition (frame artefact,
`pig_idle.front.background.png`, the pixels at the horizon, where the last terrain meets the
sky);
`source_suggested` for the order. Kind: semantic. Confidence: high. Experiment: CFOG-EXP-01.
Milestone: 1 for steps 1 and 2. `later` for step 3.

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
high for `q = 0`, medium for `q > 0`. Experiment: CFOG-EXP-04. Milestone: 1 for `q = 0`.
`later` for `q > 0`.

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
`source_suggested` for steps 2 to 4. Kind: semantic. Confidence: high for steps 1 and 5 to 7,
medium for steps 2 to 4. Experiment: CFOG-EXP-01, CFOG-EXP-02, CFOG-EXP-04, CFOG-EXP-05.
Milestone: 1.

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
  - The distances are those of the point itself. The vanilla client computes them at the
    vertices of faces of one block, and interpolates them. For such faces the difference is
    below `1/255`. CFOG-SKY-001 states the one draw where the difference is visible.
- **Edge cases:** when `es ≥ ee` or `rs ≥ re`, the ramp is a step at the start: 0 at or before
  the start, 1 after it.

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
- **Postconditions:** at `dc ≥ re` the terrain has the colour `C`, as the sky at the horizon
  (CFOG-SKY-001) and the clear colour (CFOG-TIME-002).

Provenance: `source_suggested_black_box_confirmed` (`assets/minecraft/shaders/core/terrain.fsh`;
frame artefact). Kind: semantic. Confidence: high. Milestone: 1.

### CFOG-DRAW-002: Terrain with Improved Transparency

- **Observer:** a player: with the option on, the far terrain fades into the sky behind it, not
  into `C`.
- **Inputs:** a pixel of a block face; `ve` and `vr` of CFOG-MIX-001; the pixel at the same
  place of the sky image and of the cloud image (CFOG-TIME-002, step 3).
- **Behavior:**
  1. Let `v = max(ve, vr)`. Let `b = max(smoothstep(0.9, 1, ve), vr)`.
  2. When `b = 0`, the pixel takes the fog mix of CFOG-MIX-001 step 4.
  3. When `b > 0`, let `K` be the background colour of the pixel:
     1. Start with the colour of the sky image.
     2. When the cloud image has a cloud with the alpha `a > 0` behind the face, mix the colour
        of the cloud into `K` with the fraction `a`. The colour of the cloud is its stored colour
        divided by `a`.
  4. Let `Z = lerp(b, C, K)`. The colour of the pixel is `lerp(v, rgb, Z)`. Its alpha does not
     change.
- **Postconditions:** at the end of the render distance, the terrain has the colour of the sky
  behind it, also at sunrise and with the sun or the moon behind it.

Provenance: `source_suggested_black_box_confirmed` (`assets/minecraft/shaders/core/terrain.fsh`,
the variant `IMPROVED_FOG`; the tooltip `options.improvedTransparency.oitWithImprovedFog.tooltip`
of `assets/minecraft/lang/en_us.json`). Kind: semantic. Confidence: medium. Experiment:
CFOG-EXP-08. Milestone: later.

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

- **Postconditions:** in clear weather in the overworld, a draw at 0.5 blocks has
  `v < 0.001`. In full rain (`q = 1`), the hand has `v ≈ 0.173`.

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
- **Postconditions:** above an elevation of −1°, the occluder has no effect. Below −8°, the sky
  has the colour `C`. The terrain covers the occluder.

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
- **Postconditions:** with the default Cloud Distance of 128 chunks and the overworld,
  `ce = 2048` blocks. A cloud at 1024 blocks has half its alpha. In the nether, the alpha of the
  cloud colour is 0, so the client draws no clouds.

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
| sky disc height and radius | 16, 512 | blocks | CFOG-SKY-001 |
| sky disc angle step | 45 | degrees | CFOG-SKY-001 |
| sunrise fan limit | 0.001 | alpha | CFOG-SKY-002 |
| occluder angles, normal | −1, −8 | degrees | CFOG-SKY-003 |
| occluder angles, eye below `min_y` | 15, 10 | degrees | CFOG-SKY-003 |
| blocks per chunk of the cloud end | 16 | blocks | CFOG-TIME-001 |

---

## 10. Parity tests

Each test is in the overworld in the plains biome, with the camera at `y = 80` and the sky light
15 at the camera cell. The camera looks north. `A(sky_fog_end_distance) = 512`, `c = 128`.

### CFOG-TEST-001: Noon in clear weather

- **Inputs:** time 6000, `R = T = 0`, `q = 0`, `r = 12`.
- **Postconditions:**
  - `F0 = #c0d8ff = (0.752941, 0.847059, 1)`, `S0 = #78a7ff = (0.470588, 0.654902, 1)`,
    `U.a = 0`.
  - `k = 0.14626175`. `C = (0.711644, 0.818954, 1)`, the bytes `(181, 209, 255)`.
  - `es = 0`, `ee = 1024`, `rs = 172.8`, `re = 192`, `se = 192`, `ce = 2048`.
  - The offset `(100, −20, 50)`: `ds = 113.578`, `dc = 111.803`, `v = 0.110916`.
  - The offset `(180, 0, 0)`: `ve = 0.175781`, `vr = 0.375`, `v = 0.375`.
  - The offset `(150, −100, 0)`: `ds = 180.278`, `dc = 150`, `v = 0.176052`.
  - At the zenith, the sky pixel is `lerp(0.083333, S0, C)`.

### CFOG-TEST-002: Noon with `r = 5`

- **Inputs:** as CFOG-TEST-001, with `r = 5`.
- **Postconditions:**
  - `k = 0.22156569`. `C = (176.05, 205.14, 255) / 255`, the bytes `(176, 205, 255)`.
    `rs = 72`, `re = 80`, `se = 80`.
  - In `pig_lineup.front.background.png`, the camera looks north with the pitch 15° and the
    field of view 70°. The view ray of the pixel (427, 5) has the elevation 19.4°. It meets the
    disc at the horizontal distance 45.5 blocks, so `λ = 0.0888`, `d = 60.1` and `v = 0.751`.
    The rule gives `(162.1, 195.7, 255)`. The frame has `(162, 196, 255)`.

### CFOG-TEST-003: Midnight in clear weather

- **Inputs:** time 18000, `R = T = 0`, `q = 0`, `r = 12`.
- **Postconditions:** the timeline `day` multiplies `F0` by `#0f0f16` and `S0` by `#000000`, and
  `U.a = 0`. `F0 = (0.044291, 0.049827, 0.086275)`, `S0 = (0, 0, 0)`.
  `C = (0.037813, 0.042539, 0.073656)`, the bytes `(10, 11, 19)`. The distances are those of
  CFOG-TEST-001.

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

- **Steps:** as CFOG-EXP-02 with Improved Transparency on, at the time 12732, looking west.
- **Record:** the terrain at the end of the render distance must have the colour of the sky
  pixel above it, not `C`.

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

---

## 13. Unresolved

1. CFOG-DRAW-002: the owner decides if mcrs draws Improved Transparency in milestone 1. The
   options are:
   - (a) milestone 1 uses CFOG-DRAW-001 step 1 for each value of the option, and a later
     release adds CFOG-DRAW-002;
   - (b) milestone 1 has both, so the option has its effect from the start.
2. CFOG-ENV-002 and CFOG-COL-001: `R` and `T`, and how they change, belong to a weather
   specification that does not exist yet (`environment-attributes.md`, open question 3).
3. CFOG-ENV-002: this document does not state if `q` keeps its value when the client goes to
   another world or dimension.
4. CFOG-COL-001, steps 3 and 4: in rain, the sky term is dark two times. The weather layer of
   ENVA-LAYER-005 already moves `S0` toward gray, and steps 3 and 4 then scale it again. With
   `R = 1` the sky term is half of `S0`, and the horizon is darker than the sky disc above it.
   The owner decides:
   - (a) keep steps 3 and 4, as the reference does;
   - (b) remove steps 3 and 4, if the second step of darkness is a remainder of the time before the
     weather layer. Then CFOG-TEST-004 changes, and an entry goes in section 12.

---

## 14. The subset of mcrs

### CFOG-MCRS-001: What mcrs draws now

This is a record of the state at the time of this document, not a rule.

- The image is cleared to `A(fog_color)`, with no step of CFOG-COL-001 or CFOG-COL-002.
- The sky disc is a fan at the height 16 with the radius `A(sky_fog_end_distance)`, not 512. Its
  fog value is the vertex distance divided by `A(sky_fog_end_distance)`, not by `se`. It mixes
  toward `A(fog_color)`, not toward `C`.
- When the camera `y` is below 63, a black disc 4 blocks below the camera covers the lower sky.
  `26.4-snapshot-7` has no such disc. It has the sky occluder of CFOG-SKY-003.
- The clouds fade over `A(cloud_fog_end_distance)`, with no limit from Cloud Distance.
- The terrain, the entities, the particles and the block outline take no fog. MENU-DIST-001
  steps 5 to 7 are not implemented.

### CFOG-MCRS-002: The first fog of mcrs

- **Observer:** a player.
- **Behavior:** the first release of the fog in mcrs implements the rules with the milestone 1:
  1. CFOG-TIME-001 with `q = 0`, and CFOG-TIME-002 steps 1 and 2;
  2. CFOG-COL-001;
  3. CFOG-ENV-001 with `q = 0`, and CFOG-MIX-001;
  4. MENU-DIST-001 steps 5 to 7;
  5. CFOG-DRAW-001 step 1, and the rows of CFOG-DRAW-003 with the milestone 1;
  6. CFOG-SKY-001 to CFOG-SKY-003, in place of the black disc of CFOG-MCRS-001;
  7. CFOG-CLOUD-001.
- **Postconditions:** the terrain at the end of the render distance, the sky at the horizon,
  the sky below the horizon and the clear image have the same colour `C`.

Provenance: `design`. These rules give the render-distance fog with a colour that meets the
sky at the horizon. The rain fog, the void darkness and Improved Transparency change that
colour only in some scenes. Kind: semantic. Confidence: high. Milestone: 1.

### CFOG-MCRS-003: Fog distances for each pixel

- **Observer:** a player.
- **Behavior:** mcrs computes `ds` and `dc` of a terrain pixel, an entity pixel and a particle
  pixel from the world position of the pixel. It does not interpolate them from the vertices.
- **Postconditions:** a terrain face longer than one block gets the fog of each of its points.

Provenance: `design`. The result is that of the vanilla client within `1/255` for faces of one
block (CFOG-MIX-001), and a face of mcrs can be longer than one block. The sky disc keeps the
interpolation of CFOG-SKY-001, because the image shows it. Kind: semantic. Confidence: high.
Milestone: 1.
