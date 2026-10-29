# The `minecraft:entity` argument: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |
| Parser id | 6 (`commands-packet.md`, CMDP-ARG-001) |

Source of the version values: `assets/minecraft/version.json`. Source of the parser ids:
`assets/mcrs/reports/registries.json`, through `commands-packet.md`, CMDP-ARG-001.

This document states the argument type `minecraft:entity`. It states the text that the type
accepts, the selector that it reads, the errors and their positions, and its suggestions. It
also states the entities that a selector finds when a command runs. It does not prescribe an
architecture.

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
  dimension of the source (ARG-ENT-018).
- **Source dimension**: the dimension of the source. `execute in` and `execute at` change it.
  It can differ from the dimension of the source entity.
- **Dimension order**: the order in which a search visits the dimensions of the server
  (ARG-ENT-019).
- **Origin**: the position from which a selector measures distance and places its box.
- **Bounds**: a pair of an optional minimum and an optional maximum (ARG-ENT-007).
- **Selector permission**: the atom `minecraft:commands/entity_selectors`
  (`command-permissions.md`, PERM-CHECK-003).

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. None of the experiments has been run.

### 0.3 The four forms of the argument

| Form | Amount flag | Players flag | Wire byte | Vanilla use |
|------|-------------|--------------|-----------|-------------|
| one entity | set | clear | `0x01` | `teleport <destination>` |
| entities | clear | clear | `0x00` | `teleport <targets>` |
| one player | set | set | `0x03` | `spectate <target> <player>`, `experience query` |
| players | clear | set | `0x02` | `gamemode <target>` |

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `ARG-ENT` | the selector syntax, the checks of the argument, the search |
| `ARG-ENT-DEV` | intentional deviations from the reference |

---

## 1. Syntax

### ARG-ENT-001: Top level

- **Behavior:** note the start of the argument.
  1. If next is `@`, read a target selector (ARG-ENT-002).
  2. Otherwise read a name or a UUID (ARG-ENT-003).
  3. Apply the checks of the argument (ARG-ENT-010).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator with a player `Steve` online, send `teleport Steve ~ ~ ~` and `teleport @s ~ ~ ~`. Both
succeed: the first reads a name, the second a target selector.

### ARG-ENT-002: Target selector

- **Behavior:**
  1. If the source does not hold the selector permission, fail with
     `argument.entity.selector.not_allowed` at the `@`.
  2. Skip the `@`. At the end of the command line, fail with `argument.entity.selector.missing`
     at the end.
  3. Read one character, the selector kind:

| Kind | Maximum results | Includes entities | Order | Type limit | Must be alive |
|------|-----------------|-------------------|-------|------------|---------------|
| `p` | 1 | false | nearest | player | no |
| `a` | unlimited | false | arbitrary | player | no |
| `r` | 1 | false | random | player | no |
| `s` | 1 | true | arbitrary | none | no |
| `e` | unlimited | true | arbitrary | none | yes |
| `n` | 1 | true | nearest | none | yes |

     Any other character: set the cursor back to it. Fail with
     `argument.entity.selector.unknown`, with `@` and the character as its argument.
  4. `s` marks the selector as the current entity.
  5. If next is `[`, skip it and read the options (ARG-ENT-004).
- **Edge cases:** `@P` is an unknown kind: the kinds are lower case only. `@ea` fails at the
  next step of the parser, with `dispatcher_expected_argument_separator` after `@e`.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `kill @x` and `kill @`. The replies have `argument.entity.selector.unknown` with
`@x`, and `argument.entity.selector.missing`. Then send `kill @a` while a pig is near: the pig
stays, because `@a` finds only players.

### ARG-ENT-003: Name or UUID

- **Behavior:**
  1. Read a string (`command-parser.md`, CMD-RD-009). A quoted name is allowed.
  2. If the string is a UUID (ARG-ENT-005), the selector is a UUID selector. It includes
     entities. If the string has exactly four `-` and is not a UUID, fail as ARG-ENT-005
     states.
  3. Otherwise, if the string is empty, or longer than 16 bytes in UTF-8, set the cursor back
     to the start. Fail with `argument.entity.invalid` (ARG-ENT-DEV-008).
  4. Otherwise the selector is a name selector. It does not include entities.
  5. The maximum results is 1.
