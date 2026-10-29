# Registry fields: references and write forms

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |
| World version (`DataVersion`) | `5124` |
| Data pack format | `124.0` |

Source of these values: `assets/minecraft/version.json`.

This document extends `registry.md`. For each synced registry of REG-NET-004, and for
each value type that a synced entry can contain, it lists two things:

- each field that names an entry or a tag of another registry, with its target, its kind and its write form;
- each optional field that the server leaves out when the field value equals its default.

The document does not give the full schema of a registry. Field types, numeric ranges
and required fields stay out of scope (`registry.md` §0.6), except where a rule here
needs them.

## 0. Conventions

### 0.1 Relation to `registry.md`

- The terms of `registry.md` §0.1 apply. "Key", "tag", "inline value" and "entry set" have the meaning of REG-REF-001.
- The provenance labels and confidence levels of `registry.md` §0.3 apply.
- The names rule of `registry.md` §0.7 applies. This document names data, never code.
- The rule prefix is `REG-FLD`. Rules 001 to 099 are general. Rules 101 to 133 are the synced registries, in the order of REG-NET-004. Rules from 200 are value types.

### 0.2 How to read a rule

Each value type has one rule. A rule has these parts:

- **Reached from:** the rules whose fields contain this type.
- **References:** one row for each field that names an entry or a tag of another registry. The columns are:
  - **Field:** the JSON path, relative to the object of this rule.
  - **Target registry:** the registry that the field names.
  - **Kind:** `key only`, `reference or inline`, `entry set` or `tag key` (REG-FLD-001).
  - **Checked:** `yes` if the load fails when the key or tag is absent. `no` if the field keeps the key unchecked.
  - **Inline:** for an entry set only. `yes` if a list item can be an inline value.
  - **Write form:** for an entry set, or for a list of keys that is not an entry set (REG-FLD-002).
- **Fields left out on write:** one row for each optional field with a default value (REG-FLD-003). The column **Lenient** is `yes` if an invalid value decodes as the default, instead of an entry error.
- **Not written on the wire:** for a synced registry with a network form only (REG-FLD-004).
- **Write form notes:** behaviour that the tables cannot show.
- **Nested types:** the rules of the value types that this type contains.

A dispatch type, for example a loot condition, has one rule for the `type` field and one rule for each type id.

## 1. General rules

### REG-FLD-001: Reference kinds

- **Inputs:** a field of a synced entry, or of a value inside it, that names an entry or a tag of a registry T.
- **Behavior:** each such field has one of these kinds:

| Kind | Accepted JSON | Decode |
|------|---------------|--------|
| `key only` | `"ns:path"` | The key of an entry of T. An object is an entry error. |
| `reference or inline` | `"ns:path"` or `{ ... }` | A string that is a valid identifier is always a key. It is never read as an inline value. An object is an inline value of T. |
| `entry set` | `"#ns:path"`, `"ns:path"` or `["ns:a", ...]` | A tag, one entry, or a list of entries (REG-REF-001). If the column Inline is `yes`, a list item can be an inline value. |
| `tag key` | `"ns:path"` or `"#ns:path"` | The id of a tag of T. A note in the rule states whether the `#` is required. |

  - If Checked is `yes`, an absent key or tag fails the decode of the entry. REG-REF-002 states how this failure propagates for each stage.
  - If Checked is `no`, the field keeps the key unchecked. The game looks it up only at use time.
  - A short key without a namespace, for example `stone`, decodes as `minecraft:stone` (REG-ID-001).
- **Edge cases:**
  - `sound_event` is the only static registry whose references accept an inline value. All other references to static registries are `key only`.
  - The stage 3 registries (`loot_table`, `predicate`, `item_modifier`, `context_int_provider`, `context_float_provider`, `slot_source`) do not exist when a synced entry loads. A key or a tag in a `reference or inline` field or an `entry set` of these registries fails the load (REG-REF-002). Only an inline value loads there.
  - A `type` field of a dispatch type is a `key only` reference to the type registry of that dispatch, and it is checked.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-002: Write form of references

- **Inputs:** a decoded entry that the server writes into a registry data packet (REG-NET-005).
- **Behavior:**
  - The server encodes the decoded value again. It does not keep the text of the file.
  - A key is written in full, as `namespace:path`.
  - A tag is written as `#namespace:path`.
  - An inline value is written inline, and a key is written as a key. The server never changes one form into the other. An inline value equal to a registered entry stays inline.
  - An entry set that is a tag is written as the tag. An entry set that is not a tag has one of these write forms:
    - `bare value for one element`: one element is written as a bare string. Zero elements, or two or more, are written as a list.
    - `always a list`: the set is always written as a list.
  - A list of keys that is not an entry set has one of these write forms:
    - `always a list`.
    - `bare value for one element`, as above.
    - `single value only`: the field holds one value and never a list.
  - No field holds a numeric registry id, and no field holds the resolved members of a tag.
- **Postconditions:** a vanilla client decodes the file form and the normal form to the same value. A difference in form is a byte difference only, not a conformance failure.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-003: Fields left out on write

- **Inputs:** an optional field that has a default value.
- **Behavior:**
  - On write, the server compares the field value with the default. If they are equal, the server does not write the field. This applies also when the file writes the field.
  - Numbers compare by value and type. `-0.0` is not equal to `0.0`, so a decoded `-0.0` is written. NaN is equal to NaN.
  - Lists, maps and objects compare element by element.
  - For a `reference or inline` field, a key is never equal to an inline default. A key is written even when its entry equals the default value. Only an inline value equal to the default is left out.
  - A value type that has no value equality never compares equal to the default. Its field is always written. Each rule notes such a field.
  - An optional field with no default value is written only when it is present. The tables do not list such fields.
- **Edge cases:** if a field is **Lenient**, an invalid value decodes as the default. The server then leaves the field out on write.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-004: Network form

- **Inputs:** a synced registry R.
- **Behavior:** the server writes most synced registries in the file form. These registries have a network form, which writes fewer fields:

| Registry | Rule |
|----------|------|
| `worldgen/biome` | REG-FLD-101 |
| `wolf_variant`, `pig_variant`, `frog_variant`, `cat_variant`, `cow_variant`, `chicken_variant`, `zombie_nautilus_variant` | REG-FLD-105, 107, 109, 110, 112, 114, 116 |
| `dimension_type` | REG-FLD-119 |
| `timeline` | REG-FLD-129 |

  - The table "Not written on the wire" of each of these rules lists the fields that the network form leaves out.
  - A field that the network form does not write cannot hold a reference on the wire.
- **Postconditions:** the client decodes the network form. It does not need the left-out fields.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-005: Identifiers that are not references

- **Behavior:** some fields hold an identifier that does not name a registry entry. Examples are texture ids, asset ids, function ids and translation keys. These fields are not in the tables of references. The load does not check them.
- **Edge cases:** fields that hold raw NBT, for example the `custom_data` item component, are copied as they are. A loot table id or another id in raw NBT is not a reference. The game reads it only at use time.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-009: Experiments

Each rule of this document uses these experiments.

- **Reference kind.** For a field F of a synced registry R:
  1. Make a pack with one entry of R. Set F to a key that exists.
  2. Start the server. Expected: the server starts.
  3. Set F to a key that does not exist. Expected: startup fails if Checked is `yes`, and the server starts if Checked is `no`.
  4. Set F to an inline object. Expected: the load succeeds only for `reference or inline`, or for an entry set with Inline `yes`.
- **Write form.** Set an entry set or a key list to one element in list form, for example `["minecraft:a"]`. Capture the registry data packet with the negotiated set empty (REG-NET-003). Compare the form of the field with the column Write form.
- **Left-out field.** Write the field with its default value. Capture the registry data packet with the negotiated set empty. Expected: the field is absent. Write a different value. Expected: the field is present.
- **Network form.** Write each field of the table "Not written on the wire". Capture the packet. Expected: the fields are absent.

None of these experiments has been run.

## 2. Synced registries

Each rule in this section is one synced registry of REG-NET-004, in that order.

### REG-FLD-101: Registry `worldgen/biome`

- **Reached from:** the synced registry `worldgen/biome`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `attributes` (map keys) | `environment_attribute` | key only | yes | — | — |
| `carvers` (file form only) | `worldgen/configured_carver` | entry set | yes | yes | bare value for one element |
| `features[]` (file form only) | `worldgen/placed_feature` | entry set | yes | yes | always a list |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `temperature_modifier` | `"none"` | no |
| `attributes` | `{}` | no |

- **Not written on the wire:**

| Field |
|-------|
| `carvers` |
| `features` |
| `attributes.<id>` for each attribute that is not syncable |

- **Write form notes:**
  - The fields `has_precipitation`, `temperature`, `temperature_modifier` and `downfall` are written in the biome object itself.
  - The field `effects` is required. Its value is a biome special effects (REG-FLD-250) object.
  - The network form compares `attributes` with the default `{}` before it removes the attributes that are not syncable. Thus a biome with only non-syncable attributes writes `"attributes": {}` on the wire. An example is a biome with only `gameplay/natural_mob_spawns`.
  - environment attribute map (REG-FLD-253) lists the syncable flag of each attribute.
  - The file form rejects attributes that are not positional: `gameplay/sky_light_level` and `gameplay/fast_lava`. The network form has no positional check.
  - In the file form, `carvers` and `features` are required.
  - In the file form, `features` is always a list. Each item is an entry set, and each entry set is always written as a list.
  - In `carvers` and `features`, a tag needs the `#` prefix.
- **Nested types:** environment attribute map (REG-FLD-253), biome special effects (REG-FLD-250).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-102: Registry `chat_type`

- **Reached from:** the synced registry `chat_type`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The network form is the same as the file form.
  - `chat` and `narration` are required. Each is a chat type decoration (REG-FLD-300).
- **Nested types:** chat type decoration (REG-FLD-300).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-103: Registry `trim_pattern`

- **Reached from:** the synced registry `trim_pattern`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The network form is the same as the file form.
  - `asset_id` is an identifier of an asset. It is not a reference.
  - The load reads an absent `decal` as `false`.
  - The write always writes `decal`, also when its value is `false`.
  - `description` is a text component (REG-FLD-429).
- **Nested types:** text component (REG-FLD-429).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-104: Registry `trim_material`

- **Reached from:** the synced registry `trim_material`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The network form is the same as the file form.
  - `palette_id` is an identifier of an asset. It is not a reference.
  - `description` is a text component (REG-FLD-429).
- **Nested types:** text component (REG-FLD-429).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-105: Registry `wolf_variant`

- **Reached from:** the synced registry `wolf_variant`.
- **References:** none.
- **Fields left out on write:** none.
- **Not written on the wire:**

| Field |
|-------|
| `spawn_conditions` |

- **Write form notes:**
  - The file form also has `spawn_conditions`. The network form does not have this field.
  - `assets` and `baby_assets` hold texture identifiers. They are not references.
- **Nested types:** wolf asset info (REG-FLD-200). In the file form only: spawn prioritized selectors (REG-FLD-211).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-106: Registry `wolf_sound_variant`

- **Reached from:** the synced registry `wolf_sound_variant`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The network form is the same as the file form.
  - `adult_sounds` and `baby_sounds` are required. Each is a wolf sound set (REG-FLD-201).
- **Nested types:** wolf sound set (REG-FLD-201).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-107: Registry `pig_variant`

- **Reached from:** the synced registry `pig_variant`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `model` | `"normal"` | no |

- **Not written on the wire:**

| Field |
|-------|
| `spawn_conditions` |

- **Write form notes:**
  - The file form also has `spawn_conditions`. The network form does not have this field.
  - `model` and `asset_id` are written in this object. They come from model and texture (REG-FLD-202).
  - `model` is one of `normal` or `cold`.
  - `asset_id` and `baby_asset_id` are identifiers of a texture. They are not references. Both fields are required.
- **Nested types:** model and texture (REG-FLD-202). In the file form only: spawn prioritized selectors (REG-FLD-211).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-108: Registry `pig_sound_variant`

- **Reached from:** the synced registry `pig_sound_variant`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The network form is the same as the file form.
  - `adult_sounds` and `baby_sounds` are required. Each is a pig sound set (REG-FLD-203).
- **Nested types:** pig sound set (REG-FLD-203).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-109: Registry `frog_variant`

- **Reached from:** the synced registry `frog_variant`.
- **References:** none.
- **Fields left out on write:** none.
- **Not written on the wire:**

| Field |
|-------|
| `spawn_conditions` |

- **Write form notes:**
  - The file form also has `spawn_conditions`. The network form does not have this field.
  - `asset_id` is an identifier of a texture. It is not a reference. The field is required.
- **Nested types:** none. In the file form only: spawn prioritized selectors (REG-FLD-211).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-110: Registry `cat_variant`

- **Reached from:** the synced registry `cat_variant`.
- **References:** none.
- **Fields left out on write:** none.
- **Not written on the wire:**

| Field |
|-------|
| `spawn_conditions` |

- **Write form notes:**
  - The file form also has `spawn_conditions`. The network form does not have this field.
  - `asset_id` and `baby_asset_id` are identifiers of a texture. They are not references. Both fields are required.
- **Nested types:** none. In the file form only: spawn prioritized selectors (REG-FLD-211).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-111: Registry `cat_sound_variant`

- **Reached from:** the synced registry `cat_sound_variant`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The network form is the same as the file form.
  - `adult_sounds` and `baby_sounds` are required. Each is a cat sound set (REG-FLD-204).
- **Nested types:** cat sound set (REG-FLD-204).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-112: Registry `cow_variant`

- **Reached from:** the synced registry `cow_variant`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `model` | `"normal"` | no |

- **Not written on the wire:**

| Field |
|-------|
| `spawn_conditions` |

- **Write form notes:**
  - The file form also has `spawn_conditions`. The network form does not have this field.
  - `model` and `asset_id` are written in this object. They come from model and texture (REG-FLD-202).
  - `model` is one of `normal`, `cold` or `warm`.
  - `asset_id` and `baby_asset_id` are identifiers of a texture. They are not references. Both fields are required.
- **Nested types:** model and texture (REG-FLD-202). In the file form only: spawn prioritized selectors (REG-FLD-211).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-113: Registry `cow_sound_variant`

- **Reached from:** the synced registry `cow_sound_variant`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `ambient_sound` | `sound_event` | reference or inline | yes | — | — |
| `hurt_sound` | `sound_event` | reference or inline | yes | — | — |
| `death_sound` | `sound_event` | reference or inline | yes | — | — |
| `step_sound` | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The network form is the same as the file form.
  - The sound fields are at the top level of the entry. This entry has no `adult_sounds` or `baby_sounds`.
  - Each sound field is optional and has no default. An absent sound field is not written.
- **Nested types:** sound event (REG-FLD-302).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-114: Registry `chicken_variant`

- **Reached from:** the synced registry `chicken_variant`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `model` | `"normal"` | no |

- **Not written on the wire:**

| Field |
|-------|
| `spawn_conditions` |

- **Write form notes:**
  - The file form also has `spawn_conditions`. The network form does not have this field.
  - `model` and `asset_id` are written in this object. They come from model and texture (REG-FLD-202).
  - `model` is one of `normal` or `cold`.
  - `asset_id` and `baby_asset_id` are identifiers of a texture. They are not references. Both fields are required.
- **Nested types:** model and texture (REG-FLD-202). In the file form only: spawn prioritized selectors (REG-FLD-211).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-115: Registry `chicken_sound_variant`

- **Reached from:** the synced registry `chicken_sound_variant`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The network form is the same as the file form.
  - `adult_sounds` and `baby_sounds` are required. Each is a chicken sound set (REG-FLD-205).
- **Nested types:** chicken sound set (REG-FLD-205).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-116: Registry `zombie_nautilus_variant`

- **Reached from:** the synced registry `zombie_nautilus_variant`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `model` | `"normal"` | no |

- **Not written on the wire:**

| Field |
|-------|
| `spawn_conditions` |

- **Write form notes:**
  - The file form also has `spawn_conditions`. The network form does not have this field.
  - `model` and `asset_id` are written in this object. They come from model and texture (REG-FLD-202).
  - `model` is one of `normal` or `warm`.
  - `asset_id` is an identifier of a texture. It is not a reference. This entry has no `baby_asset_id`.
- **Nested types:** model and texture (REG-FLD-202). In the file form only: spawn prioritized selectors (REG-FLD-211).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-117: Registry `painting_variant`

- **Reached from:** the synced registry `painting_variant`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The network form is the same as the file form.
  - `width` and `height` are required integers from 1 to 16.
  - `asset_id` is an identifier of a texture. It is not a reference. The field is required.
  - `title` and `author` are optional and have no default. An absent field is not written.
