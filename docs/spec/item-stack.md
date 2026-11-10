# Item stack: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| World version | `5127` |

Source of these values: `assets/minecraft/version.json`.

This document states the item stack as data: its parts, its component patch, its count, and each
format that carries it. It states the meaning of four component types, and it lists each component
type of this game version. It does not prescribe an architecture.

Scope: the stack itself. This document does not state inventories, the slots of a player, clicks,
menus or item entities. It does not state enchantment effects, the use of an item, attributes,
tooltips or the formats of Bedrock Edition. Section 8.1 states the shape of 15 component types. For
each other component type that section 7 does not state, this document gives only the row of
section 8.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **clientbound**, **serverbound**, **VarInt**: as
  `login.md`, section 0.1, defines them.
- **Identifier**, **registry**, **tag**: as `registry.md`, section 0.1, defines them.
- **Item**: an entry of the registry `minecraft:item`.
- **Air**: the item `minecraft:air`.
- **Component type**: an entry of the registry `minecraft:data_component_type`.
- **Type id**: the numeric id of a component type in that registry.
- **Item id**: the numeric id of an item in the registry `minecraft:item`.
- **Component**: one value of one component type.
- **Prototype**: the set of components that an item has. A prototype has at most one component for
  each component type. Each stack of the item has the same prototype.
- **Count**: a signed 32-bit integer that states how many items a stack holds.
- **Component patch**: a set of patch entries, with at most one patch entry for each component type.
- **Patch entry**: a value entry or a removal entry.
- **Value entry**: a component type and a component of that type.
- **Removal entry**: a component type alone.
- **Stack**: an item, a count and a component patch.
- **Effective component**: the component that a read of a stack gives for a component type
  (STK-PAT-002).
- **Empty stack**: the stack that holds nothing (STK-EMP-001).
- **Normal form**: a component patch that has no patch entry without effect (STK-PAT-004).
- **Saved form**: the form of a value as a tree of maps, lists, strings and numbers. NBT, SNBT and
  JSON carry a saved form.
- **Wire form**: the form of a value as bytes in a packet.
- **Network NBT**: one NBT tag in a packet. It has the type byte and the payload, and no name.
- **Saved type**: a component type that has a saved form (STK-TYP-002).
- **Item template**: an item, a count and a component patch that is data of another value
  (section 6.6). An item template is not empty.
- **Hashed stack**: the short form of a stack that the client sends in the `container_click`
  packet (section 6.7).
- **Maximum stack size**, **maximum damage**, **damage value**: the numbers that STK-CMP-001,
  STK-CMP-002 and STK-DMG-002 define.
- **Strict validation**: the checks of STK-VAL-001.

### 0.2 Provenance

- `black_box_confirmed`: shown by an artefact that the game writes, or by a result in section 12.
- `source_suggested_black_box_confirmed`: taken from the reference behaviour, and consistent with an
  artefact or a result in section 12.
- `source_suggested`: taken from the reference behaviour, with no result. The confidence is at most
  medium. Section 11 attaches an experiment.
- `design`: a choice of this project. The rule states the reason.
- `unresolved`: not decided. Section 10 collects these items.

No experiment of section 11 has a recorded run. Each one is a proposal. The results of section 12
come from the goldens that the game wrote for `26.4-snapshot-7`.

### 0.3 Artefacts cited

- `assets/mcrs/reports/registries.json`: the registry report of the game. It gives each type id
  and each item id.
- `assets/mcrs/reports/packets.json`: the packet id report of the game.
- `assets/mcrs/item_definition/`: one file for each item, which the game writes through
  `tools/vanilla-oracle`. STK-PRO-002 states the keys.
- The fixtures `hash_ops`, `item_plain`, `item_nested` and `inventory_packets` of
  `tools/fixtures.json`. The codecs of the game write each expected value of these goldens.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `STK-PRO` | prototypes |
| `STK-PAT` | the component patch |
| `STK-EMP` | the empty stack |
| `STK-CNT` | the count |
| `STK-EQ` | equality and the stack rule |
| `STK-SAV` | the saved stack |
| `STK-CMD` | the command form |
| `STK-NET` | the wire stack |
| `STK-TPL` | the item template |
| `STK-HSH` | the hashed stack and the hash |
| `STK-CMP` | the four component types of section 7 |
| `STK-DMG` | damage of a stack |
| `STK-VAL` | strict validation |
| `STK-TYP` | the list of component types |
| `STK-SHP` | the shapes of the component types of section 8.1 |

### 0.5 Notation

A byte string is in hexadecimal. `u32le(x)` is the 4 bytes of a 32-bit value, least significant
byte first. An integer range `a..b` includes `a` and `b`.

## 1. Items and prototypes

**STK-PRO-001** (`black_box_confirmed`, semantic). Each item has one prototype. The game fixes the
prototype. A data pack cannot change a prototype in this game version.

**STK-PRO-002** (`black_box_confirmed`, semantic). The file
`assets/mcrs/item_definition/<path>.json` gives the prototype of the item `minecraft:<path>`. The
file has these keys under `minecraft:item`:

| Key | Value |
|-----|-------|
| `description.identifier` | the identifier of the item |
| `description.protocol_id` | the item id |
| `components` | the prototype: a map from the identifier of a component type to the saved form of the component, as JSON |

**STK-PRO-003** (`source_suggested_black_box_confirmed`, semantic). A component of a prototype can
refer to an entry or a tag of a registry by its identifier. An implementation must read the
prototypes after it loads the registries and the tags. The file of `minecraft:diamond_sword`
shows a tag: the component `minecraft:repairable` has `#minecraft:diamond_tool_materials`.

**STK-PRO-004** (`black_box_confirmed`, semantic). Each prototype in the item definitions has a
component for each of these component types: `minecraft:max_stack_size`, `minecraft:lore`,
`minecraft:enchantments`, `minecraft:repair_cost`, `minecraft:use_effects`,
`minecraft:attribute_modifiers`, `minecraft:rarity`, `minecraft:break_sound`,
`minecraft:tooltip_display`, `minecraft:attack_animation`, `minecraft:interact_animation`,
`minecraft:item_name` and `minecraft:item_model`. An implementation must not assume this for an
item that a later game version adds. It must read the file.

**STK-PRO-005** (`black_box_confirmed`, semantic). No prototype has a component of a component
type that is not a saved type. Thus the JSON of STK-PRO-002 can state each prototype in full.

**STK-PRO-006** (`source_suggested_black_box_confirmed`, semantic). A prototype that has
`minecraft:damage` has a maximum stack size of 1. In the item definitions, 84 items have
`minecraft:max_damage`. Each of them has `minecraft:damage` with the value 0 and
`minecraft:max_stack_size` with the value 1.

**STK-PRO-007** (`source_suggested`, semantic). Air has a prototype as each other item has. A stack
of air is the empty stack (STK-EMP-001), so no read of a stack gives a component of air.
See experiment E1.

## 2. The component patch

**STK-PAT-001** (`source_suggested_black_box_confirmed`, semantic). A component patch has at most
one patch entry for each component type. A value entry states that the stack has that component.
A removal entry states that the stack does not have a component of that component type.
See result R6.

**STK-PAT-002** (`source_suggested`, semantic). A read of a stack for a component type gives one
of three results:

| Result | Condition | Effective component |
|--------|-----------|---------------------|
| overridden | the patch has a value entry for the component type | the component of the value entry |
| removed | the patch has a removal entry for the component type | none |
| default | the patch has no patch entry for the component type | the component of the prototype, or none when the prototype has none |

See experiment E2.

**STK-PAT-003** (`source_suggested`, semantic). A read of the empty stack gives no effective
component for each component type. See experiment E1.

**STK-PAT-004** (`source_suggested`, semantic). A patch entry has no effect in these two cases:

- a value entry whose component is equal to the component of the prototype for that component type;
- a removal entry for a component type that the prototype does not have.

A component patch is in normal form when it has no such patch entry. See experiment E3.

**STK-PAT-005** (`source_suggested`, semantic). The component patch of each stack is in normal
form. A reader of each format of section 6 must drop each patch entry without effect. A writer of
each format thus writes no such patch entry. See experiment E3.

**STK-PAT-006** (`source_suggested`, semantic). A change of one component of a stack keeps the
normal form:

| Change | Prototype | Result in the patch |
|--------|-----------|---------------------|
| set a component | has an equal component | no patch entry for the component type |
| set a component | has a different component, or none | a value entry |
| remove a component | has a component | a removal entry |
| remove a component | has none | no patch entry for the component type |

See experiment E3.

**STK-PAT-007** (`design`, semantic). Two components of one component type are equal when their
saved forms are equal as trees. In a tree, the order of the keys of a map has no effect. The
order of the elements of a list has an effect. For a component type that is not a saved type, two
components are equal when their wire forms are equal.

Reason: a client can observe equality only
through a format.

The saved form of this rule is the form that the writer gives for the component. It is not the
input text of a reader. Two inputs that decode to one component are thus equal. STK-EQ-005 states
equality of two numbers in floating point. STK-EQ-006 states equality of two numbers in free NBT.
STK-SHP-017 states one exception for `minecraft:tooltip_display`.

**STK-PAT-008** (`source_suggested`, semantic). A component patch can have a value entry for a
component type that the prototype does not have. A component patch can have a value entry for
each component type, also for a type that has no use for that item. See experiment E2.

## 3. The empty stack

**STK-EMP-001** (`source_suggested_black_box_confirmed`, semantic). A stack is the empty stack when
one of these conditions is true:

- the item is air;
- the count is 0 or less.

Only one empty stack exists. A reader of a format that has an empty stack must give the empty stack
for such a value. The saved stack has no empty stack: its reader gives an error (STK-SAV-002,
STK-CNT-003).
See results R2 and R4.

**STK-EMP-002** (`source_suggested`, semantic). The empty stack has the item air, the count 0 and
an empty component patch. No data of the item, the count or the patch of the value that became
empty remains. See experiment E1.

**STK-EMP-003** (`source_suggested_black_box_confirmed`, wire). Each format writes the empty stack
as this table states.

