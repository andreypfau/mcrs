# Block model resolution of the client: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of the game values: `assets/minecraft/version.json`.

The client resource pack has two kinds of file that give the shape of a block. A blockstate file
tells which models a block state draws. A block model file gives the elements, the textures and
the display transforms of a model, and it can take these from a parent model. This document
states how the client reads both kinds of file, how it selects the models of a block state and
of a block position, and how it resolves a model and its textures. The conversion of elements
into faces is in `block-face-bake.md` (BFB). The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Identifier**: as `registry.md`, REG-ID-001, defines it. An identifier with no namespace has
  the namespace `minecraft`. Two identifiers are equal when their namespaces and their paths are
  equal, after this rule applies.
- **Resource set**: the files that the client reads for the shapes of blocks (BMR-PACK-001).
- **Blockstate file**: the file `assets/<ns>/blockstates/<path>.json` of the block
  `<ns>:<path>`.
- **Model file**: the file `assets/<ns>/models/<path>.json` of the model `<ns>:<path>`.
- **State**: one block state. A state has one value for each property of its block.
- **Variant**: a JSON object that names one model and a rotation of it (BMR-BS-005).
- **Part**: one entry of a blockstate file that the state can draw. A part is a weighted list of
  variants. A single variant object is a weighted list with one entry of weight 1.
- **Selected parts**: the parts that a state draws, in their order (BMR-BS-003, BMR-BS-004).
- **Chosen variant**: the variant of a part that a block position draws (section 2).
- **Chain**: a model, then its parent, then the parent of the parent, to the first model with no
  parent (BMR-MODEL-002).
- **Resolved model**: the result of BMR-MODEL-003 to BMR-MODEL-006 for one model.
- **Slot**: a key of the `textures` object of a model, for example `side`.
- **Reference**: a slot value that starts with `#`. The rest of the string is the name of a
  slot.
- **Texture value**: a slot value that is not a reference: a texture identifier and a flag
  `force_translucent`.
- **Missing model**: the built-in model `minecraft:builtin/missing` (BMR-MODEL-007).
- **Missing texture**: the texture `minecraft:missingno`. The atlas makes it. It shows a
  magenta and black check pattern.
- **Content error**: a defect in a file of the resource set. The client continues. It writes
  one log line that names the file and the defect.
- **Artefact**: the file `crates/mcrs_minecraft_model/tests/fixtures/vanilla/block_models.json`.
  The task `dumpBlockModels` of `tools/vanilla-oracle` writes it. Its `units` object states the
  meaning of each field.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". Each rule has a Milestone line. `1` means the first release
of these rules, which replaces the present code. `later` means a rule that a later release
implements.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `BMR-PACK` | the resource set and the paths of its files |
| `BMR-BS` | blockstate files and the selected parts of a state |
| `BMR-PICK` | the chosen variant of a part at a block position |
| `BMR-MODEL` | model files, the chain and the resolved model |
| `BMR-TEST` | parity tests and experiments |
| `BMR-DEV` | intentional deviations |

---

## 1. The resource set

### BMR-PACK-001: Files of the resource set

- **Inputs:** the client resource pack, and the extra pack layers of mcrs.
- **Behavior:**
  1. The resource set holds each file under `assets/<ns>/<folder>/` of the client resource
     pack, for each namespace `<ns>` and each of these folders: `blockstates`, `models`,
     `textures`, `items`, `atlases`, `font` and `lang`. The files in sub-folders are included.
  2. An extra pack layer can add a file to the resource set. An extra pack layer must not
     supply a file that the resource set already holds. Such a file stops the load with an
     error that names the path of the file.
  3. The client reads the resource set one time, before it builds the first mesh. After that
     the resource set does not change. A rule of this document reads files only from the
     resource set.
- **Postconditions:** each path names at most one file.

Provenance: `design` (mcrs builds meshes without a wait for a file, so it holds all these files
in memory; one owner for each path removes the question of priority between layers). Kind:
semantic. Confidence: high. Milestone: 1.

