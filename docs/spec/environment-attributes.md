# Environment attributes, timelines and clocks: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of the game values: `assets/minecraft/version.json`.

An environment attribute is a value of a dimension that data packs control, for example the
colour of the sky or the chance that a turtle egg hatches. This document states the value
types of the attributes, the arithmetic of each modifier, the order in which the sources of a
value apply, the sampling of a timeline, and the clocks that timelines read. The JSON shapes
are in `registry-fields.md`, REG-FLD-128, REG-FLD-129 and REG-FLD-251 to REG-FLD-255. This
document does not restate them. It does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Attribute**: an entry of the built-in registry `minecraft:environment_attribute`. REG-FLD-253
  lists the attribute ids, their types, the flag "syncable" and the flag "positional".
- **Attribute type**: the kind of value of an attribute (ENVA-TYPE-001).
- **Value**: a value of the type of an attribute.
- **Modifier**: an operation that makes a new value from a value and an argument. Its id is
  one of the modifier ids of REG-FLD-255.
- **Subject**: the value to which a modifier applies.
- **Argument**: the second input of a modifier. Its kind depends on the attribute type and the
  modifier (ENVA-MOD-001).
- **Entry**: one modifier and one argument for one attribute, in an attribute map (REG-FLD-253)
  or in a built-in layer.
- **Layer**: one step that makes a new value of an attribute from the value of the steps
  before it (ENVA-LAYER-001).
- **Track**: the field `tracks.<attribute id>` of a timeline (REG-FLD-252). It has a modifier,
  keyframes and an easing.
- **Keyframe**: one element of `keyframes` of a track: a tick count and an argument.
- **Segment**: a pair of keyframes with a start tick and an end tick (ENVA-TL-002).
- **Easing**: a function from `[0, 1]` to the real numbers, named by `ease` (ENVA-TL-004).
- **Timeline**, **period**, **time marker**, **clock**, **clock state**, **total ticks**,
  **partial tick**, **rate**, **game time**: as `command-time.md`, section 0.1, defines them.
- **Float**: an IEEE 754 binary32 number. Arithmetic on floats rounds each operation to
  binary32, unless a rule says "binary64".
- **Double**: an IEEE 754 binary64 number.
- **`lerp(t, a, b)`**: `a + t · (b − a)`, in the arithmetic of `a` and `b`.
- **`floor(x)`**: the largest integer that is not greater than `x`.
- **`x mod p`** for integers: the remainder with the sign of `x`. **`floormod(x, p)`**: the
  remainder in `[0, p)`.
- **Sanitize**: the last step of a value (ENVA-TYPE-004).
- **Environment artefact**: the file
  `crates/mcrs_minecraft_environment/tests/fixtures/vanilla/environment.json`. The task
  `dumpEnvironment` of `tools/vanilla-oracle` writes it. Floats in it are JSON numbers that are
  exactly the binary32 values.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". Each rule has a Milestone line. `1` means the first release
of these rules, which replaces the present code. `later` means a rule that a later release
implements. A rule that cites the environment artefact as evidence has the label
`source_suggested_black_box_confirmed`.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `ENVA-TYPE` | attribute types, the attribute table, ranges and interpolation |
| `ENVA-MOD` | the modifiers |
| `ENVA-MAP` | how an attribute map is read |
| `ENVA-LAYER` | the order of the layers and the value in a frame |
| `ENVA-TL` | timeline tracks: validation and sampling |
| `ENVA-CLK` | the clocks that the client keeps |
| `ENVA-TEST` | parity tests |
| `ENVA-DEV` | intentional deviations |

### 0.4 Scope

In scope: the value types, the attribute table, the modifiers, the layers, timeline tracks, and
the clock of the client. The server clock and the commands that change a clock are in
`command-time.md`, CMD-TIME-005 to CMD-TIME-019. This document cites them.

Out of scope: what each consumer does with a value, for example how the sky renderer uses
`visual/sky_color`. The consumer specification owns that. `client-fog.md` states the consumers
of the fog, sky and cloud attributes when the camera is in air.

---

## 1. Attribute types

### ENVA-TYPE-001: Values of each type

- **Behavior:** each attribute type has one kind of value:

| Attribute type | Value |
|----------------|-------|
| `boolean` | true or false |
| `tri_state` | `true`, `false` or `default` |
| `float` | a float |
| `angle_degrees` | a float, an angle in degrees |
| `integer` | a signed 32-bit integer |
| `rgb_color` | three floats: red, green and blue |
| `argb_color` | four floats: red, green, blue and alpha |
| `moon_phase`, `activity`, `bed_rule`, `particle`, `background_music`, `ambient_sounds`, `mob_spawn_settings` | a value of the shape of REG-FLD-254, which no modifier other than `override` and `overlay` changes |
| `ambient_particles` | a list of pairs of a particle and a probability (a float) |

- **Colour channels:** a channel is a float. No rule clamps a channel to `[0, 1]`, except where
  a modifier states a clamp. A channel outside `[0, 1]` stays as it is until the consumer uses
  it.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Milestone: 1.

### ENVA-TYPE-002: Colour on read

- **Inputs:** the JSON form of a colour (REG-FLD-254).
- **Behavior:**
  1. A hex string gives each channel as its byte divided by 255, in float arithmetic. An
     `rgb_color` from a hex string has no alpha.
  2. An integer form of an `rgb_color` uses the low 24 bits as `0xRRGGBB`. The top 8 bits have
     no effect.
  3. An integer form of an `argb_color` uses all 32 bits as `0xAARRGGBB`.
  4. A list form gives the channels as the floats of the list, in the order `[r, g, b]` or
     `[r, g, b, a]`. The reader does not clamp them.
  5. A hex string starts with `#`. After `#`, an `rgb_color` has exactly 6 hex digits and an
     `argb_color` has exactly 8 hex digits. A digit can be a capital letter or a small letter.
     Another string is a content error.
