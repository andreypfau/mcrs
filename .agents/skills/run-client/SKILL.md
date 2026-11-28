---
name: run-client
description: Run the mcrs server and the native or the browser client against it, take a screenshot and look at it, then stop both. Load before you report a change to a client, render, GUI, mesh or model crate as done, and when the owner asks to run the client or to see a change. Triggers - run the client, start the client, launch the game, screenshot, check visually, web client, build-web.
---

# Run the client and look at it

A change that the owner sees on screen is done only when you saw it on screen. Run the client,
look at the screenshot with the Read tool, and say what you saw. If you did not run it, say so.

Work in your own worktree. Start the processes from a scratch folder, for example
`/tmp/run-client`. The server writes `whitelist.json` and `banned-players.json` into that
folder, and the client writes `screenshots/`. Save the process id of each process you start.

## 1. Check the screen

```bash
ioreg -n Root -d1 -a | grep -A1 ScreenIsLocked
```

A `<true/>` after the key means the screen is locked. No output or `<false/>` means the screen is
unlocked. If the screen is locked, stop. Tell the owner: "The screen is locked. A capture on a
locked screen never settles. Unlock the screen and ask me again."

## 2. Build

```bash
cargo build --release -p mcrs --bin mcrs -p mcrs_minecraft_client --bin mcrs_minecraft_client
```

The binaries read the assets from the folder in `BEVY_ASSET_ROOT`. `cargo run` sets it from
`.cargo/config.toml`. When you start a binary from `target/release` directly, set it to the
worktree root. Without it the server stops with "the registries are unusable".

The game jar comes from the local launcher folder. Without it, the first start downloads the
jar, which needs network access.

## 3. Server

```bash
cd /tmp/run-client
BEVY_ASSET_ROOT=<worktree> MCRS_PORT=25599 <worktree>/target/release/mcrs
```

- Run it as a background command. Pick a free port; another session can hold 25565.
- An argument is a world folder: `mcrs "<world>"`. Without one, the server generates a world.
- It is ready at the log line `entering Playing state`. The line
  `WebTransport listening on ... (certificate hash <hex>)` gives the hash for the browser.

## 4. Native client

```bash
cd /tmp/run-client
BEVY_ASSET_ROOT=<worktree> MCRS_SERVER=127.0.0.1:25599 MCRS_CAPTURE=/tmp/run-client/shot.png \
  <worktree>/target/release/mcrs_minecraft_client > /tmp/run-client/client.log 2>&1
```

- `MCRS_CAPTURE=<file>.png` waits until the chunks settle, writes one screenshot and exits
  with 0. This is the smoke run.
- Without `MCRS_SERVER`, the client hosts its own server. A world folder is then its argument.
- For a longer look, start the client without `MCRS_CAPTURE` as a background command. Then
  `touch /tmp/run-client/screenshots/capture` writes `screenshots/<millis>.png`. `MCRS_SCREENSHOT_DIR` moves that folder.
- Other knobs are `MCRS_*` variables: `MCRS_VIEW` (render distance), `MCRS_STATS=1` (debug
  lines in the log). Read `crates/mcrs_minecraft_client/src/config.rs` for the list.

## 5. Web client

```bash
cd <worktree>
scripts/build-web.sh                 # writes target/bundle/mcrs.html
BACKEND=webgl2 scripts/build-web.sh  # writes target/bundle/mcrs-webgl2.html
```

- The script needs the `wasm-bindgen` CLI of the version in `Cargo.lock`. It names the
  install command when the version differs. `wasm-opt` is optional.
- Open the page in a Chrome of its own, with a scratch profile, as a background command. Pass
  the port and the certificate hash of the server log.

  ```bash
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
    --user-data-dir=/tmp/run-client/chrome --no-first-run --new-window \
    "file://<worktree>/target/bundle/mcrs.html?server=https://127.0.0.1:25599&cert=<hex>"
  ```

- Do not open the page with `open -a`. It goes into the browser of the owner, with private
  tabs beside it.
- Without `?server=` the page draws no world. Other knobs are query parameters in
  lower case, for example `&view=8`.

## 6. Screenshot of a window on macOS

- Never capture the whole screen or a screen rectangle. The front window is often a private
  window of the owner, not yours.
- For the native client, use `MCRS_CAPTURE` or the trigger file. They capture the client
  surface only.
- For the browser, capture the window of your Chrome process by its window id:

  ```bash
  id=$(osascript -l JavaScript -e "ObjC.import('CoreGraphics');
    ObjC.deepUnwrap(ObjC.castRefToObject(\$.CGWindowListCopyWindowInfo(\$.kCGWindowListOptionAll, 0)))
    .filter(w => w.kCGWindowOwnerPID === <chrome pid> && w.kCGWindowLayer === 0
      && w.kCGWindowBounds.Height > 100)
    .map(w => w.kCGWindowNumber)[0]")
  screencapture -x -o -l "$id" /tmp/run-client/web.png
  ```

- Wait until the page has loaded and streamed the chunks.
- Scale a large image before you read it: `sips -Z 1200 shot.png --out shot-small.png`.

## 7. What to check

- The world is drawn: terrain, sky and clouds, not a single colour or a black frame.
- The changed element is on screen and matches the specification or the issue.
- No text is missing, no texture is magenta and black, no GUI part is out of place.
- The client log has no `ERROR` and no panic. Read it with
  `grep -E "ERROR|panicked" /tmp/run-client/client.log`.

## 8. Stop

- Kill the client and the server by their process ids: `kill <pid>`.
- A smoke run with `MCRS_CAPTURE` stops the client by itself.
- Kill the Chrome process that you started; delete its scratch profile.
- Delete the screenshots and the scratch folder when the owner has seen them.
