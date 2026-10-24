# The `minecraft:entity` argument: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |
| Parser id | 6 (`commands-packet.md`, CMDP-ARG-001) |

Source of the version values: `assets/minecraft/version.json`. Source of the parser ids:
`assets/mcrs/reports/registries.json`, through `commands-packet.md`, CMDP-ARG-001.

This document states the argument type `minecraft:entity`: the text it accepts, the selector
that it reads, the errors and their positions, its suggestions, and the entities that a
selector finds when a command runs. It does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Reader**, **cursor**, **position**, **whitespace**, **error**, **source**, **suggestion**:
  as `command-parser.md`, section 0.1, defines them.
- **Selector**: the value that this argument reads. It describes a set of entities.
- **Target selector**: a selector that starts with `@`.
- **Selector kind**: the letter after `@`.
- **Option**: a `key=value` pair inside `[` and `]` after the selector kind.
- **Name selector**: a selector that is a player name.
- **UUID selector**: a selector that is an entity UUID.
- **Amount flag**: the property `single` of the argument. When it is set, the argument accepts
  only a selector with at most one result.
- **Players flag**: the property `players only` of the argument.
- **Maximum results**: the most entities that a selector can find.
- **Includes entities**: a flag of a selector. When it is false, the selector can find only
  players.
- **World limited**: a flag of a selector. When it is true, the selector searches only the
  dimension of the source.
- **Origin**: the position from which a selector measures distance and places its box.
- **Range**: a pair of an optional minimum and an optional maximum (ARG-ENT-007).
- **Selector permission**: the atom `minecraft:commands/entity_selectors`
  (`command-permissions.md`, PERM-CHECK-003).

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. None of the experiments has been run.

### 0.3 The four forms of the argument

| Form | Amount flag | Players flag | Wire byte | Vanilla use |
|------|-------------|--------------|-----------|-------------|
| one entity | set | clear | `0x01` | `teleport <destination>` |
| entities | clear | clear | `0x00` | `teleport <targets>` |
| one player | set | set | `0x03` | `msg` |
| players | clear | set | `0x02` | `gamemode <target>` |

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `ARG-ENT` | the selector syntax, the checks of the argument, the search |

---

## 1. Syntax

### ARG-ENT-001: Top level

- **Behavior:** note the start of the argument.
  1. If next is `@`, read a target selector (ARG-ENT-002).
  2. Otherwise read a name or a UUID (ARG-ENT-003).
  3. Apply the checks of the argument (ARG-ENT-010).

Provenance: `source_suggested`. Confidence: high.

### ARG-ENT-002: Target selector

- **Behavior:**
  1. If the source does not hold the selector permission, fail with
     `argument.entity.selector.not_allowed` at the `@`.
  2. Skip the `@`. At the end of the command line, fail with `argument.entity.selector.missing`
     at the end.
  3. Read one code unit, the selector kind:

| Kind | Maximum results | Includes entities | Order | Type limit | Must be alive |
|------|-----------------|-------------------|-------|------------|---------------|
| `p` | 1 | false | nearest | player | no |
| `a` | unlimited | false | arbitrary | player | no |
| `r` | 1 | false | random | player | no |
| `s` | 1 | true | arbitrary | none | no |
| `e` | unlimited | true | arbitrary | none | yes |
| `n` | 1 | true | nearest | none | yes |

     Any other code unit: set the cursor back to it. Fail with
     `argument.entity.selector.unknown`, with `@` and the code unit as its argument.
  4. `s` marks the selector as the current entity.
  5. If next is `[`, skip it and read the options (ARG-ENT-004).
- **Edge cases:** `@P` is an unknown kind: the kinds are lower case only. `@ea` fails at the
  next step of the parser, with `dispatcher_expected_argument_separator` after `@e`.

Provenance: `source_suggested`. Confidence: high.

### ARG-ENT-003: Name or UUID

- **Behavior:**
  1. Read a string (`command-parser.md`, CMD-RD-009). A quoted name is allowed.
  2. If the string is a UUID (ARG-ENT-005), the selector is a UUID selector. It includes
     entities.
  3. Otherwise, if the string is empty, or longer than 16 code units, set the cursor back to
     the start. Fail with `argument.entity.invalid`.
  4. Otherwise the selector is a name selector. It does not include entities.
  5. The maximum results is 1.
- **Edge cases:** an unquoted name stops at the first code unit that is not unquoted
  (`command-parser.md`, CMD-RD-006). `Steve!` reads `Steve` and fails at the next step of the
  parser.