- **Postconditions:** `"#78a7ff"` reads as `(120/255, 167/255, 1)`. `[1.5, -0.5, 0.5]` reads as
  `(1.5, −0.5, 0.5)`.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Milestone: 1.

### ENVA-TYPE-003: Colour on write

- **Inputs:** a colour value that the server writes in a file form or in a network form.
- **Behavior:** the writer uses the hex form of REG-FLD-254. Each byte is
  `floor(clamp(c, 0, 1) · 255)`, in float arithmetic, for the channel `c`.
- **Postconditions:** a value read from a hex string is written back as the same hex string.

Provenance: `design`. The reference takes the byte as `floor(c · 255)` modulo 256, with no
clamp, so a channel above 1 wraps to a dark value. The vanilla client accepts each byte, so the
choice is free (ENVA-DEV-001). Kind: semantic. Confidence: high. Milestone: 1.

### ENVA-TYPE-004: Range and sanitize

- **Inputs:** the range of an attribute (ENVA-TYPE-006) and a value.
- **Behavior:**
  1. On read, a value in the value form must be in the range. The value form is the bare value
     of an entry, the argument of an `override` entry, and the value of a keyframe of an
     `override` track. A float outside the range fails the load with the text
     `<v> is not in range [<min>; <max>]`.
  2. An argument of a modifier other than `override` is not checked against the range of the
     attribute.
  3. After the last layer, the value is sanitized. A float outside the range becomes the
     nearer bound. A NaN stays NaN. A value of another type does not change.
- **Postconditions:** `{"modifier": "multiply", "argument": 0.85}` is valid for
  `visual/water_fog_end_distance`. A sum of layers that is less than 0 for
  `visual/fog_end_distance` gives 0.

Provenance: `source_suggested_black_box_confirmed` (environment artefact, `range_clamp`). Kind:
semantic. Confidence: high. Milestone: 1. Experiment: ENVA-TEST-001.

### ENVA-TYPE-005: Interpolation functions

- **Inputs:** an attribute type, a use, a fraction `t` (a float) and two values `a` and `b`.
- **Behavior:** each type has one interpolation function for each of the four uses: keyframe
  (ENVA-TL-003), weather (ENVA-LAYER-005), spatial (ENVA-LAYER-004) and partial tick
  (ENVA-LAYER-007).

| Attribute type | Keyframe | Weather | Spatial | Partial tick |
|----------------|----------|--------------|---------|--------------|
| `float` | linear | linear | linear | linear |
| `angle_degrees` | linear | linear | linear | degrees |
| `rgb_color`, `argb_color` | channel | channel | channel | channel |
| `integer` | integer | integer | integer | integer |
| `ambient_particles` | cross-fade | cross-fade | cross-fade | cross-fade |
| each other type | step 1 | step 0 | step 0.5 | step 0 |

- **The functions:**
  - **linear**: `lerp(t, a, b)` in float arithmetic.
  - **channel**: linear for each channel, the alpha too.
  - **integer**: `a + floor(t · d)`, where `d` is `b − a` in wrapping 32-bit arithmetic, and the
    product is a float.
  - **degrees**: let `d = wrap(b − a)`. When `|d| ≥ 90`, the result is `b`. Otherwise the result
    is `a + t · d`. `wrap(x)`: let `r` be the float remainder of `x` by 360 with the sign of
    `x`. When `r ≥ 180`, subtract 360. When `r < −180`, add 360.
  - **step `s`**: `b` when `t ≥ s`, and `a` otherwise.
  - **cross-fade**: when `t = 0`, the result is `a`. When `t = 1`, the result is `b`. Otherwise
    the result is the list of `a` with each probability multiplied by `1 − t`, followed by the
    list of `b` with each probability multiplied by `t`.
- **Postconditions:** the partial-tick function of `angle_degrees` from 350 to 10 at `t = 0.5`
  gives 360. From 0 to 90 it gives 90 for each `t`.

Provenance: `source_suggested_black_box_confirmed` (environment artefact, `type_lerps`). Kind:
semantic. Confidence: high. Milestone: 1. Experiment: ENVA-TEST-001.

### ENVA-TYPE-006: The attribute table

- **Behavior:** each attribute has a default value, a range and a flag "spatially
  interpolated". The range is "any" unless the table states it. The type, "syncable" and
  "positional" are in REG-FLD-253. The ids have the namespace `minecraft`.

