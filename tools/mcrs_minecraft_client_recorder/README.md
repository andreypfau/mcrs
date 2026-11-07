# mcrs_minecraft_client_recorder

Records the client ticks of a vanilla client for the player experiments of
`docs/spec/entity-physics.md`, section 15: E9 to E14 and E21 to E23, and for the conformance
tests C1 to C7 of `docs/spec/movement-checks.md`, section 14.2. The tool starts the
vanilla dedicated server of the corpus version, listens as a proxy in front of it, and sets
each step of an experiment up through the server console. A human plays the vanilla client.
The tool writes one recording for each step. These tests read the recordings:

- `own_player_follows_the_recorded_client_ticks` in
  `crates/mcrs_minecraft_physics/tests/recorded_player.rs` reads
  `crates/mcrs_minecraft_physics/tests/fixtures/player`.
- `a_vanilla_client_raises_no_flag` in `crates/mcrs_minecraft_move_predict/tests/conformance.rs`
  reads the same directory and `crates/mcrs_minecraft_move_predict/tests/fixtures/conformance`,
  subdirectories included. It prints the number of predicted ticks N and the bound 3/N of the
  false positive rate. With no recording, it prints that it skipped.
- `each_recording_of_a_predicted_cheat_module_is_flagged` in the same file reads
  `crates/mcrs_minecraft_move_predict/tests/fixtures/cheats/<module>` and prints the detection
  rate of each module. Each recording of `fly`, `speed`, `no_fall`, `phase` and `step` must have a
  flag. Layer 2 does not see `timer`, so its rate is only printed.

Run them with
`cargo nextest run -p mcrs_minecraft_move_predict --test conformance --no-capture`.

## Record one experiment

You need Java 21 or later, `jq`, and a vanilla client of the version in
`crates/mcrs_minecraft_client_jar/src/release.json`.

1. Read https://aka.ms/MinecraftEULA.
2. Run the script with the experiment name:

   ```sh
   MINECRAFT_EULA=true tools/mcrs_minecraft_client_recorder/record.sh e9
   ```

   The names are the files in `scenarios/`: `e9` to `e14` and `e21` to `e23`. The script
   downloads and verifies the server jar, starts the server on a new flat world, and starts the
   proxy on `localhost:25565`.
3. In the vanilla client, open Multiplayer and join `localhost`.
4. When the terminal shows the instruction of the step, read it and press Enter.
5. Go to the game window and release all keys. The tool teleports the player to the start, and
   then teleports the player again after 2 seconds. The second teleport starts the recording.
6. When the title `Go` shows, do what the instruction says.
7. When the title `Stop` shows, the step ends and the terminal shows the next instruction.
   Go back to step 4.

After the last step the tool stops the server. Make sure that each file
`<out>/<experiment>_<step>.txt` exists, then commit them. To record a step again, run the
experiment again.

The second argument of `record.sh` is the output directory `<out>`. Without it, the recordings
of `e9` to `e23` go to `crates/mcrs_minecraft_physics/tests/fixtures/player`, and the others go
to `crates/mcrs_minecraft_move_predict/tests/fixtures/conformance`.

## Record with a delay

The environment variable `DELAY_TRACE` names a trace file. The tool then holds the data between
the client and the proxy for half of the next round trip sample of the trace: each frame from the
client, and the data of each poll for the client. Each direction reads the samples in order, and
starts again at the first sample after the last. Data never passes earlier data. The times in the recording stay the times at the server
side of the delay, so the replay sees the delay as the server sees it.

A trace is text. Each line holds one round trip time in milliseconds. A `#` starts a comment.
Put each trace in `tools/mcrs_minecraft_client_recorder/traces/<name>.rtt`.

To record a trace of a real connection, send ICMP echo requests at 10 each second for 5 minutes
to a host on a connection that players use, such as a game server of another region:

```sh
mkdir -p tools/mcrs_minecraft_client_recorder/traces
ping -i 0.1 -c 3000 <host> | sed -nE 's/.*time=([0-9.]+) ms/\1/p' \
    > tools/mcrs_minecraft_client_recorder/traces/<name>.rtt
```

Record one trace for each of these connections: a wired home connection, a home Wi-Fi with
other traffic, a mobile connection, and a host on another continent. Write the connection and the
date in a comment on the first line.

The trace of C7 stalls each direction for 3 seconds after 900 frames, again and again:

```sh
{ echo "# a stall of 3 s in each direction after 900 frames"; yes 20 | head -900; echo 6000; } \
    > tools/mcrs_minecraft_client_recorder/traces/freeze.rtt
```

## The commands for the conformance tests

Run each command, then do the steps of "Record one experiment". `<trace>` is a trace name.

| Test | Command |
|------|---------|
| E9 to E14, E21 to E23 | `MINECRAFT_EULA=true tools/mcrs_minecraft_client_recorder/record.sh e9`, then the same with `e10` to `e14` and `e21` to `e23` |
| The same with a delay | `MINECRAFT_EULA=true DELAY_TRACE=tools/mcrs_minecraft_client_recorder/traces/<trace>.rtt tools/mcrs_minecraft_client_recorder/record.sh e21 crates/mcrs_minecraft_move_predict/tests/fixtures/conformance/<trace>`, for each experiment |
| C1 | `MINECRAFT_EULA=true DELAY_TRACE=tools/mcrs_minecraft_client_recorder/traces/<trace>.rtt tools/mcrs_minecraft_client_recorder/record.sh c1 crates/mcrs_minecraft_move_predict/tests/fixtures/conformance/<trace>`, for each trace, and once without `DELAY_TRACE` |
| C2 | The C1 command with the output directory `.../conformance/fps_<rate>`. Before it, set Video Settings, Max Framerate, to 5, then 20, then 144 |
| C3 | `MINECRAFT_EULA=true tools/mcrs_minecraft_client_recorder/record.sh c3` |
| C4 | The C1 command with `c4` and a trace with a median of about 500 ms |
| C5 | Not recordable yet: see "Limits" |
| C6 | `MINECRAFT_EULA=true tools/mcrs_minecraft_client_recorder/record.sh c6` |
| C7 | `MINECRAFT_EULA=true DELAY_TRACE=tools/mcrs_minecraft_client_recorder/traces/freeze.rtt tools/mcrs_minecraft_client_recorder/record.sh c7` |

