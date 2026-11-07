# Client entity definition: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| Format reference | Bedrock resource pack, `Mojang/bedrock-samples` `1.26.60.5` |

Source of the game values: `assets/minecraft/version.json`.

This document states how the client reads a client entity file and a render controller file.
It states how the client selects the client entity of an entity, runs its scripts, and selects
the geometry, the materials, the textures and the visible bones for a draw. The file formats
are those of the Bedrock resource pack. The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

The terms of `entity-geometry.md` and `entity-animation.md`, sections 0.1, apply. These terms
are added:

- **Client entity**: one definition from a client entity file. It belongs to one entity type.
- **Short name**: a key of a map of a client entity, for example `default` in `textures`.
- **Render controller**: one entry of a render controller file. A render controller selects one
  geometry, the materials, the textures and the visible bones for one draw.
- **Layer**: one draw of an entity by one render controller.
- **Render type**: a pipeline of the client for an entity layer (CENT-MAT-001).
- **Resource expression**: an expression of a render controller that gives a geometry, a
  material, a texture or an array of one of these (CENT-RC-002).

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, and the label `design` of `entity-geometry.md`,
section 0.2, apply. The kinds of rule are those of `README.md`, section "Semantics, not
accidents". The milestones are those of `entity-geometry.md`, section 0.2.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CENT-FILE` | the client entity file |
| `CENT-SEL` | the selection of a client entity and its variant for an entity |
| `CENT-MAT` | materials and render types |
| `CENT-SCR` | scripts |
| `CENT-RC` | render controllers |
| `CENT-DEV` | intentional deviations |

### 0.4 Scope

In scope: the client entity file, the render controller file, the order of the scripts in a
frame, and the selection of the parts of a draw.

Out of scope: the geometry (`entity-geometry.md`), the clips and the controllers
(`entity-animation.md`), the Molang language (`molang.md`), the queries
(`molang-queries.md`), the hurt colour and the light of a draw (`client-entity-render.md`), and
the values of a specified mob (for example `mob-pig.md`). Attachables, held items, armour,
spawn eggs and particle emitters are out of scope.

---

## 1. The client entity file

### CENT-FILE-001: Shape of the file

- **Inputs:** a file under `entity/` of the entity pack (`entity-geometry.md`, EGEO-PACK-001).
- **Behavior:**
  1. The root object has `format_version` and the object `minecraft:client_entity`.
  2. `minecraft:client_entity` has the object `description` with these fields:

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `identifier` | resource location | required | the entity type (CENT-SEL-001) |
| `min_engine_version` | string | none | CENT-FILE-002 |
| `materials` | object | empty | short name → material name (CENT-MAT-001) |
| `textures` | object | empty | short name → texture path (CENT-FILE-003) |
| `geometry` | object | empty | short name → geometry identifier |
| `animations` | object | empty | short name → clip identifier or controller identifier |
| `animation_controllers` | array | empty | CENT-FILE-004 |
| `scripts` | object | empty | CENT-SCR-001 |
| `render_controllers` | array | empty | CENT-RC-001 |
| `enable_attachables`, `hide_armor`, `spawn_egg`, `particle_effects`, `particle_emitters`, `sound_effects`, `locators`, `held_item_ignores_lighting`, `queryable_geometry` | | | ignored |

  3. The client compares short names without regard to letter case.
  4. A short name that refers to an identifier that does not exist is a content error. The
     client removes the short name from its map.
- **Postconditions:** each map of the client entity contains only references that resolve.

Provenance: `documentation`; `design` for 4. Kind: semantic. Confidence: high. Milestone: 1;
2 for `animations` entries that name a controller.

### CENT-FILE-002: Several definitions of one entity type

- **Inputs:** two or more client entities with the same `identifier`.
- **Behavior:**
  1. In one pack root, the client uses the client entity with the highest
     `min_engine_version`. An absent `min_engine_version` is lower than each version.
  2. When two of them have the same highest version, the rule of `entity-geometry.md`,
     EGEO-PACK-002, applies.
  3. A higher pack root replaces the client entity of a lower pack root.
  4. The client parses each expression with the rules of `molang.md`. A
     `min_engine_version` lower than the version that `molang.md` names does not select other
     parse rules. The client logs a warning for such a file (CENT-DEV-001).
- **Postconditions:** each entity type has at most one client entity.

Provenance: `documentation` for 1; `design` for 2 to 4. Kind: semantic. Confidence: medium.
Milestone: 1.

