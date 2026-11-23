# mcrs_minecraft_mcp

An MCP server (stdio) and a command line that build, start, wait on and stop the mcrs
server, the pinned vanilla server and the mcrs client. An agent waits for a log line,
never for a time.

## Use

- Build: `cargo build -p mcrs_minecraft_mcp`. The binary is `target/debug/mcrs_minecraft_mcp`.
- MCP: the project `.mcp.json` registers it as `mcrs`. For another checkout, run
  `claude mcp add mcrs --scope user -- <path>/target/debug/mcrs_minecraft_mcp`, then start a new session.
- Command line: `mcrs_minecraft_mcp <tool> '<json arguments>'`; `mcrs_minecraft_mcp help` lists the tools.
  The result goes to stdout; an error goes to stderr with exit code 1.

Every tool that touches a checkout takes `workdir`, the absolute path of the worktree.
The default is the directory where the binary started.

## Tools

| Tool | Example arguments | Result |
| - | - | - |
| `build` | `{"workdir":"/abs/worktree"}` | `ok in N s`, or the first 30 error lines |
| `server_start` | `{"workdir":"…","seed":"42","gamemode":"creative"}`; `"kind":"vanilla"` for the vanilla server | `id`, `pid`, `port`, run `dir`, `log` |
| `client_start` | `{"workdir":"…","server":"s1","username":"Bob"}`; `"headless":true` for no window | `id`, `pid`, `dir`, `log` |
| `wait` | `{"id":"s1","until":"server_ready"}`; also `client_joined`, `world_ready` or a regex; `timeout_s` (120) | the line that matched or the ready state, or the last 20 lines |
| `client_state` | `{"id":"c1","query":"entities 16"}` | one JSON line |
| `client_input` | `{"id":"c1","actions":["look_entity 12","attack"]}` | one line per action: the action and its reply |
| `command` | `{"id":"s1","commands":["summon minecraft:pig 0 100 0","data get entity @e[type=pig,limit=1] Pos"]}` | the reply lines of each command |
| `logs` | `{"id":"c1","level":"error"}`, or `"grep":"<regex>"`, `"limit":50` | at most `limit` (max 200) last lines |
| `screenshot` | `{"id":"c1","max_px":1024}`; it waits for `world_ready` unless `"wait":false` | the image and its file path |
| `stop` | `{"id":"s1"}` or `{"id":"all"}` | one line for each handle |

Both start tools take `env`, extra `MCRS_*` variables, for example `{"MCRS_VIEW":"8"}`.
Each handle gets a free port and its own run directory under `$TMPDIR/mcrs_minecraft_mcp/<id>`.
The client gets its own game directory there, so two runs do not share options or a port.

Typical run: `build`, `server_start`, `wait server_ready`, `client_start`, `wait world_ready`,
set up with `command`, act with `client_input`, read with `client_state` and `command`,
`screenshot`, `logs level=error`, `stop all`.

## Client control

`build` builds the client with the feature `control`. A build without it has no control
socket. `client_start` passes `--control <run dir>/c-<random>.sock`; the client opens the
socket and removes it on exit, and `stop` removes it too. One request line goes in, one
JSON line comes out. The client answers in its tick: an input applies at the start of the
tick, a read shows the state at that point.

| Request | Reply |
| - | - |
| `ready` | `joined`, `placed` (the player is loaded), `terrain` (the 5 x 5 columns around the player are held), `meshed` (null when headless), `world_ready` |
| `player` | position, `yaw`, `pitch`, `health`, `food`, `game_mode`, `on_ground`, `slot` |
| `target` | `kind` `entity`, `block` or `none`: the pick of the crosshair from the last tick |
| `entities [RADIUS]` | at most 32 entities, nearest first: `id` (network id), `type`, position, `distance`; radius 16 by default |
| `block X Y Z` | `block` and `state` |
| `look YAW PITCH`, `look_at X Y Z`, `look_entity ID` | `{"done":true}` |
| `hold KEYS TICKS` | keys `forward,back,left,right,jump,sneak,sprint`; done after TICKS ticks |
| `attack [TICKS]` | one press, then the button stays down for TICKS ticks (1) |
| `slot N` | hotbar slot 0 to 8 |
| `chat TEXT`, `command TEXT` | sent as the player |

An error is `{"error":"..."}`. `client_input` stops at the first error.

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
- Our server does not answer every read command yet: `data get entity` of a player and
  `attribute` fail, `data get entity` of a mob and `execute if block` work.
