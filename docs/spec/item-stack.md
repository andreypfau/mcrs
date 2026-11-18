# Item stack: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states the item stack as data: its parts, its component patch, its count, and each
format that carries it. It states the meaning of four component types, and it lists each component
type of this game version.

Sections 8.1 to 8.6 state the shape of 40 component types. For
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
- **Player name**: a string of 0 to 16 UTF-16 code units, each in the range `U+0021..U+007E`
  (STK-SHP-024).
- **Property**: a name, a value and an optional signature, all strings, in a profile
  (STK-SHP-025).
- **Property group**: the properties of one profile with one name (STK-SHP-025).
- **Full profile**, **partial profile**: the two forms of the component `minecraft:profile`
  (STK-SHP-023).
- **Dynamic profile**: a partial profile with no property and with exactly one of a player name and
  a UUID (STK-SHP-027).
- **Sound event**: a reference to an entry of the registry `minecraft:sound_event`, or an inline
  sound event (STK-SHP-029).
- **Entity type set**: a tag or a list of entries of the registry `minecraft:entity_type`
  (STK-SHP-030).
- **Colour**: an int with an alpha, a red, a green and a blue channel (STK-SHP-032).
- **Referenced value**: a reference to an entry of a registry, or an inline value of that registry
  (STK-SHP-033).
- **Effect instance**: a status effect with its amplifier, its duration and its flags
  (STK-SHP-034).
- **Dye colour**: one of the 16 names of STK-SHP-048.
- **Entity data**: an entity type and a compound of free NBT (STK-SHP-049).
- **NBT depth**: the number of levels of the saved form of a stack in NBT (STK-TPL-009).

### 0.2 Provenance

- `black_box_confirmed`: shown by an artefact that the game writes.
- `source_suggested_black_box_confirmed`: taken from the reference behaviour, and consistent with an
  artefact.
- `source_suggested`: taken from the reference behaviour, with no result. The confidence is at most
  medium.
- `design`: a choice of this project. The rule states the reason.
- `unresolved`: not decided. Section 10 collects these items.

### 0.3 Artefacts cited

- `assets/mcrs/reports/registries.json`: the registry report of the game. It gives each type id
  and each item id.
- `assets/mcrs/reports/packets.json`: the packet id report of the game.
- `assets/mcrs/item_definition/`: one file for each item, which the game writes through
  `tools/vanilla-oracle`. STK-PRO-002 states the keys.
- The fixtures `hash_ops`, `item_plain`, `item_nested`, `item_records`, `item_holders`,
  `item_registry_refs` and `inventory_packets` of `tools/fixtures.json`. The codecs of the game write each expected value of these goldens.

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
| `STK-SHP` | the shapes of the component types of sections 8.1 to 8.6 |

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

## 2. The component patch

**STK-PAT-001** (`source_suggested_black_box_confirmed`, semantic). A component patch has at most
one patch entry for each component type. A value entry states that the stack has that component.
A removal entry states that the stack does not have a component of that component type.

**STK-PAT-002** (`source_suggested`, semantic). A read of a stack for a component type gives one
of three results:

| Result | Condition | Effective component |
|--------|-----------|---------------------|
| overridden | the patch has a value entry for the component type | the component of the value entry |
| removed | the patch has a removal entry for the component type | none |
| default | the patch has no patch entry for the component type | the component of the prototype, or none when the prototype has none |


**STK-PAT-003** (`source_suggested`, semantic). A read of the empty stack gives no effective
component for each component type.

**STK-PAT-004** (`source_suggested`, semantic). A patch entry has no effect in these two cases:

- a value entry whose component is equal to the component of the prototype for that component type;
- a removal entry for a component type that the prototype does not have.

A component patch is in normal form when it has no such patch entry.

**STK-PAT-005** (`source_suggested`, semantic). The component patch of each stack is in normal
form. A reader of a stack in each format of section 6 must drop each patch entry without effect.
A writer of a stack thus writes no such patch entry. This rule does not apply to the patch of an
item template (STK-TPL-007). It does not apply to the reader of a hashed stack: a hashed stack
carries a hash in place of each value, and the reader keeps each entry for STK-HSH-003.

**STK-PAT-006** (`source_suggested`, semantic). A change of one component of a stack keeps the
normal form:

| Change | Prototype | Result in the patch |
|--------|-----------|---------------------|
| set a component | has an equal component | no patch entry for the component type |
| set a component | has a different component, or none | a value entry |
| remove a component | has a component | a removal entry |
| remove a component | has none | no patch entry for the component type |


**STK-PAT-007** (`design`, semantic). Two components of one component type are equal when their
saved forms are equal as trees. In a tree, the order of the keys of a map has no effect. The
order of the elements of a list has an effect. For a component type that is not a saved type, two
components are equal when their wire forms are equal.

Reason: a client can observe equality only
through a format.

The saved form of this rule is the form that the writer gives for the component. It is not the
input text of a reader. Two inputs that decode to one component are thus equal. STK-EQ-005 states
equality of two numbers in floating point. STK-EQ-006 states equality of two numbers in free NBT.
STK-SHP-017 states one exception for `minecraft:tooltip_display`. STK-SHP-027 states the exceptions
for `minecraft:profile`.

**STK-PAT-008** (`source_suggested`, semantic). A component patch can have a value entry for a
component type that the prototype does not have. A component patch can have a value entry for
each component type, also for a type that has no use for that item.

## 3. The empty stack

**STK-EMP-001** (`source_suggested_black_box_confirmed`, semantic). A stack is the empty stack when
one of these conditions is true:

- the item is air;
- the count is 0 or less.

Only one empty stack exists. A reader of a format that has an empty stack must give the empty stack
for such a value. The saved stack has no empty stack: its reader gives an error (STK-SAV-002,
STK-CNT-003).

**STK-EMP-002** (`source_suggested`, semantic). The empty stack has the item air, the count 0 and
an empty component patch. No data of the item, the count or the patch of the value that became
empty remains.

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


## 4. The count

**STK-CNT-001** (`source_suggested`, semantic). The count of a stack that is not empty is 1 or
more. The largest count that every format carries without error is 99.
A format can accept a larger count (STK-CNT-003).

**STK-CNT-002** (`source_suggested`, semantic). The maximum stack size of a stack is an effective
component (STK-CMP-001). A patch can change it for one stack. A stack can have a count larger than
its maximum stack size. Only strict validation rejects such a stack (STK-VAL-001).

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
| item template, wire form | each VarInt that is not 0 | 0 is a decode error. The reader accepts a negative count and a count larger than 99 (STK-TPL-004). |
| hashed stack | each VarInt | The reader accepts each value. The value only takes part in STK-HSH-003. |


**STK-CNT-004** (`source_suggested`, semantic). A stack with a count larger than 99 has no saved
form. The writer reports an error and writes no stack for it.

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

**STK-EQ-002** (`source_suggested`, semantic). Two stacks are equal when they have the same item
and components and the same count.

**STK-EQ-003** (`source_suggested`, semantic). A stack can stack when these two conditions are
true:

- its maximum stack size is larger than 1;
- it cannot take damage (STK-DMG-001), or its damage value is 0.


**STK-EQ-004** (`source_suggested`, semantic). Two stacks can merge when they have the same item
and components. The count of the result must not be larger than the maximum stack size. The game
has no one merge rule for each caller. Some callers also do the check of STK-EQ-003, and other
callers do not:

| Caller | Check of STK-EQ-003 |
|--------|---------------------|
| the pick up click, the drag and the pick up all click of a menu | none |
| the quick move click of a menu, for a slot that is not empty | on the stack that moves |
| the insert of a stack into the inventory of a player | on the stack that receives |

The two stacks have the same components. The check thus gives the same answer for the stack that
moves and for the stack that receives. A caller without the check gives a different result only
for a stack that strict validation rejects (STK-VAL-001). Such a stack has a maximum stack size
larger than 1 and a damage value larger than 0. `inventory-transactions.md` states how a merge
moves the count.

**STK-EQ-005** (`source_suggested`, semantic). In equality of two components, two float values are
equal when one of these conditions is true:

- each of the two values is a NaN;
- no value is a NaN, and the two values have the same bit pattern.

The same rule applies to two double values. Equality does not use the numeric comparison of
IEEE 754. Thus each NaN is equal to each other NaN, with each payload and each sign. Thus `0.0`
and `-0.0` are not equal. This rule applies to STK-PAT-004, STK-PAT-006, STK-EQ-001 and STK-EQ-004.

**STK-EQ-006** (`source_suggested`, semantic). In free NBT (STK-TYP-004), two numbers of different
tag kinds are not equal. For example, `{a:1}` and `{a:1b}` are not equal. Two float tags or two
double tags are equal as STK-EQ-005 states. A field with a known number kind is different: its
reader gives one number kind, and equality compares the values after the decode.

## 6. Formats

### 6.1 The saved stack

**STK-SAV-001** (`source_suggested_black_box_confirmed`, wire). The saved stack is a map with
these keys:

| Key | Saved form | Reader | Writer |
|-----|------------|--------|--------|
| `id` | a string: the identifier of the item | necessary | always writes it |
| `count` | an int | optional; the default is 1; the range is `1..99` | always writes it |
| `components` | a map: the component patch (STK-SAV-003) | optional; the default is an empty patch | omits it when the patch is empty |

In NBT, `count` is an Int tag. The order of the keys has no meaning.

**STK-SAV-002** (`source_suggested`, semantic). The reader gives an error and no stack in each of
these cases:

- `id` is absent, or is not an identifier;
- `id` does not name an item;
- `id` names air;
- `count` is out of range (STK-CNT-003).

The saved stack has no form for the empty stack.

**STK-SAV-003** (`source_suggested_black_box_confirmed`, wire). The component patch is a map. Each
key is one of these:

| Key | Value | Meaning |
|-----|-------|---------|
| `<type>` | the saved form of the component | a value entry |
| `!<type>` | the empty map `{}` | a removal entry |

`<type>` is the identifier of the component type. The writer writes it with the namespace.

**STK-SAV-004** (`source_suggested`, semantic). The reader accepts a key without a namespace and
gives it the namespace `minecraft`. The value of a `!<type>` key must be a map. The reader ignores
the keys of that map. A value that is not a map, for example `1b` or `"x"`, is an error of that
key (STK-SAV-005).

**STK-SAV-005** (`source_suggested`, semantic). A key of the patch is an error when one of these
conditions is true:

- `<type>` does not name a component type;
- `<type>` names a component type that is not a saved type;
- the value is not a valid saved form of the component;
- the value of a `!<type>` key is not a map (STK-SAV-004);
- an earlier key of the same kind names the same component type.

Two keys name the same component type when their identifiers are equal after the reader adds the
namespace. For example, `damage` and `minecraft:damage` are two value keys for one component type.
The second key in the order of the input is the error. A partial result keeps the entry of the
first key (STK-SAV-006).

A value key and a removal key for one component type are not an error for the reader. The patch
entry of the key that is later in the order of the input replaces the earlier one. JSON has the
order of the document. An NBT compound has no order, thus the result in NBT is not specified. A
writer must not write a value key and a removal key for one component type.


**STK-SAV-006** (`source_suggested`, semantic). When a key of the patch is an error, the reader
reports the error. The reader of a world file then gives the stack with the patch entries that
decode. A reader that has no partial result gives no stack. A design note states which reader each
caller uses.

**STK-SAV-007** (`source_suggested`, semantic). The reader of the saved stack does not do strict
validation. The reader accepts a count larger than the maximum stack size, and a damage value
equal to the maximum damage.

**STK-SAV-008** (`source_suggested`, wire). The optional saved stack is the saved stack or the
empty map `{}`. The writer writes `{}` for the empty stack. The reader gives the empty stack for a
map that has no keys.

**STK-SAV-009** (`source_suggested`, semantic). The writer omits each patch entry of a component
type that is not a saved type. The writer reports no error for it.

**STK-SAV-010** (`source_suggested`, wire). The saved stack with slot is the saved stack with one
more key:

| Key | Saved form | Reader | Writer |
|-----|------------|--------|--------|
| `Slot` | a byte, read as unsigned: `0..255` | optional; the default is 0 | always writes it |

A list of such maps holds the slots of a container. The writer writes no element for a slot that
holds the empty stack. A slot number larger than 255 is an error of the writer.

**STK-SAV-011** (`source_suggested_black_box_confirmed`, wire). The saved form has no other
variant for SNBT. In SNBT, the saved stack is the same map in the syntax of `snbt.md`. In JSON,
`count` is a number.

### 6.2 The command form

**STK-CMD-001** (`source_suggested`, client agreement). The command form states an item and a
component patch. It has no count and no form for the empty stack. Its grammar is:

```
form     = item [ "[" [ entry { "," entry } ] "]" ]
entry    = type "=" value | "!" type
```

`item` is the identifier of an item. `type` is the identifier of a component type. `value` is the
SNBT of the saved form of the component, as `snbt.md` states it. Whitespace can occur before and
after each `entry`, `=` and `,`.

**STK-CMD-002** (`source_suggested`, client agreement). An identifier without a namespace gets the
namespace `minecraft`. A `type "=" value` entry is a value entry. A `"!" type` entry is a removal
entry.

**STK-CMD-003** (`source_suggested`, client agreement). A `,` with no entry after it is an error.

**STK-CMD-004** (`source_suggested`, client agreement). The parser gives these errors. Each error
is a translation key of the game.

| Condition | Translation key |
|-----------|-----------------|
| the identifier does not name an item | `argument.item.id.invalid` |
| no component type is at the position of `type` | `arguments.item.component.expected` |
| `type` does not name a component type, or names a type that is not a saved type | `arguments.item.component.unknown` |
| one component type occurs in two entries, of each kind | `arguments.item.component.repeated` |
| `value` is not a valid saved form of the component | `arguments.item.component.malformed` |


**STK-CMD-005** (`source_suggested`, semantic). The component patch of the result is in normal
form (STK-PAT-005).

**STK-CMD-006** (`source_suggested`, semantic). A command that makes a stack of count `n` from the
command form does these checks, in this order:

1. If `n` is larger than the maximum stack size of the result, the error is
   `arguments.item.overstacked`.
2. If strict validation rejects the result, the error is `arguments.item.malformed`.


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

The item id has no offset. The empty stack is the one byte `00`.

**STK-NET-002** (`source_suggested_black_box_confirmed`, wire). The wire form of a component
depends on its component type (STK-TYP-003). No length and no end mark separates two components.
A reader must know the wire form of each component type to read a stack.

**STK-NET-003** (`source_suggested`, wire). The reader gives a decode error for an item id or a
type id that names no entry. The reader gives the empty stack when the count is larger than 0 and
the item is air.

**STK-NET-004** (`design`, wire). A writer must not write two patch entries for one component
type. The order of the patch entries is the choice of the writer. What a reader does with two
patch entries for one component type is not specified. Reason: the vanilla client and the vanilla
server write each component type one time.

**STK-NET-005** (`source_suggested_black_box_confirmed`, wire). These packets carry a wire stack:
the clientbound `container_set_slot`, `container_set_content`, `set_cursor_item`,
`set_player_inventory` and `set_equipment`. A value of the item stack kind in the entity data
carries one too. In `container_set_content`, a VarInt number of wire stacks comes before the
stack of the cursor.

**STK-NET-006** (`source_suggested`, wire). The wire stack that is not empty has the same bytes
as the wire stack. A count of 0 or less is a decode error. A count larger than 0 with the item
air is also a decode error. In that case the reader reads the bytes of the patch before it gives
the error. A writer must not give it the empty stack. The result of a merchant offer uses this
format.

### 6.4 The wire stack with length prefix

**STK-NET-007** (`source_suggested_black_box_confirmed`, wire). The wire stack with length prefix
has the fields of STK-NET-001, with one change. In each value entry, a VarInt comes between the
type id and the component. This VarInt is the number of bytes of the wire form of the component.
The serverbound `set_creative_mode_slot` packet carries this format.

**STK-NET-008** (`source_suggested`, wire). A component type whose wire form has no bytes has the
length 0.

**STK-NET-009** (`source_suggested`, semantic). After the reader decodes a stack that is not
empty, the stack must have a saved form. The reader gives a decode error of the packet when one of
these conditions is true:

- the count is larger than 99;
- a component of a saved type in the patch has no valid saved form.

The check ignores the patch entries of a component type that is not a saved type. The check does
not do strict validation.

**STK-NET-010** (`source_suggested`, wire). The length of STK-NET-007 sets the bounds of the
component. The reader does this:

| Case | Result |
|------|--------|
| the length is negative | a decode error |
| the length is larger than the number of bytes that remain in the packet | a decode error |
| the wire form of the component needs more bytes than the length | a decode error |
| the wire form of the component uses fewer bytes than the length | a decode error |

After the component, the reader continues at the first byte after the length. The length has no
maximum of its own. A writer must write the exact length. The last case of the table is stricter
than the reference. Deviation D4 records the reference behaviour.

### 6.5 Where a nested format applies