### CENT-FILE-003: Texture paths

- **Inputs:** a value of the map `textures`.
- **Behavior:**
  1. A value of the form `ns:path` names the image `assets/ns/textures/path.png` of the
     resource pack.
  2. A value of the form `textures/path` names the image `assets/minecraft/textures/path.png`
     of the resource pack.
  3. When the image does not exist, this is a content error. The client uses the missing
     texture of the resource pack for this short name.
- **Postconditions:** each texture short name has an image.

Provenance: `design` for 1 and 3; `documentation` for the form of 2. Kind: semantic.
Confidence: high. Milestone: 1.

### CENT-FILE-004: Legacy controllers

- **Inputs:** the array `animation_controllers` of a client entity. Each element is an object
  `{ short name: controller identifier }`.
- **Behavior:**
  1. The client adds each short name to the map `animations`.
  2. The client adds each short name to the end of the `animate` list (CENT-SCR-003), with the
     weight 1, in the order of the array.
- **Postconditions:** the legacy form and the current form give the same play list.

Provenance: `documentation` for 1; `unresolved` for the order of 2. Kind: semantic.
Confidence: low. Milestone: later.

---

## 2. Selection

### CENT-SEL-001: The client entity of an entity

- **Inputs:** an entity on the client, with its entity type.
- **Behavior:**
  1. The client uses the client entity with the `identifier` of the entity type.
  2. When no client entity has this identifier, the client does not draw the entity. The
     client logs this one time for each entity type.
  3. The client entity of an entity does not change while the entity exists on the client.
- **Postconditions:** none.

Provenance: `design`. Kind: unspecified for 2; semantic for 1 and 3. Confidence: high.
Milestone: 1.

### CENT-SEL-002: Variants

- **Inputs:** an entity with synced entity data, for example the variant of a pig.
- **Behavior:**
  1. A client entity selects a variant only through expressions: the scripts, the weights and
     the render controllers.
  2. The expressions read the synced entity data through queries. `molang-queries.md` names
     each such query. The specification of each mob names the queries that it uses.
  3. A variant that a registry entry defines is given to the expressions as a string, the
     resource location of the entry, for example `'minecraft:warm'`, through
     `query.variant_identifier` (`molang-queries.md`, MOLQ-STATE-001, item 5).
     `query.variant` is a number, as in Bedrock.
- **Postconditions:** a new variant needs no change to the client code. It needs only data.

Provenance: `design`. Kind: semantic. Confidence: medium. Milestone: 1.

---

## 3. Materials

### CENT-MAT-001: Material names

- **Inputs:** a value of the map `materials`.
- **Behavior:** the client maps the material name to a render type:

| Material name | Render type |
|---------------|-------------|
| `entity_alphablend`, `entity_translucent`, a name that ends with `_alphablend` | translucent |
| `entity_emissive`, `entity_emissive_alpha`, a name that starts with `entity_emissive` | emissive |
| each other name | cutout |

  The client logs each name of the last row that is not `entity` or `entity_alphatest`, one
  time for each name.
- **Postconditions:** each material has one render type.

Provenance: `design` (the vanilla Bedrock resource pack contains no material definitions; the
names are engine names). Kind: semantic. Confidence: medium. Milestone: 1 for cutout; later for
the others.

### CENT-MAT-002: Render types

- **Inputs:** a render type.
- **Behavior:**

| Render type | Alpha | Back faces | Light |
|-------------|-------|------------|-------|
| cutout | a texel with an alpha < 0.1 is discarded; other texels are opaque | drawn | `client-entity-render.md` |
| translucent | blended by alpha | not drawn | `client-entity-render.md` |
| emissive | a texel with an alpha < 0.1 is discarded | drawn | full light; no hurt colour |

- **Postconditions:** none.

Provenance: `design`; the constants of the row cutout are stated in `client-entity-render.md`
with their provenance. Kind: semantic. Confidence: medium. Milestone: 1 for cutout; later for
the others.

---

## 4. Scripts

### CENT-SCR-001: Fields of `scripts`

- **Inputs:** the object `scripts` of a client entity.
- **Behavior:** the client compares the keys without regard to letter case. The fields are:

| Field | Type | Meaning |
|-------|------|---------|
| `initialize` | array of strings | CENT-SCR-002 |
| `pre_animation` | array of strings | CENT-SCR-002 |
| `animate` | array | CENT-SCR-003 |
| `scale`, `scalex`, `scaley`, `scalez` | expression | CENT-SCR-004 |
| `should_update_bones_and_effects_offscreen` | expression | CENT-SCR-005 |
| `should_update_effects_offscreen` | expression | ignored (EANIM-FX-001) |
| `variables` | object | later |
| `parent_setup`, `hide_held_items` | expression | ignored |

