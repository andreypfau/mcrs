# Text component: behavioural specification

This document states the text component of the game as data: its parts, its saved form, its wire
form, its equality and its hash. It also states what the client draws for a text, and the plain
string of a text. It has the depth that the codecs of the item components need. It does not
prescribe an architecture.

Scope: the value, its formats and its display. Section 9 names each subject that this document
does not state.

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
- **Piece**: one string that a text gives to a consumer, with the effective style of that string.
- **Expansion**: the ordered list of the pieces of a text (TXT-DSP-001).
- **Effective style**: the style of a piece, after the style of each ancestor applies (TXT-DSP-013).
- **Drawn form**: the characters that the client draws for a text. Each character has a style.
- **Plain string**: the string that the game makes from a text when it needs a string, for example
  for a length check (TXT-DSP-018).
- **Language table**: the map from a translation key to a template in the active language of the
  process that expands a text.
- **Template**: a string of a language table, or a `fallback`, that holds placeholders
  (TXT-DSP-004).
- **Key mapping**: a named input action of the client, for example `key.jump`.
- **Unresolved content**: a content of the kind `score`, `selector` or `nbt` that reaches the
  expansion.

### 0.2 Provenance

- `black_box_confirmed`: shown by a game-produced artefact that this document cites by path.
- `source_suggested_black_box_confirmed`: taken from the reference behaviour, and consistent with
  an artefact that this document cites by path.
- `source_suggested`: taken from the reference behaviour, with no artefact. The confidence is at
  most medium.
- `design`: a choice of this project. The rule states the reason.
- `unresolved`: not decided. Section 11 collects these items.

Each rule states its label and its kind (`README.md`) in the parentheses after its id. A rule with
a label that has `black_box_confirmed` in it also names its artefact there.

### 0.3 Artefacts cited

- The fixtures `text_vanilla`, `text_probe`, `text_nbt` and `text_wire` of `tools/fixtures.json`.
  Each case has an input from this repository. The codecs of the game wrote each expected value
  and each error.
- The fixture `inventory_packets` of `tools/fixtures.json`, through the results of `item-stack.md`.
- `assets/mcrs/item_definition/`: one file for each item, which the game writes.
- `docs/spec/fixtures/text-component/e1-chat.txt`: the chat lines of the vanilla client for
  experiment E1, each after its command.
- `docs/spec/fixtures/text-component/e2-sign.txt` and `e2-sign.png`: the commands of experiment E2
  and a screenshot of the vanilla client with two signs.
- `docs/spec/fixtures/text-component/e3-dialog.txt`, `e3-dialog.png` and `e3-hotbar.png`: the
  commands of experiment E3 and two screenshots of the vanilla client.
- `docs/spec/fixtures/text-component/e4-server-string.txt`: the lines of the vanilla server and of
  the vanilla client for experiment E4, each after its command. The folder
  `docs/spec/fixtures/text-component/pack-json/` holds the data pack that two cases use.
- `docs/spec/fixtures/text-component/e5-limits.txt`: the chat lines of the vanilla client for
  experiment E5, each after its command and each reduced to its length and its characters.
- `docs/spec/fixtures/text-component/e6-chat-display.txt`, `e6-style.png`, `e6-style-2.png`,
  `e6-codes-1.png`, `e6-codes-2.png` and `e6-objects.png`: the commands of experiment E6 and five
  screenshots of the chat of the vanilla client.
- `docs/spec/fixtures/text-component/e7-keybind.txt`: the lines of the vanilla client and of the
  vanilla server for experiment E7, each after its command.

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
| `TXT-DSP` | what a text shows: the expansion, the drawn form and the plain string |

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
of the children or on the length of a string.

**TXT-MOD-002** (`source_suggested_black_box_confirmed`, semantic). A text has no parts other than
those of TXT-MOD-001. The text does not hold the spelling that a reader read. Thus the string form
`"x"` and the map form `{text:"x"}` give the same text.

**TXT-MOD-003** (`source_suggested_black_box_confirmed`, semantic). The model keeps these
differences, because the canonical form shows them:

- a style field that is absent, and the same style field with a value (TXT-STY-001);
- a named colour, and the RGB colour with the same value (TXT-STY-002);
- the number kind of an argument (TXT-CNT-004).

**TXT-MOD-004** (`source_suggested`, semantic). The empty text is the plain text with the literal
`""`. Its canonical form is the string `""`.

### 1.1 Content kinds

**TXT-CNT-001** (`source_suggested_black_box_confirmed`, wire). The content kind `text` has one
field:

| Field | Saved form | Absent | Range |
|-------|------------|--------|-------|
| `text` | string | necessary | each string |

A reader does not change the string. It accepts each character, also the section sign `U+00A7`
and a control character. A reader does not read a format code from the string.

**TXT-CNT-002** (`source_suggested_black_box_confirmed`, wire). The content kind `translatable`
has these fields:

| Field | Saved form | Absent | Range |
|-------|------------|--------|-------|
| `translate` | string | necessary | each string: a translation key |
| `fallback` | string | no fallback | each string |
| `with` | a list of arguments | no arguments | each number of arguments |

A reader takes a `fallback` that is not a string as absent. It reports no error for it. A reader
takes an empty list `with` as absent. A reader does not check that a translation has the key. It
does not check the number of the arguments against the translation.

**TXT-CNT-003** (`source_suggested_black_box_confirmed`, semantic). An argument is one of these:

- a string;
- a number of one number kind: byte, short, int, long, float or double;
- a boolean;
- a text that is not a plain text.

A reader gives a string argument for a string. For each other value, the reader tries a number or
a boolean first, then a text. When the text is a plain text, the reader gives a string argument
with the string of the literal. Thus the arguments `"a"` and `{text:"a"}` are the same argument.

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
then an int argument.

**TXT-CNT-005** (`source_suggested_black_box_confirmed`, wire). The content kind `keybind` has one
field:

| Field | Saved form | Absent | Range |
|-------|------------|--------|-------|
| `keybind` | string | necessary | each string: the name of a key of the client |

A reader does not check the name.

**TXT-CNT-006** (`source_suggested_black_box_confirmed`, wire). The content kind `score` has one
field, `score`. It is necessary, and it is a map with these keys:

| Field | Saved form | Absent | Range |
|-------|------------|--------|-------|
| `name` | string | necessary | each string |
| `objective` | string | necessary | each string |

