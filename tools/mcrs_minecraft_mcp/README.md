# mcrs_minecraft_mcp

Your `stop all` stops only your handles. Pass `--session` if you work in more than one worktree.

An MCP server (stdio) and a command line that build, start, wait on and stop the mcrs
server, the pinned vanilla server and the mcrs client. An agent waits for a log line,
never for a time.

## Use

- Build: `cargo build -p mcrs_minecraft_mcp`. The binary is `target/debug/mcrs_minecraft_mcp`.
- MCP: the project `.mcp.json` registers it as `mcrs`. For another checkout, run
  `claude mcp add mcrs --scope user -- <path>/target/debug/mcrs_minecraft_mcp`, then start a new session.
- Command line: `mcrs_minecraft_mcp <tool> '<json arguments>'`; `mcrs_minecraft_mcp help` lists the tools.
  The result goes to stdout; an error goes to stderr with exit code 1.

Each handle belongs to a session. A command-line call takes its session from
`mcrs_minecraft_mcp --session <name> <tool> '<json>'`, else from `MCRS_MCP_SESSION`, else it
uses the current directory. The working directory of an agent is its worktree, and it does not
change between two Bash calls. An MCP server is one session. `stop` refuses a handle of another
session unless `"force":true`.

Every tool that touches a checkout takes `workdir`, the absolute path of the worktree.
The default is the directory where the binary started.

## Tools

| Tool | Example arguments | Result |
| - | - | - |
| `build` | `{"workdir":"/abs/worktree"}` | `ok in N s`, or the first 30 error lines |
| `server_start` | `{"workdir":"…","seed":"42","gamemode":"creative"}`; `"kind":"vanilla"` for the vanilla server | `id`, `pid`, `port`, run `dir`, `log` |
| `client_start` | `{"workdir":"…","server":"s1","username":"Bob"}`; `"headless":true` for no window | `id`, `pid`, `dir`, `log` |
| `wait` | `{"id":"s1","until":"server_ready"}`; also `client_joined`, `world_ready`, `on_ground` or a regex; `timeout_s` (120) | the line that matched or the ready state, or the last 20 lines |
| `client_state` | `{"id":"c1","query":"entities 16"}` | one JSON line |
| `client_input` | `{"id":"c1","actions":["look_entity 12","attack"]}` | one line per action: the action and its reply |
| `command` | `{"id":"s1","commands":["summon minecraft:pig 0 100 0","data get entity @e[type=pig,limit=1] Pos"]}` | the reply lines of each command |
| `logs` | `{"id":"c1","level":"error"}`, or `"grep":"<regex>"`, `"limit":50` | at most `limit` (max 200) last lines |
| `screenshot` | `{"id":"c1","max_px":1024}`; it waits for `world_ready` unless `"wait":false` | the image and its file path |
| `stop` | `{"id":"s1"}` or `{"id":"all"}` (the handles of this session); `"force":true` for a handle of another session | one line for each handle |
| `list` | `{}`, or `{"everyone":true}` for all sessions | one JSON line for each running handle: `id`, `kind`, `port`, `age_s`, `workdir`, `session` |

Both start tools take `env`, extra `MCRS_*` variables, for example `{"MCRS_VIEW":"8"}`.
Each handle gets a free port and its own run directory under `$TMPDIR/mcrs_minecraft_mcp/<id>`.
The client gets its own game directory there, so two runs do not share options or a port.
`list` drops a handle whose process is gone; its log stays in its run directory.

Typical run: `build`, `server_start`, `wait server_ready`, `client_start`, `wait world_ready`,
`wait on_ground`, set up with `command`, act with `client_input`, read with `client_state` and `command`,
`screenshot`, `logs level=error`, `stop all`.

## Client control

`build` builds the client with the feature `control`. A build without it has no control
socket, and `--control` makes it exit with an error. `client_start` passes
`--control <run dir>/control.sock`. The run directory has the mode 0700, so only its user
can reach the socket. The client refuses a path that holds a file that is not a socket, or
the socket of a live process. It removes the socket on exit, and `stop` removes it too.

One request line goes in, one JSON line comes out. A line is at most 4096 bytes. The client
reads one request in each tick, in `FixedUpdate` before the local player tick, so a read
after an input shows the effect of that input. A connection that waits for input costs one
`accept` call in each tick, and an open connection one `read` call.

The input is a second source beside the keyboard and the mouse: the client merges the held
keys and the attack button of the control with its own. The client refuses an attack while
a screen is open or the cursor is free.

| Request | Reply |
| - | - |
| `ready` | `{"ready":{...}}`: `joined`, `placed` (the player is loaded), `terrain` (the 5 x 5 columns around the player are held), `meshing` (`not_rendered`, `pending` or `done`), `world_ready` |
| `player` | `{"player":{...}}`: position, `yaw`, `pitch`, `health`, `food`, `game_mode`, `on_ground`, `slot`; `null` for a value the client did not receive yet |
| `target` | `{"target":{...}}`: `kind` `entity`, `block` or `none`, the pick of the crosshair now |
| `entities [RADIUS]` | `{"entities":[...]}`: at most 32 entities, nearest first: `id` (network id), `type`, position, `distance`; radius 16 by default |
| `block X Y Z` | `{"block":{...}}`: `block` and `state` |
| `look YAW PITCH`, `look_at X Y Z`, `look_entity ID` | `"done"` |
| `hold KEYS TICKS` | keys `forward,back,left,right,jump,sneak,sprint`; `"done"` after TICKS ticks |
| `attack [TICKS]` | one press, then the button stays down for TICKS ticks (1); `{"attacked":{...}}` with the pick at the press, as `target` |
| `slot N` | hotbar slot 0 to 8 |
| `chat TEXT`, `command TEXT` | sent as the player; the text is normalized as in the chat screen, and a chat message is at most 256 characters |

An error is `{"error":"..."}`. `client_input` stops at the first error. The crate
`mcrs_minecraft_client_control` holds the parser `Request` and the reply type `Reply`, and
this launcher uses both.

A release build without the feature `control` does not link the crate. The client adds the
plugin only for `--control`.

## Behaviour

- The MCP server stops all its children when it exits, also on `SIGKILL`: a watchdog
  shell holds a pipe to it and stops each process group when the pipe closes.
- Handles that the command line starts outlive the call. `stop all` from the command line
  stops all handles that the command line started.
- `command` writes to the console of the server through a FIFO in the run directory. It
  returns the console answers that appear within `window_ms` (2000) after each command.
- `screenshot` uses the capture trigger file of the client. It needs a drawn window.

## Limits

- No item use, no web client.
- `server_ready` and `client_joined` are log lines. A change of those lines breaks them.
- `build` builds the release profile only.
- `screenshot` needs a drawn window; a headless client has none.
- On a locked screen the windowed client draws nothing: `world_ready` stays false and a
  capture is black. The headless client works there.
- `world_ready` does not wait for the player to land. The player can fall from the spawn
  height for about 1 s. Use `wait on_ground` before you aim. A player that flies or swims
  never gets `on_ground`.
- `attack` hits the pick of the crosshair, not the entity of `look_entity`. A block in front
  of the entity takes the hit, and in creative the block breaks. The reply `attacked` names
  what the press hit.
- Our server does not answer every read command yet: `data get entity` of a player and
  `attribute` fail, `data get entity` of a mob and `execute if block` work.