| Format | Form of the empty stack |
|--------|-------------------------|
| saved stack (STK-SAV-001) | none. The owner of the field omits the field. |
| optional saved stack (STK-SAV-008) | the empty map `{}` |
| saved stack with slot (STK-SAV-010) | none. The list has no element for that slot. |
| command form (STK-CMD-001) | none |
| wire stack (STK-NET-001) | the byte `00` |
| wire stack that is not empty (STK-NET-006) | none. A writer and a reader report an error. |
| wire stack with length prefix (STK-NET-007) | the byte `00` |
| item template (STK-TPL-001) | none |
| optional item template (STK-TPL-005) | the byte `00` |
| hashed stack (STK-HSH-001) | the byte `00` |

See results R2, R4 and R5.

## 4. The count

**STK-CNT-001** (`source_suggested`, semantic). The count of a stack that is not empty is 1 or
more. The largest count that every format carries without error is 99.
A format can accept a larger count (STK-CNT-003). See experiment E4.

**STK-CNT-002** (`source_suggested`, semantic). The maximum stack size of a stack is an effective
component (STK-CMP-001). A patch can change it for one stack. A stack can have a count larger than
its maximum stack size. Only strict validation rejects such a stack (STK-VAL-001).
See experiment E5.

**STK-CNT-003** (`source_suggested_black_box_confirmed`, wire). Each reader does this with a count
out of range:

| Format | Range | Count out of range |
|--------|-------|--------------------|
| saved stack | `1..99`; an absent count is 1 | The stack does not decode. The reader gives an error and no stack. |
| saved stack with slot | `1..99`; an absent count is 1 | as the saved stack |
| optional saved stack | `1..99`; an absent count is 1 | as the saved stack |
| item template, saved form | `1..99`; an absent count is 1 | The item template does not decode. |
| command form | no count in the form | STK-CMD-006 states the count that a command gives. |
| wire stack | each VarInt | 0 or less gives the empty stack. The reader accepts a count larger than 99. |
| wire stack with length prefix | `1..99`, and 0 or less for the empty stack | A count larger than 99 is a decode error of the packet. |
| item template, wire form | 1 or more | 0 is a decode error. A negative count is not specified (STK-TPL-004). |
| hashed stack | each VarInt | The reader accepts each value. The value only takes part in STK-HSH-003. |

See results R1 and R7, and experiments E4 and E6.

**STK-CNT-004** (`source_suggested`, semantic). A stack with a count larger than 99 has no saved
form. The writer reports an error and writes no stack for it. See experiment E4.

**STK-CNT-005** (`design`, semantic). A server must not send a wire stack with a count larger than
99. Reason: the client cannot send that stack back in a `set_creative_mode_slot` packet, and the
client cannot save it.

## 5. Equality and the stack rule

**STK-EQ-001** (`source_suggested`, semantic). Two stacks have the same item and components when
one of these conditions is true:

- the two stacks are empty;
- the two stacks are not empty, they have the same item, and their component patches are equal.

Two component patches are equal when they have patch entries for the same component types, of the
same kind, with equal components (STK-PAT-007). The count has no part in this relation.
See experiment E7.

**STK-EQ-002** (`source_suggested`, semantic). Two stacks are equal when they have the same item
and components and the same count. See experiment E7.

**STK-EQ-003** (`source_suggested`, semantic). A stack can stack when these two conditions are
true:

- its maximum stack size is larger than 1;
- it cannot take damage (STK-DMG-001), or its damage value is 0.

See experiment E8.

**STK-EQ-004** (`source_suggested`, semantic). Two stacks can merge when they have the same item
and components and the stack that receives can stack. The count of the result must not be larger
than the maximum stack size. A document for inventories states how a merge moves the count.
See experiment E8.

**STK-EQ-005** (`source_suggested`, semantic). In equality of two components, two float values are
equal when one of these conditions is true:

- each of the two values is a NaN;
- no value is a NaN, and the two values have the same bit pattern.

The same rule applies to two double values. Equality does not use the numeric comparison of
IEEE 754. Thus each NaN is equal to each other NaN, with each payload and each sign. Thus `0.0`
and `-0.0` are not equal. This rule applies to STK-PAT-004, STK-PAT-006, STK-EQ-001 and STK-EQ-004.
See experiment E28.

**STK-EQ-006** (`source_suggested`, semantic). In free NBT (STK-TYP-004), two numbers of different
tag kinds are not equal. For example, `{a:1}` and `{a:1b}` are not equal. Two float tags or two
double tags are equal as STK-EQ-005 states. A field with a known number kind is different: its
reader gives one number kind, and equality compares the values after the decode.
See experiment E28.

## 6. Formats

### 6.1 The saved stack

**STK-SAV-001** (`source_suggested_black_box_confirmed`, wire). The saved stack is a map with
these keys:

| Key | Saved form | Reader | Writer |
|-----|------------|--------|--------|
| `id` | a string: the identifier of the item | necessary | always writes it |
| `count` | an int | optional; the default is 1; the range is `1..99` | always writes it |
| `components` | a map: the component patch (STK-SAV-003) | optional; the default is an empty patch | omits it when the patch is empty |

In NBT, `count` is an Int tag. The order of the keys has no meaning. See result R6.

**STK-SAV-002** (`source_suggested`, semantic). The reader gives an error and no stack in each of
these cases:

- `id` is absent, or is not an identifier;
- `id` does not name an item;
- `id` names air;
- `count` is out of range (STK-CNT-003).

The saved stack has no form for the empty stack. See experiment E9.

**STK-SAV-003** (`source_suggested_black_box_confirmed`, wire). The component patch is a map. Each
key is one of these:

| Key | Value | Meaning |
|-----|-------|---------|
| `<type>` | the saved form of the component | a value entry |
| `!<type>` | the empty map `{}` | a removal entry |

`<type>` is the identifier of the component type. The writer writes it with the namespace.
See result R6.

**STK-SAV-004** (`source_suggested`, semantic). The reader accepts a key without a namespace and
gives it the namespace `minecraft`. The reader ignores the value of a `!<type>` key.
See experiment E10.

**STK-SAV-005** (`source_suggested`, semantic). A key of the patch is an error when one of these
conditions is true:

- `<type>` does not name a component type;
- `<type>` names a component type that is not a saved type;
- the value is not a valid saved form of the component.

See experiment E11.

**STK-SAV-006** (`source_suggested`, semantic). When a key of the patch is an error, the reader
reports the error. The reader of a world file then gives the stack with the patch entries that
decode. A reader that has no partial result gives no stack. A design note states which reader each
caller uses. See experiment E11.

**STK-SAV-007** (`source_suggested`, semantic). The reader of the saved stack does not do strict
validation. The reader accepts a count larger than the maximum stack size, and a damage value
equal to the maximum damage. See experiment E5.

**STK-SAV-008** (`source_suggested`, wire). The optional saved stack is the saved stack or the
empty map `{}`. The writer writes `{}` for the empty stack. The reader gives the empty stack for a
map that has no keys. See experiment E12.

**STK-SAV-009** (`source_suggested`, semantic). The writer omits each patch entry of a component
type that is not a saved type. The writer reports no error for it. See experiment E13.

**STK-SAV-010** (`source_suggested`, wire). The saved stack with slot is the saved stack with one
more key:

| Key | Saved form | Reader | Writer |
|-----|------------|--------|--------|
| `Slot` | a byte, read as unsigned: `0..255` | optional; the default is 0 | always writes it |

A list of such maps holds the slots of a container. The writer writes no element for a slot that
holds the empty stack. A slot number larger than 255 is an error of the writer.
See experiment E14.

**STK-SAV-011** (`source_suggested_black_box_confirmed`, wire). The saved form has no other
variant for SNBT. In SNBT, the saved stack is the same map in the syntax of `snbt.md`. In JSON,
`count` is a number. See result R6.

### 6.2 The command form

**STK-CMD-001** (`source_suggested`, client agreement). The command form states an item and a
component patch. It has no count and no form for the empty stack. Its grammar is:

```
form     = item [ "[" [ entry { "," entry } ] "]" ]
entry    = type "=" value | "!" type
```

`item` is the identifier of an item. `type` is the identifier of a component type. `value` is the
SNBT of the saved form of the component, as `snbt.md` states it. Whitespace can occur before and
after each `entry`, `=` and `,`. See experiment E15.

**STK-CMD-002** (`source_suggested`, client agreement). An identifier without a namespace gets the
namespace `minecraft`. A `type "=" value` entry is a value entry. A `"!" type` entry is a removal
entry. See experiment E15.

**STK-CMD-003** (`source_suggested`, client agreement). A `,` with no entry after it is an error.
Experiment E15 decides if `[]` is valid.

**STK-CMD-004** (`source_suggested`, client agreement). The parser gives these errors. Each error
is a translation key of the game.

| Condition | Translation key |
|-----------|-----------------|
| the identifier does not name an item | `argument.item.id.invalid` |
| no component type is at the position of `type` | `arguments.item.component.expected` |
| `type` does not name a component type, or names a type that is not a saved type | `arguments.item.component.unknown` |
| one component type occurs in two entries, of each kind | `arguments.item.component.repeated` |
| `value` is not a valid saved form of the component | `arguments.item.component.malformed` |

See experiment E16.

**STK-CMD-005** (`source_suggested`, semantic). The component patch of the result is in normal
form (STK-PAT-005). See experiment E3.

**STK-CMD-006** (`source_suggested`, semantic). A command that makes a stack of count `n` from the
command form does these checks, in this order:

1. If `n` is larger than the maximum stack size of the result, the error is
   `arguments.item.overstacked`.
2. If strict validation rejects the result, the error is `arguments.item.malformed`.

See experiment E17.

### 6.3 The wire stack

**STK-NET-001** (`source_suggested_black_box_confirmed`, wire). The wire stack has these fields,
in this order:

| Field | Form | Condition |
|-------|------|-----------|
| count | VarInt | always |
| item id | VarInt | count is larger than 0 |
| number of value entries, `A` | VarInt | count is larger than 0 |
| number of removal entries, `R` | VarInt | count is larger than 0 |
| value entries | `A` times: type id (VarInt), then the wire form of the component | count is larger than 0 |
| removal entries | `R` times: type id (VarInt) | count is larger than 0 |