| Attribute | Default | Range | Spatially interpolated |
|-----------|---------|-------|------------------------|
| `visual/fog_color` | `#000000` | any | yes |
| `visual/fog_start_distance` | 0 | any | yes |
| `visual/fog_end_distance` | 1024 | `[0, +∞]` | yes |
| `visual/sky_fog_end_distance` | 512 | `[0, +∞]` | yes |
| `visual/cloud_fog_end_distance` | 2048 | `[0, +∞]` | yes |
| `visual/water_fog_color` | `#050533` | any | yes |
| `visual/water_fog_start_distance` | −8 | any | yes |
| `visual/water_fog_end_distance` | 96 | `[0, +∞]` | yes |
| `visual/sky_color` | `#000000` | any | yes |
| `visual/sunrise_sunset_color` | `#00000000` | any | yes |
| `visual/cloud_color` | `#00000000` | any | yes |
| `visual/cloud_height` | 192.33 | any | yes |
| `visual/sun_angle` | 0 | any | yes |
| `visual/moon_angle` | 0 | any | yes |
| `visual/star_angle` | 0 | any | yes |
| `visual/moon_phase` | `full_moon` | any | no |
| `visual/star_brightness` | 0 | `[0, 1]` | yes |
| `visual/has_sky_occluder` | false | any | no |
| `visual/block_light_tint` | `#ffd88c` | any | yes |
| `visual/sky_light_color` | `#ffffff` | any | yes |
| `visual/sky_light_factor` | 1 | `[0, 1]` | yes |
| `visual/night_vision_color` | `#999999` | any | yes |
| `visual/ambient_light_color` | `#000000` | any | yes |
| `visual/default_dripstone_particle` | `{"type": "minecraft:dripping_dripstone_water"}` | any | no |
| `visual/ambient_particles` | `[]` | any | no |
| `audio/background_music` | `{}` | any | no |
| `audio/music_volume` | 1 | `[0, 1]` | no |
| `audio/ambient_sounds` | `{}` | any | no |
| `audio/firefly_bush_sounds` | false | any | no |
| `gameplay/sky_light_level` | 15 | `[0, 15]` | no |
| `gameplay/can_start_raid` | true | any | no |
| `gameplay/water_evaporates` | false | any | no |
| `gameplay/bed_rule` | `{"can_sleep": "when_dark", "can_set_spawn": "always", "error_message": {"translate": "block.minecraft.bed.no_sleep"}}` | any | no |
| `gameplay/straw_bed_rule` | `{"can_sleep": "when_dark", "can_set_spawn": "never", "destroy_on_leave": true, "error_message": {"translate": "block.minecraft.bed.no_sleep"}}` | any | no |
| `gameplay/respawn_anchor_works` | false | any | no |
| `gameplay/nether_portal_spawns_piglin` | false | any | no |
| `gameplay/fast_lava` | false | any | no |
| `gameplay/increased_fire_burnout` | false | any | no |
| `gameplay/eyeblossom_open` | `default` | any | no |
| `gameplay/turtle_egg_hatch_chance` | 0.002 | `[0, 1]` | no |
| `gameplay/piglins_zombify` | true | any | no |
| `gameplay/snow_golem_melts` | false | any | no |
| `gameplay/creaking_active` | false | any | no |
| `gameplay/surface_slime_spawn_chance` | 0 | `[0, 1]` | no |
| `gameplay/cat_waking_up_gift_chance` | 0 | `[0, 1]` | no |
| `gameplay/bees_stay_in_hive` | false | any | no |
| `gameplay/monsters_burn` | false | any | no |
| `gameplay/can_pillager_patrol_spawn` | true | any | no |
| `gameplay/natural_mob_spawns` | `{"spawns_by_category": {}, "spawn_costs": {}}` | any | no |
| `gameplay/creature_world_gen_spawn_probability` | 0.1 | `[0, 0.9999999]` | no |
| `gameplay/villager_activity` | `minecraft:idle` | any | no |
| `gameplay/baby_villager_activity` | `minecraft:idle` | any | no |

- **Notes:** the defaults of `background_music` and `ambient_sounds` have no fields. A float
  default is the binary32 value nearest to the decimal of the table.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Milestone: 1. Experiment: in
a dimension type with `"attributes": {}`, query each syncable attribute on the client. The
client shows the default of this table.

---

## 2. Modifiers

### ENVA-MOD-001: Kind of the argument

- **Behavior:** the argument kind of each modifier is:

| Attribute type | Modifier | Argument |
|----------------|----------|----------|
| each type | `override` | a value of the type |
| `float`, `angle_degrees` | `add`, `subtract`, `multiply`, `minimum`, `maximum` | a float |
| `float`, `angle_degrees` | `alpha_blend` | a float `value` and a float `alpha` in `[0, 1]` |
| `integer` | `add`, `subtract`, `multiply`, `minimum`, `maximum` | an integer |
| `boolean` | `and`, `nand`, `or`, `nor`, `xor`, `xnor` | a boolean |
| `rgb_color` | `add`, `subtract`, `multiply` | an `rgb_color` |
| `rgb_color` | `alpha_blend` | an `argb_color` |
| `argb_color` | `add`, `subtract` | an `rgb_color` |
| `argb_color` | `multiply`, `alpha_blend` | an `argb_color` |
| `rgb_color`, `argb_color` | `blend_to_gray` | a float `brightness` in `[0, 1]` and a float `factor` in `[0, 1]` |
| `ambient_particles` | `append` | a value of the type |
| `mob_spawn_settings` | `overlay` | a value of the type |

- **Validation:** a modifier that is not in the row of the attribute type fails the load. An
  `alpha`, a `brightness` or a `factor` outside `[0, 1]` fails the load. The JSON forms are in
  REG-FLD-255.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Milestone: 1.

### ENVA-MOD-002: `override`

- **Behavior:** the result is the argument.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Milestone: 1.

### ENVA-MOD-003: Float modifiers

- **Inputs:** a float subject `s` and the argument of ENVA-MOD-001.
- **Behavior:**
  - `add`: `s + a`. `subtract`: `s − a`. `multiply`: `s · a`. All in float arithmetic.
  - `minimum`: the smaller of `s` and `a`. `maximum`: the larger. When one input is NaN, the
    result is NaN. For these two modifiers, −0 is smaller than +0.
  - `alpha_blend`: `lerp(alpha, s, value)`.

Provenance: `source_suggested_black_box_confirmed` (environment artefact, `modifiers.float`).
Kind: semantic. Confidence: high. Milestone: 1. Experiment: ENVA-TEST-001.

### ENVA-MOD-004: Integer modifiers

- **Behavior:** `add`, `subtract` and `multiply` use wrapping signed 32-bit arithmetic.
  `minimum` and `maximum` give the smaller and the larger input.