**STK-NET-011** (`source_suggested`, wire). The length prefix of STK-NET-007 applies only to the
value entries of the outer stack. An item template inside a component uses the wire form of
STK-TPL-003, with no length prefix.

### 6.6 The item template

**STK-TPL-001** (`source_suggested_black_box_confirmed`, semantic). An item template has an item,
a count and a component patch. The item is not air and the count is not 0. A component that holds
stacks holds item templates (STK-TYP-004). An item template is data. It does not change while its
owner holds it.

**STK-TPL-002** (`source_suggested_black_box_confirmed`, wire). The saved form of an item template
is a map with these keys:

| Key | Saved form | Reader | Writer |
|-----|------------|--------|--------|
| `id` | a string: the identifier of the item | necessary | always writes it |
| `count` | an int, range `1..99` | optional; the default is 1 | omits it when the count is 1 |
| `components` | a map: the component patch (STK-SAV-003) | optional; the default is an empty patch | omits it when the patch is empty |

The reader also accepts one string, the identifier of the item. That string means the count 1 and
an empty patch. The writer always writes the map.

The item air is an error in the map and in the string form. The text of the error contains `Item
must not be minecraft:air`. A count out of `1..99` is an error with the text `Value must be within
range [1;99]: <count>`.

**STK-TPL-003** (`source_suggested_black_box_confirmed`, wire). The wire form of an item template
has these fields, in this order: item id (VarInt), count (VarInt), then the patch as STK-NET-001
states it. The item id comes before the count. This order is different from the order of the wire
stack.

**STK-TPL-004** (`source_suggested`, wire). A count of 0 or an item id of air in the wire form is a
decode error (STK-TPL-008). The reference reader accepts a negative count and a count
larger than 99. The item template then holds that count. A stack that someone makes from an item
template with a negative count is the empty stack (STK-EMP-001). A writer must not write a count
out of `1..99`.

**STK-TPL-005** (`source_suggested`, wire). The optional item template is one boolean byte, then
the item template when the byte is `01`.

**STK-TPL-006** (`source_suggested`, semantic). To make a stack from an item template, copy the
item, the count and the patch, then do strict validation. When strict validation rejects the
stack, the result is the empty stack.

**STK-TPL-007** (`source_suggested`, semantic). The game does not bring the patch of an item
template to normal form. The item template keeps each patch entry that its reader gets, also a
patch entry without effect (STK-PAT-004). The writers of the item template write each of these
patch entries. The hash of the component that owns the item template includes them
(STK-HSH-004). A stack that someone makes from the item template has a patch in normal form
(STK-TPL-006 and STK-PAT-005).

**STK-TPL-008** (`source_suggested_black_box_confirmed`, wire). The reader of the wire form of an
item template reads the item id, the count and the patch. Then it checks the item and the count. An
item of air or a count of 0 is a decode error with the text `Item must be non-empty`. The two causes
give the same text. When the patch has an error, the reader gives the error of the patch. Only the
reader of the saved form gives the text `Item must not be minecraft:air` (STK-TPL-002). These
component types use this reader in their wire forms:

| Component type | Item templates in the wire form |
|---|---|
| `minecraft:use_remainder` | one item template |
| `minecraft:sulfur_cube_content` | one item template |
| `minecraft:charged_projectiles` | each element of the list (STK-SHP-052) |
| `minecraft:bundle_contents` | each element of the list (STK-SHP-051) |
| `minecraft:container` | each element of the list, as an optional item template (STK-SHP-053) |
| `minecraft:pot_decorations` | each of the four sides, as an optional item template (STK-TPL-005) |

An optional item template with the byte `00` holds nothing, and the reader does no check. For
example, the wire form `01 00 01 00 00` of `minecraft:bundle_contents` has one element with the item
air and the count 1. It is a decode error with the text `Item must be non-empty`.

**STK-TPL-009** (`source_suggested`, wire). The NBT depth of a stack is the depth of the saved form
of the stack in NBT. That saved form is the tree that the writer of the saved stack writes
(STK-SAV-001), not the tree of the input. Count the depth in this way:

1. The Compound tag of the saved stack has the level 1.
2. A tag inside a Compound tag or a List tag has the level of that tag plus 1.
3. The NBT depth is the highest level of a Compound tag or a List tag in the tree.

Only a Compound tag and a List tag count. A tag of each other kind adds no level, also an array
tag. An empty Compound tag and an empty List tag count. A wrapper of `text-component.md`,
TXT-NBT-002, is a Compound tag, thus it counts. The NBT reader counts in the same way. It refuses a
Compound tag or a List tag at a level above 512 (TXT-NBT-005).

The writer writes the map form of each value, also when the input has a short form. Thus the string
form of an item template (STK-TPL-002) counts as a map with the key `id`. Each kind of nesting adds
these levels from one item template to the item template inside it:

| Component type | Tags from an item template to the next item template | Levels |
|---|---|---|
| `minecraft:use_remainder`, `minecraft:sulfur_cube_content` | the map `components`, the next item template | 2 |
| `minecraft:pot_decorations` | the map `components`, the map of the component, the next item template | 3 |
| `minecraft:bundle_contents`, `minecraft:charged_projectiles` | the map `components`, the list, the next item template | 3 |
| `minecraft:container` | the map `components`, the list, the map of the slot, the next item template | 4 |

A component of each other type adds the levels of its own saved form. For example,
`minecraft:custom_data` of the outer stack is a Compound tag at the level 3. Thus its own tree can
have at most 510 levels in a stack with an NBT depth of 512.

A file or a packet can hold the saved stack below its own root. The NBT reader of that file counts
the levels from the root of the file. Thus the file can be deeper than 512 levels when the stack is
not. See U5.

**STK-TPL-010** (`design`, deviation). Each reader of a stack must refuse a stack with an NBT depth
larger than 512. A stack with the NBT depth 512 is valid. The readers give these results:

| Reader | Result of a refusal |
|---|---|
| the wire stack (STK-NET-001 and STK-NET-006) | a decode error |
| the wire stack with length prefix (STK-NET-007) | a decode error |
| the saved stack (STK-SAV-001, STK-SAV-008 and STK-SAV-010), in each format | an error and no stack |

The rule applies to each kind of nesting, also to a component type that a later game version adds.
The same limit applies to an item template that a reader reads alone, for example in a data pack
file. The map of that item template has the level 1, as the map of a saved stack. The reader must
stop at the first level above 512. It must not overflow its own stack, also for a chain that fills
a whole packet. The text of the error is not specified.

Reason: a reader must not overflow its stack. Each stack must also have a saved form that the NBT
reader can read again. Deviation D11 records the reference behaviour.

A bundle chain of `n` levels is a stack of `minecraft:bundle` with one item template in
`minecraft:bundle_contents`. The item templates of the levels 1 to `n − 1` are bundles of the same
form, each with the item template of the next level. The item template of the level `n` is
`{"id":"minecraft:stone"}`. A container chain of `n` levels has the same form, with
`minecraft:shulker_box` and one map with the slot 0 in `minecraft:container`. In a bundle chain, the
item template of the level `k` has the NBT level `1 + 3k`. In a container chain, it has the NBT
level `1 + 4k`. These stacks show the boundary:

| Stack | NBT depth | Result |
|---|---|---|
| a bundle chain of 170 levels | 511 | valid |
| a bundle chain of 170 levels, the level 170 with the patch `{"minecraft:max_stack_size":64}` | 512 | valid |
| a bundle chain of 170 levels, the level 170 with the patch `{"minecraft:lore":[]}` | 513 | refused |
| a bundle chain of 171 levels | 514 | refused |
| a container chain of 127 levels | 509 | valid |
| a container chain of 127 levels, the level 127 with the patch `{"minecraft:custom_data":{"a":{}}}` | 512 | valid |
| a container chain of 128 levels | 513 | refused |

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
cursor.

**STK-HSH-002** (`source_suggested`, wire). The value entries have no two entries with one type
id. The removal entries have no two entries with one type id. The order of the entries has no
meaning. The reader gives no decode error for a type id that occurs two times. For two value
entries with one type id, the reader keeps the hash of the last one. A type id that occurs two
times in the removal entries counts as one removal entry. A type id that names no component type
is a decode error. An item id of air is not a decode error.

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

A hashed stack that is present can also match the empty stack. The empty stack has the item air,
the count 0 and no patch entries (STK-EMP-002). Thus a present hashed stack with the item id of
air, the count 0 and no entries matches the empty stack in the reference. A writer must write the
empty stack as a hashed stack that is not present (STK-EMP-003).

`inventory-transactions.md` states what the server does when the stacks do not match.

**STK-HSH-004** (`source_suggested_black_box_confirmed`, client agreement). The hash of a
component is the hash of its saved form (STK-HSH-005). A reference to a registry entry in the
saved form is the identifier string, as the saved form of a world file has it. The writer of the
saved form must omit each optional field that the game omits, because an extra field changes the
hash.

The hash takes the saved form of the component itself. Two equal components can have different
saved forms, and thus different hashes. STK-SHP-027 gives an example. Deviation D7 states the
difference from the reference.

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
 The table at the end of this section gives the input of each case of the
fixture `hash_ops`.

**STK-HSH-006** (`source_suggested_black_box_confirmed`, client agreement). An entry of a map is
`u32le(hash of the key)` then `u32le(hash of the value)`. The key is a string node. Sort the
entries by the hash of the key as an unsigned 32-bit number. For equal key
hashes, sort by the hash of the value in the same way. The smallest number comes first.

**STK-HSH-007** (`source_suggested_black_box_confirmed`, client agreement). The node kind of a
number is the kind that the saved form of the component type states. A small value does not
change the kind. For example, `minecraft:max_stack_size` is an int node, and a boolean field is a
boolean node.

**STK-HSH-008** (`unresolved`, client agreement). The hash of a component whose type is not a
saved type is not decided. The reference server cannot compute that hash. A match against a stack
with such a value entry gives an internal error in the reference server. What the client sends is
not known. See open question Q4.

**STK-HSH-009** (`source_suggested`, client agreement). The float node and the double node of
STK-HSH-005 take the bit pattern of the value without a change. The hash does not make a NaN
canonical. Thus two NaN values with different bit patterns have different hashes, and `0.0` and
`-0.0` have different hashes. An implementation must keep the bit pattern of each float and each
double from the reader to the writer and to the hash. Two components that are equal by STK-EQ-005
can thus have different hashes. This occurs only for a NaN.

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
stack has no such effective component. Most prototypes have the value 64.

**STK-CMP-002** (`source_suggested_black_box_confirmed`, semantic). `minecraft:max_damage` holds
one integer. The maximum damage of a stack is the effective component, or 0 when the stack has no
such effective component.

**STK-CMP-003** (`source_suggested_black_box_confirmed`, semantic). `minecraft:damage` holds one
integer. `minecraft:unbreakable` holds no data. Only its presence has a meaning.

**STK-CMP-004** (`source_suggested_black_box_confirmed`, wire). The forms of the four component
types are:

| Component type | Type id | Saved form | Range of the saved form | Wire form |
|----------------|---------|------------|-------------------------|-----------|
| `minecraft:max_stack_size` | 1 | int | `1..99` | VarInt |
| `minecraft:max_damage` | 2 | int | 1 or more | VarInt |
| `minecraft:damage` | 3 | int | 0 or more | VarInt |
| `minecraft:unbreakable` | 4 | the empty map `{}` | none | no bytes |

A value out of the range is not a valid saved form (STK-SAV-005). The reader of the wire form does
not check the range.

**STK-CMP-005** (`design`, wire). A server must not send a component of STK-CMP-004 with a value
out of the range of its saved form. Reason: the client cannot save or hash that component.

### 7.2 Damage

**STK-DMG-001** (`source_suggested`, semantic). A stack can take damage when these three
conditions are true:

- it has an effective component `minecraft:max_damage`;
- it has an effective component `minecraft:damage`;
- it has no effective component `minecraft:unbreakable`.


**STK-DMG-002** (`source_suggested`, semantic). The damage value of a stack is the value of the
effective component `minecraft:damage`, or 0 when the stack has none. The result is limited to the
range from 0 to the maximum damage.

**STK-DMG-003** (`source_suggested`, semantic). A write of the damage value limits the new value
to the range from 0 to the maximum damage. Then it sets the component `minecraft:damage`
(STK-PAT-006).

**STK-DMG-004** (`source_suggested`, semantic). A stack is damaged when it can take damage and its
damage value is larger than 0. A stack is broken when it can take damage and its damage value is
equal to the maximum damage or larger.

**STK-DMG-005** (`source_suggested`, semantic). To apply an amount of damage `a` to a stack, do
these steps:

1. If the stack cannot take damage, stop. The stack does not change.
2. If `a` is 0, stop. The stack does not change.
3. Write the damage value plus `a` as the new damage value (STK-DMG-003).
4. If the stack is broken, the stack breaks (STK-DMG-006).

`a` can be negative. A negative amount decreases the damage value. The holder and the enchantments
of the stack can change `a` or cancel the damage before step 1. Another document states that.

**STK-DMG-006** (`source_suggested`, semantic). When a stack breaks, its count decreases by 1. The
operation reports that the stack broke, and reports the item. A stack with the count 1 thus
becomes the empty stack.

**STK-DMG-007** (`source_suggested`, semantic). A second operation applies damage and never
breaks the stack. It limits the new damage value to the range from 0 to the maximum damage minus
1. The count does not change.

**STK-DMG-008** (`source_suggested`, semantic). The next unit of damage breaks a stack when two
conditions are true. The stack can take damage. Its damage value is equal to the maximum damage
minus 1, or larger.

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

**STK-VAL-002** (`source_suggested`, semantic). Strict validation applies in these places:

| Place | Result of a rejection |
|-------|-----------------------|
| a stack from an item template (STK-TPL-006) | the empty stack |
| a stack from the command form (STK-CMD-006) | the command fails |
| a loot function that sets the components of a stack | the stack keeps the patch that it had before |

Strict validation does not apply to the reader of a saved stack (STK-SAV-007) or of a wire stack.

**STK-VAL-003** (`source_suggested`, semantic). No rule connects `minecraft:damage` and
`minecraft:max_damage` in the patch at decode. A stack can have `minecraft:damage` without
`minecraft:max_damage`. Such a stack cannot take damage, and its damage value is 0.

## 8. The component types

**STK-TYP-001** (`black_box_confirmed`, wire). This game version has 122 component types, with the
type ids `0..121`. The registry `minecraft:data_component_type` in
`assets/mcrs/reports/registries.json` is the source of the type ids. An implementation must read
the type ids from that file.

**STK-TYP-002** (`source_suggested`, semantic). Three component types are not saved types:
`minecraft:creative_slot_lock`, `minecraft:additional_trade_cost` and
`minecraft:map_post_processing`. Each other component type is a saved type.

**STK-TYP-003** (`source_suggested_black_box_confirmed`, wire). Each component type can occur in a
wire stack. The column "Wire" of the table states the kind of the wire form:

- `own`: the component type has a binary wire form of its own. The specification of its domain
  states that form.
- `NBT`: the wire form is the saved form of the component as one network NBT.


**STK-TYP-004** (`source_suggested_black_box_confirmed`, semantic). The column "Stacks" has `yes`
when a component of the type holds item templates. `minecraft:custom_data`,
`minecraft:entity_data`, `minecraft:bucket_entity_data` and `minecraft:block_entity_data` hold
free NBT. That NBT can hold saved stacks, but the component does not read them.

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
| 32 | `equippable` | yes | own | no | The slot, sound and model of the item as equipment (STK-SHP-028). |
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
STK-SHP-020, and each rule of sections 8.2 to 8.6 that names a component type, states it in a table. The
columns are:

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
key `rules`.

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
entry of the registry is an error in each format.

The reader of the saved form in NBT accepts a number of each numeric tag kind for an int, a float
or a double. It converts the number to the kind of the field:

| Field | Input | Result |
|-------|-------|--------|
| int | Byte tag or Short tag | the same value |
| int | Long tag | the low 32 bits of the value |
| int | Float tag or Double tag | the value without its fraction, toward zero; a value out of the int range gives the nearest limit; a NaN gives 0 |
| float | an integer tag | the nearest float |
| float | Double tag | the nearest float; a value out of the float range gives an infinity |
| double | each numeric tag | the nearest double |

The reader checks the range after the conversion. Thus `minecraft:max_stack_size` as `64b`, `64L`
or `64.5f` gives 64. The `count` of a saved stack as a Byte tag or a Short tag is valid. A String
tag is an error.

The reader of a boolean in NBT accepts each numeric tag. The value is false when the number is
equal to zero, and true for each other number. Thus `2b` and `0.5f` are true. A NaN is true. A
String tag is an error.

In JSON, an int, a float or a double needs a JSON number, and a boolean needs `false` or `true`.
A JSON boolean for a number is an error. A JSON number for a boolean is an error. A JSON number
with a fraction for an int loses its fraction as the table states.

