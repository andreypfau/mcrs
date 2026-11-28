# Inventory save viewer

Read the inventory of a player file and print one line for each stack (slot, count,
item name), the selected slot, and each kept key of `equipment`.

```sh
cargo run -p mcrs_minecraft_inventory_save_viewer -- world/playerdata/<uuid>.dat assets/mcrs/item_definition
```

The first argument is a compressed player file. The second argument is the directory
of the item definitions; the viewer takes the item names from it. The slot numbers
follow INV-SLT-001 and INV-SAV-001 in `docs/spec/inventory.md`.

The executable uses the inventory file, item table and NBT crates without Bevy and
without tokio. The item table has the item names only, so a stack whose component
patch names an entry of another registry, such as an enchantment, fails the read.

```sh
cargo nextest run -p mcrs_minecraft_inventory_save_viewer
cargo nextest run -p xtask
```
