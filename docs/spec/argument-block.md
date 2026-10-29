# The `minecraft:block_predicate` argument: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |
| Parser ids | `block_predicate` 13; the related type `block_state` 12 (`commands-packet.md`, CMDP-ARG-001) |

Source of the version values: `assets/minecraft/version.json`. Source of the parser ids:
`assets/mcrs/reports/registries.json`, registry `minecraft:command_argument_type`.

This document states the argument type `minecraft:block_predicate`. It states the text that the
type accepts, the errors and their positions, the suggestions, and the test that the value
applies to a block in the world. Section 6 states how the related type `minecraft:block_state`
differs. The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Reader**, **cursor**, **position**, **whitespace**, **error**, **source**, **suggestion**,
  **remaining input**: as `command-parser.md`, section 0.1 and CMD-SUG-005, defines them.
- **Identifier**: as `registry.md`, REG-ID-001, defines it. The text of an identifier is its full
  form `namespace:path`, also when the command line omits the namespace.
- **Tag**: a block tag, as `registry.md` defines it. A **member** of a tag is a block that the
  resolved tag contains (`registry.md`, REG-TAG-005).
- **Block**: an entry of the block registry. Each block has an ordered list of properties and a
  default block state.
- **Property**: a named variable of a block, with an ordered list of property values. A property
  is of one of three kinds: boolean, integer or enumeration.
- **Value text**: the text form of a property value. A boolean has `true` and `false`. An
  integer has its decimal form. An enumeration value has its serialized name, for example
  `north`.
- **Block state**: a block with one property value for each of its properties.
- **Block entity**: the data object that the world keeps at a position for some blocks, for
  example a chest. A block **has a block entity** when its block states have one.
- **Compound**: an NBT compound tag.
- **Saved data**: the compound that ARG-BLK-012 states for a block entity.
- **Block form**: a predicate that starts with an identifier.
- **Tag form**: a predicate that starts with `#`.
- **Property list**: the text from `[` to the `]` that closes it.
- **Data compound**: the compound in `{` and `}` at the end of a predicate.
- **Read string**: the read of `command-parser.md`, CMD-RD-009. It reads a quoted string or an
  unquoted string. At the end of the command line, or when next is not an unquoted character,
  it gives the empty string with no error.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. None of the experiments has been run. The
experiments use the command `execute if block <pos> <predicate>` (`command-execute.md`,
CMD-EX-056). A pass gives `commands.execute.conditional.pass`. A test that does not pass gives
`commands.execute.conditional.fail`.

The vanilla client parses this argument and makes its suggestions itself, from the block
registry and the block tags that the server sends. Thus the rules of the read are of the kind
client agreement.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `ARG-BLK` | the read, the errors, the test, the suggestions and the wire form |
| `ARG-BLK-DEV` | intentional deviations from the reference |

### 0.4 Scope

In scope: the read of one predicate, its errors, its suggestions, its test of a block, and its
wire form. The vanilla commands that read the argument are `execute if block`,
`execute unless block`, `fill ... replace <filter>` and `clone ... filtered <filter>`.

Out of scope: the grammar of the text form of NBT (SNBT). `snbt.md` states it. Section 1.6
states what the predicate needs from it.

---

## 1. The read

### ARG-BLK-001: Top level

- **Behavior:** note the start of the argument.
  1. If next is `#`, read a tag (ARG-BLK-003). Then, if next is `[`, read the property list of
     a tag (ARG-BLK-006).
  2. Otherwise read a block (ARG-BLK-002). Then, if next is `[`, read the property list of a
     block (ARG-BLK-004).
  3. Then, if next is `{`, read a data compound (ARG-BLK-007).
  4. The read ends. The cursor stays after the last part that the read accepted.
- **Postconditions:**
  - The read does not skip whitespace before `[`, before `{`, or after `]`.
  - An error has the position that its rule states. After an error, the argument has no value.
  - The read stops after the data compound. A second `[` or `{` is not part of the argument.