The writer always writes the tag kind of the first table. The statements on a different tag kind
have the provenance `source_suggested`.

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

**STK-SHP-004** (`source_suggested_black_box_confirmed`, wire). A text is a text component of the
game. Its saved form is a string, a map or a list. `text-component.md` states
the text component, its canonical form, its equality and its hash. The rules of this section put no limit on the length of a
text. The writer writes a plain text without style as one string. The wire form is one network
NBT. Its reader gives a decode error for an NBT that is deeper than 512 levels. It also gives a
decode error for an NBT that is larger than a quota of 2097152 bytes. The exact measure of the
size is not specified. Two texts are equal when their content, their style and their children are
equal (STK-PAT-007).

**STK-SHP-005** (`design`, unspecified). A server must not send a component of sections 8.1 to
8.6 with a value out of the range of its saved form. Reason: the client cannot save or hash that component.
For an id that is not in a list of names, the reference reader of the wire form gives the name
with the id 0. A reader of this project can give a decode error instead. Reason: no vanilla writer
writes such an id.

**STK-SHP-006** (`source_suggested_black_box_confirmed`, wire). `minecraft:max_stack_size`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | int | `1..99` | necessary | VarInt |

Limits: none. Equality: the two values are equal. Prototypes: each prototype has this component.
The value is 64 for 1368 prototypes, 1 for 249 prototypes and 16 for 69 prototypes. See STK-CMP-001.

**STK-SHP-007** (`source_suggested_black_box_confirmed`, wire). `minecraft:max_damage`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | int | `1..2147483647` | necessary | VarInt |

Limits: none. Equality: the two values are equal. Prototypes: 84 prototypes have this component,
with 40 different values. The other 1602 prototypes do not have it. See STK-CMP-002.

**STK-SHP-008** (`source_suggested_black_box_confirmed`, wire). `minecraft:damage`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | int | `0..2147483647` | necessary | VarInt |

Limits: none. Equality: the two values are equal. Prototypes: the 84 prototypes that have
`minecraft:max_damage` have this component with the value 0 (STK-PRO-006). No other prototype has
it.

**STK-SHP-009** (`source_suggested_black_box_confirmed`, wire). `minecraft:unbreakable` has no
field. The saved form is the empty map `{}`. The wire form has no bytes. The reader of the saved
form accepts a map with keys and ignores each key. A value that is not a map is an error.
Equality: each two components of this type are equal. Prototypes: no prototype has this component.

**STK-SHP-010** (`source_suggested_black_box_confirmed`, wire). `minecraft:custom_name`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | text | each text | necessary | one network NBT |

Limits: those of STK-SHP-004 only. A menu that renames a stack has a limit of its own. A document
for menus states it. Equality: as STK-SHP-004. Prototypes: no prototype has this component.

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
this component with the empty list `[]`.

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
| each value | int | `1..255` | | VarInt, `1..255` |

`minecraft:enchantment` is a data-pack registry. The numeric id is the id that the server sent
for this connection (`registry.md`). The saved form does not accept an enchantment that is
written in place as a map. Limits: none on the number of entries. The reader of the wire form
gives a decode error for a level out of `1..255`. A writer must not write the level 0, because
that component has no saved form. A writer must not write one enchantment two times. The order of
the entries in the wire form is the choice of the writer.

The wire form of the map is a VarInt with the number of entries, then each entry. An entry is the
numeric id of the enchantment as a VarInt, then the level as a VarInt. The numeric id has no
offset of 1, and the wire form has no inline enchantment. This is different from a referenced
value (STK-SHP-033). For example, `{"minecraft:sharpness":5}` has the wire form `01 21 05`, because
`minecraft:sharpness` has the numeric id 33 in the vanilla registry. The level 255 is `FF01`. The
empty map is `00`. The saved form in NBT is a Compound tag with an Int tag for each level. The
hash node is a map node. Each key is a string node with the namespace, and each level is an int
node.

The reference reader of the wire form accepts the level 0. Deviation D5 records that behaviour.
The reference reader also accepts one enchantment two times, and the level of the last entry
wins. In the saved form, two keys that name one enchantment are an error. For example, a map with
`sharpness` and `minecraft:sharpness` is not a valid saved form. Experiment E30 confirms these two
statements with `minecraft:stored_enchantments`, which has the same readers (STK-SHP-060).
Equality: the two maps have the same enchantments with the same levels. The order has no effect.
Prototypes: each prototype has this component with the empty map `{}`.

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
display of the type `override`.

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
prototype has this component with the empty map `{}`.

**STK-SHP-018** (`source_suggested_black_box_confirmed`, wire). `minecraft:repair_cost`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | int | `0..2147483647` | necessary | VarInt |

Limits: none. Equality: the two values are equal. Prototypes: each prototype has this component
with the value 0.

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
The other 1648 prototypes do not have it. 29 of the 38 have only the key `rules`.

### 8.2 Shapes of the map and profile types

This section states the shape of 3 more component types. STK-SHP-001 to STK-SHP-005 apply to each
rule of this section. The fixture `item_records` has the cases `map_dec_*` and `profile_*`, which
confirm the forms of this section. The fixture `item_plain` has the cases of experiment E26.

**STK-SHP-021** (`source_suggested_black_box_confirmed`, wire). `minecraft:map_id`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | int | each int | necessary | VarInt |

The value is the number of a map. A negative number is valid: the fixture `item_plain` has the
case `-1`. Limits: none. Equality: the two values are equal. Prototypes: no prototype has this
component.

**STK-SHP-022** (`source_suggested_black_box_confirmed`, wire). `minecraft:map_decorations` is a
map from a key to a decoration entry:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | a map | no limit on the number of keys | necessary | one network NBT of the saved form |
| each key | string | each string, also the empty string | | a key of the NBT |
| each value | a decoration entry (below) | below | | a Compound tag of the NBT |

Each decoration entry is a map with these fields:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `type` | entry of the registry `minecraft:map_decoration_type` | each entry | necessary | String tag: the identifier |
| `x` | double | each double | necessary | Double tag |
| `z` | double | each double | necessary | Double tag |
| `rotation` | float | each float | necessary | Float tag |

`minecraft:map_decoration_type` is a static registry. The wire form is the saved form as one
network NBT (STK-TYP-003). Thus `type` is an identifier on the wire, not a numeric id. The reader of
the wire form gives a decode error when the NBT is not a valid saved form. The limits of the network
NBT of STK-SHP-004 apply.

The order of the keys of each compound in the NBT is unspecified (experiment E26). The vanilla
client reads the NBT as a map, and the hash sorts the keys (STK-HSH-006). Thus a parity test of
the wire form must compare the decoded trees, not the bytes.

The reader of the saved form ignores a key of a decoration entry that the table does not list
(STK-SHP-001). An entry that is not valid makes the whole value not valid. An identifier that
names no decoration type is an error. `x` and `z` are a position in the world, in blocks.
`rotation` is an angle in degrees. Another document states how a map shows the entries.

Limits: none on the number of entries. Equality: the two maps have the same keys, and the entries
of each key are equal. Two entries are equal when each field is equal. `x`, `z` and `rotation` are
equal as STK-EQ-005 states. Prototypes: 17 prototypes have this component with the empty map `{}`.
The other 1669 prototypes do not have it.

**STK-SHP-023** (`source_suggested_black_box_confirmed`, wire). `minecraft:profile` holds a player
profile and a skin patch. The player profile has one of two forms:

- A full profile has a UUID, a player name and a list of properties.
- A partial profile has an optional player name, an optional UUID and a list of properties.

The skin patch has four optional fields: `texture`, `cape`, `elytra` and `model`. The saved form
does not state the form of the profile. The reader of the saved form selects the form
(STK-SHP-024). The wire form states the form in its first byte (STK-SHP-026).

**STK-SHP-024** (`source_suggested_black_box_confirmed`, wire). The saved form of
`minecraft:profile` is a map, or one string. The map has these fields:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `id` | UUID (below) | each UUID | necessary in the full profile; no value in the partial profile | STK-SHP-026 |
| `name` | player name | each player name | necessary in the full profile; no value in the partial profile | STK-SHP-026 |
| `properties` | a list of properties (STK-SHP-025) | STK-SHP-025 | the empty list | STK-SHP-026 |
| `texture` | identifier | each identifier | no value | STK-SHP-026 |
| `cape` | identifier | each identifier | no value | STK-SHP-026 |
| `elytra` | identifier | each identifier | no value | STK-SHP-026 |
| `model` | a name: `wide` or `slim` | `wide`, `slim` | no value | STK-SHP-026 |

The two forms that the table uses are:

| Form | Saved form in NBT | Saved form in JSON | Hash node |
|------|-------------------|--------------------|-----------|
| UUID | Int Array tag with 4 elements | a list of 4 numbers | int array |
| player name | String tag | string | string |

The 4 ints of a UUID are the 128 bits of the UUID, most significant int first. The reader of the
saved form in NBT also accepts a List tag, a Byte Array tag or a Long Array tag of 4 numbers. It
converts each number as STK-SHP-002 states for an int. A list with more or fewer than 4 elements
is an error. A string, for example a UUID with hyphens, is an error.

A player name has 0 to 16 UTF-16 code units. Each code unit is in the range `U+0021..U+007E`.
Thus the empty string is valid, and a space or a letter outside ASCII is an error. No reader
changes the case of a player name.

The identifiers of `texture`, `cape` and `elytra` name textures of a resource pack. No registry
holds them, and no reader checks that the texture exists. The name of `model` is compared
exactly. Each other string of `model` is an error.

The reader of the saved form gives the component in this way:

1. When the value is a string, the string must be a player name. The result is a partial profile
   with that name and with no other field.
2. When the value is a map, each field of the table that is present must be valid. Else the
   value is an error.
3. When the map has both `id` and `name`, the result is a full profile. Else the result is a
   partial profile.
4. A value that is not a string or a map is an error.

The reader ignores a key that the table does not list (STK-SHP-001). The writer always writes the
map. It writes `id` and `name` when the profile has them. It omits `properties` when the list is
empty. It writes each field of the skin patch that has a value. A partial profile with both a name
and a UUID has the same saved form as a full profile with the same values. The reader of that saved
form gives the full profile.

Limits: those of the table and of STK-SHP-025. Prototypes: no prototype has this component.
Experiment E26 confirms these statements:

- the string form and the empty name;
- a UUID of numbers that are not ints;
- a full profile without properties;
- the same saved form for a partial profile with a name and a UUID.

**STK-SHP-025** (`source_suggested_black_box_confirmed`, wire). A property has these fields:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `name` | string | 0 to 64 UTF-16 code units | necessary | String (64) |
| `value` | string | 0 to 32767 UTF-16 code units | necessary | String (32767) |
| `signature` | string | 0 to 1024 UTF-16 code units | no value | Boolean, then String (1024) when the Boolean is `01` |

The saved form of the list of properties has two forms:

- The list form is a list of maps with the fields of the table. The list has at most 16 elements.
- The map form is a map from a property name to a list of strings. Each string gives a property
  with that name, that value and no signature. The map has at most 16 keys.

The reader takes the map form when the value is a map, and the list form when the value is a list.
Each other value is an error. The writer always writes the list form. It omits `signature` when
the property has no signature.

A profile always holds its properties in property groups. The groups are in the order of the
first property of each name. In a group, the properties keep the order of the input. Each reader
puts the properties in this order. A profile from a lookup and a profile that a user of the library
makes also have this order. Thus the input `x=1, textures=t, x=2` gives the list
`x=1, x=2, textures=t`. The order of the groups from the map form is unspecified. A parity test
must not compare it.

The writer of each format and the hash take the properties in the order of the profile. Thus the
order of the groups has an effect on the saved form, on the wire form and on the hash. Two equal
profiles (STK-SHP-027) can have different saved forms and different hashes. The saved form and the
hash of a profile come only from that profile. This is deviation D7.

The reader of the map form must apply the limits of the list form to the result. The result has at
most 16 properties in total, and each name and each value is in the range of the table. This is
deviation D6. Experiment E26 confirms the statements on the order, on the map form and on the
hash.

**STK-SHP-026** (`source_suggested_black_box_confirmed`, wire). The wire form of
`minecraft:profile` is its own binary form. It has these parts, in this order:

| Part | Wire form | Condition |
|------|-----------|-----------|
| form | Boolean: `01` for a full profile, `00` for a partial profile | always |
| UUID | UUID | full profile |
| name | String (16) | full profile |
| name present | Boolean | partial profile |
| name | String (16) | partial profile, name present is `01` |
| UUID present | Boolean | partial profile |
| UUID | UUID | partial profile, UUID present is `01` |
| number of properties, `P` | VarInt, at most 16 | always |
| properties | `P` times: `name`, `value` and `signature` as STK-SHP-025 states | always |
| texture present | Boolean | always |
| texture | Identifier | texture present is `01` |
| cape present | Boolean | always |
| cape | Identifier | cape present is `01` |
| elytra present | Boolean | always |
| elytra | Identifier | elytra present is `01` |
| model present | Boolean | always |
| model | Boolean: `01` for `slim`, `00` for `wide` | model present is `01` |

Boolean, String (n), UUID and Identifier have the forms of `login.md`, section 0.2. Let the saved
UUID have the ints `a`, `b`, `c` and `d` (STK-SHP-024). Its wire form is `a`, `b`, `c` and `d` in
that order, each as 4 bytes, big-endian. A number of properties larger than 16 is a decode error. A
string longer than its limit is a decode error. The reader of the wire form does not check the code
units of a name. A name that is not a player name has no valid saved form, so STK-NET-009 applies.

For example, the partial profile with only the UUID of the ints `1, 2, 3, 4` has the wire form
`00 00 01 00000001 00000002 00000003 00000004 00 00 00 00 00`.

**STK-SHP-027** (`source_suggested_black_box_confirmed`, semantic). Two components of
`minecraft:profile` are equal when each of these conditions is true:

- They have the same form: full profile or partial profile.
- They have the same name, or no name. The comparison does not ignore case. The inherited
  requirement "Name case" of `README.md` applies to a lookup of a player, not to the equality of
  this component. Equality decides if two stacks can merge (STK-EQ-004).
- They have the same UUID, or no UUID.
- Their property groups have the same names. For each name, the two groups have equal properties
  in the same order. Two properties are equal when their values are equal and their signatures
  are equal or both absent.
- Each field of the skin patch is equal, or absent in both.

The order of the groups has no effect on equality. The form has an effect on equality, also when
the saved forms are equal. These two statements are an exception to STK-PAT-007.

Thus the profiles with the properties `x=1, textures=t` and `textures=t, x=1` are equal. Their
saved forms and their hashes are different (STK-SHP-025). Two stacks with these two profiles can
merge. A hashed stack with the hash of one profile does not match a stack with the other profile
(STK-HSH-003). Experiment E26 confirms the order of the groups, the order in a group, the case of
the name, the signature, the skin patch and the form.

A dynamic profile is a partial profile with no property and with exactly one of a name and a UUID.
No format records that a profile is dynamic. The game can replace a dynamic profile with the
result of a lookup by the name or the UUID. Another document states when the game does that lookup.
The vanilla client shows the tooltip line with the translation key `component.profile.dynamic`, in
gray, for a dynamic profile.

### 8.3 Shape of the equipment type

This section states the shape of one more component type. STK-SHP-001 to STK-SHP-005 apply to each
rule of this section. The fixture `item_registry_refs` has 5 cases of `minecraft:equippable`. Each
case has the JSON, the NBT, the wire form and the hash that the game wrote. The fixture `item_plain`
has the 4 cases of `minecraft:equippable` of experiment E27.

**STK-SHP-028** (`source_suggested_black_box_confirmed`, wire). `minecraft:equippable`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `slot` | name of the list "equipment slot" | `mainhand` 0, `feet` 1, `legs` 2, `chest` 3, `head` 4, `offhand` 5, `body` 6, `saddle` 7 | necessary | VarInt: the id |
| `equip_sound` | a sound event (STK-SHP-029) | STK-SHP-029 | no value | one boolean byte, then the sound event when the byte is `01` |
| `asset_id` | identifier | each identifier | no value | one boolean byte, then String (32767) when the byte is `01` |
| `camera_overlay` | identifier | each identifier | no value | one boolean byte, then String (32767) when the byte is `01` |
| `allowed_entities` | an entity type set (STK-SHP-030) | STK-SHP-030 | no value | one boolean byte, then the entity type set when the byte is `01` |
| `dispensable` | boolean | `false`, `true` | `true` | one byte |
| `swappable` | boolean | `false`, `true` | `true` | one byte |
| `damage_on_hurt` | boolean | `false`, `true` | `true` | one byte |
| `equip_on_interact` | boolean | `false`, `true` | `false` | one byte |
| `can_be_sheared` | boolean | `false`, `true` | `false` | one byte |
| `shearing_sound` | a sound event (STK-SHP-029) | STK-SHP-029 | no value | one boolean byte, then the sound event when the byte is `01` |

