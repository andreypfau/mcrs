# Entity geometry: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| Format reference | Bedrock resource pack, `Mojang/bedrock-samples` `1.26.60.5` |

Source of the game values: `assets/minecraft/version.json`.

This document states how the client reads an entity geometry file, and what model the client
makes from it. The file format is the geometry format of the Bedrock resource pack. The
document also states the layout of the entity pack, which `entity-animation.md` and
`client-entity-definition.md` use. The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Entity pack**: the directory tree `assets/mcrs/entity/` of a pack root, with the
  subdirectories of EGEO-PACK-001.
- **Content error**: a defect in a file of the entity pack. The client logs the content error
  with the path of the file. The client continues to load the other files.
- **Model unit**: 1/16 of a block.
- **File space**: the coordinate space of the numbers in a geometry file or an animation file.
  File space is mirrored on the X axis (EGEO-SPACE-001).
- **Internal space**: a right-handed space with +Y up. The entity front points to −Z. The right
  side of the entity points to +X. The unit is the model unit. The origin is the entity
  position, at the feet.
- **Geometry**: one entry of a geometry file, with an identifier, a texture size, bones and
  optional visible bounds.
- **Bone**: a named node of a geometry. A bone has a pivot, a rest rotation, cubes, locators and
  an optional parent bone.
- **Cube**: a box that belongs to a bone.
- **Rest pose**: the pose of a geometry when no animation applies.
- **Baked geometry**: the result of EGEO-BAKE-001.
- **Euler rotation**: three angles in degrees, `(rx, ry, rz)`. The rotation matrix is
  `Rz(rz) · Ry(ry) · Rx(rx)`. Each angle turns by the right-hand rule about its axis. The
  rotation about X applies first to a vertex.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply, including the label `design`. The kinds of
rule are those of `README.md`, section "Semantics, not accidents". A parity test does not
compare a rule with the label `design` with an external artefact.

Each rule also has a line **Milestone**:

- `1`: the first mob (pig). The geometry, animation clips without controllers, and a render
  controller that selects a texture by variant.
- `2`: the second mob (frog). Animation controllers and the rules that it needs.
- `later`: no milestone uses the rule yet. An implementation of milestone 1 or 2 can
  reject the input with a content error.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `EGEO-PACK` | the layout of the entity pack, JSON, identifiers |
| `EGEO-FMT` | the format versions of a geometry file |
| `EGEO-SPACE` | file space and internal space |
| `EGEO-BONE` | bones |
| `EGEO-CUBE` | cubes |
| `EGEO-UV` | texture coordinates |
| `EGEO-LOC` | locators |
| `EGEO-LIM` | limits |
| `EGEO-BAKE` | the baked geometry |
| `EGEO-POSE` | the transform of a bone from a pose |
| `EGEO-DEV` | intentional deviations |

### 0.4 Scope

In scope: the entity pack layout, the JSON rules of all entity pack files, the geometry file,
the conversion to internal space, the baked geometry and the bone transform.

Out of scope: animations (`entity-animation.md`), the client entity file and render controllers
(`client-entity-definition.md`), the Molang language (`molang.md`), and the placement of the
model in the world (`client-entity-render.md`).

---

## 1. The entity pack

### EGEO-PACK-001: Layout of the entity pack

- **Inputs:** the pack roots of the client (`assets/mcrs/` of each layer).
- **Behavior:**
  1. The client reads the files under `assets/mcrs/entity/` of each pack root.
  2. The subdirectory selects the kind of each file:

| Subdirectory | Kind | Specification |
|--------------|------|---------------|
| `geometry/` | geometry file | this document |
| `animations/` | animation file | `entity-animation.md` |
| `animation_controllers/` | animation controller file | `entity-animation.md` |
| `render_controllers/` | render controller file | `client-entity-definition.md` |
| `entity/` | client entity file | `client-entity-definition.md` |

  3. The client reads the subdirectories recursively. The client reads only files with the
     extension `.json`.
  4. The path of a file has no meaning other than its kind. The identifiers come from the
     content of the file.
- **Postconditions:** a file outside these subdirectories is not read.

Provenance: `design`. Kind: semantic. Confidence: high. Milestone: 1.

### EGEO-PACK-002: Identifiers