### BMR-PACK-002: Paths of resources

- **Inputs:** an identifier `<ns>:<path>`, a folder and an extension.
- **Behavior:**
  1. The file of the resource is `assets/<ns>/<folder>/<path>.<extension>`.
  2. A blockstate uses the folder `blockstates` and the extension `json`. A model uses `models`
     and `json`. A texture uses `textures` and `png`.
  3. An identifier in a file of the resource set follows REG-ID-001. A string that is not a
     valid identifier is a content error of that file.
- **Postconditions:** the identifiers `block/stone` and `minecraft:block/stone` name the same
  file.

Provenance: `documentation` (the resource pack format). Kind: semantic. Confidence: high.
Milestone: 1.

---

## 2. Blockstate files

### BMR-BS-001: Structure of a blockstate file

- **Inputs:** the blockstate file of a block.
- **Behavior:**
  1. The file is a JSON object (RFC 8259). Comments and text after the object are content
     errors.
  2. The object has the field `variants`, the field `multipart`, or both. A file with neither
     field is a content error. The client ignores other fields.
  3. `variants` is a JSON object with at least one member. The key of a member is a variant key
     (BMR-BS-002). The value is a part (BMR-BS-005).
  4. `multipart` is a JSON array with at least one element. Each element is a JSON object with
     the field `apply` and the optional field `when`. `apply` is a part. `when` is a condition
     (BMR-BS-004).
  5. A blockstate file for an identifier that is not a block is ignored.
- **Postconditions:** a file with a content error of this rule, of BMR-BS-004 or of BMR-BS-005
  gives no part to any state of its block. Each state of the block then draws the missing model
  (BMR-BS-006).

Provenance: `source_suggested_black_box_confirmed` (the artefact: 1185 blocks use `variants`
and 104 blocks use `multipart`; no file uses both). Kind: semantic. Confidence: high for items 1
to 4; medium for the postcondition. Milestone: 1. Experiment: BMR-TEST-004.

### BMR-BS-002: Variant keys

- **Inputs:** a variant key, and the properties of the block.
- **Behavior:**
  1. The key is a list of terms. A comma separates two terms. The client ignores an empty term.
     So the empty key has no terms.
  2. A term is `<name>=<value>`. The first `=` separates the name and the value.
  3. The name must be the name of a property of the block. The value must be the text of a
     value of that property:
     - a boolean property accepts `true` and `false`;
     - an integer property accepts a decimal integer in its range, with an optional sign;
     - another property accepts the name of one of its values.
  4. When two terms name the same property, the last term applies.
  5. A key matches a state when the state has the value of each term of the key. The empty key
     matches each state.
- **Postconditions:** a key with a term that has no `=`, an unknown property or an unknown
  value is a content error. That key matches no state. The other keys of the file stay valid.

Provenance: `source_suggested_black_box_confirmed` (the artefact: 556 blocks use the empty key;
the parts of all 35 781 states agree with this rule). Kind: semantic. Confidence: high for items
1, 2 and 5; medium for items 3 and 4 and the postcondition. Milestone: 1. Experiment:
BMR-TEST-004.

### BMR-BS-003: Selected parts from `variants`

- **Inputs:** a state, and the members of `variants`.
- **Behavior:**
  1. When one key matches the state, the selected parts of the state are the part of that key.
  2. When two or more keys match the state, the file has a content error. The first of these
     keys in the order of the file applies.
  3. When no key matches the state, `variants` selects no part for the state.
- **Postconditions:** `variants` gives zero parts or one part to each state.

Provenance: `source_suggested_black_box_confirmed` for items 1 and 3 (the artefact); `design`
for item 2 (BMR-DEV-001). Kind: semantic. Confidence: high. Milestone: 1.

### BMR-BS-004: Conditions of `multipart`

