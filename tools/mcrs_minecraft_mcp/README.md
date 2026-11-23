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
| `client_start` | `{"workdir":"…","server":"s1","username":"Bob"}` | `id`, `pid`, `dir`, `log` |
| `wait` | `{"id":"s1","until":"server_ready"}`; also `client_joined` or a regex; `timeout_s` (120) | the line that matched, or the last 20 lines |
| `command` | `{"id":"s1","commands":["summon minecraft:pig 0 100 0","data get entity @e[type=pig,limit=1] Pos"]}` | the reply lines of each command |
| `logs` | `{"id":"c1","level":"error"}`, or `"grep":"<regex>"`, `"limit":50` | at most `limit` (max 200) last lines |
| `screenshot` | `{"id":"c1","max_px":1024}` | the image and its file path |
| `stop` | `{"id":"s1"}` or `{"id":"all"}` | one line for each handle |

Both start tools take `env`, extra `MCRS_*` variables, for example `{"MCRS_VIEW":"8"}`.
Each handle gets a free port and its own run directory under `$TMPDIR/mcrs_minecraft_mcp/<id>`.
The client gets its own game directory there, so two runs do not share options or a port.

Typical run: `build`, `server_start`, `wait server_ready`, `command`, `client_start`,
`wait client_joined`, `screenshot`, `logs level=error`, `stop all`.

## Behaviour

- The MCP server stops all its children when it exits, also on `SIGKILL`: a watchdog
  shell holds a pipe to it and stops each process group when the pipe closes.
- Handles that the command line starts outlive the call. `stop all` from the command line
  stops all handles that the command line started.
- `command` writes to the console of the server through a FIFO in the run directory. It
  returns the console answers that appear within `window_ms` (2000) after each command.
- `screenshot` uses the capture trigger file of the client. It needs a drawn window.

## Limits

- No input, no headless client, no web client, no game state other than through commands.
- `server_ready` and `client_joined` are log lines. A change of those lines breaks them.
- `build` builds the release profile only.