- **Edge cases:** an unquoted name stops at the first character that is not unquoted
  (`command-parser.md`, CMD-RD-006). `Steve!` reads `Steve` and fails at the next step of the
  parser.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `kill abcdefghijklmnopq`, a name of 17 bytes. The reply has
`argument.entity.invalid`. With a player `Steve` online, send `kill "Steve"`: the player dies.

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
        the key start. Then fail with `argument.entity.options.valueless`, with the key.
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

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `kill @e[type=cow,]` and `kill @e[type=cow`. The first kills each cow. The second
reply has `argument.entity.options.unterminated`, and its context ends at the end of the line.

### ARG-ENT-005: UUID text

- **Behavior:** a string is a UUID when it has exactly four `-`, and these split it into five
  groups of 8, 4, 4, 4 and 12 hexadecimal digits. The string then has 36 characters. A
  hexadecimal digit is `0` to `9`, `a` to `f` or `A` to `F`.
  - The first three groups make the high 64 bits of the UUID, in the order of the text. The
    last two groups make the low 64 bits.
  - A string with exactly four `-` that is not a UUID must not become a name selector. Set the
    cursor back to the start, and fail with `argument.entity.invalid` (ARG-ENT-DEV-001).
- **Edge cases:**

| Text | Result |
|------|--------|
| `dd12be42-52a9-4a91-a8a1-11c01849e498` | a UUID |
| `DD12BE42-52A9-4A91-A8A1-11C01849E498` | the same UUID |
| `00000000-0000-0000-0000-000000000000` | the UUID with all bits zero |
| `0-0-0-0-1` | `argument.entity.invalid`: the groups are short (ARG-ENT-DEV-001) |
| `+1-+2-3-4-5` | `argument.entity.invalid` (ARG-ENT-DEV-001) |
| `fffffffff-0-0-0-0` | `argument.entity.invalid` (ARG-ENT-DEV-001) |
| `0-0-0-0` | not a UUID: three `-`; a name selector |
| `00000000-0000-0000-0000-0000000000000` | `argument.entity.invalid`: the last group has 13 digits |

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator on a vanilla server, run `gamemode creative 00000000-0000-0000-0000-000000000001` and
`gamemode creative 0-0-0-0`.
The first reply has `argument.player.entities`, because a UUID selector includes entities. The
second reply has `argument.entity.notfound.player`, because `0-0-0-0` is a player name.

### ARG-ENT-006: The options

An option is available when its condition in the table is true. The value rules follow the
table.

| Key | Value | Available when | Effect |
|-----|-------|----------------|--------|
| `name` | invertible string | invertible state allows any | the plain name equals the value |
| `distance` | bounds of doubles | no `distance` yet | distance from the origin; world limited |
| `level` | bounds of integers | no `level` yet | player experience level; includes entities false |
| `x`, `y`, `z` | double | that key not set yet | origin component; world limited |
| `dx`, `dy`, `dz` | double | that key not set yet | box size; world limited |
| `x_rotation` | bounds of floats | no `x_rotation` yet | pitch bounds |
| `y_rotation` | bounds of floats | no `y_rotation` yet | yaw bounds |
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
- **`distance`:** read bounds of doubles (ARG-ENT-007). If the minimum or the maximum is
  negative, set the cursor back to the start, and fail with
  `argument.entity.options.distance.negative`.
- **`level`:** read bounds of integers. If the minimum or the maximum is negative, set the
  cursor back to the start, and fail with `argument.entity.options.level.negative`.
- **`x`, `y`, `z`, `dx`, `dy`, `dz`:** read a double (`command-parser.md`, CMD-RD-004).
- **`x_rotation`, `y_rotation`:** read bounds of floats. A minimum above the maximum is
  allowed (ARG-ENT-007). ARG-ENT-015 wraps both bounds, and then decides which angles the
  bounds select. Thus `y_rotation=350..10` selects the arc through 0.
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
- **`nbt`:** read the `!`. Read a compound tag in the text form of NBT (`snbt.md`,
  SNBT-ARG-002 and SNBT-ARG-003).