`name` is an entity selector or the name of a score holder. A reader accepts each string. A string
that is not a valid selector is the name of a score holder. A reader ignores each other key of the
map.

**TXT-CNT-007** (`source_suggested_black_box_confirmed`, wire). The content kind `selector` has
these fields:

| Field | Saved form | Absent | Range |
|-------|------------|--------|-------|
| `selector` | string | necessary | a valid entity selector argument, with no character after it |
| `separator` | text | no separator | each text |

`argument-entity.md` states the grammar of the entity selector argument. A player name is a valid
argument. A `separator` that is not a valid text is an error of the field.

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
document does not state the grammar of an NBT path (open question Q2).

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
atlas or the sprite exists.

**TXT-CNT-011** (`black_box_confirmed`, wire). The profile of a `player` object is a string or a
map. The string is the name of a player. Its canonical form is the map `{name:"<string>"}`. This
document does not state the other keys of the map (open question Q3).

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
style field with a wrong form is an error of the text. This is true for each style field.
TXT-DSP-013 states how a child takes a style field that is absent.

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
colour for `#FF5555`. It does not give the named colour `red`. See deviation D1.

**TXT-STY-003** (`source_suggested_black_box_confirmed`, wire). The value of `shadow_color` is an
int. Its bits are alpha, red, green and blue, 8 bits each, from the most significant byte. A
reader also accepts a list of 4 numbers `[red, green, blue, alpha]`. Each number is a float from
0.0 to 1.0. The channel is the largest integer that is not larger than the number times 255. A
writer always writes the int. For a number out of `0.0..1.0`, the result is not specified. A list
with a different number of elements is an error.

**TXT-STY-004** (`source_suggested_black_box_confirmed`, wire). The value of `font` is an
identifier. A reader accepts a string without a namespace and gives the namespace `minecraft`. A
writer always writes the namespace. A reader does not check that the font exists. An explicit
`minecraft:default` is a value. It is not the absent field, and a writer writes it.

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

**TXT-CLK-002** (`source_suggested_black_box_confirmed`, semantic). The `url` is a valid URI
reference with a scheme. The scheme is `http` or `https`. The comparison of the scheme ignores the
case. A string with a character that a URI does not allow, for example a space, is an error. The
writer writes the string that the reader read.

**TXT-CLK-003** (`source_suggested_black_box_confirmed`, semantic). A `command` is an error when
one of its UTF-16 code units is one of these:

- the section sign `U+00A7`;
- a code unit smaller than `U+0020`;
- `U+007F`.

The rule sets no limit on the length.

**TXT-CLK-004** (`source_suggested_black_box_confirmed`, wire). The `dialog` is a string or a map.
The string is the identifier of an entry of the registry `minecraft:dialog`. An identifier that
names no entry is an error. The map is one dialog that the text holds. This document does not
state the fields of a dialog. A writer writes the form that the reader read.

**TXT-CLK-005** (`source_suggested_black_box_confirmed`, wire). The `payload` is free NBT, as
`item-stack.md`, STK-TYP-004, uses the term. Each tag kind is valid, also a string, a number, a
list and an array. A reader of JSON gives each number the kind of TXT-JSN-003. A reader of JSON
gives the Byte tag `1b` or `0b` for a boolean.

**TXT-CLK-006** (`source_suggested`, semantic). The game has one more action, `open_file`, with
the field `path`. The saved form does not carry it. A reader gives an error for `open_file`. A
writer gives an error for a text that holds it.

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
hover event.

**TXT-HOV-002** (`source_suggested_black_box_confirmed`, wire). The action `show_item` holds one
item template. Its keys `id`, `count` and `components` are keys of the map of the hover event. The
rules of `item-stack.md`, section 6.6, apply. Thus `count` has the default 1 and the range
`1..99`, and the item is not air. A writer omits a `count` of 1 and an empty patch. A component of
the patch can hold a text.

**TXT-HOV-003** (`source_suggested_black_box_confirmed`, wire). The `uuid` has two saved forms:

- 4 ints. The first int is the most significant 32 bits of the UUID.
- A string of 36 characters in the form `8-4-4-4-12` of hexadecimal digits, in each case.

A writer writes the 4 ints: an Int Array tag in NBT, and an array of 4 numbers in JSON. In NBT, a
reader also accepts a list of 4 numeric tags. A string without the hyphens is an error. A number
of ints that is not 4 is an error.

## 2. Which fields make a text valid

**TXT-MOD-005** (`source_suggested_black_box_confirmed`, semantic). A map form is a valid text
only when it has a valid content. A map with only style fields is an error. A map with only
`extra` is an error. The empty map is an error.

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
boolean, an empty list and a JSON `null`.

**TXT-SAV-002** (`source_suggested_black_box_confirmed`, semantic). The list form gives the text
of its first element. Each other element becomes a child of that text, in order, after the
children that the first element has. Thus `["a",{text:"b",bold:true},"c"]` and
`{text:"a",extra:[{text:"b",bold:true},"c"]}` give the same text. A list with one element gives
the text of that element.

**TXT-SAV-003** (`source_suggested_black_box_confirmed`, wire). The map form has these keys:

| Key | Saved form | Absent |
|-----|------------|--------|
| `type` | string: a content kind | TXT-SAV-005 selects the content kind |
| the fields of the content kind | section 1.1 | as section 1.1 states |
| `extra` | a list with one or more elements, each a text | no children |
| the style fields | section 1.2 | absent |

An `extra` that is an empty list is an error of the text. An `extra` that is not a list is an
error of the text.

**TXT-SAV-004** (`source_suggested_black_box_confirmed`, semantic). When `type` is present, the
reader reads only the fields of that content kind. It ignores the fields of each other content
kind. A `type` that names no content kind is an error. A necessary field of the content kind that
is absent is an error.

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

**TXT-SAV-006** (`source_suggested_black_box_confirmed`, semantic). A reader ignores a key that
this document does not list. This is true for the map form, for the map of `score`, for a click
event and for a hover event. For example, `{text:"a",junk:1}` gives the plain text `a`. The keys
`value` of an old click event and `contents` of an old hover event are such keys. The reader then
reports the necessary field that is absent.

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
or an error. It gives no text with a part removed.

**TXT-SAV-008** (`source_suggested_black_box_confirmed`, semantic). A string field accepts only a
string. A number is not a string. Thus `{text:5}` and `{insertion:5,text:"x"}` are errors in each
tree format.