- **Inputs:** an identifier in a file of the entity pack.
- **Behavior:**
  1. A geometry identifier, an animation identifier, an animation controller identifier and a
     render controller identifier are strings in the Bedrock form, for example
     `geometry.pig.v3`.
  2. A client entity identifier is a resource location, for example `minecraft:pig`.
  3. The client compares identifiers without regard to letter case.
  4. Each kind has its own set of identifiers. A geometry and an animation can have the same
     identifier.
  5. When two pack roots define the same identifier of one kind, the higher layer replaces
     the definition of the lower layer.
  6. When one pack root defines the same identifier of one kind two times, this is a content
     error. The definition in the file with the last path in byte order is used.
- **Postconditions:** each identifier of each kind has at most one definition.

Provenance: `documentation` for 1 and 2; `design` for 3 to 6. Kind: semantic. Confidence:
medium. Milestone: 1.

### EGEO-PACK-003: Lenient JSON

- **Inputs:** a file of the entity pack.
- **Behavior:**
  1. The client accepts `//` line comments and `/* */` block comments outside of strings.
  2. The client accepts a comma after the last element of an array or an object.
  3. When an object has the same key two times, the last value is used.
  4. The client keeps the order of the elements of each array.
  5. Other syntax errors are content errors. The client rejects the file.
- **Postconditions:** the vanilla Bedrock resource pack parses with no content error from
  JSON syntax.

Provenance: `documentation` (vanilla Bedrock resource pack: some files contain comments,
commas after the last element, and a duplicate key). Kind: semantic. Confidence: high. Milestone: 1.

---

## 2. Format versions

### EGEO-FMT-001: Format version of a geometry file

- **Inputs:** the field `format_version` of a geometry file.
- **Behavior:**
  1. `format_version` is a string of numbers with dots, for example `1.12.0`. The client
     compares it number by number.
  2. When `format_version` is `1.8.0` or `1.10.0`, the file has the legacy shape
     (EGEO-FMT-002).
  3. When `format_version` is `1.12.0` or higher, the file has the current shape
     (EGEO-FMT-003).
  4. When `format_version` is higher than the highest version that this document knows, the
     client reads the file with the current shape and logs a warning.
  5. Another value is a content error. The client rejects the file.

The highest version that this document knows is `1.26.10`.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1 for the current
shape; later for the legacy shape.

### EGEO-FMT-002: The legacy shape

- **Inputs:** a geometry file with the legacy shape.
- **Behavior:**
  1. The root object has keys that start with `geometry.`. Each such key defines one geometry.
  2. A key of the form `geometry.a:geometry.b` defines the geometry `geometry.a` with the
     parent geometry `geometry.b`.
  3. The fields of a geometry are `texturewidth`, `textureheight`, `visible_bounds_width`,
     `visible_bounds_height`, `visible_bounds_offset` and `bones`.
  4. A cube has only `origin`, `size`, `uv` as an array, `inflate` and `mirror`.
  5. A bone can have the fields `bind_pose_rotation`, `neverRender` and `reset`.
     `neverRender: true` hides the bone in all render controllers. The client ignores `reset`.
     The client ignores `bind_pose_rotation` and logs a warning (EGEO-DEV-001).
  6. A locator is an array of three numbers.
- **Postconditions:** the geometry has the same model as the equal current shape.

Provenance: `documentation`; `unresolved` for `bind_pose_rotation`. Kind: semantic.
Confidence: medium. Milestone: later.

### EGEO-FMT-003: The current shape

- **Inputs:** a geometry file with the current shape.
- **Behavior:**
  1. The root object has the key `minecraft:geometry`, an array of geometries.
  2. Each geometry has an object `description` with these fields:

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `identifier` | string | required | the geometry identifier |
| `texture_width` | number > 0 | EGEO-UV-004 | the width of the texture in texels; a value of 0 or less is not set |
| `texture_height` | number > 0 | EGEO-UV-004 | the height of the texture in texels; a value of 0 or less is not set |
| `visible_bounds_width` | number ≥ 0 | EGEO-BAKE-002 | the width of the visible bounds in blocks |
| `visible_bounds_height` | number ≥ 0 | EGEO-BAKE-002 | the height of the visible bounds in blocks |
| `visible_bounds_offset` | 3 numbers | `[0, 0, 0]` | the centre of the visible bounds in blocks, in file space |

  3. Each geometry has an array `bones` (EGEO-BONE-001).
  4. The client ignores a field that this document does not name, and logs it one time for
     each file.