- **`scores`:** expect `{`. Skip whitespace. While next is not `}`, read one entry:
  - skip whitespace, read an unquoted string (the objective), and skip whitespace;
  - expect `=`, skip whitespace, read bounds of integers, and skip whitespace;
  - if next is `,`, skip one `,`.

  Then expect `}`. A later entry for the same objective replaces the earlier one.
- **`advancements`:** expect `{`. Skip whitespace. While next is not `}`: skip whitespace, read
  an identifier, skip whitespace, expect `=`, skip whitespace. Then read a boolean, or a
  criterion map: `{`, then entries of an unquoted string, `=` and a boolean, with the same
  whitespace and `,` rules, then `}`. Skip whitespace, and skip one `,` if next is `,`. Then
  expect `}`. A non-empty map makes the selector not include entities.
- **`predicate`:** read the `!`. Read an identifier.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: on a
vanilla server, run `kill @e[limit=0]`, `kill @e[sort=up]` and
`kill @e[type=!player,type=!cow,type=pig]`. The replies have
`argument.entity.options.limit.toosmall`, `argument.entity.options.sort.irreversible` and
`argument.entity.options.inapplicable`.

### ARG-ENT-007: Bounds

- **Behavior:** note the start.
  1. At the end of the command line, fail with `argument.range.empty` at the end.
  2. Read a number: the longest run of `0` to `9`, `-`, and `.` that is not followed by
     another `.`.
  3. If the next two characters are `..`, skip them and read a second number. Otherwise the
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

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `kill @e[distance=5..1]` and `kill @e[distance=..]`. The replies have
`argument.range.swapped` and `argument.range.empty`. Then send `kill @e[type=pig,distance=..5]`:
only the pigs within 5 blocks die.

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

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `kill @e[type=!cow,type=!pig,type=!player]`: every entity except cows, pigs and
players dies. Send `kill @e[type=cow,type=pig]`: the reply has
`argument.entity.options.inapplicable` with `type`.

### ARG-ENT-009: Selector result fields

After the options, the selector has these fields:

- the maximum results, the includes-entities flag, the world-limited flag, the order, the type
  limit, and the current-entity flag;
- the origin: the source position, with each of `x`, `y` and `z` replaced when set;
- the box: if `dx`, `dy` or `dz` is set, the box is from `min(0, d)` to `max(0, d) + 1` on each
  axis. `d` is 0 for an axis that is not set. Otherwise there is no box. The box is placed at
  the origin;
- the tests: the alive test for `@e` and `@n`, one test for each option, and the tests of
  ARG-ENT-013.

The order of the tests has no observable effect and is unspecified. An implementation can test
in any order, and can use a box around the `distance` bounds to find candidates. Such a box must
not change the result of ARG-ENT-013 (ARG-ENT-DEV-004).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator,
summon a pig at `0 64 0`. From 20 blocks away, send `kill
@e[type=pig,x=5,y=64,z=0,dx=1,dy=1,dz=1]`: the pig stays. Send `kill
@e[type=pig,x=0,y=64,z=0,dx=1,dy=1,dz=1]`: the pig dies.

### ARG-ENT-010: Checks of the argument

- **Behavior:** after a selector is read:
  1. If the amount flag is set and the maximum results is more than 1, fail. When the players
     flag is set, the error is `argument.player.toomany`. Otherwise it is
     `argument.entity.toomany`.
  2. If the players flag is set, the selector includes entities, and it is not `@s`, fail with
     `argument.player.entities`.
- **Postconditions:** these errors have the position of the start of the argument
  (ARG-ENT-DEV-002).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `gamemode creative @e`. The reply has `argument.player.entities`.

---

## 2. Suggestions

### ARG-ENT-011: Suggestions

- **Behavior:** read the remaining input from the start of the argument with ARG-ENT-001,
  ignore an error, and suggest by the state where the read stopped:
  - before anything: first the online player names. When the players flag is clear, then the
    UUID of the entity under the crosshair of the client. When the source holds the selector
    permission, then `@p`, `@a`, `@r`, `@s`, `@e` and `@n`;
  - in a name: the names as above, from the start of the argument;
  - after `@`: the six kinds, from the `@`;
  - after a complete kind: `[`;
  - after `[`: the option keys with `=`;
  - after a `,`: the option keys with `=`;
  - after a value: `,` and `]`;
  - inside the value of `sort`, `gamemode` or `type`: the values that are allowed now. For
    `gamemode`, a negative form `!` and the mode. For `type`: `!` and each entity type, each
    entity type, `#` and each tag, and `!#` and each tag, as the invertible state allows.
