# The `gamerule` command: behavioural specification

| Field | Value |
|-------|-------|
| Parser ids | `brigadier:bool` 0, `brigadier:integer` 3 |

Version: see `assets/minecraft/version.json`. Source of the parser ids: `assets/mcrs/reports/registries.json`, registry
`minecraft:command_argument_type`.

This document states the query and the set of a game rule, the messages, and the effect of six
rules that a benchmark of mobs uses. It does not prescribe an architecture.

## 0. Conventions

- **Node**, **literal**, **source**, **error**: as `command-parser.md` defines them.
- **Game rule**: an entry of the registry `minecraft:game_rule`.
- **Short name**: the path of the identifier of a game rule, for example `spawn_mobs` for
  `minecraft:spawn_mobs`.
- **Rule definition**: the entry of a game rule in
  `assets/mcrs/registry_definition/game_rule.json` (CMD-GR-006).
- **Value text**: the text of a value. For a bool, it is `true` or `false`. For an int, it is
  the decimal text, with a `-` before a negative value.
- Provenance labels: `registry.md`, section 0.3. ID prefix: `CMD-GR`.

## 1. Scope

The query and the set apply to each game rule. Section 2 states the effect of six rules only. The
other rules have their effect in the documents of their systems, or in no document yet.

---

### CMD-GR-006: The game rules come from the data

- The set of game rules is the registry `minecraft:game_rule` in
  `assets/mcrs/reports/registries.json`. This document does not list the rules.
- The rule definitions are in `assets/mcrs/registry_definition/game_rule.json`.
  `tools/vanilla-oracle` writes this file from the game. Each entry has these fields:
  - `type`: `boolean` or `integer`. In this document, `boolean` is the type bool, and `integer`
    is the type int.
  - `default`: the value of the rule in a new world.
  - `min` and `max`: the range of an int rule.
  - `required_features`: the feature flags that the rule needs. When the field is absent, the
    rule needs no feature flag.
  - `category`: the group of the rule. The command does not use it.
- A new version that adds, removes or renames a rule changes only these two files. The tree of
  CMD-GR-001 follows from them.

Provenance: `black_box_confirmed` (`assets/mcrs/reports/registries.json`,
`assets/mcrs/registry_definition/game_rule.json`). Kind: wire: the names reach the client in the
command tree. Confidence: high.

### CMD-GR-001: Tree

```
gamerule                                          require command_level gamemasters
├── <short name>                                  executable
│   └── <value: brigadier:bool | brigadier:integer>   executable
└── <identifier>                                  executable
    └── <value: brigadier:bool | brigadier:integer>   executable
```

- For each game rule whose required features are enabled, the node has two literal children: the
  short name and the full identifier, for example `spawn_mobs` and `minecraft:spawn_mobs`. Both
  lead to the same rule.
- `<value>` is `brigadier:bool` for a rule of the type bool. It is `brigadier:integer` with the
  minimum and the maximum of the rule for a rule of the type int.
- The permission is PERM-LEVEL-001, level `gamemasters`.

A name that is not a game rule matches no literal. The comparison of names does not ignore case:
`ADVANCE_TIME` matches no literal. A rule name with more characters after it, for example
`advance_timex`, also matches no literal. In each of these cases, these results apply:

- The parse fails with `command.unknown.argument` (`command-parser.md`, CMD-PARSE-007, case 3).
- The position of the error is the start of the name: 9 for the line `gamerule <name>`.
- The server does not read the value after the name, and no rule changes.

A bool value other than `true` or `false` fails with `parsing.bool.invalid`. An int value out of
the range fails with `argument.integer.low` or `argument.integer.big`. The position of these two
errors is the start of the value.

| Command line | Error | Position |
|--------------|-------|----------|
| `gamerule nosuchrule` | `command.unknown.argument` | 9 |
| `gamerule nosuchrule true` | `command.unknown.argument` | 9 |
| `gamerule ADVANCE_TIME` | `command.unknown.argument` | 9 |
| `gamerule advance_time maybe` | `parsing.bool.invalid`, argument `maybe` | 22 |
| `gamerule max_entity_cramming -1` | `argument.integer.low`, minimum 0, value -1 | 29 |

Provenance: `source_suggested`. Kind: wire for the tree, client agreement for the parse.
Confidence: high. Experiment: run `dumpReports` (`tools/vanilla-oracle`), and compare the node
`gamerule` in `commands.json` with the tree above. Run each line of the table on the vanilla
server. Each line fails with the error of the table. The context text of CMD-ERR-003 ends before
the name or before the value. For a position of 9, it has no ellipsis.

### CMD-GR-002: Query

- **Behavior:** `gamerule <rule>` sends `commands.gamerule.query` with two arguments: the short
  name and the value text. Each argument is a string. The broadcast flag is clear.
- **Postconditions:** the rule does not change. The result is 1 for `true`, 0 for `false`, and
  the value for an int.
- **Edge cases:** the full identifier gives the same reply as the short name. The first argument
  is the short name also then.

| Command line | Value | Key | Arguments | Result |
|--------------|-------|-----|-----------|--------|
| `gamerule advance_time` | `true` | `commands.gamerule.query` | `advance_time`, `true` | 1 |
| `gamerule minecraft:advance_time` | `false` | `commands.gamerule.query` | `advance_time`, `false` | 0 |
| `gamerule max_entity_cramming` | 24 | `commands.gamerule.query` | `max_entity_cramming`, `24` | 24 |

