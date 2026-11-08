# vanilla-client

Starts the vanilla client of the corpus version without a human, for the recorder of
`tools/mcrs_minecraft_client_recorder`. This is an oracle tool in the sense of
`docs/clean-room.md`: its agent knows names of the client, so code of `crates/` never copies from
it. It changes nothing that the client computes: no physics, no movement and no packet.

```sh
MINECRAFT_EULA=true tools/vanilla-client/run.sh SERVER INPUT_FILE [MAX_FPS]
```

`run.sh` downloads the client jar, the libraries for macOS and the assets of the package in
`crates/mcrs_minecraft_client_jar/src/release.json` to `target/vanilla-client`, and checks the
SHA-1 of each file. It builds the input agent with `javac` (Java 25), writes `options.txt`, and
starts the client with an offline name. The client joins `SERVER` at once.

## The input agent

`agent/mcrs/input/Agent.java` adds one call at the start of each client tick. The call
(`Input.java`) does these steps:

1. When the title `Go` shows, it reads `INPUT_FILE`. The first line is `go <tick>`; each other
   line is an `input` line of the scenario format of the recorder, without the keyword.
2. In each client tick, it sets the down state of the key mappings of the line of the tick:
   forward, backward, left, right, jump, sneak, sprint and use.
3. At the tick of a line with a rotation, it sets the yaw and the pitch of the player.
4. When the title `Stop` shows, it releases each key.

The game reads the key mappings as it reads the keyboard, so the client does what a human with
the same keys does. The agent takes no keyboard and no mouse input, and works without focus.

## The window

SDL takes two hints from the environment: `SDL_MAC_BACKGROUND_APP=1` and
`SDL_WINDOW_ACTIVATE_WHEN_SHOWN=0`. The client is then a background application, and its window
never takes the focus. Without `MAX_FPS`, the agent minimises the window. A minimised client
draws 10 frames each second; it still runs 20 client ticks each second. `MAX_FPS` keeps the
window open and sets `maxFps` with vsync off. `inactivityFpsLimit:minimized` keeps the frame
rate when nobody touches the keyboard.