The item id has no offset. The empty stack is the one byte `00`. See results R1 and R2.

**STK-NET-002** (`source_suggested_black_box_confirmed`, wire). The wire form of a component
depends on its component type (STK-TYP-003). No length and no end mark separates two components.
A reader must know the wire form of each component type to read a stack. See result R1.

**STK-NET-003** (`source_suggested`, wire). The reader gives a decode error for an item id or a
type id that names no entry. The reader gives the empty stack when the count is larger than 0 and
the item is air. See experiment E18.

**STK-NET-004** (`design`, wire). A writer must not write two patch entries for one component
type. The order of the patch entries is the choice of the writer. What a reader does with two
patch entries for one component type is not specified. Reason: the vanilla client and the vanilla
server write each component type one time.

**STK-NET-005** (`source_suggested_black_box_confirmed`, wire). These packets carry a wire stack:
the clientbound `container_set_slot`, `container_set_content`, `set_cursor_item`,
`set_player_inventory` and `set_equipment`. A value of the item stack kind in the entity data
carries one too. In `container_set_content`, a VarInt number of wire stacks comes before the
stack of the cursor. See results R1, R2 and R3.

**STK-NET-006** (`source_suggested`, wire). The wire stack that is not empty has the same bytes
as the wire stack. A count of 0 or less is a decode error. A writer must not give it the empty
stack. The result of a merchant offer uses this format. See experiment E19.

### 6.4 The wire stack with length prefix

**STK-NET-007** (`source_suggested_black_box_confirmed`, wire). The wire stack with length prefix
has the fields of STK-NET-001, with one change. In each value entry, a VarInt comes between the
type id and the component. This VarInt is the number of bytes of the wire form of the component.
The serverbound `set_creative_mode_slot` packet carries this format. See results R4 and R5.

**STK-NET-008** (`source_suggested`, wire). A component type whose wire form has no bytes has the
length 0. See result R4.

**STK-NET-009** (`source_suggested`, semantic). After the reader decodes a stack that is not
empty, the stack must have a saved form. The reader gives a decode error of the packet when one of
these conditions is true:

- the count is larger than 99;
- a component of a saved type in the patch has no valid saved form.

The check ignores the patch entries of a component type that is not a saved type. The check does
not do strict validation. See experiment E6.

**STK-NET-010** (`design`, unspecified). A reader can reject a component that uses fewer bytes
than its length states. Reason: the vanilla client writes the exact length. Open question Q5 asks
for the decision.

### 6.5 Where a nested format applies

**STK-NET-011** (`source_suggested`, wire). The length prefix of STK-NET-007 applies only to the
value entries of the outer stack. An item template inside a component uses the wire form of
STK-TPL-003, with no length prefix. See experiment E20.

### 6.6 The item template

**STK-TPL-001** (`source_suggested_black_box_confirmed`, semantic). An item template has an item,
a count and a component patch. The item is not air and the count is not 0. A component that holds
stacks holds item templates (STK-TYP-004). An item template is data. It does not change while its
owner holds it. See result R6.

**STK-TPL-002** (`source_suggested_black_box_confirmed`, wire). The saved form of an item template
is a map with these keys:

| Key | Saved form | Reader | Writer |
|-----|------------|--------|--------|
| `id` | a string: the identifier of the item | necessary | always writes it |
| `count` | an int, range `1..99` | optional; the default is 1 | omits it when the count is 1 |
| `components` | a map: the component patch (STK-SAV-003) | optional; the default is an empty patch | omits it when the patch is empty |

The reader also accepts one string, the identifier of the item. That string means the count 1 and
an empty patch. The writer always writes the map. See results R6 and R7.

**STK-TPL-003** (`source_suggested_black_box_confirmed`, wire). The wire form of an item template
has these fields, in this order: item id (VarInt), count (VarInt), then the patch as STK-NET-001
states it. The item id comes before the count. This order is different from the order of the wire
stack. See results R6 and R7.

**STK-TPL-004** (`source_suggested`, unspecified). A count of 0 in the wire form is a decode
error. An item id of air is a decode error. What a reader does with a negative count is not
specified. See experiment E21.

**STK-TPL-005** (`source_suggested`, wire). The optional item template is one boolean byte, then
the item template when the byte is `01`. See experiment E20.

**STK-TPL-006** (`source_suggested`, semantic). To make a stack from an item template, copy the
item, the count and the patch, then do strict validation. When strict validation rejects the
stack, the result is the empty stack. See experiment E22.

**STK-TPL-007** (`source_suggested`, semantic). The patch of an item template is in normal form
(STK-PAT-005). See experiment E3.

### 6.7 The hashed stack

**STK-HSH-001** (`source_suggested_black_box_confirmed`, wire). The hashed stack has these fields,
in this order:

| Field | Form | Condition |
|-------|------|-----------|
| present | boolean byte | always |
| item id | VarInt | present is `01` |
| count | VarInt | present is `01` |
| number of value entries, `A` | VarInt, at most 256 | present is `01` |
| value entries | `A` times: type id (VarInt), then the hash of the component (int, big-endian) | present is `01` |
| number of removal entries, `R` | VarInt, at most 256 | present is `01` |
| removal entries | `R` times: type id (VarInt) | present is `01` |

The empty stack is the one byte `00`. A number of entries larger than 256 is a decode error. The
serverbound `container_click` packet carries a hashed stack for each changed slot and for the
cursor. See result R3.

**STK-HSH-002** (`source_suggested`, wire). The value entries have no two entries with one type
id. The removal entries have no two entries with one type id. The order of the entries has no
meaning. See experiment E23.

**STK-HSH-003** (`source_suggested`, client agreement). A hashed stack matches a stack when one
of these conditions is true:

- the hashed stack is not present and the stack is empty;
- the hashed stack is present, the stack is not empty, and each condition of the list below is
  true.

The conditions are:

1. The item ids are equal.
2. The counts are equal.
3. The removal entries of the hashed stack name the same component types as the removal entries
   of the patch.
4. The hashed stack and the patch have the same number of value entries.
5. For each value entry of the patch, the hashed stack has a value entry of that type id. Its
   hash is equal to the hash of the component (STK-HSH-004).

A document for menus states what the server does when the stacks do not match.
See result R3 and experiment E23.

**STK-HSH-004** (`source_suggested_black_box_confirmed`, client agreement). The hash of a
component is the hash of its saved form (STK-HSH-005). A reference to a registry entry in the
saved form is the identifier string, as the saved form of a world file has it. The writer of the
saved form must omit each optional field that the game omits, because an extra field changes the
hash. See results R3 and R9.

**STK-HSH-005** (`source_suggested_black_box_confirmed`, client agreement). The hash of a node of
the saved form is the CRC-32C checksum of a byte string. CRC-32C has the reflected polynomial
`0x82F63B78`, the start value `0xFFFFFFFF` and a final inversion of each bit. The byte string of
each node kind is:

| Node | Byte string |
|------|-------------|
| absent value | `01` |
| map | `02`, then the entries, then `03` |
| list | `04`, then `u32le(hash)` of each element in order, then `05` |
| byte | `06`, then 1 byte |
| short | `07`, then 2 bytes, least significant byte first |
| int | `08`, then 4 bytes, least significant byte first |
| long | `09`, then 8 bytes, least significant byte first |
| float | `0A`, then the 4 bytes of the IEEE 754 bits, least significant byte first |
| double | `0B`, then the 8 bytes of the IEEE 754 bits, least significant byte first |
| string | `0C`, then the number of UTF-16 code units as 4 bytes, then each code unit as 2 bytes, all least significant byte first |
| boolean | `0D 00` for false, `0D 01` for true |
| byte array | `0E`, then each byte, then `0F` |
| int array | `10`, then each int as 4 bytes, least significant byte first, then `11` |
| long array | `12`, then each long as 8 bytes, least significant byte first, then `13` |

The length field of the string node is the number of UTF-16 code units of the string. It is not
the number of bytes and it is not the number of code points. The field is an unsigned 32-bit
number of 4 bytes, least significant byte first. A code point above `U+FFFF` is 2 code units: the
high surrogate, then the low surrogate. Thus the string `a\u{1F600}b` has the length 4 and the
code units `0061 D83D DE00 0062`. Its byte string is
`0C 04000000 6100 3DD8 00DE 6200`.

The hash sets no limit on the length of a string, and it does not normalize the string. The
saved form and the wire form have their own limits, which their rules state. The fixture has no
case with a surrogate that has no pair.

See results R8 and R9. The table at the end of this section gives the input of each case of the
fixture `hash_ops`.

**STK-HSH-006** (`source_suggested_black_box_confirmed`, client agreement). An entry of a map is
`u32le(hash of the key)` then `u32le(hash of the value)`. The key is a string node. Sort the
entries by the hash of the key as an unsigned 32-bit number. For equal key
hashes, sort by the hash of the value in the same way. The smallest number comes first. See result R9.

**STK-HSH-007** (`source_suggested_black_box_confirmed`, client agreement). The node kind of a
number is the kind that the saved form of the component type states. A small value does not
change the kind. For example, `minecraft:max_stack_size` is an int node, and a boolean field is a
boolean node. See result R8.

**STK-HSH-008** (`unresolved`, client agreement). The hash of a component whose type is not a
saved type is not decided. See open question Q4.

**STK-HSH-009** (`source_suggested`, client agreement). The float node and the double node of
STK-HSH-005 take the bit pattern of the value without a change. The hash does not make a NaN
canonical. Thus two NaN values with different bit patterns have different hashes, and `0.0` and
`-0.0` have different hashes. An implementation must keep the bit pattern of each float and each
double from the reader to the writer and to the hash. Two components that are equal by STK-EQ-005
can thus have different hashes. This occurs only for a NaN. See experiment E28.

#### The inputs of the fixture `hash_ops`

