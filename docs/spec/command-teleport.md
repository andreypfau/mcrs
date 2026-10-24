# The `teleport` and `tp` commands: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

Source of these values: `assets/minecraft/version.json`.

This document states the tree of `teleport` and its alias `tp`, how each form computes the
destination, the packets that a teleported player receives, and the messages. It does not
prescribe an architecture.

## 0. Conventions

- **Node**, **redirect**, **source**, **error**: as `command-parser.md` defines them.
- **Victim**: an entity that the command moves.
- **Relative set**: the set of flags that the `player_position` packet carries
  (CMD-TP-006).
- **Spawnable bounds**: the block positions with `−30 000 000 ≤ x < 30 000 000`,
  `−30 000 000 ≤ z < 30 000 000` and `−20 000 000 ≤ y < 20 000 000`.
- Provenance labels: `registry.md`, section 0.3. ID prefix: `CMD-TP`. None of the experiments
  has been run.

---

### CMD-TP-001: Tree

```
teleport                                               require command_level gamemasters
├── <location: minecraft:vec3>                         executable
├── <destination: minecraft:entity>                    executable; one entity (0x01)
└── <targets: minecraft:entity>                        entities (0x00)
    ├── <location: minecraft:vec3>                     executable
    │   ├── <rotation: minecraft:rotation>             executable
    │   └── facing
    │       ├── entity
    │       │   └── <facingEntity: minecraft:entity>   executable; one entity (0x01)
    │       │       └── <facingAnchor: minecraft:entity_anchor>   executable
    │       └── <facingLocation: minecraft:vec3>       executable
    └── <destination: minecraft:entity>                executable; one entity (0x01)
tp                                                     require command_level gamemasters; redirect to teleport
```

The children are in the order of the tree. `tp` has no command, no source modifier and no fork.

Provenance: `black_box_confirmed` (`commands.json`) for the nodes, the types, the flags and the
redirect. `source_suggested` for the order of the children, which the report sorts by name.
Confidence: high.

### CMD-TP-002: Choice between the first-level forms

`teleport 1 2 3` matches `<location>`. `teleport Steve` matches `<destination>`, and also
`<targets>`, which is not executable. Both parse to the end with no error, so the first,
`<destination>`, wins (`command-parser.md`, CMD-PARSE-005). In `teleport Steve 1 2 3`,
`<destination>` reads `Steve` and leaves ` 1 2 3`, but `<targets> <location>` reads to the end,
so the second wins.

Provenance: `source_suggested`. Confidence: high.

### CMD-TP-003: Victims

- The forms without `<targets>` move the source entity. If the source has no entity, the
  command fails with `permissions.requires.entity`.
- The forms with `<targets>` move each entity that the selector finds, in that order. If it
  finds none, the command fails with `argument.entity.notfound.entity`.

Provenance: `source_suggested`. Confidence: high.

### CMD-TP-004: Destination of each form

| Form | Dimension | Position | Rotation | Facing |
|------|-----------|----------|----------|--------|
| `<location>` | the source dimension | `location` against the source | the victim keeps its rotation | none |
| `<destination>` | the dimension of `destination` | the position of `destination` | the yaw and pitch of `destination` | none |
| `<targets> <location>` | the source dimension | `location` against the source | the victim keeps its rotation | none |
| `<targets> <location> <rotation>` | the source dimension | `location` | `rotation` against the source | none |
| `... facing entity <facingEntity> [<facingAnchor>]` | the source dimension | `location` | the victim keeps its rotation | look at the anchor of `facingEntity`, `feet` when absent |
| `... facing <facingLocation>` | the source dimension | `location` | the victim keeps its rotation | look at `facingLocation` |
| `<targets> <destination>` | the dimension of `destination` | the position of `destination` | the yaw and pitch of `destination` | none |

`location` and `rotation` resolve against the source once, before the first victim
(`argument-coordinates.md`, section 3). `destination` and `facingEntity` must each find exactly
one entity (`argument-entity.md`, ARG-ENT-016).

Provenance: `source_suggested`. Confidence: medium.

### CMD-TP-005: Move one victim

- **Inputs:** a victim, a dimension, a position `(x, y, z)`, a yaw, a pitch, a relative set, an
  optional facing target.
- **Behavior:**
  1. If the block that contains the position is outside the spawnable bounds, the command
     fails with `commands.teleport.invalidPosition`. The victims before this one stay moved.
  2. For each relative flag of the position and the rotation, the value to apply is the
     destination value minus the current value of the victim. Each other value is absolute.
     The yaw and the pitch to apply are wrapped to `[−180, 180)`.
  3. The victim moves. A sleeping player wakes up first. A player that spectates through
     another entity gets its own camera back.
  4. If there is a facing target, the victim turns to face it.
  5. Unless the victim is a living entity that glides with elytra, its vertical velocity
     becomes 0, and it is on the ground.
  6. A mob stops its path.
- **Postconditions for a player in the same dimension:** the player receives
  `player_position` with a new teleport id, the values of step 2, a velocity of zero, and the
  relative set of CMD-TP-006. The head yaw becomes the new yaw.
- **Postconditions for a player in another dimension:** the player changes dimension as the
  dimension change specification states, and then has the new position and rotation. That
  specification does not exist yet.

Provenance: `source_suggested`. Confidence: medium. Experiment: as an operator at
`0.5 64 0.5`, run `tp ~1 ~ ~`, and capture `player_position`. Its x is 1.0 with the flag for
x set, and its y and z are 0.0 with their flags set.

### CMD-TP-006: The relative set

- **Behavior:** the relative set is the union of:
  - for each axis of the position that is relative: the flag of that axis, when the victim is
    in the destination dimension;
  - for each axis of the position that is relative: the flag of the velocity on that axis;
  - the flags of yaw and pitch, when the form has no `<rotation>`. Otherwise the flag of yaw
    when the yaw of `rotation` is relative, and the flag of pitch when its pitch is relative.
- The flag bits on the wire:

| Bit | Flag |
|-----|------|
| `0x001` | x |
| `0x002` | y |
| `0x004` | z |
| `0x008` | yaw |
| `0x010` | pitch |
| `0x020` | velocity x |
| `0x040` | velocity y |
| `0x080` | velocity z |
| `0x100` | rotate the velocity |

- **Postconditions:** local coordinates (`^`) are relative on all three axes. The `<destination>`
  forms have an empty relative set.

Provenance: `source_suggested`. Confidence: medium.

### CMD-TP-007: Feedback

- **Behavior:** after all victims moved, one success message with the broadcast flag set:

| Form | One victim | More than one victim |
|------|------------|----------------------|
| to a position | `commands.teleport.success.location.single`: display name of the victim, x, y, z | `commands.teleport.success.location.multiple`: count, x, y, z |
| to an entity | `commands.teleport.success.entity.single`: display name of the victim, display name of the destination | `commands.teleport.success.entity.multiple`: count, display name of the destination |

- **Postconditions:** x, y and z are the resolved position of CMD-TP-004, written with six
  digits after the decimal point and a `.` as the decimal mark, for example `1.500000`. The
  result of the command is the number of victims.

Provenance: `source_suggested`. Confidence: medium. Experiment: run `tp 1 2 3`, and read
`1.500000`, `2.000000` and `3.500000` in the message.