- **Nested types:** text component (REG-FLD-429) (`title`, `author`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-118: Registry `sulfur_cube_archetype`

- **Reached from:** the synced registry `sulfur_cube_archetype`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `items` | `item` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `buoyant` | `false` | no |

- **Write form notes:**
  - The network form is the same as the file form.
  - A tag in `items` requires the `#` prefix.
  - `attribute_modifiers` is required. It is always a list of sulfur cube attribute entry (REG-FLD-206).
  - `knockback_modifiers` and `sound_settings` are required. They have no default, so the encoder always writes them.
  - `explosion` and `contact_damage` are optional and have no default. An absent field is not written.
- **Nested types:** sulfur cube attribute entry (REG-FLD-206), sulfur cube explosion (REG-FLD-207), sulfur cube contact damage (REG-FLD-208), sulfur cube knockback modifiers (REG-FLD-209), sulfur cube sound settings (REG-FLD-210).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-119: Registry `dimension_type`

- **Reached from:** the synced registry `dimension_type`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `infiniburn` | `block` | entry set | yes | no | bare value for one element |
| `timelines` | `timeline` | entry set | yes | no | bare value for one element |
| `default_clock` | `world_clock` | key only | yes | — | — |
| `attributes` (map keys) | `environment_attribute` | key only | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `has_fixed_time` | `false` | no |
| `skybox` | `"overworld"` | no |
| `cardinal_light` | `"default"` | no |
| `attributes` | `{}` | no |
| `timelines` | `[]` | no |

- **Not written on the wire:**

| Field |
|-------|
| `attributes.<id>` for each attribute that is not syncable |

- **Write form notes:**
  - In `infiniburn` and `timelines`, a tag needs the `#` prefix.
  - An empty `timelines` list compares equal to the default `[]`, so the field is not written.
  - The fields `monster_spawn_light_level` and `monster_spawn_block_light_limit` are written in the dimension type object itself.
  - `monster_spawn_light_level` is an int provider (REG-FLD-217) with a range of 0 to 15.
  - `default_clock` is optional and has no default. The field is not written when it is absent.
  - The network form compares `attributes` with the default `{}` before it removes the attributes that are not syncable. Thus a dimension type with only non-syncable attributes writes `"attributes": {}` on the wire.
  - The file form accepts all attributes. It has no positional check.
  - The network form differs from the file form only in the removal of non-syncable attributes.
  - An invalid value anywhere in the object gives a decode error. The decoder does not crash.
- **Nested types:** int provider (REG-FLD-217), environment attribute map (REG-FLD-253).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-120: Registry `damage_type`

- **Reached from:** the synced registry `damage_type`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `effects` | `"hurt"` | no |
| `death_message_type` | `"default"` | no |

- **Write form notes:**
  - The network form is the same as the file form.
  - `scaling`, `effects` and `death_message_type` are enum values. They are not references.
  - `message_id` is a plain string. It is not a reference.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-121: Registry `banner_pattern`

- **Reached from:** the synced registry `banner_pattern`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The network form is the same as the file form.
  - `asset_id` is an identifier of a texture. It is not a reference.
  - `translation_key` is a translation key. It is not a reference.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-122: Registry `enchantment`

- **Reached from:** the synced registry `enchantment` (top level).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `exclusive_set` | `enchantment` | entry set | yes | no | bare value for one element |
| `effects` (map keys) | `enchantment_effect_component_type` | key only | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `exclusive_set` | `[]` | no |
| `effects` | `{}` | no |

- **Write form notes:**
  - The network form and the file form are the same. No field is left out on the wire.
  - The fields of enchantment definition (REG-FLD-350) are written in this object. They have no wrapper key.
  - `description` holds a text component (REG-FLD-429). It is not a reference.
  - An explicit `exclusive_set` value `[]` equals the default, so it is not written. A tag value (`#name`) never equals the default, so it is always written.
  - An empty `effects` map equals the default, so it is not written.
  - The `effects` map can drop a component type that is not persistent. All enchantment effect component types are persistent, so the map drops no entry.
  - The registry `enchantment_provider` is not synced. No field of this registry refers to it.
- **Nested types:** enchantment definition (REG-FLD-350), enchantment effect map (REG-FLD-351), text component (REG-FLD-429).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-123: Registry `jukebox_song`

- **Reached from:** the synced registry `jukebox_song`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `sound_event` | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The network form is the same as the file form.
  - The write writes `sound_event` as a key for a reference and as an object for an inline value.
  - `description` is a text component (REG-FLD-429).
- **Nested types:** sound event (REG-FLD-302), text component (REG-FLD-429).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-124: Registry `instrument`

- **Reached from:** the synced registry `instrument`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `sound_event` | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `durability_damage` | `0` | no |

- **Write form notes:**
  - The network form is the same as the file form.
  - The write writes `sound_event` as a key for a reference and as an object for an inline value.
  - `description` is a text component (REG-FLD-429).
- **Nested types:** sound event (REG-FLD-302), text component (REG-FLD-429).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-125: Registry `test_environment`

- **Reached from:** the synced registry `test_environment`; test data (REG-FLD-428) field `environment`; test environment `all_of` (REG-FLD-419) field `definitions`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `test_environment_definition_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The file form and the network form are the same. No field is left out on the wire.
  - The type ids are `all_of`, `clock_time`, `difficulty`, `function`, `game_rules`, `timeline_attributes` and `weather`.
  - A field of another type that holds a test environment accepts a key or an inline value. A registered entry is written as its key. An inline value is written as an object.
- **Nested types:** test environment `all_of` (REG-FLD-419), test environment `clock_time` (REG-FLD-420), test environment `difficulty` (REG-FLD-421), test environment `function` (REG-FLD-422), test environment `game_rules` (REG-FLD-423), test environment `timeline_attributes` (REG-FLD-424), test environment `weather` (REG-FLD-425).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-126: Registry `test_instance`

- **Reached from:** the synced registry `test_instance`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `test_instance_type` | key only | yes | — | — |

- **Fields left out on write:** none at this level. See test data (REG-FLD-428).
- **Write form notes:**
  - The file form and the network form are the same. No field is left out on the wire.
  - The type ids are `block_based` and `function`.
- **Nested types:** test instance `block_based` (REG-FLD-427), test instance `function` (REG-FLD-426).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-127: Registry `dialog`

- **Reached from:** the synced registry `dialog`; click event (REG-FLD-440) action `show_dialog` field `dialog`; dialog `dialog_list` (REG-FLD-405) field `dialogs`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `dialog_type` | key only | yes | — | — |

- **Fields left out on write:** none at this level. See dialog common fields (REG-FLD-400) and each dialog type.
- **Write form notes:**
  - The file form and the network form are the same. No field is left out on the wire.
  - The type ids are `notice`, `confirmation`, `multi_action`, `server_links` and `dialog_list`.
  - A single field of another type that holds a dialog accepts a key or an inline value. A registered entry is written as its key. An inline value is written as an object.
  - A list field of another type that holds dialogs is an entry set. It accepts a tag with `#`, one key, or a list of keys and inline values. A list with one element is written as a bare value.
- **Nested types:** dialog common fields (REG-FLD-400), dialog `notice` (REG-FLD-401), dialog `confirmation` (REG-FLD-402), dialog `multi_action` (REG-FLD-403), dialog `server_links` (REG-FLD-404), dialog `dialog_list` (REG-FLD-405), text component (REG-FLD-429).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-128: Registry `world_clock`

- **Reached from:** the synced registry `world_clock`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - An entry has no fields. It is written as the empty object `{}`.
  - The network form is the same as the file form.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-129: Registry `timeline`

- **Reached from:** the synced registry `timeline`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `clock` | `world_clock` | key only | yes | — | — |
| `tracks` (map keys) | `environment_attribute` | key only | yes | — | — |
| `time_markers` (map keys) | `clock_time_marker` | key only | no | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `tracks` | `{}` | no |
| `time_markers` | `{}` | no |

- **Not written on the wire:**

| Field |
|-------|
| `tracks.<id>` for each attribute that is not syncable |

- **Write form notes:**
  - `clock_time_marker` is a key namespace only. No registry with this id exists, so the keys are not checked.
  - `period_ticks` is optional and has no default. The field is not written when it is absent.
  - The network form removes the non-syncable tracks before it compares `tracks` with the default. Thus `tracks` is not written when all tracks are non-syncable.
  - This behaviour differs from `attributes` in Registry `worldgen/biome` (REG-FLD-101) and Registry `dimension_type` (REG-FLD-119).
- **Nested types:** timeline time marker (REG-FLD-251), attribute track (REG-FLD-252).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-130: Registry `decorated_pot_pattern`

- **Reached from:** the synced registry `decorated_pot_pattern`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The network form is the same as the file form.
  - `asset_id` is an identifier of a texture. It is not a reference.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-131: Registry `block_transformer`

- **Reached from:** the synced registry `block_transformer`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The network form is the same as the file form.
  - The top-level value of an entry is a list, not an object.
  - The list has 1 to 200 elements. Each element is a block transform data (REG-FLD-301).
  - The write always writes a list, also for one element.
- **Nested types:** block transform data (REG-FLD-301).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-132: Registry `worldgen/block_state_provider`

- **Reached from:** the synced registry `worldgen/block_state_provider`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The network form is the same as the file form.
  - The top-level value of an entry is an inline value of a block state provider (REG-FLD-304). It cannot be a key.
  - The inline value is either a typed object with `type`, or a full-form block state (REG-FLD-303) object. block state provider (REG-FLD-304) gives the two forms.
- **Nested types:** block state provider (REG-FLD-304).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-133: Registry `block_sound_set`

- **Reached from:** the synced registry `block_sound_set`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `break_sound` | `sound_event` | reference or inline | yes | — | — |
| `step_sound` | `sound_event` | reference or inline | yes | — | — |
| `place_sound` | `sound_event` | reference or inline | yes | — | — |
| `hit_sound` | `sound_event` | reference or inline | yes | — | — |
| `fall_sound` | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `volume` | `1.0` | no |
| `pitch` | `1.0` | no |

- **Write form notes:**
  - The network form is the same as the file form.
  - Each sound field is optional and has no default. An absent sound field is not written.
- **Nested types:** sound event (REG-FLD-302).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

## 3. Mob variant types and value providers

This chapter gives the value types inside the mob variant registries, `painting_variant` and `sulfur_cube_archetype`. It also gives the int provider and the float provider, which many other types use.

### REG-FLD-200: wolf asset info

- **Reached from:** Registry `wolf_variant` (REG-FLD-105) `assets` and `baby_assets`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `wild`, `tame` and `angry` are required identifiers of a texture. They are not references.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-201: wolf sound set

- **Reached from:** Registry `wolf_sound_variant` (REG-FLD-106) `adult_sounds` and `baby_sounds`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `ambient_sound` | `sound_event` | reference or inline | yes | — | — |
| `death_sound` | `sound_event` | reference or inline | yes | — | — |
| `growl_sound` | `sound_event` | reference or inline | yes | — | — |
| `hurt_sound` | `sound_event` | reference or inline | yes | — | — |
| `pant_sound` | `sound_event` | reference or inline | yes | — | — |
| `whine_sound` | `sound_event` | reference or inline | yes | — | — |
| `step_sound` | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - Each sound field is optional and has no default. An absent sound field is not written.
- **Nested types:** sound event (REG-FLD-302).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-202: model and texture

- **Reached from:** Registry `pig_variant` (REG-FLD-107), Registry `cow_variant` (REG-FLD-112), Registry `chicken_variant` (REG-FLD-114), Registry `zombie_nautilus_variant` (REG-FLD-116).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `model` | `"normal"` for pig, cow, chicken and zombie nautilus | no |

- **Write form notes:**
  - These fields are written in the parent object.
  - `model` is an enum value. The parent rule gives the permitted values.
  - `asset_id` is a required identifier of a texture. It is not a reference.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-203: pig sound set

- **Reached from:** Registry `pig_sound_variant` (REG-FLD-108) `adult_sounds` and `baby_sounds`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `ambient_sound` | `sound_event` | reference or inline | yes | — | — |
| `hurt_sound` | `sound_event` | reference or inline | yes | — | — |
| `death_sound` | `sound_event` | reference or inline | yes | — | — |
| `step_sound` | `sound_event` | reference or inline | yes | — | — |
| `eat_sound` | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - Each sound field is optional and has no default. An absent sound field is not written.
- **Nested types:** sound event (REG-FLD-302).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-204: cat sound set

- **Reached from:** Registry `cat_sound_variant` (REG-FLD-111) `adult_sounds` and `baby_sounds`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `ambient_sound` | `sound_event` | reference or inline | yes | — | — |
| `stray_ambient_sound` | `sound_event` | reference or inline | yes | — | — |
| `hiss_sound` | `sound_event` | reference or inline | yes | — | — |
| `hurt_sound` | `sound_event` | reference or inline | yes | — | — |
| `death_sound` | `sound_event` | reference or inline | yes | — | — |
| `eat_sound` | `sound_event` | reference or inline | yes | — | — |
| `beg_for_food_sound` | `sound_event` | reference or inline | yes | — | — |
| `purr_sound` | `sound_event` | reference or inline | yes | — | — |
| `purreow_sound` | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - Each sound field is optional and has no default. An absent sound field is not written.
- **Nested types:** sound event (REG-FLD-302).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-205: chicken sound set

- **Reached from:** Registry `chicken_sound_variant` (REG-FLD-115) `adult_sounds` and `baby_sounds`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `ambient_sound` | `sound_event` | reference or inline | yes | — | — |
| `hurt_sound` | `sound_event` | reference or inline | yes | — | — |
| `death_sound` | `sound_event` | reference or inline | yes | — | — |
| `step_sound` | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - Each sound field is optional and has no default. An absent sound field is not written.
- **Nested types:** sound event (REG-FLD-302).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-206: sulfur cube attribute entry

- **Reached from:** Registry `sulfur_cube_archetype` (REG-FLD-118) `attribute_modifiers[]`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `attribute` | `attribute` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `id`, `amount` and `operation` are written in this object. All three fields are required.
  - `id` is an identifier of the attribute modifier. It is not a reference.
  - `amount` is a double. `operation` is one of `add_value`, `add_multiplied_base` or `add_multiplied_total`.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-207: sulfur cube explosion

- **Reached from:** Registry `sulfur_cube_archetype` (REG-FLD-118) `explosion`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `power` is a required integer, 0 or more. `causes_fire` is a required boolean. `fuse` is a required integer, 1 or more.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-208: sulfur cube contact damage

- **Reached from:** Registry `sulfur_cube_archetype` (REG-FLD-118) `contact_damage`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `damage_type` | `damage_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `amount` is a required float provider (REG-FLD-224). Its value is 0 or more.
  - `attribute_to_source` is a required boolean.
- **Nested types:** float provider (REG-FLD-224) (`amount`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-209: sulfur cube knockback modifiers

- **Reached from:** Registry `sulfur_cube_archetype` (REG-FLD-118) `knockback_modifiers`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `horizontal_power` and `vertical_power` are required floats.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-210: sulfur cube sound settings

- **Reached from:** Registry `sulfur_cube_archetype` (REG-FLD-118) `sound_settings`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `hit_sound` | `sound_event` | reference or inline | yes | — | — |
| `push_sound` | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `hit_sound` and `push_sound` are optional and have no default. An absent sound field is not written.
  - `push_sound_impulse_threshold` and `push_sound_cooldown` are required floats.
- **Nested types:** sound event (REG-FLD-302).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-211: spawn prioritized selectors

- **Reached from:** the `spawn_conditions` field of Registry `wolf_variant` (REG-FLD-105), Registry `pig_variant` (REG-FLD-107), Registry `frog_variant` (REG-FLD-109), Registry `cat_variant` (REG-FLD-110), Registry `cow_variant` (REG-FLD-112), Registry `chicken_variant` (REG-FLD-114) and Registry `zombie_nautilus_variant` (REG-FLD-116). Only the file form has this field.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is always a list of spawn selector (REG-FLD-212).
- **Nested types:** spawn selector (REG-FLD-212).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-212: spawn selector

- **Reached from:** spawn prioritized selectors (REG-FLD-211) `[]`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `condition` is optional and has no default. An absent `condition` is not written.
  - `priority` is a required integer.
- **Nested types:** spawn condition (REG-FLD-213).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-213: spawn condition

- **Reached from:** spawn selector (REG-FLD-212) `condition`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `spawn_condition_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The type ids are `structure`, `moon_brightness` and `biome`.
- **Nested types:** spawn condition `biome` (REG-FLD-214), spawn condition `structure` (REG-FLD-215), spawn condition `moon_brightness` (REG-FLD-216).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-214: spawn condition `biome`

- **Reached from:** spawn condition (REG-FLD-213).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `biomes` | `worldgen/biome` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Write form notes:**
  - A tag in `biomes` requires the `#` prefix.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-215: spawn condition `structure`

- **Reached from:** spawn condition (REG-FLD-213).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `structures` | `worldgen/structure` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Write form notes:**
  - A tag in `structures` requires the `#` prefix.
  - `worldgen/structure` is not a synced registry. This field occurs only in the file form, so the client never receives it.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-216: spawn condition `moon_brightness`

- **Reached from:** spawn condition (REG-FLD-213).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `range` is a required range of doubles. It is a bare number or an object with optional `min` and `max`.
  - `min` and `max` have no default. An absent bound is not written.
  - The encoder writes a range with equal bounds as a bare number.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-217: int provider

- **Reached from:** types in other chapters. No type in this chapter uses it.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `int_provider_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a bare integer or an object with a `type` field.
  - The encoder always writes a `constant` provider as a bare integer. It never writes the object form of `constant`.
  - The encoder always writes every other type as an object with a `type` field.
  - Some parents set a minimum or a maximum. These bounds change only the validation, not the form.
  - The type ids are `constant`, `uniform`, `biased_to_bottom`, `very_biased_to_bottom`, `clamped`, `weighted_list`, `clamped_normal` and `trapezoid`.
- **Nested types:** int provider `constant` (REG-FLD-218), int provider `uniform`, `biased_to_bottom` and `very_biased_to_bottom` (REG-FLD-219), int provider `clamped` (REG-FLD-220), int provider `weighted_list` (REG-FLD-221), int provider `clamped_normal` (REG-FLD-222), int provider `trapezoid` (REG-FLD-223).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-218: int provider `constant`

- **Reached from:** int provider (REG-FLD-217).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `value` is a required integer.
  - The reader accepts this object form. The encoder never writes it; see int provider (REG-FLD-217).
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-219: int provider `uniform`, `biased_to_bottom` and `very_biased_to_bottom`

- **Reached from:** int provider (REG-FLD-217).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `min_inclusive` and `max_inclusive` are required integers.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-220: int provider `clamped`

- **Reached from:** int provider (REG-FLD-217).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `source` is a required int provider (REG-FLD-217).
  - `min_inclusive` and `max_inclusive` are required integers.
- **Nested types:** int provider (REG-FLD-217) (`source`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-221: int provider `weighted_list`

- **Reached from:** int provider (REG-FLD-217).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `distribution` is required. It is always a list and it must not be empty.
  - Each list item is an object with a required `data` and a required `weight`.
  - `data` is an int provider (REG-FLD-217). `weight` is an integer, 0 or more.
- **Nested types:** int provider (REG-FLD-217) (`distribution[].data`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-222: int provider `clamped_normal`

- **Reached from:** int provider (REG-FLD-217).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `mean` and `deviation` are required floats.
  - `min_inclusive` and `max_inclusive` are required integers.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-223: int provider `trapezoid`

- **Reached from:** int provider (REG-FLD-217).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `min`, `max` and `plateau` are required integers.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-224: float provider

- **Reached from:** sulfur cube contact damage (REG-FLD-208) `amount`. Other chapters also reach this type.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `float_provider_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a bare float or an object with a `type` field.
  - The encoder always writes a `constant` provider as a bare float. It never writes the object form of `constant`.
  - The encoder always writes every other type as an object with a `type` field.
  - Some parents set a minimum or a maximum. These bounds change only the validation, not the form.
  - The type ids are `constant`, `uniform`, `clamped_normal` and `trapezoid`.
- **Nested types:** float provider `constant` (REG-FLD-225), float provider `uniform` (REG-FLD-226), float provider `clamped_normal` (REG-FLD-227), float provider `trapezoid` (REG-FLD-228).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-225: float provider `constant`

- **Reached from:** float provider (REG-FLD-224).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `value` is a required float.
  - The reader accepts this object form. The encoder never writes it; see float provider (REG-FLD-224).
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-226: float provider `uniform`

- **Reached from:** float provider (REG-FLD-224).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `min_inclusive` and `max_exclusive` are required floats.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-227: float provider `clamped_normal`

- **Reached from:** float provider (REG-FLD-224).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `mean`, `deviation`, `min` and `max` are required floats.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-228: float provider `trapezoid`

- **Reached from:** float provider (REG-FLD-224).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `min`, `max` and `plateau` are required floats.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

## 4. Biome, dimension type and timeline types, environment attributes and particle options

This chapter gives the value types inside biomes, dimension types and timelines. It also gives the environment attribute values and the particle options. Each rule lists the references and the fields that are left out on write.

### REG-FLD-250: biome special effects

- **Reached from:** Registry `worldgen/biome` (REG-FLD-101) `effects`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `grass_color_modifier` | `"none"` | no |

- **Write form notes:**
  - `water_color` is required.
  - `foliage_color`, `dry_foliage_color` and `grass_color` are optional and have no default. A field is not written when it is absent.
  - Each color is written as a `"#RRGGBB"` string. A reader also accepts an int or a list of three floats `[r, g, b]`.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-251: timeline time marker

- **Reached from:** Registry `timeline` (REG-FLD-129) `time_markers.<key>`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `show_in_commands` | `false` | no |

- **Write form notes:**
  - When `show_in_commands` is `false`, the marker is written as a bare non-negative int. The int is the value of `ticks`.
  - When `show_in_commands` is `true`, the marker is written as `{ "ticks": N, "show_in_commands": true }`.
  - A reader accepts both forms.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-252: attribute track

- **Reached from:** Registry `timeline` (REG-FLD-129) `tracks.<environment attribute id>`.
- **References:** none. The references inside `keyframes[].value` are in environment attribute value (REG-FLD-254).
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `modifier` | `"override"` | no |
| `ease` | `"linear"` | no |

- **Write form notes:**
  - `modifier` is a modifier id string. It is not a reference. The permitted ids are the modifier ids of the attribute type and `override`.
  - Here `modifier` is optional. In environment attribute map (REG-FLD-253), the `modifier` field is required.
  - `keyframes` is required and is always a list. The list is not empty and is in order of `ticks`.
  - Each keyframe is `{ "ticks": <int, 0 or more>, "value": <argument> }`. The `value` is an environment attribute modifier argument (REG-FLD-255) for the `modifier`.
  - `ease` is a simple easing name string or `{ "cubic_bezier": [x1, y1, x2, y2] }`.
- **Nested types:** environment attribute modifier argument (REG-FLD-255), environment attribute value (REG-FLD-254).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-253: environment attribute map

- **Reached from:** Registry `worldgen/biome` (REG-FLD-101) `attributes`, Registry `dimension_type` (REG-FLD-119) `attributes`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `<key>` (map key) | `environment_attribute` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - An entry with the modifier `override` is written as the bare environment attribute value (REG-FLD-254). The form `{ "modifier": "override", ... }` is never written.
  - An entry with another modifier is written as `{ "modifier": "<modifier id>", "argument": <argument> }`. Both fields are required.
  - A reader tries the bare value form first.
  - A reader also accepts `{ "modifier": "override", "argument": <value> }`. It gives the same value as the bare form.
  - Only syncable attributes are written on the wire. The table below gives the attribute ids.
  - "Positional: no" means that the biome file form rejects the attribute.
  - Only the attribute types `particle`, `ambient_particles`, `background_music` and `ambient_sounds` have references or default fields on the wire.
  - All attributes of the types `bed_rule`, `activity`, `mob_spawn_settings` and `tri_state` are not syncable. They occur only in files.

| Attribute id | Attribute type | Syncable | Positional |
|--------------|----------------|----------|------------|
| `visual/fog_color` | `rgb_color` | yes | yes |
| `visual/fog_start_distance` | `float` | yes | yes |
| `visual/fog_end_distance` | `float` | yes | yes |
| `visual/sky_fog_end_distance` | `float` | yes | yes |
| `visual/cloud_fog_end_distance` | `float` | yes | yes |
| `visual/water_fog_color` | `rgb_color` | yes | yes |
| `visual/water_fog_start_distance` | `float` | yes | yes |
| `visual/water_fog_end_distance` | `float` | yes | yes |
| `visual/sky_color` | `rgb_color` | yes | yes |
| `visual/sunrise_sunset_color` | `argb_color` | yes | yes |
| `visual/cloud_color` | `argb_color` | yes | yes |
| `visual/cloud_height` | `float` | yes | yes |
| `visual/sun_angle` | `angle_degrees` | yes | yes |
| `visual/moon_angle` | `angle_degrees` | yes | yes |
| `visual/star_angle` | `angle_degrees` | yes | yes |
| `visual/moon_phase` | `moon_phase` | yes | yes |
| `visual/star_brightness` | `float` | yes | yes |
| `visual/has_sky_occluder` | `boolean` | yes | yes |
| `visual/block_light_tint` | `rgb_color` | yes | yes |
| `visual/sky_light_color` | `rgb_color` | yes | yes |
| `visual/sky_light_factor` | `float` | yes | yes |
| `visual/night_vision_color` | `rgb_color` | yes | yes |
| `visual/ambient_light_color` | `rgb_color` | yes | yes |
| `visual/default_dripstone_particle` | `particle` | yes | yes |
| `visual/ambient_particles` | `ambient_particles` | yes | yes |
| `audio/background_music` | `background_music` | yes | yes |
| `audio/music_volume` | `float` | yes | yes |
| `audio/ambient_sounds` | `ambient_sounds` | yes | yes |
| `audio/firefly_bush_sounds` | `boolean` | yes | yes |
| `gameplay/sky_light_level` | `float` | yes | no |
| `gameplay/can_start_raid` | `boolean` | no | yes |
| `gameplay/water_evaporates` | `boolean` | yes | yes |
| `gameplay/bed_rule` | `bed_rule` | no | yes |
| `gameplay/straw_bed_rule` | `bed_rule` | no | yes |
| `gameplay/respawn_anchor_works` | `boolean` | no | yes |
| `gameplay/nether_portal_spawns_piglin` | `boolean` | no | yes |
| `gameplay/fast_lava` | `boolean` | yes | no |
| `gameplay/increased_fire_burnout` | `boolean` | no | yes |
| `gameplay/eyeblossom_open` | `tri_state` | no | yes |
| `gameplay/turtle_egg_hatch_chance` | `float` | no | yes |
| `gameplay/piglins_zombify` | `boolean` | yes | yes |
| `gameplay/snow_golem_melts` | `boolean` | no | yes |
| `gameplay/creaking_active` | `boolean` | yes | yes |
| `gameplay/surface_slime_spawn_chance` | `float` | no | yes |
| `gameplay/cat_waking_up_gift_chance` | `float` | no | yes |
| `gameplay/bees_stay_in_hive` | `boolean` | no | yes |
| `gameplay/monsters_burn` | `boolean` | no | yes |
| `gameplay/can_pillager_patrol_spawn` | `boolean` | no | yes |
| `gameplay/natural_mob_spawns` | `mob_spawn_settings` | no | yes |
| `gameplay/creature_world_gen_spawn_probability` | `float` | no | yes |
| `gameplay/villager_activity` | `activity` | no | yes |
| `gameplay/baby_villager_activity` | `activity` | no | yes |

- **Nested types:** environment attribute value (REG-FLD-254), environment attribute modifier argument (REG-FLD-255).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-254: environment attribute value

- **Reached from:** environment attribute map (REG-FLD-253) `<key>` (the bare value or `argument`), attribute track (REG-FLD-252) `keyframes[].value`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| type `activity`: the value | `activity` | key only | yes | — | — |
| type `particle`: `type` in the value | `particle_type` | key only | yes | — | — |
| type `ambient_particles`: `[].particle.type` | `particle_type` | key only | yes | — | — |
| type `background_music`: `default.sound`, `creative.sound`, `underwater.sound` | `sound_event` | reference or inline | yes | — | — |
| type `ambient_sounds`: `loop` | `sound_event` | reference or inline | yes | — | — |
| type `ambient_sounds`: `mood.sound` | `sound_event` | reference or inline | yes | — | — |
| type `ambient_sounds`: `additions[].sound` (or `additions.sound` for a bare element) | `sound_event` | reference or inline | yes | — | — |
| type `mob_spawn_settings`: `spawns_by_category.<category>[].type` | `entity_type` | key only | yes | — | — |
| type `mob_spawn_settings`: `spawn_costs` (map keys) | `entity_type` | key only | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| type `background_music`: `default.replace_current_music`, `creative.replace_current_music`, `underwater.replace_current_music` | `false` | no |
| type `ambient_sounds`: `additions` | `[]` | no |
| type `bed_rule` (files only): `destroy_on_use` | `false` | no |
| type `bed_rule` (files only): `destroy_on_leave` | `false` | no |

- **Write form notes:**
  - The attribute type is fixed for each attribute id. The JSON value does not contain the attribute type.
  - The `particle` value and each `ambient_particles[].particle` are particle options (REG-FLD-256) objects.
  - `boolean` is a JSON boolean. `float` and `angle_degrees` are numbers. `integer` is an int, and no attribute uses it.
  - `tri_state` is a boolean or one of the strings `"true"`, `"false"` and `"default"`.
  - `moon_phase` is an enum string.
  - `rgb_color` is written as a `"#RRGGBB"` string. A reader also accepts an int or a list of three floats `[r, g, b]`.
  - `argb_color` is written as a `"#AARRGGBB"` string. A reader also accepts an int or a list of four floats `[r, g, b, a]`.
  - `ambient_particles` is always a list. Each item is `{ "particle": <particle options>, "probability": <0 to 1> }`.
  - `background_music` is `{ "default", "creative", "underwater" }`. Each field is optional and has no default.
  - Each `background_music` field is a music object `{ "sound", "min_delay", "max_delay", "replace_current_music" }`.
  - `ambient_sounds` is `{ "loop", "mood", "additions" }`, and each field is optional.
  - `mood` is `{ "sound", "tick_delay", "block_search_extent", "offset" }`.
  - `additions` is written as a bare object for one element and as a list for other counts. Each item is `{ "sound", "tick_chance" }`.
  - `bed_rule` occurs only in files. It is `{ "can_sleep", "can_set_spawn", "destroy_on_use", "destroy_on_leave", "error_message" }`.
  - In `bed_rule`, `can_sleep` and `can_set_spawn` are `"always"`, `"when_dark"` or `"never"`.
  - In `bed_rule`, `error_message` is an optional text component (REG-FLD-429) without a default.
  - `mob_spawn_settings` occurs only in files. It is `{ "spawns_by_category": { <mob category>: [ { "type", "count", "weight" } ] }, "spawn_costs": { <entity id>: { "energy_budget", "charge" } } }`.
  - In `mob_spawn_settings`, `spawns_by_category`, `spawn_costs` and `weight` are required. `count` is an int provider (REG-FLD-217).
  - An attribute default is not a field default. Thus the attribute value itself is never left out on write.
- **Nested types:** particle options (REG-FLD-256), sound event (REG-FLD-302), text component (REG-FLD-429), int provider (REG-FLD-217).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-255: environment attribute modifier argument

- **Reached from:** environment attribute map (REG-FLD-253) `<key>.argument`, attribute track (REG-FLD-252) `keyframes[].value`.
- **References:** none other than the references in environment attribute value (REG-FLD-254). The modifiers `override`, `append` and `overlay` use the attribute value form.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| float `alpha_blend` argument: `alpha` | `1.0` | no |

- **Write form notes:**
  - The modifier ids are plain strings. They are not references.
  - The modifier ids for `boolean` are `and`, `nand`, `or`, `nor`, `xor` and `xnor`.
  - The modifier ids for `float` and `angle_degrees` are `alpha_blend`, `add`, `subtract`, `multiply`, `minimum` and `maximum`.
  - The modifier ids for `rgb_color` and `argb_color` are `alpha_blend`, `add`, `subtract`, `multiply` and `blend_to_gray`.
  - The modifier ids for `integer` are `add`, `subtract`, `multiply`, `minimum` and `maximum`.
  - The modifier id for `ambient_particles` is `append`. The modifier id for `mob_spawn_settings` is `overlay`.
  - All attribute types accept the modifier id `override`.
  - The float `alpha_blend` argument is a number or `{ "value", "alpha" }`. When `alpha` is `1.0`, the argument is written as a bare number.
  - The `boolean` modifier arguments are booleans. The simple `float` modifier arguments are numbers.
  - The color `alpha_blend` argument (RGB and ARGB) and the ARGB `multiply` argument are ARGB colors in the `"#AARRGGBB"` form.
  - The color `add` and `subtract` arguments and the RGB `multiply` argument are RGB colors in the `"#RRGGBB"` form.
  - The `blend_to_gray` argument is `{ "brightness": <0 to 1>, "factor": <0 to 1> }`. Both fields are required.
- **Nested types:** environment attribute value (REG-FLD-254).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-256: particle options

- **Reached from:** environment attribute value (REG-FLD-254) (attribute `visual/default_dripstone_particle` and `visual/ambient_particles[].particle`), explosion particle info (REG-FLD-270) `particle`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `particle_type` | key only | yes | — | — |

- **Fields left out on write:** none at this level.
- **Write form notes:**
  - The option fields of each type id are written in the same object as `type`. The rules for each type id follow.
  - These type ids have no option fields. Their object is `{ "type": "<id>" }`: `angry_villager`, `bubble`, `sulfur_bubbles`, `noxious_gas`, `noxious_gas_cloud`, `cloud`, `copper_fire_flame`, `crit`, `damage_indicator`, `dripping_lava`, `falling_lava`, `landing_lava`, `dripping_water`, `falling_water`, `elder_guardian`, `enchanted_hit`, `enchant`, `end_rod`, `explosion_emitter`, `explosion`, `freezing`, `gust`, `small_gust`, `gust_emitter_large`, `gust_emitter_small`, `sonic_boom`, `firework`, `fishing`, `flame`, `infested`, `cherry_leaves`, `pale_oak_leaves`, `red_poplar_leaves`, `orange_poplar_leaves`, `yellow_poplar_leaves`, `sculk_soul`, `sculk_charge_pop`, `soul_fire_flame`, `soul`, `happy_villager`, `composter`, `heart`, `pause_mob_growth`, `reset_mob_growth`, `item_slime`, `item_cobweb`, `large_smoke`, `lava`, `mycelium`, `note`, `poof`, `portal`, `rain`, `smoke`, `white_smoke`, `sneeze`, `spit`, `squid_ink`, `sweep_attack`, `totem_of_undying`, `underwater`, `splash`, `witch`, `bubble_pop`, `current_down`, `bubble_column_up`, `nautilus`, `dolphin`, `campfire_cosy_smoke`, `campfire_signal_smoke`, `dripping_honey`, `falling_honey`, `landing_honey`, `falling_nectar`, `falling_spore_blossom`, `ash`, `crimson_spore`, `warped_spore`, `spore_blossom_air`, `dripping_obsidian_tear`, `falling_obsidian_tear`, `landing_obsidian_tear`, `reverse_portal`, `white_ash`, `small_flame`, `snowflake`, `dripping_dripstone_lava`, `falling_dripstone_lava`, `dripping_dripstone_water`, `falling_dripstone_water`, `glow_squid_ink`, `glow`, `wax_on`, `wax_off`, `electric_spark`, `scrape`, `egg_crack`, `dust_plume`, `trial_spawner_detection`, `trial_spawner_detection_ominous`, `vault_connection`, `ominous_spawning`, `raid_omen`, `trial_omen`, `firefly`, `sulfur_cube_goo`.
- **Nested types:** particle options `block`, `block_marker`, `falling_dust`, `dust_pillar`, `block_crumble` (REG-FLD-257), particle options `item` (REG-FLD-258), particle options `dust` (REG-FLD-259), particle options `dust_color_transition` (REG-FLD-260), particle options `effect`, `instant_effect` (REG-FLD-261), particle options `entity_effect`, `tinted_leaves`, `flash` (REG-FLD-262), particle options `dragon_breath` (REG-FLD-263), particle options `sculk_charge` (REG-FLD-264), particle options `shriek` (REG-FLD-265), particle options `trail` (REG-FLD-266), particle options `geyser`, `geyser_plume` (REG-FLD-267), particle options `geyser_base`, `geyser_poof` (REG-FLD-268), particle options `vibration` (REG-FLD-269), block state (REG-FLD-303), item stack (REG-FLD-500).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-257: particle options `block`, `block_marker`, `falling_dust`, `dust_pillar`, `block_crumble`

- **Reached from:** particle options (REG-FLD-256).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `block_state` | `block` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - A reader accepts a block state (REG-FLD-303) object or a bare block id. A bare block id gives the default state of the block.
  - The writer always writes a block state (REG-FLD-303) object.
- **Nested types:** block state (REG-FLD-303).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-258: particle options `item`

- **Reached from:** particle options (REG-FLD-256).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `item` | `item` | key only | yes | — | — |

- **Fields left out on write:** none at this level.
- **Write form notes:**
  - A reader accepts an item stack (REG-FLD-500) object or a bare item id. The write form follows item stack (REG-FLD-500).
- **Nested types:** item stack (REG-FLD-500).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-259: particle options `dust`

- **Reached from:** particle options (REG-FLD-256).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `color` and `scale` are required.
  - `color` is an RGB color. It is written as an int. A reader also accepts a list of three floats `[r, g, b]`.
  - `scale` is in the range 0.01 to 4.0.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-260: particle options `dust_color_transition`

- **Reached from:** particle options (REG-FLD-256).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `from_color`, `to_color` and `scale` are required.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-261: particle options `effect`, `instant_effect`

- **Reached from:** particle options (REG-FLD-256).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `color` | `-1` | no |
| `power` | `1.0` | no |

- **Write form notes:**
  - `color` is an RGB color. It is written as an int.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-262: particle options `entity_effect`, `tinted_leaves`, `flash`

- **Reached from:** particle options (REG-FLD-256).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `color` is required. It is an ARGB color and is written as an int.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-263: particle options `dragon_breath`

- **Reached from:** particle options (REG-FLD-256).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `power` | `1.0` | no |

- **Write form notes:**
  - The value never compares equal to the default, so the field `power` is always written. The default of `1.0` applies only on read.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-264: particle options `sculk_charge`

- **Reached from:** particle options (REG-FLD-256).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `roll` is required.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-265: particle options `shriek`

- **Reached from:** particle options (REG-FLD-256).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `delay` is required.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-266: particle options `trail`

- **Reached from:** particle options (REG-FLD-256).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `target`, `color` and `duration` are required.
  - `target` is a list of three numbers `[x, y, z]`. `color` is an RGB color. `duration` is a positive int.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-267: particle options `geyser`, `geyser_plume`

- **Reached from:** particle options (REG-FLD-256).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `water_blocks` is required. It is a positive int.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-268: particle options `geyser_base`, `geyser_poof`

- **Reached from:** particle options (REG-FLD-256).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `water_blocks` and `burst_impulse_base` are required.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-269: particle options `vibration`

- **Reached from:** particle options (REG-FLD-256).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `destination.type` | `position_source_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `destination` and `arrival_in_ticks` are required.
  - The fields of the position source are written in the `destination` object next to `type`.
  - A `destination` of type `block` has the required field `pos`. `pos` is an int array `[x, y, z]`.
  - A `destination` of type `entity` has the field `source_entity`, a UUID. Validation rejects this type.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-270: explosion particle info

- **Reached from:** explosion types outside this chapter.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `particle.type` | `particle_type` | key only | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `scaling` | `1.0` | no |
| `speed` | `1.0` | no |

- **Write form notes:**
  - `particle` is a particle options (REG-FLD-256) object.
- **Nested types:** particle options (REG-FLD-256).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

## 5. Small registry types, sound event, block state, block state provider and world generation block predicate

This chapter gives the value types inside the small synced registries. It also gives the sound event, the block state, the block state provider and the world generation block predicate, which many other types use.

### REG-FLD-300: chat type decoration

- **Reached from:** Registry `chat_type` (REG-FLD-102) `chat` and `narration`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `style` | `{}` | no |

- **Write form notes:**
  - `translation_key` is a translation key. It is not a reference.
  - `parameters` is always a list of enum values. The permitted values are `sender`, `target` and `content`.
  - `style` is a text style (REG-FLD-439). The default is the empty style.
- **Nested types:** text style (REG-FLD-439).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-301: block transform data

- **Reached from:** Registry `block_transformer` (REG-FLD-131) list elements.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `block_state_provider` | `worldgen/block_state_provider` | reference or inline | yes | — | — |
| `sound` | `sound_event` | reference or inline | yes | — | — |
| `loot` | `loot_table` | key only | no | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `particle` | `"none"` | no |
| `disallowed_faces` | `[]` | no |
| `drop_strategy` | `"from_middle"` | no |
| `update_from_neighbors` | `true` | no |
| `transform_type` | `"single_block"` | no |
| `consume_on_use` | `true` | no |
| `item_damage_per_use` | `0` | no |

- **Write form notes:**
  - `block_state_provider` is required. It holds a block state provider (REG-FLD-304).
  - `sound` and `loot` are optional and have no default. An absent field is not written.
  - `disallowed_faces` is always a list of direction enum values. The values are not references.
- **Nested types:** block state provider (REG-FLD-304), sound event (REG-FLD-302).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-302: sound event

- **Reached from:** Registry `jukebox_song` (REG-FLD-123), Registry `instrument` (REG-FLD-124), Registry `block_sound_set` (REG-FLD-133), block transform data (REG-FLD-301), and types of other chapters.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (whole value) | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - A string value is a key of `sound_event`.
  - An inline value is an object with the required field `sound_id` and the optional field `range`.
  - The write writes a key for a reference and the object for an inline value.
  - `sound_id` is an identifier of a sound asset. It is not a reference.
  - `range` is a float. It has no default. The write writes `range` only when it is present.
  - `range` is lenient. An invalid `range` value reads as absent.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-303: block state

- **Reached from:** block state provider (REG-FLD-304), block state provider `simple` (REG-FLD-305), block state provider `weighted` (REG-FLD-310), block state provider `noise` (REG-FLD-311), block state provider `dual_noise` (REG-FLD-312), block state provider `noise_threshold` (REG-FLD-313), world generation block predicate `would_survive` (REG-FLD-325), and types of other chapters.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `id` | `block` | key only | yes | — | — |
| (whole value, short form) | `block` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - A block state has two forms. The short form is a bare block key string. The full form is an object with `id` and `properties`.
  - The full form does not use the keys `Name` and `Properties`. The keys are `id` and `properties`.
  - The load accepts both forms. The short form reads as the default state of the block.
  - Where both forms are accepted, the write uses the short form for the default state of the block, and the full form for any other state.
  - Some parents accept only the full form. The parent rule states this. There, the write uses the full form also for the default state.
  - A block with only one state has no `properties` field.
  - For any other block, the full form always writes `properties`.
  - `properties` is lenient. An invalid `properties` object reads as the default state of the block.
  - The write always writes every property of the block in `properties`.
  - The load reads an absent property as its value in the default state of the block.
  - Property values are strings, integers or booleans. They are not references.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-304: block state provider

- **Reached from:** Registry `worldgen/block_state_provider` (REG-FLD-132), block transform data (REG-FLD-301) `block_state_provider`, block state provider `copy_properties` (REG-FLD-306) `source`, block state provider `randomized_int` (REG-FLD-307) `source`, block state provider `rotated` (REG-FLD-308) `state`, block state provider `rule_based` (REG-FLD-314) `fallback`, rule-based provider rule (REG-FLD-315) `then`, and types of other chapters.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (whole value) | `worldgen/block_state_provider` | reference or inline | yes | — | — |
| `type` | `worldgen/block_state_provider_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - A string value is always a key of `worldgen/block_state_provider`. The load never reads it as a block key.
  - The write writes a key for a reference and an inline value otherwise.
  - An inline value has one of two forms. The typed form is an object with `type` and the fields of that type.
  - The block state form is a full-form block state (REG-FLD-303) object with `id` and `properties`. It reads as the type `simple`.
  - The load fails when an object is valid in both forms.
  - The write always writes a `simple` provider in the block state form. It never writes `"type": "minecraft:simple"`.
  - The write never writes a `simple` provider as a short-form block key.
  - The write writes every other type in the typed form.
  - The type ids are `copy_properties`, `dual_noise`, `noise`, `noise_threshold`, `random_block`, `randomized_int`, `rotated`, `rule_based`, `simple` and `weighted`.
- **Nested types:** block state (REG-FLD-303), block state provider `simple` (REG-FLD-305), block state provider `copy_properties` (REG-FLD-306), block state provider `randomized_int` (REG-FLD-307), block state provider `rotated` (REG-FLD-308), block state provider `random_block` (REG-FLD-309), block state provider `weighted` (REG-FLD-310), block state provider `noise` (REG-FLD-311), block state provider `dual_noise` (REG-FLD-312), block state provider `noise_threshold` (REG-FLD-313), block state provider `rule_based` (REG-FLD-314).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-305: block state provider `simple`

- **Reached from:** block state provider (REG-FLD-304) with the type `simple`, on load only.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `state` is required. It holds a block state (REG-FLD-303) in the short form or the full form.
  - The write never uses this form. block state provider (REG-FLD-304) gives the write form of a `simple` provider.
- **Nested types:** block state (REG-FLD-303).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-306: block state provider `copy_properties`

- **Reached from:** block state provider (REG-FLD-304).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `source` | `worldgen/block_state_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Nested types:** block state provider (REG-FLD-304).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-307: block state provider `randomized_int`

- **Reached from:** block state provider (REG-FLD-304).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `source` | `worldgen/block_state_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `property` is the name of a block state property. It is a plain string, not a reference.
  - `values` is an int provider (REG-FLD-217).
- **Nested types:** block state provider (REG-FLD-304), int provider (REG-FLD-217).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-308: block state provider `rotated`

- **Reached from:** block state provider (REG-FLD-304).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `state` | `worldgen/block_state_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `direction` is an optional direction enum value. It has no default. The write writes it only when it is present.
  - When `direction` is absent, the game selects a random direction at each placement.
- **Nested types:** block state provider (REG-FLD-304).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-309: block state provider `random_block`

- **Reached from:** block state provider (REG-FLD-304).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `blocks` | `block` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-310: block state provider `weighted`

- **Reached from:** block state provider (REG-FLD-304).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `entries` is always a list. The list is not empty.
  - Each element is an object `{"data": <block state>, "weight": <int>}`. `data` is a block state (REG-FLD-303) in either form. `weight` is 0 or more.
- **Nested types:** block state (REG-FLD-303).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-311: block state provider `noise`

- **Reached from:** block state provider (REG-FLD-304).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `states` is always a list of block state (REG-FLD-303) values. The list is not empty.
  - `seed` is a long integer.
  - `noise` is an inline noise parameters (REG-FLD-316) object. It is not a reference to `worldgen/noise`.
  - `scale` is a positive float.
- **Nested types:** block state (REG-FLD-303), noise parameters (REG-FLD-316).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-312: block state provider `dual_noise`

- **Reached from:** block state provider (REG-FLD-304).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - This type has the fields `states`, `seed`, `noise` and `scale` of block state provider `noise` (REG-FLD-311), with the same forms.
  - `states` is always a list of block state (REG-FLD-303) values. The list is not empty.
  - `variety` is an object `{"min_inclusive": <int>, "max_inclusive": <int>}`. Each value is in the range 1 to 64.
  - `slow_noise` is an inline noise parameters (REG-FLD-316) object. It is not a reference to `worldgen/noise`.
  - `slow_scale` is a positive float.
- **Nested types:** block state (REG-FLD-303), noise parameters (REG-FLD-316).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-313: block state provider `noise_threshold`

- **Reached from:** block state provider (REG-FLD-304).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - This type has the fields `seed`, `noise` and `scale` of block state provider `noise` (REG-FLD-311), with the same forms.
  - `default_state` is a block state (REG-FLD-303) in either form.
  - `low_states` is always a list of block state (REG-FLD-303) values. The list is not empty.
  - `high_states` is always a list of block state (REG-FLD-303) values. The list is not empty.
  - `threshold` and `high_chance` are floats.
- **Nested types:** block state (REG-FLD-303), noise parameters (REG-FLD-316).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-314: block state provider `rule_based`

- **Reached from:** block state provider (REG-FLD-304).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `fallback` | `worldgen/block_state_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `fallback` is optional and has no default. An absent `fallback` is not written.
  - `rules` is always a list. Each element is a rule-based provider rule (REG-FLD-315).
- **Nested types:** block state provider (REG-FLD-304), rule-based provider rule (REG-FLD-315).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-315: rule-based provider rule

- **Reached from:** block state provider `rule_based` (REG-FLD-314) `rules` elements.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `then` | `worldgen/block_state_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `if_true` is a world generation block predicate (REG-FLD-317).
- **Nested types:** block state provider (REG-FLD-304), world generation block predicate (REG-FLD-317).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-316: noise parameters

- **Reached from:** block state provider `noise` (REG-FLD-311) `noise`, block state provider `dual_noise` (REG-FLD-312) `noise` and `slow_noise`, block state provider `noise_threshold` (REG-FLD-313) `noise`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `base_amplitude` | `1.0` | no |
| `octave_count` | `1` | no |
| `normalize` | `true` | no |
| `amplitude_modifiers` | `[]` | no |

- **Write form notes:**
  - The fields are `base_octave`, `base_amplitude`, `octave_count`, `normalize` and `amplitude_modifiers`. The older keys `firstOctave` and `amplitudes` do not apply.
  - This value is always inline. It is not a reference.
  - `normalize` is written as `true`, `false` or `"legacy"`.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-317: world generation block predicate

- **Reached from:** rule-based provider rule (REG-FLD-315) `if_true`, enchantment entity effect `replace_block` (REG-FLD-377) `predicate`, enchantment entity effect `replace_disk` (REG-FLD-378) `predicate`, world generation block predicate `not` (REG-FLD-328) `predicate`, world generation block predicate `all_of` and `any_of` (REG-FLD-327) `predicates`, world generation block predicate `volume_match` (REG-FLD-332) `match`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `block_predicate_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - This type is not the block predicate (REG-FLD-614). The two types have different fields.
  - The fields of each type are written in the same object as `type`.
  - The type ids are `matching_blocks`, `matching_block_tag`, `matching_fluids`, `matching_biomes`, `has_sturdy_face`, `solid`, `replaceable`, `would_survive`, `inside_world_bounds`, `any_of`, `all_of`, `not`, `true`, `unobstructed`, `height_range`, `volume_match` and `below_heightmap`.
  - Many types have an `offset` field. `offset` is a list of three integers `[x, y, z]`. Each integer is in the range -16 to 16.
  - The `offset` default is `[0, 0, 0]`. The write compares the offset by its three coordinates.
- **Nested types:** world generation block predicate `matching_blocks` (REG-FLD-318), world generation block predicate `matching_block_tag` (REG-FLD-319), world generation block predicate `matching_fluids` (REG-FLD-320), world generation block predicate `matching_biomes` (REG-FLD-321), world generation block predicate `has_sturdy_face` (REG-FLD-322), world generation block predicate `solid` (REG-FLD-323), world generation block predicate `replaceable` (REG-FLD-324), world generation block predicate `would_survive` (REG-FLD-325), world generation block predicate `inside_world_bounds` (REG-FLD-326), world generation block predicate `all_of` and `any_of` (REG-FLD-327), world generation block predicate `not` (REG-FLD-328), world generation block predicate `true` (REG-FLD-329), world generation block predicate `unobstructed` (REG-FLD-330), world generation block predicate `height_range` (REG-FLD-331), world generation block predicate `volume_match` (REG-FLD-332), world generation block predicate `below_heightmap` (REG-FLD-333).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-318: world generation block predicate `matching_blocks`

- **Reached from:** world generation block predicate (REG-FLD-317).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `blocks` | `block` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `offset` | `[0, 0, 0]` | no |

- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-319: world generation block predicate `matching_block_tag`

- **Reached from:** world generation block predicate (REG-FLD-317).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `tag` | `block` | tag key | no | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `offset` | `[0, 0, 0]` | no |

- **Write form notes:**
  - `tag` is a tag id without `#`. The load does not accept a `#` prefix. The write does not write `#`.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-320: world generation block predicate `matching_fluids`

- **Reached from:** world generation block predicate (REG-FLD-317).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `fluids` | `fluid` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `offset` | `[0, 0, 0]` | no |
| `state` | `{}` | no |

- **Write form notes:**
  - `state` is a state properties predicate (REG-FLD-615). It holds property names and value matchers. It holds no references.
- **Nested types:** state properties predicate (REG-FLD-615).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-321: world generation block predicate `matching_biomes`

- **Reached from:** world generation block predicate (REG-FLD-317).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `biomes` | `worldgen/biome` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Write form notes:**
  - This type has no `offset` field.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-322: world generation block predicate `has_sturdy_face`

- **Reached from:** world generation block predicate (REG-FLD-317).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `offset` | `[0, 0, 0]` | no |

- **Write form notes:**
  - `direction` is a direction enum value. It is not a reference.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-323: world generation block predicate `solid`

- **Reached from:** world generation block predicate (REG-FLD-317).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `offset` | `[0, 0, 0]` | no |

- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-324: world generation block predicate `replaceable`

- **Reached from:** world generation block predicate (REG-FLD-317).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `offset` | `[0, 0, 0]` | no |

- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-325: world generation block predicate `would_survive`

- **Reached from:** world generation block predicate (REG-FLD-317).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `offset` | `[0, 0, 0]` | no |

- **Write form notes:**
  - `state` is required. It is a block state (REG-FLD-303) in either form. The full form uses `id`, not `Name`.
- **Nested types:** block state (REG-FLD-303).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-326: world generation block predicate `inside_world_bounds`

- **Reached from:** world generation block predicate (REG-FLD-317).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `offset` | `[0, 0, 0]` | no |

- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-327: world generation block predicate `all_of` and `any_of`

- **Reached from:** world generation block predicate (REG-FLD-317).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - This rule applies to the type ids `all_of` and `any_of`. The two types have the same fields.
  - `predicates` is always a list of world generation block predicate (REG-FLD-317) values, also for one element.
- **Nested types:** world generation block predicate (REG-FLD-317).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-328: world generation block predicate `not`

- **Reached from:** world generation block predicate (REG-FLD-317).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `predicate` is required. It is a world generation block predicate (REG-FLD-317).
- **Nested types:** world generation block predicate (REG-FLD-317).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-329: world generation block predicate `true`

- **Reached from:** world generation block predicate (REG-FLD-317).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - This type has no fields. The object holds only `type`.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-330: world generation block predicate `unobstructed`

- **Reached from:** world generation block predicate (REG-FLD-317).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `offset` | `[0, 0, 0]` | no |

- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-331: world generation block predicate `height_range`

- **Reached from:** world generation block predicate (REG-FLD-317).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - This type has no `offset` field.
  - `min_inclusive` and `max_inclusive` are required. Each is a vertical anchor.
  - A vertical anchor is an object with one key: `absolute`, `above_bottom` or `below_top`. The value is an integer. A vertical anchor holds no references.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-332: world generation block predicate `volume_match`

- **Reached from:** world generation block predicate (REG-FLD-317).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `min` and `max` are required offsets. Each is a list of three integers.
  - `match` is required. It is a world generation block predicate (REG-FLD-317).
- **Nested types:** world generation block predicate (REG-FLD-317).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-333: world generation block predicate `below_heightmap`

- **Reached from:** world generation block predicate (REG-FLD-317).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - This type has no `offset` field.
  - `heightmap` is a required heightmap enum value. It is not a reference.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

## 6. Enchantment types

This chapter gives the reference fields and the default rules for the value types inside an enchantment. The effect types also occur in other places, so other rules link to them.

### REG-FLD-350: enchantment definition

- **Reached from:** Registry `enchantment` (REG-FLD-122). These fields are written in the parent object. They have no wrapper key.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `supported_items` | `item` | entry set | yes | no | bare value for one element |
| `primary_items` | `item` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Write form notes:**
  - `weight`, `max_level`, `min_cost`, `max_cost`, `anvil_cost` and `slots` are required. They are always written.
  - `primary_items` is optional and has no default. It is written only when it is present.
  - `slots` is always a list of equipment slot group strings.
  - `min_cost` and `max_cost` are objects with two required integers: `base` and `per_level_above_first`.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-351: enchantment effect map

- **Reached from:** Registry `enchantment` (REG-FLD-122) `effects`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `trident_sound` | `sound_event` | reference or inline | yes | — | always a list |

- **Fields left out on write:** none.
- **Write form notes:**
  - Each map key is a key of `enchantment_effect_component_type`. The key selects the value form in this table.
  - Each "list of" value is always a list. A load step validates the list. This step does not change the write form.

| Key | Value |
|-----|-------|
| `damage_protection` | list of enchantment conditional effect (REG-FLD-352) with an enchantment value effect (REG-FLD-356) |
| `damage_immunity` | list of enchantment conditional effect (REG-FLD-352) with an enchantment damage immunity effect (REG-FLD-363) |
| `damage` | list of enchantment conditional effect (REG-FLD-352) with an enchantment value effect (REG-FLD-356) |
| `smash_damage_per_fallen_block` | list of enchantment conditional effect (REG-FLD-352) with an enchantment value effect (REG-FLD-356) |
| `knockback` | list of enchantment conditional effect (REG-FLD-352) with an enchantment value effect (REG-FLD-356) |
| `armor_effectiveness` | list of enchantment conditional effect (REG-FLD-352) with an enchantment value effect (REG-FLD-356) |
| `post_attack` | list of enchantment targeted conditional effect (REG-FLD-353) with an enchantment entity effect (REG-FLD-364) |
| `post_piercing_attack` | list of enchantment conditional effect (REG-FLD-352) with an enchantment entity effect (REG-FLD-364) |
| `hit_block` | list of enchantment conditional effect (REG-FLD-352) with an enchantment entity effect (REG-FLD-364) |
| `item_damage` | list of enchantment conditional effect (REG-FLD-352) with an enchantment value effect (REG-FLD-356) |
| `equipment_drops` | list of enchantment equipment drops conditional effect (REG-FLD-354) with an enchantment value effect (REG-FLD-356) |
| `location_changed` | list of enchantment conditional effect (REG-FLD-352) with an enchantment location-based effect (REG-FLD-365) |
| `tick` | list of enchantment conditional effect (REG-FLD-352) with an enchantment entity effect (REG-FLD-364) |
| `ammo_use` | list of enchantment conditional effect (REG-FLD-352) with an enchantment value effect (REG-FLD-356) |
| `projectile_piercing` | list of enchantment conditional effect (REG-FLD-352) with an enchantment value effect (REG-FLD-356) |
| `projectile_spawned` | list of enchantment conditional effect (REG-FLD-352) with an enchantment entity effect (REG-FLD-364) |
| `projectile_spread` | list of enchantment conditional effect (REG-FLD-352) with an enchantment value effect (REG-FLD-356) |
| `projectile_count` | list of enchantment conditional effect (REG-FLD-352) with an enchantment value effect (REG-FLD-356) |
| `trident_return_acceleration` | list of enchantment conditional effect (REG-FLD-352) with an enchantment value effect (REG-FLD-356) |
| `fishing_time_reduction` | list of enchantment conditional effect (REG-FLD-352) with an enchantment value effect (REG-FLD-356) |
| `fishing_luck_bonus` | list of enchantment conditional effect (REG-FLD-352) with an enchantment value effect (REG-FLD-356) |
| `block_experience` | list of enchantment conditional effect (REG-FLD-352) with an enchantment value effect (REG-FLD-356) |
| `mob_experience` | list of enchantment conditional effect (REG-FLD-352) with an enchantment value effect (REG-FLD-356) |
| `repair_with_xp` | list of enchantment conditional effect (REG-FLD-352) with an enchantment value effect (REG-FLD-356) |
| `attributes` | list of enchantment attribute effect (REG-FLD-385); each element is an object without a `type` field |
| `crossbow_charge_time` | one enchantment value effect (REG-FLD-356) |
| `crossbow_charging_sounds` | list of crossbow charging sounds (REG-FLD-355) |
| `trident_sound` | list of sound event (REG-FLD-302) values (see the reference table) |
| `prevent_equipment_drop` | `{}` |
| `prevent_armor_change` | `{}` |
| `trident_spin_attack_strength` | one enchantment value effect (REG-FLD-356) |

- **Nested types:** enchantment conditional effect (REG-FLD-352), enchantment targeted conditional effect (REG-FLD-353), enchantment equipment drops conditional effect (REG-FLD-354), enchantment attribute effect (REG-FLD-385), crossbow charging sounds (REG-FLD-355), enchantment value effect (REG-FLD-356), enchantment entity effect (REG-FLD-364), enchantment location-based effect (REG-FLD-365), enchantment damage immunity effect (REG-FLD-363), sound event (REG-FLD-302).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-352: enchantment conditional effect

- **Reached from:** enchantment effect map (REG-FLD-351) (most list values).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `requirements` | `predicate` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `requirements` is optional and has no default. It is written only when it is present. Its inline value is a loot condition (REG-FLD-700).
  - `effect` is required. Its type depends on the map key: enchantment value effect (REG-FLD-356), enchantment entity effect (REG-FLD-364), enchantment location-based effect (REG-FLD-365) or enchantment damage immunity effect (REG-FLD-363).
- **Nested types:** loot condition (REG-FLD-700), enchantment value effect (REG-FLD-356), enchantment entity effect (REG-FLD-364), enchantment location-based effect (REG-FLD-365), enchantment damage immunity effect (REG-FLD-363).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-353: enchantment targeted conditional effect

- **Reached from:** enchantment effect map (REG-FLD-351) `post_attack`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `requirements` | `predicate` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `requirements` is optional and has no default. It is written only when it is present. Its inline value is a loot condition (REG-FLD-700).
  - `enchanted` and `affected` are required enchantment target strings.
  - `effect` is a required enchantment entity effect (REG-FLD-364).
- **Nested types:** loot condition (REG-FLD-700), enchantment entity effect (REG-FLD-364).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-354: enchantment equipment drops conditional effect

- **Reached from:** enchantment effect map (REG-FLD-351) `equipment_drops`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `requirements` | `predicate` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `requirements` is optional and has no default. It is written only when it is present. Its inline value is a loot condition (REG-FLD-700).
  - `enchanted` is a required enchantment target string. It accepts a smaller set of strings than the `enchanted` field of enchantment targeted conditional effect (REG-FLD-353).
  - `affected` is never written and never read. The load sets it to `victim`.
  - `effect` is a required enchantment value effect (REG-FLD-356).
- **Nested types:** loot condition (REG-FLD-700), enchantment value effect (REG-FLD-356).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-355: crossbow charging sounds

- **Reached from:** enchantment effect map (REG-FLD-351) `crossbow_charging_sounds`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `start` | `sound_event` | reference or inline | yes | — | — |
| `mid` | `sound_event` | reference or inline | yes | — | — |
| `end` | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `start`, `mid` and `end` are optional and have no default. Each is written only when it is present.
- **Nested types:** sound event (REG-FLD-302).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-356: enchantment value effect

- **Reached from:** enchantment effect map (REG-FLD-351) (value effect keys, `crossbow_charge_time`, `trident_spin_attack_strength`), enchantment conditional effect (REG-FLD-352), enchantment equipment drops conditional effect (REG-FLD-354), enchantment value effect `all_of` (REG-FLD-358).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `enchantment_value_effect_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Nested types:** enchantment value effect `add` (REG-FLD-357), enchantment value effect `all_of` (REG-FLD-358), enchantment value effect `multiply` (REG-FLD-359), enchantment value effect `remove_binomial` (REG-FLD-360), enchantment value effect `exponential` (REG-FLD-361), enchantment value effect `set` (REG-FLD-362).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-357: enchantment value effect `add`

- **Reached from:** enchantment value effect (REG-FLD-356).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `value` is a required level-based value (REG-FLD-386).
- **Nested types:** level-based value (REG-FLD-386).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-358: enchantment value effect `all_of`

- **Reached from:** enchantment value effect (REG-FLD-356).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `effects` is always a list of enchantment value effect (REG-FLD-356) values.
- **Nested types:** enchantment value effect (REG-FLD-356).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-359: enchantment value effect `multiply`

- **Reached from:** enchantment value effect (REG-FLD-356).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `factor` is a required level-based value (REG-FLD-386).
- **Nested types:** level-based value (REG-FLD-386).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-360: enchantment value effect `remove_binomial`

- **Reached from:** enchantment value effect (REG-FLD-356).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `chance` is a required level-based value (REG-FLD-386).
- **Nested types:** level-based value (REG-FLD-386).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-361: enchantment value effect `exponential`

- **Reached from:** enchantment value effect (REG-FLD-356).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `base` and `exponent` are required level-based value (REG-FLD-386) values.
- **Nested types:** level-based value (REG-FLD-386).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-362: enchantment value effect `set`

- **Reached from:** enchantment value effect (REG-FLD-356).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `value` is a required level-based value (REG-FLD-386).
- **Nested types:** level-based value (REG-FLD-386).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-363: enchantment damage immunity effect

- **Reached from:** enchantment conditional effect (REG-FLD-352) `effect`, for the enchantment effect map (REG-FLD-351) key `damage_immunity`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value has no fields. It is always written as `{}`.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-364: enchantment entity effect

- **Reached from:** enchantment effect map (REG-FLD-351) (`post_attack`, `post_piercing_attack`, `hit_block`, `tick`, `projectile_spawned`), enchantment conditional effect (REG-FLD-352), enchantment targeted conditional effect (REG-FLD-353), enchantment entity effect `all_of` (REG-FLD-366).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `enchantment_entity_effect_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The type ids are `all_of`, `apply_mob_effect`, `change_item_damage`, `damage_entity`, `explode`, `ignite`, `apply_impulse`, `apply_exhaustion`, `play_sound`, `replace_block`, `replace_disk`, `run_function`, `set_block_properties`, `spawn_particles` and `summon_entity`.
- **Nested types:** enchantment entity effect `all_of` (REG-FLD-366), enchantment entity effect `apply_mob_effect` (REG-FLD-368), enchantment entity effect `change_item_damage` (REG-FLD-369), enchantment entity effect `damage_entity` (REG-FLD-370), enchantment entity effect `explode` (REG-FLD-371), enchantment entity effect `ignite` (REG-FLD-373), enchantment entity effect `apply_impulse` (REG-FLD-374), enchantment entity effect `apply_exhaustion` (REG-FLD-375), enchantment entity effect `play_sound` (REG-FLD-376), enchantment entity effect `replace_block` (REG-FLD-377), enchantment entity effect `replace_disk` (REG-FLD-378), enchantment entity effect `run_function` (REG-FLD-379), enchantment entity effect `set_block_properties` (REG-FLD-380), enchantment entity effect `spawn_particles` (REG-FLD-381), enchantment entity effect `summon_entity` (REG-FLD-384).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-365: enchantment location-based effect

- **Reached from:** enchantment effect map (REG-FLD-351) `location_changed`, enchantment conditional effect (REG-FLD-352), enchantment location-based effect `all_of` (REG-FLD-367).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `enchantment_location_based_effect_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The type `all_of` has its own rule: enchantment location-based effect `all_of` (REG-FLD-367).
  - The type `attribute` has the fields of enchantment attribute effect (REG-FLD-385).
  - Each other type id of enchantment entity effect (REG-FLD-364) is also a type id here. It has the same fields and the same write form as the entity effect rule with that type id.
- **Nested types:** enchantment location-based effect `all_of` (REG-FLD-367), enchantment attribute effect (REG-FLD-385), enchantment entity effect `apply_mob_effect` (REG-FLD-368), enchantment entity effect `change_item_damage` (REG-FLD-369), enchantment entity effect `damage_entity` (REG-FLD-370), enchantment entity effect `explode` (REG-FLD-371), enchantment entity effect `ignite` (REG-FLD-373), enchantment entity effect `apply_impulse` (REG-FLD-374), enchantment entity effect `apply_exhaustion` (REG-FLD-375), enchantment entity effect `play_sound` (REG-FLD-376), enchantment entity effect `replace_block` (REG-FLD-377), enchantment entity effect `replace_disk` (REG-FLD-378), enchantment entity effect `run_function` (REG-FLD-379), enchantment entity effect `set_block_properties` (REG-FLD-380), enchantment entity effect `spawn_particles` (REG-FLD-381), enchantment entity effect `summon_entity` (REG-FLD-384).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-366: enchantment entity effect `all_of`

- **Reached from:** enchantment entity effect (REG-FLD-364).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `effects` is always a list of enchantment entity effect (REG-FLD-364) values.
- **Nested types:** enchantment entity effect (REG-FLD-364).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-367: enchantment location-based effect `all_of`

- **Reached from:** enchantment location-based effect (REG-FLD-365).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `effects` is always a list of enchantment location-based effect (REG-FLD-365) values.
- **Nested types:** enchantment location-based effect (REG-FLD-365).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-368: enchantment entity effect `apply_mob_effect`

- **Reached from:** enchantment entity effect (REG-FLD-364), enchantment location-based effect (REG-FLD-365).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `to_apply` | `mob_effect` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Write form notes:**
  - `min_duration`, `max_duration`, `min_amplifier` and `max_amplifier` are required level-based value (REG-FLD-386) values.
- **Nested types:** level-based value (REG-FLD-386).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-369: enchantment entity effect `change_item_damage`

- **Reached from:** enchantment entity effect (REG-FLD-364), enchantment location-based effect (REG-FLD-365).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `amount` is a required level-based value (REG-FLD-386).
- **Nested types:** level-based value (REG-FLD-386).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-370: enchantment entity effect `damage_entity`

- **Reached from:** enchantment entity effect (REG-FLD-364), enchantment location-based effect (REG-FLD-365).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `damage_type` | `damage_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `min_damage` and `max_damage` are required level-based value (REG-FLD-386) values.
- **Nested types:** level-based value (REG-FLD-386).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-371: enchantment entity effect `explode`

- **Reached from:** enchantment entity effect (REG-FLD-364), enchantment location-based effect (REG-FLD-365).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `damage_type` | `damage_type` | key only | yes | — | — |
| `immune_blocks` | `block` | entry set | yes | no | bare value for one element |
| `sound` | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `attribute_to_user` | `false` | no |
| `offset` | `[0.0, 0.0, 0.0]` | no |
| `create_fire` | `false` | no |
| `block_particles` | `[]` | no |

- **Write form notes:**
  - `damage_type`, `immune_blocks`, `sound`, `small_particle`, `large_particle` and `knockback_multiplier` are optional and have no default. Each is written only when it is present.
  - `small_particle` and `large_particle` hold particle options (REG-FLD-256). The `type` field of that value references `particle_type`.
  - `block_particles` is always a list of explosion block particle entry (REG-FLD-372) values. A list equals the default only when it has no items.
  - `radius` is a required level-based value (REG-FLD-386). `knockback_multiplier` is a level-based value (REG-FLD-386).
  - `block_interaction` is a required explosion interaction string.
  - `offset` is a list of three numbers `[x, y, z]`.
- **Nested types:** explosion block particle entry (REG-FLD-372), particle options (REG-FLD-256), sound event (REG-FLD-302), level-based value (REG-FLD-386).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-372: explosion block particle entry

- **Reached from:** enchantment entity effect `explode` (REG-FLD-371) `block_particles`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `data.scaling` | `1.0` | no |
| `data.speed` | `1.0` | no |

- **Write form notes:**
  - The shape is `{"data": {"particle": ..., "scaling": ..., "speed": ...}, "weight": ...}`.
  - `weight` is a required integer.
  - `data.particle` holds particle options (REG-FLD-256). The `type` field of that value references `particle_type`.
- **Nested types:** particle options (REG-FLD-256).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-373: enchantment entity effect `ignite`

- **Reached from:** enchantment entity effect (REG-FLD-364), enchantment location-based effect (REG-FLD-365).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `duration` is a required level-based value (REG-FLD-386).
- **Nested types:** level-based value (REG-FLD-386).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-374: enchantment entity effect `apply_impulse`

- **Reached from:** enchantment entity effect (REG-FLD-364), enchantment location-based effect (REG-FLD-365).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `direction` and `coordinate_scale` are required lists of three numbers.
  - `magnitude` is a required level-based value (REG-FLD-386).
- **Nested types:** level-based value (REG-FLD-386).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-375: enchantment entity effect `apply_exhaustion`

- **Reached from:** enchantment entity effect (REG-FLD-364), enchantment location-based effect (REG-FLD-365).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `amount` is a required level-based value (REG-FLD-386).
- **Nested types:** level-based value (REG-FLD-386).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-376: enchantment entity effect `play_sound`

- **Reached from:** enchantment entity effect (REG-FLD-364), enchantment location-based effect (REG-FLD-365).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `sound` | `sound_event` | reference or inline | yes | — | bare value for one element |

- **Fields left out on write:** none.
- **Write form notes:**
  - A `sound` list has at most 255 items.
  - `volume` and `pitch` are required float provider (REG-FLD-224) values. The `volume` range is 0.00001 to 10. The `pitch` range is 0.00001 to 2.
- **Nested types:** sound event (REG-FLD-302), float provider (REG-FLD-224).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-377: enchantment entity effect `replace_block`

- **Reached from:** enchantment entity effect (REG-FLD-364), enchantment location-based effect (REG-FLD-365).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `block_state` | `worldgen/block_state_provider` | reference or inline | yes | — | — |
| `trigger_game_event` | `game_event` | key only | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `offset` | `[0, 0, 0]` | no |

- **Write form notes:**
  - The inline value of `block_state` is a block state provider (REG-FLD-304).
  - `predicate` holds a world generation block predicate (REG-FLD-317).
  - `predicate` and `trigger_game_event` are optional and have no default. Each is written only when it is present.
- **Nested types:** block state provider (REG-FLD-304), world generation block predicate (REG-FLD-317).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-378: enchantment entity effect `replace_disk`

- **Reached from:** enchantment entity effect (REG-FLD-364), enchantment location-based effect (REG-FLD-365).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `block_state` | `worldgen/block_state_provider` | reference or inline | yes | — | — |
| `trigger_game_event` | `game_event` | key only | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `offset` | `[0, 0, 0]` | no |

- **Write form notes:**
  - The inline value of `block_state` is a block state provider (REG-FLD-304).
  - `predicate` holds a world generation block predicate (REG-FLD-317).
  - `predicate` and `trigger_game_event` are optional and have no default. Each is written only when it is present.
  - `radius` and `height` are required level-based value (REG-FLD-386) values.
- **Nested types:** block state provider (REG-FLD-304), world generation block predicate (REG-FLD-317), level-based value (REG-FLD-386).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-379: enchantment entity effect `run_function`

- **Reached from:** enchantment entity effect (REG-FLD-364), enchantment location-based effect (REG-FLD-365).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `function` is an identifier of a function. It is not a reference. The load does not look it up.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-380: enchantment entity effect `set_block_properties`

- **Reached from:** enchantment entity effect (REG-FLD-364), enchantment location-based effect (REG-FLD-365).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `trigger_game_event` | `game_event` | key only | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `offset` | `[0, 0, 0]` | no |

- **Write form notes:**
  - `trigger_game_event` is optional and has no default. It is written only when it is present.
  - `properties` is a required map from property name strings to value strings. It has the form of item component `block_state` (REG-FLD-581).
- **Nested types:** item component `block_state` (REG-FLD-581).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-381: enchantment entity effect `spawn_particles`

- **Reached from:** enchantment entity effect (REG-FLD-364), enchantment location-based effect (REG-FLD-365).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `speed` | `0.0` | no |

- **Write form notes:**
  - `particle` holds particle options (REG-FLD-256). The `type` field of that value references `particle_type`.
  - `speed` is a float provider (REG-FLD-224). Only the constant `0.0` equals the default.
  - `horizontal_position` and `vertical_position` are required spawn particles position source (REG-FLD-382) values.
  - `horizontal_velocity` and `vertical_velocity` are required spawn particles velocity source (REG-FLD-383) values.
- **Nested types:** particle options (REG-FLD-256), float provider (REG-FLD-224), spawn particles position source (REG-FLD-382), spawn particles velocity source (REG-FLD-383).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-382: spawn particles position source

- **Reached from:** enchantment entity effect `spawn_particles` (REG-FLD-381) `horizontal_position` and `vertical_position`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `offset` | `0.0` | no |
| `scale` | `1.0` | no |

- **Write form notes:**
  - `type` is a required string: `entity_position` or `in_bounding_box`. It is not a registry key.
  - The load rejects a `scale` other than `1.0` with the type `entity_position`.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-383: spawn particles velocity source

- **Reached from:** enchantment entity effect `spawn_particles` (REG-FLD-381) `horizontal_velocity` and `vertical_velocity`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `movement_scale` | `0.0` | no |
| `base` | `0.0` | no |

- **Write form notes:**
  - `base` is a float provider (REG-FLD-224). Only the constant `0.0` equals the default.
  - A velocity source with both default values is written as `{}`.
- **Nested types:** float provider (REG-FLD-224).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-384: enchantment entity effect `summon_entity`

- **Reached from:** enchantment entity effect (REG-FLD-364), enchantment location-based effect (REG-FLD-365).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `entity` | `entity_type` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `join_team` | `false` | no |

- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-385: enchantment attribute effect

- **Reached from:** enchantment effect map (REG-FLD-351) `attributes` (always a list; each element has no `type` field), enchantment location-based effect (REG-FLD-365) with `type` `attribute`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `attribute` | `attribute` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `id` is an identifier of an attribute modifier. It is not a reference.
  - `amount` is a required level-based value (REG-FLD-386).
  - `operation` is a required attribute modifier operation string.
  - All fields are required.
- **Nested types:** level-based value (REG-FLD-386).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-386: level-based value

- **Reached from:** each effect field that holds a level-based value, and the nested values of level-based value `clamped` (REG-FLD-387), level-based value `fraction` (REG-FLD-388), level-based value `exponent` (REG-FLD-391) and level-based value `lookup` (REG-FLD-392).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `enchantment_level_based_value_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - A constant value is always written as a bare number.
  - Each other value is written as an object with a `type` field.
  - The type id `constant` is not in the type registry. The object `{"type": "constant", ...}` does not load.
- **Nested types:** level-based value `clamped` (REG-FLD-387), level-based value `fraction` (REG-FLD-388), level-based value `levels_squared` (REG-FLD-389), level-based value `linear` (REG-FLD-390), level-based value `exponent` (REG-FLD-391), level-based value `lookup` (REG-FLD-392).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-387: level-based value `clamped`

- **Reached from:** level-based value (REG-FLD-386).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `value` is a required level-based value (REG-FLD-386). `min` and `max` are required numbers.
- **Nested types:** level-based value (REG-FLD-386).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-388: level-based value `fraction`

- **Reached from:** level-based value (REG-FLD-386).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `numerator` and `denominator` are required level-based value (REG-FLD-386) values.
- **Nested types:** level-based value (REG-FLD-386).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-389: level-based value `levels_squared`

- **Reached from:** level-based value (REG-FLD-386).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `added` is a required number.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-390: level-based value `linear`

- **Reached from:** level-based value (REG-FLD-386).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `base` and `per_level_above_first` are required numbers.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-391: level-based value `exponent`

- **Reached from:** level-based value (REG-FLD-386).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `base` and `power` are required level-based value (REG-FLD-386) values.
- **Nested types:** level-based value (REG-FLD-386).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-392: level-based value `lookup`

- **Reached from:** level-based value (REG-FLD-386).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `values` is a required list of numbers. It is always a list.
  - `fallback` is a required level-based value (REG-FLD-386).
- **Nested types:** level-based value (REG-FLD-386).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

## 7. Dialog and test types and text component

This chapter defines the value types inside the `dialog`, `test_environment` and `test_instance` registries. It also defines the text component and its parts. Other registries reach the text component from many fields.

### REG-FLD-400: dialog common fields

- **Reached from:** every dialog type. These fields are written in the parent object: dialog `notice` (REG-FLD-401), dialog `confirmation` (REG-FLD-402), dialog `multi_action` (REG-FLD-403), dialog `server_links` (REG-FLD-404), dialog `dialog_list` (REG-FLD-405).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `can_close_with_escape` | `true` | no |
| `pause` | `true` | no |
| `after_action` | `"close"` | no |
| `body` | `[]` | no |
| `inputs` | `[]` | no |

- **Write form notes:**
  - `body` is a list of dialog body (REG-FLD-411) values. A list with one element is written as a bare object, not as a list.
  - `inputs` is always written as a list.
  - `title` is required. `external_title` is optional and has no default.
  - The load fails when `pause` is `true` and `after_action` is not `close` or `wait_for_response`.
- **Nested types:** text component (REG-FLD-429) (`title`, `external_title`), dialog body (REG-FLD-411), dialog input (REG-FLD-414).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-401: dialog `notice`

- **Reached from:** Registry `dialog` (REG-FLD-127) field `type`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `action` | `{"label":{"translate":"gui.ok"}}` | no |

- **Write form notes:**
  - The default `action` is an action button (REG-FLD-406) with the label `gui.ok`, `width` 150, no `tooltip` and no `action`. A button with any other value is written. A label with any style is not equal to the default.
- **Nested types:** dialog common fields (REG-FLD-400), action button (REG-FLD-406).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-402: dialog `confirmation`

- **Reached from:** Registry `dialog` (REG-FLD-127) field `type`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `yes` and `no` are required action button (REG-FLD-406) values.
- **Nested types:** dialog common fields (REG-FLD-400), action button (REG-FLD-406).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-403: dialog `multi_action`

- **Reached from:** Registry `dialog` (REG-FLD-127) field `type`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `columns` | `2` | no |

- **Write form notes:**
  - `actions` is always written as a list. The list must not be empty.
  - `exit_action` is optional and has no default.
- **Nested types:** dialog common fields (REG-FLD-400), action button (REG-FLD-406).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-404: dialog `server_links`

- **Reached from:** Registry `dialog` (REG-FLD-127) field `type`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `columns` | `2` | no |
| `button_width` | `150` | no |

- **Write form notes:**
  - `exit_action` is optional and has no default.
- **Nested types:** dialog common fields (REG-FLD-400), action button (REG-FLD-406).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-405: dialog `dialog_list`

- **Reached from:** Registry `dialog` (REG-FLD-127) field `type`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `dialogs` | `dialog` | entry set | yes | yes | bare value for one element |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `columns` | `2` | no |
| `button_width` | `150` | no |

- **Write form notes:**
  - A tag in `dialogs` requires the `#` prefix.
  - An inline item of `dialogs` is a full dialog object. The type is recursive.
  - `exit_action` is optional and has no default.
- **Nested types:** dialog common fields (REG-FLD-400), action button (REG-FLD-406), Registry `dialog` (REG-FLD-127).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-406: action button

- **Reached from:** dialog `notice` (REG-FLD-401) field `action`; dialog `confirmation` (REG-FLD-402) fields `yes` and `no`; dialog `multi_action` (REG-FLD-403) fields `actions[]` and `exit_action`; dialog `server_links` (REG-FLD-404) field `exit_action`; dialog `dialog_list` (REG-FLD-405) field `exit_action`.
- **References:** none at this level.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `width` | `150` | no |

- **Write form notes:**
  - `label` is a required text component. `tooltip` is an optional text component. `action` is an optional dialog action (REG-FLD-407). The optional fields have no default.
- **Nested types:** text component (REG-FLD-429), dialog action (REG-FLD-407).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-407: dialog action

- **Reached from:** action button (REG-FLD-406) field `action`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `dialog_action_type` | key only | yes | — | — |

- **Fields left out on write:** none at this level.
- **Write form notes:**
  - The type ids are `open_url`, `run_command`, `suggest_command`, `show_dialog`, `change_page`, `copy_to_clipboard`, `custom`, `dynamic/run_command` and `dynamic/custom`.
  - The click event action `open_file` has no dialog action type.
- **Nested types:** dialog action static types (REG-FLD-408), dialog action `dynamic/run_command` (REG-FLD-409), dialog action `dynamic/custom` (REG-FLD-410).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-408: dialog action static types

- **Reached from:** dialog action (REG-FLD-407) field `type`, for the type ids `open_url`, `run_command`, `suggest_command`, `show_dialog`, `change_page`, `copy_to_clipboard` and `custom`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `dialog` (type `show_dialog`) | `dialog` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - Each type has the fields of the click event (REG-FLD-440) action with the same name. These fields are written in the parent object, next to `type`.
  - The click event key `action` is not written. The key `type` replaces it.
  - Only `show_dialog` has a reference. The other fields are the same as in click event (REG-FLD-440).
- **Nested types:** click event (REG-FLD-440), Registry `dialog` (REG-FLD-127).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-409: dialog action `dynamic/run_command`

- **Reached from:** dialog action (REG-FLD-407) field `type`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `template` is a string. It is a command template with variable names.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-410: dialog action `dynamic/custom`

- **Reached from:** dialog action (REG-FLD-407) field `type`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `id` is a plain identifier. It is not a reference.
  - `additions` is an optional NBT compound.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-411: dialog body

- **Reached from:** dialog common fields (REG-FLD-400) field `body`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `dialog_body_type` | key only | yes | — | — |

- **Fields left out on write:** none at this level.
- **Write form notes:**
  - The type ids are `item` and `plain_message`.
- **Nested types:** dialog body `item` (REG-FLD-413), dialog body `plain_message` (REG-FLD-412).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-412: dialog body `plain_message`

- **Reached from:** dialog body (REG-FLD-411) field `type`; dialog body `item` (REG-FLD-413) field `description`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `width` | `200` | no |

- **Write form notes:**
  - In the field `description` of dialog body `item` (REG-FLD-413), the load also accepts a bare text component. The write always uses the object form `{"contents": ...}`. `width` is left out when it is 200.
- **Nested types:** text component (REG-FLD-429) (`contents`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-413: dialog body `item`

- **Reached from:** dialog body (REG-FLD-411) field `type`.
- **References:** the field `item` is an item stack (REG-FLD-500). That rule defines its references.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `show_decorations` | `true` | no |
| `show_tooltip` | `true` | no |
| `width` | `16` | no |
| `height` | `16` | no |

- **Write form notes:**
  - `description` is optional and has no default. When present, it is always written as an object. See dialog body `plain_message` (REG-FLD-412).
- **Nested types:** item stack (REG-FLD-500), dialog body `plain_message` (REG-FLD-412).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-414: dialog input

- **Reached from:** dialog common fields (REG-FLD-400) field `inputs[]`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `input_control_type` | key only | yes | — | — |

- **Fields left out on write:** none at this level.
- **Write form notes:**
  - `key` is a string. It is the name of a template variable.
  - The fields of the input control type are written in the same object, next to `key` and `type`.
  - The type ids are `boolean`, `number_range`, `single_option` and `text`.
- **Nested types:** dialog input `boolean` (REG-FLD-415), dialog input `number_range` (REG-FLD-416), dialog input `single_option` (REG-FLD-417), dialog input `text` (REG-FLD-418).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-415: dialog input `boolean`

- **Reached from:** dialog input (REG-FLD-414) field `type`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `initial` | `false` | no |
| `on_true` | `"true"` | no |
| `on_false` | `"false"` | no |

- **Nested types:** text component (REG-FLD-429) (`label`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-416: dialog input `number_range`

- **Reached from:** dialog input (REG-FLD-414) field `type`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `width` | `200` | no |
| `label_format` | `"options.generic_value"` | no |

- **Write form notes:**
  - The range fields are written in the same object. `start` and `end` are required floats. `initial` and `step` are optional and have no default.
  - `label_format` is a translation key. It is not a reference.
- **Nested types:** text component (REG-FLD-429) (`label`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-417: dialog input `single_option`

- **Reached from:** dialog input (REG-FLD-414) field `type`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `width` | `200` | no |
| `label_visible` | `true` | no |
| `options[].initial` | `false` | no |

- **Write form notes:**
  - `options` is always written as a list. The list must not be empty.
  - The load accepts an option as a bare string `id`. The write always uses the object form `{"id": ...}`.
- **Nested types:** text component (REG-FLD-429) (`label`, `options[].display`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-418: dialog input `text`

- **Reached from:** dialog input (REG-FLD-414) field `type`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `width` | `200` | no |
| `label_visible` | `true` | no |
| `initial` | `""` | no |
| `max_length` | `32` | no |

- **Write form notes:**
  - `multiline` is optional. Its fields `max_lines` and `height` are optional and have no default.
- **Nested types:** text component (REG-FLD-429) (`label`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-419: test environment `all_of`

- **Reached from:** Registry `test_environment` (REG-FLD-125) field `type`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `definitions[]` | `test_environment` | reference or inline | yes | — | always a list |

- **Fields left out on write:** none.
- **Write form notes:**
  - `definitions` is a list, not an entry set. Each item is a key or an inline test environment. The list does not accept a tag.
- **Nested types:** Registry `test_environment` (REG-FLD-125).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-420: test environment `clock_time`

- **Reached from:** Registry `test_environment` (REG-FLD-125) field `type`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `clock` | `world_clock` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `time` is a required integer. It must not be negative.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-421: test environment `difficulty`

- **Reached from:** Registry `test_environment` (REG-FLD-125) field `type`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `difficulty` is an enum string. It is not a reference.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-422: test environment `function`

- **Reached from:** Registry `test_environment` (REG-FLD-125) field `type`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `setup` and `teardown` are optional identifiers of functions. They are not references. The game resolves them when the test runs.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-423: test environment `game_rules`

- **Reached from:** Registry `test_environment` (REG-FLD-125) field `type`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `rules.<map key>` | `game_rule` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `rules` is an object. Each map key is a game rule key. Each map value is a primitive value of the type of that rule.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-424: test environment `timeline_attributes`

- **Reached from:** Registry `test_environment` (REG-FLD-125) field `type`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `timelines[]` | `timeline` | key only | yes | — | always a list |

- **Fields left out on write:** none.
- **Write form notes:**
  - Each item of `timelines` is a key. An inline value and a tag are not accepted.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-425: test environment `weather`

- **Reached from:** Registry `test_environment` (REG-FLD-125) field `type`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `weather` is an enum string: `clear`, `rain` or `thunder`. It is not a reference.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-426: test instance `function`

- **Reached from:** Registry `test_instance` (REG-FLD-126) field `type`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `function` | `test_function` | key only | no | — | — |

- **Fields left out on write:** none at this level. See test data (REG-FLD-428).
- **Write form notes:**
  - The fields of test data (REG-FLD-428) are written in the same object.
- **Nested types:** test data (REG-FLD-428).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-427: test instance `block_based`

- **Reached from:** Registry `test_instance` (REG-FLD-126) field `type`.
- **References:** none.
- **Fields left out on write:** none at this level. See test data (REG-FLD-428).
- **Write form notes:**
  - This type has only the fields of test data (REG-FLD-428). They are written in the same object.
- **Nested types:** test data (REG-FLD-428).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-428: test data

- **Reached from:** every test instance type. These fields are written in the parent object: test instance `function` (REG-FLD-426), test instance `block_based` (REG-FLD-427).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `environment` | `test_environment` | reference or inline | yes | — | — |
| `dimension` | `dimension` | key only | no | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `dimension` | `"minecraft:overworld"` | no |
| `setup_ticks` | `0` | no |
| `required` | `true` | no |
| `rotation` | `"none"` | no |
| `manual_only` | `false` | no |
| `max_attempts` | `1` | no |
| `required_successes` | `1` | no |
| `sky_access` | `false` | no |
| `padding` | `0` | no |

- **Write form notes:**
  - `structure` is an identifier of a structure template file. It is not a reference.
  - `max_ticks` is required.
- **Nested types:** Registry `test_environment` (REG-FLD-125).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-429: text component

- **Reached from:** every field that holds a text component, for example dialog titles, button labels, tooltips and the `contents` of dialog body `plain_message` (REG-FLD-412).
- **References:** none at this level.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `extra` | `[]` | no |

- **Write form notes:**
  - The load accepts three forms: a bare string, a non-empty list, or an object. A bare string is plain text. In a list, the first element is the component and the other elements become its `extra` items.
  - A component with `text` contents, no `extra` and an empty style is written as a bare string.
  - Every other component is written as an object. The list form is never written.
  - `extra` is always written as a list when present. The list must not be empty. Each item is a text component, so each item can be a bare string.
  - The contents key `type` (`text`, `translatable`, `keybind`, `score`, `selector`, `nbt`, `object`) is not written in the JSON form or the NBT form. The load infers the contents type from the content fields.
  - Only an encoder that compresses map keys writes `type`. The values of `type` are local ids. They are not references.
  - The style fields and the content fields are written in the component object.
- **Nested types:** text content `text` (REG-FLD-430), text content `translatable` (REG-FLD-431), text content `keybind` (REG-FLD-432), text content `score` (REG-FLD-433), text content `selector` (REG-FLD-434), text content `nbt` (REG-FLD-435), text content `object` (REG-FLD-437), text style (REG-FLD-439).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-430: text content `text`

- **Reached from:** text component (REG-FLD-429).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `text` is a required string.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-431: text content `translatable`

- **Reached from:** text component (REG-FLD-429).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `with` | `[]` | no |

- **Write form notes:**
  - `translate` is a required translation key. It is not a reference.
  - `fallback` is an optional string with no default. The load is lenient: an invalid `fallback` reads as absent.
  - `with` is always written as a list when present. An empty argument list is not written.
  - Each item of `with` is a primitive or a text component. A number, a boolean or a string is written as a primitive.
  - The load stores a component argument with plain text only as a string. The write then uses a bare string.
- **Nested types:** text component (REG-FLD-429) (`with[]`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-432: text content `keybind`

- **Reached from:** text component (REG-FLD-429).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `keybind` is a required string.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-433: text content `score`

- **Reached from:** text component (REG-FLD-429).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `score` is an object with `name` and `objective`. `name` is an entity selector string or a plain string.
  - `objective` is the name of a scoreboard objective. It is not a reference.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-434: text content `selector`

- **Reached from:** text component (REG-FLD-429).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `selector` is an entity selector string. `separator` is an optional text component with no default.
- **Nested types:** text component (REG-FLD-429) (`separator`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-435: text content `nbt`

- **Reached from:** text component (REG-FLD-429).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `interpret` | `false` | yes |
| `plain` | `false` | yes |

- **Write form notes:**
  - `nbt` is an NBT path string.
  - `separator` is an optional text component with no default. The load is lenient: an invalid `separator` reads as absent.
  - The fields of nbt data source (REG-FLD-436) are written in the same object.
  - The load fails when `interpret` and `plain` are both `true`.
- **Nested types:** nbt data source (REG-FLD-436), text component (REG-FLD-429) (`separator`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-436: nbt data source

- **Reached from:** text content `nbt` (REG-FLD-435). These fields are written in the parent object.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The discriminator key `source` (`entity`, `block`, `storage`) is not written in the JSON form or the NBT form. Its values are local ids. They are not references.
  - `entity`: the field `entity` is an entity selector string.
  - `block`: the field `block` is a coordinates string.
  - `storage`: the field `storage` is an identifier of a command storage. It is not a reference.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-437: text content `object`

- **Reached from:** text component (REG-FLD-429).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `fallback` is an optional text component with no default.
  - The fields of object info (REG-FLD-438) are written in the same object.
- **Nested types:** object info (REG-FLD-438), text component (REG-FLD-429) (`fallback`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-438: object info

- **Reached from:** text content `object` (REG-FLD-437). These fields are written in the parent object.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `atlas` (object `atlas`) | `"minecraft:blocks"` | no |
| `hat` (object `player`) | `true` | no |

- **Write form notes:**
  - The discriminator key `object` (`atlas`, `player`) is not written in the JSON form or the NBT form. Its values are local ids. They are not references.
  - `atlas` is an identifier of a texture atlas. It is not a reference.
  - `sprite` is a required identifier of a sprite. It is not a reference.
  - `player` has the form of item component `profile` (REG-FLD-576). The load also accepts a player name string. The write always uses the object form.
- **Nested types:** item component `profile` (REG-FLD-576).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-439: text style

- **Reached from:** text component (REG-FLD-429). These fields are written in the parent object.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The fields are `color`, `shadow_color`, `bold`, `italic`, `underlined`, `strikethrough`, `obfuscated`, `click_event`, `hover_event`, `insertion` and `font`.
  - Each field is optional and has no default. A field is written only when it is set.
  - `color` is a string. A named formatting color is written by its name, for example `"red"`. Any other color is written as `"#RRGGBB"`.
  - The load accepts `shadow_color` as an integer or as a list of four floats `[r, g, b, a]`. The write always uses an integer.
  - `font` is an identifier of a font resource. It is not a reference.
  - Only a font given by a resource id can be written. A font of the atlas form or the player form cannot be written, and the write fails.
- **Nested types:** click event (REG-FLD-440), hover event (REG-FLD-441).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-440: click event

- **Reached from:** text style (REG-FLD-439) field `click_event`; dialog action static types (REG-FLD-408) use the fields of each action.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `dialog` (action `show_dialog`) | `dialog` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The discriminator `action` is an enum string. It is not a reference.
  - The load rejects the action `open_file` from a server.
  - `show_dialog`: a registered dialog is written as its key. An inline dialog is written as an object. The type is recursive.
  - `open_url`: `url` is a URI string.
  - `run_command` and `suggest_command`: `command` is a string.
  - `change_page`: `page` is a positive integer.
  - `copy_to_clipboard`: `value` is a string.
  - `custom`: `id` is a plain identifier. It is not a reference. `payload` is optional NBT.
  - `open_file`: `path` is a string.
- **Nested types:** Registry `dialog` (REG-FLD-127).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-441: hover event

- **Reached from:** text style (REG-FLD-439) field `hover_event`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `id` (action `show_entity`) | `entity_type` | key only | yes | — | — |
| `id`, `count`, `components` (action `show_item`) | `item` | see item stack (REG-FLD-500) | see item stack (REG-FLD-500) | — | — |

- **Fields left out on write:** none at this level. item stack (REG-FLD-500) defines the defaults of the `show_item` fields.
- **Write form notes:**
  - The discriminator `action` is an enum string: `show_text`, `show_item` or `show_entity`. It is not a reference.
  - `show_text`: `value` is a text component.
  - `show_item`: the fields of item stack (REG-FLD-500) are written in the hover event object.
  - `show_entity`: `uuid` is a UUID. The load accepts an int array of 4 elements or the hyphenated text of 8, 4, 4, 4 and 12 hex digits. The reference also accepts shorter groups, for example `1-2-3-4-5`; the server rejects a pack with such text at load. `name` is an optional text component.
- **Nested types:** item stack (REG-FLD-500), text component (REG-FLD-429).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

## 8. Item stack, item components and predicates

This chapter gives the reference fields and the default rules for item stacks, item components, data component predicates and advancement predicates. An entry set in this chapter accepts a tag only with the `#` prefix. A number range in this chapter has the form of number bounds (REG-FLD-646).

### REG-FLD-500: item stack

- **Reached from:** dialog body `item` (REG-FLD-413) `item`, text component (REG-FLD-429) hover event `show_item`, particle options `item` (REG-FLD-258) `item`, item component `use_remainder` (REG-FLD-533), item component `charged_projectiles` (REG-FLD-555), item component `bundle_contents` (REG-FLD-556), item component `container` (REG-FLD-580), item component `pot_decorations` (REG-FLD-579), item component `sulfur_cube_content` (REG-FLD-583). Recipes, villager trades and advancement display icons also use this type, but these are not synced.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `id` | `item` | key only | yes | — | — |
| (whole value as a bare string) | `item` | key only | yes | — | — |
| `components` (map keys) | `data_component_type` | see data component patch (REG-FLD-502) | see data component patch (REG-FLD-502) | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `count` | `1` | no |
| `components` | `{}` (empty patch) | no |

- **Write form notes:**
  - `id` must not be `minecraft:air`. A load with `minecraft:air` fails.
  - `count` is in the range 1 to 99. A count of 0 fails.
  - A reader accepts a bare item id string, for example `"minecraft:stick"`. This string means `{"id": "minecraft:stick", "count": 1}`. The encoder never writes the bare string. It always writes the object form.
  - In the hover event `show_item`, these fields are written in the parent object. That form has no bare string form.
  - Three item components have no persistent form: `creative_slot_lock`, `additional_trade_cost` and `map_post_processing`. They never occur in `components`. The encoder skips them on write.
- **Nested types:** data component patch (REG-FLD-502).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-501: saved item stack

- **Reached from:** entity NBT, block entity NBT, villager offers and vault configuration. No synced registry reaches this type.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `id` | `item` | key only | yes | — | — |
| `components` (map keys) | `data_component_type` | see data component patch (REG-FLD-502) | see data component patch (REG-FLD-502) | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `components` | `{}` | no |

- **Write form notes:**
  - `id` must name a registered item.
  - `count` is always written, also when it is `1`. A reader uses `1` when `count` is absent.
  - This type has no bare string form.
  - The optional form writes an empty stack as `{}`.
- **Nested types:** data component patch (REG-FLD-502).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-502: data component patch

- **Reached from:** item stack (REG-FLD-500) `components`, saved item stack (REG-FLD-501) `components`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| map key `<id>` | `data_component_type` | key only | yes | — | — |
| map key `!<id>` | `data_component_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The value is one object. Each key is an item component id.
  - A key `<id>` sets the component. Its value has the form of the item component rule for that id.
  - A key `!<id>` removes the component from the item defaults. Its value is `{}`.
  - The load fails when a key names no data component type. The load also fails when the key names a component with no persistent form.
  - The encoder skips components with no persistent form.
- **Nested types:** every item component rule in this chapter.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-503: item component `custom_data`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `custom_data`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a compound tag. A reader accepts a JSON object or an SNBT string. The encoder always writes an object.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-504: item component `max_stack_size`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `max_stack_size`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is an integer in the range 1 to 99.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-505: item component `max_damage`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `max_damage`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a positive integer.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-506: item component `damage`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `damage`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a non-negative integer.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-507: item component `unbreakable`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `unbreakable`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is always `{}`.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-508: item component `use_effects`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `use_effects`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `can_sprint` | `false` | no |
| `interact_vibrations` | `true` | no |
| `speed_multiplier` | `0.2` | no |

- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-509: item component `custom_name`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `custom_name`.
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a text component (REG-FLD-429).
- **Nested types:** text component (REG-FLD-429).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-510: item component `minimum_attack_charge`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `minimum_attack_charge`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a float in the range 0 to 1.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-511: item component `damage_type`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `damage_type`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (whole value) | `damage_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-512: item component `item_name`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `item_name`.
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a text component (REG-FLD-429).
- **Nested types:** text component (REG-FLD-429).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-513: item component `item_model`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `item_model`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is an identifier of a client item model asset. It is not a reference.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-514: item component `lore`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `lore`.
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is always a list of text component (REG-FLD-429). The list has at most 256 elements.
- **Nested types:** text component (REG-FLD-429).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-515: item component `rarity`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `rarity`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is one of the strings `common`, `uncommon`, `rare` and `epic`.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-516: item components `enchantments`, `stored_enchantments`

- **Aliases:** a link to item component `enchantments` or to item component `stored_enchantments` resolves to this rule.
- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `enchantments` or `stored_enchantments`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| map keys | `enchantment` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The value is one object. Each key is an enchantment key. Each value is the level, an integer in the range 1 to 255.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-517: item components `can_place_on`, `can_break`

- **Aliases:** a link to item component `can_place_on` or to item component `can_break` resolves to this rule.
- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `can_place_on` or `can_break`.
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a non-empty list of block predicate (REG-FLD-614). The encoder writes one predicate as a bare object, and two or more predicates as a list.
- **Nested types:** block predicate (REG-FLD-614).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-518: item component `attribute_modifiers`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `attribute_modifiers`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `[].type` | `attribute` | key only | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `[].slot` | `"any"` | no |
| `[].display` | `{"type": "default"}` | no |

- **Write form notes:**
  - The value is always a list of entries.
  - `[].id`, `[].amount` and `[].operation` are required. They are written in the entry object. `[].id` is an identifier of the modifier. It is not a reference.
  - `[].display.type` is one of the strings `default`, `hidden` and `override`. It is not a reference.
  - The display type `override` has a required field `value`. `value` is a text component (REG-FLD-429).
- **Nested types:** text component (REG-FLD-429).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-519: item component `custom_model_data`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `custom_model_data`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `floats` | `[]` | no |
| `flags` | `[]` | no |
| `strings` | `[]` | no |
| `colors` | `[]` | no |

- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-520: item component `tooltip_display`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `tooltip_display`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `hidden_components[]` | `data_component_type` | key only | yes | — | always a list |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `hide_tooltip` | `false` | no |
| `hidden_components` | `[]` | no |

- **Write form notes:**
  - `hidden_components` also accepts the ids of components with no persistent form.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-521: item component `repair_cost`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `repair_cost`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a non-negative integer.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-522: item component `enchantment_glint_override`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `enchantment_glint_override`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a boolean.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-523: item component `intangible_projectile`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `intangible_projectile`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is always `{}`.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-524: item component `food`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `food`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `can_always_eat` | `false` | no |

- **Write form notes:**
  - `nutrition` and `saturation` are required.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-525: item component `consumable`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `consumable`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `sound` | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `consume_seconds` | `1.6` | no |
| `animation` | `"eat"` | no |
| `has_consume_particles` | `true` | no |
| `on_consume_effects` | `[]` | no |

- **Write form notes:**
  - `on_consume_effects` is always a list of consume effect (REG-FLD-526).
- **Nested types:** sound event (REG-FLD-302), consume effect (REG-FLD-526).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-526: consume effect

- **Reached from:** item component `consumable` (REG-FLD-525) `on_consume_effects[]`, item component `death_protection` (REG-FLD-544) `death_effects[]`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `consume_effect_type` | key only | yes | — | — |

- **Fields left out on write:** none at this level.
- **Write form notes:**
  - `type` selects the other fields. The other fields are written in the same object.
- **Nested types:** consume effect `apply_effects` (REG-FLD-527), consume effect `remove_effects` (REG-FLD-528), consume effect `teleport_randomly` (REG-FLD-529), consume effect `clear_all_effects` (REG-FLD-530), consume effect `play_sound` (REG-FLD-531).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-527: consume effect `apply_effects`

- **Reached from:** consume effect (REG-FLD-526).
- **References:** none at this level.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `probability` | `1.0` | no |

- **Write form notes:**
  - `effects` is always a list of mob effect instance (REG-FLD-532).
- **Nested types:** mob effect instance (REG-FLD-532).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-528: consume effect `remove_effects`

- **Reached from:** consume effect (REG-FLD-526).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `effects` | `mob_effect` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-529: consume effect `teleport_randomly`

- **Reached from:** consume effect (REG-FLD-526).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `diameter` | `16.0` | no |
| `directional_particles` | `true` | no |

- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-530: consume effect `clear_all_effects`

- **Reached from:** consume effect (REG-FLD-526).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - This type has no fields other than `type`.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-531: consume effect `play_sound`

- **Reached from:** consume effect (REG-FLD-526).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `sound` | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Nested types:** sound event (REG-FLD-302).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-532: mob effect instance

- **Reached from:** consume effect `apply_effects` (REG-FLD-527) `effects[]`, item component `potion_contents` (REG-FLD-557) `custom_effects[]`, and this type itself through `hidden_effect`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `id` | `mob_effect` | key only | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `amplifier` | `0` | no |
| `duration` | `0` | no |
| `ambient` | `false` | no |
| `show_particles` | `true` | no |

- **Write form notes:**
  - `show_icon` is always written. When `show_icon` is absent, a reader uses the value of `show_particles`.
  - `hidden_effect` is optional and has no default. It has the same fields as this type, but no `id`.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-533: item component `use_remainder`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `use_remainder`.
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is an item stack (REG-FLD-500).
- **Nested types:** item stack (REG-FLD-500).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-534: item component `use_cooldown`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `use_cooldown`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `seconds` is required.
  - `cooldown_group` is an identifier of a cooldown group. It is not a reference.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-535: item component `damage_resistant`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `damage_resistant`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `types` | `damage_type` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-536: item component `tool`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `tool`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `rules[].blocks` | `block` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `default_mining_speed` | `1.0` | no |
| `damage_per_block` | `1` | no |
| `can_destroy_blocks_in_creative` | `true` | no |

- **Write form notes:**
  - `rules` is required. It is always a list.
  - `rules[].speed` and `rules[].correct_for_drops` are optional and have no default.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-537: item component `weapon`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `weapon`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `item_damage_per_attack` | `1` | no |
| `disable_blocking_for_seconds` | `0.0` | no |

- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-538: item component `attack_range`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `attack_range`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `min_reach` | `0.0` | no |
| `max_reach` | `3.0` | no |
| `min_creative_reach` | `0.0` | no |
| `max_creative_reach` | `5.0` | no |
| `hitbox_margin` | `0.3` | no |
| `mob_factor` | `1.0` | no |

- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-539: item component `enchantable`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `enchantable`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `value` is required. It is a positive integer.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-540: item component `equippable`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `equippable`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `equip_sound` | `sound_event` | reference or inline | yes | — | — |
| `asset_id` | `equipment_asset` | key only | no | — | — |
| `allowed_entities` | `entity_type` | entry set | yes | no | bare value for one element |
| `shearing_sound` | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `dispensable` | `true` | no |
| `swappable` | `true` | no |
| `damage_on_hurt` | `true` | no |
| `equip_on_interact` | `false` | no |
| `can_be_sheared` | `false` | no |

- **Write form notes:**
  - `slot` is required. It is an equipment slot string.
  - `equipment_asset` is a client asset root. The server loads no registry with this id. So the key of `asset_id` is kept unchecked.
  - `camera_overlay` is an identifier of a texture. It is not a reference.
- **Nested types:** sound event (REG-FLD-302).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-541: item component `repairable`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `repairable`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `items` | `item` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-542: item component `glider`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `glider`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is always `{}`.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-543: item component `tooltip_style`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `tooltip_style`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is an identifier of a client tooltip asset. It is not a reference.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-544: item component `death_protection`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `death_protection`.
- **References:** none at this level.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `death_effects` | `[]` | no |

- **Write form notes:**
  - `death_effects` is always a list of consume effect (REG-FLD-526).
- **Nested types:** consume effect (REG-FLD-526).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-545: item component `blocks_attacks`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `blocks_attacks`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `damage_reductions[].type` | `damage_type` | entry set | yes | no | bare value for one element |
| `bypassed_by` | `damage_type` | entry set | yes | no | bare value for one element |
| `block_sound` | `sound_event` | reference or inline | yes | — | — |
| `disabled_sound` | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `block_delay_seconds` | `0.0` | no |
| `disable_cooldown_scale` | `1.0` | no |
| `damage_reductions` | `[{"base": 0.0, "factor": 1.0}]` | no |
| `item_damage` | `{"threshold": 1.0, "base": 0.0, "factor": 1.0}` | no |
| `damage_reductions[].horizontal_blocking_angle` | `90.0` | no |

- **Write form notes:**
  - The default of `damage_reductions` has one element with `horizontal_blocking_angle` `90.0`. That field is left out inside the element.
  - `damage_reductions[].base` and `damage_reductions[].factor` are required.
  - `item_damage.threshold`, `item_damage.base` and `item_damage.factor` are required.
- **Nested types:** sound event (REG-FLD-302).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-546: item component `piercing_weapon`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `piercing_weapon`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `sound` | `sound_event` | reference or inline | yes | — | — |
| `hit_sound` | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `deals_knockback` | `true` | no |
| `dismounts` | `false` | no |

- **Nested types:** sound event (REG-FLD-302).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-547: item component `kinetic_weapon`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `kinetic_weapon`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `sound` | `sound_event` | reference or inline | yes | — | — |
| `hit_sound` | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `contact_cooldown_ticks` | `10` | no |
| `delay_ticks` | `0` | no |
| `forward_movement` | `0.0` | no |
| `damage_multiplier` | `1.0` | no |
| `dismount_conditions.min_speed` | `0.0` | no |
| `knockback_conditions.min_speed` | `0.0` | no |
| `damage_conditions.min_speed` | `0.0` | no |
| `dismount_conditions.min_relative_speed` | `0.0` | no |
| `knockback_conditions.min_relative_speed` | `0.0` | no |
| `damage_conditions.min_relative_speed` | `0.0` | no |

- **Write form notes:**
  - `dismount_conditions`, `knockback_conditions` and `damage_conditions` are optional and have no default.
  - `max_duration_ticks` is required in each conditions object.
- **Nested types:** sound event (REG-FLD-302).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-548: item components `attack_animation`, `interact_animation`

- **Aliases:** a link to item component `attack_animation` or to item component `interact_animation` resolves to this rule.
- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `attack_animation` or `interact_animation`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `type` | `"whack"` | no |
| `duration` | `6` | no |

- **Write form notes:**
  - `type` is an animation string. It is not a reference.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-549: item component `block_transformer`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `block_transformer`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (whole value) | `block_transformer` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - Registry `block_transformer` (REG-FLD-131) defines the element form.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-550: item component `villager_food`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `villager_food`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `nutrition` is required. It is a positive integer.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-551: item components `dye`, `base_color`, `wolf/collar`, `cat/collar`, `sheep/color`, `shulker/color`, `tropical_fish/base_color`, `tropical_fish/pattern_color`, `cushion/color`

- **Aliases:** a link to item component `dye`, `base_color`, `wolf/collar`, `cat/collar`, `sheep/color`, `shulker/color`, `tropical_fish/base_color`, `tropical_fish/pattern_color` or `cushion/color` resolves to this rule.
- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of one of these keys.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a dye color string, for example `white` or `light_blue`. It is not a reference.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-552: item component `dyed_color`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `dyed_color`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is an RGB integer. A reader also accepts a list of floats.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-553: item component `map_id`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `map_id`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is an integer.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-554: item component `map_decorations`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `map_decorations`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `<key>.type` | `map_decoration_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The value is one object. Each key is a free string. Each value is one decoration entry.
  - `x`, `z` and `rotation` are required.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-555: item component `charged_projectiles`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `charged_projectiles`.
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is always a list of item stack (REG-FLD-500). The list has at most 1024 elements.
- **Nested types:** item stack (REG-FLD-500).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-556: item component `bundle_contents`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `bundle_contents`.
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is always a list of item stack (REG-FLD-500).
- **Nested types:** item stack (REG-FLD-500).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-557: item component `potion_contents`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `potion_contents`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `potion` | `potion` | key only | yes | — | — |
| (whole value as a bare string) | `potion` | key only | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `custom_effects` | `[]` | no |

- **Write form notes:**
  - A reader accepts a bare potion key string. The encoder never writes the bare string. It always writes the object form.
  - `custom_effects` is always a list of mob effect instance (REG-FLD-532).
- **Nested types:** mob effect instance (REG-FLD-532).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-558: item component `potion_duration_scale`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `potion_duration_scale`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a non-negative float.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-559: item component `suspicious_stew_effects`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `suspicious_stew_effects`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `[].id` | `mob_effect` | key only | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `[].duration` | `160` | yes |

- **Write form notes:**
  - The value is always a list of entries.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-560: item component `writable_book_content`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `writable_book_content`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `pages` | `[]` | no |

- **Write form notes:**
  - `pages` is always a list of filterable strings. The list has at most 100 elements.
  - The encoder always writes a filterable value as an object `{"raw": ..., "filtered": ...}`. `filtered` is optional. A reader also accepts a bare value.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-561: item component `written_book_content`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `written_book_content`.
- **References:** none at this level.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `generation` | `0` | no |
| `pages` | `[]` | no |
| `resolved` | `false` | no |

- **Write form notes:**
  - `pages[].raw` and `pages[].filtered` are text component (REG-FLD-429) values. Each value has at most 32767 characters in its string form.
  - `title` is a filterable string with at most 32 characters. `author` is required.
  - The encoder always writes a filterable value as an object. This rule is the same as in item component `writable_book_content` (REG-FLD-560).
- **Nested types:** text component (REG-FLD-429).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-562: item component `trim`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `trim`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `material` | `trim_material` | reference or inline | yes | — | — |
| `pattern` | `trim_pattern` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - An inline `material` has the form of Registry `trim_material` (REG-FLD-104). An inline `pattern` has the form of Registry `trim_pattern` (REG-FLD-103).
- **Nested types:** Registry `trim_material` (REG-FLD-104), Registry `trim_pattern` (REG-FLD-103).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-563: item component `debug_stick_state`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `debug_stick_state`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| map keys | `block` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - Each value is a block state property name. It is not a reference.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-564: item component `entity_data`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `entity_data`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `id` | `entity_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a compound tag with an embedded `id`. The other keys are free NBT.
  - `id` is always written.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-565: item component `bucket_entity_data`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `bucket_entity_data`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a compound tag. It has the form of item component `custom_data` (REG-FLD-503).
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-566: item component `block_entity_data`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `block_entity_data`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `id` | `block_entity_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The value has the form of item component `entity_data` (REG-FLD-564). `id` is always written.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-567: item component `instrument`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `instrument`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (whole value) | `instrument` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - An inline value has the form of Registry `instrument` (REG-FLD-124).
- **Nested types:** Registry `instrument` (REG-FLD-124).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-568: item component `provides_trim_material`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `provides_trim_material`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (whole value) | `trim_material` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - An inline value has the form of Registry `trim_material` (REG-FLD-104).
- **Nested types:** Registry `trim_material` (REG-FLD-104).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-569: item component `ominous_bottle_amplifier`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `ominous_bottle_amplifier`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is an integer in the range 0 to 4.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-570: item component `jukebox_playable`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `jukebox_playable`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (whole value) | `jukebox_song` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-571: item component `provides_banner_patterns`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `provides_banner_patterns`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (whole value) | `banner_pattern` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-572: item component `recipes`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `recipes`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `[]` | `recipe` | key only | no | — | always a list |

- **Fields left out on write:** none.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-573: item component `lodestone_tracker`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `lodestone_tracker`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `target.dimension` | `dimension` | key only | no | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `tracked` | `true` | no |

- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-574: item component `firework_explosion`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `firework_explosion`; item component `fireworks` (REG-FLD-575) `explosions[]`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `colors` | `[]` | no |
| `fade_colors` | `[]` | no |
| `has_trail` | `false` | no |
| `has_twinkle` | `false` | no |

- **Write form notes:**
  - `shape` is required. It is a shape string. It is not a reference.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-575: item component `fireworks`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `fireworks`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `flight_duration` | `0` | no |
| `explosions` | `[]` | no |

- **Write form notes:**
  - `explosions` is always a list of item component `firework_explosion` (REG-FLD-574). The list has at most 256 elements. The defaults of that rule apply in each element.
- **Nested types:** item component `firework_explosion` (REG-FLD-574).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-576: item component `profile`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `profile`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `properties` (partial profile form) | `[]` (empty property map) | no |

- **Write form notes:**
  - The value holds a player name, a UUID and properties.
  - A reader accepts a bare player name string. The encoder never writes the bare string. It always writes the object form.
  - `texture`, `cape` and `elytra` are identifiers of textures. They are not references. `model` is a model string.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-577: item component `note_block_sound`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `note_block_sound`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is an identifier of a sound event. It is not a reference. No load step checks it.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-578: item component `banner_patterns`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `banner_patterns`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `[].pattern` | `banner_pattern` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The value is always a list of layers.
  - `[].color` is required. It is a dye color string.
  - An inline `pattern` has the form of Registry `banner_pattern` (REG-FLD-121).
- **Nested types:** Registry `banner_pattern` (REG-FLD-121).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-579: item component `pot_decorations`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `pot_decorations`.
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - `back`, `left`, `right` and `front` are optional item stack (REG-FLD-500) values. They have no default.
- **Nested types:** item stack (REG-FLD-500).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-580: item component `container`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `container`.
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is always a list of objects `{"slot": ..., "item": ...}`. The list has at most 256 elements.
  - `slot` and `item` are required. `item` is an item stack (REG-FLD-500).
- **Nested types:** item stack (REG-FLD-500).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-581: item component `block_state`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `block_state`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is one object. Each key is a property name and each value is a property value string. They are not references.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-582: item component `bees`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `bees`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `[].entity_data.id` | `entity_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The value is always a list of occupants.
  - `[].ticks_in_hive` and `[].min_ticks_in_hive` are required.
  - `[].entity_data` has the form of item component `entity_data` (REG-FLD-564). `id` is always written.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-583: item component `sulfur_cube_content`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `sulfur_cube_content`.
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is an item stack (REG-FLD-500).
- **Nested types:** item stack (REG-FLD-500).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-584: item component `lock`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `lock`.
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is an item predicate (REG-FLD-613).
- **Nested types:** item predicate (REG-FLD-613).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-585: item component `container_loot`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `container_loot`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `loot_table` | `loot_table` | key only | no | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `seed` | `0` | no |

- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-586: item component `break_sound`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `break_sound`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (whole value) | `sound_event` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Nested types:** sound event (REG-FLD-302).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-587: item component `compostable`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `compostable`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `layers` (string form) | `context_int_provider` | key only | no | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `layers` is a resolvable int (REG-FLD-739). The encoder writes an integer for a constant and a key string for a reference.
- **Nested types:** resolvable int (REG-FLD-739).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-588: item component `cooking_fuel`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `cooking_fuel`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `burn_time` (string form) | `context_int_provider` | key only | no | — | — |
| `speed_multiplier` (string form) | `context_float_provider` | key only | no | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `burn_time` is a resolvable int (REG-FLD-739). `speed_multiplier` is a resolvable float (REG-FLD-740).
- **Nested types:** resolvable int (REG-FLD-739), resolvable float (REG-FLD-740).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-589: item component `brewing_fuel`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `brewing_fuel`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `uses` (string form) | `context_int_provider` | key only | no | — | — |
| `speed_multiplier` (string form) | `context_float_provider` | key only | no | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `uses` is a resolvable int (REG-FLD-739). `speed_multiplier` is a resolvable float (REG-FLD-740).
- **Nested types:** resolvable int (REG-FLD-739), resolvable float (REG-FLD-740).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-590: item component `mob_visibility`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `mob_visibility`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `targeting_entity_types` | `entity_type` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Write form notes:**
  - `visibility` is required.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-591: item components `villager/variant`, `wolf/variant`, `wolf/sound_variant`, `pig/variant`, `pig/sound_variant`, `cow/variant`, `cow/sound_variant`, `chicken/variant`, `chicken/sound_variant`, `zombie_nautilus/variant`, `frog/variant`, `painting/variant`, `cat/variant`, `cat/sound_variant`, `provides_pottery_pattern`

- **Aliases:** a link to item component `<id>` for each id in the title resolves to this rule.
- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of one of these keys.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `villager/variant` (whole value) | `villager_type` | key only | yes | — | — |
| `wolf/variant` (whole value) | `wolf_variant` | key only | yes | — | — |
| `wolf/sound_variant` (whole value) | `wolf_sound_variant` | key only | yes | — | — |
| `pig/variant` (whole value) | `pig_variant` | key only | yes | — | — |
| `pig/sound_variant` (whole value) | `pig_sound_variant` | key only | yes | — | — |
| `cow/variant` (whole value) | `cow_variant` | key only | yes | — | — |
| `cow/sound_variant` (whole value) | `cow_sound_variant` | key only | yes | — | — |
| `chicken/variant` (whole value) | `chicken_variant` | key only | yes | — | — |
| `chicken/sound_variant` (whole value) | `chicken_sound_variant` | key only | yes | — | — |
| `zombie_nautilus/variant` (whole value) | `zombie_nautilus_variant` | key only | yes | — | — |
| `frog/variant` (whole value) | `frog_variant` | key only | yes | — | — |
| `painting/variant` (whole value) | `painting_variant` | key only | yes | — | — |
| `cat/variant` (whole value) | `cat_variant` | key only | yes | — | — |
| `cat/sound_variant` (whole value) | `cat_sound_variant` | key only | yes | — | — |
| `provides_pottery_pattern` (whole value) | `decorated_pot_pattern` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-592: item components `fox/variant`, `salmon/size`, `parrot/variant`, `tropical_fish/pattern`, `mooshroom/variant`, `rabbit/variant`, `horse/variant`, `llama/variant`, `axolotl/variant`

- **Aliases:** a link to item component `<id>` for each id in the title resolves to this rule.
- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of one of these keys.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a fixed variant name string. It is not a reference.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-593: item components `sign_text_front`, `sign_text_back`

- **Aliases:** a link to item component `sign_text_front` or to item component `sign_text_back` resolves to this rule.
- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `sign_text_front` or `sign_text_back`.
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - `messages` is required. It is a list of exactly 4 text component (REG-FLD-429) values.
  - `filtered_messages` is optional and has no default. It is lenient. It is a list of exactly 4 text component (REG-FLD-429) values.
  - `color` and `has_glowing_text` are always written. A reader uses `black` and `false` when they are absent.
- **Nested types:** text component (REG-FLD-429).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-594: item component `waxed`

- **Reached from:** data component patch (REG-FLD-502) and data component exact predicate (REG-FLD-595), as the value of the key `waxed`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is always `{}`.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-595: data component exact predicate

- **Reached from:** data component matchers (REG-FLD-596) `components`, entity predicate `components` (REG-FLD-637).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| map keys | `data_component_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The value is one object. Each key is an item component id. Each value has the form of the item component rule for that id.
  - The load fails when a key names a component with no persistent form. The encoder skips such components.
  - The empty value is `{}`. Two values compare equal when their contents are equal. So a parent field with the default `{}` is left out when the map is empty.
- **Nested types:** every item component rule in this chapter.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-596: data component matchers

- **Reached from:** item predicate (REG-FLD-613), block predicate (REG-FLD-614). These fields are written in the parent object.
- **References:** none at this level.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `components` | `{}` | no |
| `predicates` | `{}` | no |

- **Write form notes:**
  - `components` is a data component exact predicate (REG-FLD-595). `predicates` is a data component predicate (REG-FLD-597).
- **Nested types:** data component exact predicate (REG-FLD-595), data component predicate (REG-FLD-597).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-597: data component predicate

- **Reached from:** data component matchers (REG-FLD-596) `predicates`, entity predicate `predicates` (REG-FLD-638).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| map keys | `data_component_predicate_type` or `data_component_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The value is one object. A reader first looks up each key in `data_component_predicate_type`. When that lookup fails, the reader looks up the key in `data_component_type`.
  - A key that is a predicate type id selects that predicate type. The value has the form of the rule "data component predicate `<id>`".
  - A key that is only a data component id means "the item has this component". The value is `{}`. The encoder writes the component id as the key and `{}` as the value.
  - A component id that is also a predicate type id always selects the predicate type. Examples are `damage`, `enchantments`, `potion_contents` and `trim`.
- **Nested types:** data component predicate `damage` (REG-FLD-598), data component predicate `enchantments`, `stored_enchantments` (REG-FLD-599), data component predicate `potion_contents` (REG-FLD-601), data component predicate `custom_data` (REG-FLD-602), data component predicate `container`, `bundle_contents` (REG-FLD-603), data component predicate `firework_explosion` (REG-FLD-604), data component predicate `fireworks` (REG-FLD-605), data component predicate `writable_book_content` (REG-FLD-606), data component predicate `written_book_content` (REG-FLD-607), data component predicate `attribute_modifiers` (REG-FLD-608), data component predicate `trim` (REG-FLD-609), data component predicate `jukebox_playable` (REG-FLD-610), data component predicate `villager/variant` (REG-FLD-611).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-598: data component predicate `damage`

- **Reached from:** data component predicate (REG-FLD-597).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `durability` | `{}` (any) | no |
| `damage` | `{}` (any) | no |

- **Write form notes:**
  - `durability` and `damage` are number bounds (REG-FLD-646) of integers.
- **Nested types:** number bounds (REG-FLD-646).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-599: data component predicate `enchantments`, `stored_enchantments`

- **Reached from:** data component predicate (REG-FLD-597).
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is always a list of enchantment predicate (REG-FLD-600).
- **Nested types:** enchantment predicate (REG-FLD-600).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-600: enchantment predicate

- **Reached from:** data component predicate `enchantments`, `stored_enchantments` (REG-FLD-599).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `enchantments` | `enchantment` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `levels` | `{}` (any) | no |

- **Write form notes:**
  - `levels` is a number bounds (REG-FLD-646) of integers.
- **Nested types:** number bounds (REG-FLD-646).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-601: data component predicate `potion_contents`

- **Reached from:** data component predicate (REG-FLD-597).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `potions` | `potion` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Write form notes:**
  - `potions` and `effects` are optional and have no default.
  - `effects` is a collection predicate (REG-FLD-612) with mob effects predicate (REG-FLD-622) elements.
- **Nested types:** collection predicate (REG-FLD-612), mob effects predicate (REG-FLD-622).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-602: data component predicate `custom_data`

- **Reached from:** data component predicate (REG-FLD-597).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is an nbt predicate (REG-FLD-623).
- **Nested types:** nbt predicate (REG-FLD-623).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-603: data component predicate `container`, `bundle_contents`

- **Reached from:** data component predicate (REG-FLD-597).
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - `items` is optional and has no default. It is a collection predicate (REG-FLD-612) with item predicate (REG-FLD-613) elements.
- **Nested types:** collection predicate (REG-FLD-612), item predicate (REG-FLD-613).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-604: data component predicate `firework_explosion`

- **Reached from:** data component predicate (REG-FLD-597), data component predicate `fireworks` (REG-FLD-605) `explosions`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `shape`, `has_twinkle` and `has_trail` are optional and have no default. `shape` is a shape string.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-605: data component predicate `fireworks`

- **Reached from:** data component predicate (REG-FLD-597).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `flight_duration` | `{}` (any) | no |

- **Write form notes:**
  - `flight_duration` is a number bounds (REG-FLD-646) of integers.
  - `explosions` is optional and has no default. It is a collection predicate (REG-FLD-612) with data component predicate `firework_explosion` (REG-FLD-604) elements.
- **Nested types:** number bounds (REG-FLD-646), collection predicate (REG-FLD-612), data component predicate `firework_explosion` (REG-FLD-604).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-606: data component predicate `writable_book_content`

- **Reached from:** data component predicate (REG-FLD-597).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `pages` is optional and has no default. It is a collection predicate (REG-FLD-612) with string elements.
- **Nested types:** collection predicate (REG-FLD-612).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-607: data component predicate `written_book_content`

- **Reached from:** data component predicate (REG-FLD-597).
- **References:** none at this level.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `generation` | `{}` (any) | no |

- **Write form notes:**
  - `generation` is a number bounds (REG-FLD-646) of integers.
  - `pages` is optional and has no default. It is a collection predicate (REG-FLD-612) with text component (REG-FLD-429) elements.
  - `author`, `title` and `resolved` are optional and have no default.
- **Nested types:** number bounds (REG-FLD-646), collection predicate (REG-FLD-612), text component (REG-FLD-429).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-608: data component predicate `attribute_modifiers`

- **Reached from:** data component predicate (REG-FLD-597).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `modifiers` element `attribute` | `attribute` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `modifiers` element `amount` | `{}` (any) | no |

- **Write form notes:**
  - `modifiers` is optional and has no default. It is a collection predicate (REG-FLD-612) of entry predicates.
  - In an entry predicate, `amount` is a number bounds (REG-FLD-646) of doubles. `id`, `operation` and `slot` are optional and have no default.
- **Nested types:** collection predicate (REG-FLD-612), number bounds (REG-FLD-646).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-609: data component predicate `trim`

- **Reached from:** data component predicate (REG-FLD-597).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `material` | `trim_material` | entry set | yes | no | bare value for one element |
| `pattern` | `trim_pattern` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-610: data component predicate `jukebox_playable`

- **Reached from:** data component predicate (REG-FLD-597).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `song` | `jukebox_song` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-611: data component predicate `villager/variant`

- **Reached from:** data component predicate (REG-FLD-597).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (whole value) | `villager_type` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-612: collection predicate

- **Reached from:** data component predicate `container`, `bundle_contents` (REG-FLD-603), data component predicate `potion_contents` (REG-FLD-601), data component predicate `fireworks` (REG-FLD-605), data component predicate `writable_book_content` (REG-FLD-606), data component predicate `written_book_content` (REG-FLD-607), data component predicate `attribute_modifiers` (REG-FLD-608).
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - `contains`, `count` and `size` are optional and have no default.
  - `contains` is always a list of element predicates.
  - `count` is always a list of objects `{"test": ..., "count": ...}`. `test` and `count` are required.
  - `size` is a number bounds (REG-FLD-646) of integers.
  - The parent rule gives the form of the element predicate.
- **Nested types:** number bounds (REG-FLD-646).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-613: item predicate

- **Reached from:** item component `lock` (REG-FLD-584), data component predicate `container`, `bundle_contents` (REG-FLD-603), entity predicate `equipment` (REG-FLD-632), entity predicate `slots` (REG-FLD-636), loot conditions.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `items` | `item` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `count` | `{}` (any) | no |
| `components` | `{}` | no |
| `predicates` | `{}` | no |

- **Write form notes:**
  - `count` is a number bounds (REG-FLD-646) of integers.
  - `components` and `predicates` come from data component matchers (REG-FLD-596). They are written in this object.
- **Nested types:** number bounds (REG-FLD-646), data component matchers (REG-FLD-596).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-614: block predicate

- **Reached from:** item components `can_place_on`, `can_break` (REG-FLD-517), location predicate (REG-FLD-618) `block`, loot conditions.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `blocks` | `block` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `components` | `{}` | no |
| `predicates` | `{}` | no |

- **Write form notes:**
  - `components` and `predicates` come from data component matchers (REG-FLD-596). They are written in this object.
  - `state` is optional and has no default. It is a state properties predicate (REG-FLD-615).
  - `nbt` is optional and has no default. It is an nbt predicate (REG-FLD-623).
- **Nested types:** data component matchers (REG-FLD-596), state properties predicate (REG-FLD-615), nbt predicate (REG-FLD-623).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-615: state properties predicate

- **Reached from:** block predicate (REG-FLD-614) `state`, loot conditions.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is one object. Each key is a property name. Each value is an exact value string or an object `{"min": ..., "max": ...}`.
  - `min` and `max` are optional.
  - The encoder writes an exact value as a bare string.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-616: fluid predicate

- **Reached from:** location predicate (REG-FLD-618) `fluid`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `fluids` | `fluid` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Write form notes:**
  - `state` is optional and has no default. It is a state properties predicate (REG-FLD-615).
- **Nested types:** state properties predicate (REG-FLD-615).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-617: light predicate

- **Reached from:** location predicate (REG-FLD-618) `light`.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `light` | `{}` (any) | no |

- **Write form notes:**
  - `light` is a number bounds (REG-FLD-646) of integers.
- **Nested types:** number bounds (REG-FLD-646).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-618: location predicate

- **Reached from:** entity predicate `location`, `stepping_on`, `movement_affected_by` (REG-FLD-626), loot conditions.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `biomes` | `worldgen/biome` | entry set | yes | no | bare value for one element |
| `structures` | `worldgen/structure` | entry set | yes | no | bare value for one element |
| `dimension` | `dimension` | key only | no | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `position.x` | `{}` (any) | no |
| `position.y` | `{}` (any) | no |
| `position.z` | `{}` (any) | no |

- **Write form notes:**
  - `position.x`, `position.y` and `position.z` are number bounds (REG-FLD-646) of doubles.
  - `position`, `smokey`, `light`, `block`, `fluid` and `can_see_sky` are optional and have no default.
- **Nested types:** number bounds (REG-FLD-646), block predicate (REG-FLD-614), fluid predicate (REG-FLD-616), light predicate (REG-FLD-617).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-619: damage source predicate

- **Reached from:** damage predicate (REG-FLD-620) `type`, loot condition `damage_source_properties`, enchantment effect requirements through loot conditions.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `tags[].id` | `damage_type` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `tags` | `[]` | no |

- **Write form notes:**
  - `tags[].expected` is required.
  - `direct_entity` and `source_entity` are optional entity predicate (REG-FLD-624) values. They have no default.
  - `is_direct` is optional and has no default.
- **Nested types:** entity predicate (REG-FLD-624).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-620: damage predicate

- **Reached from:** advancement criteria. No synced registry reaches this type.
- **References:** none at this level.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `dealt` | `{}` (any) | no |
| `taken` | `{}` (any) | no |

- **Write form notes:**
  - `dealt` and `taken` are number bounds (REG-FLD-646) of doubles.
  - `source_entity` is an optional entity predicate (REG-FLD-624). `type` is an optional damage source predicate (REG-FLD-619). They have no default.
- **Nested types:** number bounds (REG-FLD-646), entity predicate (REG-FLD-624), damage source predicate (REG-FLD-619).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-621: distance predicate

- **Reached from:** entity predicate `distance` (REG-FLD-627), loot conditions.
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `x` | `{}` (any) | no |
| `y` | `{}` (any) | no |
| `z` | `{}` (any) | no |
| `horizontal` | `{}` (any) | no |
| `absolute` | `{}` (any) | no |

- **Write form notes:**
  - Each field is a number bounds (REG-FLD-646) of doubles.
- **Nested types:** number bounds (REG-FLD-646).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-622: mob effects predicate

- **Reached from:** entity predicate `effects` (REG-FLD-629), data component predicate `potion_contents` (REG-FLD-601) `effects`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| map keys | `mob_effect` | key only | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `<effect>.amplifier` | `{}` (any) | no |
| `<effect>.duration` | `{}` (any) | no |

- **Write form notes:**
  - The value is one object. Each key is a mob effect key.
  - `amplifier` and `duration` are number bounds (REG-FLD-646) of integers.
  - `ambient` and `visible` are optional and have no default.
- **Nested types:** number bounds (REG-FLD-646).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-623: nbt predicate

- **Reached from:** data component predicate `custom_data` (REG-FLD-602), block predicate (REG-FLD-614) `nbt`, entity predicate `nbt` (REG-FLD-630).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a compound tag. A reader accepts an SNBT string or a JSON object. The encoder always writes an SNBT string.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-624: entity predicate

- **Reached from:** damage source predicate (REG-FLD-619), damage predicate (REG-FLD-620), entity predicate `vehicle`, `passenger`, `targeted_entity` (REG-FLD-634), entity predicate `type_specific/lightning` (REG-FLD-640), entity predicate `type_specific/player` (REG-FLD-642), loot conditions such as `entity_properties`.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| map keys | `entity_sub_predicate_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The value is one object. Each key is a sub-predicate type id. The value has the form of the rule "entity predicate `<id>`".
  - The sub-predicate type ids are `entity_type`, `location`, `stepping_on`, `movement_affected_by`, `distance`, `movement`, `effects`, `nbt`, `flags`, `equipment`, `periodic_tick`, `vehicle`, `passenger`, `targeted_entity`, `team`, `slots`, `components`, `predicates`, `entity_tags`, `type_specific/lightning`, `type_specific/fishing_hook`, `type_specific/player`, `type_specific/cube_mob`, `type_specific/raider` and `type_specific/sheep`.
- **Nested types:** entity predicate `entity_type` (REG-FLD-625), entity predicate `location`, `stepping_on`, `movement_affected_by` (REG-FLD-626), entity predicate `distance` (REG-FLD-627), entity predicate `movement` (REG-FLD-628), entity predicate `effects` (REG-FLD-629), entity predicate `nbt` (REG-FLD-630), entity predicate `flags` (REG-FLD-631), entity predicate `equipment` (REG-FLD-632), entity predicate `periodic_tick` (REG-FLD-633), entity predicate `vehicle`, `passenger`, `targeted_entity` (REG-FLD-634), entity predicate `team` (REG-FLD-635), entity predicate `slots` (REG-FLD-636), entity predicate `components` (REG-FLD-637), entity predicate `predicates` (REG-FLD-638), entity predicate `entity_tags` (REG-FLD-639), entity predicate `type_specific/lightning` (REG-FLD-640), entity predicate `type_specific/fishing_hook` (REG-FLD-641), entity predicate `type_specific/player` (REG-FLD-642), entity predicate `type_specific/cube_mob` (REG-FLD-643), entity predicate `type_specific/raider` (REG-FLD-644), entity predicate `type_specific/sheep` (REG-FLD-645).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-625: entity predicate `entity_type`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (whole value) | `entity_type` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-626: entity predicate `location`, `stepping_on`, `movement_affected_by`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a location predicate (REG-FLD-618).
- **Nested types:** location predicate (REG-FLD-618).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-627: entity predicate `distance`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a distance predicate (REG-FLD-621).
- **Nested types:** distance predicate (REG-FLD-621).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-628: entity predicate `movement`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `x` | `{}` (any) | no |
| `y` | `{}` (any) | no |
| `z` | `{}` (any) | no |
| `speed` | `{}` (any) | no |
| `horizontal_speed` | `{}` (any) | no |
| `vertical_speed` | `{}` (any) | no |
| `fall_distance` | `{}` (any) | no |

- **Write form notes:**
  - Each field is a number bounds (REG-FLD-646) of doubles.
- **Nested types:** number bounds (REG-FLD-646).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-629: entity predicate `effects`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a mob effects predicate (REG-FLD-622).
- **Nested types:** mob effects predicate (REG-FLD-622).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-630: entity predicate `nbt`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is an nbt predicate (REG-FLD-623).
- **Nested types:** nbt predicate (REG-FLD-623).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-631: entity predicate `flags`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `is_on_ground`, `is_on_fire`, `is_sneaking`, `is_sprinting`, `is_swimming`, `is_flying`, `is_baby`, `is_in_water` and `is_fall_flying` are optional booleans. They have no default.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-632: entity predicate `equipment`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - `head`, `chest`, `legs`, `feet`, `body`, `mainhand` and `offhand` are optional item predicate (REG-FLD-613) values. They have no default.
- **Nested types:** item predicate (REG-FLD-613).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-633: entity predicate `periodic_tick`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a positive integer.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-634: entity predicate `vehicle`, `passenger`, `targeted_entity`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is an entity predicate (REG-FLD-624). The type contains itself.
- **Nested types:** entity predicate (REG-FLD-624).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-635: entity predicate `team`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a scoreboard team name string. It is not a reference.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-636: entity predicate `slots`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is one object. Each key is a slot range name string. It is not a reference. Each value is an item predicate (REG-FLD-613).
- **Nested types:** item predicate (REG-FLD-613).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-637: entity predicate `components`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a data component exact predicate (REG-FLD-595).
- **Nested types:** data component exact predicate (REG-FLD-595).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-638: entity predicate `predicates`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none at this level.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a data component predicate (REG-FLD-597).
- **Nested types:** data component predicate (REG-FLD-597).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-639: entity predicate `entity_tags`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `any_of`, `all_of` and `none_of` are optional and have no default. Each is always a list of strings.
  - The strings are scoreboard entity tags. They are not registry tags and not references.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-640: entity predicate `type_specific/lightning`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none at this level.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `blocks_set_on_fire` | `{}` (any) | no |

- **Write form notes:**
  - `blocks_set_on_fire` is a number bounds (REG-FLD-646) of integers.
  - `entity_struck` is an optional entity predicate (REG-FLD-624). It has no default.
- **Nested types:** number bounds (REG-FLD-646), entity predicate (REG-FLD-624).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-641: entity predicate `type_specific/fishing_hook`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `in_open_water` is an optional boolean. It has no default.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-642: entity predicate `type_specific/player`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `stats[].type` | `stat_type` | key only | yes | — | — |
| `stats[].stat` | the registry of the stat type, for example `item`, `block`, `entity_type` or `custom_stat` | key only | yes | — | — |
| `recipes` map keys | `recipe` | key only | no | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `level` | `{}` (any) | no |
| `food` | `{}` | no |
| `gamemode` | `["survival", "creative", "adventure", "spectator"]` | no |
| `stats` | `[]` | no |
| `recipes` | `{}` | no |
| `advancements` | `{}` | no |
| `stats[].value` | `{}` (any) | no |

- **Write form notes:**
  - `level` and `stats[].value` are number bounds (REG-FLD-646) of integers.
  - `food` has the fields `level` and `saturation`. Each is a number bounds (REG-FLD-646) with the default `{}`. Each is left out when it equals the default.
  - `gamemode` is left out only when the list holds these four strings in this order.
  - `advancements` map keys are advancement identifiers. They are not references.
  - `looking_at` is an optional entity predicate (REG-FLD-624). It has no default.
  - `input` is optional and has no default. Its fields `forward`, `backward`, `left`, `right`, `jump`, `sneak` and `sprint` are optional booleans.
- **Nested types:** number bounds (REG-FLD-646), entity predicate (REG-FLD-624).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-643: entity predicate `type_specific/cube_mob`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `size` | `{}` (any) | no |

- **Write form notes:**
  - `size` is a number bounds (REG-FLD-646) of integers.
- **Nested types:** number bounds (REG-FLD-646).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-644: entity predicate `type_specific/raider`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `has_raid` | `false` | no |
| `is_captain` | `false` | no |

- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-645: entity predicate `type_specific/sheep`

- **Reached from:** entity predicate (REG-FLD-624).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `sheared` is an optional boolean. It has no default.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-646: number bounds

- **Reached from:** every predicate field in this chapter with the default `{}` (any), and collection predicate (REG-FLD-612) `size`.
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a range of integers, of doubles, or of float degrees. The parent rule gives the number kind.
  - The encoder writes a bare number when `min` equals `max`. Else it writes an object `{"min": ..., "max": ...}`.
  - `min` and `max` are optional. The range "any" has no bound and is written as `{}`.
- **Nested types:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

## 9. Loot conditions, context number providers and item modifiers

This chapter defines the references in loot conditions, context number providers, item modifiers, loot tables and slot sources.
These types are values of stage 3 registries: `predicate`, `item_modifier`, `loot_table`, `context_int_provider`, `context_float_provider` and `slot_source`.
These registries are not synced. In a synced entry, a key in a `reference or inline` field of these registries fails the load (REG-REF-002), so only inline values occur there.
None of these types has a network form that differs from the file form.

### REG-FLD-700: loot condition

- **Reached from:** every `condition`, `requirements`, `term` and `terms` field in this chapter; the `requirements` field of conditional effects in Registry `enchantment` (REG-FLD-122); villager trade entries; advancement criteria.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (whole value) | `predicate` | reference or inline | yes | — | — |
| `type` (inline value) | `loot_condition_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - A key is written as a key string. An inline value is written as an object with a `type` field.
  - A string that parses as an identifier is always read as a key.
- **Nested types:** loot condition `inverted` (REG-FLD-702), loot condition `any_of`, `all_of` (REG-FLD-703), loot condition `random_chance` (REG-FLD-704), loot condition `random_chance_with_enchanted_bonus` (REG-FLD-705), loot condition `entity_properties` (REG-FLD-706), loot condition `killed_by_player`, `survives_explosion` (REG-FLD-707), loot condition `entity_scores` (REG-FLD-708), loot condition `match_block` (REG-FLD-709), loot condition `match_tool` (REG-FLD-710), loot condition `table_bonus` (REG-FLD-711), loot condition `damage_source_properties` (REG-FLD-712), loot condition `location_check` (REG-FLD-713), loot condition `weather_check` (REG-FLD-714), loot condition `time_check` (REG-FLD-715), loot condition `int_value_check` (REG-FLD-716), loot condition `float_value_check` (REG-FLD-717), loot condition `enchantment_active_check` (REG-FLD-718), loot condition `environment_attribute_check` (REG-FLD-719).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-701: loot condition list

- **Reached from:** loot condition `any_of`, `all_of` (REG-FLD-703).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (whole value) | `predicate` | entry set | yes | yes | bare value for one element |

- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a `#` tag, a key, an inline loot condition (REG-FLD-700) object, or a list of keys and inline objects.
- **Nested types:** loot condition (REG-FLD-700).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-702: loot condition `inverted`

- **Reached from:** loot condition (REG-FLD-700).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `term` | `predicate` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Nested types:** loot condition (REG-FLD-700).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-703: loot condition `any_of`, `all_of`

- **Reached from:** loot condition (REG-FLD-700).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `terms` | `predicate` | entry set | yes | yes | bare value for one element |

- **Fields left out on write:** none.
- **Nested types:** loot condition list (REG-FLD-701).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-704: loot condition `random_chance`

- **Reached from:** loot condition (REG-FLD-700).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `chance` | `context_float_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Nested types:** context float provider (REG-FLD-723).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-705: loot condition `random_chance_with_enchanted_bonus`

- **Reached from:** loot condition (REG-FLD-700).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `enchantment` | `enchantment` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `unenchanted_chance` is a required float in the range 0 to 1.
- **Nested types:** level-based value (REG-FLD-386) (`enchanted_chance`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-706: loot condition `entity_properties`

- **Reached from:** loot condition (REG-FLD-700).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `entity` is an enum string: `this`, `attacker`, `direct_attacker`, `attacking_player`, `target_entity` or `interacting_entity`. It is not a reference.
  - `predicate` is optional and has no default.
- **Nested types:** entity predicate (REG-FLD-624) (`predicate`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-707: loot condition `killed_by_player`, `survives_explosion`

- **Reached from:** loot condition (REG-FLD-700).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - These types have no fields other than `type`.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-708: loot condition `entity_scores`

- **Reached from:** loot condition (REG-FLD-700).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `scores` is a map from an objective name string to an int range. An objective name is not a reference.
  - `entity` is the entity target enum string of loot condition `entity_properties` (REG-FLD-706).
- **Nested types:** int range predicate (REG-FLD-720) (`scores.<name>`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-709: loot condition `match_block`

- **Reached from:** loot condition (REG-FLD-700).
- **References:** none of its own.
- **Fields left out on write:** none of its own.
- **Write form notes:**
  - The fields of the block predicate are written in the parent object. There is no wrapper field.
- **Nested types:** block predicate (REG-FLD-614) (fields at this level).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-710: loot condition `match_tool`

- **Reached from:** loot condition (REG-FLD-700).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `predicate` is optional and has no default.
- **Nested types:** item predicate (REG-FLD-613) (`predicate`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-711: loot condition `table_bonus`

- **Reached from:** loot condition (REG-FLD-700).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `enchantment` | `enchantment` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `chances` is a non-empty list of floats. It is always a list.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-712: loot condition `damage_source_properties`

- **Reached from:** loot condition (REG-FLD-700).
- **References:** none.
- **Fields left out on write:** none.
- **Nested types:** damage source predicate (REG-FLD-619) (`predicate`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-713: loot condition `location_check`

- **Reached from:** loot condition (REG-FLD-700).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `offsetX` | `0` | no |
| `offsetY` | `0` | no |
| `offsetZ` | `0` | no |

- **Write form notes:**
  - `predicate` is optional and has no default.
- **Nested types:** location predicate (REG-FLD-618) (`predicate`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-714: loot condition `weather_check`

- **Reached from:** loot condition (REG-FLD-700).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `raining` and `thundering` are optional and have no default.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-715: loot condition `time_check`

- **Reached from:** loot condition (REG-FLD-700).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `clock` | `world_clock` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `period` is an optional long and has no default.
- **Nested types:** int range predicate (REG-FLD-720) (`value`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-716: loot condition `int_value_check`

- **Reached from:** loot condition (REG-FLD-700).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `value` | `context_int_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Nested types:** context int provider (REG-FLD-722) (`value`), int range predicate (REG-FLD-720) (`test`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-717: loot condition `float_value_check`

- **Reached from:** loot condition (REG-FLD-700).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `value` | `context_float_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Nested types:** context float provider (REG-FLD-723) (`value`), float range predicate (REG-FLD-721) (`test`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-718: loot condition `enchantment_active_check`

- **Reached from:** loot condition (REG-FLD-700).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `active` is a required boolean.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-719: loot condition `environment_attribute_check`

- **Reached from:** loot condition (REG-FLD-700).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `attribute` | `environment_attribute` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The form of `value` is the value form of the attribute that `attribute` names (environment attribute value (REG-FLD-254)).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-720: int range predicate

- **Reached from:** loot condition `entity_scores` (REG-FLD-708) (`scores.<name>`), loot condition `time_check` (REG-FLD-715) (`value`), loot condition `int_value_check` (REG-FLD-716) (`test`).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (point form, whole value) | `context_int_provider` | reference or inline | yes | — | — |
| `min` | `context_int_provider` | reference or inline | yes | — | — |
| `max` | `context_int_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The value is either one context int provider (the point form) or an object with optional `min` and `max` (the range form).
  - The encoder writes the point form for a point and the range form for a range.
  - `min` and `max` have no default.
- **Nested types:** context int provider (REG-FLD-722).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-721: float range predicate

- **Reached from:** loot condition `float_value_check` (REG-FLD-717) (`test`).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (point form, whole value) | `context_float_provider` | reference or inline | yes | — | — |
| `min` | `context_float_provider` | reference or inline | yes | — | — |
| `max` | `context_float_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The shape is the shape of int range predicate (REG-FLD-720), with context float providers.
- **Nested types:** context float provider (REG-FLD-723).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-722: context int provider

- **Reached from:** every context int provider field in this chapter; villager trade entries, trade sets and trade costs.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (whole value) | `context_int_provider` | reference or inline | yes | — | — |
| `type` (object form) | `context_int_provider_type` | key only | yes | — | — |
| (list form, whole value) | `context_int_provider` | entry set | yes | yes | bare value for one element |

- **Fields left out on write:** none.
- **Write form notes:**
  - An inline value is a bare int or an object with a `type` field.
  - A `constant` provider is always written as a bare int. The encoder never writes `{"type":"constant","value":N}`.
  - The list form is used where a field takes several providers.
- **Nested types:** context int provider `constant` and context float provider `constant` (REG-FLD-724), context int provider unary types and context float provider unary types (REG-FLD-725), context int provider binary types and context float provider binary types (REG-FLD-726), context int provider `pow` and context float provider `pow` (REG-FLD-727), context int provider `uniform` and context float provider `uniform` (REG-FLD-728), context int provider aggregate types and context float provider aggregate types (REG-FLD-729), context int provider `conditional` and context float provider `conditional` (REG-FLD-730), context int provider `number_dispatcher` and context float provider `number_dispatcher` (REG-FLD-731), context int provider `weighted_list` and context float provider `weighted_list` (REG-FLD-732), context int provider `environment_attribute` and context float provider `environment_attribute` (REG-FLD-733), context int provider `storage` and context float provider `storage` (REG-FLD-734), context int provider `binomial` (REG-FLD-735), context int provider `score` (REG-FLD-736).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-723: context float provider

- **Reached from:** every context float provider field in this chapter.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (whole value) | `context_float_provider` | reference or inline | yes | — | — |
| `type` (object form) | `context_float_provider_type` | key only | yes | — | — |
| (list form, whole value) | `context_float_provider` | entry set | yes | yes | bare value for one element |

- **Fields left out on write:** none.
- **Write form notes:**
  - An inline value is a bare float or an object with a `type` field.
  - A `constant` provider is always written as a bare float. The encoder never writes the object form for it.
- **Nested types:** context int provider `constant` and context float provider `constant` (REG-FLD-724), context int provider unary types and context float provider unary types (REG-FLD-725), context int provider binary types and context float provider binary types (REG-FLD-726), context int provider `pow` and context float provider `pow` (REG-FLD-727), context int provider `uniform` and context float provider `uniform` (REG-FLD-728), context int provider aggregate types and context float provider aggregate types (REG-FLD-729), context int provider `conditional` and context float provider `conditional` (REG-FLD-730), context int provider `number_dispatcher` and context float provider `number_dispatcher` (REG-FLD-731), context int provider `weighted_list` and context float provider `weighted_list` (REG-FLD-732), context int provider `environment_attribute` and context float provider `environment_attribute` (REG-FLD-733), context int provider `storage` and context float provider `storage` (REG-FLD-734), context float provider `enchantment_level` (REG-FLD-737).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-724: context int provider `constant` and context float provider `constant`

- **Reached from:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `value` is a required int or float.
  - This object form occurs only on read. The encoder writes a bare number instead.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-725: context int provider unary types and context float provider unary types

- **Reached from:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `input` | `context_int_provider` or `context_float_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The int types are `abs`, `negate` and `from_float`.
  - The float types are `abs`, `ceil`, `cos`, `floor`, `negate`, `round`, `sin`, `sqrt`, `truncate` and `from_int`.
  - `input` targets the registry of the owner type. Exceptions: `from_float` targets `context_float_provider`, and `from_int` targets `context_int_provider`.
- **Nested types:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-726: context int provider binary types and context float provider binary types

- **Reached from:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `left` | registry of the owner type | reference or inline | yes | — | — |
| `right` | registry of the owner type | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The int types are `sub`, `mod`, `floor_mod`, `div` and `floor_div`.
  - The float types are `sub`, `mod` and `div`.
  - "Registry of the owner type" is `context_int_provider` for an int type and `context_float_provider` for a float type.
- **Nested types:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-727: context int provider `pow` and context float provider `pow`

- **Reached from:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `base` | registry of the owner type | reference or inline | yes | — | — |
| `exponent` | registry of the owner type | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Nested types:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-728: context int provider `uniform` and context float provider `uniform`

- **Reached from:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `min` | registry of the owner type | reference or inline | yes | — | — |
| `max` | registry of the owner type | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Nested types:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-729: context int provider aggregate types and context float provider aggregate types

- **Reached from:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `inputs` | registry of the owner type | entry set | yes | yes | bare value for one element |

- **Fields left out on write:** none.
- **Write form notes:**
  - The int types are `avg`, `max`, `min`, `mul` and `add`.
  - The float types are `avg`, `length`, `max`, `min`, `mul` and `add`.
  - `inputs` is a `#` tag, a key, an inline value, or a list.
- **Nested types:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-730: context int provider `conditional` and context float provider `conditional`

- **Reached from:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `condition` | `predicate` | reference or inline | yes | — | — |
| `on_true` | registry of the owner type | reference or inline | yes | — | — |
| `on_false` | registry of the owner type | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `on_false` | `0` (int type) or `0.0` (float type), an inline constant | no |

- **Write form notes:**
  - An inline `0` or an inline `{"type":"constant","value":0}` equals the default.
  - A key equals no inline default, even when the entry value is 0. Such a key is always written.
- **Nested types:** loot condition (REG-FLD-700), context int provider (REG-FLD-722), context float provider (REG-FLD-723).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-731: context int provider `number_dispatcher` and context float provider `number_dispatcher`

- **Reached from:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `cases[].condition` | `predicate` | reference or inline | yes | — | — |
| `cases[].value` | registry of the owner type | reference or inline | yes | — | — |
| `default` | registry of the owner type | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `default` | `0` (int type) or `0.0` (float type), an inline constant | no |

- **Write form notes:**
  - `cases` is required. It is always a list.
  - The default comparison follows context int provider `conditional` and context float provider `conditional` (REG-FLD-730): a key is always written.
- **Nested types:** loot condition (REG-FLD-700), context int provider (REG-FLD-722), context float provider (REG-FLD-723).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-732: context int provider `weighted_list` and context float provider `weighted_list`

- **Reached from:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `distribution[].data` | registry of the owner type | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `distribution` is a non-empty list of `{data, weight}` objects. It is always a list.
- **Nested types:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-733: context int provider `environment_attribute` and context float provider `environment_attribute`

- **Reached from:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `attribute` | `environment_attribute` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The load also fails when the attribute value does not convert to an int (int type) or a float (float type).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-734: context int provider `storage` and context float provider `storage`

- **Reached from:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `fallback` | registry of the owner type | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `fallback` | `0` (int type) or `0.0` (float type), an inline constant | no |

- **Write form notes:**
  - `storage` and `path` are written in the parent object.
  - `storage` is an identifier of a command storage. It is not a reference.
  - `path` is an NBT path string.
  - The default comparison follows context int provider `conditional` and context float provider `conditional` (REG-FLD-730): a key is always written.
- **Nested types:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-735: context int provider `binomial`

- **Reached from:** context int provider (REG-FLD-722).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `n` | `context_int_provider` | reference or inline | yes | — | — |
| `p` | `context_float_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Nested types:** context int provider (REG-FLD-722), context float provider (REG-FLD-723).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-736: context int provider `score`

- **Reached from:** context int provider (REG-FLD-722).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `fallback` | `context_int_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `fallback` | `0`, an inline constant | no |

- **Write form notes:**
  - `score` is an objective name string. It is not a reference.
  - The default comparison follows context int provider `conditional` and context float provider `conditional` (REG-FLD-730): a key is always written.
- **Nested types:** context int provider (REG-FLD-722), scoreboard name provider (REG-FLD-738) (`target`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-737: context float provider `enchantment_level`

- **Reached from:** context float provider (REG-FLD-723).
- **References:** none.
- **Fields left out on write:** none.
- **Nested types:** level-based value (REG-FLD-386) (`amount`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-738: scoreboard name provider

- **Reached from:** context int provider `score` (REG-FLD-736) (`target`).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` (object form) | `loot_score_provider_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a bare entity target string or an object with a `type` field.
  - A `context` provider is always written as the bare entity target string.
  - The type `fixed` has the field `name`, a string. The type `context` has the field `target`, the entity target enum string.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-739: resolvable int

- **Reached from:** item component `compostable` (REG-FLD-587) (`layers`), item component `cooking_fuel` (REG-FLD-588) (`burn_time`), item component `brewing_fuel` (REG-FLD-589) (`uses`).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (string form) | `context_int_provider` | key only | no | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a bare int or a key string.
  - The key is kept unchecked, so a key in a synced entry does not fail the load.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-740: resolvable float

- **Reached from:** item component `cooking_fuel` (REG-FLD-588) (`speed_multiplier`), item component `brewing_fuel` (REG-FLD-589) (`speed_multiplier`).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (string form) | `context_float_provider` | key only | no | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a bare float or a key string.
  - The key is kept unchecked, so a key in a synced entry does not fail the load.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-741: item modifier

- **Reached from:** item modifier `filtered` (REG-FLD-762) (`on_pass`, `on_fail`), item modifier `modify_contents` (REG-FLD-761) (`modifier`), loot table (REG-FLD-799), loot pool (REG-FLD-800), loot pool entry common fields (REG-FLD-791) (`modifier`); villager trade entries.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (whole value) | `item_modifier` | reference or inline | yes | — | — |
| `type` (object form) | `loot_function_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - A key is written as a key string. An inline value is an object with a `type` field, or the bare entry set of a `sequence`.
  - A `sequence` without `condition` is always written as the bare `functions` entry set. See item modifier `sequence` (REG-FLD-780).
  - Every other inline value is written as an object with a `type` field.
- **Nested types:** item modifier common fields (REG-FLD-742) and one rule per type id: item modifier `set_count` (REG-FLD-743), item modifier `set_item` (REG-FLD-744), item modifier `enchant_with_levels` (REG-FLD-745), item modifier `enchant_randomly` (REG-FLD-746), item modifier `set_enchantments` (REG-FLD-747), item modifier `set_custom_data` (REG-FLD-748), item modifier `set_components` (REG-FLD-749), item modifier `furnace_smelt` (REG-FLD-750), item modifier `enchanted_count_increase` (REG-FLD-751), item modifier `set_damage` (REG-FLD-752), item modifier `set_attributes` (REG-FLD-753), item modifier `set_name` (REG-FLD-755), item modifier `exploration_map` (REG-FLD-756), item modifier `set_stew_effect` (REG-FLD-757), item modifier `copy_name` (REG-FLD-759), item modifier `set_contents` (REG-FLD-760), item modifier `modify_contents` (REG-FLD-761), item modifier `filtered` (REG-FLD-762), item modifier `limit_count` (REG-FLD-763), item modifier `apply_bonus` (REG-FLD-765), item modifier `set_loot_table` (REG-FLD-766), item modifier `explosion_decay`, `discard` (REG-FLD-767), item modifier `set_lore` (REG-FLD-768), item modifier `fill_player_head` (REG-FLD-771), item modifier `copy_custom_data` (REG-FLD-772), item modifier `copy_state` (REG-FLD-774), item modifier `set_banner_pattern` (REG-FLD-775), item modifier `set_potion` (REG-FLD-776), item modifier `set_random_dyes` (REG-FLD-777), item modifier `set_random_potion` (REG-FLD-778), item modifier `set_instrument` (REG-FLD-779), item modifier `sequence` (REG-FLD-780), item modifier `copy_components` (REG-FLD-781), item modifier `set_fireworks` (REG-FLD-782), item modifier `set_firework_explosion` (REG-FLD-783), item modifier `set_book_cover` (REG-FLD-784), item modifier `set_written_book_pages` (REG-FLD-785), item modifier `set_writable_book_pages` (REG-FLD-786), item modifier `toggle_tooltips` (REG-FLD-787), item modifier `set_ominous_bottle_amplifier` (REG-FLD-788), item modifier `set_custom_model_data` (REG-FLD-789).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-742: item modifier common fields

- **Reached from:** every item modifier type rule.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `condition` | `predicate` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `condition` is optional and has no default.
  - The item modifier type rules list only the fields specific to the type.
- **Nested types:** loot condition (REG-FLD-700).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-743: item modifier `set_count`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `count` | `context_int_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `add` | `false` | no |

- **Nested types:** context int provider (REG-FLD-722).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-744: item modifier `set_item`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `item` | `item` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The load fails for `minecraft:air`.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-745: item modifier `enchant_with_levels`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `levels` | `context_int_provider` | reference or inline | yes | — | — |
| `options` | `enchantment` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `include_additional_cost_component` | `false` | no |

- **Write form notes:**
  - `options` is optional and has no default.
- **Nested types:** context int provider (REG-FLD-722).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-746: item modifier `enchant_randomly`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `options` | `enchantment` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `only_compatible` | `true` | no |
| `include_additional_cost_component` | `false` | no |

- **Write form notes:**
  - `options` is optional and has no default.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-747: item modifier `set_enchantments`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `enchantments` (map keys) | `enchantment` | key only | yes | — | — |
| `enchantments.<key>` (map values) | `context_int_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `enchantments` | `{}` | no |
| `add` | `false` | no |

- **Nested types:** context int provider (REG-FLD-722).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-748: item modifier `set_custom_data`

- **Reached from:** item modifier (REG-FLD-741).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `tag` is an SNBT string or an NBT compound. It contains no references.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-749: item modifier `set_components`

- **Reached from:** item modifier (REG-FLD-741).
- **References:** none of its own.
- **Fields left out on write:** none.
- **Write form notes:**
  - `components` is a data component patch (REG-FLD-502). Each entry follows the item component rule for its component id.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-750: item modifier `furnace_smelt`

- **Reached from:** item modifier (REG-FLD-741).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `use_input_count` | `true` | no |

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-751: item modifier `enchanted_count_increase`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `enchantment` | `enchantment` | key only | yes | — | — |
| `count` | `context_float_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `limit` | `0` | no |

- **Nested types:** context float provider (REG-FLD-723).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-752: item modifier `set_damage`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `damage` | `context_float_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `add` | `false` | no |

- **Nested types:** context float provider (REG-FLD-723).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-753: item modifier `set_attributes`

- **Reached from:** item modifier (REG-FLD-741).
- **References:** none of its own.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `replace` | `true` | no |

- **Write form notes:**
  - `modifiers` is always a list.
- **Nested types:** `set_attributes` modifier entry (REG-FLD-754) (`modifiers[]`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-754: `set_attributes` modifier entry

- **Reached from:** item modifier `set_attributes` (REG-FLD-753) (`modifiers[]`).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `attribute` | `attribute` | key only | yes | — | — |
| `amount` | `context_float_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `id` is an identifier of the attribute modifier. It is not a reference.
  - `operation` is an enum string.
  - `slot` is a non-empty list of equipment slot group enum strings. It is not a reference. One element is written as a bare value.
- **Nested types:** context float provider (REG-FLD-723).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-755: item modifier `set_name`

- **Reached from:** item modifier (REG-FLD-741).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `target` | `"custom_name"` | no |

- **Write form notes:**
  - `entity` is the entity target enum string.
  - `target` is the enum string `custom_name` or `item_name`.
  - `name` is optional.
- **Nested types:** text component (REG-FLD-429) (`name`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-756: item modifier `exploration_map`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `destination` | `worldgen/structure` | entry set | yes | no | bare value for one element |
| `decoration` | `map_decoration_type` | key only | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `decoration` | `"minecraft:woodland_mansion"` | no |
| `zoom` | `2` | no |
| `search_radius` | `50` | no |
| `skip_existing_chunks` | `true` | no |

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-757: item modifier `set_stew_effect`

- **Reached from:** item modifier (REG-FLD-741).
- **References:** none of its own.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `effects` | `[]` | no |

- **Write form notes:**
  - `effects` is always a list.
- **Nested types:** `set_stew_effect` effect entry (REG-FLD-758) (`effects[]`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-758: `set_stew_effect` effect entry

- **Reached from:** item modifier `set_stew_effect` (REG-FLD-757) (`effects[]`).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `mob_effect` | key only | yes | — | — |
| `duration` | `context_int_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Nested types:** context int provider (REG-FLD-722).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-759: item modifier `copy_name`

- **Reached from:** item modifier (REG-FLD-741).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `source` is a string from a fixed set: the entity targets and the block entity target. It is not a reference.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-760: item modifier `set_contents`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `component` | `data_component_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `component` accepts only `container`, `bundle_contents` and `charged_projectiles`. Other keys fail the load.
  - `entries` is always a list.
- **Nested types:** loot pool entry (REG-FLD-790) (`entries[]`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-761: item modifier `modify_contents`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `component` | `data_component_type` | key only | yes | — | — |
| `modifier` | `item_modifier` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `component` accepts only `container`, `bundle_contents` and `charged_projectiles`.
- **Nested types:** item modifier (REG-FLD-741).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-762: item modifier `filtered`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `on_pass` | `item_modifier` | reference or inline | yes | — | — |
| `on_fail` | `item_modifier` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `on_pass` and `on_fail` are optional and have no default.
- **Nested types:** item modifier (REG-FLD-741), item predicate (REG-FLD-613) (`item_filter`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-763: item modifier `limit_count`

- **Reached from:** item modifier (REG-FLD-741).
- **References:** none of its own.
- **Fields left out on write:** none.
- **Nested types:** int limit (REG-FLD-764) (`limit`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-764: int limit

- **Reached from:** item modifier `limit_count` (REG-FLD-763) (`limit`).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `min` | `context_int_provider` | reference or inline | yes | — | — |
| `max` | `context_int_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `min` and `max` are optional and have no default.
- **Nested types:** context int provider (REG-FLD-722).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-765: item modifier `apply_bonus`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `enchantment` | `enchantment` | key only | yes | — | — |

- **Fields left out on write:** none with a default.
- **Write form notes:**
  - `formula` is an identifier from a fixed set: `minecraft:binomial_with_bonus_count`, `minecraft:ore_drops` and `minecraft:uniform_bonus_count`. It is not a reference.
  - The `parameters` of `binomial_with_bonus_count` are `extra` (int) and `probability` (float).
  - The `parameters` of `uniform_bonus_count` are `bonusMultiplier` (int). The `parameters` of `ore_drops` are empty.
  - `parameters` is not written when it encodes to an empty object. This is always true for `ore_drops`.
  - On read, an absent `parameters` reads as `{}`.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-766: item modifier `set_loot_table`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `loot_table_id` | `loot_table` | key only | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `seed` | `0` | no |

- **Write form notes:**
  - An inline loot table fails the load. Only a key is accepted.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-767: item modifier `explosion_decay`, `discard`

- **Reached from:** item modifier (REG-FLD-741).
- **References:** none of its own.
- **Fields left out on write:** none.
- **Write form notes:**
  - These types have only the item modifier common fields (REG-FLD-742).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-768: item modifier `set_lore`

- **Reached from:** item modifier (REG-FLD-741).
- **References:** none of its own.
- **Fields left out on write:** none of its own.
- **Write form notes:**
  - `entity` is the optional entity target enum string.
  - `lore` is a list of at most 256 elements. It is always a list.
  - The list operation fields are written in the parent object, with a maximum size of 256.
- **Nested types:** text component (REG-FLD-429) (`lore[]`), list operation (REG-FLD-769).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-769: list operation

- **Reached from:** item modifier `set_lore` (REG-FLD-768) (maximum 256), item modifier `set_writable_book_pages` (REG-FLD-786) (maximum 100), item modifier `set_written_book_pages` (REG-FLD-785) (no maximum), standalone list operation (REG-FLD-770).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `offset` (mode `insert`) | `0` | no |
| `offset` (mode `replace_section`) | `0` | no |

- **Write form notes:**
  - These fields are written in the parent object.
  - `mode` is an enum string: `replace_all`, `replace_section`, `insert` or `append`. It is not a reference.
  - `size` (mode `replace_section`) is optional and has no default.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-770: standalone list operation

- **Reached from:** item modifier `set_custom_model_data` (REG-FLD-789) (`floats`, `flags`, `strings`, `colors`), item modifier `set_fireworks` (REG-FLD-782) (`explosions`).
- **References:** only through `values[]`. The owner rule lists them.
- **Fields left out on write:** none of its own.
- **Write form notes:**
  - `values` is a list with a maximum size that the owner sets. It is always a list.
  - The list operation (REG-FLD-769) fields are written in the same object as `values`.
- **Nested types:** list operation (REG-FLD-769).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-771: item modifier `fill_player_head`

- **Reached from:** item modifier (REG-FLD-741).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `entity` is the entity target enum string.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-772: item modifier `copy_custom_data`

- **Reached from:** item modifier (REG-FLD-741).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `ops` is always a list.
  - Each operation has `source` and `target` (NBT path strings) and `op` (enum string `replace`, `append` or `merge`).
- **Nested types:** nbt provider (REG-FLD-773) (`source`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-773: nbt provider

- **Reached from:** item modifier `copy_custom_data` (REG-FLD-772) (`source`).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` (object form) | `loot_nbt_provider_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - The value is a bare target string or an object with a `type` field.
  - A `context` provider is always written as the bare target string.
  - The type `storage` has the field `source`, an identifier of a command storage. It is not a reference.
  - The type `context` has the field `target`, a string from the entity targets and the block entity target.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-774: item modifier `copy_state`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `block` | `block` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `properties` is a list of property name strings of `block`. It is always a list. The names are not references.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-775: item modifier `set_banner_pattern`

- **Reached from:** item modifier (REG-FLD-741).
- **References:** none of its own.
- **Fields left out on write:** none.
- **Write form notes:**
  - `append` is a required boolean.
- **Nested types:** item component `banner_patterns` (REG-FLD-578) (`patterns`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-776: item modifier `set_potion`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `id` | `potion` | key only | yes | — | — |

- **Fields left out on write:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-777: item modifier `set_random_dyes`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `number_of_dyes` | `context_int_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Nested types:** context int provider (REG-FLD-722).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-778: item modifier `set_random_potion`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `options` | `potion` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Write form notes:**
  - `options` is optional and has no default.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-779: item modifier `set_instrument`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `options` | `instrument` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** none.
- **Write form notes:**
  - `options` is required.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-780: item modifier `sequence`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `functions` | `item_modifier` | entry set | yes | yes | bare value for one element |

- **Fields left out on write:** none.
- **Write form notes:**
  - An inline element of `functions` must be an object with a `type` field. A bare list as an element fails the load.
  - When `condition` is absent, the whole modifier is written as the bare `functions` entry set, with no `type` and no object.
  - A sequence with one element is then written as a bare object. That object reads back as the single modifier, not as a sequence.
- **Nested types:** item modifier (REG-FLD-741).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-781: item modifier `copy_components`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `include[]` | `data_component_type` | key only | yes | — | always a list |
| `exclude[]` | `data_component_type` | key only | yes | — | always a list |

- **Fields left out on write:** none.
- **Write form notes:**
  - `include` and `exclude` are optional and have no default.
  - `source` is a string from a fixed set: the entity targets, the block entity target and the item stack targets. It is not a reference.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-782: item modifier `set_fireworks`

- **Reached from:** item modifier (REG-FLD-741).
- **References:** none of its own.
- **Fields left out on write:** none.
- **Write form notes:**
  - `flight_duration` is an optional unsigned byte with no default.
  - `explosions` is a standalone list operation (REG-FLD-770) with a maximum size of 256.
- **Nested types:** standalone list operation (REG-FLD-770) (`explosions`), item component `firework_explosion` (REG-FLD-574) (`explosions.values[]`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-783: item modifier `set_firework_explosion`

- **Reached from:** item modifier (REG-FLD-741).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - `shape` is an enum string.
  - `colors` and `fade_colors` are lists of ints.
  - `trail` and `twinkle` are optional booleans with no default.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-784: item modifier `set_book_cover`

- **Reached from:** item modifier (REG-FLD-741).
- **References:** none.
- **Fields left out on write:** none of its own.
- **Write form notes:**
  - `title` is a filterable string of at most 32 characters.
  - `author` is a string. `generation` is an int from 0 to 3.
  - All three fields are optional and have no default.
- **Nested types:** item component `written_book_content` (REG-FLD-561) (the filterable form of `title`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-785: item modifier `set_written_book_pages`

- **Reached from:** item modifier (REG-FLD-741).
- **References:** none of its own.
- **Fields left out on write:** none of its own.
- **Write form notes:**
  - The list operation fields are written in the parent object, with no maximum size.
- **Nested types:** item component `written_book_content` (REG-FLD-561) (the page form of `pages`, which contains text components), list operation (REG-FLD-769).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-786: item modifier `set_writable_book_pages`

- **Reached from:** item modifier (REG-FLD-741).
- **References:** none of its own.
- **Fields left out on write:** none of its own.
- **Write form notes:**
  - The list operation fields are written in the parent object, with a maximum size of 100.
- **Nested types:** item component `writable_book_content` (REG-FLD-560) (the page form of `pages`), list operation (REG-FLD-769).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-787: item modifier `toggle_tooltips`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `toggles` (map keys) | `data_component_type` | key only | yes | — | — |

- **Fields left out on write:** none.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-788: item modifier `set_ominous_bottle_amplifier`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `amplifier` | `context_int_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** none.
- **Nested types:** context int provider (REG-FLD-722).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-789: item modifier `set_custom_model_data`

- **Reached from:** item modifier (REG-FLD-741).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `floats.values[]` | `context_float_provider` | reference or inline | yes | — | always a list |
| `colors.values[]` | `context_int_provider` | reference or inline | yes | — | always a list |

- **Fields left out on write:** none.
- **Write form notes:**
  - `floats`, `flags`, `strings` and `colors` are optional standalone list operation (REG-FLD-770) values with no default.
  - On read, an element of `colors.values` can also be an RGB color form. On write, the provider form is used.
  - `flags.values[]` are booleans and `strings.values[]` are strings. They contain no references.
- **Nested types:** standalone list operation (REG-FLD-770), context float provider (REG-FLD-723), context int provider (REG-FLD-722).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-790: loot pool entry

- **Reached from:** item modifier `set_contents` (REG-FLD-760) (`entries[]`), loot pool (REG-FLD-800) (`entries[]`), loot pool entry `alternatives`, `sequence`, `group` (REG-FLD-798) (`children[]`).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `loot_pool_entry_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Nested types:** loot pool entry common fields (REG-FLD-791), loot pool entry `item` (REG-FLD-792), loot pool entry `tag` (REG-FLD-793), loot pool entry `loot_table` (REG-FLD-794), loot pool entry `dynamic` (REG-FLD-795), loot pool entry `empty` (REG-FLD-796), loot pool entry `slots` (REG-FLD-797), loot pool entry `alternatives`, `sequence`, `group` (REG-FLD-798).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-791: loot pool entry common fields

- **Reached from:** every loot pool entry type rule.
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `condition` | `predicate` | reference or inline | yes | — | — |
| `modifier` | `item_modifier` | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `weight` | `1` | no |
| `quality` | `0` | no |

- **Write form notes:**
  - `condition` and `modifier` are optional and occur in every entry type.
  - `weight` and `quality` occur only in the types `empty`, `item`, `loot_table`, `dynamic`, `tag` and `slots`.
- **Nested types:** loot condition (REG-FLD-700), item modifier (REG-FLD-741).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-792: loot pool entry `item`

- **Reached from:** loot pool entry (REG-FLD-790).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `name` | `item` | key only | yes | — | — |

- **Fields left out on write:** none of its own.
- **Write form notes:**
  - The load fails for `minecraft:air`.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-793: loot pool entry `tag`

- **Reached from:** loot pool entry (REG-FLD-790).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `items` | `item` | entry set | yes | no | bare value for one element |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `expand` | `false` | no |

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-794: loot pool entry `loot_table`

- **Reached from:** loot pool entry (REG-FLD-790).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `value` | `loot_table` | entry set | yes | yes | bare value for one element |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `expand` | `false` | no |

- **Write form notes:**
  - An inline element of `value` is a loot table object.
- **Nested types:** loot table (REG-FLD-799) (inline elements of `value`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-795: loot pool entry `dynamic`

- **Reached from:** loot pool entry (REG-FLD-790).
- **References:** none.
- **Fields left out on write:** none of its own.
- **Write form notes:**
  - `name` is an identifier of a dynamic drop. It is not a reference.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-796: loot pool entry `empty`

- **Reached from:** loot pool entry (REG-FLD-790).
- **References:** none of its own.
- **Fields left out on write:** none of its own.
- **Write form notes:**
  - This type has only the loot pool entry common fields (REG-FLD-791), `weight` and `quality` included.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-797: loot pool entry `slots`

- **Reached from:** loot pool entry (REG-FLD-790).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `slot_source` | `slot_source` | reference or inline | yes | — | — |

- **Fields left out on write:** none of its own.
- **Nested types:** slot source (REG-FLD-801).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-798: loot pool entry `alternatives`, `sequence`, `group`

- **Reached from:** loot pool entry (REG-FLD-790).
- **References:** none of its own.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `children` | `[]` | no |

- **Write form notes:**
  - `children` is always a list.
  - These types have no `weight` and no `quality`.
- **Nested types:** loot pool entry (REG-FLD-790) (`children[]`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-799: loot table

- **Reached from:** loot pool entry `loot_table` (REG-FLD-794) (inline elements of `value`).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `type` | `context_key_set` | key only | yes | — | — |
| `modifier` | `item_modifier` | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `type` | `"minecraft:generic"` | yes |
| `pools` | `[]` | no |

- **Write form notes:**
  - `modifier` is optional and has no default.
  - `random_sequence` is an identifier of a random sequence. It is not a reference.
  - `pools` is always a list.
- **Nested types:** item modifier (REG-FLD-741), loot pool (REG-FLD-800) (`pools[]`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-800: loot pool

- **Reached from:** loot table (REG-FLD-799) (`pools[]`).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `condition` | `predicate` | reference or inline | yes | — | — |
| `modifier` | `item_modifier` | reference or inline | yes | — | — |
| `rolls` | `context_int_provider` | reference or inline | yes | — | — |
| `bonus_rolls` | `context_float_provider` | reference or inline | yes | — | — |

- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `bonus_rolls` | `0.0`, an inline constant | no |

- **Write form notes:**
  - `condition` and `modifier` are optional and have no default.
  - `entries` is always a list.
  - The default comparison follows context int provider `conditional` and context float provider `conditional` (REG-FLD-730): a key is always written.
- **Nested types:** loot condition (REG-FLD-700), item modifier (REG-FLD-741), context int provider (REG-FLD-722), context float provider (REG-FLD-723), loot pool entry (REG-FLD-790) (`entries[]`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-801: slot source

- **Reached from:** loot pool entry `slots` (REG-FLD-797) (`slot_source`), slot source `group` (REG-FLD-802), slot source `filtered`, `limit_slots`, `contents` (REG-FLD-803).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| (whole value) | `slot_source` | reference or inline | yes | — | — |
| `type` | `slot_source_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - An inline value is an object with a `type` field, or the bare `terms` entry set of a `group`.
  - A `group` slot source is always written as the bare `terms` entry set.
- **Nested types:** slot source `group` (REG-FLD-802), slot source `filtered`, `limit_slots`, `contents` (REG-FLD-803), slot source `slot_range` (REG-FLD-804), slot source `empty` (REG-FLD-805).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-802: slot source `group`

- **Reached from:** slot source (REG-FLD-801).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `terms` | `slot_source` | entry set | yes | yes | bare value for one element |

- **Fields left out on write:** none.
- **Write form notes:**
  - An inline element of `terms` must be an object with a `type` field.
- **Nested types:** slot source (REG-FLD-801).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-803: slot source `filtered`, `limit_slots`, `contents`

- **Reached from:** slot source (REG-FLD-801).
- **References:**

| Field | Target registry | Kind | Checked | Inline | Write form |
|-------|-----------------|------|---------|--------|------------|
| `slot_source` | `slot_source` | reference or inline | yes | — | — |
| `component` (type `contents` only) | `data_component_type` | key only | yes | — | — |

- **Fields left out on write:** none.
- **Write form notes:**
  - `component` accepts only `container`, `bundle_contents` and `charged_projectiles`.
  - `limit` (type `limit_slots`) is a positive int.
- **Nested types:** slot source (REG-FLD-801), item predicate (REG-FLD-613) (`item_filter` of type `filtered`).

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-804: slot source `slot_range`

- **Reached from:** slot source (REG-FLD-801).
- **References:** none.
- **Fields left out on write:** a field in this table is not written when its value equals the default.

| Field | Default | Lenient |
|-------|---------|---------|
| `source` | `"container"` | no |

- **Write form notes:**
  - `slots` is a slot range string. It is not a reference.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

### REG-FLD-805: slot source `empty`

- **Reached from:** slot source (REG-FLD-801).
- **References:** none.
- **Fields left out on write:** none.
- **Write form notes:**
  - This type has no fields other than `type`.

Provenance: `source_suggested`. Confidence: medium. Experiment: REG-FLD-009.

## 10. Open points

Each point lists the affected rules and an experiment that decides it.

- REG-FLD-228: the game has a float provider kind that multiplies several float providers. The `float_provider_type` registry has no entry for it, so the file form and the network form cannot carry it. Experiment: put `{"type": "minecraft:multiplied", ...}` in a sulfur cube `contact_damage.amount` in a pack. Check the log for an unknown type error.
- REG-FLD-105, REG-FLD-107, REG-FLD-109, REG-FLD-110, REG-FLD-112, REG-FLD-114, REG-FLD-116, REG-FLD-200, REG-FLD-202: `asset_id`, `baby_asset_id`, `wild`, `tame` and `angry` are identifiers of a texture, not references. The server does not check them. Experiment: give a variant in a pack an `asset_id` that names no texture. Check that the server loads the pack without error. Capture the registry data packet and confirm the identifier is sent unchanged.
- REG-FLD-118, REG-FLD-214, REG-FLD-215: a tag in an entry set requires the `#` prefix, and the encoder writes a set with one element as a bare key. The analysis did not inspect the encoder for these two points. Experiment: put `"items": ["minecraft:stone"]` and then `"items": "#minecraft:logs"` in a `sulfur_cube_archetype` entry in a pack. Capture the registry data packet. Check that the first value comes back as the bare string `"minecraft:stone"` and that the second keeps `#`.
- REG-FLD-201, REG-FLD-113, REG-FLD-203, REG-FLD-204, REG-FLD-205, REG-FLD-210: the encoder writes a registered sound event as its key and an unregistered sound event as an inline value. The analysis did not inspect the encode path. Experiment: give `ambient_sound` in a `cat_sound_variant` of a pack an inline value `{"sound_id": "minecraft:custom.meow"}` and give `hurt_sound` a registered key. Capture the registry data packet and compare the two forms.
- REG-FLD-215: `worldgen/structure` is not a synced registry. The `structure` spawn condition does not affect the network, because the network form has no `spawn_conditions`. Experiment: add a `structure` spawn condition to a `wolf_variant` in a pack. Capture the registry data packet and confirm that the entry has no `spawn_conditions`.
- REG-FLD-263: the analysis derives from the source that `power` of particle options `dragon_breath` is always written, also when the value is `1.0`. Experiment: make a pack with a biome whose `visual/default_dripstone_particle` is `{ "type": "dragon_breath", "power": 1.0 }`. Capture the registry data on the wire and check that `power` is present.
- REG-FLD-129: the keys of `time_markers` use the key namespace `clock_time_marker`, which is not a registry. The keys are kept unchecked. Experiment: make a pack with a timeline whose `time_markers` has an unknown key, for example `test:no_such_marker`. Check the server log for a load error, and capture the timeline on the wire.
- REG-FLD-253: a reader accepts an entry `{ "modifier": "override", "argument": <value> }` and gives the same value as the bare form. This is not tested. Experiment: make a pack with a biome attribute in this form. Check that the load succeeds, and capture the wire form. The expected wire form is the bare value.
- REG-FLD-300: the default of `style` is the empty style. The analysis did not confirm that a loaded `{}` compares equal to the empty style. Experiment: put `"style": {}` in a `chat_type` decoration in a pack. Capture the registry data packet and check that `style` is absent.
- REG-FLD-303: the full form of a block state uses `id` and `properties`, not `Name` and `Properties`. Clients and tools that expect `Name` fail. Experiment: put `{"Name": "minecraft:oak_log"}` as a block state in a `worldgen/block_state_provider` entry in a pack. Check the log for a load error. Then put `{"id": "minecraft:oak_log"}` and check that the pack loads.
- REG-FLD-303: the full form writes every property in `properties`, and a parent that accepts both forms writes the default state in the short form. Experiment: put `{"id": "minecraft:oak_log", "properties": {}}` and `{"id": "minecraft:oak_log", "properties": {"axis": "x"}}` as two `worldgen/block_state_provider` entries in a pack. Capture the registry data packet. Expected: the provider value accepts only the full form, so both entries are written in the full form, each with all properties of the block. Then repeat with a parent that accepts both forms, for example `would_survive` `state`. Expected: the default state is written as the bare key.
- REG-FLD-304: the load fails when an inline object is valid both as a typed provider and as a block state. The analysis did not test an object with both `id` and `type`. Experiment: put `{"type": "minecraft:simple", "state": "minecraft:stone", "id": "minecraft:stone"}` as a `worldgen/block_state_provider` entry in a pack. Check the log for a load error.
- REG-FLD-304: the write writes a `simple` provider in the block state form, never with `type`. Experiment: put `{"type": "minecraft:simple", "state": "minecraft:stone"}` as a `worldgen/block_state_provider` entry in a pack. Capture the registry data packet and check that the entry is `{"id": "minecraft:stone"}`.
- REG-FLD-317: the world generation block predicate is a different type from the block predicate (REG-FLD-614). A type list that merges the two is wrong. Experiment: put `{"type": "minecraft:matching_blocks", "blocks": "minecraft:stone"}` in a rule `if_true` of a `rule_based` provider in a pack. Check that the pack loads without error.
- REG-FLD-320: `state` is a state properties predicate (REG-FLD-615). The analysis recorded only its default `{}` and that it holds no references. Experiment: put `"state": {}` in a `matching_fluids` predicate in a pack. Capture the registry data packet and check that `state` is absent.
- REG-FLD-331: the analysis did not expand the vertical anchor. It assumes that the anchor forms `absolute`, `above_bottom` and `below_top` hold no references. Experiment: load a pack with one `height_range` predicate for each anchor form. Capture the registry data packet and compare each anchor with the pack value.
- REG-FLD-377, REG-FLD-378: `block_state` can be a key of `worldgen/block_state_provider`. It is not verified that the client has this registry. Experiment: make a pack with an enchantment whose `replace_block.block_state` is a key, not an inline value. Join with a vanilla client and check the client log for a registry load error.
- REG-FLD-354: the set of strings that `enchanted` accepts is not confirmed. Experiment: make a pack with `equipment_drops.enchanted` set to each enchantment target string. Check the server log for the strings that fail to load.
- REG-FLD-377, REG-FLD-378: world generation block predicate (REG-FLD-317) contains a vertical anchor in the type `height_range`. The anchor forms (`absolute`, `above_bottom`, `below_top`) are assumed to hold no reference. Experiment: load a pack with each anchor form and capture the synced enchantment bytes.
- REG-FLD-122: an explicit `exclusive_set` value `[]` is expected to be left out on write, and a tag value is expected to be always written. Experiment: make two enchantments, one with `"exclusive_set": []` and one with `"exclusive_set": "#minecraft:exclusive_set/armor"`. Capture the registry data packet and compare the written fields.
- REG-FLD-379, REG-FLD-385: `run_function.function` and the attribute effect `id` are treated as plain identifiers, not references. Experiment: load a pack with a function id that does not exist and a new modifier id. Check the server log for the absence of a load error.
- REG-FLD-122: the registry `enchantment_provider` is not synced and no enchantment field refers to it. Experiment: capture the registry data packets at login and check that no packet carries `enchantment_provider`.
- REG-FLD-426: The `function` field of a test instance is a key that the load does not check. It is not known whether the client receives the `test_function` registry. Experiment: join a client to a server with a pack that defines a test instance with an unknown `function` key. Capture the registry data packets and the client log.
- REG-FLD-401: The default `action` of a `notice` dialog is left out only when the button equals the default button exactly. A label with any style is written. Experiment: load a pack with two `notice` dialogs, one with `{"label":{"translate":"gui.ok"}}` and one with an extra `color`. Capture the registry data packet and compare the written `action` fields.
- REG-FLD-429, REG-FLD-436, REG-FLD-438: The keys `type`, `source` and `object` are left out for the JSON form and the NBT form. It is not verified that every registry encoder on the sync path leaves them out. Experiment: load a pack with a dialog title of each contents type. Capture the registry data packet and check each component for these keys.
- REG-FLD-413, REG-FLD-438, REG-FLD-441: The internal defaults of item stack (REG-FLD-500) and item component `profile` (REG-FLD-576) are not listed in this chapter. Experiment: none here. Check the rules of those types for the dialog body `item`, the hover event `show_item` and the object `player`.
- REG-FLD-423: Each game rule value is a primitive, and no references were found. The value forms of each game rule are not listed. Experiment: load a pack with a `game_rules` test environment that sets each game rule. Check the log for load errors and capture the written values.
- REG-FLD-500, REG-FLD-501: the analysis found item stack (REG-FLD-500) in the dialog body `item`, the particle options `item` and the hover event `show_item`. It found saved item stack (REG-FLD-501) only in entity NBT, block entity NBT and other data that is not synced. Item modifiers and enchantment effects were not traced for item stack fields. Experiment: make a pack with an item modifier and an enchantment effect that hold an item. Capture the synced registry data and check whether `count` `1` is left out (form of REG-FLD-500) or written (form of REG-FLD-501).
- REG-FLD-540: `asset_id` is classified as key only and unchecked, because the server loads no `equipment_asset` registry. Experiment: make a pack with an item that has an `equippable` component with an `asset_id` that does not exist. Check the server log for the absence of a load error.
- REG-FLD-513, REG-FLD-543, REG-FLD-534, REG-FLD-540, REG-FLD-576, REG-FLD-577: `item_model`, `tooltip_style`, `use_cooldown.cooldown_group`, `equippable.camera_overlay`, the skin texture fields of `profile` and `note_block_sound` are treated as plain identifiers, not references. `note_block_sound` names a sound event, but no load step checks it. Experiment: load a pack that sets each field to an identifier that does not exist. Check the server log for the absence of a load error.
- REG-FLD-597: a key that is both a data component id and a predicate type id always selects the predicate type. The full list of such ids is not known. Experiment: list the ids of `data_component_predicate_type` and `data_component_type` from a vanilla data report. Compare the two lists and record each common id.
- REG-FLD-576: the default of `properties` in the partial profile form is assumed to compare equal to an empty property map. Experiment: make a pack with a `profile` component that has `"properties": []`. Capture the written component and check that `properties` is left out.
- REG-FLD-500 to REG-FLD-646: the rule "left out when equal to the default" depends on value equality. The analysis checked records, enum values, singleton values, the empty patch and the empty exact predicate. Other default values are assumed to compare by content. Experiment: for each default in this chapter, write the default value explicitly in a pack. Capture the written form and check that the field is left out.
- REG-FLD-549, REG-FLD-562, REG-FLD-567, REG-FLD-568, REG-FLD-578, REG-FLD-587, REG-FLD-588, REG-FLD-589: the element form of `block_transformer`, the inline forms of `trim_material`, `trim_pattern`, `instrument` and `banner_pattern`, and the context int and float providers are not expanded in this chapter. Other rules own them. Experiment: none here. Check the rules of those types.
- REG-FLD-700, REG-FLD-722, REG-FLD-723, REG-FLD-741, REG-FLD-801: a key in a `reference or inline` field of a stage 3 registry inside a synced entry probably fails the load. The exact lookup set during the synced load and during the client read is not confirmed. Experiment: make a pack with an enchantment whose effect `requirements` is a key string of a `predicate` entry. Load the pack on a server and check the log for a "registry does not exist" error. Then connect a client and capture the registry data packet for `enchantment`.
- REG-FLD-739, REG-FLD-740: the unchecked key of a resolvable int or a resolvable float is expected to load in a synced entry. Experiment: make a pack with an item component `cooking_fuel` whose `burn_time` is a key string of a `context_int_provider` entry. Load the pack and check the log for no error.
- REG-FLD-730, REG-FLD-731, REG-FLD-734, REG-FLD-736, REG-FLD-800: a read value of `-0.0` probably does not equal the default `0.0`, so the field is written. An inline `{"type":"constant","value":0}` probably equals the default, so the field is left out. Experiment: make a pack with a loot pool with `bonus_rolls: -0.0` and a second pool with `bonus_rolls: {"type":"constant","value":0}`. Re-encode the loaded loot table with a data dump and check which pools contain `bonus_rolls`.
- REG-FLD-705, REG-FLD-711, REG-FLD-715, REG-FLD-747, REG-FLD-751, REG-FLD-765: the classification of the `enchantment` and `clock` fields as `key only`, checked, is not confirmed. Experiment: make a pack with a `table_bonus` condition that names an absent enchantment key, and a `time_check` condition that names an absent world clock key. Load the pack and check the log for a load error for each file.
- REG-FLD-784, REG-FLD-785, REG-FLD-786: the default omissions inside the filterable title form and the book page forms belong to the item component rules. Experiment: make a pack with `set_book_cover` and `set_written_book_pages` modifiers that set only the raw text. Re-encode the loaded modifiers with a data dump and compare the written page objects with the item component rules.
- REG-FLD-709: the fields of the block predicate are written in the `match_block` object. The block predicate rule must list them as fields of the parent object. Experiment: make a pack with a `match_block` condition with `blocks` at the top level of the condition. Load the pack and check the log for no error.