Provenance: `source_suggested`. Confidence: high.

### ARG-ENT-004: Options

- **Behavior:** the cursor is after `[`.
  1. Skip whitespace.
  2. While the reader is not at the end and next is not `]`:
     1. Skip whitespace. Note the key start. Read a string: the key.
     2. Find the option of the key (ARG-ENT-006). If the key is unknown, set the cursor back to
        the key start, and fail with `argument.entity.options.unknown`, with the key. If the
        option is not available now (ARG-ENT-006), set the cursor back to the key start, and
        fail with `argument.entity.options.inapplicable`, with the key.
     3. Skip whitespace. If the reader is at the end, or next is not `=`, set the cursor back to
        the key start, and fail with `argument.entity.options.valueless`, with the key.
     4. Skip `=`. Skip whitespace. Read the value of the option (ARG-ENT-006).
     5. Skip whitespace. If the reader is at the end, go to step 3. If next is `,`, skip it and
        repeat step 2. If next is `]`, go to step 3. Otherwise fail with
        `argument.entity.options.unterminated` at the cursor.
  3. If the reader is at the end, fail with `argument.entity.options.unterminated` at the end.
     Otherwise skip `]`.
- **Edge cases:**

| Text | Result |
|------|--------|
| `@e[]` | no option |
| `@e[ type = cow ]` | one option |
| `@e[type=cow,]` | one option: a `,` before `]` is allowed |
| `@e[,type=cow]` | `argument.entity.options.unknown` with the empty key |
| `@e[type=cow` | `argument.entity.options.unterminated` at the end |
| `@e[type=cow x]` | `argument.entity.options.unterminated` at `x` |

Provenance: `source_suggested`. Confidence: high.

### ARG-ENT-005: UUID text

A string is a UUID when all of these are true:

- it has at most 36 code units;
- it has exactly four `-`, which split it into five fields;
- each field is an optional `+`, then one or more hexadecimal digits;
- the value of each field fits in a signed 64-bit integer.

The UUID keeps the low bits of each field: 32, 16, 16, 16 and 48 bits. The first three fields
make the high 64 bits, and the last two make the low 64 bits.

- **Edge cases:**

| Text | Result |
|------|--------|
| `dd12be42-52a9-4a91-a8a1-11c01849e498` | a UUID |
| `0-0-0-0-0` | the UUID with all bits zero |
| `+1-+2-3-4-5` | a UUID |
| `fffffffff-0-0-0-0` | a UUID: the first field keeps its low 32 bits |
| `0-0-0-0` | not a UUID; a name selector, because it has at most 16 code units |
| `00000000-0000-0000-0000-0000000000000` | not a UUID: 37 code units; and too long for a name |

A quoted string can hold a hexadecimal digit outside ASCII, for example a full-width digit. The
game accepts such digits as hexadecimal digits. Section 5 has the open point.

Provenance: `source_suggested`. Confidence: medium. Experiment: as an operator on a vanilla
server, run `gamemode creative +1-+2-3-4-5` and `gamemode creative 0-0-0-0`. The first reply
has `argument.player.entities`, because a UUID selector includes entities. The second reply has
`argument.entity.notfound.player`, because `0-0-0-0` is a player name.

### ARG-ENT-006: The options

An option is available when its condition in the table is true. The value rules follow the
table.

| Key | Value | Available when | Effect |
|-----|-------|----------------|--------|
| `name` | invertible string | invertible state allows any | the plain name equals the value |
| `distance` | range of doubles | no `distance` yet | distance from the origin; world limited |
| `level` | range of integers | no `level` yet | player experience level; includes entities false |
| `x`, `y`, `z` | double | that key not set yet | origin component; world limited |
| `dx`, `dy`, `dz` | double | that key not set yet | box size; world limited |
| `x_rotation` | range of floats | no `x_rotation` yet | pitch range |
| `y_rotation` | range of floats | no `y_rotation` yet | yaw range |
| `limit` | integer | not current entity, and no `limit` yet | maximum results |
| `sort` | word | not current entity, and no `sort` yet | order |
| `gamemode` | invertible word | invertible state allows any | player game mode |
| `team` | invertible word | invertible state allows any | team name |
| `type` | invertible identifier or tag | invertible state allows any | entity type |
| `tag` | invertible word | always | entity tag |
| `nbt` | invertible compound | always | entity data |
| `scores` | score map | no `scores` yet | scores |
| `advancements` | advancement map | no `advancements` yet | advancement progress |
| `predicate` | invertible identifier | always | loot predicate |

