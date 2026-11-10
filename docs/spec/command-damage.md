# The `damage` command: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| Parser ids | `brigadier:float` 1, `minecraft:resource` 46 |

Source of these values: `assets/minecraft/version.json` and `assets/mcrs/reports/registries.json`,
registry `minecraft:command_argument_type`.

This document states the tree of `damage`, the damage source that each form makes, the hit on the
target, and the messages. It does not prescribe an architecture.

## 0. Conventions

- **Node**, **source**, **error**: as `command-parser.md` defines them.
- **Display name**: as `command-execute.md`, section 0, defines it.
- **Hit**, **amount**, **damage type**, **damage source**, **causing entity**, **direct entity**,
  **source position**, **full hit**, **partial hit**, **rejected hit**: as `entity-damage.md`,
  section 0.1, defines them.
- **Target**: the entity that the command hits.
- Provenance labels: `registry.md`, section 0.3. ID prefix: `CMD-DMG`. None of the experiments
  has been run.

## 1. Scope

This document states a hit on a living entity that is not a player. These subjects are out of
scope:

| Subject | Where |
|---------|-------|
| The hit on an entity that is not a living entity, for example an item entity or an end crystal | a later specification of each type |
| The hit on a player, and the difficulty scale of the field `scaling` | `entity-damage.md`, section 10, and `player-attack.md` |
| The knockback of a damage type without the tag `no_knockback` | `entity-damage.md`, DMG-HIT-005, step 2 |
| Shields and other items that change the amount | `entity-damage.md`, section 12 |

---

### CMD-DMG-001: Tree

```
damage                                               require command_level gamemasters
└── <target: minecraft:entity>                       one entity (0x01)
    └── <amount: brigadier:float>                    executable; min 0
        └── <damageType: minecraft:resource>         executable; registry minecraft:damage_type
            ├── at
            │   └── <location: minecraft:vec3>       executable
            └── by
                └── <entity: minecraft:entity>       executable; one entity (0x01)
                    └── from
                        └── <cause: minecraft:entity>    executable; one entity (0x01)
```

The permission is PERM-LEVEL-001, level `gamemasters`. CMD-TIME-004 states `minecraft:resource`.
An amount less than 0 fails with `argument.float.low` (`command-parser.md`). An identifier that is
not a damage type fails with `argument.resource.not_found`.

`<target>` finds one entity. A selector that finds more than one entity fails with
`argument.entity.toomany` (ARG-ENT-016). Thus a hit on a group of mobs needs one run for each
mob, for example `execute as @e[type=pig,distance=..8] run damage @s 1 minecraft:cactus`.

Provenance: `source_suggested`. Kind: wire. Confidence: high. Experiment: run `dumpReports`
(`tools/vanilla-oracle`), and compare the node `damage` in `commands.json` with the tree above.

### CMD-DMG-002: The damage source of each form

| Form | Damage type | Causing entity | Direct entity | Source position |
|------|-------------|----------------|---------------|-----------------|
| `damage <target> <amount>` | `minecraft:generic` | none | none | none |
| `... <damageType>` | `damageType` | none | none | none |
| `... <damageType> at <location>` | `damageType` | none | none | `location` against the source |
| `... <damageType> by <entity>` | `damageType` | `entity` | `entity` | none |
| `... <damageType> by <entity> from <cause>` | `damageType` | `cause` | `entity` | none |

A source with an entity has no source position. The amount is the f32 value of `<amount>`.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

### CMD-DMG-003: The hit

- **Behavior:**
  1. Do one hit on the target with the damage source of CMD-DMG-002 and the amount
     (`entity-damage.md`, section 3). The hit occurs when the command runs, before the next
     entity pass of the dimension.
  2. If the hit is a rejected hit, fail with `commands.damage.invulnerable`. The command sends no
     success message.
  3. Else send the success message of CMD-DMG-004. The result is 1.
- **Postconditions:**
  - A hit is rejected by a test of DMG-HIT-001, or by the cooldown when the amount is not more
    than the last amount (DMG-HIT-003). Each other hit succeeds, also a partial hit and a full hit
    with the amount 0.
  - The hit resets the idle time of the target (DMG-HIT-002, step 1). This also occurs when the
    cooldown rejects the hit.
  - A hit that succeeds sets the damage record of the target (`mob-goals.md`, GOAL-PNC-001).
    When its damage type is in `panic_causes`, an animal of `mob-animals.md` panics
    (GOAL-PNC-002). `minecraft:generic`, the default type, is not in `panic_causes`.
- **Edge cases:**
  - `damage <pig> 0 minecraft:cactus` is a full hit with no health change. It sends
    `damage_event`, sets the hurt time and the damage record, and starts a panic. A second run
    within 10 ticks is a rejected hit, because the amount 0 is not more than the last amount 0.
  - A hit that leaves the health at 0 kills the target (DMG-DTH-001).

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: summon a pig. Run
`damage @n[type=pig] 1 minecraft:cactus`, and record its position in each tick. The pig starts to
run at 0.215 blocks in each tick within 3 ticks (`mob-goals.md`, E-G2). Run the same command again
in the same tick. It fails with `commands.damage.invulnerable`.

### CMD-DMG-004: Feedback

- **Behavior:** the success message has the translation key `commands.damage.success` and the
  broadcast flag set (`commands-packet.md`, CMDP-EXEC-004). The arguments are, in order:
  1. the amount, as a float primitive;
  2. the display name of the target.
- **Postconditions:** the client makes the text of the amount, for example `1.0`.

Provenance: `source_suggested`. Kind: wire for the key and the argument order, semantic for the
broadcast. Confidence: high.

---

## 2. Use in a benchmark

A stimulus that makes a group of pigs panic must use a damage type of `panic_causes`. The type
`minecraft:cactus` has the tag `no_knockback`, so the hit adds no velocity. The amount 0 hurts no
pig, so the same group can take the stimulus again after 10 ticks. One run hits one pig, so the
stimulus is `execute as <selector> run damage @s 0 minecraft:cactus`.

A pig that panics does not make another pig panic. Only the pigs that take the hit panic. The
other pigs move only when the push of a running pig moves them.
