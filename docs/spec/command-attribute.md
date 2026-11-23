# The `attribute` command: `get` and `base`

| Field | Value |
|-------|-------|
| Parser ids | `brigadier:double` 2, `minecraft:entity` 6, `minecraft:resource_location` 36, `minecraft:resource` 46 |

Version: see `assets/minecraft/version.json`. Source of the parser ids: `assets/mcrs/reports/registries.json`, registry
`minecraft:command_argument_type`.

This document states the tree of `attribute` and the forms `get`, `base get`, `base set` and
`base reset`: the checks of the target, the messages and the result. The forms under `modifier`
are in the tree only. Their behaviour is out of scope. It does not prescribe an architecture.

## 0. Conventions

- **Node**, **source**, **error**: as `command-parser.md` defines them.
- **Display name**: as `command-execute.md`, section 0, defines it.
- **Result**: as `command-execute.md`, section 0, defines it.
- **Attribute**, **attribute set**, **base of the type**, **instance**, **base value**, **value**,
  **range**: as `attributes.md`, section 0.1, defines them.
- **Living entity**: as `entity-physics.md`, section 0.1, defines it.
- **Target**: the entity that `<target>` finds.
- **Attribute name**: a translatable text with no arguments. Its key is `attribute.name.` and the
  path of the identifier of the attribute, for example `attribute.name.max_health`.
- **Double argument**: an argument of a translatable text with the number kind double
  (`text-component.md`, TXT-CNT-003 and TXT-CNT-004).
- **Double text**: the text of an f64 `x` in a message:
  - `NaN`, `Infinity` or `-Infinity` for these values;
  - `0.0` or `-0.0` for a zero;
  - else the shortest decimal digits that read back as `x`. When `10^-3 <= |x| < 10^7`, the text
    is a plain decimal with at least one digit after the point. Else it is one digit, the point, at
    least one digit, `E` and the decimal exponent.
- Provenance labels: `registry.md`, section 0.3. ID prefix: `CMD-ATTR`.

These are double texts for a test:

| f64 | Double text |
|-----|-------------|
| 10 | `10.0` |
| 0.25 | `0.25` |
| 0.001 | `0.001` |
| 0.0001 | `1.0E-4` |
| 9999999 | `9999999.0` |
| 10000000 | `1.0E7` |
| 123456789.5 | `1.234567895E8` |
| negative zero | `-0.0` |

---

### CMD-ATTR-001: Tree

```
attribute                                            require command_level gamemasters
└── <target: minecraft:entity>                       one entity (0x01)
    └── <attribute: minecraft:resource>              registry minecraft:attribute
        ├── get                                      executable
        │   └── <scale: brigadier:double>            executable
        ├── base
        │   ├── set
        │   │   └── <value: brigadier:double>        executable
        │   ├── get                                  executable
        │   │   └── <scale: brigadier:double>        executable
        │   └── reset                                executable
        └── modifier
            ├── add
            │   └── <id: minecraft:resource_location>
            │       └── <value: brigadier:double>
            │           ├── add_value                executable
            │           ├── add_multiplied_base      executable
            │           └── add_multiplied_total     executable
            ├── remove
            │   └── <id: minecraft:resource_location>    executable; suggestion type minecraft:ask_server
            └── value
                └── get
                    └── <id: minecraft:resource_location>    executable; suggestion type minecraft:ask_server
                        └── <scale: brigadier:double>        executable
```

The permission is PERM-LEVEL-001, level `gamemasters`. Each `brigadier:double` node has no bounds
(CMD-ARG-002). Thus a value or a scale is a finite f64 of each sign (CMD-RD-004).
CMD-TIME-004 states `minecraft:resource`. An identifier that is not an attribute fails with
`argument.resource.not_found`, with the identifier and `minecraft:attribute`. This is an error of the
parse, with the position after the identifier.

`<target>` finds one entity. A selector that can find more than one entity fails with
`argument.entity.toomany`. A selector that finds no entity fails with
`argument.entity.notfound.entity` when the command runs (ARG-ENT-016).

Provenance: `source_suggested_black_box_confirmed`. Kind: wire. Confidence: high. Experiment
CMD-ATTR-E1 confirms the nodes, the parsers and the permission. The suggestion types come from the
reference.