The fixture `hash_ops` records one hash for each of 26 case names. The table below gives the
input of each case and the hash that the game wrote. The hash is a signed 32-bit number. The
input is a tree of the nodes of STK-HSH-005, in this notation:

- `absent` is the absent value.
- `byte 1`, `short -2`, `int 1`, `long -5`, `float 1.5` and `double -2.25` are number nodes of the
  kind and the width that the word states. Each number is decimal.
- `true` and `false` are boolean nodes. A boolean node is not a byte node.
- `"hello"` is a string node. Each character is ASCII, or an escape `\u{X}` with the code point
  in hexadecimal.
- `[a, b]` is a list node with the elements in order.
- `{"k": a, "m": b}` is a map node. The entries are in the order in which the game adds them.
  STK-HSH-006 sorts them, thus this order does not change the hash.
- `byte array [..]`, `int array [..]` and `long array [..]` are array nodes with the elements in
  order. Each element is a signed decimal number of the width of the array.

| Case | Input | Hash |
|------|-------|------|
| `empty` | `absent` | -1609117614 |
| `empty_map` | `{}` | -982207288 |
| `empty_list` | `[]` | -1978007022 |
| `a_byte_1` | `{"a": byte 1}` | -1990988826 |
| `a_int_1` | `{"a": int 1}` | -108872706 |
| `a_short_neg2` | `{"a": short -2}` | 2053373670 |
| `str_empty` | `""` | 1615905556 |
| `str_hello` | `"hello"` | 773640809 |
| `str_astral` | `"a\u{1F600}b"` | 2126615229 |
| `list_ints` | `[int 1, int -2, int 300]` | 2000803920 |
| `nested` | `{"inner": {"x": "y", "n": long -5}, "flag": byte 1, "pos": int array [1, 2, 3]}` | -1698105082 |
| `int_array` | `int array [7, -8, 9]` | 709107336 |
| `long_array` | `long array [1099511627776, -1]` | -309389825 |
| `byte_array` | `byte array [1, 2, -3]` | 1491979440 |
| `float` | `float 1.5` | -1709970540 |
| `double` | `double -2.25` | -17727433 |
| `long` | `long 1234567890123` | -556570646 |
| `bool_true` | `true` | -1019818302 |
| `bool_false` | `false` | 828198337 |
| `byte_true` | `byte 1` | 1791337955 |
| `list_of_bools` | `[true, false]` | 1072798967 |
| `sixteen_keys` | a map with 16 entries, see below | 1387017585 |
| `mixed_sign_keys` | `{"k": "v", "k2": "v2", "count": "1", "id": "x"}` | 1656600283 |
| `key_a` | `"a"` | 754812039 |
| `key_h` | `"h"` | -1572085944 |
| `key_p` | `"p"` | 10153329 |

Notes on the cases:

- `sixteen_keys`: the keys are the 16 strings of one letter from `"a"` to `"p"`, in that order.
  The value of each key is an int node with the code point of the letter. Thus the entries are
  `"a": int 97`, `"b": int 98`, and so on to `"p": int 112`.
- `long_array`: the first element is 2 to the power 40.
- `byte_true`: the case is a byte node, not a boolean node. Compare its hash with `bool_true`.
- `a_byte_1`, `a_int_1`, `a_short_neg2`, `nested`: the value 1 of the key `"a"` and of the key
  `"flag"` is a byte node, not a boolean node.
- `mixed_sign_keys`: each value is a string node. The value of `"count"` is the string `"1"`.
  As a signed number, each of the 4 key hashes is negative. Thus this case does not separate the
  signed order from the unsigned order of STK-HSH-006.
- `key_a`, `key_h`, `key_p`: each case is one string node, not a map. They give three of the key
  hashes of `sixteen_keys`. As signed numbers, the hash of `"h"` is negative and the hashes of
  `"a"` and `"p"` are positive. Thus `sixteen_keys` fails when an implementation sorts the key
  hashes as signed numbers.

## 7. Four component types

### 7.1 Forms and ranges

**STK-CMP-001** (`source_suggested_black_box_confirmed`, semantic). `minecraft:max_stack_size`
holds one integer. The maximum stack size of a stack is the effective component, or 1 when the
stack has no such effective component. Most prototypes have the value 64. See result R8.

**STK-CMP-002** (`source_suggested_black_box_confirmed`, semantic). `minecraft:max_damage` holds
one integer. The maximum damage of a stack is the effective component, or 0 when the stack has no
such effective component. See result R8.

**STK-CMP-003** (`source_suggested_black_box_confirmed`, semantic). `minecraft:damage` holds one
integer. `minecraft:unbreakable` holds no data. Only its presence has a meaning.
See results R1 and R8.

**STK-CMP-004** (`source_suggested_black_box_confirmed`, wire). The forms of the four component
types are:

| Component type | Type id | Saved form | Range of the saved form | Wire form |
|----------------|---------|------------|-------------------------|-----------|
| `minecraft:max_stack_size` | 1 | int | `1..99` | VarInt |
| `minecraft:max_damage` | 2 | int | 1 or more | VarInt |
| `minecraft:damage` | 3 | int | 0 or more | VarInt |
| `minecraft:unbreakable` | 4 | the empty map `{}` | none | no bytes |

A value out of the range is not a valid saved form (STK-SAV-005). The reader of the wire form does
not check the range. See results R1, R6 and R8.

**STK-CMP-005** (`design`, wire). A server must not send a component of STK-CMP-004 with a value
out of the range of its saved form. Reason: the client cannot save or hash that component.

### 7.2 Damage

**STK-DMG-001** (`source_suggested`, semantic). A stack can take damage when these three
conditions are true:

- it has an effective component `minecraft:max_damage`;
- it has an effective component `minecraft:damage`;
- it has no effective component `minecraft:unbreakable`.

See experiment E24.

**STK-DMG-002** (`source_suggested`, semantic). The damage value of a stack is the value of the
effective component `minecraft:damage`, or 0 when the stack has none. The result is limited to the
range from 0 to the maximum damage. See experiment E24.

**STK-DMG-003** (`source_suggested`, semantic). A write of the damage value limits the new value
to the range from 0 to the maximum damage. Then it sets the component `minecraft:damage`
(STK-PAT-006). See experiment E24.

**STK-DMG-004** (`source_suggested`, semantic). A stack is damaged when it can take damage and its
damage value is larger than 0. A stack is broken when it can take damage and its damage value is
equal to the maximum damage or larger. See experiment E24.

**STK-DMG-005** (`source_suggested`, semantic). To apply an amount of damage `a` to a stack, do
these steps:

1. If the stack cannot take damage, stop. The stack does not change.
2. If `a` is 0, stop. The stack does not change.
3. Write the damage value plus `a` as the new damage value (STK-DMG-003).
4. If the stack is broken, the stack breaks (STK-DMG-006).

`a` can be negative. A negative amount decreases the damage value. The holder and the enchantments
of the stack can change `a` or cancel the damage before step 1. Another document states that.
See experiment E25.

**STK-DMG-006** (`source_suggested`, semantic). When a stack breaks, its count decreases by 1. The
operation reports that the stack broke, and reports the item. A stack with the count 1 thus
becomes the empty stack. See experiment E25.

**STK-DMG-007** (`source_suggested`, semantic). A second operation applies damage and never
breaks the stack. It limits the new damage value to the range from 0 to the maximum damage minus
1. The count does not change. See experiment E25.

**STK-DMG-008** (`source_suggested`, semantic). The next unit of damage breaks a stack when two
conditions are true. The stack can take damage. Its damage value is equal to the maximum damage
minus 1, or larger.
See experiment E24.

### 7.3 Strict validation

**STK-VAL-001** (`source_suggested`, semantic). Strict validation rejects a stack when one of
these conditions is true. A parity test compares the result, and not which condition gave it.

1. The stack has an effective component `minecraft:max_damage` and its maximum stack size is
   larger than 1.
2. An item template in an effective component `minecraft:container`, `minecraft:bundle_contents`
   or `minecraft:charged_projectiles` has a count larger than its own maximum stack size.
3. The count is larger than the maximum stack size.
4. The stack is broken (STK-DMG-004).

A document for bundles states one more condition for `minecraft:bundle_contents`.
See experiment E5.

**STK-VAL-002** (`source_suggested`, semantic). Strict validation applies in these places:

| Place | Result of a rejection |
|-------|-----------------------|
| a stack from an item template (STK-TPL-006) | the empty stack |
| a stack from the command form (STK-CMD-006) | the command fails |
| a loot function that sets the components of a stack | the stack keeps the patch that it had before |

Strict validation does not apply to the reader of a saved stack (STK-SAV-007) or of a wire stack.
See experiments E5 and E22.

**STK-VAL-003** (`source_suggested`, semantic). No rule connects `minecraft:damage` and
`minecraft:max_damage` in the patch at decode. A stack can have `minecraft:damage` without
`minecraft:max_damage`. Such a stack cannot take damage, and its damage value is 0.
See experiment E24.

## 8. The component types

**STK-TYP-001** (`black_box_confirmed`, wire). This game version has 122 component types, with the
type ids `0..121`. The registry `minecraft:data_component_type` in
`assets/mcrs/reports/registries.json` is the source of the type ids. An implementation must read
the type ids from that file.

**STK-TYP-002** (`source_suggested`, semantic). Three component types are not saved types:
`minecraft:creative_slot_lock`, `minecraft:additional_trade_cost` and
`minecraft:map_post_processing`. Each other component type is a saved type. See experiment E13.

**STK-TYP-003** (`source_suggested_black_box_confirmed`, wire). Each component type can occur in a
wire stack. The column "Wire" of the table states the kind of the wire form:

- `own`: the component type has a binary wire form of its own. The specification of its domain
  states that form.
- `NBT`: the wire form is the saved form of the component as one network NBT.

See result R1 and experiment E26.

**STK-TYP-004** (`source_suggested_black_box_confirmed`, semantic). The column "Stacks" has `yes`
when a component of the type holds item templates. `minecraft:custom_data`,
`minecraft:entity_data`, `minecraft:bucket_entity_data` and `minecraft:block_entity_data` hold
free NBT. That NBT can hold saved stacks, but the component does not read them. See result R6.