- **Edge cases:**

| Text | Result |
|------|--------|
| `stone` | block form, no property list |
| `stone []` | block form `stone`; the argument ends before the separator |
| `stone{}` | block form with an empty data compound |
| `#logs[axis=y]{}` | tag form with a property list and a data compound |
| `stone{}[]` | block form with a data compound; the argument ends before `[` |

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `execute if block ~ ~ ~ stone{}[] run say x`. The reply is
`command.expected.separator`, with the context text that ends before `[`.

### ARG-BLK-002: Read a block

- **Behavior:** note the start.
  1. Read an identifier (`argument-resource.md`, ARG-RES-001). A read that fails gives
     `argument.id.invalid` at the start.
  2. If the block registry has no entry with that identifier, set the cursor back to the start.
     Fail with `argument.block.id.invalid`, with the text of the identifier.
- **Edge cases:**
  - `Stone` reads the identifier `minecraft:` with an empty path, because `S` is not an
    identifier character. No block has that identifier. The read fails with
    `argument.block.id.invalid`, with `minecraft:`, at the start.
  - `nope` fails with `argument.block.id.invalid`, with `minecraft:nope`.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `execute if block ~ ~ ~ nope`. The reply has `argument.block.id.invalid` with
`minecraft:nope`, and its context text ends before `nope`.

### ARG-BLK-003: Read a tag

- **Behavior:** note the position of `#`.
  1. Skip the `#`. Read an identifier (ARG-RES-001). A read that fails gives
     `argument.id.invalid` at the position after `#`.
  2. If the block registry has no tag with that identifier, set the cursor back to the `#`.
     Fail with `arguments.block.tag.unknown`, with the text of the identifier. The text has no
     `#`.
- **Postconditions:** a tag that exists and has no members is a valid tag. Its test passes for
  no block.
- **Edge cases:** the key `arguments.block.tag.unknown` starts with `arguments`, not with
  `argument`. The other keys of this document start with `argument`.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `execute if block ~ ~ ~ #minecraft:nope`. The reply has
`arguments.block.tag.unknown` with `minecraft:nope`, and its context text ends before `#`.

### ARG-BLK-004: Property list of a block

- **Inputs:** the block of ARG-BLK-002. Next is `[`.
- **Behavior:**
  1. Skip the `[`. Skip whitespace.
  2. If next is `]`, or the reader is at the end, go to step 4.
  3. Read one entry:
     1. Skip whitespace. Note the name start. Read a string, the name.
     2. If the block has no property with that name, set the cursor back to the name start.
        Fail with `argument.block.property.unknown`, with the text of the block identifier and
        the name.
     3. If the list already has an entry for that property, set the cursor back to the name
        start. Fail with `argument.block.property.duplicate`, with the name and the text of the
        block identifier.
     4. Skip whitespace. If next is not `=`, or the reader is at the end, fail with
        `argument.block.property.novalue`, with the name and the text of the block identifier.
        The position is the cursor: after the name and the whitespace.
     5. Skip the `=`. Skip whitespace. Note the value start. Read a string, the value text.
     6. Read the property value from the value text (ARG-BLK-005). If the value text is not
        valid, set the cursor back to the value start. Fail with
        `argument.block.property.invalid`, with the text of the block identifier, the value text
        and the name, in this order.
     7. Skip whitespace. At the end, go to step 4. If next is `]`, go to step 4. If next is `,`,
        skip it and go to step 2. Otherwise fail with `argument.block.property.unclosed` at the
        cursor.
  4. If the reader is at the end, fail with `argument.block.property.unclosed` at the end.
     Otherwise skip the `]`.
- **Postconditions:** the value of the list is the set of pairs of a property and a property
  value. A property that the list does not name is not part of the value.
- **Edge cases:**