- **Inputs:** a condition, and a state.
- **Behavior:**
  1. A condition is a JSON object.
  2. An object with exactly one member, with the key `AND` or the key `OR`, is a combined
     condition. Its value is an array of conditions.
     - `AND` is true when each condition of the array is true. An empty array is true.
     - `OR` is true when at least one condition of the array is true. An empty array is false.
  3. Another object is a set of property tests, with at least one member. It is true when each
     test is true.
  4. In a property test, the key is the name of a property of the block. The value is a JSON
     string, a JSON integer or a JSON boolean. An integer or a boolean is the same as the string
     of its JSON text, for example `true` is `"true"`.
  5. The string is a list of alternatives. A `|` separates two alternatives. An alternative is a
     value text, or `!` and a value text. The value texts are those of BMR-BS-002, item 3.
  6. An alternative without `!` accepts its value. An alternative with `!` accepts each value of
     the property except its value. The test is true when the state has a value that at least
     one alternative accepts.
- **Postconditions:** each of these is a content error of the whole file (BMR-BS-001):
  - an unknown property;
  - an unknown value;
  - an empty alternative, for example in `a||b`;
  - an empty set of property tests;
  - an object with the key `AND` or `OR` and another member.

Provenance: `source_suggested_black_box_confirmed` for items 1 to 5 without `!` (the artefact:
14 files use `AND`, 2 files use `OR`; the parts of all states agree); `source_suggested` for the
`!` alternative and for the postconditions (no file of the client resource pack uses them).
Kind: semantic. Confidence: high for items 1 to 5; medium for item 6 and the postconditions.
Milestone: 1. Experiment: BMR-TEST-004.

### BMR-BS-005: Parts and variants

- **Inputs:** a part in a blockstate file.
- **Behavior:**
  1. A part is a variant object, or a JSON array of variant objects with at least one element.
  2. A variant object has these fields. The client ignores other fields.

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `model` | identifier | required | the model of the variant |
| `x` | integer | `0` | the rotation about the X axis, in degrees |
| `y` | integer | `0` | the rotation about the Y axis, in degrees |
| `z` | integer | `0` | the rotation about the Z axis, in degrees |
| `uvlock` | boolean | `false` | the texture lock of BFB |
| `weight` | integer | `1` | the weight of the variant in its part; only in an array |

  3. The value of `x`, `y` and `z`, modulo 360, must be 0, 90, 180 or 270. The variant keeps the
     value modulo 360.
  4. `weight` must be 1 or more.
  5. The rotation and `uvlock` go to BFB without a change. BFB states the order of the three
     rotations.
- **Postconditions:** each of these is a content error of the whole file (BMR-BS-001): an empty
  array, a variant with no `model`, a rotation that item 3 refuses, and a weight below 1.

Provenance: `source_suggested_black_box_confirmed` (the artefact: each variant of each part
agrees; the client resource pack uses only the weight 2 and the rotations 90, 180 and 270).
Kind: semantic. Confidence: high for items 1, 2 and 5; medium for items 3, 4 and the
postconditions. Milestone: 1. Experiment: BMR-TEST-004.

### BMR-BS-006: Selected parts of a state

- **Inputs:** a block, its blockstate file, and a state of the block.
- **Behavior:**
  1. When `variants` selects a part for the state (BMR-BS-003), the selected parts are that part.
  2. Otherwise, when the file has `multipart`, the selected parts are the `apply` parts of the
     elements of `multipart` that are true for the state, in the order of the array. An element
     with no `when` is true for each state.
  3. Otherwise the state has no selected parts from the file. The state draws the missing model
     as its one part. This is a content error that names the block and the state.
  4. A block with no blockstate file, or with a file that has a content error of the whole file,
     draws the missing model for each state.
- **Postconditions:** under item 2 a state can have zero selected parts. Such a state draws
  nothing. It does not draw the missing model.