- **Postconditions:** an option key is suggested only when the option is available. A selector
  kind has a tooltip with the key `argument.entity.selector.<description>`: `nearestPlayer`,
  `allPlayers`, `randomPlayer`, `self`, `allEntities` and `nearestEntity`. An option key has a
  tooltip with the key `argument.entity.options.<key>.description`. The order in which the
  argument adds suggestions is unspecified: `command-parser.md`, CMD-SUG-006, sets the final
  order.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. The client makes these
suggestions itself. The server makes them when it answers a suggestion request
(`commands-packet.md`, CMDP-SUG-001 and CMDP-SUG-002). Experiment: in the vanilla client, type
`/kill @e[`. The list has the option keys with `=`, for example `type=`, and no `]`.

---

## 3. The search

### ARG-ENT-012: Permission when the command runs

When a selector is a target selector and the source does not hold the selector permission, the
search fails with `argument.entity.selector.not_allowed`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### ARG-ENT-013: Find entities

- **Behavior:**
  1. If the selector does not include entities, find players (ARG-ENT-014).
  2. A name selector: the online player whose name equals the name without regard to case, or
     nothing.
  3. A UUID selector: the entity with that UUID in any dimension, if its type is enabled.
  4. `@s`: the entity of the source, if it passes every test of the options, the box test and
     the distance test. Otherwise nothing.
  5. Otherwise: find each entity that passes all tests, in the dimension of the source when the
     selector is world limited, or in every dimension in the dimension order (ARG-ENT-019). Put
     the entities in the order of the selector, and keep the first maximum results.
- **Tests:**
  - the alive test and the tests of the options (ARG-ENT-015);
  - the type limit, and the type is enabled;
  - the box test;
  - the distance from the origin to the entity position is in the `distance` bounds.
- **Box test:** an entity passes when its bounding box overlaps the placed box. Two boxes that
  only touch do not overlap. An entity with an empty bounding box passes when its position is
  in the placed box, faces included (ARG-ENT-DEV-003).
- **Orders:**
  - `nearest` sorts by distance from the origin (ARG-ENT-020), the smallest first. `furthest`
    sorts by distance, the largest first. The order of entities at equal distance is unspecified.
  - `random` gives each order of the entities the same probability.
  - `arbitrary` keeps any entities that pass, up to the maximum results, in an order that the
    implementation selects. For the same world and the same command, the result must be the
    same.
- **Postconditions:** these unspecified orders decide which entity `limit=1`, `@p` or `@n`
  picks in a tie, and which entities `arbitrary` keeps (ARG-ENT-DEV-007).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### ARG-ENT-014: Find players

- **Behavior:**
  1. A name selector: as ARG-ENT-013.
  2. A UUID selector: the online player with that UUID, or nothing.
  3. `@s`: the source entity, if it is a player and passes every test.
  4. Otherwise: test each player of the dimension of the source when the selector is world
     limited, or each online player in every dimension (ARG-ENT-019). Put the players in the
     order of the selector (ARG-ENT-013), and keep the first maximum results.
- **Postconditions:** the type limit does not apply here. The tests of `type` still apply. The
  order of the players for `arbitrary` and for equal distances is unspecified
  (ARG-ENT-DEV-007).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### ARG-ENT-015: Predicates of the options