- **Postconditions:** each geometry of the array is a separate definition.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1.

### EGEO-FMT-004: Inheritance in the legacy shape

- **Inputs:** a geometry `a` with the parent geometry `b` (EGEO-FMT-002).
- **Behavior:**
  1. The client resolves `b` after it loads all geometry files of all pack roots.
  2. The bones of `a` start as a copy of the resolved bones of `b`, in the order of `b`.
  3. A bone of `a` with the name of a bone of `b` replaces that bone at the same position.
  4. The client adds the other bones of `a` after these, in the order of `a`.
  5. A field of the description of `a` replaces the same field of `b`. A field that `a` does
     not set comes from `b`.
  6. When `b` does not exist, or the chain of parents has a cycle, this is a content error.
     The client rejects `a`.
- **Postconditions:** a resolved geometry has no parent.

Provenance: `documentation` for 1 and 2; `design` for 3 to 6. Kind: semantic. Confidence:
medium. Milestone: later.

---

## 3. Space

### EGEO-SPACE-001: File space

- **Inputs:** a position, a pivot, an origin or a rotation in a geometry file or an animation
  file.
- **Behavior:**
  1. In file space, +X points to the left side of the entity. +Y points up. The entity front
     points to −Z. The unit is the model unit.
  2. A rotation in file space is an Euler rotation in degrees. The file states it in the order
     x, y, z.
  3. Pivots, cube origins and locator positions are absolute points of the model. They are not
     relative to the parent bone.
- **Postconditions:** none.

Provenance: `documentation` (the right arm bone of the vanilla player geometry has a negative
x; the vanilla pig head has a negative z). Kind: semantic. Confidence: high. Milestone: 1.

### EGEO-SPACE-002: Conversion to internal space

- **Inputs:** a value in file space.
- **Behavior:** the client converts each value to internal space at load:

| File value | Internal value |
|------------|----------------|
| point `(x, y, z)`: a pivot, a locator position, `visible_bounds_offset` | `(−x, y, z)` |
| cube box: `origin (ox, oy, oz)`, `size (sx, sy, sz)` | low corner `(−(ox + sx), oy, oz)`, high corner `(−ox, oy + sy, oz + sz)` |
| rotation `(rx, ry, rz)` of a bone, a cube or a locator | `(−rx, −ry, rz)` |
| animation position `(x, y, z)` | `(−x, y, z)` |
| animation rotation `(rx, ry, rz)` | `(−rx, −ry, rz)` |
| animation scale `(x, y, z)` | `(x, y, z)` |

- **Postconditions:**
  - a positive pitch value of the file turns the front of a bone down;
  - a positive yaw value of the file turns the front of a bone to the right side of the entity.

Provenance: `documentation` (the import conversion of Blockbench; the vanilla pig geometry).
Kind: semantic. Confidence: high. Milestone: 1.

---

## 4. Bones

### EGEO-BONE-001: Fields of a bone

- **Inputs:** an element of the array `bones`.
- **Behavior:** the fields are:

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `name` | string | required | the bone name |
| `parent` | string | none | the name of the parent bone |
| `pivot` | 3 numbers | `[0, 0, 0]` | the centre of rotation and scale of the bone |
| `rotation` | 3 numbers | `[0, 0, 0]` | the rest rotation, in degrees |
| `mirror` | bool | `false` | the default `mirror` of the cubes of the bone |
| `inflate` | number | `0` | the default `inflate` of the cubes of the bone |
| `cubes` | array | empty | the cubes (EGEO-CUBE-001) |
| `locators` | object | empty | the locators (EGEO-LOC-001) |
| `binding` | string | none | Molang (EGEO-BONE-003); later |
| `render_group_id` | integer | none | ignored |
| `texture_meshes`, `poly_mesh` | | none | later; ignored now, with one warning for each geometry |

- **Postconditions:** none.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1, except where the
table says later.

### EGEO-BONE-002: Bone names and parents