Provenance: `source_suggested_black_box_confirmed` for items 1 and 2 (the artefact: the parts of
all 35 781 states); `source_suggested` for items 3 and 4 and for a file with both fields.
Kind: semantic. Confidence: high for items 1 and 2; medium for items 3 and 4. Milestone: 1.
Experiment: BMR-TEST-004.

---

## 3. The chosen variant at a block position

### BMR-PICK-001: Position seed

- **Inputs:** a block position `(x, y, z)` of 32-bit signed integers.
- **Behavior:** the client calculates the 64-bit signed integer `seed(x, y, z)`. All products and
  sums wrap in two's complement.
  1. `a` is the 32-bit product `x · 3129871`, with its sign extended to 64 bits.
  2. `b` is the 64-bit product `z · 116129781`.
  3. `s = a XOR b XOR y`, where `y` has its sign extended to 64 bits.
  4. `s = s · s · 42317861 + s · 11`, in 64 bits.
  5. `seed(x, y, z)` is `s` shifted right by 16 bits, with the sign kept.
- **Postconditions:** `seed(0, 0, 0)` is `0`. `seed(1, 64, −3)` is `−62687207132611`.

Provenance: `source_suggested_black_box_confirmed` (the artefact: the field `seed` of each entry
of `picks`). Kind: client agreement. Confidence: high. Milestone: 1.

### BMR-PICK-002: Seed of a state

- **Inputs:** a state at the block position `p = (x, y, z)`, and the properties of its block.
- **Behavior:**
  1. The seed rule of a block comes from its properties. The client does not read the artefact
     at run time:
     - a block with a property `half` whose values are exactly `upper` and `lower` has the seed
       rule `lower_half`;
     - a block with a property `part` whose values are exactly `head` and `foot`, and with a
       property `facing`, has the seed rule `bed_head`;
     - each other block has no seed rule, and uses `seed(x, y, z)`.
  2. A block with `lower_half` has the property `half`. A state with `half=upper` uses
     `seed(x, y − 1, z)`. A state with `half=lower` uses `seed(x, y, z)`. These blocks are the
     doors and the plants that are two blocks high.
  3. A block with `bed_head` is a bed, with the properties `facing` and `part`. A state with
     `part=head` uses `seed(x, y, z)`. A state with `part=foot` uses `seed(x + dx, y, z + dz)`.
     `(dx, dz)` is the step of `facing`: `north` `(0, −1)`, `south` `(0, 1)`, `east` `(1, 0)`
     and `west` `(−1, 0)`.
- **Postconditions:** the two halves of a door, of a tall plant and of a bed have the same seed.
  Item 1 gives the `seed_rule` of the artefact for each block of the artefact.

Provenance: `design` for item 1 (the property test replaces a list of blocks, so that a new
block of the same shape needs no change of the client);
`source_suggested_black_box_confirmed` for items 2 and 3 (the artefact: `seed_rule` of 49 blocks, and
the seeds of `picks` for `minecraft:oak_door`, `minecraft:red_bed`, `minecraft:tall_grass` and
`minecraft:sunflower` in each state). Kind: client agreement. Confidence: high. Milestone: 1.

### BMR-PICK-003: Random generator

- **Inputs:** a 64-bit seed.
- **Behavior:** the generator is the 48-bit linear congruential generator that the
  documentation of the class `java.util.Random` of the Java platform states. `M` is `2^48`.
  1. To set the seed `s`, the state becomes `(s XOR 0x5DEECE66D) mod M`.
  2. `next(bits)`: the state becomes `(state · 0x5DEECE66D + 0xB) mod M`. The result is the
     state shifted right by `48 − bits`, as a 32-bit signed integer.
  3. `next_int(n)`, for `n ≥ 1`:
     - when `n` is a power of two, the result is `(n · next(31)) >> 31`, in 64 bits;
     - otherwise, `r = next(31)` and `m = r mod n`. When `r − m + (n − 1)` overflows a 32-bit
       signed integer, the generator calculates a new `r`. Otherwise the result is `m`.
  4. `next_long()`: `hi = next(32)` and then `lo = next(32)`. The result is `(hi · 2^32) + lo`
     in 64 bits, where `lo` has its sign extended.