An invertible value starts with an optional `!`, with optional whitespace before and after it.
The invertible state of `name`, `gamemode`, `team` and `type` follows ARG-ENT-008.

Value rules. "Start" is the cursor before the optional `!`.

- **`name`:** read the `!`. Read a string. If the invertible state does not allow the element,
  set the cursor back to the start, and fail with `argument.entity.options.inapplicable`, with
  `name`.
- **`distance`:** read a range of doubles (ARG-ENT-007). If the minimum or the maximum is
  negative, set the cursor back to the start, and fail with
  `argument.entity.options.distance.negative`.
- **`level`:** read a range of integers. If the minimum or the maximum is negative, set the
  cursor back to the start, and fail with `argument.entity.options.level.negative`.
- **`x`, `y`, `z`, `dx`, `dy`, `dz`:** read a double (`command-parser.md`, CMD-RD-004).
- **`x_rotation`, `y_rotation`:** read a range of floats. A minimum above the maximum is
  allowed: it selects a range that wraps around 180.
- **`limit`:** read an integer. If it is less than 1, set the cursor back to the start, and fail
  with `argument.entity.options.limit.toosmall`.
- **`sort`:** read an unquoted string. It must be `nearest`, `furthest`, `random` or
  `arbitrary`. Otherwise set the cursor back to the start, and fail with
  `argument.entity.options.sort.irreversible`, with the word.
- **`gamemode`:** read the `!`. If the invertible state does not allow the element, set the
  cursor back to the start, and fail with `argument.entity.options.inapplicable`, with
  `gamemode`. Read an unquoted string. It must be `survival`, `creative`, `adventure` or
  `spectator`. Otherwise set the cursor back to the start, and fail with
  `argument.entity.options.mode.invalid`, with the word. The selector no longer includes
  entities.
- **`team`:** read the `!`. Read an unquoted string. If the invertible state does not allow the
  element, set the cursor back to the start, and fail with
  `argument.entity.options.inapplicable`, with `team`. The empty word matches an entity with no
  team.
- **`type`:** read the `!`. Skip whitespace. If next is `#`, skip it and skip whitespace:
  - Tag: if the state allows no tag, set the cursor back to the start, and fail with
    `argument.entity.options.inapplicable`, with `type`. Read an identifier
    (`argument-resource.md`, ARG-RES-001). If the state does not allow this tag, set the cursor
    back to the start, and fail in the same way.
  - Element: if the state does not allow the element, fail in the same way. Read an identifier.
    If it is not an entity type, set the cursor back to the start, and fail with
    `argument.entity.options.type.invalid`, with the identifier. A positive `type=player`
    makes the selector not include entities. A positive type sets the type limit.
- **`tag`:** read the `!`. Read an unquoted string. The empty word matches an entity with no
  tag.
- **`nbt`:** read the `!`. Read a compound tag in the text form of NBT. Section 5 has the open
  point.
- **`scores`:** expect `{`. Skip whitespace. While next is not `}`: skip whitespace, read an
  unquoted string (the objective), skip whitespace, expect `=`, skip whitespace, read a range of
  integers, skip whitespace, and skip one `,` if next is `,`. Then expect `}`. A later entry
  for the same objective replaces the earlier one.
- **`advancements`:** expect `{`. Skip whitespace. While next is not `}`: skip whitespace, read
  an identifier, skip whitespace, expect `=`, skip whitespace. Then read a boolean, or a
  criterion map: `{`, then entries of an unquoted string, `=` and a boolean, with the same
  whitespace and `,` rules, then `}`. Skip whitespace, and skip one `,` if next is `,`. Then
  expect `}`. A non-empty map makes the selector not include entities.
- **`predicate`:** read the `!`. Read an identifier.

Provenance: `source_suggested`. Confidence: medium. Experiment: on a vanilla server, run
`kill @e[limit=0]`, `kill @e[sort=up]` and `kill @e[type=!player,type=!cow,type=pig]`. The
replies have `argument.entity.options.limit.toosmall`, `argument.entity.options.sort.irreversible`
and `argument.entity.options.inapplicable`.

### ARG-ENT-007: Ranges

- **Behavior:** note the start.
  1. At the end of the command line, fail with `argument.range.empty` at the end.
  2. Read a number: the longest run of `0` to `9`, `-`, and `.` that is not followed by
     another `.`.
  3. If the next two code units are `..`, skip them and read a second number. Otherwise the
     second number equals the first.
  4. If both numbers are empty, fail with `argument.range.empty`.
  5. A number that does not parse fails with the `reader_invalid_*` error of its type
     (`command-parser.md`, CMD-ERR-002), with the number text.
  6. For integers and doubles: if both bounds exist and the minimum is greater than the
     maximum, fail with `argument.range.swapped`.
  7. Every error of this rule has the position of the start.