| Option | An entity passes when |
|--------|-----------------------|
| `name` | its plain text name equals the value, case included; the inverse when `!` |
| `level` | it is a player and its experience level is in the bounds |
| `x_rotation` | its pitch, wrapped to `[−180, 180)`, is in the bounds. Each bound that is present is wrapped to `[−180, 180)`. An absent bound sets no limit on that side. When both bounds are present and the wrapped minimum is greater than the wrapped maximum, the pitch must be at least the minimum or at most the maximum (ARG-ENT-DEV-005) |
| `y_rotation` | its yaw, wrapped to `[0, 360)`, is on the arc from the minimum to the maximum, in the direction in which the angle increases, both ends included. Each bound that is present is wrapped to `[0, 360)`. An absent minimum is 0. When the maximum is absent, the arc ends just below 360 (ARG-ENT-DEV-005) |
| `gamemode` | it is a player and its game mode equals the value; for `!`, it is a player with another mode |
| `team` | the name of its team, or the empty string, equals the value; the inverse when `!` |
| `type` | its type equals the value, or is in the tag; the inverse when `!` |
| `tag` | it has the tag; for the empty word, it has no tag; the inverse when `!` |
| `nbt` | its saved data contains the compound; the inverse when `!` |
| `scores` | each objective exists, the entity has a score in it, and the score is in the bounds |
| `advancements` | it is a player, and each advancement exists and its progress matches |
| `predicate` | the loot predicate passes, with the entity as `this` and its position as `origin`; the inverse when `!`. When the loot predicate does not exist, the search fails (ARG-ENT-017) |
| alive, for `@e` and `@n` | the entity is alive |

The saved data for `nbt` is the data of the entity without its id. For a player, it also has
`SelectedItem` with the selected item when that item is not empty.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### ARG-ENT-016: Errors of the commands that read the argument

| Situation | Translation key |
|-----------|-----------------|
| the command needs one entity and finds none | `argument.entity.notfound.entity` |
| the command needs one entity and finds more than one | `argument.entity.toomany` |
| the command needs one player and does not find exactly one | `argument.entity.notfound.player` |
| the command needs entities and finds none | `argument.entity.notfound.entity` |
| the command needs players and finds none | `argument.entity.notfound.player` |

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### ARG-ENT-017: A loot predicate that does not exist

- **Behavior:** when the search starts, check each identifier of a `predicate` option. If the
  server has no loot predicate with that identifier, the search fails with
  `argument.resource.not_found`, with the identifier and `minecraft:predicate`. The search
  tests no entity. The error has no position.
- **Postconditions:** the parse does not check the identifier, because the client has no loot
  predicates.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. The run-time error corrects
a deviation (ARG-ENT-DEV-006).


### ARG-ENT-018: World limited selectors

- **Behavior:** a selector is world limited when it has at least one of these options:
  `distance`, `x`, `y`, `z`, `dx`, `dy` or `dz`. No other option and no selector kind makes a
  selector world limited.
- **Postconditions:**

| Selector | Without those options | With one of those options |
|----------|-----------------------|---------------------------|
| `@p`, `@n` | every dimension | the source dimension |
| `@a`, `@e`, `@r` | every dimension | the source dimension |
| `@s` | the source entity, in any dimension (ARG-ENT-021) | the same |
| a name selector | the online player, in any dimension | not applicable |
| a UUID selector | the entity, in any dimension | not applicable |

- **Edge cases:**
  - `@p` and `@n` are not world limited. They find the nearest candidate in every dimension,
    with the distance of ARG-ENT-020.
  - `@a[sort=nearest]`, `@e[limit=1]` and `@r[gamemode=creative]` search every dimension.
  - `@e[distance=0..]` is world limited, although every distance passes. It finds each entity
    of the source dimension.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: put the player
`A` in the Overworld at `0 64 0` and the player `B` in the Nether at `0 64 0`. As `A`, run
`execute if entity @a`: the count is 2. Run `execute if entity @a[distance=0..]`: the count is
1.

### ARG-ENT-019: The dimension order and the order of candidates

- **Behavior:**
  - The dimension order is `minecraft:overworld`, `minecraft:the_nether`, `minecraft:the_end`,
    then each other dimension of the server. A dimension that the server does not have is
    skipped.
  - A search for entities that is not world limited visits the dimensions in the dimension
    order. Each candidate of an earlier dimension comes before each candidate of a later
    dimension. Inside one dimension, the order is the order of ARG-ENT-013.
  - A search for players that is not world limited does not group the players by dimension.
    It visits the online players in the order of ARG-ENT-014, in all dimensions together.
  - The order `arbitrary` keeps the first candidates in this order. Thus `@e[limit=1]` finds an
    entity of the Overworld before an entity of the Nether.
  - The orders `nearest` and `furthest` keep this order for candidates at equal distance.
  - The order `random` gives each order of all candidates of all dimensions the same
    probability. Thus `@r` picks each player of the server with the same probability.
