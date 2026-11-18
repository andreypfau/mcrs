# mcrs_minecraft_probe

The probe runs one scenario file against a server and writes a trace. A scenario is an experiment
of a specification under `docs/spec/`, written as steps of scripted clients. A trace holds what
each client observes at each step. The clients connect over TCP in offline mode.

The library has the scenario type, the trace type, the trait `Host` and the driver `run`. The
driver takes the scenario, the socket address of the server, the quiet window and the host. The
host reads the player ban file, reads a saved player file and restarts the server. The driver does
not start a server, and it does not apply the `target` part of the scenario. The host of the
server does that before it calls the driver.

The binary is the host for the vanilla dedicated server. It starts the server on a new flat world
in an empty directory. It writes the operators of the scenario to `ops.json` and the bans to
`banned-players.json` before the start. It reads a player file from `world/players/data`.

## Scenario

A scenario is a JSON file in `scenarios/`.

| Field | Content |
|-------|---------|
| `target.operators` | the names of the players that are operators |
| `target.operator_levels` | optional map of operator name to permission level `1..=4` (default `4`) |
| `target.banned_players` | the content of the player ban file at the start |
| `observe.inventory` | `true` records the menu packets of each step; the default is `false` |
| `observe.sound` | `true` records the sound packets of each step; the default is `false` |
| `observe.commands` | a list of command names; each step records, for each `commands` packet, the subtree of each named literal child of the root; the default is the empty list |
| `observe.swing` | `true` records the `swing_animation` packets of each step; the default is `false` |
| `observe.animate` | `true` records the `animate` packets of each step; the default is `false` |
| `observe.packet_order` | `true` allows steps with `packet_order: true` to record tracked packet arrival order; the default is `false` |
| `observe.chat` | `true` records system chat messages received by non-sender clients during `command` steps; the default is `false` |
| `steps` | the steps, each with an `id`, one action, and optional flags (`packet_order`) |

| Action | Effect |
|--------|--------|
| `"join": "<name>"` | a new client logs in with this name |
| `"join_deferred": "<name>"` | a new client logs in with this name without sending `player_loaded` |
| `"send_loaded": "<name>"` | the client sends `player_loaded` |
| `"sync_position": "<name>"` | the client sends `move_player_pos_rot` for its last accepted teleport pose |
| `"leave": "<name>"` | the client closes its connection |
| `"command": { "client": "<name>", "line": "<command>" }` | the client sends a chat command, without the slash |
| `"suggest": { "client": "<name>", "line": "<command>" }` | the client requests command suggestions for the text up to the cursor |
| `"client_main_arm": { "client": "<name>", "arm": "left" \| "right" }` | the client sends `client_information` with the named main arm |
| `"punch_extra_byte": { "client": "<name>" }` | the client sends `punch` with one extra trailing byte |
| `"console": { "lines": ["<command>", ...] }` | the host runs the commands on the server console, in order, without the slash; the step records the replies that the server printed |
| `"click": { "client", "slot", "button", "mode", "container", "state_id_offset", "changed_slots", "carried" }` | the client sends a container click |
| `"close": { "client", "container" }` | the client sends a container close |
| `"select_slot": { "client", "slot" }` | the client sends the selected hotbar slot; the value goes on the wire as it is, so 9 and -1 are possible |
| `"player_action": { "client", "status", "position", "face" }` | the client sends a player action with optional `position` (default `[0, 0, 0]`) and `face` (default `down`), sequence `0` |
| `"creative_slot": { "client", "slot", "stack" }` | the client sets a slot in creative mode; the count can be more than 64 |
| `"use_block": { "client", "position": [x, y, z] }` | the client uses the main hand on the top face of the block, with the cursor at the centre of the face |
| `"punch": { "client": "<name>" }` | the client sends `punch` |
| `"punch_burst": { "client": "<name>", "count": <n> }` | the client sends `count` `punch` packets without delay |
| `"use_item": { "client", "hand", "sequence" }` | the client sends `use_item` for `main` or `off` hand |
| `"use_item_on": { "client", "hand", "position": [x, y, z], "face", "sequence" }` | the client sends `use_item_on` at the block position and face |
| `"attack": { "client", "target" }` | the client sends `attack` against the nearest tracked entity matching `target` |
| `"interact": { "client", "target", "hand", "secondary" }` | the client sends `interact` against `target`; optional `secondary` defaults to `false` |
| `"wait": { "ms": <n> }` | the driver waits `ms` milliseconds and settles events |
| `"await_packet": { "client": "<name>", "packet": { "swing": { "subject": "<name>" } } }` | waits up to ten seconds for a matching packet received since the step began. A swing predicate requires `observe.swing`. Other predicates are `add_entity` and `remove_entity` with `subject`, `block_ack` with `sequence`, and `player_position` with `block_center: [x, y, z]` |
| `"advance_ticks": { "ticks": <n> }` | within five seconds verifies the server is frozen, steps the world by `n` ticks, and waits until the reported game time advances by exactly `n`. With zero ticks, it checks that world time holds |
| `"await_player_ticks": { "client": "<name>", "ticks": <n> }` | waits up to 30 seconds for the player's timed Luck effect to decrease by at least `n` ticks. Give the player Luck beforehand and allow enough duration for both samples |
| `"sequence": { "steps": [...] }` | runs inner `send_loaded`, `sync_position`, `click`, `close`, `select_slot`, `player_action`, `creative_slot`, `use_block`, `punch`, `punch_burst`, `use_item`, `use_item_on`, `attack`, `interact`, `client_main_arm`, `punch_extra_byte`, `console`, `wait`, `await_packet`, `advance_ticks`, and `await_player_ticks` in order, then settles events once at the end |
| `"restart_server": {}` | the driver closes the connection of each client; then the host stops the server, which saves the world, and starts it again on the same world and port. A later step of such a client needs a new `join` |

