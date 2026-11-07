# mcrs_minecraft_client_recorder

Records the client ticks of a vanilla client for the player experiments of
`docs/spec/entity-physics.md`, section 15: E9 to E14 and E21 to E23. The tool starts the
vanilla dedicated server of the corpus version, listens as a proxy in front of it, and sets
each step of an experiment up through the server console. A human plays the vanilla client.
The tool writes one recording for each step into
`crates/mcrs_minecraft_physics/tests/fixtures/player`. The ignored test
`own_player_follows_the_recorded_client_ticks` in
`crates/mcrs_minecraft_physics/tests/recorded_player.rs` replays these recordings.

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
`crates/mcrs_minecraft_physics/tests/fixtures/player/<experiment>_<step>.txt` exists, then
commit them. To record a step again, run the experiment again.

## How the tool finds the tick of a clientbound packet

The client applies clientbound packets between two client ticks. After each clientbound packet
that the recording keeps, the proxy sends its own `ping`. The client answers a `ping` after it
applied the packets before it, and before the packets of its next tick. Thus the answer gives the
tick that applied the packet. The proxy drops the answers to its own `ping` packets, so the
server never sees them.

## The recording format

`mcrs_minecraft_client_ticks` reads and writes the format. A recording is text, one item for
each line. The first line is `client-ticks 1`. The header lines follow:

| Line | Meaning |
|------|---------|
| `version <id>` | the game version |
| `experiment <name>` | the experiment, for example `E9` |
| `step <name>` | the step of the experiment |
| `instruction <text>` | the text that the human read |
| `entity <id>` | the network id of the player |
| `fill <x0> <y0> <z0> <x1> <y1> <z1> <state>` | blocks of the step; all other blocks are air |

The line `events` ends the header. Each event line after it starts with `<` for a clientbound
packet or with `>` for a serverbound packet. A clientbound event stands where the client applied
it. The line `> tick` is `client_tick_end` and ends one client tick. A tick without a `> move`
line is a tick in which the client sent no move packet.

| Event | Packet |
|-------|--------|
| `< teleport <id> <x> <y> <z> <vx> <vy> <vz> <yaw> <pitch> <relative>` | `player_position` |
| `< motion <vx> <vy> <vz>` | `set_entity_motion` of the player |
| `< explosion <x> <y> <z>` | the impulse of an `explode` that has one |
| `< abilities <flags> <flying_speed> <walking_speed>` | `player_abilities` |
| `< attribute <name> <base> [<id> <amount> <operation>]...` | one entry of `update_attributes` |
| `< effect <name> <amplifier> <duration> <flags>` | `update_mob_effect` |
| `< remove_effect <name>` | `remove_mob_effect` |
| `< block <x> <y> <z> <state>` | `block_update`, or one entry of `section_blocks_update` |
| `< chunk <x> <z>`, `< forget <x> <z>` | `level_chunk_with_light`, `forget_level_chunk` |
| `> accept_teleport <id> <x> <y> <z> <yaw> <pitch>` | `accept_teleportation` |
| `> input <keys>` | `player_input` |
| `> command <action>` | `player_command` |
| `> flying <true or false>` | the serverbound `player_abilities` |
| `> use_item`, `> release_use_item` | `use_item`, `player_action` that releases the item |
| `> move <x,y,z or -> <yaw,pitch or -> <flags>` | one of the four move packets |
| `> tick` | `client_tick_end` |

A list of flags or keys has commas between its names, or is `none`. Each floating-point value is
the shortest decimal text that gives the same value again. The events before the first teleport
give the abilities, the attributes and the effects of the player at the start of the step.

## The scenario format

Each file in `scenarios/` is one experiment. A line that starts with `#` is a comment.

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
