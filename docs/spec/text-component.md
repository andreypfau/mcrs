# Text component: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| World version | `5127` |

Source of these values: `assets/minecraft/version.json`.

This document states the text component of the game as data: its parts, its saved form, its wire
form, its equality and its hash. It has the depth that the codecs of the item components need. It
does not prescribe an architecture.

Scope: the value and its formats. Section 8 names each subject that this document does not state.

The text of this document is the text of Java Edition. It is the one model of a text in this
project. A Bedrock client gets a string that an edge makes from this model (issues #654 and #666).
This document states no form of Bedrock Edition.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**: as `login.md`, section 0.1, defines them.
- **Identifier**, **registry**: as `registry.md`, section 0.1, defines them.
- **Saved form**, **wire form**, **network NBT**, **item template**, **hashed stack**: as
  `item-stack.md`, section 0.1, defines them.
- **Text**: one text component. A text has one content, one style and a list of children.
- **Content**: the part of a text that gives its own characters. A content has one content kind.
- **Content kind**: one of `text`, `translatable`, `keybind`, `score`, `selector`, `nbt` and
  `object`.
- **Literal**: a content of the kind `text`. It holds one string.
- **Child**: a text in the list of children of a different text.
- **Style**: the set of style fields of a text. Each style field is absent or has a value.
- **Empty style**: a style in which each style field is absent.
- **Plain text**: a text with a literal, the empty style and no child.
- **Argument**: one element of the list `with` of a translation.
- **Named colour**: a colour that a name of TXT-STY-002 gives.
- **RGB colour**: a colour that a string `#RRGGBB` gives.
- **String form**, **list form**, **map form**: the three spellings of a text in the saved form
  (TXT-SAV-001).
- **Canonical form**: the one saved form that a writer gives for a text (section 3.3).
- **Tree format**: NBT, SNBT or JSON. A tree format carries a saved form.
- **Numeric tag**: a Byte, Short, Int, Long, Float or Double tag of NBT.

### 0.2 Provenance

- `black_box_confirmed`: shown by a result in section 12.
- `source_suggested_black_box_confirmed`: taken from the reference behaviour, and consistent with
  a result in section 12.
- `source_suggested`: taken from the reference behaviour, with no result. The confidence is at most
  medium. Section 11 attaches an experiment.
- `design`: a choice of this project. The rule states the reason.
- `unresolved`: not decided. Section 10 collects these items.

Each rule states its label and its kind (`README.md`) in the parentheses after its id.

### 0.3 Artefacts cited

- The fixtures `text_vanilla`, `text_probe`, `text_nbt` and `text_wire` of `tools/fixtures.json`.
  Each case has an input from this repository. The codecs of the game wrote each expected value
  and each error.
- The fixture `inventory_packets` of `tools/fixtures.json`, through the results of `item-stack.md`.
- `assets/mcrs/item_definition/`: one file for each item, which the game writes.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `TXT-MOD` | the parts of a text |
| `TXT-CNT` | the content kinds |
| `TXT-STY` | the style fields |
| `TXT-CLK` | the click event |
| `TXT-HOV` | the hover event |
| `TXT-SAV` | the saved form and the canonical form |
| `TXT-NBT` | NBT and the wire form |
| `TXT-JSN` | JSON |
| `TXT-EQ` | equality |
| `TXT-HSH` | the hash |

### 0.5 Notation

A byte string is in hexadecimal. An integer range `a..b` includes `a` and `b`. An int is a signed
32-bit integer. In a table of fields, the column "Absent" gives `necessary`, or the value that a
reader takes when the key is absent. A field that is not necessary is optional.

## 1. The model

**TXT-MOD-001** (`source_suggested_black_box_confirmed`, semantic). A text has these three parts:

- one content;
- one style;
- an ordered list of children. Each child is a text.

The list of children can be empty. The model sets no limit on the number of children, on the depth
of the children or on the length of a string. See result R1.

**TXT-MOD-002** (`source_suggested_black_box_confirmed`, semantic). A text has no parts other than
those of TXT-MOD-001. The text does not hold the spelling that a reader read. Thus the string form
`"x"` and the map form `{text:"x"}` give the same text. See result R2.

**TXT-MOD-003** (`source_suggested_black_box_confirmed`, semantic). The model keeps these
differences, because the canonical form shows them:

- a style field that is absent, and the same style field with a value (TXT-STY-001);
- a named colour, and the RGB colour with the same value (TXT-STY-002);
- the number kind of an argument (TXT-CNT-004).

See results R4 and R7.

**TXT-MOD-004** (`source_suggested`, semantic). The empty text is the plain text with the literal
`""`. Its canonical form is the string `""`. See result R3.

### 1.1 Content kinds

**TXT-CNT-001** (`source_suggested_black_box_confirmed`, wire). The content kind `text` has one
field:

| Field | Saved form | Absent | Range |
|-------|------------|--------|-------|
| `text` | string | necessary | each string |

A reader does not change the string. It accepts each character, also the section sign `U+00A7`
and a control character. A reader does not read a format code from the string. See result R1.

**TXT-CNT-002** (`source_suggested_black_box_confirmed`, wire). The content kind `translatable`
has these fields:

| Field | Saved form | Absent | Range |
|-------|------------|--------|-------|
| `translate` | string | necessary | each string: a translation key |
| `fallback` | string | no fallback | each string |
| `with` | a list of arguments | no arguments | each number of arguments |

A reader takes a `fallback` that is not a string as absent. It reports no error for it. A reader
takes an empty list `with` as absent. A reader does not check that a translation has the key. It
does not check the number of the arguments against the translation. See results R5 and R6.

**TXT-CNT-003** (`source_suggested_black_box_confirmed`, semantic). An argument is one of these:

- a string;
- a number of one number kind: byte, short, int, long, float or double;
- a boolean;
- a text that is not a plain text.

A reader gives a string argument for a string. For each other value, the reader tries a number or
a boolean first, then a text. When the text is a plain text, the reader gives a string argument
with the string of the literal. Thus the arguments `"a"` and `{text:"a"}` are the same argument.
See result R6.

**TXT-CNT-004** (`source_suggested_black_box_confirmed`, wire). The number kind of an argument
comes from the tree format:

| Tree format | Value | Argument |
|-------------|-------|----------|
| NBT, SNBT | a numeric tag | a number of the kind of the tag |
| NBT, SNBT | a Byte Array, Int Array or Long Array tag as one argument | an error |
| JSON | `true` or `false` | a boolean |
| JSON | a number | a number of the kind of TXT-JSN-003 |
| JSON | `null` | an error |

NBT has no boolean. A writer writes a boolean argument to NBT as the Byte tag `1b` or `0b`. A
reader of that NBT gives a byte argument. A writer writes a number argument with a tag of its
number kind. In NBT, a reader accepts an Int Array tag as the whole list `with`. Each element is
then an int argument. See results R6 and R7.

**TXT-CNT-005** (`source_suggested_black_box_confirmed`, wire). The content kind `keybind` has one
field:

| Field | Saved form | Absent | Range |
|-------|------------|--------|-------|
| `keybind` | string | necessary | each string: the name of a key of the client |

A reader does not check the name. See result R8.

**TXT-CNT-006** (`source_suggested_black_box_confirmed`, wire). The content kind `score` has one
field, `score`. It is necessary, and it is a map with these keys:

| Field | Saved form | Absent | Range |
|-------|------------|--------|-------|
| `name` | string | necessary | each string |
| `objective` | string | necessary | each string |

`name` is an entity selector or the name of a score holder. A reader accepts each string. A string
that is not a valid selector is the name of a score holder. A reader ignores each other key of the
map. See result R8.

**TXT-CNT-007** (`source_suggested_black_box_confirmed`, wire). The content kind `selector` has
these fields:

| Field | Saved form | Absent | Range |
|-------|------------|--------|-------|
| `selector` | string | necessary | a valid entity selector argument, with no character after it |
| `separator` | text | no separator | each text |

`argument-entity.md` states the grammar of the entity selector argument. A player name is a valid
argument. A `separator` that is not a valid text is an error of the field. See result R9.

**TXT-CNT-008** (`source_suggested_black_box_confirmed`, wire). The content kind `nbt` has these
fields:

| Field | Saved form | Absent | Range |
|-------|------------|--------|-------|
| `nbt` | string | necessary | a valid NBT path, with no character after it |
| `interpret` | boolean | `false` | `false` or `true` |
| `plain` | boolean | `false` | `false` or `true` |
| `separator` | text | no separator | each text |
| one source field | TXT-CNT-009 | necessary | TXT-CNT-009 |

A reader takes an `interpret`, a `plain` or a `separator` that has a wrong form as absent. It
reports no error for it. A content with `interpret` true and `plain` true is an error. This
document does not state the grammar of an NBT path (open question Q2). See result R10.

**TXT-CNT-009** (`source_suggested_black_box_confirmed`, wire). The source of an `nbt` content is
one of these, in this order:

| Source | Field | Saved form | Range |
|--------|-------|------------|-------|
| `entity` | `entity` | string | a valid entity selector argument, with no character after it |
| `block` | `block` | string | a valid block position argument, with no character after it |
| `storage` | `storage` | string | an identifier |

`argument-coordinates.md` states the grammar of the block position argument. The optional key
`source` holds the name of the source. When `source` is present, the reader reads only the field
of that source. When `source` is absent, the reader takes the first source of the table whose
field is valid. A content with no valid source is an error. A writer does not write `source`.
See result R10.

**TXT-CNT-010** (`source_suggested_black_box_confirmed`, wire). The content kind `object` has an
object and one more field:

| Field | Saved form | Absent | Range |
|-------|------------|--------|-------|
| `fallback` | text | no fallback | each text |

The object is one of these, in this order:

| Object | Field | Saved form | Absent | Range |
|--------|-------|------------|--------|-------|
| `atlas` | `atlas` | identifier | `minecraft:blocks` | each identifier |
| `atlas` | `sprite` | identifier | necessary | each identifier |
| `player` | `player` | a profile | necessary | TXT-CNT-011 |
| `player` | `hat` | boolean | `true` | `false` or `true` |

The optional key `object` holds the name of the object. When `object` is present, the reader reads
only the fields of that object. When `object` is absent, the reader takes the first object of the
table whose fields are valid. A writer does not write `object`. A reader does not check that the
atlas or the sprite exists. See result R11.

**TXT-CNT-011** (`black_box_confirmed`, wire). The profile of a `player` object is a string or a
map. The string is the name of a player. Its canonical form is the map `{name:"<string>"}`. This
document does not state the other keys of the map (open question Q3). See result R11.

### 1.2 Style

**TXT-STY-001** (`source_suggested_black_box_confirmed`, wire). The style has these fields. Each
field is optional. An absent field has no value: it is not equal to a default.

| Field | Saved form | Range |
|-------|------------|-------|
| `color` | string | TXT-STY-002 |
| `shadow_color` | int, or a list of 4 numbers | TXT-STY-003 |
| `bold` | boolean | `false` or `true` |
| `italic` | boolean | `false` or `true` |
| `underlined` | boolean | `false` or `true` |
| `strikethrough` | boolean | `false` or `true` |
| `obfuscated` | boolean | `false` or `true` |
| `click_event` | map | section 1.3 |
| `hover_event` | map | section 1.4 |
| `insertion` | string | each string |
| `font` | identifier | each identifier |

The style fields are keys of the map form of the text. They are not in a map of their own. A
style field with a wrong form is an error of the text. This is true for each style field. A
document for the client states how a child takes a style field that is absent. See results R4 and
R12.

**TXT-STY-002** (`source_suggested_black_box_confirmed`, wire). The value of `color` is a named
colour or an RGB colour. The names and their values are:

| Name | Value | Name | Value |
|------|-------|------|-------|
| `black` | `000000` | `dark_gray` | `555555` |
| `dark_blue` | `0000AA` | `blue` | `5555FF` |
| `dark_green` | `00AA00` | `green` | `55FF55` |
| `dark_aqua` | `00AAAA` | `aqua` | `55FFFF` |
| `dark_red` | `AA0000` | `red` | `FF5555` |
| `dark_purple` | `AA00AA` | `light_purple` | `FF55FF` |
| `gold` | `FFAA00` | `yellow` | `FFFF55` |
| `gray` | `AAAAAA` | `white` | `FFFFFF` |

A reader compares a name exactly. Thus `RED` and `reset` are errors. A string that starts with `#`
is an RGB colour. After the `#`, the string has 1 to 6 hexadecimal digits, in upper case or in
lower case. The digits give a number from `000000` to `FFFFFF`. A writer writes a named colour as
its name. A writer writes an RGB colour as `#` and 6 digits in upper case. A reader gives an RGB
colour for `#FF5555`. It does not give the named colour `red`. See result R4 and deviation D1.

**TXT-STY-003** (`source_suggested_black_box_confirmed`, wire). The value of `shadow_color` is an
int. Its bits are alpha, red, green and blue, 8 bits each, from the most significant byte. A
reader also accepts a list of 4 numbers `[red, green, blue, alpha]`. Each number is a float from
0.0 to 1.0. The channel is the largest integer that is not larger than the number times 255. A
writer always writes the int. For a number out of `0.0..1.0`, the result is not specified. A list
with a different number of elements is an error. See result R13.

**TXT-STY-004** (`source_suggested_black_box_confirmed`, wire). The value of `font` is an
identifier. A reader accepts a string without a namespace and gives the namespace `minecraft`. A
writer always writes the namespace. A reader does not check that the font exists. An explicit
`minecraft:default` is a value. It is not the absent field, and a writer writes it.
See result R12.

### 1.3 The click event

**TXT-CLK-001** (`source_suggested_black_box_confirmed`, wire). The click event is a map. The
necessary key `action` holds the name of the action. The fields of the action are keys of the same
map:

| Action | Field | Saved form | Absent | Range |
|--------|-------|------------|--------|-------|
| `open_url` | `url` | string | necessary | TXT-CLK-002 |
| `run_command` | `command` | string | necessary | TXT-CLK-003 |
| `suggest_command` | `command` | string | necessary | TXT-CLK-003 |
| `change_page` | `page` | int | necessary | `1..2147483647` |
| `copy_to_clipboard` | `value` | string | necessary | each string |
| `show_dialog` | `dialog` | string or map | necessary | TXT-CLK-004 |
| `custom` | `id` | identifier | necessary | each identifier |
| `custom` | `payload` | each value of the tree format | no payload | TXT-CLK-005 |

An `action` that is not in this table is an error. A reader ignores each other key of the map.
See result R14.

**TXT-CLK-002** (`source_suggested_black_box_confirmed`, semantic). The `url` is a valid URI
reference with a scheme. The scheme is `http` or `https`. The comparison of the scheme ignores the
case. A string with a character that a URI does not allow, for example a space, is an error. The
writer writes the string that the reader read. See result R14.

**TXT-CLK-003** (`source_suggested_black_box_confirmed`, semantic). A `command` is an error when
one of its UTF-16 code units is one of these:

- the section sign `U+00A7`;
- a code unit smaller than `U+0020`;
- `U+007F`.

The rule sets no limit on the length. See result R14.

**TXT-CLK-004** (`source_suggested_black_box_confirmed`, wire). The `dialog` is a string or a map.
The string is the identifier of an entry of the registry `minecraft:dialog`. An identifier that
names no entry is an error. The map is one dialog that the text holds. This document does not
state the fields of a dialog. A writer writes the form that the reader read. See result R14.

**TXT-CLK-005** (`source_suggested_black_box_confirmed`, wire). The `payload` is free NBT, as
`item-stack.md`, STK-TYP-004, uses the term. Each tag kind is valid, also a string, a number, a
list and an array. A reader of JSON gives each number the kind of TXT-JSN-003. A reader of JSON
gives the Byte tag `1b` or `0b` for a boolean. See result R15.

**TXT-CLK-006** (`source_suggested`, semantic). The game has one more action, `open_file`, with
the field `path`. The saved form does not carry it. A reader gives an error for `open_file`. A
writer gives an error for a text that holds it. See experiment X5.

### 1.4 The hover event

**TXT-HOV-001** (`source_suggested_black_box_confirmed`, wire). The hover event is a map. The
necessary key `action` holds the name of the action. The fields of the action are keys of the same
map:

| Action | Field | Saved form | Absent | Range |
|--------|-------|------------|--------|-------|
| `show_text` | `value` | text | necessary | each text |
| `show_item` | the keys of an item template | TXT-HOV-002 | | |
| `show_entity` | `id` | identifier | necessary | an entry of the registry `minecraft:entity_type` |
| `show_entity` | `uuid` | TXT-HOV-003 | necessary | each UUID |
| `show_entity` | `name` | text | no name | each text |

An `action` that is not in this table is an error. A reader ignores each other key of the map.
The reader of `id` accepts a string without a namespace. A hover event can hold a text that has a
hover event. See result R16.

**TXT-HOV-002** (`source_suggested_black_box_confirmed`, wire). The action `show_item` holds one
item template. Its keys `id`, `count` and `components` are keys of the map of the hover event. The
rules of `item-stack.md`, section 6.6, apply. Thus `count` has the default 1 and the range
`1..99`, and the item is not air. A writer omits a `count` of 1 and an empty patch. A component of
the patch can hold a text. See result R17.

**TXT-HOV-003** (`source_suggested_black_box_confirmed`, wire). The `uuid` has two saved forms:

- 4 ints. The first int is the most significant 32 bits of the UUID.
- A string of 36 characters in the form `8-4-4-4-12` of hexadecimal digits, in each case.

A writer writes the 4 ints: an Int Array tag in NBT, and an array of 4 numbers in JSON. In NBT, a
reader also accepts a list of 4 numeric tags. A string without the hyphens is an error. A number
of ints that is not 4 is an error. See result R16.

## 2. Which fields make a text valid

**TXT-MOD-005** (`source_suggested_black_box_confirmed`, semantic). A map form is a valid text
only when it has a valid content. A map with only style fields is an error. A map with only
`extra` is an error. The empty map is an error. See result R18.

## 3. The saved form

### 3.1 The three spellings

**TXT-SAV-001** (`source_suggested_black_box_confirmed`, wire). A reader of a text accepts these
three spellings, in each place where a text occurs:

| Spelling | Value | Text |
|----------|-------|------|
| string form | a string | the plain text with that string |
| list form | a list with one or more elements, each a text | TXT-SAV-002 |
| map form | a map | TXT-SAV-003 |

A text occurs at the root and in these fields: `extra`, `with`, `separator`, `fallback` of an
object, `value` of `show_text` and `name` of `show_entity`. Each other value is an error: a number, a
boolean, an empty list and a JSON `null`. See results R1, R2 and R18.

**TXT-SAV-002** (`source_suggested_black_box_confirmed`, semantic). The list form gives the text
of its first element. Each other element becomes a child of that text, in order, after the
children that the first element has. Thus `["a",{text:"b",bold:true},"c"]` and
`{text:"a",extra:[{text:"b",bold:true},"c"]}` give the same text. A list with one element gives
the text of that element. See result R19.

**TXT-SAV-003** (`source_suggested_black_box_confirmed`, wire). The map form has these keys:

| Key | Saved form | Absent |
|-----|------------|--------|
| `type` | string: a content kind | TXT-SAV-005 selects the content kind |
| the fields of the content kind | section 1.1 | as section 1.1 states |
| `extra` | a list with one or more elements, each a text | no children |
| the style fields | section 1.2 | absent |

An `extra` that is an empty list is an error of the text. An `extra` that is not a list is an
error of the text. See results R1 and R18.

**TXT-SAV-004** (`source_suggested_black_box_confirmed`, semantic). When `type` is present, the
reader reads only the fields of that content kind. It ignores the fields of each other content
kind. A `type` that names no content kind is an error. A necessary field of the content kind that
is absent is an error. See result R20.

**TXT-SAV-005** (`source_suggested_black_box_confirmed`, semantic). When `type` is absent, the
reader tries the content kinds in this order: `text`, `translatable`, `keybind`, `score`,
`selector`, `nbt`, `object`. It takes the first content kind whose fields are all valid. It
ignores the fields of each other content kind. A map in which no content kind is valid is an
error. For example:

| Map | Content |
|-----|---------|
| `{keybind:"k",text:"t"}` | the literal `t` |
| `{translate:"t",keybind:"k"}` | the translation `t` |
| `{text:5,translate:"t"}` | the translation `t` |
| `{text:5}` | an error |

See result R20 and experiment X6.

**TXT-SAV-006** (`source_suggested_black_box_confirmed`, semantic). A reader ignores a key that
this document does not list. This is true for the map form, for the map of `score`, for a click
event and for a hover event. For example, `{text:"a",junk:1}` gives the plain text `a`. The keys
`value` of an old click event and `contents` of an old hover event are such keys. The reader then
reports the necessary field that is absent. See results R14, R16 and R18.

### 3.2 Wrong forms

**TXT-SAV-007** (`source_suggested_black_box_confirmed`, semantic). A field with a wrong form has
one of two results:

| Field | Result |
|-------|--------|
| `fallback` of a translation, `interpret`, `plain`, `separator` of an `nbt` content | the reader takes the field as absent |
| each other field | an error |

An error in a field of a content kind makes that content kind not valid (TXT-SAV-005). An error
in `extra` or in a style field is an error of the text. An error in a child, in an argument or in
a text of an event is an error of the text that holds it. A reader of a text gives the whole text
or an error. It gives no text with a part removed. See results R5, R10 and R18.

**TXT-SAV-008** (`source_suggested_black_box_confirmed`, semantic). A string field accepts only a
string. A number is not a string. Thus `{text:5}` and `{insertion:5,text:"x"}` are errors in each
tree format. See result R18.

**TXT-SAV-009** (`source_suggested_black_box_confirmed`, semantic). In NBT and SNBT, a boolean
field accepts each numeric tag. The value is false when the number is zero, and true for each
other number. Thus `bold:2b` and `bold:1` are true. A String tag is an error. A writer writes the
Byte tag `1b` or `0b`. In JSON, TXT-JSN-002 applies. See result R21 and experiment X3.

**TXT-SAV-010** (`source_suggested_black_box_confirmed`, semantic). In NBT and SNBT, an int field
accepts each numeric tag. The reader converts the number to an int:

| Tag | Int |
|-----|-----|
| Byte, Short, Int | the value |
| Long | the least significant 32 bits |
| Float, Double | the number without its fraction, limited to `-2147483648..2147483647` |

The reader checks the range of the field after the conversion. Thus `page:3.7d` gives 3, and
`page:4294967297L` gives 1. A writer writes an Int tag. This rule applies to `page`, to
`shadow_color` and to `count` of an item template. Experiment X3 decides the result of a NaN. In JSON, TXT-JSN-004 applies.
See result R22 and experiment X3.

**TXT-SAV-011** (`design`, unspecified). A writer of this project must write an int field only as
an Int tag. It must write a boolean field only as the Byte tag `0b` or `1b`. Reason: the conversions of
TXT-SAV-009 and TXT-SAV-010 lose data, and no vanilla writer uses them.

### 3.3 The canonical form

**TXT-SAV-012** (`source_suggested_black_box_confirmed`, wire). A writer gives one saved form for
a text:

- For a plain text, the writer writes the string form.
- For each other text, the writer writes the map form.

The writer never writes the list form. This rule applies at the root and in each place where a
text occurs. Thus a child that is a plain text is a string in `extra`. A text with the literal
`""` and one or more children is a map with the key `text` and the value `""`. See results R2, R3
and R19.

**TXT-SAV-013** (`source_suggested_black_box_confirmed`, wire). The map form of the writer has
these keys:

- each necessary field of the content kind;
- each optional field of the content kind whose value is not equal to its default;
- `extra`, when the text has one or more children;
- each style field that has a value.

The writer does not write `type`, `source` or `object`. The writer writes a style field with the
value `false`. The writer does not write these fields with these values:

- `interpret` or `plain` with the value `false`;
- `hat` with the value `true`;
- `atlas` with the value `minecraft:blocks`;
- `with` with no arguments.
 See results R1,
R10, R11 and R20.

**TXT-SAV-014** (`black_box_confirmed`, wire). In a format that keeps the order of keys, the
writer writes the keys of the map form in this order:

1. The fields of the content kind, in the order of the rows of its table in section 1.1.
2. `extra`.
3. The style fields, in the order of the rows of the table of TXT-STY-001.

In an `nbt` content, the source field comes after `separator`. In an `object` content, `fallback`
comes after the fields of the object. In a click event and in a hover event, the fields of the
action come first, in the order of the rows. `action` comes last. This order applies to JSON.
See result R23.

**TXT-SAV-015** (`black_box_confirmed`, unspecified). The order of the keys of an NBT compound has
no meaning. A reader must accept each order. The order in which the game writes the keys of a
compound is an accident, and a parity test must not compare it. A test that compares bytes must
sort the keys, or compare the SNBT, which has the keys in the order of `snbt.md`. The hash does
not depend on the order (TXT-HSH-002). See result R23.

**TXT-SAV-016** (`source_suggested_black_box_confirmed`, semantic). A read and then a write gives
the canonical form. The canonical form of a canonical form is the same form. A read does not keep
these parts of the input:

- the spelling: string form, list form or map form;
- `type`, `source`, `object` and each key that the reader ignores;
- an optional field with a value equal to its default;
- the case and the number of the digits of an RGB colour;
- a `uuid` string, a `shadow_color` list and a profile string;
- an identifier without a namespace, which gets the namespace `minecraft`.

See results R2, R4, R12, R13 and R16.

## 4. NBT and the wire form

**TXT-NBT-001** (`source_suggested_black_box_confirmed`, wire). In NBT, the saved form uses these
tags:

| Saved form | Tag of the writer |
|------------|-------------------|
| string, identifier | String |
| boolean | Byte, `0b` or `1b` |
| int | Int |
| map | Compound |
| list | List (TXT-NBT-002) |
| `uuid` | Int Array with 4 elements |
| number argument | the tag of its number kind |
| `payload` | the tag that the text holds |

See results R1 and R24.

**TXT-NBT-002** (`source_suggested_black_box_confirmed`, wire). An NBT List tag holds elements of
one tag kind. A list of the saved form can hold elements of different kinds, for example a string
and a map in `extra`. The writer then writes a list of Compound tags:

- An element that is a Compound tag stays as it is, unless it is a wrapper.
- Each other element becomes a wrapper: a Compound tag with one entry, whose name is the empty
  string and whose value is the element.

A wrapper is a Compound tag with exactly one entry whose name is the empty string. A reader of a
list of Compound tags replaces each wrapper with its value. When each element of a list has the
same tag kind, the writer writes the elements without a wrapper. For example, `extra` of
`{text:"a",extra:["b",{text:"c",color:"blue"}]}` has the bytes
`0a 00000002 | 08 0000 0001 62 00 | 08 0005 636f6c6f72 0004 626c7565 08 0004 74657874 0001 63 00`.
This rule is a rule of NBT. It is here because no other document states it yet. See result R24 and
experiment X4.

**TXT-NBT-003** (`source_suggested_black_box_confirmed`, wire). The wire form of a text is one
network NBT of its canonical form. The plain text `plain` is `08 0005 706c61696e`. The empty text
is `08 0000`. The tag is a String tag or a Compound tag when a vanilla writer wrote it. A reader
accepts also a List tag (TXT-SAV-001). A tag of the kind End in the place of the text is a decode
error. See result R25.

**TXT-NBT-004** (`source_suggested_black_box_confirmed`, wire). The wire form has no difference
from the saved form in NBT. It has the same keys, the same tags and the same optional fields. A
reference to a registry entry is the identifier string, also in the wire form. This is true for
`id` of `show_item`, for `id` of `show_entity` and for `dialog`. A component of a `show_item`
patch has its saved form, not its wire form. See result R25.

**TXT-NBT-005** (`source_suggested`, wire). The reader of the wire form in an item component has
these limits:

| Limit | Value |
|-------|-------|
| depth of the NBT | 512 levels |
| size of the NBT | a quota of 2097152 bytes |

A tag past a limit is a decode error. The exact measure of the size is not specified. The same
limits apply to a text in each packet that does not state a different quota. A document for a
packet states the quota of that packet. The depth of a text counts each list and each compound.
Thus a chain of children uses 2 levels for each child. See experiment X2.

**TXT-NBT-006** (`source_suggested`, wire). The text sets no limit on the length of a string. NBT
does: a String tag holds at most 65535 bytes of its encoded form. A writer gives an error for a
longer string. JSON has no such limit. See experiment X2.

**TXT-NBT-007** (`source_suggested`, semantic). A saved form that is not a valid text is a decode
error in the wire form. The reader gives no text. A text that has no saved form is an error of
the writer (TXT-CLK-006). See experiment X5.

## 5. JSON

**TXT-JSN-001** (`source_suggested_black_box_confirmed`, wire). The JSON form of a text is the
saved form with these values:

| Saved form | JSON |
|------------|------|
| string, identifier | string |
| boolean | `true` or `false` |
| int | number |
| map | object |
| list | array |
| `uuid` | an array of 4 numbers |

The keys, the spellings, the content kinds and the canonical form are those of sections 1 and 3.
A data pack file and an item definition hold a text in this form. Each `minecraft:item_name` of
the 1686 files of `assets/mcrs/item_definition/` is an object with the one key `translate`.
See result R1.

**TXT-JSN-002** (`black_box_confirmed`, semantic). In JSON, a boolean field accepts only `true`
and `false`. A number and a string are errors. Thus `"bold":1` is an error in JSON, and `bold:1`
is true in NBT. For `interpret` and `plain`, the reader takes such a value as absent
(TXT-SAV-007). See result R21.

**TXT-JSN-003** (`source_suggested_black_box_confirmed`, wire). A JSON number has no number kind.
Where the saved form keeps a number kind, the reader of JSON gives the first kind of this list
that holds the number exactly:

1. For a number without a fraction: byte, short, int, long.
2. For each other number: float, then double.

A number such as `2.0` has no fraction, thus it is a byte. This rule applies to a number argument
(TXT-CNT-004) and to each number in a `payload` (TXT-CLK-005). For example, the argument `5` in
JSON is the byte 5, and its NBT is `5b`. See results R7 and R15.

**TXT-JSN-004** (`black_box_confirmed`, semantic). In JSON, an int field accepts each number. The
reader removes the fraction. It keeps the least significant 32 bits of an integer that is out of
the int range. Thus `"page":3.5` gives 3, and `"shadow_color":4294967295` gives -1. The reader
checks the range of the field after the conversion. A string is an error. See result R22.

**TXT-JSN-005** (`black_box_confirmed`, wire). A JSON array has no kinds, thus TXT-NBT-002 does
not apply to JSON. A `payload` that has an NBT array is a JSON array of numbers. A read of that
JSON gives a List tag, not the array. Thus JSON does not keep each `payload`. See result R15.

**TXT-JSN-006** (`source_suggested`, semantic). The JSON form has no other difference from the
saved form in NBT. In SNBT, the saved form is the same tree in the syntax of `snbt.md`.

## 6. Equality

**TXT-EQ-001** (`source_suggested`, semantic). Two texts are equal when these three conditions are
true:

- their contents are equal (TXT-EQ-003);
- their styles are equal (TXT-EQ-004);
- they have the same number of children, and each child is equal to the child at the same
  position.

The order of the children has a meaning. Equality compares the decoded values. It does not compare
a saved form, a wire form or a hash. See experiment X1.

**TXT-EQ-002** (`source_suggested_black_box_confirmed`, semantic). Thus two saved forms that give
the same text give equal texts. These pairs are equal:

- `"x"`, `{text:"x"}`, `["x"]` and `{type:"text",text:"x"}`;
- `["a","b"]` and `{text:"a",extra:["b"]}`;
- a form with an optional field equal to its default, and the form without that field.

The game writes one canonical form for the forms of each pair. See results R2 and R19, and
experiment X1.

**TXT-EQ-003** (`source_suggested`, semantic). Two contents are equal when they have the same
content kind and each field is equal. These statements apply:

- Two strings are equal when they have the same code units.
- A selector, an NBT path and a block position are equal as strings. `@a[limit=1]` and
  `@a[ limit = 1 ]` are not equal.
- Two lists of arguments are equal when they have the same length and equal arguments in order.
- A string argument is equal only to a string argument. A boolean argument is equal only to a
  boolean argument.
- Two number arguments are equal when they have the same number kind and the same value. The byte
  5 and the int 5 are not equal. Two floats or two doubles are equal as STK-EQ-005 states.
- An absent `fallback` and the fallback `""` are not equal.
- The `name` of a `score` content compares as a string.

See experiment X1.

**TXT-EQ-004** (`source_suggested`, semantic). Two styles are equal when each style field is equal
in the two styles. These statements apply:

- An absent style field is equal only to an absent style field. `bold:false` and no `bold` are
  not equal.
- Two colours are equal when their values are equal. Thus the named colour `red` and the RGB
  colour `#FF5555` are equal.
- Two click events are equal when they have the same action and equal fields. Two `payload` values
  are equal as STK-EQ-006 states.
- Two hover events are equal when they have the same action and equal fields. Two item templates
  are equal as `item-stack.md` states. Two UUIDs are equal when their 128 bits are equal.
- An absent `font` and the font `minecraft:default` are not equal.

See experiment X1.

**TXT-EQ-005** (`source_suggested`, semantic). Equal texts can have different canonical forms.
This occurs only for a named colour and the RGB colour with the same value. Such texts have
different wire forms and different hashes. An implementation must keep the difference in the
model (TXT-MOD-003) and must ignore it in equality. See result R4 and experiment X1.

**TXT-EQ-006** (`source_suggested`, semantic). The merge of two stacks uses this equality. The
relation "same item and components" of STK-EQ-001 compares a component that holds a text with
TXT-EQ-001. Thus a stack with the name `"x"` and a stack with the name `{text:"x"}` can merge. A
stack with a `red` name and a stack with a `#FF5555` name can merge. A list of texts, for example
`minecraft:lore`, is equal when it has equal texts in the same order. See experiment X1.

**TXT-EQ-007** (`source_suggested`, semantic). Text equality answers the question of U9 of
`item-stack.md`: the string `"x"` and the map `{text:"x"}` decode to equal texts. They also have
one saved form, one wire form and one hash. See result R2.

## 7. The hash

**TXT-HSH-001** (`source_suggested_black_box_confirmed`, client agreement). The hash of a text is
the hash of its canonical form, with the node kinds of STK-HSH-005. A component that holds a text
uses this hash in the hashed stack (STK-HSH-004). The plain text `named` is one string node, and
its hash is `d2ee6d5e`. See result R26.

**TXT-HSH-002** (`source_suggested_black_box_confirmed`, client agreement). The map form is a map
node. Each key is a string node. STK-HSH-006 sorts the entries by the hash of the key. Thus the
order of the keys of the writer (TXT-SAV-014) does not change the hash. Only the set of the keys
and their values change it. See result R26.

**TXT-HSH-003** (`source_suggested`, client agreement). The parts of the canonical form have
these node kinds:

| Part | Node |
|------|------|
| plain text, string field, identifier, `color` | string |
| `bold`, `italic`, `underlined`, `strikethrough`, `obfuscated`, `interpret`, `plain`, `hat` | boolean |
| `shadow_color`, `page`, `count` | int |
| `extra`, `with` | list |
| a map form, `score`, a click event, a hover event | map |
| `uuid` | int array with 4 elements |
| string argument | string |
| number argument | the number node of its number kind |
| boolean argument | boolean |
| `payload` | the node of each tag: a Byte tag is a byte node, a Compound tag is a map node, an array tag is an array node |
| `components` of `show_item` | as STK-HSH-004 states for the saved form of each component |

A list node has no wrapper: TXT-NBT-002 applies only to NBT. A boolean of a style field is a
boolean node, not a byte node. See experiment X7.

**TXT-HSH-004** (`source_suggested`, client agreement). The hash has an entry only for a key that
the canonical form has. An implementation must omit each field that TXT-SAV-013 omits. For
example, a `bold:false` is an entry, and an `interpret:false` is not. A text in the list form or
with a `type` key has the hash of its canonical form. See experiment X7.

**TXT-HSH-005** (`source_suggested`, client agreement). A `minecraft:lore` component is a list
node whose elements are the hashes of its texts in order. `minecraft:custom_name` and
`minecraft:item_name` are the node of the one text. The `override` display of an attribute
modifier holds one text in its key `value`. See experiment X7.

## 8. Boundaries

This document does not state these subjects:

- Rendering: fonts, glyphs, the width of a text, the shadow, and how a child takes the style of
  its parent. A document for the client states them.
- The text that a translation, a keybind, a score, a selector or an `nbt` content gives. A
  document for commands states the resolve step of the server. A document for the client states
  the translation tables and the format of a translation.
- The legacy format codes of the section sign. A reader keeps the section sign as a character of
  the literal (TXT-CNT-001).
- The chat packets, signed messages and chat types.
- The limit on the JSON length of a text that some values apply, for example a page of a book. The
  document of each such value states it.
- The limit of a menu that renames a stack, and the text limits of a sign (issue #219).
- The fields of a dialog, of a profile and of an item component. `item-stack.md` states the item
  template.
- The grammars of the entity selector, the block position and the NBT path.
- The quota of each packet that carries a text, and the optional text of a packet.
- The string of Bedrock Edition that an edge makes from a text (issue #666).

## 9. Intentional deviations

**D1.**

- **Rule**: TXT-STY-002.
- **Reference**: after the `#`, the reference reader accepts a sign, for example `#+ff`. It also
  accepts more than 6 digits when the number is not larger than `FFFFFF`, and digits that are not
  ASCII.
- **Visible to the client**: no. The vanilla writer writes `#` and 6 ASCII digits.
- **Test**: a differential test must not send a sign, more than 6 digits or a digit that is not
  ASCII. The fixture `text_probe` has the case `color_plus_hex`; a parity test must skip it.

## 10. Open questions

For the owner:

- **Q1**: TXT-CNT-007, TXT-CNT-009. The reader of a text must parse an entity selector, a block
  position and an NBT path to accept or reject a text. These grammars are above the wire layer.
  Can a reader below them keep the string and leave the check to a later step? Such a reader
  accepts texts that the game rejects.
- **Q2**: TXT-CNT-008. Which document owns the grammar of an NBT path?
- **Q3**: TXT-CNT-011. Which document owns the profile of a player, with its keys `name`, `id` and
  `properties` and its skin fields?
- **Q4**: TXT-CLK-004. Which document owns the fields of a dialog that a text holds?
- **Q5**: TXT-NBT-002. The wrapper rule is a rule of NBT. Does it move to a document for NBT?
- **Q6**: TXT-EQ-005. `docs/text-item-hover.md` keeps the components of a `show_item` hover as
  NBT below the item crate. Equality of two such hovers then compares NBT, not components. The
  owner decides if this difference from TXT-EQ-004 is acceptable below the item crate.

Not decided by an experiment yet:

- **U1**: TXT-NBT-005. The exact measure of the NBT quota. Experiment X2.
- **U2**: TXT-SAV-005. The result of a `type` key that is not a string. Experiment X6.
- **U3**: TXT-SAV-009, TXT-SAV-010. The result of a NaN, of an infinity and of a fraction smaller
  than 1 in a boolean field and in an int field. Experiment X3.

## 11. Experiments

Each experiment uses the game version of the table at the top of this document. An experiment of
the kind "golden" adds cases to a fixture of `tools/fixtures.json`. Run it with
`cargo run -p mcrs_minecraft_update -- recapture <name>`. The column "Case kind" states if the
fixture has the case kind now. No experiment of this table has a recorded run.

| Experiment | Kind | Case kind | Steps | Record |
|------------|------|-----------|-------|--------|
| X1 | golden | new: a comparison of two decoded values | Decode each pair: `"x"` and `{text:"x"}`; `{text:"x",color:"red"}` and `{text:"x",color:"#FF5555"}`; `{text:"x",bold:false}` and `"x"`; `{translate:"k",with:[5b]}` and `{translate:"k",with:[5]}`; `{text:"x",font:"minecraft:default"}` and `"x"`; `{text:"a",extra:["b","c"]}` and `{text:"a",extra:["c","b"]}`. Compare the two texts. Then compare two stacks with each pair as `custom_name`. | Which pairs are equal; which stacks have the same item and components. |
| X2 | golden | new: a decode of chosen wire bytes of a component | Decode `custom_name` from a network NBT with 511, 512 and 513 levels of `extra`. Decode a String tag of 65535 bytes. Decode a text whose NBT has 2097152 bytes and one with 2097153 bytes. Encode a literal of 65536 bytes. | The error or the text of each case; the size at which the quota fails. |
| X3 | golden | existing: `text_nbt` | Add cases: `bold` as `0.5f`, `0.0d`, `NaNf` and `-0.0f`; `page` as `NaNd`, `Infinityd` and `0.9f`; `shadow_color` as `-1.5d`. | The canonical form or the error of each case. |
| X4 | golden | existing: `text_nbt` | Add cases: `extra` with a wrapper around a compound; `with` with a wrapper around a wrapper; a `payload` list that holds a compound with the one key `""` and a string. Encode each result to the wire form. | The JSON, the SNBT and the bytes of each case. |
| X5 | golden | existing: `text_probe`, and new: an encode of a text that code builds | Decode a click event with the action `open_file` and a `path`. Build a text with that action in code and encode it to the saved form and to the wire form. | The error of each case. |
| X6 | golden | existing: `text_probe` | Add cases: `{"type":5,"text":"x"}`; `{"text":5,"translate":"t"}`; `{"selector":"@a","separator":5}`; `{"translate":"k","with":[null]}`; `{"text":"x","extra":"y"}`. | The canonical form or the error of each case. |
| X7 | golden | existing: the case kind of `item_nested` with a patch | Add stacks with `custom_name` as: a text with each style field; a translation with the arguments `5b`, `5`, `1.5f`, `"s"` and a styled text; a text with `show_entity`; a text with a `custom` click event whose `payload` has a Byte tag, a list and an Int Array tag. Add `lore` with a plain text and a styled text. | The saved form, the wire bytes and the hash of each component. |
| X8 | server | none | Give two stacks whose `custom_name` are `"x"` and `{text:"x"}`. Give two whose names are `red` and `#FF5555`. Move one on the other with a click. Capture the `container_click` packet of a vanilla client for each stack. | If the stacks merge; the hash of each `custom_name`. |

## 12. Captured results

Each result is a set of cases of a fixture that the game wrote. A case name is a name in the file
of the fixture. "→" separates the input from the canonical form that the game wrote.

| Result | Fixture | Cases | What they show |
|--------|---------|-------|----------------|
| R1 | `text_vanilla` | `bare_literal`, `extra`, `literal_with_style` | `"hello"` → `"hello"`, NBT `08 0005 68656c6c6f`. `{text:"a",extra:["b",{text:"c",color:"blue"}]}` stays as it is. |
| R2 | `text_vanilla` | `literal_object_collapses`, `typed_text` | `{"text":"hello"}` → `"hello"`, with the NBT of `bare_literal`. `{"type":"text","text":"typed"}` → `"typed"`. |
| R3 | `text_vanilla`, `text_wire` | `empty_literal`, `empty_component`, `empty_appended` | `{"text":""}` → `""`, NBT `08 0000`. A literal `""` with two children → `{"text":"","extra":[...]}`. |
| R4 | `text_probe`, `text_vanilla` | `color_hex_equals_named`, `color_hex_lower`, `color_short_hex`, `color_reset`, `color_bad`, `hex_color_and_custom_font` | `#FF5555` → `#FF5555`, not `red`. `#abcdef` → `#ABCDEF`. `#f80` → `#000F80`. `reset` and `RED` are errors. |
| R5 | `text_probe` | `with_empty`, `translate_fallback_int` | `"with":[]` → no `with`. `"fallback":5` → no `fallback`, and no error. |
| R6 | `text_vanilla`, `text_probe`, `text_nbt` | `translatable_heterogeneous_args`, `with_nested_list_arg`, `with_byte_array_arg`, `with_int_array` | The argument `{"text":"plain"}` → `"plain"`. The argument `["a","b"]` → `{"text":"a","extra":["b"]}`. A Byte Array tag as an argument is an error. `with:[I;1,2]` → the int arguments 1 and 2. |
| R7 | `text_probe`, `text_nbt`, `text_wire` | `with_int`, `with_bool_float_string`, `with_long_arg`, `mixed_args` | JSON `[5]` → SNBT `[5b]`. JSON `[true,1.5,"s",2.0]` → SNBT `[1b,1.5f,"s",2b]`. `7L` stays a long. An int argument 5 has the wire bytes `03 00000005`. |
| R8 | `text_vanilla`, `text_probe`, `text_nbt` | `keybind`, `score`, `score_bad_selector`, `score_extra_value`, `keybind_byte` | `{"score":{"name":"@zzz","objective":"o"}}` is valid. The key `value` of `score` is ignored. `{keybind:1b}` is an error. |
| R9 | `text_vanilla`, `text_probe` | `selector`, `selector_bad`, `selector_bad_syntax` | `"notaselector"` is valid. `"@e[type="` is an error. |
| R10 | `text_vanilla`, `text_probe`, `text_nbt` | `nbt_entity`, `nbt_typed_source`, `nbt_two_sources`, `nbt_typed_source_conflict`, `nbt_no_source`, `nbt_bad_path`, `nbt_interpret_string`, `nbt_separator_bad`, `interpret_plain` | `entity` wins over `block`. `"source":"block"` selects `block`. No source, the path `[[`, and `interpret` with `plain` are errors. `"interpret":"yes"` and `"separator":5` are taken as absent. |
| R11 | `text_vanilla`, `text_probe`, `text_nbt` | `object_atlas`, `object_atlas_default_explicit`, `object_player_bare_name`, `object_player_nohat`, `object_player_and_sprite`, `object_atlas_only`, `object_typed_bad` | `{"object":"atlas","sprite":...}` → `{"sprite":...}`. `"atlas":"minecraft:blocks"` is omitted. `{"player":"Steve"}` → `{"player":{"name":"Steve"}}`. `sprite` wins over `player`. An `atlas` with no `sprite` is an error. |
| R12 | `text_probe`, `text_nbt` | `font_no_ns`, `font_default`, `font_bad`, `insertion_int` | `"font":"alt"` → `minecraft:alt`. `minecraft:default` stays. `Bad Font` and `insertion:5` are errors. |
| R13 | `text_vanilla`, `text_probe`, `text_nbt` | `shadow_color_int`, `shadow_color_floats`, `shadow_floats_3`, `shadow_list_int` | `[1.0,0.5,0.25,0.75]` → -1073774785, which is `BFFF7F3F`. `[1,0,0,1]` → -65536. A list of 3 numbers is an error. |
| R14 | `text_vanilla`, `text_probe` | the cases `click_*` | Each action of TXT-CLK-001 stays as it is. `ftp://`, a URL with a space, and a URL with no scheme are errors. A `command` with `§` is an error. A `page` string is an error. An unknown dialog is an error. An inline dialog stays. The key `value` in `run_command` is ignored, and `value` with no `command` is an error. |
| R15 | `text_probe`, `text_nbt` | the cases `click_custom_payload_*` and `custom_payload_*` | JSON payload `[1,2,3]` → `[1b,2b,3b]`; `true` → `1b`; `1.5` → `1.5f`; `3000000000` → a long; `1.1` → a double. An Int Array payload → JSON `[1,2]`. |
| R16 | `text_vanilla`, `text_probe`, `text_nbt` | the cases `hover_show_text*`, `hover_show_entity*`, `uuid_*`, `hover_legacy_contents`, `hover_extra_key` | The `uuid` string → 4 ints, `[I;1,131075,262144,5]` in SNBT. An upper case string is valid. A string without hyphens and 3 ints are errors. `"id":"pig"` → `minecraft:pig`. The key `contents` with no `value` is an error. |
| R17 | `text_vanilla`, `text_probe`, `text_nbt` | the cases `hover_show_item_*` and `show_item_*` | `count` 1 and an empty `components` are omitted. `count` 0 and 100, air and an unknown item are errors. `count:3b` and `count:3L` → 3. `max_stack_size:16b` → 16. A `custom_name` and a `lore` in the patch keep their canonical form. |
| R18 | `text_probe`, `text_nbt` | `style_only`, `empty_object`, `extra_only`, `text_number`, `text_int`, `extra_empty`, `extra_int_list`, `unknown_top_key`, `literal_with_empty_extra_style` | `{"bold":true}`, `{}`, `{"extra":["a"]}`, `{"text":5}` and an empty `extra` are errors, also in a child. `{"text":"a","junk":1}` → `"a"`. |
| R19 | `text_vanilla`, `text_probe`, `text_nbt` | `list_form`, `list_nested`, `list_single`, `list_single_styled`, `list_first_styled`, `list_root`, `deep_extra_collapse` | `["a",{"text":"b","bold":true},"c"]` → `{"text":"a","extra":[{"text":"b","bold":true},"c"]}`. `[["a","b"],"c"]` → `{"text":"a","extra":["b","c"]}`. `["only"]` → `"only"`. A child `{"text":"c"}` → `"c"`. |
| R20 | `text_vanilla`, `text_probe` | `typed_keybind`, `typed_translatable_with_text`, `typed_unknown`, `typed_translatable_missing`, `keybind_and_text`, `translate_and_keybind` | With `type`, the key `text` is ignored. `"type":"bogus"` is an error. `"type":"translatable"` with no `translate` is an error. `{"keybind":"k","text":"t"}` → `"t"`. |
| R21 | `text_nbt`, `text_probe` | `bold_2b`, `bold_int`, `bold_string`, `interpret_int`, `object_hat_byte`, `nbt_interpret_int` | NBT `bold:2b` and `bold:1` → `1b`. A string is an error in each format. JSON `"bold":1` is an error. JSON `"interpret":1` is taken as absent. |
| R22 | `text_nbt`, `text_probe` | `page_byte`, `page_double_frac`, `page_double_big`, `page_long_big`, `shadow_long`, `shadow_float`, `shadow_big`, `click_change_page_float_frac`, `click_change_page_big`, `click_change_page_string` | NBT `3.7d` → 3; `3.0E9d` → 2147483647; `4294967297L` → 1; `1.9f` → 1. JSON `3.5` → 3; `4294967295` → -1; `2147483648` → -2147483648, which is out of the range of `page`. |
| R23 | `text_probe`, `text_vanilla`, `text_wire` | `text_and_extra_style`, `nbt_entity`, `hover_show_text`, `literal_rgb` | JSON key order: `text`, `extra`, `color`. `nbt`, `interpret`, `separator`, `entity`. `value`, `action`. The NBT of `literal_rgb` has `color` before `text`, and the NBT of `hover_show_text_bare` has `text` before `hover_event`. |
| R24 | `text_vanilla`, `text_wire` | `extra`, `mixed_args`, `kill_multiple` | The bytes of TXT-NBT-002. A list of arguments of one kind has no wrapper: `09 0004 77697468 03 00000001 00000005`. |
| R25 | `text_wire` | `literal_plain`, `literal_rgb`, `show_item`, `show_dialog_inline`, `empty_component` | `"plain"` → `08 0005 706c61696e`. `{"text":"x","color":"#FF5555"}` → `0a 08 0005 636f6c6f72 0007 23464635353535 08 0004 74657874 0001 78 00`. The wire form of `show_item` has the identifier string of the item. |
| R26 | `inventory_packets` | `container_click` | Results R3 and R6 of `item-stack.md`: the hash of `custom_name` `"named"` is `d2ee6d5e`, the hash of the string node `named`. |