The list "equipment slot" is not the list "slot group" of STK-SHP-003. Its ids are also not the
slot ids of the packet `set_equipment` (`client-entities.md`, CENTS-EQUIP-001). For example,
`offhand` has the id 5 in this component and the id 1 in that packet.

A field with "no value" has no default. When `equip_sound` or `shearing_sound` has no value, the
item has no sound for that action. The absence does not select a sound of the game. Each
prototype has both sounds in its file, and an implementation must read them from the file.

`asset_id` names an equipment asset of a resource pack. `camera_overlay` names a texture of a
resource pack. No registry holds them, and no reader checks that they exist.

The fields have the purposes below. Other documents state the behaviour.

- `slot`: the equipment slot that the item goes into.
- `equip_sound`: the sound when an entity equips the item.
- `asset_id`: the model of the item on the wearer (`client-entities.md`, CENTS-EQUIP-001).
- `camera_overlay`: the texture over the view of a player who wears the item.
- `allowed_entities`: the entity types that can equip the item (STK-SHP-031).
- `dispensable`: a dispenser can put the item on an entity.
- `swappable`: a use of the item swaps it with the stack of its equipment slot.
- `damage_on_hurt`: the item takes damage when the wearer gets hurt.
- `equip_on_interact`: a player can put the item on another entity with an interaction.
- `can_be_sheared`: shears can remove the item from the entity.
- `shearing_sound`: the sound when shears remove the item.

For example, the component `{"slot":"head"}` has the wire form `04 00 00 00 00 01 01 01 00 00 00`.
Its saved form has only the key `slot`, because each other field has its default or no value.

The hash takes the saved form that the writer writes (STK-HSH-004). The writer omits each field
with no value. It omits each boolean field whose value is equal to its default. Each boolean is a
boolean node. A sound event is a string node or a map node (STK-SHP-029). An entity type set is a
string node or a list node (STK-SHP-030).

Limits: none on the number of entity types. Each wire string holds at most 32767 UTF-16 code
units. A longer string is a decode error. Equality: each field is equal. Two fields with no value
are equal. A field with no value is not equal to a field with a value. STK-SHP-029 states the
equality of two sound events, and STK-SHP-030 states the equality of two entity type sets.

Prototypes: 84 prototypes have this component. The other 1602 prototypes do not have it. The
`slot` is `body` for 44 prototypes, `head` for 16, `chest` for 8, `feet` for 7, `legs` for 7,
`saddle` for 1 and `offhand` for 1. The item with the slot `offhand` is `minecraft:shield`. Each of
the 84 prototypes has `equip_sound` and `shearing_sound` as a reference to an entry. 75 have
`asset_id`, 45 have `allowed_entities` and 1 has `camera_overlay`. No prototype has
`dispensable`. 9 have `swappable` with `false`, 12 have `damage_on_hurt` with `false`, 22 have
`equip_on_interact` with `true` and 45 have `can_be_sheared` with `true`.

**STK-SHP-029** (`source_suggested_black_box_confirmed`, wire). A sound event is a reference to an
entry of the registry `minecraft:sound_event`, or an inline sound event. An inline sound event has
these fields:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `sound_id` | identifier | each identifier | necessary | String (32767) |
| `range` | float | each float | no value | one boolean byte, then 4 bytes when the byte is `01` |

The two kinds of a sound event have these forms:

| Sound event | Saved form | Wire form |
|-------------|------------|-----------|
| a reference | the identifier of the entry, as a string | VarInt: the numeric id of the entry plus 1 |
| an inline sound event | a map with the fields above | VarInt 0, then the fields above |

`minecraft:sound_event` is a static registry. `assets/mcrs/reports/registries.json` gives the
numeric ids. `sound_id` names a sound of a resource pack. No reader checks that the sound exists.

The readers apply these statements:

- The reader of the saved form reads a string as a reference. A string that names no entry is an
  error. The reader does not read such a string as an inline sound event. A string that is not an
  identifier is an error.
- The reader of the saved form reads a map as an inline sound event.
- The reader of the saved form reads a `range` that is not a valid float (STK-SHP-002) as no
  value. The inline sound event stays valid. For example, a string, a list and a map give no
  value. The string `"2.5"` also gives no value. This is an exception to STK-SHP-001. Experiment
  E27 confirms this statement.
- The reader of the wire form reads the VarInt 0 as an inline sound event. It reads each other
  number `n` as the entry with the numeric id `n - 1`. A number that names no entry is a decode
  error.

The writer writes a reference as the identifier of the entry, and an inline sound event as the
map. It omits `range` when `range` has no value.

For example, the reference `minecraft:item.armor.equip_iron` has the numeric id 52 and the wire
form `35`. The inline sound event `{"sound_id":"mcrs:snip","range":3.5}` has the wire form
`00 09 6D6372733A736E6970 01 40600000`.

Equality: two references are equal when they name the same entry. Two inline sound events are
equal when their `sound_id` values are equal and their `range` values are equal or absent in both.
`range` is equal as STK-EQ-005 states. A reference is not equal to an inline sound event, also when
the identifier of the entry is equal to `sound_id`.

**STK-SHP-030** (`source_suggested_black_box_confirmed`, wire). An entity type set is a tag or a
list of entries of the registry `minecraft:entity_type`. It has the form of the block set of
STK-SHP-020:

| Entity type set | Saved form | Wire form |
|-----------------|------------|-----------|
| a tag | the string `#<identifier of the tag>` | VarInt 0, then the identifier of the tag as String (32767) |
| a list of `n` entity types | a list of identifiers; one string when `n` is 1 | VarInt `n + 1`, then `n` numeric ids as VarInt |

`minecraft:entity_type` is a static registry. The tag is a tag of that registry. The reader of the
saved form accepts a list of one identifier and one string for one entity type. The writer writes
one string for one entity type, and a list for each other number. Thus the empty list is `[]`. A
tag that does not exist is an error in each format. An identifier or a numeric id that names no
entity type is an error in each format. A list keeps the order of its elements, and an element can
occur two times.

For example, the list of `minecraft:zombie` (id 156) and `minecraft:pig` (id 103) has the wire
form `03 9C01 67`. The tag `minecraft:skeletons` has the wire form `00 13`, then the 19 bytes of
the string `minecraft:skeletons`.

No vanilla writer writes a first VarInt below 0. The reference reader reads such a VarInt as the
empty list. A reader of this project can give a decode error instead. The statement on the
reference reader has the provenance `source_suggested`.

Limits: none on the number of entity types. Equality: two tags are equal when they are the same
tag. Two lists are equal when they have the same entity types in the same order. A tag is not
equal to a list, also when the tag has the same members.

**STK-SHP-031** (`source_suggested`, semantic). An entity type set contains an entity type when one
of these conditions is true:

- The set is a list, and an element of the list is that entity type.
- The set is a tag, and the entity type is a member of that tag. The members are the resolved
  members of the tag (`registry.md`, REG-TAG-005) at the time of the test.

The empty list contains no entity type. A stack keeps its tag after a reload, and the test then
uses the new members of the tag (`registry.md`, REG-RELOAD-003). Thus a data pack can change the
result of the test. The vanilla client makes the same test with the tags of the last
`update_tags` packet.

When a component of `minecraft:equippable` has no `allowed_entities`, each entity type can equip
the item. The empty list is different: no entity type can equip the item.

### 8.4 Shapes of the colour, effect, trim, potion, instrument and data types

This section states the shape of 11 more component types. STK-SHP-001 to STK-SHP-005 apply to each
rule of this section. STK-SHP-032 to STK-SHP-034 state the forms that more than one rule uses. The
fixture `item_holders` has the cases `trim_*` and `instrument_*`. The fixture `item_records` has the
cases `cmd_*`, `block_state_*` and `lodestone_*`. The fixture `item_registry_refs` has samples of
`minecraft:potion_contents` and `minecraft:suspicious_stew_effects`. The fixture `item_plain` has
the cases of experiment E28. Each case has the JSON, the NBT, the wire form and the hash that the
game wrote.

**STK-SHP-032** (`source_suggested_black_box_confirmed`, wire). The rules of this section use these
forms in addition to the forms of STK-SHP-002:

| Form | Saved form in NBT | Saved form in JSON | Hash node | Wire form |
|------|-------------------|--------------------|-----------|-----------|
| string | String tag | string | string | String (32767) of `login.md`, section 0.2 |
| int of 4 bytes | as int | as int | int | 4 bytes, two's complement, big-endian |
| colour | Int tag | number | int | 4 bytes, two's complement, big-endian |
| list of X | List tag of the NBT of X | array | list | VarInt: the number of elements, then each element in the wire form of X |

A string has no limit in the saved form. A server must not send a string that is longer than
32767 UTF-16 code units (STK-SHP-005). An int of 4 bytes and a colour have the range of an int. In
JSON, an integer number out of the int range gives the low 32 bits of the number. Thus `2147483648`
gives `-2147483648`. A number with a fraction loses its fraction, as STK-SHP-002 states.

A colour is an int with 4 channels of 8 bits. Alpha is in the bits 24 to 31, and red is in the
bits 16 to 23. Green is in the bits 8 to 15, and blue is in the bits 0 to 7. The reader of the saved form also
accepts a list of exactly 3 numbers: red, green and blue. It reads each number as a float
(STK-SHP-002). It then makes the colour as follows:

1. Multiply each float by 255 in float arithmetic.
2. Round each product down to an int. A product out of the int range gives the nearest limit. A
   NaN gives 0.
3. Keep the low 8 bits of each int as the channel.
4. Set the alpha channel to 255.

A list with more or fewer than 3 elements is an error. A map is an error. The writer always writes
the int. For example, `[1.0,0.5,0.0]` gives `-33024` (`FFFF7F00`), and `[2.0,-0.1,0.5]` gives
`-72065` (`FFFEE67F`). Equality: two colours are equal when the two ints are equal.

**STK-SHP-033** (`source_suggested_black_box_confirmed`, wire). A referenced value of a registry is
a reference to an entry of that registry, or an inline value. The rule of a component states the
registry and the fields of the inline value. The two kinds have these forms:

| Kind | Saved form | Hash node | Wire form |
|------|------------|-----------|-----------|
| a reference | the identifier of the entry, as a string | string | VarInt: the numeric id of the entry plus 1 |
| an inline value | a map with the fields of the inline value | map | VarInt 0, then the fields of the inline value |

The numeric id of an entry of a data-pack registry is the id that the server sent for this
connection (`registry.md`). The readers, the writer and the equality of a referenced value follow the
statements of STK-SHP-029 for a sound event. A sound event is the referenced value of the registry
`minecraft:sound_event`. For example, the reader of the saved form reads a string as a reference.
A string that names no entry is an error.

**STK-SHP-034** (`source_suggested_black_box_confirmed`, wire). An effect instance is a status
effect with its details. `status-effects.md` defines the effect kinds, the amplifier and the
duration. An effect instance has these fields:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `id` | entry of the registry `minecraft:mob_effect` | each entry | necessary | VarInt: the numeric id |
| `amplifier` | unsigned byte (below) | each int | 0 | VarInt |
| `duration` | int | each int | 0 | VarInt |
| `ambient` | boolean | `false`, `true` | `false` | one byte |
| `show_particles` | boolean | `false`, `true` | `true` | one byte |
| `show_icon` | boolean | `false`, `true` | the value of `show_particles` | one byte |
| `hidden_effect` | a hidden effect (below) | below | no value | one boolean byte, then the hidden effect when the byte is `01` |

`minecraft:mob_effect` is a static registry. `assets/mcrs/reports/registries.json` gives the
numeric ids. The duration `-1` is an infinite duration.

The unsigned byte has these forms:

- The reader of the saved form reads an int (STK-SHP-002). It keeps the low 8 bits as a number in
  `0..255`. Thus `256` gives 0, `300` gives 44 and `-1` gives 255. No int is an error.
- The writer of NBT writes a Byte tag with the same 8 bits. The writer of JSON writes the signed
  value of that byte. Thus 255 is `-1b` in NBT and `-1` in JSON.
- The hash node is a byte node with the same 8 bits.
- The writer of the wire form writes the number in `0..255`. The reference reader of the wire form
  changes a number out of `0..255` to the nearest limit. This statement has the provenance
  `source_suggested`.

The writer of the saved form always writes `show_icon`. This is an exception to STK-SHP-001. It
omits each other field whose value is equal to its default.

A hidden effect has the fields of the table without `id`. Its effect kind is the `id` of the
effect instance that holds it. A hidden effect can hold a hidden effect. The rules put no limit on
the depth.

Equality: two effect instances are equal when each field is equal. The reference does not compare
`hidden_effect`. Deviation D8 records that behaviour.

**STK-SHP-035** (`source_suggested_black_box_confirmed`, wire). `minecraft:dyed_color`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | colour (STK-SHP-032) | each int | necessary | 4 bytes |

For example, 16711680 has the wire form `00FF0000`. Limits: none. Equality: as STK-SHP-032.
Prototypes: no prototype has this component.

**STK-SHP-036** (`source_suggested_black_box_confirmed`, wire). `minecraft:custom_model_data`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `floats` | list of float | each float | the empty list | list of 4 bytes |
| `flags` | list of boolean | `false`, `true` | the empty list | list of one byte |
| `strings` | list of string | each string | the empty list | list of String (32767) |
| `colors` | list of colour (STK-SHP-032) | each int | the empty list | list of 4 bytes |

Each element of `colors` can be an int or a list of 3 numbers in the saved form. The writer
writes each colour as an int. Limits: none on the number of elements. Equality: the four lists are
equal, element by element, in order. Floats are equal as STK-EQ-005 states. Prototypes: no
prototype has this component.

**STK-SHP-037** (`source_suggested_black_box_confirmed`, wire). `minecraft:enchantment_glint_override`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | boolean | `false`, `true` | necessary | one byte |

Limits: none. Equality: the two values are equal. Prototypes: 7 prototypes have this component,
each with `true`. The other 1679 prototypes do not have it.

**STK-SHP-038** (`source_suggested_black_box_confirmed`, wire). `minecraft:block_state` is a map
from the name of a block state property to a value:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | a map | no limit on the number of keys | necessary | VarInt: the number of entries, then the entries |
| each key | string | each string | | String (32767) |
| each value | string | each string | | String (32767) |

No reader checks that a block has the property or the value. Thus `{"no_such_property":"any
text"}` is valid. A value must be a string in the saved form. Thus `{"level":1}` and
`{"level":true}` are errors. The order of the entries in the wire form is the choice of the
writer. Thus a parity test of the wire form must compare the entries as a set. The reference
reader of the wire form accepts one key two times, and the value of the last entry wins. This
statement has the provenance `source_suggested`.

Limits: none on the number of entries. Equality: the two maps have the same keys, and each key
has the same value. The order has no effect. Prototypes: 12 prototypes have this component, each
with one key. The map `{"copper_golem_pose":"standing"}` occurs 8 times, `{"honey_level":"0"}` 2
times, `{"level":"15"}` 1 time and `{"mode":"start"}` 1 time.

**STK-SHP-039** (`source_suggested_black_box_confirmed`, wire). `minecraft:lodestone_tracker`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `target` | a global position (below) | below | no value | one boolean byte, then the global position when the byte is `01` |
| `tracked` | boolean | `false`, `true` | `true` | one byte |

A global position has these fields:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `dimension` | identifier | each identifier | necessary | String (32767) |
| `pos` | block position (below) | 3 ints | necessary | a long of 8 bytes, big-endian (below) |

No reader checks that the dimension exists. A block position is 3 ints `x`, `y` and `z`, in this
order. Its saved form in NBT is an Int Array tag with 3 elements. In JSON, it is a list of 3
numbers. Its hash node is an int array. The reader of the saved form in NBT also accepts a List
tag, a Byte Array tag or a Long Array tag of 3 numbers. STK-SHP-024 states the same for a UUID. A list
with more or fewer than 3 elements is an error.

The wire form of a block position is one long, as `client-block-breaking.md`, CBK-PKT-001, states:
`x` in the bits 38 to 63, `z` in the bits 12 to 37 and `y` in the bits 0 to 11. The reference
writer keeps only the low bits of each coordinate. Thus `y` = 4096 gives 0 on the wire, and the
client gets another position. A server must not send a block position with `x` or `z` out of
`-33554432..33554431`, or `y` out of `-2048..2047` (STK-SHP-005).

For example, the position `[1,-2,3]` has the wire form `0000004000003FFE`. The value
`{"tracked":false}` has the wire form `00 00`. Limits: none. Equality: each field is equal. Two
targets with no value are equal. Prototypes: no prototype has this component.

**STK-SHP-040** (`source_suggested_black_box_confirmed`, wire). `minecraft:trim`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `material` | referenced value of the registry `minecraft:trim_material` (STK-SHP-033) | STK-SHP-033 | necessary | STK-SHP-033 |
| `pattern` | referenced value of the registry `minecraft:trim_pattern` (STK-SHP-033) | STK-SHP-033 | necessary | STK-SHP-033 |