A click has these fields:

- `mode` is the click kind: `pickup`, `quick_move`, `swap`, `clone`, `throw`, `quick_craft` or
  `pickup_all`. The packet type has no field for a kind outside this list.
- `container` is optional. The default is the container of the last `open_screen` that the client
  got. After a close in one of the two directions, the default is 0.
- Each menu has its own state id: the inventory menu (container 0) and the open container. The
  client sends the last state id that it got for the container of the click, plus
  `state_id_offset`. For a container that is not open, the state id is 0. The default offset is 0.
- `changed_slots` is a list of `{ "slot", "stack" }`, and `carried` is a stack. They are the
  prediction of the scenario. The defaults are no changed slots and no carried stack.

A stack in a scenario is `null` or `{ "item": "<name>", "count": <n> }`, with no components.
A close without `container` uses the same default as a click.
A click and a close need `observe.inventory`. A scenario with a click or a close and without it
does not load.

The status of a player action is one of `start_destroy_block`, `change_destroy_direction`,
`abort_destroy_block`, `stop_destroy_block`, `drop_all_items`, `drop_item`, `release_use_item`,
`swap_item_with_offhand` or `stab`.

The probe does not respawn a client, and it records no health packet and no combat packet. Thus a
scenario must prevent the death of its players, for example with `difficulty peaceful` as its
first command.

A step with `"read_player_bans": true` adds the player ban file after the step to the trace.
A step with `"read_player_file": { "client": "<name>", "fields": [...] }` adds the named top-level
fields of the saved player file after the step to the trace. The harness does not save the world.
Put a `save-all flush` command in the step.

With `observe.inventory`, `observe.swing`, `observe.animate` or `observe.packet_order`, each client answers a position sync with an accept and the end of a chunk batch with a receipt. With `join`, each client sends `player_loaded` once after login. `join_deferred` waits for an explicit `send_loaded` step. Scenarios with `attack`, `interact`, or `use_item` acknowledge `player_position` so target lookups and item use have the current position or look. Chunk receipts, automatic `player_loaded`, and the `sync_position` action still require an observation flag from the first sentence.

Use long lower-case names that no account holds. The vanilla server asks the account service for
a name that is not online.

## Trace

A trace is a JSON file in `traces/`. Its header has the name and the protocol number of the game
version. Each step has one result:

| Result | Content |
|--------|---------|
| `joined` | the client is in the game state |
| `refused` | the connection state (`login`, `configuration` or `play`) and the reason; the reason is `null` when the server closes the connection without a disconnect packet |
| `left` | no content |
| `command` | `replies`: the system chat messages that the sender gets; `chat`: optional non-sender client system chat when `observe.chat` is `true`, grouped by client; `disconnects`: each client that the server disconnects, with the reason |
| `suggestions` | `start`, `length`, `suggestions`, `disconnects`: the suggestion range and candidate strings that the client gets |
| `sent` | `disconnects`, as for `command`; `console_output`: optional replies from `console` steps inside a `sequence`; the result of a step that sends a game packet |
| `console` | `output`: the reply texts that the server printed for the commands, in order, without the log prefix; `chat`: the system chat messages that the clients received, each with `client` and `text`, grouped by client in order of arrival |
| `restarted` | no content |

With `observe.inventory`, each step also has `inventory`: the menu packets that the clients get
in the step, in the order of their arrival. A join step has them too. Each entry has the `client`,
the `kind` and the fields of the packet:

| Kind | Fields |
|------|--------|
| `open_screen` | `container`, `menu` (the name of the menu type), `title` (a text component) |
| `container_close` | `container` |
| `container_set_content` | `container`, `state_id`, `slots` (a stack for each slot), `carried` |
| `container_set_slot` | `container`, `state_id`, `slot`, `stack` |
| `container_set_data` | `container`, `id`, `value` |
| `set_cursor_item` | `stack` |
| `set_player_inventory` | `slot`, `stack` |
| `set_held_slot` | `slot` |

A stack in a trace is `null` or `{ "item", "count", "components" }`. The item is a name.
`components` has the lists `added` and `removed` with the names of the component types of the
patch. The trace does not hold the values of the added components.

