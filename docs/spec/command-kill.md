# The `kill` command: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`.

This document states the tree of `kill`, the victims, the result of a kill for each kind of victim,
and the messages. It does not prescribe an architecture.

## 0. Conventions

- **Node**, **source**, **error**: as `command-parser.md` defines them.
- **Display name**: as `command-execute.md`, section 0, defines it.
- **Living entity**, **mob**: as `entity-physics.md` and `entity-tracking.md`, section 0.1,
  define them.
- **Hit**, **amount**, **dying state**, **death time**: as `entity-damage.md`, section 0.1, defines
  them.
- **Victim**: an entity that the command kills.
- Provenance labels: `registry.md`, section 0.3. ID prefix: `CMD-KILL`. None of the experiments
  has been run.

## 1. Scope

This document states the kill of a mob and of an entity that is not a living entity, for example an
item entity. The kill of a player is out of scope: the death screen and the respawn have no
specification yet. The game event `entity_die`, the kill statistics and the scoreboard criteria are
out of scope, as in `entity-damage.md`, section 12.

---

### CMD-KILL-001: Tree

```
kill                                     require command_level gamemasters; executable
└── <targets: minecraft:entity>          executable; entities (0x00)
```

The permission is PERM-LEVEL-001, level `gamemasters`. `ARG-ENT-001` to `ARG-ENT-021` state the
argument.

Provenance: `source_suggested`. Kind: wire. Confidence: high. Experiment: run `dumpReports`
(`tools/vanilla-oracle`), and compare the node `kill` in `commands.json` with the tree above.

### CMD-KILL-002: Victims

- `kill` without an argument kills the source entity. If the source has no entity, the command
  fails with `permissions.requires.entity`. Thus `kill` from the server console fails.
- `kill <targets>` kills each entity that the selector finds, in the order of the selector. If it
  finds none, the command fails with `argument.entity.notfound.entity` (ARG-ENT-016).

An entity selector does not find an entity in the dying state (`entity-damage.md`, DMG-DTH-004).
Thus a second `kill @e[type=pig]` in the 20 ticks after the first finds no pig, and fails.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: from the server
console, run `kill`. It fails with `permissions.requires.entity`. Then run
`kill @e[type=pig]` two times in the same tick. The second fails with
`argument.entity.notfound.entity`.

### CMD-KILL-003: Kill a living entity

- **Behavior:** the kill of a living entity is one hit (`entity-damage.md`, section 3) with these
  values:
  - the damage type `minecraft:generic_kill`, with no causing entity, no direct entity and no
    source position;
  - the amount `3.4028235E38`, the largest finite f32 value.
- **Postconditions:**
  - `generic_kill` has the tags `bypasses_invulnerability`, `bypasses_armor` and `no_knockback`
    in the data pack. Thus DMG-HIT-001 rejects the hit only for a victim that is removed or in the
    dying state.
  - The amount is more than each possible last amount, so the damage cooldown never rejects the
    hit. Inside the cooldown, the hit is a partial hit. The applied damage is more than the
    health and the absorption, so the health becomes 0.
  - The victim dies (DMG-DTH-001). It stays in the world in the dying state for 20 ticks, and
    then the server removes it (DMG-DTH-003). In the dying state, it has no AI step
    (`mob-goals.md`, GOAL-FRM-009). It still pushes other entities (DMG-DTH-004).
  - A full hit sends `damage_event` with the type `generic_kill` and plays the death sound. The
    death sends the entity event 3 and the pose `dying` (`entity-damage.md`, section 9).
  - The death drops the loot of the victim when the game rule `mob_drops` is true. For the four
    farm animals, `mob-animals.md`, ANIM-DROP-001, states the loot. No player hit the victim, so
    an animal drops no experience (ANIM-DROP-005).
  - The hit sets the damage record of the victim to `generic_kill` (`mob-goals.md`,
    GOAL-PNC-001). `generic_kill` is not in `panic_causes`.
- **Edge cases:**
  - A victim that is already in the dying state rejects the hit. It still counts as a victim
    (CMD-KILL-005).
  - A totem of undying can stop the death. Items are out of scope (`entity-damage.md`, section
    12).

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: summon a pig with
`{NoAI:1b}`. Run `kill @e[type=pig]`. Capture `damage_event` with the id of `generic_kill`, the
entity event 3, and `remove_entities` 20 ticks later. With `mob_drops` true, 1 to 3 `porkchop`
items appear.

### CMD-KILL-004: Kill an entity that is not a living entity

- **Behavior:** the command removes the victim at once, with the reason "killed". The victim does
  not take a hit.
- **Postconditions:** the server removes the row in the same tick as the command. The tracker
  sends `remove_entities` to each viewer (`entity-tracking.md`, TRK-REM-001).
- **Edge cases:** a victim with an inventory, for example a chest minecart, drops its contents at
  the removal. No document states that drop yet.

An item entity, an experience orb and a primed TNT are examples. The kill of an armor stand is
also a removal at once, although it is a living entity.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: drop an item, and
run `kill @e[type=item]`. The item disappears in the same tick.

### CMD-KILL-005: Feedback and result

- **Behavior:** after the last victim, the command sends one success message with the broadcast
  flag set (`commands-packet.md`, CMDP-EXEC-004):

| Victims | Translation key | Arguments |
|---------|-----------------|-----------|
| one | `commands.kill.success.single` | the display name of the victim |
| more than one | `commands.kill.success.multiple` | the count of victims, as an int primitive |

- **Postconditions:** the result of the command is the count of victims. Each victim counts, also
  a victim that rejected the hit.
- **Edge cases:** the display name of a victim with no custom name is the translatable name of its
  type, with the hover event `show_entity` (`entity-tracking.md`, TRK-SUM-004).

Provenance: `source_suggested`. Kind: semantic; the keys and the argument order are wire.
Confidence: high. Experiment: summon three pigs, and run `kill @e[type=pig]`. The reply is
`commands.kill.success.multiple` with 3. Run `kill @n[type=cow]` on one cow. The reply is
`commands.kill.success.single` with `entity.minecraft.cow`.

### CMD-KILL-006: Order of the kills

- **Behavior:**
  1. The command kills the victims one after the other, in the order of the selector.
  2. Each kill applies when the command runs, before the next entity pass of the dimension.
  3. After the last kill, the command sends the feedback of CMD-KILL-005.
- **Postconditions:** a later command of the same tick sees each victim in the dying state or
  removed. The UUIDs of the victims do not change the result, because each kill writes only
  its own victim.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

---

## 2. Use in a benchmark

A benchmark that clears its mobs with `kill @e[type=pig]` must account for these effects:

- The pigs stay in the world for 20 ticks in the dying state, and push other entities.
- With `mob_drops` true, each adult pig drops 1 to 3 `porkchop` items. Set `gamerule mob_drops
  false` before the kill, or run `kill @e[type=item]` 20 ticks after it.
- Each kill of a mob sends a `damage_event`, an entity event, a sound and an entity data update
  to each viewer.