| Text after the block | Result |
|----------------------|--------|
| `[]` | an empty list |
| `[ facing = north ]` | `facing` is `north` |
| `[facing=north,]` | `facing` is `north`; a `,` directly before `]` is accepted |
| `[facing=north, ]` | `argument.block.property.unknown` with the empty name, at the `]` |
| `[,]` | `argument.block.property.unknown` with the empty name, at the `,` |
| `["facing"="north"]` | `facing` is `north`; the name and the value can be quoted |
| `[facing=north,facing=south]` | `argument.block.property.duplicate` at the second `facing` |
| `[facing]` | `argument.block.property.novalue` at the `]` |
| `[facing=up]` | `argument.block.property.invalid`, at `up` |
| `[facing=north` | `argument.block.property.unclosed` at the end |
| `[facing=north;]` | `argument.block.property.unclosed` at the `;` |
| `[facing="north` | `parsing.quote.expected.end` at the end (CMD-RD-007) |

  A quoted name or a quoted value can raise the errors of `command-parser.md`, CMD-RD-007, at
  their positions.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send each line of the table after `execute if block ~ ~ ~ oak_stairs`. Compare the
reply and its context text with the table.

### ARG-BLK-005: Property value from a value text

- **Inputs:** a property and a value text.
- **Behavior:** the value text is valid when it names a property value of the property:
  - boolean: the text is exactly `true` or `false`;
  - enumeration: the text is exactly the value text of one value of the property;
  - integer: the text is a decimal integer in the 32-bit signed range, and the property has that
    value. The text is an optional `+` or `-`, then one or more ASCII digits `0` to `9`
    (ARG-BLK-DEV-002).
- **Edge cases:**

| Property | Value text | Result |
|----------|------------|--------|
| `waterlogged` | `True` | not valid: the comparison is exact |
| `age` of `minecraft:wheat`, 0 to 7 | `+7`, `07`, `-0` | valid: 7, 7 and 0 |
| `age` of `minecraft:wheat` | `8` | not valid: 8 is not a value of the property |
| `age` of `minecraft:wheat` | `1.0` | not valid |
| `facing` of `minecraft:oak_stairs` | `up` | not valid: the property has only four values |

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, place `minecraft:wheat` with `age` 7 at a position `P`. Send
`execute if block P wheat[age=+7]`. The reply is `commands.execute.conditional.pass`. Send
`execute if block P wheat[age=8]`. The reply has `argument.block.property.invalid`.

### ARG-BLK-006: Property list of a tag

- **Inputs:** the tag of ARG-BLK-003. Next is `[`.
- **Behavior:** let the value start be "none".
  1. Skip the `[`. Skip whitespace.
  2. If next is `]`, or the reader is at the end, go to step 4.
  3. Read one entry:
     1. Skip whitespace. Note the name start. Read a string, the name.
     2. If the list already has an entry with that name, set the cursor back to the name start.
        Fail with `argument.block.property.duplicate`, with the name and the tag text
        (ARG-BLK-DEV-001).
     3. Skip whitespace. If next is not `=`, or the reader is at the end, set the cursor back
        to the name start. Fail with `argument.block.property.novalue`, with the name and the
        tag text.
     4. Skip the `=`. Skip whitespace. Note the value start. Read a string, the value text. Add
        the pair of the name and the value text to the list.
     5. Skip whitespace. If the reader is at the end, go to step 4. Otherwise let the value
        start be "none". If next is `]`, go to step 4. If next is `,`, skip it and go to step 2.
        Otherwise fail with `argument.block.property.unclosed` at the cursor.
  4. If the reader is not at the end, skip the `]`. Otherwise, if the value start is not
     "none", set the cursor back to the value start. Fail with
     `argument.block.property.unclosed`.
- **Postconditions:**
  - The read does not check the name or the value text. A name that no member has, an empty
    name and an empty value text are accepted. ARG-BLK-010 decides them in the test.
  - The tag text is `#` and the text of the tag identifier, for example `#minecraft:logs`.
- **Edge cases:**

