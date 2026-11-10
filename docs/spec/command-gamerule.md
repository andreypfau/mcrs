# The `gamerule` command: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| Parser ids | `brigadier:bool` 0, `brigadier:integer` 3 |

Source of these values: `assets/minecraft/version.json` and `assets/mcrs/reports/registries.json`,
registry `minecraft:command_argument_type`.

This document states the query and the set of a game rule, the messages, and the effect of six
rules that a benchmark of mobs uses. It does not prescribe an architecture.

## 0. Conventions

- **Node**, **literal**, **source**, **error**: as `command-parser.md` defines them.
- **Game rule**: an entry of the registry `minecraft:game_rule`. The crate
  `mcrs_minecraft_game_rule` holds the table of the entries, with the type, the default and the
  range of each.
- **Short name**: the path of the identifier of a game rule, for example `spawn_mobs` for
  `minecraft:spawn_mobs`.
- Provenance labels: `registry.md`, section 0.3. ID prefix: `CMD-GR`. None of the experiments has
  been run.

## 1. Scope

The query and the set apply to each game rule. Section 2 states the effect of six rules only. The
other rules have their effect in the documents of their systems, or in no document yet.

---

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

A name that is not a game rule matches no literal. The parse then fails with
`command.unknown.argument` (`command-parser.md`). A bool value other than `true` or `false` fails
with `parsing.bool.invalid`. An int value out of the range fails with `argument.integer.low` or
`argument.integer.big`.

Provenance: `source_suggested`. Kind: wire. Confidence: high. Experiment: run `dumpReports`
(`tools/vanilla-oracle`), and compare the node `gamerule` in `commands.json` with the tree above.
Run `gamerule no_such_rule true`. It fails with `command.unknown.argument`.

### CMD-GR-002: Query

- **Behavior:** `gamerule <rule>` sends `commands.gamerule.query` with the short name and the
  value text, with the broadcast flag clear.
- **Postconditions:** the value text of a bool is `true` or `false`. The value text of an int is
  its decimal text. The result is 1 or 0 for a bool, and the value for an int.

Provenance: `source_suggested`. Kind: semantic; the key and the argument order are wire.
Confidence: high.

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

Provenance: `source_suggested`. Kind: semantic; the keys and the argument order are wire.
Confidence: high. Experiment: run `gamerule mob_drops false` two times. The first reply is
`commands.gamerule.set` with `mob_drops` and `false`. The second reply is
`commands.gamerule.not_set` with the same arguments.

---

## 2. The rules of a benchmark of mobs

### CMD-GR-004: The six rules

| Rule | Type, default, range | Effect in this server |
|------|----------------------|-----------------------|
| `minecraft:spawn_mobs` | bool, `true` | When false, natural spawning spawns no mob of any category, and the special spawners (phantoms, patrols, wandering traders, cats, sieges) do not run (`natural-spawning.md`). `summon` and spawn eggs still make mobs. |
| `minecraft:advance_time` | bool, `true` | When false, no clock advances, and the time of day stays (`command-time.md`, CMD-TIME-006 and the rules after it). |
| `minecraft:advance_weather` | bool, `true` | When false, the weather timers stop, and the rain and the thunder do not start or end by themselves (CMD-W-003). |
| `minecraft:max_entity_cramming` | int, `24`, from 0 | The value `n` of the crowd damage (`entity-physics.md`, PHY-PSH-007; `entity-damage.md`, DMG-CRW-001). When 0, no entity takes crowd damage. Also when 0, the push limit of PHY-CMP-019 applies. |
| `minecraft:mob_drops` | bool, `true` | When false, a mob that dies drops no loot and no experience (`mob-animals.md`, ANIM-DROP-001, ANIM-DROP-005). |
| `minecraft:max_block_modifications` | int, `32768`, from 1 | The volume limit of `fill` (`command-fill.md`, CMD-FILL-002). |

A change applies from the next read of the rule. A read in the same tick after the command sees
the new value. A change of `advance_time` also sends the state of each clock to each player
(`command-time.md`).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. The types, the defaults and the
ranges come from the table of `mcrs_minecraft_game_rule`.

### CMD-GR-005: The push limit with `max_entity_cramming` 0

`max_entity_cramming 0` turns off the crowd damage in both servers. In this server, it also turns
on the push limit of PHY-CMP-019: an entity pushes only its 24 nearest neighbours. The vanilla
server pushes each neighbour (PHY-DEV-008). The two servers do the same pushes while no entity
touches more than 24 other entities. A benchmark with a denser crowd does less push work on this
server. Its report must state the rule value.

Provenance: `design`. Kind: semantic. Confidence: medium.
