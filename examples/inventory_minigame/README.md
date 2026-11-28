# Inventory minigame

One headless app with no window. It links neither the packet edge of the inventory
(`mcrs_minecraft_inventory_server`) nor the click and client view crates. The engine
still links the network crates today. Each feature of the minigame is one plugin in
its own file, and `src/main.rs` adds the plugins.

- `KitPlugin`: at `EnterGame`, the plugin keeps the items of the player in a
  `KitSnapshot` and gives the player the kit. At `LeaveGame`, the player gets the
  items of the snapshot back, and the snapshot goes away.
- `ShopPlugin`: at `OpenShop`, the player gets a shop menu. Pressing an offer entry gives the
  player the offered stack.

```sh
cargo run -p mcrs_minecraft_inventory_minigame
```

The app uses the engine item and vanilla inventory crates, and no server, client or
inventory packet crate. The integration test has one case for each plugin.

```sh
cargo nextest run -p mcrs_minecraft_inventory_minigame
cargo nextest run -p xtask
```