**TXT-SAV-009** (`source_suggested_black_box_confirmed`, semantic). In NBT and SNBT, a boolean
field accepts each numeric tag. The value is false when the number is zero, and true for each
other number. Thus `bold:2b` and `bold:1` are true. A String tag is an error. A writer writes the
Byte tag `1b` or `0b`. In JSON, TXT-JSN-002 applies.

**TXT-SAV-010** (`source_suggested_black_box_confirmed`, semantic). In NBT and SNBT, an int field
accepts each numeric tag. The reader converts the number to an int:

| Tag | Int |
|-----|-----|
| Byte, Short, Int | the value |
| Long | the least significant 32 bits |
| Float, Double | the number without its fraction, limited to `-2147483648..2147483647` |

The reader checks the range of the field after the conversion. Thus `page:3.7d` gives 3, and
`page:4294967297L` gives 1. A writer writes an Int tag. This rule applies to `page`, to
`shadow_color` and to `count` of an item template. In JSON, TXT-JSN-004 applies.

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
`""` and one or more children is a map with the key `text` and the value `""`.

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

**TXT-SAV-014** (`black_box_confirmed`, wire). In a format that keeps the order of keys, the
writer writes the keys of the map form in this order:

1. The fields of the content kind, in the order of the rows of its table in section 1.1.
2. `extra`.
3. The style fields, in the order of the rows of the table of TXT-STY-001.

In an `nbt` content, the source field comes after `separator`. In an `object` content, `fallback`
comes after the fields of the object. In a click event and in a hover event, the fields of the
action come first, in the order of the rows. `action` comes last. This order applies to JSON.

**TXT-SAV-015** (`black_box_confirmed`, unspecified). The order of the keys of an NBT compound has
no meaning. A reader must accept each order. The order in which the game writes the keys of a
compound is an accident, and a parity test must not compare it. A test that compares bytes must
sort the keys, or compare the SNBT, which has the keys in the order of `snbt.md`. The hash does
not depend on the order (TXT-HSH-002).

**TXT-SAV-016** (`source_suggested_black_box_confirmed`, semantic). A read and then a write gives
the canonical form. The canonical form of a canonical form is the same form. A read does not keep
these parts of the input:

- the spelling: string form, list form or map form;
- `type`, `source`, `object` and each key that the reader ignores;
- an optional field with a value equal to its default;
- the case and the number of the digits of an RGB colour;
- a `uuid` string, a `shadow_color` list and a profile string;
- an identifier without a namespace, which gets the namespace `minecraft`.

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
This rule is a rule of NBT. It is here because no other document states it yet.

**TXT-NBT-003** (`source_suggested_black_box_confirmed`, wire). The wire form of a text is one
network NBT of its canonical form. The plain text `plain` is `08 0005 706c61696e`. The empty text
is `08 0000`. The tag is a String tag or a Compound tag when a vanilla writer wrote it. A reader
accepts also a List tag (TXT-SAV-001). A tag of the kind End in the place of the text is a decode
error.

**TXT-NBT-004** (`source_suggested_black_box_confirmed`, wire). The wire form has no difference
from the saved form in NBT. It has the same keys, the same tags and the same optional fields. A
reference to a registry entry is the identifier string, also in the wire form. This is true for
`id` of `show_item`, for `id` of `show_entity` and for `dialog`. A component of a `show_item`
patch has its saved form, not its wire form.

**TXT-NBT-005** (`source_suggested`, wire). The reader of the wire form in an item component has
these limits:

| Limit | Value |
|-------|-------|
| depth of the NBT | 512 levels |
| size of the NBT | a quota of 2097152 bytes |

A tag past a limit is a decode error. The exact measure of the size is not specified. The same
limits apply to a text in each packet that does not state a different quota. A document for a
packet states the quota of that packet. The depth of a text counts each list and each compound.
Thus a chain of children uses 2 levels for each child.

**TXT-NBT-006** (`source_suggested`, wire). The text sets no limit on the length of a string. NBT
does: a String tag holds at most 65535 bytes of its encoded form. A writer gives an error for a
longer string. JSON has no such limit.

**TXT-NBT-007** (`source_suggested`, semantic). A saved form that is not a valid text is a decode
error in the wire form. The reader gives no text. A text that has no saved form is an error of
the writer (TXT-CLK-006).

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

**TXT-JSN-002** (`black_box_confirmed`, semantic). In JSON, a boolean field accepts only `true`
and `false`. A number and a string are errors. Thus `"bold":1` is an error in JSON, and `bold:1`
is true in NBT. For `interpret` and `plain`, the reader takes such a value as absent
(TXT-SAV-007).

**TXT-JSN-003** (`source_suggested_black_box_confirmed`, wire). A JSON number has no number kind.
Where the saved form keeps a number kind, the reader of JSON gives the first kind of this list
that holds the number exactly:

1. For a number without a fraction: byte, short, int, long.
2. For each other number: float, then double.

A number such as `2.0` has no fraction, thus it is a byte. This rule applies to a number argument
(TXT-CNT-004) and to each number in a `payload` (TXT-CLK-005). For example, the argument `5` in
JSON is the byte 5, and its NBT is `5b`.

**TXT-JSN-004** (`black_box_confirmed`, semantic). In JSON, an int field accepts each number. The
reader removes the fraction. It keeps the least significant 32 bits of an integer that is out of
the int range. Thus `"page":3.5` gives 3, and `"shadow_color":4294967295` gives -1. The reader
checks the range of the field after the conversion. A string is an error.

**TXT-JSN-005** (`black_box_confirmed`, wire). A JSON array has no kinds, thus TXT-NBT-002 does
not apply to JSON. A `payload` that has an NBT array is a JSON array of numbers. A read of that
JSON gives a List tag, not the array. Thus JSON does not keep each `payload`.

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
a saved form, a wire form or a hash.

**TXT-EQ-002** (`source_suggested_black_box_confirmed`, semantic). Thus two saved forms that give
the same text give equal texts. These pairs are equal:

- `"x"`, `{text:"x"}`, `["x"]` and `{type:"text",text:"x"}`;
- `["a","b"]` and `{text:"a",extra:["b"]}`;
- a form with an optional field equal to its default, and the form without that field.

The game writes one canonical form for the forms of each pair.

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

**TXT-EQ-005** (`source_suggested`, semantic). Equal texts can have different canonical forms.
This occurs only for a named colour and the RGB colour with the same value. Such texts have
different wire forms and different hashes. An implementation must keep the difference in the
model (TXT-MOD-003) and must ignore it in equality.