| Text after the tag `#logs` | Result |
|----------------------------|--------|
| `[]` | an empty list |
| `[foo=bar]` | accepted; the test passes for no block |
| `[axis=]` | accepted, with the empty value text |
| `[=y]` | accepted, with the empty name |
| `[axis=y,]` | accepted |
| `[axis=y, ]` | `argument.block.property.novalue` with the empty name, at the `]` |
| `[axis]` | `argument.block.property.novalue` at `axis`, not at the `]` |
| `[axis=y` | `argument.block.property.unclosed` at `y`, not at the end |
| `[axis=y ` | `argument.block.property.unclosed` at `y` |
| `[axis=y,` | `argument.block.property.unclosed` at the end |
| `[` | `argument.block.property.unclosed` at the end |

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send each line of the table after `execute if block ~ ~ ~ #minecraft:logs`. Compare
the reply and its context text with the table. For `[foo=bar]`, the reply is
`commands.execute.conditional.fail` at every position.

### ARG-BLK-007: Data compound

- **Inputs:** next is `{`.
- **Behavior:** read one compound in the text form of NBT, from the `{` to the `}` that closes
  it. The cursor stays after that `}`. The read does not check the text after the `}`.
- **What the predicate needs from the SNBT read:**
  - The read accepts the same set of texts as the vanilla client: the compound read of
    `snbt.md`, SNBT-ARG-002 and SNBT-ARG-003.
  - The result keeps the type of each tag: byte, short, int, long, float, double, string, list,
    compound, byte array, int array and long array. The test of ARG-BLK-011 compares types.
  - An error has a position and a key from the table of ARG-BLK-008, part 2.
- **Edge cases:** `{}` is a valid empty compound. `{` at the end of the command line fails with
  an error of the SNBT read.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, place a chest at a position `P`. Send `execute if block P chest{}`. The reply is
`commands.execute.conditional.pass`.

---

## 2. Errors

### ARG-BLK-008: Error keys

Part 1: the errors of this argument. The key arguments come in the order of the column. Each
key argument is a plain text.

| Translation key | Key arguments | Position | Rule |
|-----------------|---------------|----------|------|
| `argument.id.invalid` | none | the start of the identifier | ARG-BLK-002, ARG-BLK-003 |
| `argument.block.id.invalid` | identifier | the start of the argument | ARG-BLK-002 |
| `arguments.block.tag.unknown` | identifier, without `#` | the `#` | ARG-BLK-003 |
| `argument.block.property.unknown` | block identifier, name | the name start | ARG-BLK-004 |
| `argument.block.property.duplicate` | name, block identifier or tag text | the name start | ARG-BLK-004, ARG-BLK-006 |
| `argument.block.property.novalue` | name, block identifier | the cursor after the name and the whitespace | ARG-BLK-004 |
| `argument.block.property.novalue` | name, tag text | the name start | ARG-BLK-006 |
| `argument.block.property.invalid` | block identifier, value text, name | the value start | ARG-BLK-004 |
| `argument.block.property.unclosed` | none | the cursor, or the value start (ARG-BLK-006, step 4) | ARG-BLK-004, ARG-BLK-006 |
| `parsing.quote.expected.end`, `parsing.quote.escape` | as CMD-ERR-002 | as CMD-RD-007 | a quoted name or value |

The English texts show the order. For example, `argument.block.property.invalid` is
"Block %s does not accept '%s' for %s property", and `argument.block.property.duplicate` is
"Property '%s' can only be set once for block %s".

Part 2: the errors of the SNBT read that reach this argument through ARG-BLK-007.

