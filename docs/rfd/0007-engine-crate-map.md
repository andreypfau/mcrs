# RFD 7: Crate map of the engine layer

- Authors: andreypfau
- State: prediscussion
- Labels: engine

This page proposes to split the crate `mcrs_minecraft_engine` into six crates and a facade. It
gives each module of the crate one target crate, the dependency graph between the new crates,
and the order of the moves. Read it before you add a module to the engine, or before you move
one out.

The maintainer chose the six crates and the job of each one. The maintainer also chose three
related splits of other engine crates. The rest of this page is the proposal of the writer
of this page. The writer checked the choice against the code at `main` and assigned each
module. The text marks with the words "the code says" each place where the code forced a change
to the first sketch. Section "Open questions" lists what the maintainer has not decided.

## The problem

`mcrs_minecraft_engine` holds the packet bus, the dimension cells of the host, the column
pipeline, the player flow, the block simulation and the network edge. These are six
abstractions in one crate. The crate breaks four rules of `CLAUDE.md` and of `rust-style`.

- **A crate is one abstraction.** A bot or a world viewer that needs only the column store
  must also take the player flow, the block simulation and the edge.
- **Flat `src/`.** `src/` holds 104 entries, and `src/muxer/` holds 23 more. The crate has
  two folders with submodules, `coordinator/` and `muxer/`.
- **No module cycle across abstractions.** One cycle of 34 modules runs through the column
  pipeline, the player flow, the block simulation and the host.
- **One public path per item.** Some modules are `pub`, and `lib.rs` also re-exports items of
  private modules at the root.

A fix inside the crate cannot remove the width. Only a split into crates removes it.

## What is known

| Fact | Source |
|---|---|
| 138 `.rs` files, 22,818 lines | `find crates/mcrs_minecraft_engine/src -name '*.rs' \| xargs wc -l` |
| 104 entries in `src/`, 23 in `src/muxer/`, 13 in `src/coordinator/` | `ls` of each folder |
| Module references | `crate::<module>` and `super::<module>` paths of each file, without `#[cfg(test)] mod tests` |
| One strongly connected set of 34 modules, and one of 5 modules in `muxer/` | Tarjan's algorithm over those references |
| 24 other crates import `mcrs_minecraft_engine::…` | `grep -rl 'mcrs_minecraft_engine::' crates` |
| The client imports `ability::PlayerOpLevel` and `coordinator::ops::{OpList, OpListEntry}` | `crates/mcrs_minecraft_client/src/local_server.rs`, lines 16 and 17 |
| The feature `world-access` gates items in `column_streams`, `coordinator/dim_cells`, `muxer` and `lib.rs` (the re-export of `CellVolume`) | `grep -rn 'cfg(feature' crates/mcrs_minecraft_engine/src` |
| The feature `telemetry-tracy` gates items in `drain_player_despawn`, `publish_player_tiles` and `muxer/bridge` | the same command |

The module graph is an import-level graph. It sees a path that starts with a module name. It
does not see a root re-export such as `crate::SavedSpawn`. A grep for those finds one, and it
points down. Each extraction checks the graph again with `cargo check` on the new crate.

A run with the test modules included adds no reference that crosses two target crates in the wrong
direction.

## Determination

The maintainer decided the following.

1. The engine becomes six crates and a facade, with these jobs:
   - `mcrs_minecraft_engine_shared`: the shared state and its edits (`Shared` and `Edit`), the
     time, the table of dimensions and the bus messages. It is the vocabulary of the others.
   - `mcrs_minecraft_engine_host`: the dimension cells, the roster, the join placement, the world
     state and the autosave.
   - `mcrs_minecraft_engine_edge`: the network edge, today `src/muxer/`.
   - `mcrs_minecraft_engine_columns`: the column store, the cold and stored columns, the light
     codec and the heightmap.
   - `mcrs_minecraft_engine_player`: the player, its persistence, the teleport and the transfer
     between dimensions.
   - `mcrs_minecraft_engine_block`: the block simulation, the block break and the digs.
   - `mcrs_minecraft_engine` stays as the facade plugin.