- **Edge cases:** the order of the dimensions after `minecraft:the_end` is unspecified. The
  order inside one dimension stays unspecified (ARG-ENT-DEV-007).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: summon a pig
with the tag `o` in the Overworld and a pig with the tag `o` in the Nether. From the Nether, run
`tag @e[tag=o,limit=1] add first`. Then run
`execute in minecraft:overworld if entity @e[tag=first,distance=0..]`. The reply is
`commands.execute.conditional.pass`.

### ARG-ENT-020: Distance to a candidate in another dimension

- **Behavior:** the distance from the origin to an entity is the Euclidean distance between
  the coordinates of the origin and the coordinates of the entity. The dimension of the entity
  does not change the distance. No coordinate scale applies.
- **Postconditions:**
  - `nearest` and `furthest` compare the candidates of every dimension with this distance.
    Thus `@p` in the Overworld at `0 64 0` finds a player in the Nether at `1 64 0` before a
    player in the Overworld at `5 64 0`.
  - A world limited selector has no candidate in another dimension, except `@s`
    (ARG-ENT-021). Thus the `distance` bounds and the box test apply to another dimension only
    for `@s`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: put the player
`A` in the Overworld at `0 64 0`, the player `B` in the Overworld at `5 64 0`, and the player
`C` in the Nether at `1 64 0`. As `A`, run `tp @p[name=!A] ~ ~ ~`. `C` moves to the Overworld.

### ARG-ENT-021: `@s` when the source entity is in another dimension

- **Behavior:**
  - `@s` finds the source entity in every dimension. The source dimension does not need to be
    the dimension of the source entity. `@s` does not search a dimension, so the world limited
    flag has no effect on it.
  - The options of `@s` test the source entity as ARG-ENT-013, step 4, states. The origin is the
    source position, which `execute in` scales (`command-execute.md`, CMD-EX-021). The
    distance and the box test use the coordinates alone (ARG-ENT-020).
- **Edge cases:**

| Situation, the player at `800 64 -160` in the Overworld | `@s` finds the player |
|----------------------------------------------------------|-----------------------|
| `execute in minecraft:the_end run ... @s` | yes |
| `execute in minecraft:the_nether run ... @s[distance=..1]`: the origin is `100 64 -20` | no |
| `execute in minecraft:the_end run ... @s[distance=..1]`: the scale is 1, the origin is the position of the player | yes |
| `execute in minecraft:the_nether positioned as @s run ... @s[distance=..1]` | yes |

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator
in the Overworld at `800 64 -160`, run
`execute in minecraft:the_nether if entity @s[distance=..1]`. The reply is
`commands.execute.conditional.fail`. Run
`execute in minecraft:the_end if entity @s[distance=..1]`. The reply is
`commands.execute.conditional.pass`.

---

## 4. Examples

`Player`, `0123`, `@e`, `@e[type=foo]`, `dd12be42-52a9-4a91-a8a1-11c01849e498`.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: in the
vanilla client, type each example after `/kill `. The client shows no error, except for
`@e[type=foo]`: it shows `argument.entity.options.type.invalid`, because `foo` is not an entity
type.

---

## 5. Open questions

- ARG-ENT-015: the plain text name of an entity with a custom name that has styles or
  translations. This needs the text specification.
- ARG-ENT-021: a source entity that is not a player and that changed dimension earlier in the
  same function. The source still holds the removed entity (`dimension-change.md`,
  DIM-CHG-009). Whether `@s` then finds it is not confirmed. Experiment: make a function with
  `execute in minecraft:the_nether run tp @s 0 64 0` and then `say @s`. Run it as a pig, and
  record the second line.
- ARG-ENT-019: the order of the dimensions after `minecraft:the_end` in a data pack with more
  dimensions. Experiment: add two dimensions `a:x` and `a:y`, summon a pig with a tag in each,
  and run `tag @e[tag=t,limit=1] add first` several times over several restarts.

---

## 6. Intentional deviations

### ARG-ENT-DEV-001: Malformed UUID text