- **Postconditions:** none.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1.

### CENT-SCR-002: Script arrays are one program

- **Inputs:** an array of strings: `initialize`, `pre_animation`, or `on_entry` or `on_exit`
  of a controller state.
- **Behavior:**
  1. The client joins the strings in their order, with no character between them.
  2. The client parses the result as one complex expression (`molang.md`).
  3. A parse error is a content error. The client ignores the whole array.
- **Postconditions:** a block can start in one string and end in a later string.

Provenance: `documentation` (vanilla Bedrock zombie: a block opens in one entry of
`pre_animation` and closes in a later entry). Kind: semantic. Confidence: high. Milestone: 1.

### CENT-SCR-003: Order of a frame

- **Inputs:** an entity that the client draws, or an entity whose pose a CPU user needs
  (CENT-SCR-005).
- **Behavior:**
  1. When the client creates the entity, it runs `initialize` one time.
  2. In each frame, the client runs `pre_animation`.
  3. The client then makes the play list from `animate` (`entity-animation.md`,
     EANIM-BLEND-002). An element of `animate` is a short name of the map `animations`, or an
     object `{ short name: weight expression }`.
  4. The client then computes the pose (EANIM-BLEND-001).
  5. The client then evaluates the render controllers (CENT-RC-001).
  6. The variables that a step writes are visible to the later steps of the same frame and to
     later frames.
- **Postconditions:** a render controller sees the variables that `pre_animation` set in the
  same frame.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1; 2 for controllers
in `animate`.

### CENT-SCR-004: Scale

- **Inputs:** the scale expressions of a client entity.
- **Behavior:**
  1. The scale on each axis is `scale · scaleX`, `scale · scaleY` and `scale · scaleZ`. An
     absent expression is 1.
  2. The client evaluates the scale expressions in each frame, after `pre_animation`.
  3. The scale applies to the model about the entity origin. It does not change the collision
     box.
  4. `client-entity-render.md`, CEREN-PLACE-002, states the step of the placement transform
     that applies the scale.
- **Postconditions:** none.

Provenance: `documentation`; `design` for the product in 1. Kind: semantic. Confidence:
medium. Milestone: 1.

### CENT-SCR-005: Entities that are not visible

- **Inputs:** an entity that is outside the view, and the expression
  `should_update_bones_and_effects_offscreen`.
- **Behavior:**
  1. When the expression is 0 or absent, the client can skip the pose of the entity while it is
     outside the view.
  2. When a CPU user needs a locator or a bone transform of the entity, the client computes the
     pose for that frame.
  3. When the expression is not 0, the client computes the pose in each frame.
  4. The variables and the controller states of an entity must not depend on the view.
     CENT-SCR-006 states when the client can skip `pre_animation` and the controllers.
- **Postconditions:** an entity that comes into the view has the pose of the current frame.

Provenance: `documentation` for 1 and 3; `design` for 2 and 4. Kind: semantic. Confidence:
medium. Milestone: 1.

### CENT-SCR-006: Evaluation only when the inputs change

- **Inputs:** a script, a controller or a render controller of an entity, and the frames in
  which the rules of this document evaluate it.
- **Behavior:**
  1. The client can skip an evaluation in a frame when the result is the same as the result of
     the evaluation in that frame. The result includes the variables that the evaluation writes,
     the controller state, and the selected resources.
  2. The result is the same, for example, when no input of the evaluation changed since the
     last evaluation. An input is a query, a variable, a context value or the frame time that
     the evaluation reads. `molang-queries.md`, MOLQ-HOST-004, states how often a query can
     change.
  3. An evaluation that reads a variable that it also writes, or a controller during a
     transition or a cross-fade, depends on the number of frames. The client evaluates it in
     each frame, also for an entity outside the view.
  4. For an entity outside the view, the client can also skip an evaluation that depends only
     on the inputs of the current frame. Before the next draw of the entity, or before a CPU user
     reads its pose, the client evaluates it with the inputs of that frame.
  5. CENT-SCR-005 states the pose of an entity outside the view.
- **Postconditions:** a skipped evaluation is not visible. The pose, the variables and the
  resources are the same as with an evaluation in each frame.