- **Edge cases:**

| Text | Minimum | Maximum |
|------|---------|---------|
| `5` | 5 | 5 |
| `5..` | 5 | none |
| `..5` | none | 5 |
| `1..5` | 1 | 5 |
| `..` | `argument.range.empty` | |
| `5..1` | `argument.range.swapped`, except for rotations | |
| `1.5..2` | 1.5 | 2 |

  The number grammar is CMD-RD-002 for integers, CMD-RD-004 for doubles, and CMD-RD-005 for
  floats.

Provenance: `source_suggested`. Confidence: high.

### ARG-ENT-008: Invertible state

- **Behavior:** each of `name`, `gamemode`, `team` and `type` has a state: none, single or
  multiple. It starts at none.
  - A positive element is allowed only in the state none. It sets the state to single.
  - A negative element is allowed unless the state is single. It sets the state to multiple.
  - A tag (only `type`) is allowed unless the state is single, and unless the same tag
    identifier was already read, with or without `!`. It sets the state to multiple.
  - The option is available (ARG-ENT-006) unless the state is single.
- **Edge cases:** `@e[type=!cow,type=!pig]` is allowed. `@e[type=cow,type=pig]` fails at the
  second key. `@e[type=!cow,type=pig]` fails at the second value.

Provenance: `source_suggested`. Confidence: high.

### ARG-ENT-009: Selector result fields

After the options, the selector has these fields:

- the maximum results, the includes-entities flag, the world-limited flag, the order, the type
  limit, and the current-entity flag;
- the origin: the source position, with each of `x`, `y` and `z` replaced when set;
- the box: if `dx`, `dy` or `dz` is set, a box from `min(0, d)` to `max(0, d) + 1` on each axis,
  with `d` = 0 for an axis that is not set. Otherwise, if `distance` has a maximum `m`, the box
  from `−m` to `m + 1` on each axis. Otherwise no box. The box is placed at the origin;
- the predicates, in the order of the options. After the options, the parser adds the pitch
  test, then the yaw test, then the level test.

Provenance: `source_suggested`. Confidence: high.

### ARG-ENT-010: Checks of the argument

- **Behavior:** after a selector is read:
  1. If the amount flag is set and the maximum results is more than 1, fail with
     `argument.player.toomany` when the players flag is set, or `argument.entity.toomany`
     otherwise.
  2. If the players flag is set, the selector includes entities, and it is not `@s`, fail with
     `argument.player.entities`.
- **Postconditions:** these errors have the position 0 of the command line, not the position of
  the argument.

Provenance: `source_suggested`. Confidence: medium. Experiment: as an operator, send
`gamemode creative @e`. The reply has `argument.player.entities`. Its context line
(`commands-packet.md`, CMDP-EXEC-003) shows the whole command line in red and underlined.

---

## 2. Suggestions

### ARG-ENT-011: Suggestions

- **Behavior:** read the remaining input from the start of the argument with ARG-ENT-001,
  ignore an error, and suggest by the state where the read stopped:
  - before anything: the online player names, then, when the players flag is clear, the
    UUID of the entity under the crosshair of the client, then `@p`, `@a`, `@r`, `@s`, `@e` and `@n` when the source
    holds the selector permission;
  - in a name: the names as above, from the start of the argument;
  - after `@`: the six kinds, from the `@`;
  - after a complete kind: `[`;
  - after `[`: `]` and the option keys with `=`;
  - after a `,`: the option keys with `=`;
  - after a value: `,` and `]`;
  - inside the value of `sort`, `gamemode` or `type`: the values that are allowed now. For
    `gamemode`, a negative form `!` and the mode. For `type`: `!` and each entity type, each
    entity type, `#` and each tag, and `!#` and each tag, as the invertible state allows.
- **Postconditions:** an option key is suggested only when the option is available. A selector
  kind has a tooltip with the key `argument.entity.selector.<description>`: `nearestPlayer`,
  `allPlayers`, `randomPlayer`, `self`, `allEntities` and `nearestEntity`. An option key has a
  tooltip with the key `argument.entity.options.<key>.description`.

Provenance: `source_suggested`. Confidence: medium. The client makes these suggestions itself.
The server makes them only for an `ask_server` request on a sibling node
(`commands-packet.md`, CMDP-SUG-002).

