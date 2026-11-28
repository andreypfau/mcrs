# Player attributes

Read a player file and print each attribute of the player with its base, its
modifiers and its value. Set a base and write the file back.

```sh
cargo run -p mcrs_minecraft_player_attributes -- players/data/<uuid>.dat
cargo run -p mcrs_minecraft_player_attributes -- players/data/<uuid>.dat --set max_health=30
```

A row has the attribute, the base and the value. A `*` after the base marks a
base that differs from the base of the type `player`. Each modifier is a line
under its row, with its identifier, its operation and its amount. The value is
the value of ATR-VAL-001 in `docs/spec/attributes.md`.

`--set <attribute>=<base>` can occur more than once. A bare attribute name is in
the `minecraft` namespace. The tool sets each base, writes the list `attributes`
as ATR-SAV-001 states, and prints the rows that changed. It keeps every other key
of the file. A rewrite normalises the list as a load and a save of the game do,
and the tool prints on stderr what of the list it does not keep as it is. A
`--set` that changes nothing prints `no change; the file is not written` and
leaves the file. An unknown attribute, an attribute outside the set of the player, a
base that is not a finite number, or a file that does not load is an error, and
the tool does not write the file. A file with another `DataVersion` does not load.

The executable uses `mcrs_minecraft_entity_attribute`, the bases of the player
from `mcrs_minecraft_entity`, `mcrs_minecraft_save` for the file and
`mcrs_minecraft_nbt`. Its entry in `examples/dependencies.json` forbids Bevy,
tokio, the engine, the server, the client, the network crates, the vanilla
plugins, world generation, rendering, the GPU and window crates, the chat and
command crates and the game protocol.

```sh
cargo nextest run -p mcrs_minecraft_player_attributes
cargo nextest run -p xtask
```