- **Inputs:** the bones of one geometry.
- **Behavior:**
  1. The client compares bone names without regard to letter case.
  2. Two bones with the same name in one geometry are a content error. The client rejects the
     geometry.
  3. A bone can come before its parent bone in the array.
  4. A bone without `parent` attaches to the model origin.
  5. A `parent` that names no bone of the geometry is a content error. The client attaches the
     bone to the model origin.
  6. A cycle of parents is a content error. The client rejects the geometry.
- **Postconditions:** the bones form a forest.

Provenance: `documentation` for 3 and 4; `design` for the others. Kind: semantic. Confidence:
medium. Milestone: 1.

### EGEO-BONE-003: Bone binding

- **Inputs:** the field `binding` of a bone.
- **Behavior:**
  1. The value is a Molang expression (`molang.md`) that gives a string: the name of a bone of
     another model that the bone attaches to. The context value `context.item_slot` is the
     slot of the item that the model draws.
  2. The client compiles the expression at load. A load error of the expression is a content
     error. The client logs it, and the geometry loads.
  3. Until the milestone of this rule, the client does not run the expression. The bone keeps
     the parent of EGEO-BONE-002.
- **Postconditions:** a geometry with `binding` loads in each milestone.

Provenance: `documentation` (geometry format; bedrock-samples uses the field in attachable
geometry with `query.item_slot_to_bone_name`). Kind: semantic. Confidence: low. Milestone:
item 2: 1; items 1 and 3: later.

---

## 5. Cubes

### EGEO-CUBE-001: Fields of a cube

- **Inputs:** an element of the array `cubes` of a bone.
- **Behavior:** the fields are:

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `origin` | 3 numbers | required | the low corner of the box before rotation, absolute |
| `size` | 3 numbers ≥ 0 | required | the extent of the box from `origin` |
| `rotation` | 3 numbers | `[0, 0, 0]` | the rotation of the cube in degrees, about the cube pivot |
| `pivot` | 3 numbers | the centre of the box | the centre of the cube rotation |
| `inflate` | number | the `inflate` of the bone | the growth of the box on each side |
| `mirror` | bool | the `mirror` of the bone | the mirror of the box UV (EGEO-UV-002) |
| `uv` | 2 numbers, or an object | `[0, 0]` | the box UV (EGEO-UV-001) or the per-face UV (EGEO-UV-003) |

  A negative value in `size` is a content error. The client uses 0 for that axis.
- **Postconditions:** none.

Provenance: `documentation`; `design` for the negative size. Kind: semantic. Confidence: high.
Milestone: 1.

### EGEO-CUBE-002: The box of a cube

- **Inputs:** a cube with the internal low corner `lo` and the internal high corner `hi`
  (EGEO-SPACE-002), and the inflate `g`.
- **Behavior:**
  1. The drawn box has the corners `lo − (g, g, g)` and `hi + (g, g, g)`.
  2. A negative `g` makes the box smaller. When a low value becomes higher than the high
     value, the box is flat on that axis. The flat box is at the centre of the box before
     inflate on that axis.
  3. The cube rotation turns each corner `v` to `T(c) · R · T(−c) · v`. `R` is the Euler
     rotation of the converted cube rotation. `c` is the converted cube pivot.
  4. The cube rotation is relative to the bone. The bone transform (EGEO-POSE-001) applies
     after it.
  5. The texture coordinates use the size before inflate (EGEO-UV-001).
- **Postconditions:** a cube with a size of 0 on one axis has six faces. Two faces cover the
  same plane, and four faces have no area.

Provenance: `documentation`; `design` for the position of the flat box in item 2. Reason: an
inverted box would turn its faces inside out, and the face rules of
`client-entity-render.md`, CEREN-SURF-001, expect faces that point out of the box. Kind:
semantic. Confidence: high. Milestone: 1.

### EGEO-CUBE-003: The cube pivot in format 1.12.0

- **Inputs:** a cube with `pivot` in a file with `format_version` `1.12.0`.
- **Behavior:** the client uses the pivot as in format `1.14.0` and logs a warning.
- **Postconditions:** none.

Provenance: `unresolved`. The format reference states that the cube pivot of `1.12.0` is
"flipped upside-down" and that `1.14.0` corrects it. It does not state the transform. Kind:
semantic. Confidence: low. Milestone: later.

---

## 6. Texture coordinates

### EGEO-UV-001: Box UV