Provenance: `source_suggested_black_box_confirmed` (environment artefact,
`modifiers.integer`). Kind: semantic. Confidence: high. Milestone: 1. No attribute of the game
has the type `integer`.

### ENVA-MOD-005: Boolean modifiers

- **Behavior:** for the subject `s` and the argument `a`:

| Modifier | Result |
|----------|--------|
| `and` | `s ∧ a` |
| `nand` | `¬(s ∧ a)` |
| `or` | `s ∨ a` |
| `nor` | `¬(s ∨ a)` |
| `xor` | `s ≠ a` |
| `xnor` | `s = a` |

Provenance: `source_suggested_black_box_confirmed` (environment artefact,
`modifiers.boolean`). Kind: semantic. Confidence: high. Milestone: 1.

### ENVA-MOD-006: Colour modifiers

- **Inputs:** a colour subject `s` and a colour argument `a` (ENVA-MOD-001). All arithmetic is
  float arithmetic, for each channel.
- **Behavior:**
  - `add`: each of red, green and blue is `min(s_c + a_c, 1)`. The alpha of an `argb_color`
    subject does not change.
  - `subtract`: each of red, green and blue is `max(s_c − a_c, 0)`. The alpha of an
    `argb_color` subject does not change.
  - `multiply`: each channel of the subject is `s_c · a_c`. For an `argb_color`, this includes
    the alpha.
  - `alpha_blend` on an `rgb_color` subject: when `a_alpha = 0`, the result is `s`. Otherwise
    each channel is `lerp(a_alpha, s_c, a_c)`.
  - `alpha_blend` on an `argb_color` subject:
    1. When `a_alpha = 1`, the result is `a`. When `a_alpha = 0`, the result is `s`.
    2. Otherwise let `A = a_alpha + s_alpha · (1 − a_alpha)`. The alpha of the result is `A`.
    3. Each of red, green and blue is `(a_c · a_alpha + s_c · (A − a_alpha)) / A`.
- **Postconditions:** `add` does not clamp at 0, and `subtract` does not clamp at 1. A subject
  channel of 1.5 stays 1.5 after `subtract` with 0.

Provenance: `source_suggested_black_box_confirmed` (environment artefact,
`modifiers.rgb_color` and `modifiers.argb_color`). Kind: semantic. Confidence: high.
Milestone: 1. Experiment: ENVA-TEST-001.

### ENVA-MOD-007: `blend_to_gray`

- **Inputs:** a colour subject `s`, a `brightness` and a `factor`.
- **Behavior:**
  1. Let `g = 0.3 · s_r + 0.59 · s_g + 0.11 · s_b`, added from left to right in float
     arithmetic.
  2. Let `m = clamp(g · brightness, 0, 1)`.
  3. Each of red, green and blue of the result is `lerp(factor, s_c, m)`.
  4. The alpha of an `argb_color` subject does not change.

Provenance: `source_suggested_black_box_confirmed` (environment artefact,
`modifiers.*.blend_to_gray`). Kind: semantic. Confidence: high. Milestone: 1.

### ENVA-MOD-008: `append` and `overlay`

- **Behavior:**
  - `append`: the result is the list of the subject, followed by the list of the argument.
  - `overlay` on the mob spawn settings `S` (subject) and `A` (argument):
    1. For each mob category, when `A` has the category in `spawns_by_category`, the result has
       the list of `A`. The list of `A` is used also when it is empty.
    2. Otherwise, when `S` has the category, the result has the list of `S`.
    3. Otherwise the result does not have the category.
    4. `spawn_costs` of the result has each entry of `S` and each entry of `A`. For an entity
       type in both, the entry of `A` is used.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Milestone: 1.

### ENVA-MOD-009: Interpolation of an argument

- **Behavior:** a track interpolates its arguments between two keyframes (ENVA-TL-003) with the
  function of its modifier:

| Modifier | Interpolation of the argument |
|----------|-------------------------------|
| `override`, `append` | the keyframe function of the attribute type (ENVA-TYPE-005) |
| float `add`, `subtract`, `multiply`, `minimum`, `maximum` | linear |
| float `alpha_blend` | linear on `value`, and linear on `alpha` |
| integer modifiers | integer |
| boolean modifiers | step 1 |
| colour `add`, `subtract`, `multiply`, `alpha_blend` | channel, with the channels of the argument kind |
| `blend_to_gray` | linear on `brightness`, and linear on `factor` |
| `overlay` | step 1 |

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Milestone: 1.

---

## 3. Attribute maps

### ENVA-MAP-001: Read of an attribute map

- **Inputs:** an attribute map (REG-FLD-253) of a dimension type or of a biome.
- **Behavior:**
  1. An unknown attribute id fails the load.
  2. The reader tries the value form of the attribute first. When the value form is valid, the
     entry has the modifier `override`.
  3. Otherwise the entry is the object with `modifier` and `argument`, and ENVA-MOD-001 checks
     it.
  4. The reader ignores an unknown field of an object, as REG-ENTRY-003 states. This applies
     to each object of a value, of an argument and of a time marker.
  5. The JSON numbers `1` and `0` are valid values of a `boolean`. `1` is `true` and `0` is
     `false`. Another number for a `boolean` is outside the domain.
  6. A JSON number keeps the sign of a zero: `-0.0` reads as `−0`. ENVA-DEV-002 states an
     allowed difference.
- **Postconditions:** an object for `audio/background_music` is always a value, because each
  field of that value is optional. `{"argument": {}, "modifier": "override"}` for that
  attribute is a value with no fields.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: 1. Experiment:
add a data pack whose dimension type has
`"minecraft:audio/background_music": {"argument": {}, "modifier": "override"}`. The pack loads.

---

## 4. Layers

