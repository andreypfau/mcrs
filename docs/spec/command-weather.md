# The `weather` command: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| Parser ids | `minecraft:time` 43 |

Source of these values: `assets/minecraft/version.json` and `assets/mcrs/reports/registries.json`,
registry `minecraft:command_argument_type`.

This document states `weather clear`, the weather state that it writes, and how the rain level then
changes. The forms `rain` and `thunder` are out of scope, except in the tree. It does not prescribe
an architecture.

## 0. Conventions

- **Node**, **source**, **error**: as `command-parser.md` defines them.
- **Weather state**: five values of the world: the clear time, the rain time, the thunder time,
  the rain flag and the thunder flag. The server has one weather state for all dimensions.
- **Rain level**, **thunder level**: two f32 values from 0 to 1 of a dimension. The sky darkness
  reads them (`environment-attributes.md`).
- **Weather dimension**: a dimension whose type has sky light and no ceiling, for example the
  overworld. Only a weather dimension has rain.
- Provenance labels: `registry.md`, section 0.3. ID prefix: `CMD-W`. None of the experiments has
  been run.

---

### CMD-W-001: Tree

```
weather                                        require command_level gamemasters
├── clear                                      executable
│   └── <duration: minecraft:time>             executable; min 1
├── rain                                       executable
│   └── <duration: minecraft:time>             executable; min 1
└── thunder                                    executable
    └── <duration: minecraft:time>             executable; min 1
```

The permission is PERM-LEVEL-001, level `gamemasters`. `command-time.md`, CMD-TIME-003, states
`minecraft:time`.

Provenance: `source_suggested`. Kind: wire. Confidence: high. Experiment: run `dumpReports`
(`tools/vanilla-oracle`), and compare the node `weather` in `commands.json` with the tree above.

### CMD-W-002: `weather clear`

- **Behavior:**
  1. Let `d` be the value of `<duration>`, in ticks. Without the argument, draw `d` uniform from
     12000 to 180000, both ends included (fidelity `distribution`).
  2. Set the clear time to `d`, the rain time to 0 and the thunder time to 0.
  3. Set the rain flag and the thunder flag to false.
  4. Send `commands.weather.set.clear`, with no argument, as a success message with the broadcast
     flag set.
- **Postconditions:** the result is the value of `<duration>`. Without the argument, the result is
  −1, not `d`.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: run
`execute store result score @s r run weather clear`. The score is −1. Run the same with
`weather clear 10s`. The score is 200.

### CMD-W-003: The weather in each tick

- **Behavior:** in each tick of a weather dimension:
  1. If the game rule `advance_weather` is true, advance the weather state. While the clear time is
     more than 0, decrease it by 1, and keep the rain flag and the thunder flag false. After it
     reaches 0, the cycle of rain and thunder runs again. That cycle is out of scope.
  2. If `advance_weather` is false, the weather state does not change.
  3. Move the rain level by 0.01 toward 1 when the rain flag is true, and toward 0 when it is false.
     Clamp it to the range from 0 to 1. Do the same for the thunder level with the thunder flag.
- **Postconditions:** step 3 runs also when `advance_weather` is false. Thus after `weather clear`
  in rain, the rain level falls to 0 within 100 ticks. Each change of a level sends the
  `game_event` packet of that level to the players of the dimension.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

---

## 1. Use in a benchmark

To fix clear weather, run `gamerule advance_weather false` and `weather clear`. Start the measure
at least 100 ticks after these commands. Then the rain level is 0, and the sky darkness does not
depend on the weather (`mob-goals.md`, GOAL-POS-006).