### CMD-ATTR-002: Checks of the target

- **Behavior:** each form of CMD-ATTR-003 to CMD-ATTR-006 does these checks first, in this order:
  1. If the target is not a living entity, fail with `commands.attribute.failed.entity`, with
     the display name of the target.
  2. If the attribute set of the target does not contain the attribute (ATR-BASE-001), fail with
     `commands.attribute.failed.no_attribute`. The arguments are the display name of the target,
     then the attribute name.
- **Postconditions:** an error has no position. A command that fails changes no instance.
- **Edge cases:**
  - A `marker` is not a living entity. `attribute @n[type=marker] minecraft:max_health base reset`
    fails with `commands.attribute.failed.entity`.
  - `attack_damage` is not in the attribute set of a `pig`. Each form fails with
    `commands.attribute.failed.no_attribute`, also `base set`.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic; the keys and the argument
order are wire. Confidence: high. Experiment: CMD-ATTR-E3.

### CMD-ATTR-003: `attribute <target> <attribute> get [<scale>]`

- **Behavior:** read the value `v` of the instance (ATR-VAL-001). Send
  `commands.attribute.value.get.success`, with the broadcast flag clear. The arguments are, in
  order:
  1. the attribute name;
  2. the display name of the target;
  3. `v`, as a double argument.
- **Postconditions:** the result is the scaled result of CMD-ATTR-007, with `v` and the scale.
  Without `<scale>`, the scale is 1.
- **Edge cases:** the value has the range (ATR-VAL-003). A `pig` with the base value 5000 of
  `max_health` gives `1024.0` and the result 1024.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic; the key and the argument order
are wire. Confidence: high. Experiment: CMD-ATTR-E2.

### CMD-ATTR-004: `attribute <target> <attribute> base get [<scale>]`

- **Behavior:** read the base value `b` of the instance. Send
  `commands.attribute.base_value.get.success`, with the broadcast flag clear. The arguments are
  the attribute name, the display name of the target and `b` as a double argument, in this order.
- **Postconditions:** the result is the scaled result of CMD-ATTR-007, with `b` and the scale.
  Without `<scale>`, the scale is 1.
- **Edge cases:** the base value has no range (ATR-INS-002). A `pig` with the base value 5000 of
  `max_health` gives `5000.0`. The scale 0.5 then gives the result 2500.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic; the key and the argument order
are wire. Confidence: high. Experiment: CMD-ATTR-E2.

### CMD-ATTR-005: `attribute <target> <attribute> base set <value>`

- **Behavior:**
  1. Set the base value of the instance to `<value>` (ATR-INS-002). The command does not clamp
     `<value>` to the range, and it does not reject a value outside the range.
  2. Send `commands.attribute.base_value.set.success`, with the broadcast flag clear. The
     arguments are the attribute name, the display name of the target and `<value>` as a double
     argument, in this order.
- **Postconditions:**
  - The result is 1.
  - The value of the instance comes from the new base value, with the range (ATR-VAL-003).
  - A change of `max_health` below the health changes the health (`entity-damage.md`,
    DMG-ATT-001).
- **Edge cases:**
  - A value with the same bits as the base value does not change the instance (ATR-INS-002). The
    command still succeeds, with the result 1.
  - `-0` sets the base value to negative zero, and the message shows `-0.0`. The value of
    `max_health` is then 1, the minimum.
  - The base value 0.03125 of `scale` stays 0.03125. The value is 0.0625, the minimum.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic; the key and the argument order
are wire. Confidence: high. Experiment: CMD-ATTR-E2.

### CMD-ATTR-006: `attribute <target> <attribute> base reset`

- **Behavior:**
  1. Set the base value of the instance to the base of the type of the target (ATR-BASE-003).
  2. Send `commands.attribute.base_value.reset.success`, with the broadcast flag clear. The
     arguments are the attribute name, the display name of the target and the new base value as a
     double argument, in this order.