### ENVA-LAYER-001: Order of the layers

- **Inputs:** an attribute, a dimension, and optional a position.
- **Behavior:** the value starts at the default of ENVA-TYPE-006. These layers apply, in this
  order:
  1. The entry for the attribute in `attributes` of the dimension type.
  2. The biome layer (ENVA-LAYER-003), only when the read has a position.
  3. For each timeline of the tag `timelines` of the dimension type, in the order of the tag:
     the track of the timeline for the attribute (ENVA-TL-005).
  4. The weather layer (ENVA-LAYER-005).
  5. On the client only: the flash layer (ENVA-LAYER-006).
  6. Sanitize (ENVA-TYPE-004).
- **A layer with no entry:** a layer that has no entry for the attribute does not change the
  value.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Milestone: 1 for items 1, 3
and 6; later for items 2, 4 and 5.

### ENVA-LAYER-002: Dimension value and position value

- **Behavior:**
  - A read with no position skips the biome layer. This value is the dimension value.
  - A consumer reads a positional attribute (REG-FLD-253) at a position. It reads a
    non-positional attribute with no position.
  - A biome file cannot contain a non-positional attribute (REG-FLD-253). Thus a
    non-positional attribute has the same value at each position.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: later.

### ENVA-LAYER-003: Biome layer

- **Inputs:** an attribute and a position `p` (three doubles).
- **Behavior:**
  1. When the read comes from the client frame (ENVA-LAYER-007) and the attribute is spatially
     interpolated, use ENVA-LAYER-004.
  2. Otherwise take the biome that the chunk stores for the 4 × 4 × 4 cell that contains the
     block `(floor(p_x), floor(p_y), floor(p_z))`. Apply the entry of that biome for the
     attribute.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: later.

### ENVA-LAYER-004: Spatial interpolation of biomes

- **Inputs:** a spatially interpolated attribute, the value `v` before the biome layer, and the
  camera position `p`.
- **Behavior:**
  1. Let `q = p / 4 − (0.5, 0.5, 0.5)`, in doubles. Let `i = floor(q)` and `f = q − i` on each
     axis.
  2. Let `K = [0, 1, 4, 6, 4, 1, 0]`. For each axis and each `k` from 0 to 5, the cell is
     `i − 2 + k`, and its weight is `lerp(f, K[k + 1], K[k])` in doubles.
  3. Visit the cells with Z in the outer loop, X in the middle loop and Y in the inner loop.
     The weight of a cell is the product of its three axis weights.
  4. For each cell, take the biome that the chunk stores for that 4 × 4 × 4 cell. Add the
     weight to the group of the biome. All biomes with no attribute entries form one group.
     The groups keep the order in which they first occur.
  5. When there are no groups, the result is `v`. When there is one group, apply its entry to
     `v`.
  6. Otherwise, for each group in order, let `u` be its entry applied to `v`. Let `W` be the
     sum of the weights of the groups up to and including this one. The first group gives
     `r = u`. Each next group gives `r = spatial(w / W, r, u)`, where `w / W` is converted to a
     float and `spatial` is the function of ENVA-TYPE-005.
- **Postconditions:** for linear types the result is the mean of the group values, weighted by
  the group weights.

Provenance: `source_suggested_black_box_confirmed` (environment artefact, `spatial` and
`gaussian`). Kind: semantic. Confidence: medium. Milestone: later.

### ENVA-LAYER-005: Weather layer

- **Inputs:** an attribute, the rain level `R` and the thunder level `T` of the dimension, at
  the time of the read. `T` here is the thunder level multiplied by the rain level.
- **Behavior:**
  1. The layer exists only when the dimension type has sky light, has no ceiling, and the
     dimension is not `minecraft:the_end`.
  2. Let `r = R − T`.
  3. When the rain entry of the table exists and `r > 0`: `v = state(r, v, rain(v))`.
  4. Then, when the thunder entry exists and `T > 0`: `v = state(T, v, thunder(v))`.
  5. `state` is the weather function of ENVA-TYPE-005. `rain(v)` and `thunder(v)` apply
     the entry of the table to `v`.

| Attribute | Rain entry | Thunder entry |
|-----------|------------|---------------|
| `visual/sky_color` | `blend_to_gray` 0.6, 0.75 | `blend_to_gray` 0.24, 0.94 |
| `visual/fog_color` | `multiply` `#7f7f99` | `multiply` `#3f3f4c` |
| `visual/cloud_color` | `blend_to_gray` 0.24, 0.5 | `blend_to_gray` 0.095, 0.94 |
| `gameplay/sky_light_level` | `alpha_blend` value 4, alpha 0.3125 | `alpha_blend` value 4, alpha 0.52734375 |
| `visual/sky_light_color` | `alpha_blend` `#4f7a7aff` | `alpha_blend` `#867a7aff` |
| `visual/sky_light_factor` | `alpha_blend` value 0.24, alpha 0.3125 | `alpha_blend` value 0.24, alpha 0.52734375 |
| `visual/star_brightness` | `override` 0 | `override` 0 |
| `visual/sunrise_sunset_color` | `multiply` `#ff7f7f99` | `multiply` `#ff3f3f4c` |
| `gameplay/bees_stay_in_hive` | `override` true | `override` true |

- **Notes:** a hex colour of the table gives floats as ENVA-TYPE-002 states. For
  `visual/sky_light_color` the argument is an `argb_color`. A step type takes the value of the
  entry at each level above 0.

Provenance: `source_suggested_black_box_confirmed` (environment artefact, `weather`). Kind:
semantic. Confidence: high. Milestone: later.

### ENVA-LAYER-006: Flash layer of the client