`minecraft:trim_material` and `minecraft:trim_pattern` are data-pack registries. An inline trim
material has these fields:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `palette_id` | identifier | each identifier | necessary | String (32767) |
| `description` | text (STK-SHP-004) | each text | necessary | one network NBT |

An inline trim pattern has these fields:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `asset_id` | identifier | each identifier | necessary | String (32767) |
| `description` | text (STK-SHP-004) | each text | necessary | one network NBT |
| `decal` | boolean | `false`, `true` | `false` | one byte |

The writer of the saved form always writes `decal`, also when its value is `false`. This is an
exception to STK-SHP-001. `palette_id` and `asset_id` name assets of a resource pack. No reader
checks that the asset exists.

For example, the trim with the reference `minecraft:amethyst` (id 0) and the reference
`minecraft:coast` (id 1) has the wire form `01 02`. The inline material
`{"palette_id":"mcrs:pal","description":"Pal"}` has the wire form
`00 08 6D6372733A70616C 08 0003 50616C`. Limits: those of STK-SHP-004 only. Equality: each field is
equal (STK-SHP-033). Prototypes: no prototype has this component.

**STK-SHP-041** (`source_suggested_black_box_confirmed`, wire). `minecraft:potion_contents`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `potion` | entry of the registry `minecraft:potion` | each entry | no value | one boolean byte, then VarInt: the numeric id when the byte is `01` |
| `custom_color` | int of 4 bytes (STK-SHP-032) | each int | no value | one boolean byte, then 4 bytes when the byte is `01` |
| `custom_effects` | list of effect instance (STK-SHP-034) | STK-SHP-034 and STK-SHP-042 | the empty list | list of effect instance |
| `custom_name` | string | each string | no value | one boolean byte, then String (32767) when the byte is `01` |

`minecraft:potion` is a static registry. `assets/mcrs/reports/registries.json` gives the numeric
ids. The wire form of `potion` is the numeric id, not the numeric id plus 1. `custom_color` is not
a colour: a list of 3 numbers is an error.

The reader of the saved form also accepts a string. It reads the string as the value with that
`potion` and with each other field absent. The writer always writes the map. For example,
`"minecraft:water"` gives `{"potion":"minecraft:water"}` and the wire form `01 00 00 00 00`.

Limits: none on the number of effects. Equality: each field is equal. The order of
`custom_effects` has an effect. Prototypes: 4 prototypes have this component with the empty map
`{}`. The other 1682 prototypes do not have it.

**STK-SHP-042** (`black_box_confirmed`, wire). The reader of the saved form of
`minecraft:potion_contents` ignores `hidden_effect` in each element of `custom_effects`. Thus the
saved form, the hash and the wire form of the value have no hidden effect. The reader of an effect
instance in `minecraft:consumable` keeps `hidden_effect`. Experiment E28 shows the two results for
one effect instance.

A writer of the wire form of `minecraft:potion_contents` must write `00` for `hidden_effect`. The
reader of the wire form reads a hidden effect, and this project then drops it. Reason: a value must
have the same saved form after a read from each format. The provenance of this paragraph is
`design`. What the vanilla client does with a hidden effect on the wire is not known (U4).

**STK-SHP-043** (`source_suggested_black_box_confirmed`, wire). `minecraft:custom_data` is free
NBT. The game gives it no meaning:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | an NBT compound | each compound | necessary | one network NBT of the compound |

The value is a tree of NBT tags. A list of the tree can hold elements of different tag kinds
(`snbt.md`, SNBT-GR-003). The binary NBT of such a list follows `text-component.md`, TXT-NBT-002.
The saved form in NBT is the Compound tag. The reader of the saved form also accepts a string. It
reads the string as SNBT (`snbt.md`, SNBT-ARG-004). The result must be a compound. Thus `"{}"` gives
the empty compound, and `"[1,2]"` and `"{a:"` are errors. A number and a list are errors. The
writer always writes the compound, never the string.

The saved form in JSON has no tag kinds. The writer of JSON writes each number tag as a number and a
String tag as a string. It writes an array tag or a List tag as an array. The reader of JSON makes these
tags:

| JSON value | Tag |
|------------|-----|
| an integer number in the range of a long | the first of Byte, Short, Int and Long that holds the value |
| another number | Float, when a float holds the value exactly; else Double |
| `true`, `false` | Byte tag 1 or 0 |
| a string | String tag |
| an array | List tag |
| an object | Compound tag |

A number with a zero fraction, for example `1.0`, is an integer number. Thus a JSON round trip can
change the tag kinds. For example, `{"n":true}` gives `{n:1b}`. The statements on JSON have the
provenance `source_suggested`, except the cases of experiment E28.

The hash node of each tag kind is the node of STK-HSH-005 with the same name. A Compound tag is a
map node, and a List tag is a list node. A list node holds the elements without the wrapper of
TXT-NBT-002. For example, `{mixed:[1b,"a",{}]}` has the hash 767856123. The wire form is the
network NBT of the compound, with the limits of STK-SHP-004.

Equality: two values are equal when their trees are equal, with the tag kinds. Thus `1b` and `1`
are not equal. Float tags and Double tags are equal as STK-EQ-005 states. Prototypes: no prototype
has this component.

**STK-SHP-044** (`source_suggested_black_box_confirmed`, wire). `minecraft:suspicious_stew_effects`
is a list of entries:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | list of entry (below) | each list | necessary | list of entry |

An entry has these fields:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `id` | entry of the registry `minecraft:mob_effect` | each entry | necessary | VarInt: the numeric id |
| `duration` | int | each int | 160 | VarInt |

The reader of the saved form reads a `duration` that is not a valid int as 160. The entry stays
valid. For example, a string, `true` and `null` give 160. This is an exception to STK-SHP-001. A
negative duration and the duration 0 are valid, and the writer writes them. The writer omits the
duration 160.

For example, the list of `speed` with the duration 160 and `slowness` with the duration 200 has the
wire form `02 00 A001 01 C801`. Limits: none on the number of entries. Equality: the two lists have
equal entries in the same order. Prototypes: 1 prototype has this component with the empty list
`[]`.

**STK-SHP-045** (`source_suggested_black_box_confirmed`, wire). `minecraft:ominous_bottle_amplifier`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | int | `0..4` | necessary | VarInt |

Limits: none. Equality: the two values are equal. Prototypes: 1 prototype has this component with
the value 0.

**STK-SHP-046** (`source_suggested_black_box_confirmed`, wire). `minecraft:instrument`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | referenced value of the registry `minecraft:instrument` (STK-SHP-033) | STK-SHP-033 | necessary | STK-SHP-033 |

`minecraft:instrument` is a data-pack registry. An inline instrument has these fields:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `sound_event` | a sound event (STK-SHP-029) | STK-SHP-029 | necessary | STK-SHP-029 |
| `use_duration` | float | `+0.0` to `3.4028235E38` | necessary | 4 bytes |
| `range` | float | larger than `+0.0`, up to `3.4028235E38` | necessary | 4 bytes |
| `durability_damage` | int | `0..2147483647` | 0 | VarInt |
| `description` | text (STK-SHP-004) | each text | necessary | one network NBT |

The ranges of `use_duration` and `range` use a total order of floats. In that order, `-0.0` is
smaller than `+0.0`, and a NaN is larger than each other float. Thus `-0.0` is an error for
`use_duration`, and `+0.0` is an error for `range`. A NaN and an infinity are errors. The smallest
positive float is valid for `range`. `use_duration` is in seconds, and `range` is in blocks.
Another document states how a holder uses an instrument.

For example, the reference `minecraft:ponder_goat_horn` (id 4) has the wire form `05`. The inline
instrument `{"sound_event":"minecraft:entity.item.break","use_duration":7.0,"range":256.0,
"description":"Horn"}` has the wire form `00 8E05 40E00000 43800000 00 08 0004 486F726E`. Limits:
those of STK-SHP-004 only. Equality: as STK-SHP-033. Floats are equal as STK-EQ-005 states.
Prototypes: 1 prototype has this component with the reference `minecraft:ponder_goat_horn`.

### 8.5 Shapes of the container, bee, firework and banner types

This section states the shape of 8 more component types. STK-SHP-001 to STK-SHP-005 apply to each
rule of this section. STK-SHP-032 to STK-SHP-034 and STK-SHP-047 to STK-SHP-050 state the forms
that more than one rule uses. The fixture `item_nested` has the cases `charged_*`, `bundle_*` and
`container_*`. The fixture `item_records` has the cases `explosion_*`, `fireworks_*` and `loot_*`.
The fixture `item_holders` has the cases `banner_patterns_*`. The fixture `item_registry_refs` has
samples of `minecraft:bees`. The fixture `item_plain` has the cases of experiment E29. Each case
has the JSON, the NBT, the wire form and the hash that the game wrote.

**STK-SHP-047** (`source_suggested_black_box_confirmed`, wire). The rules of this section use this
form in addition to the forms of STK-SHP-002 and STK-SHP-032:

| Form | Saved form in NBT | Saved form in JSON | Hash node | Wire form |
|------|-------------------|--------------------|-----------|-----------|
| long | Long tag | number | long | a Long tag in the network NBT of the component (STK-SHP-055) |

A long is a signed 64-bit integer. The reader of the saved form converts a number to a long as
STK-SHP-002 states for an int, with 64 bits in place of 32 bits. In JSON, an integer from 2^63 to
2^64 − 1 gives the low 64 bits of the number. Thus `18446744073709551615` gives -1. An integer
larger than 2^64 − 1 gives 9223372036854775807. An integer smaller than -2^63 gives
-9223372036854775808. Thus `18446744073709551617` gives 9223372036854775807. Deviation D13 records
the reference behaviour. The provenance of the statements on an integer larger than 2^64 − 1 or smaller
than -2^63 is `design`. A number with a fraction loses its fraction. Thus `1.9` gives 1. The
conversion of a Float tag or a Double tag has the provenance `source_suggested`.

**STK-SHP-048** (`source_suggested_black_box_confirmed`, wire). A dye colour is a name of a list
(STK-SHP-003). The list has these names and ids:

| Name | Id | Name | Id | Name | Id | Name | Id |
|------|----|------|----|------|----|------|----|
| `white` | 0 | `yellow` | 4 | `light_gray` | 8 | `brown` | 12 |
| `orange` | 1 | `lime` | 5 | `cyan` | 9 | `green` | 13 |
| `magenta` | 2 | `pink` | 6 | `purple` | 10 | `red` | 14 |
| `light_blue` | 3 | `gray` | 7 | `blue` | 11 | `black` | 15 |

The comparison of a name is exact. Thus `Red` is an error. The wire form is the id as a VarInt.
STK-SHP-005 states the reader of an id out of `0..15`.

**STK-SHP-049** (`source_suggested_black_box_confirmed`, wire). Entity data is an entity type and a
compound of free NBT. The entity type is an entry of the static registry `minecraft:entity_type`.
`assets/mcrs/reports/registries.json` gives the numeric ids. Each entity type is valid. For
example, `minecraft:bees` accepts the entity type `minecraft:pig`.

The saved form is one NBT compound, with the forms and the JSON conversions of STK-SHP-043. The
compound must have the key `id`. Its value is a String tag: the identifier of the entity type. The
reader takes the entity type from `id`, and keeps the other keys as the compound. The reader of the
saved form gives an error in these cases:

- the compound has no key `id`;
- the value of `id` is not a String tag;
- the value of `id` does not name an entity type.

The writer of the saved form writes the compound with the key `id` added, as an identifier with its
namespace. The hash node is the map node of that compound, as STK-SHP-043 states.

The wire form has these fields, in this order:

| Field | Wire form |
|-------|-----------|
| entity type | VarInt: the numeric id |
| compound | one network NBT of the compound, without the key `id` |

The network NBT has the limits of STK-SHP-004. An NBT that is not a compound is a decode error. The
end tag `00` in place of the compound is also a decode error. When the compound on the wire has the
key `id`, the reader drops that key. The entity type comes only from the VarInt.

Equality: two values are equal when the entity types are equal and the compounds without `id` are
equal (STK-EQ-006).

**STK-SHP-050** (`source_suggested_black_box_confirmed`, wire). An element of the types
`minecraft:bundle_contents`, `minecraft:charged_projectiles` and `minecraft:container` holds one
item template (section 6.6). These statements apply to such an element:

- The saved form is the saved form of the item template (STK-TPL-002). The reader also accepts the
  string form. The writer always writes the map, and omits `count` when the count is 1.
- The wire form is the wire form of the item template (STK-TPL-003). It has no length prefix, also
  inside the wire stack with length prefix (STK-NET-011).
- No element holds the empty stack. In the saved form, the item air and a count out of `1..99` are
  errors (STK-TPL-002). In the wire form, the item air and the count 0 are decode errors with the
  text `Item must be non-empty` (STK-TPL-008).
- The patch of the item template keeps each patch entry, also an entry without effect
  (STK-TPL-007). The writers write each such entry. For example, an arrow with
  `minecraft:max_stack_size` 64 keeps that entry in the JSON, the NBT, the wire form and the hash.
- The rules of this section put no limit on the number of levels of nested item templates.
  STK-TPL-010 limits the NBT depth of the stack that holds them.

The hash of the component is the hash of its whole saved form (STK-HSH-004). The tree of an element
is the map of STK-TPL-002. The key `components` holds a map. Its keys are `<type>` and `!<type>`
with the namespace, as STK-SAV-003 states. The value of a `<type>` key is the saved form of the
nested component, as a subtree of the same tree. The value of a `!<type>` key is the empty map. The
hash does not use a separate hash of a nested component.

Equality: two elements are equal when their items, their counts and their patches are equal. Two
patches are equal when they have patch entries for the same component types, of the same kind, with
equal components (STK-PAT-007). A patch entry without effect takes part. Thus
`{"id":"minecraft:arrow"}` and `{"id":"minecraft:arrow","components":{"minecraft:max_stack_size":64}}`
are not equal.

**STK-SHP-051** (`source_suggested_black_box_confirmed`, wire). `minecraft:bundle_contents`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | list of item template (STK-SHP-050) | no limit on the length | necessary | list of item template |

The order of the list has a meaning. The writers keep the order. The value has no other field. A
document for bundles states the weight of the contents, the selected element and the extra check of
strict validation (STK-VAL-001).

For example, `["minecraft:arrow",{"id":"minecraft:arrow","count":1,"components":{"minecraft:max_stack_size":64}}]`
gives the saved form
`[{"id":"minecraft:arrow"},{"id":"minecraft:arrow","components":{"minecraft:max_stack_size":64}}]`.
Its wire form is `02 8808 01 00 00 8808 01 01 00 01 40`, and its hash is -128193339. Limits: none
on the number of elements. Equality: the two lists have equal elements (STK-SHP-050) in the same
order. Prototypes: 17 prototypes have this component with the empty list `[]`. The other 1669
prototypes do not have it.

**STK-SHP-052** (`source_suggested_black_box_confirmed`, wire). `minecraft:charged_projectiles`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | list of item template (STK-SHP-050) | 0 to 1024 elements | necessary | list of item template, at most 1024 elements |

Limits: 1024 elements. A saved form with more elements is not a valid saved form. A wire form with
a number larger than 1024 is a decode error. No reader removes the elements past the limit. The
order of the list has a meaning, and the writers keep it.

For example, the value with one firework rocket that has
`{"flight_duration":3,"explosions":[{"shape":"creeper","colors":[255]}]}` as `minecraft:fireworks`
has the wire form `01 8A0B 01 01 00 47 03 01 03 01 000000FF 00 00 00`.
Equality: the two lists have equal elements (STK-SHP-050) in the same order. Prototypes: 1
prototype has this component with the empty list `[]`. The other 1685 prototypes do not have it.

**STK-SHP-053** (`source_suggested_black_box_confirmed`, wire). `minecraft:container` holds the
slots of a container item. The value is a list of `n` slots, from slot 0 to slot `n - 1`. Each slot
holds an item template or nothing. Slot `n - 1` holds an item template. The empty value has `n` = 0.
`n` is at most 256.

The saved form is a list of maps, with 0 to 256 elements. Each map has these fields:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `slot` | int | `0..255` | necessary | none: the position in the list of the wire form |
| `item` | item template (STK-SHP-050) | STK-SHP-050 | necessary | none: see below |

The reader of the saved form puts the item template of each map into its slot. When two maps have
one slot, the map that is later in the list wins. This is not an error. The limit of 256 counts
each map of the input, also a map with a repeated slot. The writer of the saved form writes one map
for each slot that holds an item template, in the order of the slot numbers. It writes no map for a
slot that holds nothing. Thus `[{"slot":5,"item":"minecraft:stone"},{"slot":2,"item":{"id":"minecraft:apple","count":3}}]`
gives `[{"slot":2,"item":{"id":"minecraft:apple","count":3}},{"slot":5,"item":{"id":"minecraft:stone"}}]`.