**TXT-EQ-006** (`source_suggested`, semantic). The merge of two stacks uses this equality. The
relation "same item and components" of STK-EQ-001 compares a component that holds a text with
TXT-EQ-001. Thus a stack with the name `"x"` and a stack with the name `{text:"x"}` can merge. A
stack with a `red` name and a stack with a `#FF5555` name can merge. A list of texts, for example
`minecraft:lore`, is equal when it has equal texts in the same order.

**TXT-EQ-007** (`source_suggested`, semantic). Text equality answers the question of U9 of
`item-stack.md`: the string `"x"` and the map `{text:"x"}` decode to equal texts. They also have
one saved form, one wire form and one hash.

## 7. The hash

**TXT-HSH-001** (`source_suggested_black_box_confirmed`, client agreement). The hash of a text is
the hash of its canonical form, with the node kinds of STK-HSH-005. A component that holds a text
uses this hash in the hashed stack (STK-HSH-004). The plain text `named` is one string node, and
its hash is `d2ee6d5e`.

**TXT-HSH-002** (`source_suggested_black_box_confirmed`, client agreement). The map form is a map
node. Each key is a string node. STK-HSH-006 sorts the entries by the hash of the key. Thus the
order of the keys of the writer (TXT-SAV-014) does not change the hash. Only the set of the keys
and their values change it.

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
boolean node, not a byte node.

**TXT-HSH-004** (`source_suggested`, client agreement). The hash has an entry only for a key that
the canonical form has. An implementation must omit each field that TXT-SAV-013 omits. For
example, a `bold:false` is an entry, and an `interpret:false` is not. A text in the list form or
with a `type` key has the hash of its canonical form.

**TXT-HSH-005** (`source_suggested`, client agreement). A `minecraft:lore` component is a list
node whose elements are the hashes of its texts in order. `minecraft:custom_name` and
`minecraft:item_name` are the node of the one text. The `override` display of an attribute
modifier holds one text in its key `value`.

## 8. What a text shows

This section states what the client draws for a text, and the plain string of a text. It states
the text as the receiver gets it. The server can replace a content before it sends the text. A
document for commands states that resolve step. Some places send a text with no resolve step. An
item name, a sign without operator features and the title of a dialog are such places. An
unresolved content reaches the client in these places.

### 8.1 The expansion and the translation

**TXT-DSP-001** (`black_box_confirmed`, semantic, artefacts
`docs/spec/fixtures/text-component/e1-chat.txt` and
`docs/spec/fixtures/text-component/e4-server-string.txt`). The expansion of a text is an
ordered list of pieces. A piece has one string and one effective style. The expansion has these
parts, in this order:

- the pieces of the content of the text;
- the expansion of each child, in the order of the children.

The children come after the whole content of their parent. Thus a child of a translation comes
after the last piece of that translation. A content can give no piece. The drawn form and the
plain string both come from the expansion. Sections 8.2 to 8.4 state where they differ.

**TXT-DSP-002** (`black_box_confirmed`, semantic, artefacts
`docs/spec/fixtures/text-component/e1-chat.txt` and
`docs/spec/fixtures/text-component/e5-limits.txt`). The content `text` gives one piece. The string
of the piece is the string of the literal, unchanged. A literal with the empty string gives no
piece. This differs from a string argument of a translation, which gives a piece also when it is
empty (TXT-DSP-005).

**TXT-DSP-003** (`black_box_confirmed`, semantic, artefacts
`docs/spec/fixtures/text-component/e1-chat.txt` and
`docs/spec/fixtures/text-component/e4-server-string.txt`). The content `translatable`
selects a template in this order:

1. When the language table holds the key, the template is the entry of the table. The `fallback`
   is not used.
2. When the text has a `fallback`, the template is the `fallback`.
3. The template is the key.

A key that the table does not hold is not an error. TXT-DSP-004 expands the template. A process uses
its own language table. The client uses the language of the player. A server uses the language of
the server. Thus `no.such.key` shows as `no.such.key`, and the key `no.such.key` with the
`fallback` `fb %s!` and the argument `X` shows as `fb X!`.

**TXT-DSP-004** (`black_box_confirmed`, semantic, artefact
`docs/spec/fixtures/text-component/e1-chat.txt`). A template is read from left to
right. It has these parts:

| Part | Pieces |
|------|--------|
| a run of characters with no `%` | one piece with the run |
| `%%` | one piece with `%` |
| `%s` | the expansion of the next argument |
| `%N$s`, N a decimal number | the expansion of the argument number N, counted from 1 |

At the start, the next argument is the first argument. Each `%s` moves it by one argument. A
`%N$s` does not move it. Thus `%2$s %1$s %s` with the arguments `a` and `b` gives `b a a`. An
argument can occur more than once, or not at all. An argument that no placeholder uses does not
change the result.

A template has a format error in these cases:

- a `%` that does not start a part of the table, for example `%d`, `%5s`, `% o` and a `%` at the
  end;
- a `%s` with no argument left;
- a `%N$s` with N equal to 0, or larger than the number of arguments, or too large for an int.

A translation with a format error gives one piece. The string of that piece is the whole
template, unchanged. The pieces before the error are not kept. Thus the template `<%s> %s` with
one argument shows as `<%s> %s`. An empty template gives no piece.

**TXT-DSP-005** (`black_box_confirmed`, semantic, artefacts
`docs/spec/fixtures/text-component/e1-chat.txt`,
`docs/spec/fixtures/text-component/e5-limits.txt` and
`docs/spec/fixtures/text-component/e6-chat-display.txt`). An argument gives these pieces:

| Argument | Pieces |
|----------|--------|
| string | one piece with the string, also when it is empty |
| byte, short, int, long | one piece (TXT-DSP-006) |
| float, double | one piece (TXT-DSP-006) |
| boolean | one piece (TXT-DSP-007) |
| text | the expansion of the text, with the style of TXT-DSP-014 |

**TXT-DSP-006** (`black_box_confirmed`, semantic, artefacts
`docs/spec/fixtures/text-component/e1-chat.txt` and
`docs/spec/fixtures/text-component/e4-server-string.txt`). The piece of a byte, short, int
or long argument has the decimal digits of the value, with `-` for a negative value. The piece of a
float or double argument has the shortest string of digits that reads back as the same value in
that number kind. TXT-DSP-027 states which string, when more than one is shortest. The string has
one of these forms:

- When the absolute value is at least 0.001 and smaller than 10000000, a decimal number with at
  least one digit after the point. Examples: `1.5`, `2.0`, `0.001` and `9999999.0`.
- Otherwise, one digit, the point, one or more digits, `E` and a decimal exponent. Examples:
  `1.0E10`, `1.0E-5` and `1.6777216E7`.

Zero is `0.0`. The strings of a NaN, of an infinity and of a negative zero are not specified. A
number of a JSON text has the number kind of TXT-JSN-003. Thus the JSON argument `1.0E10` has the
digits `10000000000`.

**TXT-DSP-007** (`black_box_confirmed`, semantic, artefacts
`docs/spec/fixtures/text-component/e1-chat.txt` and
`docs/spec/fixtures/text-component/e4-server-string.txt`). The piece of a boolean argument is
`true` or `false`. NBT has no boolean (TXT-CNT-004). A text that comes in NBT holds `1b` or `0b` in
its place, and the piece is `1` or `0`. SNBT reads `true` as `1b`. A text that comes in JSON keeps
the boolean. Thus a JSON text in a data pack gives `true`, and the same text in SNBT or in NBT gives
`1`. The client never draws `true` for an argument of a text that it got from the server.

**TXT-DSP-027** (`source_suggested`, semantic). The digits of a float or double piece
(TXT-DSP-006) have the least count that is two or more and gives a string that reads back as the
same value. When more than one string of that count reads back, the piece is the string that is
nearest to the exact value of the number. When two such strings are equally near, the piece is the
string with an even last digit. Thus the smallest positive float gives `1.4E-45` and not `1.0E-45`.
The smallest positive double gives `4.9E-324` and not `5.0E-324`. No artefact shows these cases yet
(U14).

### 8.2 The other content kinds

**TXT-DSP-008** (`black_box_confirmed`, client agreement, artefacts
`docs/spec/fixtures/text-component/e1-chat.txt`,
`docs/spec/fixtures/text-component/e4-server-string.txt` and
`docs/spec/fixtures/text-component/e7-keybind.txt`). The client gives
pieces for the content `keybind` in this way:

- When the name is the name of a key mapping, the pieces are the name of the key that is bound to
  it, in the language of the player. Examples: `Space`, `Left Shift` and `Left Button`. When the
player binds `key.jump` to the key X, the pieces are `X`.
- When the key mapping has no key, the string is the entry of the language table for an unbound
  key. In `en_us` it is `Not Bound`. This is true for a key mapping that has no default key, and
  for a key mapping that the player unbinds.
- Otherwise, the content is a translation with the name as its key and no arguments
  (TXT-DSP-003). Thus a name that the table does not hold shows as the name, and `gui.yes` shows
  as `Yes`.

The comparison of the name with the names of the key mappings is exact. Thus `KEY.JUMP` is not a
key mapping. A binding that the player changes gives a different piece at the next expansion. A
process that has no key mappings, such as a server, gives one piece with the name, unchanged. This
is true also when the language table holds the name. An empty name gives no piece. The pieces of a
keybind take the effective style of the keybind node (TXT-DSP-013).

**TXT-DSP-009** (`black_box_confirmed`, semantic, artefacts
`docs/spec/fixtures/text-component/e4-server-string.txt`,
`docs/spec/fixtures/text-component/e2-sign.txt` and
`docs/spec/fixtures/text-component/e3-dialog.txt`). An unresolved content of the kind `score` or
`nbt` gives no piece. The client draws nothing for it. The plain string has no character for it.
The `separator` of an `nbt` content does not give a piece.

**TXT-DSP-010** (`black_box_confirmed`, semantic, artefacts
`docs/spec/fixtures/text-component/e4-server-string.txt`,
`docs/spec/fixtures/text-component/e2-sign.txt` and
`docs/spec/fixtures/text-component/e3-dialog.txt`). An unresolved content of the kind `selector`
gives one piece. The string of the piece is the selector string, exactly as the saved form holds
it. For example, the spaces of `@e[ type = cow ]` stay. The client does not look for an entity.
The `separator` does not give a piece.

**TXT-DSP-011** (`black_box_confirmed`, client agreement, artefacts
`docs/spec/fixtures/text-component/e1-chat.txt`,
`docs/spec/fixtures/text-component/e4-server-string.txt` and
`docs/spec/fixtures/text-component/e6-chat-display.txt`). The content `object`
gives these pieces:

- In the drawn form: one glyph. The glyph is the sprite of the atlas, or the head of the player.
  It does not come from the `font` of the style: the font of the object replaces that `font`. The
  client does not draw the `fallback`, also when the text has one. A sprite that the atlas does not
  hold draws as the missing texture. An empty `fallback` does not change the glyph. The glyph of an
  atlas sprite takes the colour of the effective style as a tint (TXT-DSP-024 states the string of
  the piece).
- In the plain string: the expansion of the `fallback` when the text has one. Otherwise one piece
  with a default string (the table below).

| Object | Default string |
|--------|----------------|
| atlas sprite, atlas `minecraft:blocks` | `[` name of the sprite `]` |
| atlas sprite, other atlas | `[` name of the sprite `@` name of the atlas `]` |
| player, profile with a `name` | `[` name of the player ` head]` |
| player, profile with no `name` | `[unknown player head]` |

The name of an identifier is its path when the namespace is `minecraft`. Otherwise it is the whole
identifier. Examples: `[block/stone]`, `[item/diamond@items]` and `[foo/bar@my:atl]`.

**TXT-DSP-012** (`source_suggested`, client agreement). The client draws the description of a
server in the server list. Before it draws, it replaces each `player` object of the description
with the expansion of its `fallback`, or with the default string of TXT-DSP-011 when the text has
no `fallback`. An atlas object stays a glyph. No experiment shows this yet: the server list needs a
pointer on the screen of the client.

**TXT-DSP-024** (`source_suggested`, client agreement). The piece of an `object` content in the
drawn form has the string `U+FFFC`. The glyph of a `player` object has the hat layer of the skin
when `hat` is true. It has no hat layer when `hat` is false. No experiment shows the string or the
layer: the string is not on the screen, and the experiments have no skin with a hat layer.

### 8.3 Style