The experiments E21 to E23 send teleports. With a delay, these teleports arrive while the client
moves.

## Record a cheat module

Use a client with one movement cheat module and connect it through the tool as a vanilla client.
For each module of `fly`, `speed`, `timer`, `no_fall`, `phase` and `step`:

1. Run `MINECRAFT_EULA=true tools/mcrs_minecraft_client_recorder/record.sh c1 crates/mcrs_minecraft_move_predict/tests/fixtures/cheats/<module>`.
2. Turn the module on before the first step, and keep it on.
3. Do the steps. Use the module in each step where it can change the movement.

Do not commit a cheat client. Commit only the recordings.

## How the tool finds the tick of a clientbound packet

The client applies clientbound packets between two client ticks. After each clientbound packet
that the recording keeps, the proxy sends its own `ping`. The client answers a `ping` after it
applied the packets before it, and before the packets of its next tick. Thus the answer gives the
tick that applied the packet. The proxy drops the answers to its own `ping` packets, so the
server never sees them.

## The recording format

`mcrs_minecraft_client_ticks` reads and writes the format. A recording is text, one item for
each line. The first line is `client-ticks 2`. The header lines follow in this order:

| Line | Meaning |
|------|---------|
| `version <id>` | the game version |
| `experiment <name>` | the experiment, for example `E9` |
| `step <name>` | the step of the experiment |
| `instruction <text>` | the text that the human read |
| `fill <x0> <y0> <z0> <x1> <y1> <z1> <state>` | blocks of the step; all other blocks are air |

The line `events` ends the header. Each event line after it starts with the time of the event at
the proxy, in microseconds since the client connected. The time of a clientbound event is the
time at which the proxy received it from the server. The time of a serverbound event is the time
at which the proxy passed it on to the server. Then `<` follows for a clientbound packet, or `>`
for a serverbound packet. A clientbound event stands where the client applied
it. The line `> tick` is `client_tick_end` and ends one client tick. A tick without a `> move`
line is a tick in which the client sent no move packet.

| Event | Packet |
|-------|--------|
| `< teleport <id> <x> <y> <z> <vx> <vy> <vz> <yaw> <pitch> <relative>` | `player_position` |
| `< motion <vx> <vy> <vz>` | `set_entity_motion` of the player |
| `< explosion <x> <y> <z>` | the impulse of an `explode` that has one |
| `< abilities <flying,may_fly> <flying_speed>` | `player_abilities` |
| `< attribute <name> <base> [<id> <amount> <operation>]...` | one entry of `update_attributes` |
| `< effect <name> <amplifier>` | `update_mob_effect` |
| `< remove_effect <name>` | `remove_mob_effect` |
| `< block <x> <y> <z> <state>` | `block_update`, or one entry of `section_blocks_update` |
| `< forget <x> <z>` | `forget_level_chunk` |
| `> input <keys>` | `player_input` |
| `> sprint <true or false>` | `player_command` that starts or stops the sprint state |
| `> use_item`, `> release_use_item` | `use_item`, `player_action` that releases the item |
| `> move <x,y,z or -> <yaw,pitch or -> <flags>` | one of the four move packets |
| `> tick` | `client_tick_end` |

A list of flags or keys has commas between its names, or is `none`. Each floating-point value is
the shortest decimal text that gives the same value again. The events before the first teleport
give the abilities, the attributes and the effects of the player at the start of the step.

## The scenario format

Each file in `scenarios/` is one experiment. The files `c1`, `c3`, `c4`, `c6` and `c7` are
conformance tests; C2 is C1 at other frame rates. A line that starts with `#` is a comment.

| Line | Meaning |
|------|---------|
| `experiment <name>` | the experiment |
| `property <key>=<value>` | a line of `server.properties` |
| `arena <x0> <y0> <z0> <x1> <y1> <z1>` | the cuboid that each step fills with air first |
| `step <name>` | starts a step |
| `instruction <text>` | the text for the human |
| `fill <x0> <y0> <z0> <x1> <y1> <z1> <state>` | a console `fill`; it also goes into the header |
| `run <command>` | a console command before the teleport |
| `start <x> <y> <z> <yaw> <pitch>` | the teleport that starts the step |
| `go <tick>` | the client tick of the title `Go`; 60 by default |
| `ticks <count>` | the client ticks of the step; 200 by default |
| `at <tick> <command>` | a console command at a client tick of the step |

The arena must have at most 32768 blocks, the limit of `fill`.

## Limits

- The tool accepts one client at a time. The server runs in offline mode on loopback.
- The replay maps block states to the small block table of the physics kernel tests. A state
  outside that table stops the replay with a message.
- The format has no entities near the player. The replay of layer 2 has no pushers, so a mob
  that touches the player can cause a flag. C4 uses a skeleton that shoots from a distance.
- The format has no `block_changed_ack` and no edit request of the client. Thus the replay cannot
  give a window to the own edits of the client, and C5 is not recorded. A vanilla server also has
  no plugin that refuses one block in two.
- The kernel applies the sprint boost itself. The replay finds the `movement_speed` modifiers
  that follow the `> sprint` events of the recording and leaves them out.