The wire form is a list of 0 to 256 elements. Element `i` is slot `i`. Each element is an optional
item template (STK-TPL-005): `00` for a slot that holds nothing, or `01` and the item template. The
writer writes `n` elements. A number larger than 256 is a decode error. For example, the value
above has the wire form `06 00 00 01 8608 03 00 00 00 00 01 01 01 00 00`. A value with one item
template in slot 255 has 256 elements on the wire.

The hash is the hash of the saved form (STK-SHP-050). Limits: 256 slots, and 256 maps in the saved
form. Equality: the two values have the same `n`, and each slot holds equal item templates or
nothing in both. The order of the maps in the input has no effect. Prototypes: 45 prototypes have
this component with the empty list `[]`. The other 1641 prototypes do not have it. A document for
containers states how a block copies the slots.

**STK-SHP-054** (`design`, wire). The reader of the wire form of `minecraft:container` drops each
element `00` at the end of the list. Thus `02 01 8808 01 00 00 00` gives the value with one slot,
and `01 00` gives the empty value. A writer must not write an element `00` at the end of the list.
Reason: a value must have one saved form, one wire form and one hash. The saved form has no form
for an empty slot at the end. Deviation D9 records the reference behaviour.

**STK-SHP-055** (`source_suggested_black_box_confirmed`, wire). `minecraft:container_loot`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `loot_table` | identifier | each identifier | necessary | in the network NBT |
| `seed` | long (STK-SHP-047) | each long | `0` | in the network NBT |

The wire form is the saved form as one network NBT (STK-TYP-003), with the limits of STK-SHP-004.
`loot_table` names an entry of the registry `minecraft:loot_table`. No reader checks that the loot
table exists. Thus `minecraft:chests/x` is valid. A string that is not an identifier is an error.

For example, `{"loot_table":"minecraft:chests/x","seed":-1}` has the hash -148875692. The wire
form of `{"loot_table":"minecraft:chests/simple_dungeon"}` is
`0A 08 000A 6C6F6F745F7461626C65 001F <the identifier> 00`. Limits: none. Equality: each field is
equal. Prototypes: no prototype has this component.

**STK-SHP-056** (`source_suggested_black_box_confirmed`, wire). `minecraft:bees` is a list of
occupants:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | list of occupant (below) | no limit on the length | necessary | list of occupant |

An occupant has these fields:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `entity_data` | entity data (STK-SHP-049) | STK-SHP-049 | necessary | STK-SHP-049 |
| `ticks_in_hive` | int | each int | necessary | VarInt |
| `min_ticks_in_hive` | int | each int | necessary | VarInt |

The component puts no limit on the number of occupants. A list of 4 occupants is valid. A document
for hives states how many occupants a hive takes. A negative number of ticks is valid.

For example, `[{"entity_data":"{id:\"bee\",HasNectar:1b}","ticks_in_hive":-1,"min_ticks_in_hive":2400}]`
gives the saved form
`[{"entity_data":{"HasNectar":1,"id":"minecraft:bee"},"ticks_in_hive":-1,"min_ticks_in_hive":2400}]`
in JSON. In NBT, `HasNectar` stays a Byte tag. The wire form is
`01 0B 0A 01 0009 4861734E6563746172 01 00 FFFFFFFF0F E012`, and the hash is -1122153002. Limits:
none on the number of occupants. Equality: the two lists have equal occupants in the same order.
Two occupants are equal when each field is equal. Prototypes: 2 prototypes have this component with
the empty list `[]`. The other 1684 prototypes do not have it.

**STK-SHP-057** (`source_suggested_black_box_confirmed`, wire). `minecraft:firework_explosion`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `shape` | name of the list "firework shape" (below) | below | necessary | VarInt: the id |
| `colors` | list of int of 4 bytes (STK-SHP-032) | each int | the empty list | list of 4 bytes |
| `fade_colors` | list of int of 4 bytes (STK-SHP-032) | each int | the empty list | list of 4 bytes |
| `has_trail` | boolean | `false`, `true` | `false` | one byte |
| `has_twinkle` | boolean | `false`, `true` | `false` | one byte |

The list "firework shape" has the names `small_ball` 0, `large_ball` 1, `star` 2, `creeper` 3 and
`burst` 4. The comparison of a name is exact. Thus `STAR` is an error. An element of `colors` or
`fade_colors` is an int. It is not a colour of STK-SHP-032, so a list of 3 numbers is an error. In
NBT, each list is a List tag of Int tags.

For example, `{"shape":"star","colors":[16711680,65280],"fade_colors":[255],"has_trail":true,"has_twinkle":true}`
has the wire form `02 02 00FF0000 0000FF00 01 000000FF 01 01`. The value `{"shape":"small_ball"}`
has the wire form `00 00 00 00 00`. Limits: none on the number of colours. Equality: each field is
equal. The order of each list has an effect. Prototypes: no prototype has this component.

**STK-SHP-058** (`source_suggested_black_box_confirmed`, wire). `minecraft:fireworks`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `flight_duration` | unsigned byte (STK-SHP-034) | each int | `0` | VarInt, `0..255` |
| `explosions` | list of firework explosion (STK-SHP-057) | 0 to 256 elements | the empty list | list of firework explosion, at most 256 elements |

`flight_duration` has the saved form, the reader and the hash node of the unsigned byte of
STK-SHP-034. Thus `255` gives `-1` in JSON, `-1b` in NBT and `FF01` on the wire. `300` gives 44,
and `-1` gives 255. The wire form is a VarInt with the number in `0..255`, not one byte.

The reference reader of the wire form keeps each VarInt as the flight duration. Such a value out of
`0..255` has no saved form. The reader of this project gives a decode error for a number out of
`0..255`. The provenance of this paragraph is `design`. Deviation D10 records the reference
behaviour.

Limits: 256 explosions. A saved form with more explosions is not a valid saved form. A wire form
with a number larger than 256 is a decode error. For example,
`{"flight_duration":-56,"explosions":[{"shape":"burst","colors":[1]}]}` has the wire form
`C801 01 04 01 00000001 00 00 00`. Equality: each field is equal, and the two lists have equal
explosions in the same order. Prototypes: 1 prototype has this component with
`{"flight_duration":1}`. The other 1685 prototypes do not have it.

**STK-SHP-059** (`source_suggested_black_box_confirmed`, wire). `minecraft:banner_patterns` is a
list of layers:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | list of layer (below) | no limit on the length | necessary | list of layer |

A layer has these fields:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `pattern` | referenced value of the registry `minecraft:banner_pattern` (STK-SHP-033) | STK-SHP-033 | necessary | STK-SHP-033 |
| `color` | dye colour (STK-SHP-048) | STK-SHP-048 | necessary | VarInt: the id |

`minecraft:banner_pattern` is a data-pack registry. An inline banner pattern has these fields:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `asset_id` | identifier | each identifier | necessary | String (32767) |
| `translation_key` | string | each string | necessary | String (32767) |

`asset_id` names an asset of a resource pack. No reader checks that the asset exists, or that a
translation of `translation_key` exists. The component puts no limit on the number of layers. A
list of 7 layers is valid. A document for the loom states the limit of a loom.

The order of the layers has a meaning, and the writers keep it. For example,
`[{"pattern":"minecraft:creeper","color":"red"},{"pattern":{"asset_id":"mcrs:x","translation_key":"block.mcrs.banner.x"},"color":"light_blue"}]`
has the wire form `02 05 0E 00 06 6D6372733A78 13 <the translation key> 03`. The reference
`minecraft:creeper` has the id 4. Limits: those of the strings only. Equality: the two lists have
equal layers in the same order. Two layers are equal when each field is equal (STK-SHP-033).
Prototypes: 17 prototypes have this component with the empty list `[]`. The other 1669 prototypes
do not have it.

### 8.6 Shapes of the stored enchantment and painting variant types

This section states the shape of 2 more component types. STK-SHP-001 to STK-SHP-005 apply to each
rule of this section. The fixture `item_plain` has the cases of experiment E30. Each case has the
JSON, the NBT, the wire form and the hash that the game wrote. The fixture `item_registry_refs` has
one sample of `minecraft:stored_enchantments`.

**STK-SHP-060** (`source_suggested_black_box_confirmed`, wire). `minecraft:stored_enchantments` has
the shape of `minecraft:enchantments` (STK-SHP-015). The saved form, the ranges, the wire form, the
hash node and the equality are the same. Deviation D5 also applies to this component type. Only the
type id is different: 13 for `minecraft:enchantments` and 45 for `minecraft:stored_enchantments`.

The component holds the enchantments that an enchanted book stores. They do not act on the holder of
the stack. Another document states how an anvil applies them to another stack.

For example, `{"sharpness":255}` gives the saved form `{"minecraft:sharpness":255}`, the wire form
`01 21 FF01` and the hash -604126434. The empty map `{}` has the wire form `00` and the hash
-982207288. The wire form `02 21 01 21 02` gives `{"minecraft:sharpness":2}`. The wire form
`01 FF01 01` names no enchantment, thus it is a decode error.

Prototypes: 1 prototype has this component. It is `minecraft:enchanted_book`, with the empty map
`{}`. The other 1685 prototypes do not have it.

**STK-SHP-061** (`source_suggested_black_box_confirmed`, wire). `minecraft:painting/variant`:

| Field | Saved form | Range | Absent | Wire form |
|-------|------------|-------|--------|-----------|
| `(value)` | entry of the registry `minecraft:painting_variant` | each entry | necessary | VarInt: the numeric id of the entry plus 1 |

`minecraft:painting_variant` is a data-pack registry. The numeric id is the id that the server sent
for this connection (`registry.md`). The component sets the variant of the painting that the item
places. Another document states the painting.

The saved form is one String tag: the identifier of the entry. The reader accepts an identifier
without a namespace. The saved form has no inline painting variant. Thus a map is an error, also a
map with the fields of a painting variant. A number is an error. An identifier that names no entry
is an error. The hash node is a string node with the namespace.

The wire form is the wire form of a reference of STK-SHP-033: the numeric id plus 1. STK-SHP-062
states the VarInt 0. For example, `"minecraft:kebab"` has the numeric id 24 in the vanilla
registry. It has the wire form `19` and the hash -670067346. `"aztec"` gives `"minecraft:aztec"`,
the wire form `02` and the hash -260769345. The wire form `01` gives `minecraft:alban`, the entry
with the numeric id 0. The wire form `FF01` names no entry, thus it is a decode error.

Limits: none. Equality: the two values name the same entry. Prototypes: no prototype has this
component.

**STK-SHP-062** (`design`, deviation). The reader of the wire form of `minecraft:painting/variant`
gives a decode error for the VarInt 0. A writer must not write the VarInt 0. In the reference, the
VarInt 0 starts an inline painting variant, and such a value has no saved form (STK-SHP-061).
Reason: a reader must not accept a value that the writer of the saved form and the hash cannot
write. Deviation D14 records the reference behaviour.

## 9. Intentional deviations

**D1.**

- **Rule**: STK-NET-004.
- **Reference**: the reference reader accepts two patch entries for one component type. The last
  value entry wins, and a removal entry wins over a value entry.
- **Visible to the client**: no. The vanilla server writes each component type one time.

**D2.**

- **Rule**: STK-CNT-005 and STK-CMP-005.
- **Reference**: the reference writer of the wire stack does not check the count or the range of a
  component. The reference reader of the wire stack accepts a count larger than 99.
- **Visible to the client**: yes, when a server sends such a stack. The client shows the count,
  and its next `set_creative_mode_slot` packet for that stack fails at the server.

**D3.**

- **Rule**: STK-CNT-003.
- **Reference**: the reference reader of the wire stack and of the hashed stack accepts each
  positive VarInt as the count.
- **mcrs**: the count has 8 bits. The reader of the wire stack gives a decode error for a count
  larger than 255. A hashed stack with a count larger than 255 is equal to no stack.
- **Visible to the client**: no. The vanilla client and the vanilla server send no count larger
  than 99.

**D4.**

- **Rule**: STK-NET-010.
- **Reference**: the reference reader accepts a component that uses fewer bytes than its length.
  It ignores the bytes that remain inside the length.
- **mcrs**: the reader of the wire stack with length prefix gives a decode error for such a
  component. Reason: a strict reader refuses input that no conforming writer produces.
- **Visible to the client**: no. The vanilla client writes the exact length.

**D5.**

- **Rule**: STK-SHP-015.
- **Reference**: the reference reader of the wire form of `minecraft:enchantments` accepts the
  level 0. In the wire stack with length prefix, the check of STK-NET-009 then rejects the stack.
- **mcrs**: the reader of the wire form gives a decode error for the level 0 in each format that
  carries the component. Reason: a strict reader refuses input that no conforming writer produces.
- **Visible to the client**: no. The vanilla client and the vanilla server write no level 0.

**D6.**

- **Rule**: STK-SHP-025.
- **Reference**: the reference reader of the map form of the properties checks only the number of
  keys. It accepts more than 16 properties in total, a name longer than 64 code units and a value
  longer than 32767 code units. The writers of the saved form, of the hash and of the wire form
  then fail for that component.
- **mcrs**: the reader of the map form gives an error for such a value. Reason: a reader must not
  accept a value that no writer can write.
- **Visible to the client**: no. The vanilla client and the vanilla server write only the list
  form. A data pack, a command or a save file with such a map form gets an error in mcrs. In the
  reference, the stack fails later, when the game saves it or sends it.
- **Bug reference**: none.
- **Test**: a differential test does not compare a map form that is outside these limits.

**D7.**

- **Rule**: STK-SHP-025 and STK-HSH-004.
- **Reference**: the reference keeps the saved form and the hash of a profile in a cache. It finds
  an entry of the cache by equality (STK-SHP-027). Thus a profile can get the saved form and the
  hash of an earlier equal profile with a different order of groups. The result depends on the
  profiles that the process encoded before. The vanilla client and the vanilla server each have
  their own cache. The wire form does not use the cache.
- **mcrs**: the saved form and the hash of a profile come only from that profile. Reason: a result
  must not depend on earlier values.
- **Visible to the client**: only for two equal profiles with different orders of groups. The
  client can then send the hash of the other order. The hashed stack does not match, and the
  server sends the slot again (`inventory-transactions.md`). A vanilla server gives the same result
  when its cache holds the other order. The game makes such a pair only from a command, a data pack
  or a save file.
- **Bug reference**: none.
- **Test**: in the fixture `item_plain`, the second profile of two equal profiles has the saved
  form and the hash of the first. A differential test compares only the wire form of such a
  profile.

**D8.**

- **Rule**: STK-SHP-034 and STK-PAT-007.
- **Reference**: the reference finds two effect instances equal when only `hidden_effect` is
  different. The reference keeps the saved form and the hash of `minecraft:potion_contents` and
  `minecraft:consumable` in a cache, as D7 states for a profile. Thus an effect instance can get
  the saved form and the hash of an earlier instance with another hidden effect.
- **mcrs**: two effect instances are equal only when their `hidden_effect` values are also equal.
  The saved form and the hash come only from the value. Reason: STK-PAT-007 compares saved forms,
  and `hidden_effect` is a field of the saved form.
- **Visible to the client**: only for two effect instances that differ only in `hidden_effect`.
  A value of `minecraft:potion_contents` has no hidden effect (STK-SHP-042).
- **Bug reference**: none.
- **Test**: a differential test does not compare two such values in one process.

**D9.**

- **Rule**: STK-SHP-054.
- **Reference**: the reference reader of the wire form of `minecraft:container` keeps each element
  `00` at the end of the list. The value then has more slots than its saved form states. The
  reference finds it not equal to the value without those slots, but the two values have one saved
  form and one hash.
- **mcrs**: the reader drops each element `00` at the end of the list. Reason: a value must have
  one saved form, one wire form and one hash.
- **Visible to the client**: no. The vanilla client and the vanilla server write no element `00` at
  the end of the list.
- **Bug reference**: none.
- **Test**: a differential test compares the saved form and the hash of such a value, not the wire
  form that the reference writes again.

**D10.**

- **Rule**: STK-SHP-058.
- **Reference**: the reference reader of the wire form of `minecraft:fireworks` accepts each VarInt
  as `flight_duration`. The writer of the saved form then gives an error for a number larger than
  255. It writes a negative number as its low 8 bits.
- **mcrs**: the reader of the wire form gives a decode error for a number out of `0..255`. Reason: a
  strict reader refuses input that no conforming writer produces.
- **Visible to the client**: no. The vanilla client and the vanilla server write no such number.
- **Bug reference**: none.
- **Test**: a differential test does not compare a wire form with such a number.

**D11.**

- **Rule**: STK-TPL-010.
- **Reference**: the reference puts no limit on the depth of nested item templates in the wire
  form and in the JSON form. Only the size of the packet or of the file limits the depth. Thus the
  reference can hold and write a stack with an NBT depth larger than 512 (STK-TPL-009). Its NBT
  reader then refuses each file that holds that stack.
- **mcrs**: each reader of a stack refuses a stack with an NBT depth larger than 512. The count
  is the same for each kind of nesting. Reason: a reader must not overflow its stack. Each stack
  must also have a saved form that the NBT reader can read again.