- **Inputs:** a cube with `uv` as an array `[U, V]`, and the size `(sx, sy, sz)` before inflate.
- **Behavior:**
  1. The layout uses `w = floor(sx)`, `h = floor(sy)` and `d = floor(sz)`.
  2. Each face gets a rectangle `[u1, v1, u2, v2]` in texels. The texture `v` increases down.
     `u1 > u2` or `v1 > v2` flips the image on that axis.

| Face (internal) | `[u1, v1, u2, v2]` |
|-----------------|--------------------|
| east (+X) | `[U, V+d, U+d, V+d+h]` |
| north (−Z) | `[U+d, V+d, U+d+w, V+d+h]` |
| west (−X) | `[U+d+w, V+d, U+2d+w, V+d+h]` |
| south (+Z) | `[U+2d+w, V+d, U+2d+2w, V+d+h]` |
| up (+Y) | `[U+d+w, V+d, U+d, V]` |
| down (−Y) | `[U+d+2w, V, U+d+w, V+d]` |

- **Postconditions:** a size below 1 on an axis gives a strip of 0 texels on that axis. A file
  that needs a strip of the exact fractional size uses the per-face form (EGEO-UV-003).
  `block-entity-models.md`, BEM-ART-001, item 6, uses this form.

Provenance: `documentation` (the box UV of Blockbench; the vanilla Bedrock resource pack).
Kind: semantic. Confidence: high. Milestone: 1.

### EGEO-UV-002: Mirror

- **Inputs:** a cube with box UV and `mirror` true.
- **Behavior:**
  1. For each face, swap `u1` and `u2`.
  2. Then swap the rectangles of the east face and the west face.
  3. `mirror` does not change the positions of the vertices.
  4. `mirror` has no effect on a per-face UV.
- **Postconditions:** none.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1.

### EGEO-UV-003: Per-face UV

- **Inputs:** a cube with `uv` as an object.
- **Behavior:**
  1. The keys are the face names `north`, `south`, `east`, `west`, `up` and `down`. They name
     the faces of internal space.
  2. Each value is an object with `uv` (2 numbers, required), `uv_size` (2 numbers), and
     optional `uv_rotation` and `material_instance`.
  3. The default `uv_size` is the size of that face of the box before inflate.
  4. For `north`, `south`, `east` and `west`, the rectangle is
     `[uv.x, uv.y, uv.x + uv_size.x, uv.y + uv_size.y]`.
  5. For `up` and `down`, the rectangle is `[uv.x + uv_size.x, uv.y + uv_size.y, uv.x, uv.y]`.
  6. `uv_rotation` is 0, 90, 180 or 270. It turns the rectangle clockwise in steps of 90°.
     Each step moves the texture coordinate of corner `c2` to corner `c0`, of `c3` to `c2`, of
     `c1` to `c3`, and of `c0` to `c1`. The corners are the columns of EGEO-UV-005.
  7. A face that is not in the object is not drawn.
  8. The client ignores `material_instance` and logs it one time for each file.
- **Postconditions:** none.

Provenance: `documentation`. Kind: semantic. Confidence: medium. Milestone: 1 for 1 to 5 and 7;
later for 6 and 8.

### EGEO-UV-004: Normalisation

- **Inputs:** a rectangle in texels, and the texture size `(TW, TH)` of the geometry.
- **Behavior:**
  1. The normalised coordinates are `(u / TW, v / TH)`.
  2. When the geometry does not set the texture size, the texture size is `(16, 16)`. The
     client logs a warning one time for each geometry. A width or a height of 0 or less counts
     as not set. Thus such a geometry also uses `(16, 16)` and gives the warning.
  3. The client never uses the size of an image for the texture size.
- **Postconditions:** the baked geometry does not depend on the image that an entity uses. An
  image with a larger size than the texture size maps to the same rectangles.

Provenance: `documentation` for 1; `design` for 2 and 3. Kind: semantic. Confidence: high.
Milestone: 1.

### EGEO-UV-005: Corners of a face

- **Inputs:** a face of a box with the internal low corner `lo` and high corner `hi`, and its
  rectangle `[u1, v1, u2, v2]`.
- **Behavior:** each corner of the face gets the texture coordinate of its column:

| Face | `c0 = (u1, v1)` | `c1 = (u2, v1)` | `c2 = (u1, v2)` | `c3 = (u2, v2)` |
|------|-----------------|-----------------|-----------------|-----------------|
| east (+X) | `(hi.x, hi.y, hi.z)` | `(hi.x, hi.y, lo.z)` | `(hi.x, lo.y, hi.z)` | `(hi.x, lo.y, lo.z)` |
| west (−X) | `(lo.x, hi.y, lo.z)` | `(lo.x, hi.y, hi.z)` | `(lo.x, lo.y, lo.z)` | `(lo.x, lo.y, hi.z)` |
| up (+Y) | `(lo.x, hi.y, lo.z)` | `(hi.x, hi.y, lo.z)` | `(lo.x, hi.y, hi.z)` | `(hi.x, hi.y, hi.z)` |
| down (−Y) | `(lo.x, lo.y, hi.z)` | `(hi.x, lo.y, hi.z)` | `(lo.x, lo.y, lo.z)` | `(hi.x, lo.y, lo.z)` |
| south (+Z) | `(lo.x, hi.y, hi.z)` | `(hi.x, hi.y, hi.z)` | `(lo.x, lo.y, hi.z)` | `(hi.x, lo.y, hi.z)` |
| north (−Z) | `(hi.x, hi.y, lo.z)` | `(lo.x, hi.y, lo.z)` | `(hi.x, lo.y, lo.z)` | `(lo.x, lo.y, lo.z)` |

  The face normal points out of the box along the axis of the face name.
- **Postconditions:** a viewer in front of the entity sees the right side of the entity on the
  left side of the north face rectangle.

Provenance: `documentation` (the vertex order of Blockbench). Kind: semantic. Confidence: high.
Milestone: 1.

---

## 7. Locators

### EGEO-LOC-001: Locators

- **Inputs:** the object `locators` of a bone.
- **Behavior:**
  1. Each key is a locator name. The client compares locator names without regard to letter
     case.
  2. A value is an array of three numbers, the position. Or a value is an object with
     `offset` (the position), `rotation` (3 numbers, default `[0, 0, 0]`) and
     `ignore_inherited_scale` (bool, default `false`).
  3. The position is an absolute point of the model. The client converts it as a point, and
     the rotation as a rotation (EGEO-SPACE-002).
  4. The locator moves with its bone. The transform of the locator is
     `B · T(p) · R(r)`, with `B` the transform of the bone (EGEO-POSE-001), `p` the position and
     `R(r)` the Euler rotation.
  5. When `ignore_inherited_scale` is true, the client removes the scale from `B` for this
     locator.
  6. Two locators with the same name, without regard to letter case, are a content warning.
     The client keeps the first locator in the order of the bone table (EGEO-BAKE-001, item 1),
     and the order of the keys in one bone. A lookup by name gives that locator.
- **Postconditions:** none.

Provenance: `documentation`; `design` for 4, 5 and 6. Reason for 6: the documentation does
not state the case, and the first locator gives a result that does not depend on later bones.
Kind: semantic. Confidence: medium.
Milestone: later.

---

## 8. Limits

### EGEO-LIM-001: Limits of a geometry

- **Inputs:** a resolved geometry.
- **Behavior:** the client rejects the geometry with a content error when one of these is
  true:
  - it has more than 128 bones;
  - a chain from a root bone to a bone has more than 16 bones;
  - it has more than 2048 cubes;
  - it has no bone.
- **Postconditions:** each accepted geometry fits these limits.

Provenance: `design` (the GPU pose pass stores a bone index in one byte with spare values, and
evaluates the chain of each bone in a bounded loop). Kind: semantic. Confidence: medium.
Milestone: 1.

---

## 9. The baked geometry

### EGEO-BAKE-001: Content of the baked geometry

- **Inputs:** an accepted geometry.
- **Behavior:** the baked geometry has:
  1. A bone table. A parent bone comes before each of its child bones. Bones that have no order
     between them keep the order of the file.
  2. For each bone: the name, the index of the parent bone or none, the internal pivot, the
     internal rest rotation, and the rest rotation of the file. `entity-animation.md` uses the
     rest rotation of the file for `this`.
  3. For each cube: the index of its bone, the six faces of the box (EGEO-CUBE-002), and for
     each face the four corners in internal space, the normal, and the normalised texture
     coordinates (EGEO-UV-004). A face that is not drawn (EGEO-UV-003) is not in the table.
  4. The corners are in the rest pose of the cube. The bone transform does not apply to them.
  5. The locators (EGEO-LOC-001).
  6. The visible bounds (EGEO-BAKE-002).