- **Postconditions:** the same seed gives the same sequence.

Provenance: `documentation` (the Java platform); `source_suggested_black_box_confirmed` that the
client uses it (the artefact: `chosen` of each entry of `picks`). Kind: client agreement.
Confidence: high. Milestone: 1.

### BMR-PICK-004: Chosen variant of a part

- **Inputs:** a part with the variants `v[0] … v[k−1]` in the order of the file, their weights
  `w[i]`, and a generator.
- **Behavior:**
  1. `W` is the sum of the weights.
  2. `r = next_int(W)`.
  3. The chosen variant is the first `v[i]` for which `r − (w[0] + … + w[i])` is below 0.
- **Postconditions:** a part with one variant always gives that variant.

Provenance: `source_suggested_black_box_confirmed` (the artefact: `picks`). Kind: client
agreement. Confidence: high. Milestone: 1.

### BMR-PICK-005: Chosen variants of a state

- **Inputs:** a state at a block position, its seed (BMR-PICK-002), and its selected parts.
- **Behavior:**
  1. The client sets the seed of a generator to the seed of the state.
  2. When the selected parts come from `variants`, BMR-PICK-004 gives the chosen variant of the
     one part, with this generator.
  3. When the selected parts come from `multipart`, `t = next_long()`. Then, for each selected
     part in order, the client sets the seed of the generator to `t`, and BMR-PICK-004 gives the
     chosen variant of that part.
  4. The missing model has one variant with no rotation.
- **Postconditions:** each selected part has one chosen variant. The chosen variants depend
  only on the state and the block position.

Provenance: `source_suggested_black_box_confirmed` (the artefact: 732 entries of `picks`, for
each block with a weighted part in its default state, at six positions). Kind: client
agreement. Confidence: high. The multipart block `minecraft:chorus_plant` has weighted parts,
and its entries of `picks` confirm item 3. Milestone: 1. Experiment: BMR-TEST-004.

---

## 4. Models

### BMR-MODEL-001: Structure of a model file

- **Inputs:** a model file.
- **Behavior:**
  1. The file is a JSON object (RFC 8259). Comments and text after the object are content
     errors. The client ignores fields that this rule does not name.
  2. The object can have these fields:

| Field | Type | Meaning |
|-------|------|---------|
| `parent` | identifier | the parent model; the empty string means no parent |
| `elements` | array | the elements of the model; BFB states an element |
| `textures` | object | the slots of the model (BMR-MODEL-004) |
| `ambientocclusion` | boolean | the ambient occlusion flag (BMR-MODEL-003) |
| `gui_light` | string | `front` or `side` (BMR-MODEL-003) |
| `display` | object | the display transforms (BMR-MODEL-006) |

  3. A slot value is one of these:
     - a JSON string that starts with `#`: a reference;
     - another JSON string: a texture identifier, with `force_translucent` false;
     - a JSON object with the field `sprite`, a texture identifier, and the optional boolean
       field `force_translucent`, with the default `false`.
- **Postconditions:** a file with a content error has no model. The client treats the model as
  absent (BMR-MODEL-002).

Provenance: `documentation` (the resource pack format) for the fields;
`source_suggested_black_box_confirmed` for item 3 (the artefact: 110 model files use the object
form; 371 resolved slots have `force_translucent`). Kind: semantic. Confidence: high.
Milestone: 1.

### BMR-MODEL-002: The chain

- **Inputs:** a model identifier `m`.
- **Behavior:**
  1. The chain of `m` starts with `m`. Each next model is the `parent` of the model before it.
     The chain ends at a model with no parent.
  2. When the model file of `m` is absent or has a content error, a reference to `m` gives the
     missing model. This is a content error that names `m`.
  3. When the `parent` of a model is absent or has a content error, the missing model is the
     parent. The model takes inherited fields from the missing model.
  4. When the chain of `m` contains one model two times, the chain has a cycle. `m` is then
     invalid, and each reference to `m` gives the missing model. This is a content error that
     names `m`.
  5. `minecraft:builtin/missing` and `minecraft:builtin/generated` are built-in models with no
     parent (BMR-MODEL-007). A model file with one of these identifiers has no effect.
  6. The chain has no length limit other than item 4.