**TXT-DSP-013** (`black_box_confirmed`, client agreement, artefacts
`docs/spec/fixtures/text-component/e6-chat-display.txt` and
`docs/spec/fixtures/text-component/e3-dialog.txt`). The effective style of a
piece comes from the nodes on the path from the root text to the node that gives the piece. The
root starts with the style that its place sets, or with the empty style. Each node applies its style
over the style of its parent, field by field:

- A style field that the node has wins.
- A style field that the node does not have comes from the parent.
- A value `false` is a value. Thus `bold:false` wins over `bold:true` of the parent.

This applies to the style fields `color`, `shadow_color`, `bold`, `italic`, `underlined`,
`strikethrough`, `obfuscated` and `font`. TXT-DSP-025 states it for `click_event`, `hover_event` and
`insertion`. A child takes the effective style of its parent. The pieces of the content of a node,
also of a `keybind` node, take the effective style of that node, except where TXT-DSP-011 and
TXT-DSP-014 state otherwise. An item name above the hotbar starts with italic when the item has a
`custom_name`. A message of chat and the title of a dialog start with the empty style.

**TXT-DSP-014** (`black_box_confirmed`, client agreement, artefacts
`docs/spec/fixtures/text-component/e6-chat-display.txt`,
`docs/spec/fixtures/text-component/e2-sign.txt` and
`docs/spec/fixtures/text-component/e3-dialog.txt`). A translation gives
these effective styles to its pieces:

- A literal run of the template, a `%` and the piece of a string, number or boolean argument take
  the effective style of the translation node.
- A text argument is a child of the translation node for the style. Its own style fields win, and
  the other fields come from the effective style of the translation node. The children of the
  argument take the effective style of the argument.
- The style of an argument does not reach a later piece. The children of the translation node
  come after its content. They take the effective style of the translation node, not the style
  of an argument.

An argument that is a plain text has the empty style, thus it takes the whole style of the
translation node (TXT-CNT-003). For example, the text `{translate:"%s-%s-%s", color:"red",
bold:false, with:[{text:"a",color:"blue"}, {text:"b",bold:true}, "c"], extra:["!"]}` draws `a` blue
and not bold. It draws `b` red and bold. It draws the two `-`, `c` and `!` red and not bold.

**TXT-DSP-025** (`source_suggested`, client agreement). The effective style of a piece takes
`click_event`, `hover_event` and `insertion` in the way of TXT-DSP-013 and TXT-DSP-014. A piece that
has a click event reacts to a click, a piece that has a hover event shows it when the pointer is on
the piece, and a piece that has an insertion gives it to the chat input at a click with the sneak
key. No experiment shows this yet: it needs a pointer on the screen of the client.

### 8.4 The section sign

**TXT-DSP-015** (`black_box_confirmed`, client agreement, artefacts
`docs/spec/fixtures/text-component/e1-chat.txt`,
`docs/spec/fixtures/text-component/e2-sign.txt` and
`docs/spec/fixtures/text-component/e6-chat-display.txt`). The drawn form omits a
section sign `U+00A7` and the character after it in the same piece. This applies to each piece,
also when that character is not a format code. A section sign at the end of a piece is omitted. The
plain string keeps both characters. Thus the literal `§cR§zX` draws `RX`, and its plain string is
`§cR§zX`.

**TXT-DSP-016** (`black_box_confirmed`, client agreement, artefact
`docs/spec/fixtures/text-component/e6-chat-display.txt`). A format code is the character after the
section sign when it is one of `0` to `9`, `a` to `f`, `k` to `o` or `r`. The case of the letter
does not matter. A format code changes the style of the characters that follow it in the same
piece. The change ends at the end of the piece. Thus a code in a template does not change an
argument, and a code at the end of a text does not change the next text. The codes have these
effects:

| Code | Effect |
|------|--------|
| `0` `1` `2` `3` `4` `5` `6` `7` | the colours `black`, `dark_blue`, `dark_green`, `dark_aqua`, `dark_red`, `dark_purple`, `gold`, `gray` |
| `8` `9` `a` `b` `c` `d` `e` `f` | the colours `dark_gray`, `blue`, `green`, `aqua`, `red`, `light_purple`, `yellow`, `white` |
| `k` `l` `m` `n` `o` | `obfuscated`, `bold`, `strikethrough`, `underlined`, `italic` |
| `r` | the effective style of the piece |

A colour code also sets `obfuscated`, `bold`, `strikethrough`, `underlined` and `italic` to false.
The codes `k` to `o` keep each other style field. A character after the section sign that is not a
format code changes no style.

### 8.5 The places and the plain string

**TXT-DSP-017** (`black_box_confirmed`, client agreement, artefacts
`docs/spec/fixtures/text-component/e2-sign.txt`,
`docs/spec/fixtures/text-component/e3-dialog.txt` and
`docs/spec/fixtures/text-component/e6-chat-display.txt`). The client expands a text in the same way
in each place where it draws a text, except the server list. TXT-DSP-012 states the server list. The places that an experiment covers are chat, a sign, the
title and the body of a dialog, and the name of the held item above the hotbar. The places differ
in the layout, in the style of the root, and in the texts that arrive unresolved. A sign draws a
text with no colour in black. Chat and a dialog draw it in white. Thus the same unresolved selector
shows as its string on a sign, in a dialog and in an item name. TXT-DSP-026 states the tooltip.

**TXT-DSP-026** (`source_suggested`, client agreement). The tooltip of an item draws the name of
the item with the expansion of TXT-DSP-001 to TXT-DSP-016, in the same way as the name above the
hotbar. No experiment shows this yet: a tooltip needs a pointer on a slot of the client.

**TXT-DSP-018** (`black_box_confirmed`, semantic, artefacts
`docs/spec/fixtures/text-component/e1-chat.txt`,
`docs/spec/fixtures/text-component/e4-server-string.txt`,
`docs/spec/fixtures/text-component/e2-sign.txt` and
`docs/spec/fixtures/text-component/e3-dialog.txt`). The plain string of a text is
the concatenation of the strings of its expansion, in order, with the limits of TXT-DSP-019 and
TXT-DSP-020. It differs from the drawn form in these cases only:

- A section sign and its next character are in the plain string (TXT-DSP-015).
- An `object` content gives its fallback or its default string (TXT-DSP-011).
- A `keybind` content on a server gives the name (TXT-DSP-008).

This table sums up the content kinds:

| Content kind | Drawn form | Plain string on the client | Plain string on a server |
|--------------|------------|----------------------------|--------------------------|
| `text` | the string | the string | the string |
| `translatable` | the pieces of TXT-DSP-003 | the same pieces | the same pieces, with the table of the server |
| `keybind` | the key name, or the translation of the name | the same | the name |
| `score` | nothing | nothing | nothing |
| `selector` | the selector string | the selector string | the selector string |
| `nbt` | nothing | nothing | nothing |
| `object` | one glyph | the fallback or the default string | the same |

### 8.6 The limits

**TXT-DSP-019** (`black_box_confirmed`, semantic, artefacts
`docs/spec/fixtures/text-component/e5-limits.txt` and
`docs/spec/fixtures/text-component/e1-chat.txt`). A consumer of an expansion
reads at most 65536 pieces. It ignores each piece after the 65536th. This gives no error and no
mark. The limit counts pieces, not characters. An empty literal gives no piece, thus it does not
count. An empty string argument gives a piece, thus it counts. The limit applies to each root text
alone. The experiments confirm it for the plain string. No experiment shows the drawn form yet.
U16 lists it.
The expansion of a text can be much larger than the text, because a `%N$s` can repeat an
argument. A translation that repeats one argument ten times, in five levels, has 100000 pieces.
The plain string does not hold the text after the 65536th piece. TXT-DSP-022 adds a bound on the nodes.
TXT-DSP-023 adds a bound on the code units of the drawn form.

**TXT-DSP-020** (`black_box_confirmed`, semantic, artefact
`docs/spec/fixtures/text-component/e5-limits.txt`). The plain string has at most
1048576 UTF-16 code units. The game cuts the string at that length, with no mark. The cut can fall
in a surrogate pair. What the cut leaves of that pair is not specified. TXT-DSP-019 applies first.
Thus a text of one character in each of 100000 pieces has a plain string of 65536 characters. A text
of 20 characters in each of 100000 pieces has a plain string of 1048576 characters.

**TXT-DSP-021** (`source_suggested_black_box_confirmed`, semantic, artefact
`docs/spec/fixtures/text-component/e5-limits.txt`). The expansion has no limit on the depth of the
nodes. This is true for the nesting in `with`, in `extra` and in both. The limits of the reader
(TXT-NBT-005) limit the text that comes in, not the expansion. A translation does not expand
itself: an argument is a text, never a key. TXT-DSP-022 bounds the number of nodes, not the depth.
The artefact shows a nesting of 50 levels and of 200 levels in `with` and in `extra`. The plain
string holds the text of each level.

**TXT-DSP-022** (`design`, deviation). The expansion of one root text visits at most 262144 nodes.
A node is a text that the walk enters: the root, each text in `extra`, each text argument that a
placeholder of the template expands and the fallback text of an object in the plain string. The walk
counts a text again at each time that it enters it, thus an argument that a template repeats counts
once for each repeat. An argument that no placeholder uses is not entered. A number and a boolean
argument are not nodes. The limit applies to each root text alone, in the drawn form and in the plain
string. When the walk has visited 262144 nodes, it enters no other node. The pieces that it has
emitted until then stay, in order. This gives no error and no mark. The reason is that a library
does not let input that a caller can get wrong take unbounded time. TXT-DSP-019 does not bound a
tree whose nodes give no piece, for example translations that repeat an argument with `%1$s` and
have empty literals as leaves. The work of such a tree grows exponentially with its depth. A text
does not differ from the game, unless the walk visits more than 262144 nodes while it emits at most
65536 pieces. The pieces that follow the 262144th node do not show, although the game shows them.
Thus a text of six levels that repeat the argument ten times over an empty literal, followed by the
argument `END` in the root, shows nothing. The game shows `END`. See deviation D2.

**TXT-DSP-023** (`design`, deviation). The drawn form of one root text has at most 1048576 UTF-16
code units. The cut is the same cut as the cut of the plain string in TXT-DSP-020. The drawn form
stops after the 1048576th code unit. The code units that follow do not show. This gives no error and
no mark. The limit applies to each root text alone. The order of the bounds is: the limit of
TXT-DSP-019 on the pieces, then the bound of TXT-DSP-022 on the nodes, then this cut. The reason is
that a library does not let input that a caller can get wrong take unbounded memory. Without the
cut, 65536 long pieces give a drawn form of gigabytes. What the cut leaves of a surrogate pair is
not specified. Whether a character that TXT-DSP-015 omits counts as a code unit is not specified.
A text that the game shows within 1048576 code units does not differ. See deviation D3.

### 8.7 Experiments

Each experiment runs the vanilla server and the vanilla client of the version of this project. The
console of the server gives each command. Each fixture holds the command of each case with its
output. A fixture of a screen holds the commands, a note of what the screenshot shows, and the
screenshot.

- **E1, chat and the plain string of the client.** Run `tellraw` for the template cases of
  TXT-DSP-004, for the number kinds of TXT-DSP-006, for the keybind names of TXT-DSP-008 and for
  the objects of TXT-DSP-011. The log line of the client for each message shows the plain string
  of the client. The command `tellraw` resolves a selector, a score and an nbt content on the
  server, thus E4 covers them. The output is in `docs/spec/fixtures/text-component/e1-chat.txt`.
- **E2, sign.** Set a sign with `setblock`, with raw lines that hold unresolved contents, a
  translation, an object, a section sign and styled arguments. A screenshot of the client window
  shows the drawn form of TXT-DSP-009 to TXT-DSP-011, TXT-DSP-014 and TXT-DSP-015. The fixtures are
  `e2-sign.txt` and `e2-sign.png`.
- **E3, dialog and item name.** Run `dialog show` with a notice that has a title and a body of the
  same contents. Run `item replace` with a custom name of the same contents. Screenshots show that
  the title, the body and the name above the hotbar draw as the sign does. The fixtures are
  `e3-dialog.txt`, `e3-dialog.png` and `e3-hotbar.png`.
- **E4, plain string of a server.** Run `item replace` with a custom name of each content kind. The
  command prints the name in its feedback, thus the log line of the server shows the plain string
  of the server and the log line of the client shows the plain string of the client. Run `loot
  replace` with a loot table of a data pack that has a JSON boolean and JSON numbers as arguments.
  The fixture is `e4-server-string.txt`. The data pack is in the folder `pack-json`.