2. `mcrs_engine_muxer` splits off `mcrs_engine_tile`.
3. `mcrs_minecraft_engine_server` splits off `mcrs_minecraft_engine_login` after #1111.
4. `mcrs_minecraft_engine_world` splits into a tick crate, a world crate and a session crate
   after the GUI rewrite (#1159) merges.

## Proposal

### Layers and names

All seven crates are in layer 4, Engine. Each one uses Bevy. The lists of
`banlist`, `whitelist` and `coordinator/ops` are vanilla policy, and #1110 moves them out.

| Crate | Layer | Files | Lines | Job |
|---|---|---|---|---|
| `mcrs_minecraft_engine_shared` | 4 | about 22 | about 2,400 | Vocabulary: messages, shared values, time, dimension table |
| `mcrs_minecraft_engine_columns` | 4 | 22 | 4,449 | Column store, cold and stored columns, column streams, light codec, heightmap |
| `mcrs_minecraft_engine_player` | 4 | 36 | about 5,700 | Player spawn and despawn, persistence, movement, teleport, transfer, player tiles |
| `mcrs_minecraft_engine_block` | 4 | 11 | 2,186 | Block simulation, block updates, block break, digs |
| `mcrs_minecraft_engine_host` | 4 | about 26 | about 4,000 | Dimension cells, sub-app builder, roster, join placement, world state, autosave, host plugins of the shared lists |
| `mcrs_minecraft_engine_edge` | 4 | 24 | 3,880 | The bridge, sessions, dimension inputs and outputs, entity replication, column views |
| `mcrs_minecraft_engine` | 4 | 2 | about 160 | `EnginePlugin` and the code between two ticks |

The counts come from `wc -l` of the files that move. "About" marks a crate that also takes a
part of a file that splits (section "Cuts").

Each new crate has private modules and re-exports its public items at the root, as
`rust-style` prefers. Each new crate starts with `src/` flat. `heightmap/tests.rs` becomes the
inline test module of `heightmap.rs` or a flat file `heightmap_tests.rs`.

### Dependency graph

An arrow points from a crate to a crate it depends on.

```text
mcrs_minecraft_engine  ->  edge, host, block, player, columns, shared
edge                   ->  host, player, shared
host                   ->  block, player, columns, shared
block                  ->  player, columns, shared
player                 ->  columns, shared
columns                ->  shared
shared                 ->  mcrs_minecraft_engine_world, and crates of layers 1 to 3
```

The graph is acyclic. A topological order is shared, columns, player, block, host, edge, facade.
The code says that this order holds only after the cuts of section "Cuts". Without them, nine
module references go in the wrong direction.

The code says that the edge depends on the host. The edge reads the channel types between the
host and the dimensions (`channel_types`, `to_muxer`, `to_coordinator`). These types carry
`Departure`, `PlayerTransfer` and `OrderedPlayer` of the player crate. So they cannot go down to
the shared crate without those types. Open question 2 asks the maintainer to choose.

### What each crate holds

A module name is the file name in `crates/mcrs_minecraft_engine/src/` today.

**`mcrs_minecraft_engine_shared`** (22 modules and parts of two more):
`ability`, `background_writer`, `banlist`, `block_action_ack`, `bus`, `column_change`,
`column_request`, `column_version`, `dimension_generators`, `game_profile`, `json_list`,
`persisted_registry`, `player_view`, `replicated`, `server_edit`, `tile_events`, `whitelist`,
`coordinator/clock`, `coordinator/default_game_mode`, `coordinator/dimension_order`,
`coordinator/ops`, `coordinator/running_dimensions`. It also takes `SavedSpawn` from
`world_state` and the set `DimInboxDrain` from `dim_tick`.
The host plugins of `banlist`, `whitelist`, `coordinator/ops`, `coordinator/clock` and
`server_edit` do not stay here. They move to the host (cut 2).

**`mcrs_minecraft_engine_columns`** (21 modules): `advance_production`, `block_entity_packet`,
`cold_columns`, `cold_light_edit`, `column_asks`, `column_generation`, `column_light`,
`column_packet`, `column_production`, `column_snapshot`, `column_store`, `column_streams`,
`column_ticks`, `foreign_views`, `heightmap`, `held_light`, `hot_ticket`, `light_codec`,
`stored_column`, `stored_columns_landing`, `wanted_column`.

**`mcrs_minecraft_engine_player`** (36 modules): `carried_attributes`, `cell_crossing`,
`client_info`, `client_on_ground`, `client_view_distance`, `consume_inbound_player_spawn`,
`departure`, `despawn_inbound_player`, `dim_player_plugin`, `dimension_change`,
`drain_player_despawn`, `enter_level_packets`, `game_mode`, `loaded_client`, `max_health`,
`minecraft_entity`, `movement`, `network_id`, `op_level`, `persistence`, `player_bundle`,
`player_dimension_change`, `player_dimensions`, `player_order`, `player_signal`,
`player_sprinting`, `player_state`, `player_tracker`, `pose_systems`, `publish_player_tiles`,
`publish_pose`, `respawn_packet`, `send_own_attributes`, `shift_key`, `swing`, `teleport`.
The autosave part of `persistence` moves to the host (cut 3).

**`mcrs_minecraft_engine_block`** (11 modules): `advance_digs`, `block_break`,
`block_simulation`, `block_update`, `break_session`, `cell_volume`, `dig_rate`, `diggers`,
`random_tick_speed`, `use_item_on_request`, `world_kinds`.

**`mcrs_minecraft_engine_host`** (19 modules and the new files of cuts 2 and 3):
`assign_global_entity_id`, `cell_mail`, `channel_types`, `command_handoff`, `dim_mail`,
`dim_tick`, `held_joins`, `player_roster`, `to_coordinator`, `to_muxer`, `world_state`,
`coordinator/birth_clock`, `coordinator/coordinator_plugin`, `coordinator/dim`,
`coordinator/dim_cells`, `coordinator/entity_id_ranges`, `coordinator/join`,
`coordinator/roster`, `coordinator/sub_app_builder`. The files lose the prefix `coordinator/`.

**`mcrs_minecraft_engine_edge`** (24 modules): the 23 files of `muxer/` and `view_radius`. The
files lose the prefix `muxer/`.

**`mcrs_minecraft_engine`** (facade): `lib.rs` with `EnginePlugin`, and `between_ticks`.
`between_ticks` stays in the facade because it calls the host and the edge. `coordinator.rs`
and `muxer.rs` hold only `mod` lines, so they go away.

Each of the 138 files has one target crate. Section "Open questions" lists the assignments
that are a choice and not a consequence of the graph.

### Features

A crate that holds a gated item gets its own feature of the same name. The facade forwards its
feature to each of them.

| Feature | Crates |
|---|---|
| `world-access` | columns, block, host, edge, facade |
| `telemetry-tracy` | player, edge |

### Shared state of RFD 6

RFD 6, the boundary between the host and the dimensions, is drafted in parallel as
`docs/rfd/0006-host-dimension-boundary.md`. It adds `Shared` and `Edit`. These two types land in
`mcrs_minecraft_engine_shared` first. Either the pull request of RFD 6 or step 8 below creates
the crate, whichever merges first. No other crate holds a copy of them.

### `mcrs_minecraft_engine_world` below the shared crate

The maintainer placed the shared crate below `mcrs_minecraft_engine_world`. The code says that
this does not hold today. The shared modules use these items of `mcrs_minecraft_engine_world`:

| Item | Used by | Part of `engine_world` after its split |
|---|---|---|
| `session::PlayerSession` | `bus`, `ability`, `block_action_ack`, `column_change`, `player_view` | session |
| `entity::player::chunk_view::ChunkTrackingView` | `player_view` | session |
| `tick_pace::{TickPace, tick_runs_normally}` | `server_edit` | tick |
| `game_rules::GameRules` | `server_edit` | session |
| `world::dimension_list::DimensionList` | `dimension_generators` | world |
| `world::sub_app::AddDimensionPlugins` | `persisted_registry` | world |

So until the split of `mcrs_minecraft_engine_world`, the shared crate depends on it. After the
split, the shared crate depends on the tick crate and the session crate. It can then sit below
the world crate if `DimensionList` and `AddDimensionPlugins` move. Open question 1 asks how.

## Cuts

A cut is a refactor inside `mcrs_minecraft_engine`. It removes a reference in the wrong
direction, or it splits a file whose parts go to two crates. A cut changes no behaviour. Each
cut is one pull request and lands before the extraction that needs it.

| Cut | Reference it removes | Change | Needed by |
|---|---|---|---|
| 1 | `cell_mail`, `dim_player_plugin`, `minecraft_entity`, `player_tracker` to `dim_tick` | `DimInboxDrain` moves to a file of its own and becomes a set with ordered variants: before the drain, the drain, after the drain. The four modules order against a variant, never against the function `drain_dim_inbox`. | player |
| 2 | `server_edit` to `muxer/online_count` | The plugins `WorldClockHostPlugin`, `OpListHostPlugin`, `BanListHostPlugin`, `WhitelistHostPlugin` and `ServerStateHostPlugin` leave the files of their values. Each goes to a file named after it, for the host. The order of `announce_tick_state` after `send_online_count` moves to the edge plugin. | shared |
| 3 | none. The determination puts the autosave in the host. | `AUTOSAVE_INTERVAL`, `AutosaveCadence`, `advance_autosave_cadence`, `autosave_due` and `AutosavePlugin` move from `persistence` to `autosave`. `autosave_players` stays in `persistence`. | host |
| 4 | `consume_inbound_player_spawn` to `world_state` | `SavedSpawn` moves to a file of its own. | shared |
| 5 | `dim_player_plugin` to `block_break` | The host adds `BlockBreakPlugin` to the dimension sub-app. `DimPlayerPlugin` stops adding it. | player |
| 6 | `block_simulation` and `column_store` to `cell_mail` | The block simulation and the column store each own the type of their part of the mail. `CellMail` holds the parts. | columns, block |
| 7 | `cold_columns` to `block_simulation` | `write_cold_columns` and the two other cold systems call `take_column` and `copy_column` on `BlockSimulation`. The columns crate gets a small trait for the ticks of a column, with one empty implementation. The block crate implements it and adds the cold systems with its type as the generic parameter. | columns |

Cut 7 is the only cut that changes the shape of a system. Open question 4 asks for a review of it.

The cuts also widen visibility. A module of one target crate imports 21 private modules and
about 10 `pub(crate)` items of another target crate. Each one becomes `pub` in the extraction
that splits them, and so becomes a commitment of layer 4. The count does not include a
`pub(crate)` field that a module of another target crate reads. The cuts remove most of those
reads, and each extraction lists the rest.

## Module cycles

Today one cycle of 34 modules crosses four target crates: 13 player modules, 10 column modules,
7 host modules and 4 block modules. A second cycle of 5 modules lies in `muxer/`.

After cuts 1 to 7, the graph has no cycle that crosses two target crates. Six cycles stay, each
inside one target crate. A module cycle inside one crate compiles. The map does not remove
these, and this RFD does not claim to:

| Crate | Modules |
|---|---|
| columns | `advance_production`, `cold_columns`, `column_asks`, `column_production`, `column_snapshot`, `column_store`, `column_streams`, `stored_column`, `stored_columns_landing`, `wanted_column` |
| player | `cell_crossing`, `consume_inbound_player_spawn`, `despawn_inbound_player`, `dim_player_plugin`, `minecraft_entity`, `player_dimension_change`, `publish_player_tiles` |
| player | `persistence`, `teleport` |
| block | `advance_digs`, `block_break` |
| host | `coordinator/birth_clock`, `world_state` |
| edge | `departure_routing`, `dim_outputs`, `entity_replication`, `latency`, `muxer_link` |

Each of these is a refactor inside its new crate, after the extraction.

## Migration

Each step is one pull request. A cut changes code and no behaviour. An extraction moves files
with no edit besides `use` paths, visibility, `mod` lines and `Cargo.toml`. Each step leaves
`main` green. The step that moves the last caller of a path deletes the path.

| Step | Kind | Content | Deletes |
|---|---|---|---|
| 1 | cut | Cut 1 | the references to `drain_dim_inbox` in `cell_mail` and `dim_player_plugin` |
| 2 | cut | Cut 2 | the host plugins from the files of their values |
| 3 | cut | Cut 3 | the autosave items in `persistence.rs` |
| 4 | cut | Cut 4 | `SavedSpawn` in `world_state.rs` |
| 5 | cut | Cut 5 | `BlockBreakPlugin` in `DimPlayerPlugin` |
| 6 | cut | Cut 6 | the access of `block_simulation` and `column_store` to the fields of `CellMail` |
| 7 | cut | Cut 7 | the import of `BlockSimulation` in `cold_columns.rs` |
| 8 | extraction | Add `mcrs_minecraft_engine_shared` | the 22 shared files and the moved items from the engine crate |
| 9 | extraction | Add `mcrs_minecraft_engine_columns` | the 21 column files and `src/heightmap/` |
| 10 | extraction | Add `mcrs_minecraft_engine_player` | the 36 player files |
| 11 | extraction | Add `mcrs_minecraft_engine_block` | the 11 block files |
| 12 | extraction | Add `mcrs_minecraft_engine_host` | the host files, `src/coordinator/` and `coordinator.rs` |
| 13 | extraction | Add `mcrs_minecraft_engine_edge` | `src/muxer/`, `muxer.rs` and `view_radius.rs` |
| 14 | cleanup | Regenerate `docs/crates.md` with `cargo run -p xtask -- crate-map` | the old description of `mcrs_minecraft_engine` |

Steps 1 to 7 can land in any order, and all of them land before step 8. Each extraction needs
the extractions before it in the table, because each new crate depends only on crates that
exist.

Each extraction changes the `use` lines of every caller in the same pull request. No re-export
stays in the facade, because a re-export is a second path to the same item. Step 8 changes two
lines in `mcrs_minecraft_client/src/local_server.rs`, so it needs a run of `run-client` with
the feature `singleplayer`. An extraction crosses crate groups through these `use` lines.
Open question 3 asks for that waiver.

Steps 9, 10, 12 and 13 move more than 3,000 lines. Open question 5 asks how a move counts
against the limit.

### Where the map does not fix the width

The player crate has 36 files. That is wider than the rest, and it holds two jobs: the flow
of a player and the replication of a player. The replication modules are `publish_player_tiles`,
`publish_pose`, `player_signal`, `player_tracker`, `send_own_attributes`, `swing` and
`despawn_inbound_player`. One of them is in a cycle with the flow modules. Open question 6 asks
whether a seventh crate takes them.

## Related splits

These splits are part of the determination. Each one is its own pull request and touches one
crate group.

- **`mcrs_engine_tile`** from `mcrs_engine_muxer`. It takes the value types and the tile
  arithmetic: `tile_blob`, `tile_delta`, `tile_grid`, `tile_radius`, `tile_reach`,
  `tile_stream`, `tier`, `view`, `entity_event`, `entity_id`. `mcrs_engine_muxer` keeps
  `muxer`, `muxers`, `viewer`, `planner`, `subscription`, `chunk_stream`, `replication_codec`
  and `viewer_bytes`. Both crates are in layer 4 and use no Bevy. This split can land at any
  time.
- **`mcrs_minecraft_engine_login`** from `mcrs_minecraft_engine_server`, after #1111. It takes
  `login`, `configuration`, `keep_alive`, `status`, `shutdown`, `loopback_ops` and
  `server_network_plugin`. #1111 can move the listener, so the list is final only after it.
- **`mcrs_minecraft_engine_world`** into three crates, after #1159 merges:
  - a tick crate: `tick`, `tick_pace`, `server_loop`, `birth_clock`;
  - a world crate: `world/`, `owned_columns`, `world_cells`, `world_placement`, `world_seed`,
    `palette`, flat;
  - a session crate: `session/`, `aoi/`, `entity/`, `entity_ids`, `entity_births`,
    `block_drops`, `block_update`, `voxel_update`, `game_rules`.

  The client and the GUI import paths of `mcrs_minecraft_engine_world`, so this split waits for
  the GUI rewrite. The order of the three crates is not checked. `world/lifecycle` reads
  `Player` and `Transform` of the session part, so the world crate depends on the session crate.

## Options

- **Keep one crate and flatten it.** It removes the folders, but `src/` keeps more than 130
  entries and the cycle of 34 modules stays. Turned down: only a split removes the width.
- **The edge beside the host.** The channel types and `Departure`, `PlayerTransfer` and
  `OrderedPlayer` move to the shared crate. The edge then depends on the shared crate and the
  player crate, and not on the host. It is cleaner, but `Departure` uses `Transform`, `Player`
  and the teleport. Open question 2.
- **Re-exports in the facade.** Each extraction keeps the old paths with `pub use`, and a last
  step deletes them. It keeps each pull request inside one crate group. Turned down for now: it
  opens a second path for each moved item, for the length of the migration.
- **Fewer crates.** For example, columns and block in one crate. `rust-style` says to split a
  crate only when a dependency forces it. Turned down by the maintainer's choice of six crates.

## Open questions

1. **The shared crate and `mcrs_minecraft_engine_world`.** Should `PlayerSession`,
   `ChunkTrackingView`, `DimensionList` and `AddDimensionPlugins` move down so that the shared
   crate sits below the world crate? Recommendation: the dimension table of the shared crate
   takes `DimensionList` with #154. The rest waits for the split of `mcrs_minecraft_engine_world`.
2. **The edge on the host, or the edge beside the host.** Recommendation: the edge on the host,
   as proposed. The other option moves player types into the vocabulary.
3. **Crate groups.** May an extraction change the `use` lines of callers in other crate groups?
   Recommendation: yes, for `use` lines only, as RFD 5 allowed for its step 2.
4. **Cut 7.** Is a generic cold system over a small trait the right seam between the columns and
   the block simulation? The other way is a message from the block crate per cooling column.
5. **The 3,000-line limit for a move.** Recommendation: count the lines that `git diff -M` shows
   as changed. A rename with only `use` edits then counts as small.
6. **The width of the player crate.** Should the replication of a player be a seventh crate?
7. **Assignments that are a choice.**
   - `banlist`, `whitelist`, `json_list` and the values of `coordinator/ops`: they go to the
     shared crate because the player crate reads the op list. #1110 moves the concrete lists to
     a vanilla crate later.
   - `background_writer`: a file writer on the task pool, used by the player crate and the
     host. It can be a generic crate of its own.
   - `hot_ticket`: the tickets of a column across cells. The player crate reads it, so it is in
     the columns crate and not in the host.
   - `persisted_registry`: the registry of saved forms. The bus carries `PersistedSnapshot`, so
     it is in the shared crate.
   - `command_handoff`: only the host reads it, so it is in the host.
   - `dimension_generators`: no module of the engine reads it. It is in the shared crate as a
     part of the dimension table.

## What would reopen this

- An extraction finds a reference in the wrong direction that this graph does not show, and no cut
  removes it.
- A user of the engine needs one of the crates without a crate that this graph puts below it.
- The compile time of the engine crates grows after the split. Measure it with
  `cargo build --timings` before step 8 and after step 13.

## Objections

The review pass has not run yet.

## References

- RFD 1: Requests for Discussion.
- RFD 5: Split the client GUI into crates by screen. The migration table follows its shape.
- RFD 6: The boundary between the host and the dimensions.
- `CLAUDE.md`, sections "Layers" and "Rust style and comments".
- The style skill `rust-style`, section "Crates and modules".
- #566, #1110, #1111, #1115, #154, #1159.