- **Postconditions:** each chain is finite. Each chain ends at a model with no parent.

Provenance: `source_suggested_black_box_confirmed` for items 1 and 5 (the artefact: `parent` and
`geometry_from` of 4215 models); `source_suggested` for items 2, 3, 4 and 6 (no model of the
client resource pack has an absent parent or a cycle). Kind: semantic. Confidence: high for
items 1 and 5; medium for items 2 to 4 and 6. Milestone: 1. Experiment: BMR-TEST-004.

### BMR-MODEL-003: Inherited fields

- **Inputs:** the chain of a model.
- **Behavior:** for each of these fields, the resolved model takes the value of the first model
  of the chain that has the field. A model has the field when its file has the key, also when
  the value is empty.
  1. `elements`. When no model of the chain has it, the resolved model has no elements. An empty
     array gives a model with no elements. The elements of two models never merge.
  2. `ambientocclusion`. The default is `true`.
  3. `gui_light`. The default is `side`. Another string than `front` or `side` is a content
     error of the file.
- **Postconditions:** the field `geometry_from` of the artefact names the model that supplies
  `elements`. It is absent when no model of the chain has `elements`.

Provenance: `source_suggested_black_box_confirmed` (the artefact: `geometry_from`,
`ambient_occlusion` and `gui_light` of 4215 models). Kind: semantic. Confidence: high.
Milestone: 1.

### BMR-MODEL-004: Texture slots

- **Inputs:** the chain of a model.
- **Behavior:**
  1. The client merges the `textures` objects of the chain. It starts at the last model of the
     chain and ends at the first model. A slot of a later step replaces the same slot of an
     earlier step. A reference replaces a texture value, and a texture value replaces a
     reference.
  2. After the merge, each slot with a texture value is resolved to that value.
  3. Then the client repeats this step until a pass changes nothing: each slot with a reference
     to a resolved slot becomes resolved, with the texture value of the target slot.
  4. A slot that stays unresolved has no texture. This is a content error that names the model
     and the slot.
- **Postconditions:** a reference to a slot that only a child model gives resolves in the child
  model. For example, `minecraft:block/cube_column` has its slot `up` unresolved, and
  `minecraft:block/oak_log` resolves `up` through `end`. A cycle of references stays unresolved.

Provenance: `source_suggested_black_box_confirmed` (the artefact: `textures` and `unresolved` of
4215 models). Kind: semantic. Confidence: high. Milestone: 1.

### BMR-MODEL-005: Texture of a face and the particle texture

- **Inputs:** a resolved model, and the string `texture` of a face of one of its elements.
- **Behavior:**
  1. When the string starts with `#`, the client removes the `#`. The rest is the name of a
     slot. A string without `#` is also the name of a slot, not a texture identifier.
  2. When the slot is resolved, the face uses its texture value.
  3. Otherwise the face uses the missing texture. This is a content error that names the model
     and the slot.
  4. The particle texture of the model is the texture value of the slot `particle`. When that
     slot has no texture, the particle texture is the missing texture.
- **Postconditions:** the value `force_translucent` goes with the texture to BFB.

Provenance: `source_suggested` (the model files `minecraft:block/heavy_core` use the face
texture `all` without `#`). Kind: semantic. Confidence: medium. Milestone: 1. Experiment:
BMR-TEST-004.

### BMR-MODEL-006: Display transforms