- **Visible to the client**: no. A stack of normal play has an NBT depth far below 512. Only a
  command, a data pack or a modified client can make a deeper stack.
- **Bug reference**: none.
- **Test**: each stack of the boundary table of STK-TPL-010 gives the result of the table. The test
  reads each stack as a wire stack, as a wire stack with length prefix, and as a saved stack in NBT
  and in JSON. A bundle chain of 100000 levels in one wire stack gives a
  decode error, and the reader does not overflow its stack. A differential test does not compare a
  refused stack with the result of the game.

**D12.**

- **Rule**: STK-TPL-004.
- **Reference**: the reference reader of the wire form of an item template accepts a negative
  count. A stack that someone makes from that item template is the empty stack.
- **mcrs**: the reader gives a decode error for a negative count. Reason: a writer must not write a
  count out of `1..99`. The reader accepts a count of 100 to 255. The saved form check then refuses
  the item template in the wire stack with length prefix.
- **Visible to the client**: no. The vanilla client and the vanilla server write no such count.
- **Bug reference**: none.
- **Test**: none.

**D13.**

- **Rule**: STK-SHP-047.
- **Reference**: the reference reader of the saved form in JSON converts each integer number to a
  long by its low 64 bits. Thus `18446744073709551617` gives 1, and `-9223372036854775809` gives
  9223372036854775807.
- **mcrs**: the JSON reader reads an integer out of the range -2^63 to 2^64 − 1 as a double. The
  long then comes from the double, as STK-SHP-002 states for a Double tag. Thus
  `18446744073709551617` gives 9223372036854775807, and `-9223372036854775809` gives
  -9223372036854775808. Reason: the JSON reader of mcrs holds an integer in at most 64 bits.
- **Visible to the client**: no. The JSON form is not on the wire, and the vanilla writers write no
  such integer. Only a hand-written data pack file can hold one.
- **Bug reference**: none.
- **Test**: the case of the fixture `item_plain` with the seed `18446744073709551617` expects the
  seed 9223372036854775807. The test does not compare that case with the result of the game.

**D14.**

- **Rule**: STK-SHP-062.
- **Reference**: the reference reader of the wire form of `minecraft:painting/variant` reads the
  VarInt 0, then an inline painting variant. That value has no saved form and no hash. In the wire
  stack with length prefix, the check of STK-NET-009 then refuses the stack. In each other packet,
  the vanilla client keeps the value.
- **mcrs**: the reader of the wire form gives a decode error for the VarInt 0 in each format that
  carries the component. Reason: a reader must not accept a value that the writer of the saved form
  and the hash cannot write.
- **Visible to the client**: no. The vanilla client and the vanilla server make a painting variant
  of an item only from its saved form, which has no inline form. Thus they write no VarInt 0.
- **Bug reference**: none.
- **Test**: a wire form `00` of `minecraft:painting/variant` gives a decode error. A differential
  test does not compare a wire form with the VarInt 0.

## 10. Unresolved

- **Q1**: STK-SAV-006. Which callers keep a stack whose patch has a bad key, and which drop the
  stack? The owner decides if this project copies the reference for each caller.
- **Q2**: STK-DMG-006. A stack with a count larger than 1 that breaks keeps a damage value equal
  to the maximum damage. Strict validation prevents such a stack, but the saved reader does not.
  Does this project copy the reference, or does the remainder get the damage value 0?
- **Q3**: STK-CMD-001. Is the command form in the scope of this document, or does it move to an
  `argument-item.md` with the item predicate form?
- **Q4**: STK-HSH-008. A stack with `minecraft:map_post_processing` can be in a result slot when
  the client clicks. Which hash does the client send, or does the client fail?
- **Q7**: STK-TYP-003. The wire form of each `own` component type needs a specification. Which
  document owns each group of types? Sections 8.1 to 8.6 state 40 types, with `map_id`,
  `map_decorations`, `profile`, `equippable`, the 11 types of section 8.4, the 8 types of
  section 8.5 and the 2 types of section 8.6.
- **Q9**: STK-SHP-002. The four integer component types are signed 32-bit integers in the game.
  Does this project keep 32 bits, or use a smaller type with a deviation as D3?
- **U2**: STK-CMD-003. If `[]` is valid in the command form.
- **U3**: the command form with the item `minecraft:air`.
- **U4**: STK-SHP-042. What the vanilla client does with a hidden effect in the wire form of
  `minecraft:potion_contents`: it keeps the effect, drops it or fails.
- **U5**: STK-TPL-009. A file holds a stack below its own root, for example in the inventory of a
  player or in a block entity of a chunk. The NBT reader of the file counts from the root of the
  file. Thus it can refuse a file with a stack that STK-TPL-010 accepts. What does the writer of
  such a file do with that stack? The document of each file decides.

## 11. Experiments

Each experiment ran against the game version of `assets/minecraft/version.json`. The fixture of an
experiment holds its raw output.

**E26.** Rules: STK-SHP-022, STK-SHP-024, STK-SHP-025, STK-SHP-026, STK-SHP-027 and STK-HSH-004.

Steps:

1. Add each case of the table below to the input of the fixture `item_plain` of `tools/fixtures.json`.
2. Write the fixture again with `tools/vanilla-oracle`.
3. For a case of the kind "value", read the JSON, the NBT, the wire form and the hash.
4. For a case of the kind "error", read the rejection.
5. For a case of the kind "decode", give the wire form to the reader. Read the JSON of the result.
6. For a case of the kind "decode error", give the wire form to the reader. Read the decode error.

The fixture keeps the cases of the four kinds in its lists `values`, `errors`, `decodes` and
`decode_errors`. The field `kind` of a case gives the component type.

| Component type | Kind | Input | Rule |
|----------------|------|-------|------|
| `map_decorations` | value | `{"m1":{"type":"minecraft:player","x":1.5,"z":-2.5,"rotation":90.0}}` | STK-SHP-022 |
| `map_decorations` | value | `{"":{"type":"red_x","x":1,"z":-2,"rotation":-45,"extra":true}}` | STK-SHP-022 |
| `map_decorations` | value | `{"a":{"type":"minecraft:target_x","x":0.0,"z":0.0,"rotation":0.0},"b":{"type":"minecraft:banner_red","x":-1.0E300,"z":1.0E-300,"rotation":359.5}}` | STK-SHP-022 |
| `map_decorations` | error | `{"m1":{"type":"minecraft:player","x":1.0,"z":2.0}}` | STK-SHP-022 |
| `map_decorations` | error | `{"m1":{"type":"minecraft:player","x":"1","z":2.0,"rotation":0.0}}` | STK-SHP-022 |
| `map_decorations` | error | `[]` | STK-SHP-022 |
| `profile` | value | `"Notch"` | STK-SHP-024 |
| `profile` | value | `""` | STK-SHP-024 |
| `profile` | value | `{}` | STK-SHP-024 |
| `profile` | value | `{"id":[1,2,3,4],"name":"Steve"}` | STK-SHP-024 |
| `profile` | value | `{"id":[1.9,2,3,4]}` | STK-SHP-024 |
| `profile` | value | `{"name":"abcdefghijklmnop","unknown":1,"model":"slim"}` | STK-SHP-024 |
| `profile` | value | `{"id":[1,2,3,4],"name":"Steve","properties":[{"name":"x","value":"1"},{"name":"textures","value":"t"},{"name":"x","value":"2"}]}` | STK-SHP-025 |
| `profile` | value | `{"name":"Steve","properties":{"textures":["v1","v2"]}}` | STK-SHP-025 |
| `profile` | value | `{"name":"Steve","properties":[{"name":"x","value":"1"},{"name":"textures","value":"t"}]}` | STK-SHP-027 |
| `profile` | value | `{"name":"Steve","properties":[{"name":"textures","value":"t"},{"name":"x","value":"1"}]}` | STK-SHP-027 |
| `profile` | value | `{"name":"Steve","properties":[{"name":"x","value":"1"},{"name":"x","value":"2"}]}` | STK-SHP-027 |
| `profile` | value | `{"name":"Steve","properties":[{"name":"x","value":"2"},{"name":"x","value":"1"}]}` | STK-SHP-027 |
| `profile` | value | `{"id":[1,2,3,4],"name":"steve"}` | STK-SHP-027 |
| `profile` | value | `{"name":"Steve","properties":[{"name":"x","value":"1"}]}` | STK-SHP-027 |
| `profile` | value | `{"name":"Steve","properties":[{"name":"x","value":"1","signature":"s"}]}` | STK-SHP-027 |
| `profile` | value | `{"name":"Steve","properties":[{"name":"x","value":"1"}],"model":"slim"}` | STK-SHP-027 |
| `profile` | error | `{"id":[1,2,3]}` | STK-SHP-024 |
| `profile` | error | `{"id":"00000001-0000-0002-0000-000300000004"}` | STK-SHP-024 |
| `profile` | error | `{"name":"é"}` | STK-SHP-024 |
| `profile` | error | `{"model":"default"}` | STK-SHP-024 |
| `profile` | error | a list form with 17 properties `{"name":"p","value":"v"}` | STK-SHP-025 |
| `profile` | error | a list form with one property whose name has 65 code units | STK-SHP-025 |
| `profile` | error | a map form with the 17 keys `"a"` to `"q"`, each with the value `[]` | STK-SHP-025 |
| `profile` | decode | `00 01 05 5374657665 01 00000001 00000002 00000003 00000004 00 00 00 00 00` | STK-SHP-026 |
| `profile` | decode | `00 00 00 03 01 78 01 31 00 08 7465787475726573 01 74 00 01 78 01 32 00 00 00 00 00` | STK-SHP-025 |
| `profile` | decode error | `00 00 00 11` | STK-SHP-026 |

Results:

- The game accepts each case of the kind "value". It rejects each case of the kind "error".
- The wire form of each value of `minecraft:map_decorations` is the network NBT of its saved form.
- The case with the key `""` keeps the empty key. It gives the type `minecraft:red_x`, the
  numbers `1.0`, `-2.0` and `-45.0`, and no key `extra`.
- The game writes the keys of a compound of `minecraft:map_decorations` in an order that is
  different from the order of the input.
- `"Notch"` and `""` give a partial profile with only a name. `{}` gives a partial profile with no
  field.
- `{"id":[1.9,2,3,4]}` gives the UUID of the ints `1, 2, 3, 4`.
- The saved form of the full profile without properties has no key `properties`.
- The saved form of the case with the key `unknown` has no key `unknown`. Its wire form ends with
  `01 01`, the model `slim`.
- The input `x=1, textures=t, x=2` gives the list `x=1, x=2, textures=t` in each format.
- The map form `{"textures":["v1","v2"]}` gives the list form with two properties.
- The first case of the kind "decode" gives a saved form with the keys `name` and `id`. As a
  tree, this saved form is equal to the saved form of the full profile with the same values.
- The second case of the kind "decode" has the properties `x=1, textures=t, x=2` on the wire. It
  gives the list `x=1, x=2, textures=t`.
- The case `textures=t, x=1` gives the saved form and the hash of the case `x=1, textures=t`
  before it. Its wire form keeps the order `textures=t, x=1`. Thus the game gives a profile the
  saved form and the hash of an earlier equal profile (D7).
- Four later cases each differ from an earlier case in one condition: the order in a group, the
  case of the name, the signature or the skin patch. Each of the four cases keeps its own saved
  form. Thus the game does not find it equal to the earlier case.
- The saved form of the first case of the kind "decode" has the keys in a different order from
  the full profile with the same values. Thus the game does not find a partial profile and a full
  profile equal.
- The case of the kind "decode error" gives a decode error for 17 properties.

**E27.** Rule: STK-SHP-029, the read of a `range` that is not a valid float.

Steps:

1. Add each case of the table below to the input of the fixture `item_plain` of
   `tools/fixtures.json`. Each case has the kind "value" and the component type `equippable`.
2. Write the fixture again with `tools/vanilla-oracle`.
3. For each case, read the JSON, the NBT, the wire form and the hash.

The fixture keeps the cases in its list `values`. Each input is
`{"slot":"head","equip_sound":{"sound_id":"minecraft:item.armor.equip_generic","range":R}}`, with
the value `R` of the table.

| `R` | Kind of the value |
|-----|-------------------|
| `"x"` | a string |
| `"2.5"` | a string of a number |
| `[1.0]` | a list |
| `{"a":1.0}` | a map |

Results:

- The game accepts each case.
- Each case gives the same JSON, NBT, wire form and hash. The inline sound event has the
  `sound_id` of the input and no `range`.
- The wire form of the sound event is `00`, then `sound_id` as String (32767), then `00` for no
  range.

**E28.** Rules: STK-SHP-032 to STK-SHP-044 and STK-SHP-046.

Steps:

1. Add each case of the table below to the input of the fixture `item_plain` of `tools/fixtures.json`.
2. Write the fixture again with `tools/vanilla-oracle`.
3. For a case of the kind "value", read the JSON, the NBT, the wire form and the hash.
4. For a case of the kind "error", read the rejection.
5. For a case of the kind "decode", give the wire form to the reader. Read the JSON of the result.

The fixture keeps the cases of the three kinds in its lists `values`, `errors` and `decodes`. The
field `kind` of a case gives the component type.

| Component type | Kind | Input | Rule |
|----------------|------|-------|------|
| `dyed_color` | value | `[2.0,-0.1,0.5]` | STK-SHP-032 |
| `custom_model_data` | value | `{}` | STK-SHP-036 |
| `custom_model_data` | value | `{"floats":[1,-0.5],"flags":[false],"strings":["","a b"],"colors":[[1.0,0.0,0.0],255]}` | STK-SHP-036 |
| `block_state` | value | `{"facing":"north","half":"top","waterlogged":"false"}` | STK-SHP-038 |
| `block_state` | value | `{"no_such_property":"any text"}` | STK-SHP-038 |
| `lodestone_tracker` | value | `{"target":{"dimension":"overworld","pos":[33554431,2047,-33554432]},"tracked":true}` | STK-SHP-039 |
| `lodestone_tracker` | value | `{"target":{"dimension":"mcrs:custom","pos":[0,4096,-1]}}` | STK-SHP-039 |
| `lodestone_tracker` | value | `{"tracked":false}` | STK-SHP-039 |
| `trim` | value | `{"material":{"palette_id":"mcrs:pal","description":"Pal","extra":1},"pattern":"coast"}` | STK-SHP-040 |
| `potion_contents` | value | `"minecraft:water"` | STK-SHP-041 |
| `potion_contents` | value | `{"custom_effects":[{"id":"minecraft:speed"}]}` | STK-SHP-041 |
| `potion_contents` | value | `{"custom_effects":[{"id":"minecraft:speed","amplifier":3,"duration":40,"hidden_effect":{"amplifier":1,"duration":200,"hidden_effect":{"duration":600}}}]}` | STK-SHP-042 |
| `potion_contents` | value | `{"custom_effects":[{"id":"minecraft:speed","show_particles":false}]}` | STK-SHP-041 |
| `potion_contents` | value | `{"custom_effects":[{"id":"minecraft:speed","amplifier":255},{"id":"minecraft:speed","amplifier":256},{"id":"minecraft:speed","amplifier":300},{"id":"minecraft:speed","amplifier":-1}]}` | STK-SHP-034 |
| `potion_contents` | value | `{"custom_name":""}` | STK-SHP-041 |
| `suspicious_stew_effects` | value | `[{"id":"minecraft:speed","duration":-5},{"id":"minecraft:speed","duration":0}]` | STK-SHP-044 |
| `instrument` | value | `{"sound_event":"minecraft:entity.item.break","use_duration":0.0,"range":3.4028235E38,"durability_damage":0,"description":""}` | STK-SHP-046 |
| `custom_data` | value | `{}` | STK-SHP-043 |
| `custom_data` | value | `"{}"` | STK-SHP-043 |
| `custom_data` | value | `"{a:1b,b:2s,c:3,d:4L,e:5.0f,f:6.0d,g:\"x\",h:[I;1,2],i:[B;1b],j:[L;1L],k:[],l:{m:[1,2]},n:true}"` | STK-SHP-043 |
| `custom_data` | value | `{"a":1,"b":1.5,"c":true,"d":"x","e":[1,2],"f":{},"g":300,"h":3000000000}` | STK-SHP-043 |
| `custom_data` | value | `"{mixed:[1b,\"a\",{}]}"` | STK-SHP-043 |
| `consumable` | value | `{"on_consume_effects":[{"type":"apply_effects","effects":[{"id":"minecraft:haste","amplifier":7,"duration":77,"hidden_effect":{"amplifier":1,"duration":200}}]}]}` | STK-SHP-042 |
| `potion_contents` | value | `{"custom_effects":[{"id":"minecraft:haste","amplifier":7,"duration":77,"hidden_effect":{"amplifier":1,"duration":200}}]}` | STK-SHP-042 |
| `dyed_color` | error | `{"rgb":16711680}` | STK-SHP-032 |
| `dyed_color` | error | `[1.0,1.0,1.0,1.0]` | STK-SHP-032 |
| `custom_model_data` | error | `{"floats":["x"]}` | STK-SHP-036 |
| `custom_model_data` | error | `{"colors":["#FF0000"]}` | STK-SHP-036 |
| `block_state` | error | `{"level":1}` | STK-SHP-038 |
| `block_state` | error | `{"level":true}` | STK-SHP-038 |
| `lodestone_tracker` | error | `{"target":{"pos":[0,0,0]}}` | STK-SHP-039 |
| `lodestone_tracker` | error | `{"target":{"dimension":"overworld","pos":[0,0,0]},"tracked":"yes"}` | STK-SHP-039 |
| `trim` | error | `{"material":"minecraft:amethyst","pattern":"minecraft:no_such_pattern"}` | STK-SHP-040 |
| `trim` | error | `{"material":{"palette_id":"mcrs:pal"},"pattern":"minecraft:coast"}` | STK-SHP-040 |
| `trim` | error | `{"material":"minecraft:amethyst"}` | STK-SHP-040 |
| `potion_contents` | error | `"minecraft:no_such_potion"` | STK-SHP-041 |
| `potion_contents` | error | `{"custom_effects":[{"amplifier":1}]}` | STK-SHP-041 |
| `potion_contents` | error | `{"custom_effects":[{"id":"minecraft:speed","duration":"long"}]}` | STK-SHP-041 |
| `potion_contents` | error | `{"custom_name":5}` | STK-SHP-041 |
| `suspicious_stew_effects` | error | `[{"duration":5}]` | STK-SHP-044 |
| `suspicious_stew_effects` | error | `{"id":"minecraft:speed"}` | STK-SHP-044 |
| `instrument` | error | `"minecraft:no_such_horn"` | STK-SHP-046 |
| `instrument` | error | `{"sound_event":"minecraft:entity.item.break","use_duration":1.0,"range":0.0,"description":"x"}` | STK-SHP-046 |
| `instrument` | error | `{"sound_event":"minecraft:entity.item.break","use_duration":-0.0,"range":1.0,"description":"x"}` | STK-SHP-046 |
| `instrument` | error | `{"sound_event":"minecraft:entity.item.break","use_duration":1.0,"range":1.0,"durability_damage":-1,"description":"x"}` | STK-SHP-046 |
| `instrument` | error | `{"sound_event":"minecraft:entity.item.break","use_duration":1.0,"range":1.0}` | STK-SHP-046 |
| `custom_data` | error | `"[1,2]"` | STK-SHP-043 |
| `custom_data` | error | `"{a:"` | STK-SHP-043 |
| `custom_data` | error | `5` | STK-SHP-043 |
| `custom_data` | error | `[]` | STK-SHP-043 |
| `potion_contents` | error | `{"custom_color":[1.0,0.0,0.0]}` | STK-SHP-041 |
| `potion_contents` | decode | `0000010003280001010101c8010001010000` | STK-SHP-042 |