- **Behavior:** while the sky of the client shows a lightning flash:
  - `visual/sky_color` becomes `lerp(0.22, v, (0.8, 0.8, 1.0))` for each channel;
  - `visual/sky_light_factor` becomes 1.
- **Notes:** the specification of lightning owns the duration of a flash.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: later.

### ENVA-LAYER-007: The value in a frame of the client

- **Inputs:** an attribute, the camera position, and the partial tick `pt` of the frame.
- **Behavior:**
  1. At each client tick, the client computes the value of the attribute at the camera
     position with the state of that tick. This is the tick value.
  2. In a frame, the value is `partial(pt, previous, current)`. `previous` is the tick value of
     the previous tick, and `current` is the tick value of this tick. `partial` is the
     partial-tick function of ENVA-TYPE-005.
  3. When the client did not read the attribute in the previous tick, `previous` is
     `current`.
- **Postconditions:** a float attribute moves smoothly between ticks. A step type changes at the
  start of the tick.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: 1.

### ENVA-LAYER-008: Change of a clock

- **Behavior:** a value that a track gives reads the clock state at the time of the read. After
  a change of a clock, the next read uses the new state (CMD-TIME-005). Two reads in one tick
  with no change between them give the same value.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Milestone: 1.

---

## 5. Timeline tracks

### ENVA-TL-001: Validation of a track

- **Inputs:** a track of a timeline, and the period of the timeline when it has one.
- **Behavior:** the load fails when one of these is true:
  1. The track has no keyframe.
  2. The ticks of the keyframes decrease at one place.
  3. All keyframes of the track have the same tick, and there are 3 or more of them.
  4. Otherwise, 4 or more keyframes in a row have the same tick.
  5. The timeline has a period `p`, and a keyframe has a tick outside `[0, p]`.
  6. `ease` names no easing of ENVA-TL-004.
  7. `ease` is a cubic Bézier curve, and `x1` or `x2` is outside `[0, 1]`.
  8. The modifier or an argument is not valid for the attribute (ENVA-MOD-001, ENVA-TYPE-004).
- **Notes:** a keyframe tick can be equal to the period. A time marker cannot (CMD-TIME-007).
  The fields of a track can be in any order. An unknown field is ignored (REG-ENTRY-003).
- **Postconditions:** the ticks `[0, 50, 50, 50]` are valid. The ticks `[50, 50, 50]` are not
  valid. The ticks `[0, 50, 50, 50, 50]` are not valid.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: 1. Experiment:
add a data pack with a timeline track whose ticks are `[0, 50, 50, 50]`. The pack loads. Change
the ticks to `[50, 50, 50]`. The load fails.

### ENVA-TL-006: A timeline from the network

- **Inputs:** a timeline that the client reads from the registry data of the server
  (REG-FLD-129).
- **Behavior:**
  1. The client ignores a track of an attribute that is not syncable.
  2. The other tracks follow ENVA-TL-001.
- **Postconditions:** the timeline of the client has only tracks of syncable attributes.

Provenance: `design`. The server of REG-FLD-129 never sends such a track, so the rule changes
nothing for a correct server. Kind: semantic. Confidence: high. Milestone: 1.

### ENVA-TL-002: Segments of a track

- **Inputs:** a track with the keyframes `k_0 … k_(n−1)`, and the period `p` when the timeline
  has one.
- **Behavior:**
  1. When `n = 1`, there is one segment from `k_0` to `k_0` with the start and end tick 0, and
     the easing `constant`.
  2. Otherwise, for each `i` from 0 to `n − 2`, there is a segment from `k_i` to `k_(i+1)`. The
     start tick is the tick of `k_i`, and the end tick is the tick of `k_(i+1)`.
  3. With a period, there are two more segments, from `k_(n−1)` to `k_0`. The first goes before
     the others, with the start tick `tick(k_(n−1)) − p` and the end tick `tick(k_0)`. The last
     goes after the others, with the start tick `tick(k_(n−1))` and the end tick
     `tick(k_0) + p`.
  4. Each segment, except the one of item 1, uses the easing of the track.

Provenance: `source_suggested_black_box_confirmed` (environment artefact, `timelines`). Kind:
semantic. Confidence: high. Milestone: 1.

### ENVA-TL-003: Sample of a track

- **Inputs:** the segments of a track and an integer tick `t` (signed 64-bit).
- **Behavior:**
  1. Let `s = floormod(t, p)` when the timeline has a period `p`. Otherwise let `s = t`.
  2. Take the first segment whose end tick is greater than `s`. When there is none, take the
     last segment.
  3. When `s` is not greater than the start tick, the result is the start argument of the
     segment.
  4. When `s` is not less than the end tick, the result is the end argument.
  5. Otherwise let `x = (s − start) / (end − start)`, in float arithmetic. Let `e = ease(x)`.
     The result is the interpolation of ENVA-MOD-009 at `e`, from the start argument to the end
     argument.
- **Postconditions:**
  - Before the first keyframe of a track with no period, the result is the first argument.
    After the last keyframe, it is the last argument.
  - Two keyframes on one tick `T` make a step: the sample at `T` is the argument of the second
    of them.
  - With the easing `constant`, each segment holds its start argument until its end tick.
  - An easing can give a value outside `[0, 1]`. The interpolation then goes past the
    arguments.

Provenance: `source_suggested_black_box_confirmed` (environment artefact, `timelines`). Kind:
semantic. Confidence: high. Milestone: 1. Experiment: ENVA-TEST-001.

### ENVA-TL-004: Easings

- **Inputs:** an easing and a float `x`.
- **Behavior:** the named easings are the functions of the table. Each constant is a float.
  `c1 = 1.70158`, `c2 = 2.5949094`, `c3 = 2.70158`, `c4 = 2π/3` and `c5 = 4π/9`, as floats.

