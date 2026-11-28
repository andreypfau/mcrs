# Inventory bot click

A headless bot that joins a server and clicks in its inventory menu. It links no window,
no render crate, no GUI crate and no model crate. The inventory comes from
`ClientRegistriesPlugin`, which adds the holders of the local player and the inventory
edge of the client (`mcrs_minecraft_inventory_client`).

- `HeadlessClientPlugin`: the connection, the session registries, the game mode and the
  abilities.
- `JoinServerPlugin`: joins one server as an offline player at startup.
- `ClickScriptPlugin`: when the `ClickScript` resource exists and the inventory menu has
  a state id from the server, the bot sends one mouse gesture each frame until it has
  done each step of `ClickScript::STEPS`. Each step is a press and a release of one mouse
  button over one menu entry. The bot turns each gesture into a `Click` through
  `Click::from_input`, the same function that a GUI uses, and writes a `MenuInput` with
  `MenuAction::Click`. The abilities of the local player give the creative flag. The
  client predicts the click and sends its packet in the same frame.

A press with an empty cursor clicks, and the bot then drops the release that follows, as a
GUI must. A press with a stack on the cursor clicks nothing, and the release puts the
stack down.

The script picks up the stack of the first hotbar slot, puts it in the first main
inventory slot, takes half of it back with a right click, puts that half in the next
slot, and moves that half back to the hotbar with a shift click. Start it with a stack in
the first hotbar slot, the first two main inventory slots empty and an empty cursor. With no stack in the
first hotbar slot, the clicks change nothing.

```sh
cargo run -p mcrs_minecraft_inventory_bot_click -- 127.0.0.1:25565 Bot assets <vanilla data pack folder>
```

The vanilla data pack folder is the one that `cargo run -p mcrs_minecraft_client_jar --example fetch`
installs. The integration test runs the bot against our server in the same process. It
gives the bot a stack, runs the script, and checks the final slots and that the server
sent no correction.

```sh
cargo nextest run -p mcrs_minecraft_inventory_bot_click
cargo nextest run -p xtask
```