- **Postconditions:** the baked geometry contains no data that changes per entity.

Provenance: `design`. Kind: semantic. Confidence: high. Milestone: 1.

### EGEO-BAKE-002: Visible bounds

- **Inputs:** a geometry.
- **Behavior:**
  1. The visible bounds is a box in blocks. Its centre is the converted `visible_bounds_offset`.
     Its size on X and on Z is `visible_bounds_width`. Its size on Y is
     `visible_bounds_height`.
  2. When the geometry does not set `visible_bounds_width` or `visible_bounds_height`, the
     client computes the box that contains all corners of all cubes in the rest pose. The rest
     pose applies the rest transform of each bone (EGEO-POSE-001 with no animation) to the
     corners of its cubes. The client then multiplies the size of that box by 1.5 about its
     centre.
  3. The visible bounds turns with the entity and scales with the scale script
     (`client-entity-definition.md`, CENT-SCR-004). Animations do not change it.
  4. The culling volume of an entity is the union of the visible bounds of each geometry that
     the entity draws and the entity box (`client-entity-render.md`, CEREN-VIS-002).
  5. Each box goes through the full placement transform of the entity before the test. The
     placement transform includes the death tilt and the turn of an upside-down entity
     (`client-entity-render.md`, CEREN-PLACE-002).
  6. The client must not cull an entity when its culling volume intersects the view.
- **Postconditions:** an animation that moves a cube out of the visible bounds can make that
  cube disappear at the edge of the view. This is the behaviour of the format.

Provenance: `documentation` for 1, 3 and 6 (the unit of the visible bounds is the block);
`design` for 2, 4 and 5. Kind: semantic. Confidence: medium. Milestone: 1.

---

## 10. The bone transform

### EGEO-POSE-001: Transform of a bone

- **Inputs:** a bone of a baked geometry with the internal pivot `p`, and the pose values of
  the bone in internal space: the position `t`, the rotation `r` and the scale `s`
  (`entity-animation.md`, EANIM-BLEND-001). With no animation, `t = (0, 0, 0)`, `r` is the
  internal rest rotation, and `s = (1, 1, 1)`.
- **Behavior:**
  1. The local transform is `L = T(t) · T(p) · R(r) · S(s) · T(−p)`. `R(r)` is the Euler
     rotation of `r`.
  2. The model transform of a root bone is `L`.
  3. The model transform of another bone is `M_parent · L`.
  4. A vertex of a cube goes to `M_bone · v`, with `v` the corner of EGEO-BAKE-001.
  5. A normal goes to the normalised `M_bone` without translation, applied to the normal, with
     the inverse transpose when `s` is not uniform.
  6. The model transform is in model units. `client-entity-render.md` states how the model goes
     into the world.
- **Postconditions:** with no animation, the vertices are those of the rest pose.

Provenance: `documentation` (the bone transform of Blockbench). Kind: semantic. Confidence:
high. Milestone: 1.

### EGEO-POSE-002: Hidden bones

- **Inputs:** a bone that a render controller hides (`client-entity-definition.md`,
  CENT-RC-005), or a bone with `neverRender` (EGEO-FMT-002).
- **Behavior:**
  1. The cubes of the bone are not drawn.
  2. The transform of the bone still applies to its child bones.
  3. A hidden bone does not hide its child bones.
- **Postconditions:** none.

Provenance: `unresolved` for 3. The format reference does not state whether a hidden parent
hides its children. The vanilla Bedrock render controllers hide all bones and then show some
bones, so both meanings give the same result for them. Kind: semantic. Confidence: low.
Experiment: in Blockbench, load a geometry with a child bone under a hidden parent. Compare
with the Bedrock game. Milestone: 1.

---

## 11. Intentional deviations

### EGEO-DEV-001: `bind_pose_rotation` is ignored

- **Rule:** EGEO-FMT-002.
- **Reference:** the legacy format reads `bind_pose_rotation`. The format reference does not
  state its effect.
- **Visible to the client:** yes, for a legacy geometry that uses it. The bone can show at a
  different rotation.
- **Test:** a differential test skips a legacy geometry with `bind_pose_rotation`.