Provenance: `source_suggested`. Kind: semantic; the key and the argument order are wire.
Confidence: high. Experiment: run each line of the table on the vanilla server, with the rule
at the value of the table. Each reply has the key and the arguments of the table. Run
`scoreboard objectives add r dummy`. Then run each line after
`execute store result score #q r run`. The score of `#q` is the result of the table.

### CMD-GR-003: Set

- **Behavior:**
  1. If the rule already has the value, fail with `commands.gamerule.not_set`, with the short name
     and the value text. The rule does not change.
  2. Else set the rule to the value. Apply the effect of the change (section 2).
  3. Send `commands.gamerule.set` with the short name and the value text, with the broadcast flag
     set. The result is that of CMD-GR-002 for the new value.
- **Postconditions:** the server has one value of each rule for the whole world. A set in one
  dimension changes the rule in each dimension. The save keeps the value.
- **Edge cases:** a script that sets a rule to its current value gets an error. For example,
  `gamerule spawn_mobs false` fails on a world where the rule is already false.

The arguments of both keys are strings. The first argument is the short name, also when the
command line has the full identifier. The error has no position.

| Command line | Value before | Key | Arguments | Result |
|--------------|--------------|-----|-----------|--------|
| `gamerule advance_time false` | `true` | `commands.gamerule.set` | `advance_time`, `false` | 0 |
| `gamerule advance_time false` | `false` | `commands.gamerule.not_set` (error) | `advance_time`, `false` | failure |
| `gamerule advance_time true` | `false` | `commands.gamerule.set` | `advance_time`, `true` | 1 |
| `gamerule max_entity_cramming 5` | 24 | `commands.gamerule.set` | `max_entity_cramming`, `5` | 5 |
| `gamerule max_entity_cramming 5` | 5 | `commands.gamerule.not_set` (error) | `max_entity_cramming`, `5` | failure |

Provenance: `source_suggested`. Kind: semantic; the keys and the argument order are wire.
Confidence: high. Experiment: run the lines of the table in their order on a new world on the
vanilla server. Each reply has the key and the arguments of the table. Make the objective `r`
as in CMD-GR-002. Put `execute store result score #s r run` before each line that does not fail.
The score of `#s` is the result of the table.

### CMD-GR-007: Names of earlier versions

The game rules of earlier versions had camelCase names, for example `doDaylightCycle`. The
registry of CMD-GR-006 does not contain these names. The server must not map an old name to a
rule. An old name, with or without the namespace `minecraft:`, is a name that is not a game rule
(CMD-GR-001).

| Command line | Error | Position | Rule change |
|--------------|-------|----------|-------------|
| `gamerule doDaylightCycle` | `command.unknown.argument` | 9 | none |
| `gamerule doDaylightCycle false` | `command.unknown.argument` | 9 | none |
| `gamerule minecraft:doDaylightCycle true` | `command.unknown.argument` | 9 | none |

The error feedback is that of `commands-packet.md`, CMDP-EXEC-003. The context text is
`gamerule `. The rest of the line follows it, from the name to the end.

Provenance: `source_suggested`. Kind: client agreement. Confidence: high. Experiment: run each
line of the table on the vanilla server. Each line fails with `command.unknown.argument`. The
context text has no ellipsis, so the position is at most 10. Then run `gamerule advance_time`.
The value is the default of the rule definition.

---

## 2. Rules that change mob behaviour

### CMD-GR-004: The six rules

| Rule | Type, default, range | Effect |
|------|----------------------|--------|
| `minecraft:spawn_mobs` | bool, `true` | When false, natural spawning spawns no mob of any category, and the special spawners (phantoms, patrols, wandering traders, cats, sieges) do not run (`natural-spawning.md`). `summon` and spawn eggs still make mobs. |
| `minecraft:advance_time` | bool, `true` | When false, no clock advances, and the time of day stays (`command-time.md`, CMD-TIME-006 and the rules after it). |
| `minecraft:advance_weather` | bool, `true` | When false, the weather timers stop, and the rain and the thunder do not start or end by themselves (CMD-W-003). |
| `minecraft:max_entity_cramming` | int, `24`, from 0 | The value `n` of the crowd damage (`entity-physics.md`, PHY-PSH-007; `entity-damage.md`, DMG-CRW-001). When 0, no entity takes crowd damage. Also when 0, the push limit of PHY-CMP-019 applies. |
| `minecraft:mob_drops` | bool, `true` | When false, a mob that dies drops no loot and no experience (`mob-animals.md`, ANIM-DROP-001, ANIM-DROP-005). |
| `minecraft:max_block_modifications` | int, `32768`, from 1 | The volume limit of `fill` (`command-fill.md`, CMD-FILL-002). |

A change applies from the next read of the rule. A read in the same tick after the command sees
the new value. A change of `advance_time` also sends the state of each clock to each player
(`command-time.md`).

The types, the defaults and the ranges of the table agree with the rule definitions (CMD-GR-006).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CMD-GR-005: The push limit with `max_entity_cramming` 0

`max_entity_cramming 0` turns off the crowd damage in both servers. In this server, it also turns
on the push limit of PHY-CMP-019: an entity pushes only its 24 nearest neighbours. The vanilla
server pushes each neighbour (PHY-DEV-008). The two servers do the same pushes while no entity
touches more than 24 other entities. A benchmark with a denser crowd does less push work on this
server. Its report must state the rule value.

Provenance: `design`. Kind: semantic. Confidence: medium.