Each identifier in the table has the namespace `minecraft`.

| Type id | Component type | Saved | Wire | Stacks | Purpose |
|---------|----------------|-------|------|--------|---------|
| 0 | `custom_data` | yes | NBT | no | Free NBT of a pack or a tool. The game gives it no meaning. |
| 1 | `max_stack_size` | yes | own | no | The maximum stack size (section 7). |
| 2 | `max_damage` | yes | own | no | The maximum damage (section 7). |
| 3 | `damage` | yes | own | no | The damage value (section 7). |
| 4 | `unbreakable` | yes | own | no | The stack cannot take damage (section 7). |
| 5 | `use_effects` | yes | own | no | Sprint, vibrations and speed of the holder while the holder uses the item. |
| 6 | `custom_name` | yes | own | no | The name that a player or a pack gave to the stack. |
| 7 | `minimum_attack_charge` | yes | own | no | The attack charge, from 0 to 1, that an attack with the item needs. |
| 8 | `damage_type` | yes | own | no | The damage type of an attack with the item. |
| 9 | `item_name` | yes | own | no | The default name of the item. |
| 10 | `item_model` | yes | own | no | The identifier of the item model. |
| 11 | `lore` | yes | own | no | The lines of text below the name in the tooltip. |
| 12 | `rarity` | yes | own | no | The rarity, which sets the colour of the name. |
| 13 | `enchantments` | yes | own | no | The enchantments of the stack and their levels. |
| 14 | `can_place_on` | yes | own | no | The blocks on which a player in adventure mode can place the item. |
| 15 | `can_break` | yes | own | no | The blocks that a player in adventure mode can break with the item. |
| 16 | `attribute_modifiers` | yes | own | no | The attribute modifiers that the item gives in a slot. |
| 17 | `custom_model_data` | yes | own | no | Numbers, flags, strings and colours for the item model. |
| 18 | `tooltip_display` | yes | own | no | Hides the tooltip, or the lines of some component types. |
| 19 | `repair_cost` | yes | own | no | The added cost of the next anvil operation. |
| 20 | `creative_slot_lock` | no | own | no | Marks a slot of the creative screen of the client. |
| 21 | `enchantment_glint_override` | yes | own | no | Forces the glint on or off. |
| 22 | `intangible_projectile` | yes | NBT | no | A projectile from this stack cannot be picked up. |
| 23 | `food` | yes | own | no | Nutrition and saturation of the item. |
| 24 | `consumable` | yes | own | no | How a holder consumes the item, and the effects. |
| 25 | `use_remainder` | yes | own | yes | The stack that replaces the item after use. |
| 26 | `use_cooldown` | yes | own | no | The cooldown after use, and its group. |
| 27 | `damage_resistant` | yes | own | no | The damage types that do not hurt the item. |
| 28 | `tool` | yes | own | no | Mining speeds and correct blocks of a tool. |
| 29 | `weapon` | yes | own | no | Damage to the item for each attack, and the time it disables a block. |
| 30 | `attack_range` | yes | own | no | The reach and the hitbox margin of an attack. |
| 31 | `enchantable` | yes | own | no | The enchantment value of the item. |
| 32 | `equippable` | yes | own | no | The slot, sound and model of the item as equipment. |
| 33 | `repairable` | yes | own | no | The items that repair this item in an anvil. |
| 34 | `glider` | yes | own | no | The item lets its wearer glide. |
| 35 | `tooltip_style` | yes | own | no | The identifier of the tooltip style. |
| 36 | `death_protection` | yes | own | no | The item prevents the death of its holder, with effects. |
| 37 | `blocks_attacks` | yes | own | no | How the item blocks attacks while the holder uses it. |
| 38 | `piercing_weapon` | yes | own | no | The data of a piercing attack. |
| 39 | `kinetic_weapon` | yes | own | no | The data of a charge attack. |
| 40 | `attack_animation` | yes | own | no | The swing of an attack and its duration. |
| 41 | `interact_animation` | yes | own | no | The swing of an interaction and its duration. |
| 42 | `additional_trade_cost` | no | own | no | The added price of a trade result. |
| 43 | `block_transformer` | yes | own | no | The set of block changes that a use of the item on a block applies. |
| 44 | `villager_food` | yes | own | no | The food value of the item for a villager. |
| 45 | `stored_enchantments` | yes | own | no | The enchantments that an enchanted book holds. |
| 46 | `dye` | yes | own | no | The dye colour that the item supplies. |
| 47 | `dyed_color` | yes | own | no | The colour of a dyed item. |
| 48 | `map_id` | yes | own | no | The number of the map. |
| 49 | `map_decorations` | yes | NBT | no | The added icons of a map. |
| 50 | `map_post_processing` | no | own | no | A lock or a scale operation that waits for a map. |
| 51 | `charged_projectiles` | yes | own | yes | The projectiles in a crossbow. |
| 52 | `bundle_contents` | yes | own | yes | The stacks in a bundle. |
| 53 | `potion_contents` | yes | own | no | The potion, its colour and its custom effects. |
| 54 | `potion_duration_scale` | yes | own | no | The factor on the duration of the potion effects. |
| 55 | `suspicious_stew_effects` | yes | own | no | The effects of a suspicious stew. |
| 56 | `writable_book_content` | yes | own | no | The pages of a book and quill. |
| 57 | `written_book_content` | yes | own | no | The title, author, generation and pages of a written book. |
| 58 | `trim` | yes | own | no | The material and pattern of an armour trim. |
| 59 | `debug_stick_state` | yes | NBT | no | The property that the debug stick selected for each block. |
| 60 | `entity_data` | yes | own | no | The type and NBT of the entity that the item makes. |
| 61 | `bucket_entity_data` | yes | own | no | The NBT of the entity in a bucket. |
| 62 | `block_entity_data` | yes | own | no | The type and NBT of the block entity that the item places. |
| 63 | `instrument` | yes | own | no | The instrument of a goat horn. |
| 64 | `provides_trim_material` | yes | own | no | The trim material that the item supplies. |
| 65 | `ominous_bottle_amplifier` | yes | own | no | The amplifier of an ominous bottle. |
| 66 | `jukebox_playable` | yes | own | no | The song that a jukebox plays for the item. |
| 67 | `provides_banner_patterns` | yes | own | no | The banner patterns that the item supplies in a loom. |
| 68 | `recipes` | yes | NBT | no | The recipes that a knowledge book gives. |
| 69 | `lodestone_tracker` | yes | own | no | The target of a lodestone compass. |
| 70 | `firework_explosion` | yes | own | no | The explosion of a firework star. |
| 71 | `fireworks` | yes | own | no | The flight duration and the explosions of a rocket. |
| 72 | `profile` | yes | own | no | The player profile of a head. |
| 73 | `note_block_sound` | yes | own | no | The sound of a note block below this head. |
| 74 | `banner_patterns` | yes | own | no | The layers of a banner or a shield. |
| 75 | `base_color` | yes | own | no | The base colour of a shield. |
| 76 | `pot_decorations` | yes | own | yes | The four sides of a decorated pot. |
| 77 | `container` | yes | own | yes | The stacks in the slots of a container item. |
| 78 | `block_state` | yes | own | no | The block state properties that the placed block gets. |
| 79 | `bees` | yes | own | no | The bees in a nest or a hive. |
| 80 | `sulfur_cube_content` | yes | own | yes | The stack that a sulfur cube holds. |
| 81 | `lock` | yes | NBT | no | The predicate of the key that opens the placed container. |
| 82 | `container_loot` | yes | NBT | no | The loot table and seed of a container that is not yet open. |
| 83 | `break_sound` | yes | own | no | The sound when the stack breaks. |
| 84 | `compostable` | yes | own | no | The chance that the item adds a layer in a composter. |
| 85 | `cooking_fuel` | yes | own | no | The burn time and speed of the item as fuel. |
| 86 | `brewing_fuel` | yes | own | no | The uses and speed of the item as fuel of a brewing stand. |
| 87 | `mob_visibility` | yes | own | no | Changes how well some mobs see the wearer. |
| 88 | `villager/variant` | yes | own | no | The variant of a villager. |
| 89 | `wolf/variant` | yes | own | no | The variant of a wolf. |
| 90 | `wolf/sound_variant` | yes | own | no | The sound variant of a wolf. |
| 91 | `wolf/collar` | yes | own | no | The collar colour of a wolf. |
| 92 | `fox/variant` | yes | own | no | The variant of a fox. |
| 93 | `salmon/size` | yes | own | no | The size of a salmon. |
| 94 | `parrot/variant` | yes | own | no | The variant of a parrot. |
| 95 | `tropical_fish/pattern` | yes | own | no | The pattern of a tropical fish. |
| 96 | `tropical_fish/base_color` | yes | own | no | The base colour of a tropical fish. |
| 97 | `tropical_fish/pattern_color` | yes | own | no | The pattern colour of a tropical fish. |
| 98 | `mooshroom/variant` | yes | own | no | The variant of a mooshroom. |
| 99 | `rabbit/variant` | yes | own | no | The variant of a rabbit. |
| 100 | `pig/variant` | yes | own | no | The variant of a pig. |
| 101 | `pig/sound_variant` | yes | own | no | The sound variant of a pig. |
| 102 | `cow/variant` | yes | own | no | The variant of a cow. |
| 103 | `cow/sound_variant` | yes | own | no | The sound variant of a cow. |
| 104 | `chicken/variant` | yes | own | no | The variant of a chicken. |
| 105 | `chicken/sound_variant` | yes | own | no | The sound variant of a chicken. |
| 106 | `zombie_nautilus/variant` | yes | own | no | The variant of a zombie nautilus. |
| 107 | `frog/variant` | yes | own | no | The variant of a frog. |
| 108 | `horse/variant` | yes | own | no | The variant of a horse. |
| 109 | `painting/variant` | yes | own | no | The variant of a painting. |
| 110 | `llama/variant` | yes | own | no | The variant of a llama. |
| 111 | `axolotl/variant` | yes | own | no | The variant of an axolotl. |
| 112 | `cat/variant` | yes | own | no | The variant of a cat. |
| 113 | `cat/sound_variant` | yes | own | no | The sound variant of a cat. |
| 114 | `cat/collar` | yes | own | no | The collar colour of a cat. |
| 115 | `sheep/color` | yes | own | no | The colour of a sheep. |
| 116 | `shulker/color` | yes | own | no | The colour of a shulker. |
| 117 | `provides_pottery_pattern` | yes | own | no | The pot pattern that the item supplies. |
| 118 | `sign_text_front` | yes | own | no | The front text of the sign that the item places. |
| 119 | `sign_text_back` | yes | own | no | The back text of the sign that the item places. |
| 120 | `waxed` | yes | own | no | The sign that the item places is waxed. |
| 121 | `cushion/color` | yes | own | no | The colour of a cushion. |