| Name | Result for `x < 0.5` (or for all `x`) | Result for `x ≥ 0.5` |
|------|---------------------------------------|----------------------|
| `constant` | 0 | |
| `linear` | `x` | |
| `in_quad`, `in_cubic`, `in_quart`, `in_quint` | `x²`, `x³`, `x⁴`, `x⁵` | |
| `out_quad`, `out_cubic`, `out_quart`, `out_quint` | `1 − (1 − x)^k` for `k` = 2, 3, 4, 5 | |
| `in_out_quad`, `in_out_cubic`, `in_out_quart`, `in_out_quint` | `2^(k−1) · x^k` for `k` = 2, 3, 4, 5 | `1 − (2 − 2x)^k / 2` |
| `in_sine` | `1 − cos(πx/2)` | |
| `out_sine` | `sin(πx/2)` | |
| `in_out_sine` | `(1 − cos(πx)) / 2` | |
| `in_expo` | 0 when `x = 0`, else `2^(10x − 10)` | |
| `out_expo` | 1 when `x = 1`, else `1 − 2^(−10x)` | |
| `in_out_expo` | 0 when `x = 0`, else `2^(20x − 10) / 2` | 1 when `x = 1`, else `(2 − 2^(10 − 20x)) / 2` |
| `in_circ` | `1 − √(1 − x²)` | |
| `out_circ` | `√(1 − (x − 1)²)` | |
| `in_out_circ` | `(1 − √(1 − (2x)²)) / 2` | `(√(1 − (2 − 2x)²) + 1) / 2` |
| `in_back` | `c3 · x³ − c1 · x²` | |
| `out_back` | `1 + c3 · (x − 1)³ + c1 · (x − 1)²` | |
| `in_out_back` | `(2x)² · ((c2 + 1) · 2x − c2) / 2` | `((2x − 2)² · ((c2 + 1) · (2x − 2) + c2) + 2) / 2` |
| `in_elastic` | 0 when `x = 0`, 1 when `x = 1`, else `−2^(10x − 10) · sin((10x − 10.75) · c4)` | |
| `out_elastic` | 0 when `x = 0`, 1 when `x = 1`, else `2^(−10x) · sin((10x − 0.75) · c4) + 1` | |
| `in_out_elastic` | 0 when `x = 0`, else `−2^(20x − 10) · sin((20x − 11.125) · c5) / 2` | 1 when `x = 1`, else `2^(10 − 20x) · sin((20x − 11.125) · c5) / 2 + 1` |
| `out_bounce` | see below | |
| `in_bounce` | `1 − out_bounce(1 − x)` | |
| `in_out_bounce` | `(1 − out_bounce(1 − 2x)) / 2` | `(1 + out_bounce(2x − 1)) / 2` |

- **`out_bounce(x)`**, with `n = 7.5625` and `d = 2.75`:
  - when `x < 1/d`: `n · x²`;
  - when `x < 2/d`: `n · (x − 1.5/d)² + 0.75`;
  - when `x < 2.5/d`: `n · (x − 2.25/d)² + 0.9375`;
  - otherwise: `n · (x − 2.625/d)² + 0.984375`.
- **Cubic Bézier** `{"cubic_bezier": [x1, y1, x2, y2]}`:
  1. Let `X(u) = ((a_x · u + b_x) · u + c_x) · u`, with `a_x = 3·x1 − 3·x2 + 1`,
     `b_x = −6·x1 + 3·x2` and `c_x = 3·x1`. `Y(u)` is the same with `y1` and `y2`.
  2. Find `u` in `[0, 1]` with `|X(u) − x| < 10⁻⁵`. The result is `Y(u)`.
- **Precision:** a named easing agrees with the environment artefact within `10⁻⁵`. For the
  three sine easings, the bound is `2 · 10⁻⁴`, because the reference uses a table for the sine.
  For a cubic Bézier curve, a steep curve can change `Y` by more than `10⁻⁵` inside the
  tolerance of item 2. A result `y` agrees when a `u` in `[0, 1]` has `|X(u) − x| < 10⁻⁵` and
  `|Y(u) − y| ≤ 10⁻⁶`. The value of the artefact must also agree by this test.

Provenance: `documentation` (the formulas of the named easings are the public set that
easings.net shows) and `source_suggested_black_box_confirmed` (environment artefact, `easings`)
for the names, the constants and the curve. Kind: semantic. Confidence: high. Milestone: 1.

### ENVA-TL-005: A track as a layer

- **Inputs:** a track of a timeline, the value `v` before the track, and the clock of the
  timeline.
- **Behavior:**
  1. Let `t` be the total ticks of the clock. The partial tick of the clock has no effect.
  2. Let `a` be the sample of the track at `t` (ENVA-TL-003).
  3. The result is the modifier of the track applied to `v` and `a`.
- **Postconditions:** on the client, the movement between two ticks comes from
  ENVA-LAYER-007, not from the clock.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Milestone: 1.

---

## 6. Clocks of the client

### ENVA-CLK-001: Clock state of the client

- **Behavior:**
  - The client keeps, for each clock that it knows, the total ticks, the partial tick and the
    rate. It keeps no paused flag. A rate of 0 means that the clock does not advance.
  - A clock that the client did not receive has total ticks 0, partial tick 0 and rate 1.
  - The server keeps its clocks as CMD-TIME-005 and CMD-TIME-006 state. The packet `set_time`
    is CMD-TIME-009.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Milestone: 1.

### ENVA-CLK-002: Advance of the clocks of the client