Provenance: `design`. Kind: semantic for the result; unspecified for the frames of the
evaluation. Confidence: medium. Milestone: 1.

---

## 5. Render controllers

### CENT-RC-001: The render controller list

- **Inputs:** the array `render_controllers` of a client entity, and the render controller
  files.
- **Behavior:**
  1. The root object of a render controller file has `format_version` and the object
     `render_controllers`. Each key is a render controller identifier.
  2. An element of the array of the client entity is a render controller identifier, or an
     object `{ identifier: condition expression }`.
  3. In each frame, the client draws one layer for each element in the order of the array.
     The client skips an element when its condition is 0.
  4. A render controller has these fields:

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `arrays` | object | empty | CENT-RC-002 |
| `geometry` | resource expression | required | the geometry of the layer |
| `materials` | array | `[{"*": "Material.default"}]` | CENT-RC-004 |
| `textures` | array of resource expressions | `["Texture.default"]` | CENT-RC-003 |
| `part_visibility` | array | empty | CENT-RC-005 |
| `color`, `overlay_color`, `is_hurt_color`, `on_fire_color` | object | none | CENT-RC-006 |
| `uv_anim`, `ignore_lighting`, `filter_lighting`, `light_color_multiplier`, `rebuild_animation_matrices` | | | later; ignored |

  5. All geometries of one entity share one pose (EANIM-BLEND-001). The pose gives values by
     bone name. A bone of a geometry takes the values of the pose bone with the same name.
  6. A bone of a geometry that the pose does not name takes its rest pose.
- **Postconditions:** each layer uses the pose of the frame. For example, a saddle geometry with
  the bone names of the body geometry moves with the body.

Provenance: `documentation`; `design` for 5 and 6. Kind: semantic. Confidence: high.
Milestone: 1.

### CENT-RC-002: Resource expressions and arrays

- **Inputs:** a resource expression.
- **Behavior:**
  1. `Geometry.x`, `Material.x` and `Texture.x` name the short name `x` of the map of that kind
     in the client entity. The client compares these names without regard to letter case.
  2. `Array.x[e]` selects an element of the array `x`. The index is
     `max(0, trunc(e)) mod n`, with `n` the number of elements. When `e` is NaN, `+∞` or
     `−∞`, the index is 0.
  3. `c ? A : B` selects `A` when `c` is not 0, and `B` otherwise.
  4. The object `arrays` has the objects `geometries`, `materials` and `textures`. Each maps an
     array name of the form `Array.x` to a list of resource expressions of that kind.
  5. An element of an array that is an array adds the elements of that array in its place.
  6. The client compiles a render controller one time for each pair of a client entity and a
     render controller that it lists. The short names resolve against the maps of that client
     entity.
  7. An array with no elements, or a resource expression that names a short name that does not
     exist in the client entity, is a load error of that pair. The client logs it one time for
     each pair.
  8. The layer of a pair with a load error draws nothing. The other layers of the client entity
     and the other client entities that use the render controller do not change.
- **Postconditions:** each resource expression gives one resource in each frame. A texture can also come from a string expression (CENT-RC-007).

Provenance: `documentation`; `design` for the value that is not finite and for 6 to 8. Kind:
semantic. Confidence: high. Milestone: 1.

### CENT-RC-003: Textures of a layer

- **Inputs:** the array `textures` of a render controller.
- **Behavior:**
  1. Element `i` gives the texture of texture slot `i` of the layer.
  2. The render types of CENT-MAT-002 use only slot 0. The client logs the other slots one time
     for each render controller.
- **Postconditions:** none.

Provenance: `documentation` for 1; `design` for 2. Kind: semantic. Confidence: high.
Milestone: 1.

### CENT-RC-007: Textures from registry data

- **Inputs:** an element of the array `textures` that is a string expression, for example
  `q.variant_asset` or `q.equipment_texture('saddle', 'pig_saddle')`
  (`molang-queries.md`, MOLQ-STATE-002).