- **Postconditions:** the result is 1. The modifiers of the instance do not change.
- **Edge cases:** the reset does not restore a base value that the spawn of the entity set. A
  `slime` of size 4 has the base value 16 of `max_health`. A reset sets it to 20, the base of the
  type.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic; the key and the argument order
are wire. Confidence: high. Experiment: CMD-ATTR-E4.

### CMD-ATTR-007: Scaled result

- **Behavior:** the scaled result of an f64 `x` and a scale `s` is the signed 32-bit integer of
  `p = x × s`, one f64 multiplication:
  - 0, when `p` is a NaN;
  - `2147483647`, when `p` is not less than 2^31;
  - `-2147483648`, when `p` is not more than -2^31;
  - else `p` rounded toward zero.
- **Edge cases:**

| `x` | `s` | Result |
|-----|-----|--------|
| 0.25 | 1000 | 250 |
| 0.25 | 3 | 0 |
| 0.25 | -3 | 0 |
| 10 | 10^12 | 2147483647 |
| 10 | -10^12 | -2147483648 |

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high. Experiment:
CMD-ATTR-E2.

---

## 1. Messages

| Key | Arguments | Rule |
|-----|-----------|------|
| `commands.attribute.value.get.success` | attribute name, display name, value (double) | CMD-ATTR-003 |
| `commands.attribute.base_value.get.success` | attribute name, display name, base value (double) | CMD-ATTR-004 |
| `commands.attribute.base_value.set.success` | attribute name, display name, `<value>` (double) | CMD-ATTR-005 |
| `commands.attribute.base_value.reset.success` | attribute name, display name, base value (double) | CMD-ATTR-006 |
| `commands.attribute.failed.entity` | display name | CMD-ATTR-002 |
| `commands.attribute.failed.no_attribute` | display name, attribute name | CMD-ATTR-002 |
| `argument.resource.not_found` | identifier, `minecraft:attribute` | CMD-ATTR-001 |

- The success messages go only to the source (`commands-packet.md`, CMDP-EXEC-004).
- The vanilla client makes the double text from a double argument. The server makes the same text
  when it writes a message to its console.

## 2. Unresolved

None.

## 3. Experiments

Each experiment runs on the vanilla server of the game version of this document. Run
`forceload add 0 0` first. Summon a `pig` and a `slime` at the chunk 0 0, with `NoAI` and
`NoGravity`. Read a result with `execute store result score #a r run ...`.

| Id | Steps | Observed result | Rules |
|----|-------|-----------------|-------|
| CMD-ATTR-E1 | Run `dumpReports` (`tools/vanilla-oracle`). Read the node `attribute` in `commands.json`. | The nodes, the parsers and the level `gamemasters` agree with the tree. | CMD-ATTR-001 |
| CMD-ATTR-E2 | On the pig, run `get` of `movement_speed` with the scales 1000, 3 and -3. Run `get` of `max_health` with 10^12 and -10^12. Set the base of `max_health` to 5000, then run `get` and `base get 0.5`. Set it to `-0`, and run `get`. | The value text is `0.25`, and the results are 250, 0 and 0. The results are 2147483647 and -2147483648. The base text is `5000.0`, the value text is `1024.0`, and the result is 2500. The set shows `-0.0`, and the value is `1.0`. Each set gives the result 1. | CMD-ATTR-003 to CMD-ATTR-005, CMD-ATTR-007 |
| CMD-ATTR-E3 | Run `get`, `base set 5` and `base reset` of `max_health` on a `marker`. Run each form of `attack_damage` on the pig. Run `get` of `minecraft:nope`. | The marker gives `commands.attribute.failed.entity`. The pig gives `commands.attribute.failed.no_attribute` for each form. `minecraft:nope` gives `argument.resource.not_found`, with the position after the identifier. | CMD-ATTR-001, CMD-ATTR-002 |
| CMD-ATTR-E4 | Summon the slime with `Size:3`. Run `base get` of `max_health`, then `base reset`, then `get`. | The base is `16.0`. The reset shows `20.0`, and the value is `20.0`. | CMD-ATTR-006 |
| CMD-ATTR-E5 | On the pig, set the base of `scale` to 0.0001, 0.001, 9999999, 10000000 and 123456789.5. | The messages show the double texts of section 0. | Section 0 |