A component type from `villager/variant` to `cushion/color`, except `provides_pottery_pattern`,
holds data of an entity or a block entity. The item copies the data to the entity or the block
entity that it makes.

### 8.1 Shapes of the first batch

This section states the shape of 15 component types. The counts of prototypes come from the 1686
files of `assets/mcrs/item_definition/`.

**STK-SHP-001** (`source_suggested_black_box_confirmed`, wire). Each rule from STK-SHP-006 to
STK-SHP-020 states one component type in a table. The columns are:

- "Field": the key of the field in the saved form. `(value)` means that the component is one
  value and not a map.
- "Saved form": the form of the value (STK-SHP-002).
- "Range": the values that the reader of the saved form accepts.
- "Absent": `necessary`, or the default that the reader takes when the key is absent.
- "Wire form": the bytes of the field. The wire form has the fields in the order of the rows.

These statements apply to each table:

- A value out of the range is not a valid saved form (STK-SAV-005). This is true also for an
  optional field.
- The reader of the saved form ignores a key that the table does not list.
- The writer of the saved form always writes a necessary field.
- The writer of the saved form omits an optional field whose value is equal to its default. An
  extra key changes the hash (STK-HSH-004).
- The wire form has each field, also a field whose value is equal to its default.
- The reader of the wire form does not check the range, unless the rule states a check.
- A number of elements in the wire form is a VarInt. A negative number is a decode error.
- "No limit" means that only the size of the file or of the packet limits the length.

The item definitions show the omitted defaults: 29 prototypes have `minecraft:tool` with only the
key `rules`. See experiment E29.

**STK-SHP-002** (`source_suggested_black_box_confirmed`, wire). The tables use these forms:

| Form | Saved form in NBT | Saved form in JSON | Hash node | Wire form |
|------|-------------------|--------------------|-----------|-----------|
| int | Int tag | number | int | VarInt |
| float | Float tag | number | float | 4 bytes, IEEE 754, big-endian |
| double | Double tag | number | double | 8 bytes, IEEE 754, big-endian |
| boolean | Byte tag, `0b` or `1b` | `false` or `true` | boolean | one byte, `00` or `01` |
| identifier | String tag | string | string | String (32767) of `login.md`, section 0.2 |
| text | STK-SHP-004 | STK-SHP-004 | as the saved form | one network NBT of the saved form |
| entry of a registry | String tag: the identifier of the entry | string | string | VarInt: the numeric id of the entry in that registry |
| name of a list | String tag | string | string | VarInt: the id of the name (STK-SHP-003) |

An int is a signed 32-bit integer. The reader of an identifier accepts a string without a
namespace and gives the namespace `minecraft`. The writer always writes the namespace. A string
that is not an identifier is an error in each format. An identifier or a numeric id that names no
entry of the registry is an error in each format. Experiment E29 decides if the reader of the
saved form accepts a number of a different tag kind. See results R1, R6 and R8.

**STK-SHP-003** (`source_suggested_black_box_confirmed`, wire). A name of a list is one of a fixed
set of strings. The reader of the saved form compares the string exactly. Each other string is an
error. The wire form is the id of the name. These lists occur in more than one rule:

| List | Names and ids |
|------|---------------|
| operation | `add_value` 0, `add_multiplied_base` 1, `add_multiplied_total` 2 |
| slot group | `any` 0, `mainhand` 1, `offhand` 2, `hand` 3, `feet` 4, `legs` 5, `chest` 6, `head` 7, `armor` 8, `body` 9, `saddle` 10 |
| display type | `default` 0, `hidden` 1, `override` 2 |

A writer must write an id of the list. The item definitions show the names `add_value`,
`add_multiplied_total`, `mainhand`, `body`, `head`, `feet`, `chest`, `legs` and `hidden`.
See experiment E30.

**STK-SHP-004** (`source_suggested_black_box_confirmed`, wire). A text is a text component of the
game. Its saved form is a string, a map or a list. This repository has no specification of the
text component yet (open question Q8). The rules of this section put no limit on the length of a
text. The writer writes a plain text without style as one string. The wire form is one network
NBT. Its reader gives a decode error for an NBT that is deeper than 512 levels. It also gives a
decode error for an NBT that is larger than a quota of 2097152 bytes. The exact measure of the
size is not specified. Two texts are equal when their content, their style and their children are
equal (STK-PAT-007). See results R1 and R6, and experiment E32.

**STK-SHP-005** (`design`, unspecified). A server must not send a component of this section with
a value out of the range of its saved form. Reason: the client cannot save or hash that component.
For an id that is not in a list of names, the reference reader of the wire form gives the name
with the id 0. A reader of this project can give a decode error instead. Reason: no vanilla writer
writes such an id.

**STK-SHP-006** (`source_suggested_black_box_confirmed`, wire). `minecraft:max_stack_size`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | int | `1..99` | necessary | VarInt |

Limits: none. Equality: the two values are equal. Prototypes: each prototype has this component.
The value is 64 for 1368 prototypes, 1 for 249 prototypes and 16 for 69 prototypes.
See STK-CMP-001 and result R8.

**STK-SHP-007** (`source_suggested_black_box_confirmed`, wire). `minecraft:max_damage`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | int | `1..2147483647` | necessary | VarInt |

Limits: none. Equality: the two values are equal. Prototypes: 84 prototypes have this component,
with 40 different values. The other 1602 prototypes do not have it. See STK-CMP-002 and result R8.

**STK-SHP-008** (`source_suggested_black_box_confirmed`, wire). `minecraft:damage`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | int | `0..2147483647` | necessary | VarInt |

Limits: none. Equality: the two values are equal. Prototypes: the 84 prototypes that have
`minecraft:max_damage` have this component with the value 0 (STK-PRO-006). No other prototype has
it. See results R1 and R8.

**STK-SHP-009** (`source_suggested_black_box_confirmed`, wire). `minecraft:unbreakable` has no
field. The saved form is the empty map `{}`. The wire form has no bytes. The reader of the saved
form accepts a map with keys and ignores each key. A value that is not a map is an error.
Equality: each two components of this type are equal. Prototypes: no prototype has this component.
See results R1 and R6, and experiment E29.

**STK-SHP-010** (`source_suggested_black_box_confirmed`, wire). `minecraft:custom_name`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | text | each text | necessary | one network NBT |

Limits: those of STK-SHP-004 only. A menu that renames a stack has a limit of its own. A document
for menus states it. Equality: as STK-SHP-004. Prototypes: no prototype has this component.
See results R1 and R6.

**STK-SHP-011** (`source_suggested_black_box_confirmed`, wire). `minecraft:item_name` has the
shape of `minecraft:custom_name` (STK-SHP-010). Prototypes: each prototype has this component.
Each value is a map with the one key `translate`. The string of that key is
`item.minecraft.<path>` or `block.minecraft.<path>`, and it is different for each item. An
implementation must read it from the file.

**STK-SHP-012** (`source_suggested_black_box_confirmed`, wire). `minecraft:item_model`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | identifier | each identifier | necessary | String (32767) |

The identifier names an item model of a resource pack. No registry holds it, and no reader checks
that the item model exists. Limits: the wire string holds at most 32767 UTF-16 code units. A
longer string is a decode error. Equality: the two identifiers are equal. Prototypes: each
prototype has this component. For each of the 1686 items, the value is the identifier of the item.

**STK-SHP-013** (`source_suggested_black_box_confirmed`, wire). `minecraft:lore`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | a list of text | 0 to 256 elements | necessary | the number of elements, then each text as one network NBT |

Limits: 256 elements. A saved form with more elements is not a valid saved form. A wire form with
a number larger than 256 is a decode error. No reader removes the elements past the limit.
Equality: the two lists have equal elements in the same order. Prototypes: each prototype has
this component with the empty list `[]`. See experiments E29 and E30.

**STK-SHP-014** (`source_suggested_black_box_confirmed`, wire). `minecraft:rarity`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | name of a list | `common` 0, `uncommon` 1, `rare` 2, `epic` 3 | necessary | VarInt: the id |

Limits: none. Equality: the two names are equal. Prototypes: each prototype has this component.
The value is `common` for 1570 prototypes, `uncommon` for 79, `epic` for 19 and `rare` for 18. An
implementation must read the value from the file. It must not assume `common`.

**STK-SHP-015** (`source_suggested_black_box_confirmed`, wire). `minecraft:enchantments` is a map
from an enchantment to a level:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | a map | no limit on the number of keys | necessary | the number of entries, then the entries |
| each key | entry of the registry `minecraft:enchantment` | each entry | | VarInt: the numeric id |
| each value | int | `1..255` | | VarInt, `0..255` |

`minecraft:enchantment` is a data-pack registry. The numeric id is the id that the server sent
for this connection (`registry.md`). The saved form does not accept an enchantment that is
written in place as a map. Limits: none on the number of entries. The reader of the wire form
gives a decode error for a level out of `0..255`. A server must not send the level 0, because
that component has no saved form. A writer must not write one enchantment two times. The order of
the entries in the wire form is the choice of the writer. Equality: the two maps have the same
enchantments with the same levels. The order has no effect. Prototypes: each prototype has this
component with the empty map `{}`. See experiments E29 and E30.