Results:

- The game accepts each case of the kind "value" and rejects each case of the kind "error".
- The colour `[2.0,-0.1,0.5]` gives `-72065`. In `custom_model_data`, the colour `[1.0,0.0,0.0]`
  gives `-65536`. `custom_color` of `potion_contents` rejects the list `[1.0,0.0,0.0]`.
- The position `[33554431,2047,-33554432]` has the wire form `7FFFFFE0000007FF`. The position
  `[0,4096,-1]` keeps `4096` in the saved form, and its wire form is `0000003FFFFFF000`. Thus the
  wire form has `y` = 0.
- The value with `"tracked":true` has no key `tracked` in the saved form.
- The inline trim material with the key `extra` loses that key. The pattern `coast` is written as
  `minecraft:coast`.
- The amplifiers 255, 256, 300 and -1 give `-1`, no key, `44` and `-1` in JSON. On the wire, they
  are `FF 01`, `00`, `2C` and `FF 01`.
- The effect with `"show_particles":false` gets `"show_icon":false` in the saved form.
- The two values of `potion_contents` with `hidden_effect` have no hidden effect in the JSON, the
  NBT and the wire form. The value of `consumable` with the same effect instance keeps
  `hidden_effect`. Its wire form has `01 01 C801 00 01 01 00` for the hidden effect.
- The case of the kind "decode" has a hidden effect on the wire. The JSON of the result has no
  hidden effect. The cache of D8 can give this JSON, so this case does not show what the reader of
  the wire form keeps.
- The durations `-5` and `0` of `suspicious_stew_effects` stay in the saved form.
- `instrument` accepts `use_duration` `0.0` and `range` `3.4028235E38`, and omits
  `durability_damage` `0`. It rejects `range` `0.0`, `use_duration` `-0.0` and
  `durability_damage` `-1`.
- The SNBT case of `custom_data` keeps each tag kind. In the JSON case, `1` and `true` give Byte
  tags, and `1.5` gives a Float tag. `300` gives a Short tag, `3000000000` gives a Long tag, and
  `[1,2]` gives a list of Byte tags.
- The list `[1b,"a",{}]` has the NBT `0A 00000003 | 01 0000 01 00 | 08 0000 0001 61 00 | 00`.
  Its hash is the hash of the list without the wrappers.

**E29.** Rules: STK-SHP-047 to STK-SHP-059.

Steps:

1. Add each case of the table below to the input of the fixture `item_plain` of `tools/fixtures.json`.
2. Write the fixture again with `tools/vanilla-oracle`.
3. For a case of the kind "value", read the JSON, the NBT, the wire form and the hash.
4. For a case of the kind "error", read the rejection.
5. For a case of the kind "decode", give the wire form to the reader. Read the JSON of the result.
6. For a case of the kind "decode error", give the wire form to the reader. Read the decode error.

The fixture keeps the cases of the four kinds in its lists `values`, `errors`, `decodes` and
`decode_errors`. The field `kind` of a case gives the component type.

| Component type | Kind | Input | Rule |
|----------------|------|-------|------|
| `bundle_contents` | value | `["minecraft:arrow",{"id":"minecraft:arrow","count":1,"components":{"minecraft:max_stack_size":64}}]` | STK-SHP-050 |
| `bundle_contents` | value | `[{"id":"minecraft:shulker_box","components":{"minecraft:container":[{"slot":1,"item":{"id":"minecraft:arrow","count":2,"components":{"minecraft:custom_name":"x","!minecraft:max_stack_size":{}}}}]}}]` | STK-SHP-050 |
| `charged_projectiles` | value | `[{"id":"minecraft:firework_rocket","components":{"minecraft:fireworks":{"flight_duration":3,"explosions":[{"shape":"creeper","colors":[255]}]}}}]` | STK-SHP-052 |
| `container` | value | `[{"slot":5,"item":"minecraft:stone"},{"slot":2,"item":{"id":"minecraft:apple","count":3}}]` | STK-SHP-053 |
| `container` | value | `[{"slot":0,"item":"minecraft:stone"},{"slot":0,"item":"minecraft:apple"}]` | STK-SHP-053 |
| `container` | value | `[{"slot":255,"item":"minecraft:stone"}]` | STK-SHP-053 |
| `container_loot` | value | `{"loot_table":"chests/x","seed":1.9}` | STK-SHP-047 |
| `container_loot` | value | `{"loot_table":"minecraft:chests/x","seed":18446744073709551617}` | STK-SHP-047 |
| `container_loot` | value | `{"loot_table":"minecraft:chests/x","seed":-1}` | STK-SHP-055 |
| `bees` | value | `[{"entity_data":"{id:\"bee\",HasNectar:1b}","ticks_in_hive":-1,"min_ticks_in_hive":2400}]` | STK-SHP-049 |
| `bees` | value | a list of 4 occupants `{"entity_data":{"id":"minecraft:bee"},"ticks_in_hive":0,"min_ticks_in_hive":0}` | STK-SHP-056 |
| `firework_explosion` | value | `{"shape":"large_ball","colors":[1.9,-2.9],"fade_colors":[],"has_trail":false}` | STK-SHP-057 |
| `fireworks` | value | `{"flight_duration":255}` | STK-SHP-058 |
| `fireworks` | value | `{"flight_duration":300}` | STK-SHP-058 |
| `fireworks` | value | `{"flight_duration":-1}` | STK-SHP-058 |
| `banner_patterns` | value | 7 layers with the pattern `creeper` and the colours `white`, `orange`, `magenta`, `light_blue`, `yellow`, `lime` and `black` | STK-SHP-059 |
| `bundle_contents` | error | `[{"id":"minecraft:air"}]` | STK-SHP-050 |
| `bundle_contents` | error | `[{"id":"minecraft:stone","count":0}]` | STK-SHP-050 |
| `bundle_contents` | error | `[{"id":"minecraft:stone","count":100}]` | STK-SHP-050 |
| `charged_projectiles` | error | a list of 1025 strings `"minecraft:arrow"` | STK-SHP-052 |
| `container` | error | `[{"slot":256,"item":"minecraft:stone"}]` | STK-SHP-053 |
| `container` | error | `[{"slot":-1,"item":"minecraft:stone"}]` | STK-SHP-053 |
| `container` | error | `[{"item":"minecraft:stone"}]` | STK-SHP-053 |
| `container` | error | a list of 257 maps `{"slot":0,"item":"minecraft:stone"}` | STK-SHP-053 |
| `container` | error | `[{"slot":0,"item":{"id":"minecraft:air"}}]` | STK-SHP-050 |
| `container_loot` | error | `{"seed":1}` | STK-SHP-055 |
| `container_loot` | error | `{"loot_table":"minecraft:chests/x","seed":"1"}` | STK-SHP-047 |
| `container_loot` | error | `{"loot_table":"Not Valid"}` | STK-SHP-055 |
| `bees` | error | `[{"entity_data":{"Health":5.0},"ticks_in_hive":0,"min_ticks_in_hive":0}]` | STK-SHP-049 |
| `bees` | error | `[{"entity_data":{"id":"minecraft:no_such_entity"},"ticks_in_hive":0,"min_ticks_in_hive":0}]` | STK-SHP-049 |
| `bees` | error | `[{"entity_data":{"id":"minecraft:bee"},"ticks_in_hive":0}]` | STK-SHP-056 |
| `bees` | error | `[{"entity_data":{"id":5},"ticks_in_hive":0,"min_ticks_in_hive":0}]` | STK-SHP-049 |
| `firework_explosion` | error | `{"colors":[1]}` | STK-SHP-057 |
| `firework_explosion` | error | `{"shape":"STAR"}` | STK-SHP-057 |
| `firework_explosion` | error | `{"shape":"star","colors":[[1.0,0.0,0.0]]}` | STK-SHP-057 |
| `fireworks` | error | a map with `explosions`: a list of 257 maps `{"shape":"star"}` | STK-SHP-058 |
| `banner_patterns` | error | `[{"pattern":"minecraft:no_such_pattern","color":"red"}]` | STK-SHP-059 |
| `banner_patterns` | error | `[{"pattern":"minecraft:creeper","color":"Red"}]` | STK-SHP-048 |
| `banner_patterns` | error | `[{"pattern":{"asset_id":"mcrs:x"},"color":"red"}]` | STK-SHP-059 |
| `banner_patterns` | error | `[{"pattern":"minecraft:creeper"}]` | STK-SHP-059 |
| `container` | decode | `02 01 8808 01 00 00 00` | STK-SHP-054 |
| `container` | decode | `01 00` | STK-SHP-054 |
| `bees` | decode | `01 67 0A 08 0002 6964 000D 6D696E6563726166743A636F77 00 0A 00` | STK-SHP-049 |
| `container` | decode error | `81 02` | STK-SHP-053 |
| `charged_projectiles` | decode error | `81 08` | STK-SHP-052 |
| `fireworks` | decode error | `00 81 02` | STK-SHP-058 |
| `bees` | decode error | `01 67 00 0A 00` | STK-SHP-049 |
| `bundle_contents` | decode error | `01 00 01 00 00` | STK-SHP-050 |

Results:

- The game accepts each case of the kind "value" and rejects each case of the kind "error".
- The string `"minecraft:arrow"` gives `{"id":"minecraft:arrow"}`. The count 1 is not in the saved
  form. The entry `minecraft:max_stack_size` 64 of an arrow stays in the JSON, the NBT, the wire
  form and the hash.
- The nested `minecraft:container` keeps its removal entry `!minecraft:max_stack_size`. In the
  hash, the value of that key is the empty map.
- Each hash of a value with item templates is the hash of the whole saved tree.
- The slots 5 and 2 give the saved order 2, 5 and 6 elements on the wire. Two maps with the slot 0
  give the item of the later map. The slot 255 gives 256 elements on the wire.
- The seed `1.9` gives 1, and `18446744073709551617` gives 1. The seed `-1` stays in the saved form.
  The table `chests/x` gets the namespace `minecraft`.
- The SNBT entity data gives the entity type `minecraft:bee`. `HasNectar` is a Byte tag in NBT and
  `1` in JSON. The ticks `-1` stay. Four occupants are valid.
- The colours `1.9` and `-2.9` give 1 and -2. The defaults of the explosion are not in the saved
  form.
- The flight durations 255, 300 and -1 give `-1`, `44` and `-1` in JSON. On the wire, they are
  `FF01`, `2C` and `FF01`.
- Seven layers are valid.
- The case `02 01 8808 01 00 00 00` gives the JSON of one slot. The case `01 00` gives `[]`. The
  JSON does not show if the reference keeps the element `00` at the end (D9).
- The case of `minecraft:bees` with the key `id` in the NBT gives the entity type
  `minecraft:pig`, from the VarInt, and no other key.
- Each case of the kind "decode error" gives a decode error: a list that is too long, the end tag
  in place of the compound, and the item air.

**E30.** Rules: STK-SHP-015, STK-SHP-060 and STK-SHP-061.

Steps:

1. Add each case of the table below to the input of the fixture `item_plain` of `tools/fixtures.json`.
2. Write the fixture again with `tools/vanilla-oracle`.
3. For a case of the kind "value", read the JSON, the NBT, the wire form and the hash.
4. For a case of the kind "error", read the rejection.
5. For a case of the kind "decode", give the wire form to the reader. Read the JSON of the result.
6. For a case of the kind "decode error", give the wire form to the reader. Read the decode error.

The fixture keeps the cases of the four kinds in its lists `values`, `errors`, `decodes` and
`decode_errors`. The field `kind` of a case gives the component type.

| Component type | Kind | Input | Rule |
|----------------|------|-------|------|
| `stored_enchantments` | value | `{"sharpness":255}` | STK-SHP-060 |
| `stored_enchantments` | value | `{}` | STK-SHP-060 |
| `painting/variant` | value | `"minecraft:kebab"` | STK-SHP-061 |
| `painting/variant` | value | `"aztec"` | STK-SHP-061 |
| `stored_enchantments` | error | `{"minecraft:sharpness":0}` | STK-SHP-015 |
| `stored_enchantments` | error | `{"minecraft:sharpness":256}` | STK-SHP-015 |
| `stored_enchantments` | error | `{"minecraft:no_such_enchantment":1}` | STK-SHP-015 |
| `stored_enchantments` | error | `{"sharpness":1,"minecraft:sharpness":2}` | STK-SHP-015 |
| `stored_enchantments` | error | `[]` | STK-SHP-015 |
| `painting/variant` | error | `"minecraft:no_such_painting"` | STK-SHP-061 |
| `painting/variant` | error | `{"width":1,"height":1,"asset_id":"mcrs:x"}` | STK-SHP-061 |
| `painting/variant` | error | `5` | STK-SHP-061 |
| `painting/variant` | decode | `01` | STK-SHP-061 |
| `stored_enchantments` | decode | `02 21 01 21 02` | STK-SHP-015 |
| `painting/variant` | decode error | `FF01` | STK-SHP-061 |
| `stored_enchantments` | decode error | `01 FF01 01` | STK-SHP-015 |

Results:

- The game accepts each case of the kind "value" and rejects each case of the kind "error".
- `sharpness` gets the namespace `minecraft`. The level 255 is an Int tag in NBT and `FF01` on the
  wire. The numeric id of `minecraft:sharpness` is `21` on the wire, with no offset.
- The levels 0 and 256 are errors. An enchantment that does not exist is an error. The keys
  `sharpness` and `minecraft:sharpness` in one map give an error for a duplicate key.
- The wire form `02 21 01 21 02` gives the level 2 of the last entry.
- `minecraft:kebab` has the wire form `19`: its numeric id 24 plus 1. `aztec` gets the namespace
  `minecraft`. The saved form and the hash node of each painting variant are a string.
- A map with the fields of a painting variant is an error: the saved form must be a string.
- The wire form `01` gives `minecraft:alban`. `FF01` gives a decode error for the id 254 of
  `minecraft:painting_variant`, and `01 FF01 01` gives a decode error for the id 255 of
  `minecraft:enchantment`. Thus only the painting variant has the offset 1.