| Translation key | Key arguments |
|-----------------|---------------|
| `argument.literal.incorrect` | the expected characters, joined with `\|`, for example `}` |
| `argument.nbt.expected.compound` | none |
| `snbt.parser.number_parse_failure` | a message |
| `snbt.parser.expected_hex_escape` | a length |
| `snbt.parser.invalid_codepoint` | a code point |
| `snbt.parser.no_such_operation` | an operation |
| `snbt.parser.expected_number_or_boolean` | none |
| `snbt.parser.expected_string_uuid` | none |
| `arguments.nbtpath.too_deep` | none |
| `snbt.parser.expected_integer_type` | none |
| `snbt.parser.expected_float_type` | none |
| `snbt.parser.expected_non_negative_number` | none |
| `snbt.parser.invalid_character_name` | none |
| `snbt.parser.invalid_array_element_type` | none |
| `snbt.parser.invalid_unquoted_start` | none |
| `snbt.parser.expected_unquoted_string` | none |
| `snbt.parser.invalid_string_contents` | none |
| `snbt.parser.expected_binary_numeral` | none |
| `snbt.parser.underscore_not_allowed` | none |
| `snbt.parser.expected_decimal_numeral` | none |
| `snbt.parser.expected_hex_numeral` | none |
| `snbt.parser.empty_key` | none |
| `snbt.parser.leading_zero_not_allowed` | none |
| `snbt.parser.infinity_not_allowed` | none |

The SNBT read gives `argument.nbt.expected.compound` only for a value that is not a compound.
The data compound starts with `{`, so the read never gives it here (`snbt.md`, SNBT-ARG-002).
No input raises `snbt.parser.expected_integer_type` or `snbt.parser.expected_float_type`. The
client language file has no text for `snbt.parser.underscore_not_allowed`, so the client shows
the key. `arguments.nbtpath.too_deep` is the depth limit of `snbt.md`, SNBT-LIM-001. The
positions of the SNBT errors, and the choice between two errors at one position, are in
`snbt.md`, SNBT-ERR-001 to SNBT-ERR-004. The keys `argument.nbt.expected.key`,
`argument.nbt.expected.value`, `argument.nbt.list.mixed` and `argument.nbt.array.*` are in the
language file, but the SNBT read does not raise them.

Provenance: `source_suggested` (the language file `en_us.json` of the client jar). Kind: wire.
Confidence: medium. Experiment: as an operator, send
`execute if block ~ ~ ~ oak_stairs[facing=north,facing=south]`. The chat line reads "Property
'facing' can only be set once for block minecraft:oak_stairs". Send
`execute if block ~ ~ ~ chest{a:1`. The reply has `argument.literal.incorrect`.

---

## 3. The test

### ARG-BLK-009: Test of the block form

- **Inputs:** the value of a block form: a block `B`, the set of pairs of ARG-BLK-004, and an
  optional data compound `D`. A block state `S` at a position.
- **Behavior:** the test passes when all of these are true:
  1. The block of `S` is `B`.
  2. For each pair of a property and a property value, `S` has that value for that property.
  3. If `D` is present, ARG-BLK-011 passes for `D` and the position.
- **Postconditions:** a property that the list does not name can have any value. The default
  block state of `B` does not take part in the test.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator,
place `minecraft:oak_stairs[facing=east]` at a position `P`. Send
`execute if block P oak_stairs`, then `execute if block P oak_stairs[facing=east]`. Both replies
are `commands.execute.conditional.pass`. Send `execute if block P oak_stairs[facing=north]`. The
reply is `commands.execute.conditional.fail`.

### ARG-BLK-010: Test of the tag form

- **Inputs:** the value of a tag form: a tag `T`, the list of pairs of ARG-BLK-006, and an
  optional data compound `D`. A block state `S` at a position.
- **Behavior:** the test passes when all of these are true:
  1. The block of `S` is a member of `T`.
  2. For each pair of a name and a value text:
     - the block of `S` has a property with that name;
     - the value text is valid for that property (ARG-BLK-005);
     - `S` has that property value.
  3. If `D` is present, ARG-BLK-011 passes for `D` and the position.
- **Postconditions:** each member decides the names and the value texts with its own
  properties. A pair that a member does not have makes the test fail for that member. It is not
  an error.
- **Edge cases:** `#minecraft:slabs[type=double]` passes for a double oak slab. It fails for
  each block outside the tag, also for a block that has a property `type`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator,
