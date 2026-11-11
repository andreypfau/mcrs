# mcrs_minecraft_probe

The probe runs one scenario file against a server and writes a trace. A scenario is an experiment
of a specification under `docs/spec/`, written as steps of scripted clients. A trace holds what
each client observes at each step. The clients connect over TCP in offline mode.

The library has the scenario type, the trace type and the driver `run`. The driver takes the
scenario, the socket address of the server, the quiet window and a function that reads the player
ban file. It does
not start a server, and it does not apply the `target` part of the scenario. The host of the
server does that before it calls the driver.

The binary is the host for the vanilla dedicated server. It starts the server on a new flat world
in an empty directory. It writes the operators of the scenario to `ops.json` and the bans to
`banned-players.json` before the start.

## Scenario

A scenario is a JSON file in `scenarios/`.

| Field | Content |
|-------|---------|
| `target.operators` | the names of the players that are operators |
| `target.banned_players` | the content of the player ban file at the start |
| `steps` | the steps, each with an `id` and one action |

| Action | Effect |
|--------|--------|
| `"join": "<name>"` | a new client logs in with this name |
| `"leave": "<name>"` | the client closes its connection |
| `"command": { "client": "<name>", "line": "<command>" }` | the client sends a chat command, without the slash |

A step with `"read_player_bans": true` adds the player ban file after the step to the trace.

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
| `command` | `replies`: the system chat messages that the sender gets; `disconnects`: each client that the server disconnects, with the reason |

A text component is stored with its structure: the translation key, the arguments and the style.
A ban entry has the fields `uuid`, `name`, `source`, `expires` and `reason`. The entries are in
the order of their names. The trace has no times and no entity ids.

A step is complete when no client gets a chat message, a disconnect or a close for the quiet
window. The caller of the driver gives the quiet window. The binary uses 1 second for each
recording. A command and a leave wait at most 10 seconds for the first of these. A command that
gets none has an empty `replies` list.

The trace holds only the chat of the client that sends a command. It does not hold the chat that
other clients get, or the chat of a `join` or a `leave` step.

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
3. Run the script again. Make sure that `git diff` shows no change in the trace.
4. Commit the trace.

After an update of the game version, record each trace again.