- **Behavior:**
  1. The static type of an element of `textures` decides its kind (`molang.md`, MOL-VAL-001):
     - an element with the type resource is a resource expression (CENT-RC-002);
     - an element with the type string is a string expression (this rule);
     - an element that can give a resource in one branch and a string in another branch is the
       load error `static_type` (`molang.md`, MOL-LOAD-004) of the pair of client entity and
       render controller.
  2. The value of a string expression is a texture identifier `ns:path`. It names the image
     `assets/ns/textures/path.png` of the resource pack. For example,
     `minecraft:entity/pig/pig_cold` names `assets/minecraft/textures/entity/pig/pig_cold.png`.
  3. The value `''` selects no texture. The client does not draw the layer for that entity.
  4. When the registries are synced or reloaded, the client finds each texture identifier that
     a string expression can give:
     - the asset identifier and the baby asset identifier of each entry of each variant
       registry;
     - each texture identifier that `equipment_texture` can give for each equipment asset file
       and each layer type.
  5. The client loads each of these images before the first frame that uses it.
  6. A value with no image, or an image that the client did not load, selects the missing
     texture of the resource pack. The client logs it one time for each identifier.
- **Postconditions:** a variant entry or an equippable item that a data pack adds gets its
  texture with no change to the client entity files and no change to the client code.

Provenance: `design`. Kind: semantic. Confidence: medium. Milestone: 1.

### CENT-RC-004: Materials of a layer

- **Inputs:** the array `materials` of a render controller. Each element is an object
  `{ bone pattern: resource expression }`.
- **Behavior:**
  1. A bone pattern matches a bone name. `*` in the pattern matches each sequence of
     characters, also an empty one. The client compares without regard to letter case.
  2. The client applies the elements in their order. A later element replaces the material of
     a bone that an earlier element set.
  3. A bone that no pattern matches has no material. The client does not draw its cubes in
     this layer.
- **Postconditions:** each bone has at most one material in a layer.

Provenance: `documentation` for 1 and 2; `design` for the letter case (CENT-DEV-002) and for 3.
Kind: semantic. Confidence: medium. Milestone: 1.

### CENT-RC-005: Visible bones

- **Inputs:** the array `part_visibility` of a render controller. Each element is an object
  `{ bone pattern: expression or bool }`.
- **Behavior:**
  1. Each bone is visible at the start of the frame.
  2. The client applies the elements in their order. A bone that a pattern matches is visible
     when the value is not 0, and hidden when the value is 0.
  3. The patterns are those of CENT-RC-004.
  4. A hidden bone is `entity-geometry.md`, EGEO-POSE-002.
- **Postconditions:** none.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1.

### CENT-RC-006: Colours of a layer

- **Inputs:** the objects `color`, `overlay_color`, `is_hurt_color` and `on_fire_color` of a
  render controller. Each has the expressions `r`, `g`, `b` and `a`. An absent component is
  1 for `color` and 0 for the others.
- **Behavior:**
  1. `color` multiplies the colour of each texel of the layer.
  2. `overlay_color` mixes its colour over the texel with its alpha as the factor.
  3. `is_hurt_color` replaces the hurt colour of `client-entity-render.md` for this layer. All
     components at 0 remove the hurt colour.
  4. `on_fire_color` replaces the fire colour of `client-entity-render.md` for this layer.
  5. Without these objects, the colours of `client-entity-render.md` apply.
  6. In the expression of one component, `this` (`molang.md`, MOL-VAR-007) is the value of
     that component when the object is absent. For `color`, it is `1.0`. For `overlay_color`,
     `is_hurt_color` and `on_fire_color`, it is the component of the colour that
     `client-entity-render.md` gives the layer in the current frame.
- **Postconditions:** an expression `this` gives the colour that the client uses with no
  object. `v.flash ? 1 : this` replaces the colour only while `v.flash` is truthy.

Provenance: `documentation`; item 6: `documentation` (the keyword `this`: "the current value
that this expression will ultimately write to"); bedrock-samples uses `this` in these
channels. Kind: semantic. Confidence: medium. Milestone: later. In
milestone 1, the colours of `client-entity-render.md` apply to all layers.

---

## 6. Intentional deviations

### CENT-DEV-001: One parse rule for all versions

- **Rule:** CENT-FILE-002.
- **Reference:** the Bedrock client selects older parse rules of Molang for a file with an older
  `min_engine_version`.
- **Visible to the client:** yes, for an old pack that depends on the old precedence of `&&`
  and `||` or of the conditional operator.
- **Test:** a differential test uses only files with a `min_engine_version` of the version that
  `molang.md` names or higher.

### CENT-DEV-002: Bone patterns ignore letter case

- **Rule:** CENT-RC-004.
- **Reference:** the format reference states that the match of a bone pattern is sensitive to
  letter case.
- **Visible to the client:** yes, only for a geometry with two bone names that differ only in
  letter case. `entity-geometry.md`, EGEO-BONE-002, rejects such a geometry.
- **Test:** none. No accepted geometry shows the difference.