place `minecraft:oak_log[axis=x]` at a position `P`. Send
`execute if block P #minecraft:logs[axis=x]`. The reply is `commands.execute.conditional.pass`.
Send `execute if block P #minecraft:logs[axis=up]`. The reply is
`commands.execute.conditional.fail`, not a parse error.

### ARG-BLK-011: Test of the data compound

- **Inputs:** a data compound `D` and a position.
- **Behavior:**
  1. If the world has no block entity at the position, the test fails.
  2. Otherwise compute the saved data `A` of the block entity (ARG-BLK-012). The test passes
     when `D` matches `A`.
- **Match:** a tag `e` matches a tag `a` when all of these are true:
  - `e` and `a` have the same tag type. A byte `1b` does not match an int `1`.
  - If `e` is a compound: for each key of `e`, `a` has that key, and the tag of `e` at that key
    matches the tag of `a` at that key. `a` can have more keys.
  - If `e` is a list:
    - if `e` is empty, `a` is empty;
    - otherwise `a` has at least as many elements as `e`, and each element of `e` matches at
      least one element of `a`. Two elements of `e` can match the same element of `a`.
  - Otherwise `e` equals `a`. Arrays compare element by element, with equal length.
- **Edge cases:**
  - `{}` passes for each block entity, and fails where no block entity exists.
  - A list `[x, x]` in `e` fails against a list in `a` with one element `x`, because of the
    count. It passes against a list with the elements `x` and `y`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator,
place a chest at a position `P` with one stack of stone in slot 0 and one stack of dirt in slot
1. Send `execute if block P chest{Items:[{id:"minecraft:stone"},{id:"minecraft:stone"}]}`. The
reply is `commands.execute.conditional.pass`. Then send `execute if block P stone{}` on a stone
block. The reply is `commands.execute.conditional.fail`.

### ARG-BLK-012: Saved data of a block entity

- **Behavior:** the saved data is the compound that the save format writes for the block
  entity, with these keys too:
  - `id`: a string, the identifier of the block entity type;
  - `x`, `y`, `z`: ints, the position of the block entity;
  - `components`: a compound, the components of the block entity. It is an empty compound when
    the block entity has no components.
- **Postconditions:** a data compound can test the position. `{x:5}` passes only for the block
  entities with the x coordinate 5.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator,
place a chest at `10 64 10`. Send this command:
`execute if block 10 64 10 chest{x:10,y:64,z:10,id:"minecraft:chest",components:{}}`. The reply
is `commands.execute.conditional.pass`. Send `execute if block 10 64 10 chest{x:10b}`. The reply
is `commands.execute.conditional.fail`.

---

## 4. Suggestions

### ARG-BLK-013: The suggestion point

- **Behavior:** read the truncated input from the start of the argument with ARG-BLK-001. Ignore
  an error. The suggestion point is the cursor where the read stops:
  - after an error, the position of that error;
  - after a read that succeeds, the end of the read.

  Make the suggestions of ARG-BLK-014 with a builder at the suggestion point
  (`command-parser.md`, CMD-SUG-005). The stage is the last row of ARG-BLK-014 that the read
  reached.
- **Edge cases:** for `#a:b:c`, the identifier read fails after `#`. The suggestion point is the
  position after `#`. The remaining input `a:b:c` has no common start with the prefix `#`, so
  the list is empty.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### ARG-BLK-014: Suggestions by stage

"If empty" means: only when the remaining input is empty.

