# Library examples

Each example is a package under `examples/<name>/` with its own `Cargo.toml`.
Add its path to the root `workspace.members` when you add the package. Use
`publish = false` and inherit the workspace edition and lints.

Add the package name to `examples/dependencies.json`. Its value is a list of
forbidden normal dependencies, including transitive dependencies. Names match
exactly, or by prefix with a final `*`. For example, a network client can forbid
`bevy` and `bevy_*`. An empty list explicitly permits every dependency.

Every example must have an entry, and every entry must name an example workspace
member. [Status ping](status_ping/) queries a server without Bevy.
[Inventory save viewer](inventory_save_viewer/) reads a player file without Bevy.
[Player attributes](player_attributes/) prints and edits the attributes of a player file
without Bevy or the engine.
[Inventory minigame](inventory_minigame/) runs inventory plugins without the packet
edge of the inventory (`mcrs_minecraft_inventory_server`) and without the click and
client view crates. The engine still links the network crates today.
[Loot validate](loot_validate/) checks the loot tables of a data pack without the engine.
[Worldgen preview](worldgen_preview/) draws the terrain of a data pack as PNG maps without
Bevy, the engine or a server.

Run `cargo nextest run -p xtask` to check the policies. The `portability` trigger
also checks them in CI. The check reads workspace membership through Cargo
metadata and resolves each example's normal dependency tree with its own default
features, across all targets. Build and development dependencies are excluded.
A failure reports the example and the dependency path to the forbidden crate.

Put behavior tests in `tests/it/main.rs` and its modules. Share expensive fixtures,
such as an embedded server, through `LazyLock`. Server dependencies belong in
`dev-dependencies` so the executable keeps its declared library boundary.