- **Inputs:** a new game time `G` of the client, and the game time `G0` of the last advance.
- **Behavior:**
  1. Let `d = G − G0`. Set `G0` to `G`.
  2. For each clock: let `x = partial + d · rate`, in doubles. Let `n = floor(x)`.
  3. Set the partial tick to `x − n`, converted to a float. Add `n` to the total ticks.
- **When:** in each client tick that runs normally, the client adds 1 to its game time, then
  advances the clocks to the new game time.
- **Postconditions:** `d` can be 0 or negative. A negative `d` moves the clocks back.

Provenance: `source_suggested_black_box_confirmed` (environment artefact, `client_clock`).
Kind: semantic. Confidence: high. Milestone: 1.

### ENVA-CLK-003: `set_time` on the client

- **Inputs:** the packet `set_time` (CMD-TIME-009).
- **Behavior:**
  1. Set the game time of the client to the game time of the packet.
  2. Advance the clocks to that game time (ENVA-CLK-002).
  3. For each entry, set the total ticks, the partial tick and the rate of its clock to the
     values of the entry.
- **Postconditions:** a packet with no entries only corrects the game time and advances the
  clocks.

Provenance: `source_suggested_black_box_confirmed` (environment artefact, `client_clock`).
Kind: semantic. Confidence: high. Milestone: 1.

### ENVA-CLK-004: Server clock arithmetic

- **Behavior:** the server advances a clock as CMD-TIME-006 states, in float arithmetic for the
  partial tick. A rate of `1/3` gives the total ticks 100, 100, 101 after one, two and three
  ticks from 100 with partial tick 0.

Provenance: `source_suggested_black_box_confirmed` (environment artefact, `server_clock`).
Kind: semantic. Confidence: high. Milestone: 1.

---

## 7. Parity tests

### ENVA-TEST-001: Arithmetic against the environment artefact

- **Inputs:** the environment artefact.
- **Behavior:** for each section of the artefact, compute the result with the rules and compare:

| Section | Rule | Bound |
|---------|------|-------|
| `modifiers` | ENVA-MOD-002 to ENVA-MOD-007 | exact, the sign of zero too |
| `type_lerps` | ENVA-TYPE-005 | exact |
| `easings` | ENVA-TL-004 | ENVA-TL-004, "Precision" |
| `timelines` | ENVA-TL-002, ENVA-TL-003 | for an eased track, `10⁻⁴ · A`, where `A` is the largest magnitude of an argument of the track; exact otherwise |
| `spatial`, `gaussian` | ENVA-LAYER-004 | `10⁻⁶` |
| `weather` | ENVA-LAYER-005 | `10⁻⁶` |
| `range_clamp` | ENVA-TYPE-004 | exact |
| `server_clock` | ENVA-CLK-004 | exact |
| `client_clock` | ENVA-CLK-002, ENVA-CLK-003 | exact |
| `time_markers` | CMD-TIME-007 | exact |

- **Artefact shapes:**
  - A colour is a list of three or four floats. A float `alpha_blend` argument is
    `{"value", "alpha"}`. A `blend_to_gray` argument is `{"brightness", "factor"}`.
  - In `timelines`, each track has `modifier` and `samples`. A sample is `[tick, argument]`.
    The argument is the sample of ENVA-TL-003, before the modifier applies.
  - In `weather`, the thunder level is the level that ENVA-LAYER-005 calls `T`. The last
    element has the base values.
  - In `spatial`, the source `c` is an attribute map with no entries.
  - In `client_clock`, the update is a `set_time` at game time 0. Each step is
    `[game time, total ticks, partial tick]` after the advance.
  - In `time_markers`, a sample is `[t, occurs, next total ticks, repetition count]`. No rule
    uses the repetition count, and the test ignores it.
- **Postconditions:** the test fails when one value does not agree.

Provenance: `source_suggested_black_box_confirmed` (environment artefact). Kind: semantic.
Confidence: high. Milestone: 1 for the sections of milestone-1 rules; later for `spatial`,
`gaussian` and `weather`.

### ENVA-TEST-002: Sky over one day

- **Inputs:** a world in the overworld with no weather, and the default data pack.
- **Behavior:**
  1. Run `time set 0`, then take a screenshot of the sky each 1000 ticks, for 24 000 ticks, in
     the reference and in the client.
  2. Compare the pairs.
- **Postconditions:** the position of the sun and the moon agrees within 1°. The sky colour
  agrees within 3 of 255 in each channel.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: 1.

---

## 8. Intentional deviations

### ENVA-DEV-001: Colour channels outside `[0, 1]` on write

- **Rule:** ENVA-TYPE-003.
- **Reference:** the reference writes `floor(c · 255)` modulo 256 for each channel. A channel of
  1.5 becomes the byte 126.
- **Visible to the client:** only for a data pack that gives a colour in the list form with a
  channel outside `[0, 1]`. The client then shows the clamped colour.
- **Test:** a differential test does not compare such a colour.

### ENVA-DEV-002: The sign of a zero from a file

- **Rule:** ENVA-MAP-001, item 6.
- **Reference:** the reference keeps `−0` from a JSON file.
- **mcrs:** the client can read `-0.0` in a file of a data pack as `+0`.
- **Visible to the client:** no. Only the modifiers `minimum` and `maximum` compare zeros
  (ENVA-MOD-003), and a vanilla file has no `-0.0`.
- **Test:** a differential test does not use `-0.0` in a file.

---

## 9. Open questions

1. ENVA-LAYER-001: the order of the timelines comes from the order of the tag `timelines` of
   the dimension type. `registry.md` does not yet state the order of the entries of a tag that
   includes another tag.
2. ENVA-LAYER-007: the client reads the camera position for the value of a frame. The camera
   specification owns that position.
3. ENVA-LAYER-005: the rain level and the thunder level, and how they change, belong to a
   weather specification that does not exist yet.