- **Inputs:** the chain of a model.
- **Behavior:**
  1. The display contexts are `thirdperson_righthand`, `thirdperson_lefthand`,
     `firstperson_righthand`, `firstperson_lefthand`, `head`, `gui`, `ground`, `fixed` and
     `on_shelf`.
  2. In one model file, the key of a context in `display` declares the context. When the file
     declares `thirdperson_righthand` and not `thirdperson_lefthand`, the left context takes the
     value of the right context of the same file. The same applies to `firstperson_righthand`
     and `firstperson_lefthand`.
  3. A transform object has the optional fields `rotation`, `translation` and `scale`. Each is an
     array of exactly three numbers. The defaults are `(0, 0, 0)`, `(0, 0, 0)` and `(1, 1, 1)`.
  4. `rotation` is in degrees. The client divides `translation` by 16, then clamps each
     component to `[−5, 5]`. The client clamps each component of `scale` to `[−4, 4]`.
  5. For each context, the resolved model takes the transform of the first model of the chain
     that declares the context. An empty object `{}` declares the context with the defaults. A
     context that no model declares has the identity transform: the rotation `(0, 0, 0)`, the
     translation `(0, 0, 0)` and the scale `(1, 1, 1)`.
  6. The fields of two transforms never merge.
- **Postconditions:** an array with a length other than 3 is a content error of the file.

Provenance: `source_suggested_black_box_confirmed` (the artefact: `display_table` and the
`display` index of 4215 models). Kind: semantic. Confidence: high. Milestone: 1 for `gui`;
later for the other contexts, which item rendering in the hand and on the ground uses.

### BMR-MODEL-007: Built-in models

- **Inputs:** none.
- **Behavior:**
  1. The missing model `minecraft:builtin/missing` behaves as this model file:

```json
{
  "textures": { "particle": "#missingno", "missingno": "minecraft:missingno" },
  "elements": [
    {
      "from": [0, 0, 0],
      "to": [16, 16, 16],
      "faces": {
        "down":  { "uv": [0, 0, 16, 16], "texture": "#missingno", "cullface": "down" },
        "up":    { "uv": [0, 0, 16, 16], "texture": "#missingno", "cullface": "up" },
        "north": { "uv": [0, 0, 16, 16], "texture": "#missingno", "cullface": "north" },
        "south": { "uv": [0, 0, 16, 16], "texture": "#missingno", "cullface": "south" },
        "west":  { "uv": [0, 0, 16, 16], "texture": "#missingno", "cullface": "west" },
        "east":  { "uv": [0, 0, 16, 16], "texture": "#missingno", "cullface": "east" }
      }
    }
  ]
}
```

  2. The generated model `minecraft:builtin/generated` has no parent and the slot `particle`
     with the reference `#layer0`. Its `gui_light` is `front`. Its elements are the extruded
     shapes of the slots `layer0` to `layer4`. The item model specification states them.
- **Postconditions:** `minecraft:item/generated` has `gui_light` `front` and takes
  `geometry_from` from `minecraft:builtin/generated`.

Provenance: `source_suggested_black_box_confirmed` (the artefact: the models
`minecraft:builtin/missing`, `minecraft:item/generated` and `minecraft:item/stick`). Kind:
semantic. Confidence: high. Milestone: 1.

---

## 5. Parity tests

### BMR-TEST-001: Selected parts against the artefact

- **Inputs:** the client resource pack and the artefact.
- **Behavior:**
  1. For each block of `blocks` and each entry of `states`, get the selected parts of the state
     (BMR-BS-006).
  2. Compare them with the entries of `part_table` that `parts` names, in order. Compare
     `model`, `x`, `y`, `z`, `uvlock` and `weight` of each variant.
  3. An entry with `missing` must draw the missing model.
- **Postconditions:** the test fails when one state does not agree.

Provenance: `source_suggested_black_box_confirmed` (the artefact). Kind: semantic. Confidence:
high. Milestone: 1.

### BMR-TEST-002: Resolved models against the artefact