With `observe.sound`, each step also has `sounds`: the sound packets that the clients get in the
step, in the order of their arrival. Each entry has the `client`, the `sound` (the name of the
sound event), the `source` (the number of the sound category as sent), the `position` (three
integers as sent, in eighths of a block), the `volume`, the `pitch` and the `seed`. The seed is
random in each run. A packet with an inline sound event stops the run with an error.

With `observe.commands`, each step also has `command_trees`: one entry for each `commands` packet
that the clients get in the step, with the `client`, the `node_count` and `roots`. The roots are
the subtrees of the named children of the root. A node has its `index` in the packet, `node`
(`literal` or `argument`), `name`, `executable`, `restricted`, an optional `redirect` index and its
`children`. An argument also has `parser` (the name of the argument type), `properties` (the
decoded properties as debug text) and an optional `suggestions` name.

With `observe.swing`, each step also has `swings`: each `swing_animation` packet that the
clients receive in the step, grouped by client in the order of arrival, with `client`, `subject`
(player name, entity type name, or `entity:<id>`), `hand` (`main` or `off`), `animation` (`none`,
`whack`, or `stab`), and `duration`. When `swings` is non-empty, `swing_bytes` records `canonical`
(`true` when every wire body equals canonical `VarInt` encoding) and optional
`shared_across_recipients` (`true` when every multi-recipient `(entity_id, occurrence_index)`
group received identical wire bytes across recipients). This field is an internal byte-equality
summary over observed deliveries, not a standalone conformance proof: because deliveries are aligned
only by `(entity_id, occurrence_index)` without knowing expected recipients, an omitted recipient or
a mid-step visibility change shifts occurrence alignment and prevents `shared_across_recipients`
from proving all expected recipients received matching payloads.

With `observe.animate`, each step also has `animates`: each `animate` packet that the clients
receive in the step, grouped by client in the order of arrival, with `client`, `subject`, and raw
`action` id.

With `observe.packet_order` and `"packet_order": true` on a step, the step also has `packets`:
`{ "client", "packet" }` entries for `swing_animation`, `sound`, `block_update`,
`section_blocks_update`, `set_equipment`, and `set_entity_data`, grouped by client and preserving
arrival order within each client.

`await_packet`, `advance_ticks`, and `await_player_ticks` add successful checkpoints to the step's
`checkpoints` in action order. Packet checkpoints identify the receiving client and the matched
predicate. A `player_position` predicate matches the accepted position at `x + 0.5, y, z + 0.5`
after the probe writes the teleport acknowledgement. A packet read while waiting for a different
predicate stays available to later checkpoints within the step. Game-tick checkpoints store the
verified world-time delta. Player-tick checkpoints store the minimum measured interval. A failed
checkpoint stops the recording.

A step with `read_player_file` has `player_file`: each named field as SNBT text, with the keys of
each compound in order. A field that the file does not have is `null`.

A text component is stored with its structure: the translation key, the arguments and the style.
A ban entry has the fields `uuid`, `name`, `source`, `expires` and `reason`. The entries are in
the order of their names. The trace has no times and no entity ids.

A step is complete when no client gets a chat message, a menu packet, a disconnect or a close for
the quiet window. The caller of the driver gives the quiet window. The binary uses 1 second for each
recording. A command and a leave wait at most 10 seconds for the first of these. A step that sends
a game packet waits at most 2 seconds. With `observe.inventory`, a join also waits at most 2 seconds.
A command that gets none has an empty `replies` list. When the connection of the sender of a game
packet closes without a disconnect packet, the run stops with an error.

A console step does not wait for a chat message, but it waits for the quiet window after the
server printed the last reply, so that the clients get the sound and tree packets of the commands.
A console step does not respawn a client: a player that a command kills stays on the respawn
screen.

Without `observe.chat`, the trace holds only the chat of the client that sends a `command` step
and `console` step chat; with `observe.chat: true`, non-sender chat on `command` steps is recorded
too. It does not hold the chat of a `join` or a `leave` step.

## Known differences

The file `traces/<scenario>.mcrs.json` lists the steps where the mcrs server gives a result that is
not in the trace. Each entry has the fields of a trace step with the result of mcrs, and a `reason`.
A test of the mcrs server compares a step in the list with its entry, and each other step with the
trace. Remove or change an entry when the result of mcrs changes.

## Record a trace

You need Java 25 or later and `jq`.

1. Read https://aka.ms/MinecraftEULA.
2. Run the script with the name of the scenario:

   ```sh
   MINECRAFT_EULA=true tools/mcrs_minecraft_probe/record.sh bans
   ```

   The script downloads and verifies the server jar, runs the scenario and writes
   `traces/<scenario>.json`.
3. Run the script again. Make sure that `git diff` shows no change in the trace. A trace with
   `observe.sound` differs in the seeds and in the player identifiers of the text components.
4. Commit the trace.

After an update of the game version, record each trace again.