**STK-SHP-016** (`source_suggested_black_box_confirmed`, wire). `minecraft:attribute_modifiers`
is a list of entries. The saved form is a list of maps. The wire form is the number of entries,
then each entry. Each entry has these fields:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `type` | entry of the registry `minecraft:attribute` | each entry | necessary | VarInt: the numeric id |
| `id` | identifier | each identifier | necessary | String (32767) |
| `amount` | double | each double | necessary | 8 bytes |
| `operation` | name of the list "operation" | STK-SHP-003 | necessary | VarInt: the id |
| `slot` | name of the list "slot group" | STK-SHP-003 | `any` | VarInt: the id |
| `display` | a map (below) | below | `{type:"default"}` | below |

The map `display` has these fields:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `type` | name of the list "display type" | STK-SHP-003 | necessary | VarInt: the id |
| `value` | text | each text | necessary when `type` is `override`; not read for the other types | one network NBT, only when the id is 2 |

`minecraft:attribute` is a static registry. `id` names the modifier and is not an entry of a
registry. Limits: none on the number of entries. Equality: the two lists have equal entries in
the same order. Two entries are equal when each field is equal. `amount` is equal as STK-EQ-005
states. Prototypes: each prototype has this component. The value is the empty list `[]` for 1593
prototypes. The other 93 prototypes have 1 to 3 entries. No prototype has `slot` `any` or a
display of the type `override`. See experiment E31.

**STK-SHP-017** (`source_suggested_black_box_confirmed`, wire). `minecraft:tooltip_display`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `hide_tooltip` | boolean | `false`, `true` | `false` | one byte |
| `hidden_components` | a list of entries of the registry `minecraft:data_component_type` | each component type, also one that is not a saved type | the empty list | the number of elements, then each type id as a VarInt |

`hidden_components` is a set that keeps the order of its elements. A reader of each format drops
an element that occurred before in the list. The writers write the elements in the order of the
set. The hash uses that order (STK-HSH-005). Limits: none on the number of elements. Equality:
the two values of `hide_tooltip` are equal, and the two sets have the same elements. The order of
the elements has no effect on equality. This is an exception to STK-PAT-007. Prototypes: each
prototype has this component with the empty map `{}`. See experiments E30 and E31.

**STK-SHP-018** (`source_suggested_black_box_confirmed`, wire). `minecraft:repair_cost`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | int | `0..2147483647` | necessary | VarInt |

Limits: none. Equality: the two values are equal. Prototypes: each prototype has this component
with the value 0. See result R6.

**STK-SHP-019** (`source_suggested_black_box_confirmed`, wire). `minecraft:food`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `nutrition` | int | `0..2147483647` | necessary | VarInt |
| `saturation` | float | each float | necessary | 4 bytes |
| `can_always_eat` | boolean | `false`, `true` | `false` | one byte |

Limits: none. Equality: each field is equal. `saturation` is equal as STK-EQ-005 states.
Prototypes: 44 prototypes have this component. The other 1642 prototypes do not have it. 5 of the
44 have `can_always_eat`. The saved `saturation` is a float, for example `1.8000001`. An
implementation must not read it through a double that it rounds again.

**STK-SHP-020** (`source_suggested_black_box_confirmed`, wire). `minecraft:tool`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `rules` | a list of maps (below) | no limit on the length | necessary | the number of rules, then each rule |
| `default_mining_speed` | float | each float | `1.0` | 4 bytes |
| `damage_per_block` | int | `0..2147483647` | `1` | VarInt |
| `can_destroy_blocks_in_creative` | boolean | `false`, `true` | `true` | one byte |

Each rule has these fields:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `blocks` | a block set (below) | below | necessary | below |
| `speed` | float | larger than 0, at most `3.4028235e38` | no value | one boolean byte, then 4 bytes when the byte is `01` |
| `correct_for_drops` | boolean | `false`, `true` | no value | one boolean byte, then one byte when the first byte is `01` |

The range of `speed` excludes `0.0`, `-0.0`, each infinity and each NaN. A rule with no `speed`
does not set a speed. That is different from each number. A block set is a tag or a list of
entries of the registry `minecraft:block`:

| Block set | Saved form | Wire form |
|-----------|------------|-----------|
| a tag | the string `#<identifier of the tag>` | VarInt 0, then the identifier of the tag as String (32767) |
| a list of `n` blocks | a list of identifiers; one string when `n` is 1 | VarInt `n + 1`, then `n` numeric ids as VarInt |

The reader of the saved form accepts a list of one identifier and one string for one block. The
writer writes one string. A tag that does not exist is an error in each format. Limits: none on
the number of rules or of blocks. Equality: each field is equal, and the two lists of rules have
equal rules in the same order. The two float fields are equal as STK-EQ-005 states. Two block sets
are equal when they are the same tag. They are also equal when they are lists with the same blocks
in the same order. A tag is not equal to a list. Prototypes: 38 prototypes have this component.
The other 1648 prototypes do not have it. 29 of the 38 have only the key `rules`. See experiments
E29 and E32.

## 9. Intentional deviations

**D1.**

- **Rule**: STK-NET-004.
- **Reference**: the reference reader accepts two patch entries for one component type. The last
  value entry wins, and a removal entry wins over a value entry.
- **Visible to the client**: no. The vanilla server writes each component type one time.
- **Test**: a differential test must not send two patch entries for one component type.

**D2.**

- **Rule**: STK-CNT-005 and STK-CMP-005.
- **Reference**: the reference writer of the wire stack does not check the count or the range of a
  component. The reference reader of the wire stack accepts a count larger than 99.
- **Visible to the client**: yes, when a server sends such a stack. The client shows the count,
  and its next `set_creative_mode_slot` packet for that stack fails at the server.
- **Test**: a differential test must not make a stack with a count larger than 99.

**D3.**

- **Rule**: STK-CNT-003.
- **Reference**: the reference reader of the wire stack and of the hashed stack accepts each
  positive VarInt as the count.
- **mcrs**: the count has 8 bits. The reader of the wire stack gives a decode error for a count
  larger than 255. A hashed stack with a count larger than 255 is equal to no stack.
- **Visible to the client**: no. The vanilla client and the vanilla server send no count larger
  than 99.
- **Test**: a differential test must not make a stack with a count larger than 255.

## 10. Open questions

For the owner:

- **Q1**: STK-SAV-006. Which callers keep a stack whose patch has a bad key, and which drop the
  stack? Experiment E11 gives the reference behaviour for a world file. The owner decides if this
  project copies it for each caller.
- **Q2**: STK-DMG-006. A stack with a count larger than 1 that breaks keeps a damage value equal
  to the maximum damage. Strict validation prevents such a stack, but the saved reader does not.
  Does this project copy the reference, or does the remainder get the damage value 0?
- **Q3**: STK-CMD-001. Is the command form in the scope of this document, or does it move to an
  `argument-item.md` with the item predicate form?
- **Q4**: STK-HSH-008. A stack with `minecraft:map_post_processing` can be in a result slot when
  the client clicks. Which hash does the client send, or does the client fail? Experiment E27.
- **Q5**: STK-NET-010. Does the server reject a `set_creative_mode_slot` packet in which a
  component uses fewer bytes than its length?
- **Q6**: answered. STK-EQ-005, STK-EQ-006 and STK-HSH-009 state the rules. Experiment E28
  confirms them.
- **Q7**: STK-TYP-003. The wire form of each `own` component type needs a specification. Which
  document owns each group of types? Section 8.1 states the first 15 types.
- **Q8**: STK-SHP-004. The text component has no specification. Which document owns its saved
  form, its canonical form and its equality?
- **Q9**: STK-SHP-002. The four integer component types are signed 32-bit integers in the game.
  Does this project keep 32 bits, or use a smaller type with a deviation as D3?

Not decided by an experiment yet:

- **U1**: STK-SAV-004. The value of a `!<type>` key that is not `{}`. Experiment E10.
- **U2**: STK-CMD-003. If `[]` is valid in the command form. Experiment E15.
- **U3**: the command form with the item `minecraft:air`. Experiment E16.
- **U4**: STK-TPL-004. A negative count in the wire form of an item template. Experiment E21.
- **U5**: STK-SAV-001. If the reader of `count` accepts a Byte tag or a Short tag in NBT.
  Experiment E9.
- **U6**: STK-SHP-002. If the reader of an int field or a float field accepts a number of a
  different tag kind, and how it converts the number. Experiment E29.
- **U7**: STK-SHP-015. The result of a saved map with the keys `sharpness` and
  `minecraft:sharpness`, and of a wire form with one enchantment two times. Experiments E29 and
  E30.
- **U8**: STK-SHP-009. If the reader accepts a map with keys for `minecraft:unbreakable`.
  Experiment E29.
- **U9**: STK-SHP-004. If the string `"x"` and the map `{text:"x"}` decode to equal texts.
  Experiment E32.

## 11. Experiments

Each experiment uses the game version of the table at the top of this document. An experiment of
the kind "golden" adds cases to a golden of `tools/fixtures.json`. Run it with
`cargo run -p mcrs_minecraft_update -- recapture <name>`. The codecs of the game then write the
expected values. Where no golden has the case kind, the first step adds that kind to
`tools/vanilla-oracle`. An experiment of the kind "server" uses a vanilla server with
`online-mode=false` and one scripted client that is an operator.