| Stage | The read reached | Suggestions |
|-------|------------------|-------------|
| start | nothing; or the block read failed | each tag with the prefix `#`, then each block, with ARG-RES-002 |
| tag name | `#`, and the tag read failed | each tag with the prefix `#`, with ARG-RES-002 |
| after a block | a block | if empty: `[` when the block has a property; `{` when the block has a block entity |
| after a tag | a tag | if empty: `[` when a member has a property; `{` when a member has a block entity |
| block, first name | the `[` of a block | if empty: `]`; and ARG-BLK-015 for the block |
| block, `=` | a name of a block | if empty: `=` |
| block, value | the `=` of a block | each value text of the property (ARG-BLK-016) |
| block, after a value | a valid value of a block | if empty: `]`; and `,` when the list names fewer properties than the block has |
| block, next name | a `,` of a block | ARG-BLK-015 for the block, with no `]` |
| tag, first name | the `[` of a tag | if empty: `]`; and ARG-BLK-015 for the members |
| tag, value | the `=` of a tag | each value text of that name from each member that has it (ARG-BLK-016); then `,` when a member has a property that the list does not name; then `]` |
| tag, next name | a `,` of a tag | ARG-BLK-015 for the members, with no `]` |
| after the list | the `]` | if empty: `{` when the block state has a block entity, or when a member has one |
| data | the `{` | nothing |

- **Postconditions:**
  - At the start, the block read fails for each text that is not a complete block identifier.
    When the text is a complete block identifier, the stage is "after a block". Then the list
    does not contain the longer identifiers: `stone` does not suggest `minecraft:stone_bricks`.
  - An error in a name does not change the stage: the stage stays "first name" or "next name",
    at the name start. A tag property list has no stage "`=`". Thus for `#logs[axis`, the stage
    is "tag, first name" at `axis`, and the list has `axis=`.
  - In the stage "tag, value", `,` and `]` do not need an empty remaining input. Their range
    starts at the suggestion point, so the suggestion replaces the value text (section 8).
  - CMD-SUG-006 removes duplicates and sets the final order. The order in which the argument
    adds suggestions is not significant.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. The client makes these
suggestions itself. The server makes them when it answers a suggestion request
(`commands-packet.md`, CMDP-SUG-001 and CMDP-SUG-002). Experiment: in the vanilla client, type
`/execute if block ~ ~ ~ oak_stairs[facing=north`. The list is `,` and `]`. Type
`/execute if block ~ ~ ~ oak_stairs[facing=north,half=top,shape=straight,waterlogged=false`. The
list is only `]`.

### ARG-BLK-015: Property name suggestions

- **Inputs:** the remaining input `r`, and a block or the members of a tag.
- **Behavior:**
  1. Map `r` to lower case with the locale-independent mapping.
  2. For each property of the block, or of each member, take its name `n`. Skip `n` when the
     list already names it.
  3. Suggest `n` and `=` when `n` starts with the result of step 1.
- **Postconditions:** the filter is a plain prefix test. It is not the substring filter of
  `command-parser.md`, CMD-SUG-008: a match after `_` does not count.
- **Edge cases:** for `oak_stairs[`, the list is `]`, `facing=`, `half=`, `shape=` and
  `waterlogged=`. For `oak_stairs[FAC`, the list is `facing=`. For `oak_stairs[logged`, the
  list is empty.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the vanilla
client, type `/execute if block ~ ~ ~ oak_stairs[logged`. The client shows no suggestion. Type
`/execute if block ~ ~ ~ oak_stairs[wat`. The list is `waterlogged=`.

### ARG-BLK-016: Property value suggestions

- **Behavior:** suggest each value text of the property. The suggestions do not use the
  remaining input as a filter. CMD-SUG-005 drops only a text that equals the remaining input.
- **Postconditions:** the text of an integer value is its decimal form. CMD-SUG-006 sorts the
  list by text, so `10` comes before `2` (`command-parser.md`, CMD-DEV-010).
- **Edge cases:** for `oak_stairs[facing=nor`, the read fails at `nor`, and the list is `north`,
  `south`, `west` and `east`, from the position of `nor`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the vanilla
client, type `/execute if block ~ ~ ~ oak_stairs[facing=nor`. The list has all four values. Type
`/execute if block ~ ~ ~ redstone_wire[power=x`. The list has the 16 values `0` to `15`. The
vanilla client shows them in numeric order (`command-parser.md`, CMD-DEV-010).