- **E5, limits.** Run `tellraw` with translations that repeat one argument ten times, in five
  levels, with leaves of 1, 5 and 20 characters. The log line of the client has 65536, 327680 and
  1048576 characters. With the argument `END` after the repeated part, the chat shows no `END`.
  Run it also with leaves that are an empty string and an empty literal, and with a nesting of 50
  and of 200 levels. The output is in `docs/spec/fixtures/text-component/e5-limits.txt`.
- **E6, style, format codes and objects in chat.** Run `tellraw` with nested styles, with
  translations that have styled arguments, with format codes and with objects. A screenshot of the
  chat shows the drawn form of TXT-DSP-011 and of TXT-DSP-013 to TXT-DSP-016. The fixtures are
  `e6-chat-display.txt` and the images `e6-style.png`, `e6-style-2.png`, `e6-codes-1.png`,
  `e6-codes-2.png` and `e6-objects.png`.
- **E7, key mappings that the player changed.** Start the client with an options file that binds
  `key.jump` to the key X and leaves `key.sneak` with no key. Run `tellraw` and `item replace`
  with `keybind` contents. The fixture is `e7-keybind.txt`.
## 9. Boundaries

This document does not state these subjects:

- Rendering: fonts, glyphs, the width of a text, the shadow, the layout of a place and the order of
  the characters of a right-to-left line. A document for the client states them.
- The resolve step of the server, which replaces a `score`, `selector` or `nbt` content before the
  server sends a text. A document for commands states it, with its limits on depth and on count.
- The language tables, the names of the key mappings and their default keys.
- The input of the legacy format codes of the section sign. A reader keeps the section sign as a
  character of the literal (TXT-CNT-001). TXT-DSP-015 and TXT-DSP-016 state what the client draws.
- The chat packets, signed messages and chat types.
- The limit on the JSON length of a text that some values apply, for example a page of a book. The
  document of each such value states it.
- The limit of a menu that renames a stack, and the text limits of a sign (issue #219).
- The fields of a dialog, of a profile and of an item component. `item-stack.md` states the item
  template.
- The grammars of the entity selector, the block position and the NBT path.
- The quota of each packet that carries a text, and the optional text of a packet.
- The string of Bedrock Edition that an edge makes from a text (issue #666).

## 10. Intentional deviations

**D1.**

- **Rule**: TXT-STY-002.
- **Reference**: after the `#`, the reference reader accepts a sign, for example `#+ff`. It also
  accepts more than 6 digits when the number is not larger than `FFFFFF`, and digits that are not
  ASCII.
- **Visible to the client**: no. The vanilla writer writes `#` and 6 ASCII digits.
- **Test**: a differential test must not send a sign, more than 6 digits or a digit that is not
  ASCII. The fixture `text_probe` has the case `color_plus_hex`; a parity test must skip it.

**D2.**

- **Rule**: TXT-DSP-022.
- **Reference**: the reference expansion has no bound on the nodes that it visits. The bound of
  TXT-DSP-019 counts pieces, thus a tree that gives no piece never reaches it. The expansion of such
  a tree grows exponentially, for example with nested translations that repeat an argument with
  `%1$s` over empty literals (the artefact `e5-limits.txt` shows five levels of ten repeats).
- **Reason**: a library does not let input that a caller can get wrong take unbounded time. The
  piece bound and the cut of TXT-DSP-020 stop a text that gives many pieces before the node bound.
  Only a tree that gives few pieces or none reaches the node bound.
- **Visible to the client**: yes, for a text that visits more than 262144 nodes and emits at most
  65536 pieces. The expansion shows only the pieces that the walk emitted before the bound. The
  reference shows all of them, after a long time. A text of six levels that repeat the argument ten
  times over an empty literal, followed by `END` in the root, shows nothing, and the reference shows
  `END`. No artefact shows the case of six levels yet; the artefact of five levels shows `END`.
  Another text does not differ.
- **Bug reference**: none.
- **Test**: a differential test must not send a text that visits more than 262144 nodes. A parity
  test over texts within the bound is not affected.

**D3.**

- **Rule**: TXT-DSP-023.
- **Reference**: the reference drawn form has only the limit of 65536 pieces of TXT-DSP-019. It has
  no limit on its characters. Only the plain string has the cut of TXT-DSP-020.
- **Reason**: a library does not let input that a caller can get wrong take unbounded memory. A
  text of 65536 long pieces gives a drawn form of gigabytes.
- **Visible to the client**: yes, for a text that has more than 1048576 code units. The reference
  shows all of them. A text with at most 1048576 code units does not differ.
- **Bug reference**: none.
- **Test**: a differential test must not send a text of more than 1048576 code units to the drawn
  form. A parity test over shorter texts is not affected.

## 11. Unresolved

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
- **Q6**: TXT-EQ-005. The item hover keeps the components of a `show_item` hover as
  NBT below the item crate. Equality of two such hovers then compares NBT, not components. The
  owner decides if this difference from TXT-EQ-004 is acceptable below the item crate.
- **Q7**: TXT-DSP-003, TXT-DSP-008. Which document owns the language tables, the names of the key
  mappings and their default keys? This document uses them and does not list them.

Not decided by an experiment yet:

- **U1**: TXT-NBT-005. The exact measure of the NBT quota.
- **U2**: TXT-SAV-005. The result of a `type` key that is not a string.
- **U3**: TXT-SAV-009, TXT-SAV-010. The result of a NaN, of an infinity and of a fraction smaller
  than 1 in a boolean field and in an int field.
- **U4**: TXT-DSP-026. The drawn form in a tooltip. The experiment needs a pointer on a slot of
  the screen of the client.
- **U6**: TXT-DSP-012. The server list. The experiment needs a pointer on the menu of the client
  and a server that answers a status request with a description.
- **U12**: TXT-DSP-025. The hover event, the click event and the insertion of a piece. The
  experiment needs a pointer on a piece of chat.
- **U13**: TXT-DSP-024. The hat layer of a head, and the string of the piece of an object. The
  experiment needs a skin with a hat layer, which comes from a session server.
- **U14**: TXT-DSP-027. The string of the smallest positive float and double, and of the values
  next to them. Extend E1 with `tellraw` arguments `1.4E-45f`, `4.9E-324d`, `1.0E-44f` and
  `2.0E-323d`.
- **U15**: TXT-DSP-022, D2. The reference shows `END` after six levels of ten repeats over an empty
  literal. Extend E5 with that case.
- **U16**: TXT-DSP-019. The limit of 65536 pieces in the drawn form. Run the case LIM-a1 of E5
  and take a screenshot of the chat. Compare the drawn text with the plain string.