- **Inputs:** the client resource pack and the artefact.
- **Behavior:**
  1. For each entry of `models`, resolve the model of `id`.
  2. Compare the model that supplies the elements with `geometry_from`.
  3. Compare the ambient occlusion flag and `gui_light`.
  4. Compare each resolved slot with `textures`: the texture identifier and
     `force_translucent`. Compare the unresolved slots of the chain with `unresolved`.
  5. Compare the transform of each context with the entry of `display_table` that `display`
     names, within `1e-6`. A context that the entry does not list has no transform.
- **Postconditions:** the test fails when one model does not agree.

Provenance: `source_suggested_black_box_confirmed` (the artefact). Kind: semantic. Confidence:
high. Milestone: 1, with item 5 for `gui` only until the other contexts are implemented.

### BMR-TEST-003: Chosen variants against the artefact

- **Inputs:** the artefact.
- **Behavior:**
  1. For each entry of `picks`, calculate the seed of the state at `position` (BMR-PICK-002).
     Compare it with `seed`.
  2. Calculate the chosen variants (BMR-PICK-005). Compare the index of each chosen variant in
     its part with `chosen`.
- **Postconditions:** the test fails when one entry does not agree.

Provenance: `source_suggested_black_box_confirmed` (the artefact). Kind: client agreement.
Confidence: high. Milestone: 1.

### BMR-TEST-004: Experiments for the content errors

- **Inputs:** the reference client, and a resource pack with the files of each step.
- **Behavior:** for each step, load the pack, look at a block of each listed state, and read the
  log.
  1. Give a block a blockstate file with an unknown property in one variant key. Expect: the
     other keys apply; the states of that key draw the missing model.
  2. Give a fence a multipart condition with an unknown value. Expect: each state draws the
     missing model.
  3. Give a fence the condition `{"north": "!true"}` on the post part. Expect: the post shows
     only when `north` is `false`.
  4. Give a block both `variants` with only the key `facing=north` and `multipart`. Expect: the
     north state draws its variant; the other states draw the multipart parts.
  5. Give a model a parent that does not exist. Expect: the model has the elements of the
     missing model when it has no elements of its own.
  6. Make two models that name each other as the parent. Expect: each state that uses them
     draws the missing model.
  7. Give a face the texture `#nothing`, with no slot `nothing`. Expect: the face shows the
     missing texture.
  8. Give a multipart block a weighted part with two variants. Look at 20 positions. Expect:
     the chosen variants agree with BMR-PICK-005, item 3.
  9. Give a block the variant rotation `x: 45`. Expect: each state of the block draws the
     missing model.
- **Postconditions:** each result changes the label of the rule that it tests to a confirmed
  label, or it changes the rule.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Milestone: 1.

---

## 6. Intentional deviations

### BMR-DEV-001: Two variant keys that match one state

- **Rule:** BMR-BS-003, item 2.
- **Reference:** the reference gives the state the part of the later key, and it stops the
  assignment of the later key at that state. The states of the later key that come after it
  keep the part of an earlier key or draw the missing model.
- **Visible to the client:** only for a resource pack with this content error. The client
  resource pack has no such file.
- **Test:** none. The rule is a choice for a defective file.

### BMR-DEV-002: Pack layers

- **Rule:** BMR-PACK-001, item 2.
- **Reference:** the reference loads a stack of resource packs. A model file of a higher pack
  replaces the file of a lower pack. For a blockstate file, the states that a higher pack
  assigns replace the states of a lower pack, and the other states keep the parts of the lower
  pack.
- **Visible to the client:** only for a resource pack that replaces a file of the client
  resource pack. mcrs refuses such a pack.
- **Test:** none.

---

## 7. Open questions

1. BMR-BS-001: the reference can log a parse error of a blockstate file and still use a lower
   pack. The order of log lines is not specified.
2. BMR-MODEL-001: the effect of a duplicate key in a JSON object is not specified.
3. BMR-PICK-005: the client resource pack has no multipart block with a weighted part. Item 3
   depends on experiment 8 of BMR-TEST-004.