---

## 5. The wire

### ARG-BLK-017: Parser id, properties and suggestion type

- **Behavior:** the type `minecraft:block_predicate` has the parser id 13. It has no
  properties. A node of this type has no suggestion type: the client makes its suggestions
  itself (`commands-packet.md`, CMDP-ARG-002).
- **Postconditions:** the client resolves blocks from its block registry, and tags from the
  block tags of the update tags packet (`registry.md`). A block or a tag that the client does
  not know is an error in the client.

Provenance: `black_box_confirmed` (`assets/mcrs/reports/registries.json`) for the id.
`source_suggested` for the properties. Kind: wire. Confidence: medium. Experiment: decode the
`execute` tree of a captured `commands` packet. The node `block` under `if block <pos>` has the
parser id 13, no property bytes, and no flag `0x10`.

---

## 6. The related type `minecraft:block_state`

### ARG-BLK-018: What `block_state` changes

`minecraft:block_state` (parser id 12, no properties) reads with the rules of the block form.
This rule lists only the differences. A later specification of that type can cite
ARG-BLK-001, ARG-BLK-002, ARG-BLK-004, ARG-BLK-005 and ARG-BLK-007.

- **No tags:** if next is `#`, fail with `argument.block.tag.disallowed` at the `#`. The tag
  form and ARG-BLK-006 do not apply.
- **Suggestions:** the stage "start" suggests only the blocks, with no tags.
- **Value:** the value is one complete block state. Each property that the list does not name
  has its value in the default block state of the block. A command that places the value uses
  that complete state. A command that tests a block with the value uses ARG-BLK-009.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `setblock ~ ~ ~ #minecraft:logs`. The reply has `argument.block.tag.disallowed`,
and its context text ends before `#`.

---

## 7. Examples

`stone`, `minecraft:stone`, `stone[foo=bar]`, `#stone`, `#stone[foo=bar]{baz=nbt}`.

These are the examples that the type declares (`command-parser.md`, CMD-TREE-006). On a vanilla
server, only `stone` and `minecraft:stone` read without an error: the block `minecraft:stone`
has no property `foo`, and no block tag `minecraft:stone` exists.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `execute if block ~ ~ ~ <x>` for each example `<x>`. Compare the replies with the
rules of section 1.

---

## 8. Open questions

- **`,` and `]` in the stage "tag, value".** The reference suggests them with the range of the
  value text, so a selection replaces the value. Whether a later revision limits them to an
  empty remaining input. Experiment: in the vanilla client, type `/execute if block ~ ~ ~
  #minecraft:logs[axis=y`. Select `]` from the list. The command line then ends in `[axis=]`.

---

## 9. Intentional deviations

### ARG-BLK-DEV-001: The errors of a tag property list name the tag

- **Rule:** ARG-BLK-006.
- **Reference:** the errors `argument.block.property.duplicate` and
  `argument.block.property.novalue` of a tag property list give the text `minecraft:` in place
  of the tag text. The read of a tag does not set the block identifier, and `minecraft:` is its
  initial value.
- **Visible to the client:** yes. The message from the server names the tag, for example
  `#minecraft:logs`. The message that the client shows for its own parse names `minecraft:`.
- **Test:** a differential test accepts `minecraft:` from the reference where the
  implementation gives the tag text.

### ARG-BLK-DEV-002: An integer value text has ASCII digits only

- **Rule:** ARG-BLK-005.
- **Reference:** the game reads an integer value text with the integer parser of the Java
  library. That parser also accepts the decimal digits of other scripts, for example `٧`
  (U+0667). Only a quoted value can contain such a digit.
- **Visible to the client:** yes, but only for unusual input. The client accepts
  `wheat[age="٧"]`. The server rejects it with `argument.block.property.invalid`. The grammar
  policy of `README.md`, section "Semantics, not accidents", allows this difference.
- **Test:** a differential test uses only ASCII digits in an integer value text.