| Experiment | Kind | Steps | Record |
|------------|------|-------|--------|
| E1 | golden | Decode the wire stack `01 00 00 00` (count 1, air). Decode a wire stack with the count 0. Read `minecraft:max_stack_size` of each result. | If each result is the empty stack; the component that the read gives. |
| E2 | server | Give `stone[max_stack_size=16]`, `stone[!max_stack_size]`, `stone` and `stone[damage=3]`. Read each stack with `data get`. Put 20 of each in one slot with `item replace`. | The saved patch; which counts the server accepts. |
| E3 | golden | Decode a saved stack, a wire stack and an item template of `minecraft:stone` with `max_stack_size` 64 in the patch, and with `!minecraft:damage`. Encode each result. | If the output has the patch entry. |
| E4 | golden | Decode a saved stack with the counts 0, -1, 99, 100 and 2147483647. Encode a stack with the count 100 to the saved form and to the wire form. | The error or the stack of each case. |
| E5 | server | Write a chest with `data merge block`: 64 diamond swords in one slot, a sword with `damage` equal to `max_damage`, and stone with `max_damage` 10. Read the block with `data get`. Do the same stacks with `give`. | Which stacks the block keeps; the error of each `give`. |
| E6 | server | Send `set_creative_mode_slot` in creative mode with the counts 99, 100 and 127, with `max_stack_size` 0 in the patch, and with 64 diamond swords. | For each case: the slot that the server holds, or the disconnect reason. |
| E7 | server | Put two stacks in two slots: equal patches written in a different key order, and patches that differ in one component. Move one on the other with a click. | If the stacks merge. |
| E8 | server | Click a stack of 1 damaged sword with `max_stack_size` 16 on an equal stack. Repeat with `damage` 0, and with `unbreakable`. | If the stacks merge, and the counts. |
| E9 | golden | Decode saved stacks: no `id`, `id` of an unknown item, `id` `minecraft:air`, `id` without a namespace, `count` as a Byte tag, `count` as a Short tag. | The error or the stack of each case. |
| E10 | golden | Decode saved stacks with the patch keys `damage`, `!repair_cost`, `!minecraft:repair_cost` with the values `{}`, `1b` and `"x"`. | The patch of each result, or the error. |
| E11 | server | Stop the server. Write a player inventory with one stack that has an unknown patch key, one with `minecraft:creative_slot_lock`, and one with `max_stack_size` 0. Start the server and read the inventory. | For each stack: absent, or present with which patch entries; the log line. |
| E12 | golden | Encode the empty stack as an optional saved stack. Decode `{}`, and decode a map with only `count`. | The output; the result of each decode. |
| E13 | golden | Encode a stack with `minecraft:additional_trade_cost` 3 to the saved form and to the wire form. | If the saved form has the key; the wire bytes. |
| E14 | server | Fill a chest with stacks in the slots 0 and 26 only. Read `Items` with `data get`. Write `Items` with a `Slot` of `-1b`, and with no `Slot`. | The elements of the list; the slot that each stack gets. |
| E15 | server | Run `give @s` with `stone[]`, `stone[ damage = 1 , !lore ]`, `stone[damage=1,]`, `stone[`, and `minecraft:stone[minecraft:damage=1]`. Read the suggestions packet of the client for each. | The result or the translation key of each case. |
| E16 | server | Run `give @s` with `nope`, `stone[nope=1]`, `stone[creative_slot_lock={}]`, `stone[damage=1,!damage]`, `stone[damage="x"]`, `stone[=1]` and `air`. | The translation key of each case. |
| E17 | server | Run `give @s diamond_sword[max_stack_size=16] 1`, `give @s stone[max_stack_size=4] 5`, and `item replace entity @s hotbar.0 with stone[max_stack_size=4] 5`. | The translation key of each case. |
| E18 | golden | Decode wire stacks with an item id of 100000, and with a type id of 122. | The error of each case. |
| E19 | golden | Encode the empty stack as a wire stack that is not empty. Decode the byte `00` in that format. | The error of each case. |
| E20 | golden | Encode a bundle that holds one damaged sword as a wire stack with length prefix. Encode a decorated pot with two empty sides. | The bytes; where a length occurs. |
| E21 | golden | Decode the wire form of an item template with the counts 0, -1 and 100, and with the item id of air. | The error or the item template of each case. |
| E22 | server | Give a bundle whose contents are 64 diamond swords, and one whose contents are a broken sword. Take the contents out. | The stacks that the player gets. |
| E23 | server | Send `container_click` with a hashed stack whose value entries are in reverse order, with one wrong hash, and with one type id two times. | If the server sends the slot again, or the disconnect reason. |
| E24 | server | Give `stone[damage=5]`, `stone[max_damage=10,max_stack_size=1,damage=50]` and a sword with `damage` 3 and `unbreakable`. Hit a block with each in survival mode. Read each stack. | The saved patch before and after; if the client shows a durability bar. |
| E25 | server | Give a sword with `damage` at the maximum damage minus 1. Break one block. Repeat with the item modifier `set_damage` with `add` -0.1 on a damaged sword. | The stack after each step; the packets of the break. |
| E26 | golden | Encode a stack with each of `custom_data`, `intangible_projectile`, `map_decorations`, `debug_stick_state`, `recipes`, `lock` and `container_loot`. | The wire bytes of each component. |
| E27 | server | Put a filled map and a glass pane in a cartography table. Click the result slot with a vanilla client. Capture the `container_click` packet. | The hashed stack of the result slot, or the failure of the client. |
| E28 | golden | Decode a stack whose patch has `minimum_attack_charge` equal to the prototype as `-0.0f` and `0.0f`. Decode wire stacks of an apple with `food` whose `saturation` has the bits `7fc00000`, `7fc00001`, `ffc00000`, `00000000` and `80000000`. Compare each pair of the results. Compare `custom_data` `{a:1}` with `{a:1b}`. Encode each result and hash each component. | If the first result has the patch entry; which pairs have the same item and components; the wire bytes and the hash of each. |
| E29 | golden | Decode saved components: `max_stack_size` as `64b`, `64L` and `64.5f`; `food` with `saturation` as `1.0d` and with no `saturation`; `food` with an unknown key; `unbreakable` as `{a:1}` and as `1b`; `rarity` as `"COMMON"`; `lore` with 257 lines; `enchantments` with the levels 0 and 256, and with the keys `sharpness` and `minecraft:sharpness`; `tool` with a rule `speed` of `0.0f`, `-0.0f` and `Infinityf`; `tooltip_display` with `hide_tooltip` `"x"`. Encode each result. | The error or the saved form of each case. |
| E30 | golden | Decode wire components: `rarity` with the ids 4 and -1; a slot group with the id 11; `enchantments` with the levels 0 and 256, and with one enchantment two times; `lore` with the number 257; `tooltip_display` with one type id two times. Encode each result to the saved form and hash it. | The error or the component of each case; the saved form and the hash. |
| E31 | server | Put two stacks in two slots: `tooltip_display` with `hidden_components` `[lore,enchantments]` and `[enchantments,lore]`. Repeat with two `attribute_modifiers` entries in the two orders. Move one stack on the other with a click. | If the stacks merge. |
| E32 | golden | Decode saved components: `item_name` as `"x"`, `{text:"x"}` and `["x"]`; `tool` with `blocks` as `"minecraft:stone"`, `["minecraft:stone"]`, `[]` and `"#minecraft:nope"`. Compare the results. Encode each result and hash it. | Which results are equal; the saved form, the wire bytes and the hash of each. |

## 12. Captured results

Each result is a case of a golden that the game wrote. The fixture name is a name of
`tools/fixtures.json`.

| Result | Fixture | Case | Bytes or value |
|--------|---------|------|----------------|
| R1 | `inventory_packets` | `container_set_slot` | `03 07 0005 01 b108 03 00 03 07 06 0800056e616d6564 04` |
| R2 | `inventory_packets` | `container_set_slot_empty` | `00 01 ffff 00` |
| R3 | `inventory_packets` | `container_click` | `01 02 0003 00 00 02 0004 00 0003 01 b108 01 03 03 9915c56e 04 c574b4c8 06 d2ee6d5e 00 01 8608 01 00 00` |
| R4 | `inventory_packets` | `set_creative_mode_slot` | `0024 01 b108 03 00 03 01 07 06 08 0800056e616d6564 04 00` |
| R5 | `inventory_packets` | `set_creative_mode_slot_empty` | `ffff 00` |
| R6 | `item_nested` | `use_remainder_full` | saved: `{"id":"minecraft:diamond_sword","count":3,"components":{"minecraft:max_stack_size":16,"minecraft:damage":7,"minecraft:custom_name":"named","minecraft:unbreakable":{},"!minecraft:repair_cost":{}}}`; wire: `b108 03 04 01 01 10 03 07 06 0800056e616d6564 04 13`; hash: -1619506781 |
| R7 | `item_nested` | `use_remainder_plain` | saved: `{"id":"minecraft:stone"}`; wire: `01 01 00 00`; hash: 725422587 |
| R8 | `item_plain` | `max_stack_size` 64, `damage` 0, `max_damage` 1561 | `max_stack_size`: wire `40`, NBT `03 00000040`, hash 733160003. `damage`: hash -2146075228. `max_damage`: wire `990c`. |
| R9 | `hash_ops` | the node kinds | absent value: -1609117614. Empty map: -982207288. Empty list: -1978007022. True: -1019818302. False: 828198337. Float 1.5: -1709970540. String `hello`: 773640809. Empty string: 1615905556. Section 6.7 gives the input and the hash of each of the 26 cases. |

Notes on the results:

- R1: the fields before the stack are the container id 3, the state id 7 and the slot 5. The stack
  has the count 1, the item id 1073 (`minecraft:diamond_sword`), 3 value entries and 0 removal
  entries. The value entries are `damage` 7, `custom_name` as network NBT, and `unbreakable` with
  no bytes.
- R3: the packet has 2 changed slots. Slot 4 has a hashed stack that is not present. Slot 3 has
  the item id 1073, the count 1 and 3 value entries. The hash `9915c56e` is the hash of the int 7.
  The hash `c574b4c8` is the hash of the empty map. The hash `d2ee6d5e` is the hash of the string
  `named`. The cursor has the item id 1030, the count 1 and no patch entries.
- R4: the lengths of the 3 components are 1, 8 and 0.
- R6: the wire form has the item id, the count 3, 4 value entries and 1 removal entry. The removal
  entry is the type id 19 (`repair_cost`). The hash is the hash of the saved map, with the unsigned
  order of STK-HSH-006. The signed order gives 1172329907, which is not the value of the game.
- R9: the `float` case of the fixture is the float 1.5. An earlier revision of this row gave
  "Float 0.5: -1631070359". The fixture has no case with the float 0.5, thus the game did not
  record that number. The number is the result of STK-HSH-005 for the float 0.5.