- **Rule:** ARG-ENT-005.
- **Reference:** the game reads the UUID with the UUID parser of the Java library. That parser
  accepts groups of 1 to 8, 1 to 4, 1 to 4, 1 to 4 and 1 to 12 digits, and fills a short group
  with zeros at the start. It also accepts a `+` before a group, a group longer than its width,
  and digits outside ASCII. It keeps only the low bits of a long group. A string with four `-`
  that it rejects becomes a name selector when it has at most 16 code units.
- **Visible to the client:** yes, but only for unusual input. The client accepts such text, for
  example `0-0-0-0-1`, and the server replies with `argument.entity.invalid`. The server is
  stricter than the client, as `command-parser.md`, section 0.2.1, allows.
- **Test:** a differential test skips UUID text that is not in the grammar of ARG-ENT-005.

### ARG-ENT-DEV-002: Position of the checks of the argument

- **Rule:** ARG-ENT-010.
- **Reference:** the errors have the position 0 of the command line. The context of the error
  underlines the whole command line.
- **Visible to the client:** yes, in the context of the error message from the server. The
  local highlight of the client still starts at position 0.
- **Test:** a differential test compares the translation key and ignores the position.

### ARG-ENT-DEV-003: Entities with an empty bounding box

- **Rule:** ARG-ENT-013, box test.
- **Reference:** the box test needs a strict overlap. An entity with an empty bounding box, for
  example a marker, fails when its position is on a face of the box. Thus `@e[dx=0]` misses a
  marker at the exact origin.
- **Visible to the client:** no. Only the result of the command differs.
- **Test:** a differential test that finds a marker on a face of the box records the difference
  as expected.

### ARG-ENT-DEV-004: The box of the `distance` option

- **Rule:** ARG-ENT-009 and ARG-ENT-013.
- **Reference:** the game finds candidates in a box from `−m` to `m + 1` around the origin, with
  the strict box test. Take an entity with an empty bounding box at the exact distance `m`, in
  the direction −x, −y or −z. That entity fails, although its distance is in the bounds.
- **Visible to the client:** no. Only the result of the command differs.
- **Test:** a differential test that finds such an entity records the difference as expected.

### ARG-ENT-DEV-005: Absent bounds of the rotation options

- **Rule:** ARG-ENT-015, `x_rotation` and `y_rotation`.
- **Reference:** an absent minimum is 0 and an absent maximum is 359, both wrapped to
  `[−180, 180)`. Thus `x_rotation=..0` selects only the pitch 0, and `x_rotation=10..` also
  selects each pitch at or below −1. `y_rotation=10..` misses each yaw in the open interval from
  359 to 360.
- **Visible to the client:** no. Only the result of the command differs.
- **Test:** a differential test uses only rotation options with both bounds present.

### ARG-ENT-DEV-006: A loot predicate that does not exist

- **Rule:** ARG-ENT-017.
- **Reference:** the search does not fail. No entity passes the test, with or without `!`.
- **Visible to the client:** yes. The server replies with an error instead of the result of
  the command.
- **Test:** a differential test uses only loot predicates that exist.

### ARG-ENT-DEV-007: The order of entities and players

- **Rule:** ARG-ENT-013 and ARG-ENT-014, orders.
- **Reference:** the game collects players in the order of the player list of the server. A
  player joins at the end of that list, and a respawn moves the player to the end. A world
  limited search uses the player list of the dimension. A player that changes dimension goes to
  the end of the list of the new dimension. The game collects other entities in
  the order in which the dimension stores them. `nearest` and `furthest` keep the collection
  order for equal distances. `random` shuffles the collected list.
- **Visible to the client:** yes, through the entity that the command picks. Examples are
  `@e[limit=1]`, `@p` with two players at equal distance, and the result of `@r`.
- **Test:** a differential test compares the set of selected entities, and ignores their order.
  The maximum results can remove entities. Then the test compares the result only when no tie
  and no `arbitrary` or `random` order decides which entities stay.

### ARG-ENT-DEV-008: The length of a name counts bytes

- **Rule:** ARG-ENT-003, step 3.
- **Reference:** the game counts the length of a name in code units.
- **Visible to the client:** yes, but only for a name with a character outside ASCII. A UTF-8
  form never has fewer bytes than code units, so the server is stricter than the client. A
  player name has only ASCII characters (`login.md`, LOGIN-SEQ-001), so no player has such a
  name.
- **Test:** a differential test skips a name with a character outside ASCII.