---

## 3. The search

### ARG-ENT-012: Permission when the command runs

When a selector is a target selector and the source does not hold the selector permission, the
search fails with `argument.entity.selector.not_allowed`.

Provenance: `source_suggested`. Confidence: medium.

### ARG-ENT-013: Find entities

- **Behavior:**
  1. If the selector does not include entities, find players (ARG-ENT-014).
  2. A name selector: the online player whose name equals the name without regard to case, or
     nothing.
  3. A UUID selector: the entity with that UUID in any dimension, if its type is enabled.
  4. `@s`: the entity of the source, if it passes every predicate and the box and distance
     tests. Otherwise nothing.
  5. Otherwise: collect each entity that passes all tests, from the dimension of the source when
     the selector is world limited, or from every dimension in order. When the order is
     arbitrary, stop at the maximum results. Then sort by the order and keep the first maximum
     results.
- **Tests:** every predicate; the type limit; the type is enabled; the bounding box of the
  entity intersects the placed box; the squared distance from the origin is in the squared
  `distance` range.
- **Orders:** nearest sorts by squared distance from the origin, ascending. Furthest sorts
  descending. Random shuffles. Arbitrary keeps the order of collection. Equal distances keep
  the order of collection.

Provenance: `source_suggested`. Confidence: medium.

### ARG-ENT-014: Find players

- **Behavior:**
  1. A name selector: as ARG-ENT-013.
  2. A UUID selector: the online player with that UUID, or nothing.
  3. `@s`: the source entity, if it is a player and passes every test.
  4. Otherwise: test each player of the dimension of the source when the selector is world
     limited, or each online player in join order. Stop at the maximum results when the order
     is arbitrary. Sort and keep the first maximum results.
- **Postconditions:** the type limit does not apply here. The predicates of `type` still apply.

Provenance: `source_suggested`. Confidence: medium.

### ARG-ENT-015: Predicates of the options

| Option | An entity passes when |
|--------|-----------------------|
| `name` | its plain text name equals the value, case included; the inverse when `!` |
| `level` | it is a player and its experience level is in the range |
| `x_rotation` | its pitch, wrapped to `[−180, 180)`, is in the wrapped range. A missing minimum is 0, a missing maximum is 359. When the wrapped minimum is greater than the wrapped maximum, the pitch must be at least the minimum or at most the maximum |
| `y_rotation` | the same test with its yaw |
| `gamemode` | it is a player and its game mode equals the value; for `!`, it is a player with another mode |
| `team` | the name of its team, or the empty string, equals the value; the inverse when `!` |
| `type` | its type equals the value, or is in the tag; the inverse when `!` |
| `tag` | it has the tag; for the empty word, it has no tag; the inverse when `!` |
| `nbt` | its saved data contains the compound; the inverse when `!` |
| `scores` | each objective exists, the entity has a score in it, and the score is in the range |
| `advancements` | it is a player, and each advancement exists and its progress matches |
| `predicate` | the loot predicate exists and passes, with the entity as `this` and its position as `origin`; the inverse when `!` |
| alive, for `@e` and `@n` | the entity is alive |

The saved data for `nbt` is the data of the entity without its id. For a player, it also has
`SelectedItem` with the selected item when that item is not empty.

Provenance: `source_suggested`. Confidence: medium.

### ARG-ENT-016: Errors of the commands that read the argument

| Situation | Translation key |
|-----------|-----------------|
| the command needs one entity and finds none | `argument.entity.notfound.entity` |
| the command needs one entity and finds more than one | `argument.entity.toomany` |
| the command needs one player and does not find exactly one | `argument.entity.notfound.player` |
| the command needs entities and finds none | `argument.entity.notfound.entity` |
| the command needs players and finds none | `argument.entity.notfound.player` |

Provenance: `source_suggested`. Confidence: medium.

---

## 4. Examples

`Player`, `0123`, `@e`, `@e[type=foo]`, `dd12be42-52a9-4a91-a8a1-11c01849e498`.

Provenance: `source_suggested`. Confidence: high.

---

## 5. Open questions

- The value of `nbt` uses the text form of NBT. Its grammar needs its own specification. Until
  it exists, an implementation cannot accept the same set of `nbt` values as the client.
- ARG-ENT-005: the set of non-ASCII code units that the game accepts as hexadecimal digits in a
  quoted UUID. Experiment: run `tp "１-0-0-0-0"` with a full-width digit, and read the reply.
- ARG-ENT-015: the plain text name of an entity with a custom name that has styles or
  translations. This needs the text specification.
