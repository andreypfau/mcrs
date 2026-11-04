# Cells, the Muxer and the Data Layer

This document describes how the engine divides a world between owners, and how a player sees and
simulates that world. It extends `minecraft-bevy-sharding-architecture.md`: shards and kernels
stay as that document defines them. This document adds where shards run.

The engine does not know Minecraft. Minecraft is one game that uses it (section 9).

Every part of this document is optional. A user of mcrs can build a simple server on one thread
with none of it (section 10).

The rule for every part: make the simplest version now, behind a boundary of messages. A part
that sits behind messages can move to another thread, process or machine without a change to
the parts around it.

---

# 1. Terms

```text
World
    One 3D space with its own cell tree. A Minecraft dimension is one world.

Chunk
    The unit of world data: a fixed 3D box of voxels. It has a key and a version.
    In Minecraft, a chunk is a section (16 x 16 x 16).

Cell
    The owner of a 3D box of hot space. Only the owner changes the chunks in its box.
    A cell ticks its box and runs the shards of the domains inside it.

Cell tree
    An octree over the chunk keys of one world. Its leaves are the cells.

Hot space
    Space that ticks. A ticket makes space hot.

Cold space
    Space that does not tick. Its chunks are data in the data layer, with no owner.

Ticket
    A request to keep a box of space hot: a player's simulation area, the spawn area,
    a chunk loader, an ender pearl.

Stage
    A step that turns the data of a chunk into the data of its next level, off the tick.
    World generation and lighting are stages.

Link
    The connection between two parts. Messages go in, messages come out.
    The placement of the far end decides where it runs.

Placement
    Where the far end of a link runs: Inline, Thread, Remote.

Muxer
    The gateway for clients. It owns the connections and decides what each client gets.

Data layer
    The versioned chunks of cold space, and their store on disk.

Coordinator
    The owner of global state and of the cell tree.
```

A cell is a unit of placement. A shard is a unit of causality. A cell holds many shards. This
is section 22 of the sharding document: shard, thread and server stay independent.

---

# 2. Layers

```text
Client ──► Muxer ─── view interest: chunk versions ───► Data layer (cold)
             │                                           ▲      ▲
             │ simulation interest: tickets              │      │ stage results
             ▼                                           │      │
          Cells (hot, cell tree) ─── chunk changes ──────┘      │
             ▲                                                  │
          Coordinator: cell tree, global state                  │
                                                                │
          Stages: generation, light ────────────────────────────┘
```

Every arrow is a link. No part reads or writes the state of another part.

---

# 3. The cell tree

The tree is an octree over chunk keys. Each axis has a minimum leaf size. A leaf never gets
smaller than that size on that axis.

A world with a bounded height is a configuration, not another design. In Minecraft the
minimum leaf height is the world height, so the tree never splits on Y. Sky light then never
crosses a vertical cell boundary. A world without a height limit uses the same code with a
smaller minimum on Y.

The tree covers hot space only. Cold space has no cell.

Space becomes hot when a ticket covers it. The cell then takes the chunks from the data layer.
Space becomes cold when no ticket covers it. The cell then writes the last version of each
chunk to the data layer and lets it go. This is the load and unload of chunks.

Tickets make space hot, not players. A stasis chamber with an ender pearl keeps its area
ticking with no player near, as in vanilla.

## Split and merge

The Coordinator splits a cell when its tick costs too much, and merges cells when they cost
little. The cost counts the tick time, the active chunks and the entities. It also counts the
coupling across the boundary, so that a split does not cut a redstone mechanism (sharding
document, section 14).

A move happens at a tick boundary. The old owner serializes the chunks and the entities. The
new owner takes them in at the start of its next tick. Only stable IDs cross the boundary:
the session of a player and the global ID of an entity.

## Static division

Before split and merge, the tree is static. A world has a fixed number of cells, one by
default. A world of more cells divides its chunks into boxes of the minimum leaf size. In
Minecraft a box is a region: 32 x 32 columns, and the whole height of the world. The boxes
repeat in a grid of the cells, so two neighbour boxes have different owners when the count
allows it. A cell owns every box of its place in the grid.

Each cell is a whole world of its own: the same domains, stages and data layer. It owns the
columns of its boxes. It also holds the halo: the columns of the other cells within the halo
width of its boxes. A cell never changes a column of its halo.

A cell raises a ticket only on a column within the halo width of its boxes. The halo width is
a world parameter. Its default is the loading rings of a ticket (2 columns). With this width,
an owned column loads, ticks and lights exactly as in a world of one cell, and every
neighbour that its light reads is loaded.

A player belongs to the cell that owns the column it stands in. When a player stands in a
column of another cell at the end of a tick, it moves to that cell. Its session, its wire id,
its view and its replication state go with it.

---

# 4. The tick

All cells count the same logical tick `T`.

A cell receives messages from other cells at the start of a tick. A message that a cell sends
during tick `T` applies at tick `T + 1`, in a stable order by key. A cross-cell effect is
therefore always one tick late. The ordering policy of each domain states this delay.

The start of a tick is the sync point (`DomainInput`). It applies, in this order:

1. the messages from other cells;
2. the stage results;
3. the input of the players.

Each group applies in a stable order by key.

## Synchronization

Simplest now: lockstep. All cells tick `T`, then all cells tick `T + 1`. The dimensions tick
this way today.

Later: synchronization with neighbours only. A cell starts `T + 1` when it has the messages of
its neighbours for `T`. Far cells do not wait for each other.

Both modes give the same world, because a message from `T` always applies at `T + 1`. The
change between them is local to the driver of the tree.

## Halo

A cell holds a read-only copy of the chunks of its neighbours along the boundary. The width
is a parameter of the world, in chunks on each axis. The game sets a default, and the operator
can change it. Collision, box queries, explosions, pistons and the edge of the light read the
halo. The neighbour updates the halo with messages at the sync point.

A write into a chunk of another cell is a message to its owner.

## Rules between cells

Each domain has one rule across a cell boundary: a halo read, a message to the owner, or a
limit. A message that a cell sends during tick `T` applies at tick `T + 1`. At the sync point,
a cell applies the tickets first, then the halo changes, then the writes, then the players
that arrive. Each kind keeps the order of the cells that sent it.

| Domain | Rule | Delay |
|---|---|---|
| Player that walks across | a move at the end of the tick; the new owner spawns it with its session, its wire id, its view, its replication state and its hot ticket. The client gets no packet of the move, and the viewers see no despawn | none: the old owner publishes the tick of the move, the new owner the next tick |
| Hot ticket of a player | the owner sends each change to the other cells. Each cell keeps a remote ticket and raises it on its own columns and on its halo | one tick for the cells that do not own the player |
| Block change in an owned column | the owner applies it, and sends it to the other cells for their halo | one tick for the halo |
| Block write into a column of another cell | a message to the owner, which applies it with all its flags | one tick, and one more for the halo of the writer |
| Light at the edge | each cell lights its columns from its halo | the delay of the halo change |
| Collision, box queries | read the halo. The movement of the server takes the position of the client and does no collision yet | none |
| View: column versions and changes | the coordinator sends each column request to the owner of the column. The owner publishes the version and the changes | none |
| Entity tiles | each cell publishes the tiles of its players. The coordinator merges the deltas of the cells of a world into one delta for each tile | none |
| Commands and selectors | a command runs in the cell of its player. The players of the other cells of the world are players of the same dimension, and an effect on one of them is an order to its cell | one tick for an order |
| Chat, player list, world clocks | the host serves the whole world, as for one cell | none |
| Entity ids | the coordinator grants a range to each cell | none |
| Neighbour and shape updates, redstone, fluids, scheduled ticks, pistons, explosions | not supported across cells yet. The server does not run them in a dimension yet. The simulation kernels mark a section of another cell as `Foreign` and send a write to it as a message | — |
| Ticking of the sections of a remote player | not supported across cells yet: a cell marks the sections of its own players only, and only those it owns | — |

---

# 5. Two interests of a player

A player has two distances. They need different things.

| | Simulation distance | View distance |
|---|---|---|
| What it means | the space that ticks for the player | the space the client draws |
| What it needs | an owner, a tick, domains | the data of chunks only |
| Who serves it | the cells, through a ticket | the Muxer, from chunk versions |
| Can be far | no, it stays near the player | yes, any cell or cold space |

The simulation area of a player is a ticket. The Muxer sends it to the cells that own the area,
or to the Coordinator when the area is cold.

The view of a player is a subscription to chunk versions. It needs no cell.

---

# 6. The Muxer

The Muxer owns the client connections. For a client, the Muxer is the whole server.

The Muxer decides what each client gets:

- which chunks, and in which order;
- the priority by distance;
- the bandwidth budget of each client;
- the merge of many sources into one connection.

The Muxer never decides what a chunk contains. A cell publishes each change of a hot chunk with
the tick that made it. The data layer gives the version of a cold chunk. The Muxer subscribes to
the version stream of each chunk in a view, whoever owns the chunk. It merges a snapshot and the
changes after it by version, so a snapshot taken during a change is never mixed with it.

The Muxer encodes a version of a chunk once, and sends the same bytes to every subscriber.

In Minecraft, a version stream holds a column. The Muxer asks the owner of a column for a version
with `Want`, and lets the stream go with `Drop`. The owner encodes the version once, as the packet
of the column, and publishes it in the output of a tick. The version holds the changes of the ticks before that tick. The Muxer applies each
change of that tick and of the later ticks after the version. A change that the version holds
already sets the same blocks or light again, so it does no harm. While clients wait for a
version, the Muxer keeps the changes after it, and a client gets the version and then those
changes. When no client waits, the Muxer lets the version go, and a later client asks again.
The Muxer sends the same bytes of a version to every client that takes it.

The encode stays with the owner while the Muxer runs on the server thread of the host. With 300
bots apart, the encode on the host cost 3.5 ms between ticks and took only 0.4 ms off the tick.

The owner of a player reports where the player stands and how far it sees. The Muxer makes the
view from that report: it asks for the columns of the view nearest first, sends them at the rate
that the client answers with, and sends the center and the radius of the view and the forget of
each column that leaves it.

The Muxer hides moves between cells. An entity keeps one network ID in the whole world, so a
client sees no despawn and spawn when the entity changes its owner.

The Muxer sends the input of a player to the cell that owns the player.

In a cold chunk, entities are visible but do not move, as in vanilla. A cold snapshot holds its
entities. A cell publishes its entities within their tracking range.

`replication.md` describes how the Muxer sends entities to many viewers at a bounded cost.

---

# 7. The data layer

The data layer holds the versioned chunks of cold space. It reads and writes the store on disk.

Simplest now: an in-memory cache in the process of the Muxer, over the disk store.

Later: a service of its own, scaled apart from the Muxer.

The Muxer, the cells and the stages reach the data layer only through a link, by chunk key and
version. The move to a service changes only the placement of that link.

`mcrs_engine_store` is the data layer. A `StoreStage` holds a `ChunkCache` over a `DiskStore`
that the game supplies. It takes three inputs:

- `Put`: keep this version of the key. The cache keeps a later version that it already holds.
- `Get`: answer with the last version of the key, or with `Missing`.
- `Flush`: write to the disk each version that the disk does not hold yet.

The stage answers in the order of the inputs, so a `Get` after a `Put` sees that `Put`.

Minecraft keys the data layer by column, because a client receives whole columns. One column
version holds the blocks, the biomes and the light of each section, the block entities and the
heightmaps. Each dimension world has a link of its own. The world is the only writer of its
columns, so it numbers the versions it writes.

The rules of a dimension world with a data layer:

- A column is hot when a hot ticket covers it. A player holds one hot ticket: the simulation
  area one ring wider, as far as the view reaches. A hot ticket loads its columns and the two
  rings around them.
- A column that no ticket loads goes cold. Before its sections go, the world writes its last
  version. It does not write a column that came from the data layer and did not change. It does
  not write a generated column whose light did not settle, because that light at the edge of the
  loaded space was lit without the neighbours.
- A section that a ticket loads asks the data layer first. A section that the data layer does
  not hold goes on to generation. The section takes the blocks of the answer, not its light. The
  dimension lights the section again.
- The dimension publishes a version of a column from its sections when it holds the column and
  the eight columns around it. The dimension then lights the column from the blocks of its
  neighbours. The data layer makes each other column that the Muxer wants, also a column of the
  outer ring of a hot area. A view holds no ticket, so a view loads no column into the dimension.
- The data layer makes the final version of each column that the Muxer wants. It takes the
  blocks of the column and of the eight columns around it from the dimension when the dimension
  holds the column. Else it takes them from its cache when the world wrote the column. Else
  generation makes them. Then the light stage of cold space lights the column. The dimension
  encodes the final column once and publishes it for all clients that wait on it.
- A light level reaches 15 blocks, so the blocks of a column and of the eight columns around it
  decide the light of the column. The light stage of cold space holds the latest blocks of each
  column near a view. A block change in a column that it holds goes to it too. The data layer
  takes the light of a column when the light of the column and of the eight columns around it
  has settled. Thus the light of a cold column is final and has no seam.
- The data layer keeps a generated column at version zero. It keeps the new light of a column
  that the world wrote at the version of that write. A later version that the world writes
  replaces both.
- The data layer makes a column again when the world wrote the column, when the dimension
  loaded the column, or when a block of the column changed since the data layer took it from
  the dimension. A change after the publish goes to the Muxer as a change of the column.
- An answer is older than a `Put` that the world sent after the `Get`. The world then asks
  again and drops the older answer. A column that the dimension lights before the data layer
  finished it is published from the dimension. A client thus never gets a version older than
  the hot one.
- The Muxer subscribes to a column by its position, so the changes of a column that comes into
  the dimension again reach the clients that hold it. The dimension sends no light change of a
  column of the outer ring, because it lights that column without its outer neighbours.
- The dimension does not read what the stages of cold space make. Thus the simulation does not
  depend on when a cold column is done.

The disk store of Minecraft is `NoDisk` for now: the data layer lives in memory, and a restart
loses the cold columns that the save on disk does not hold.

---

# 8. Stages

A chunk climbs a ladder of levels. In Minecraft: requested, generated, lit, active. An active
chunk is hot or can become hot, and the client can draw it.

A stage takes the data of a chunk, and of the neighbours it needs, and gives the data of the
next level. It runs off the tick, with the placement of its link.

The same stage code serves cold and hot space:

- In cold space, the data layer runs the stages. After generation, the light stage lights the
  new chunk from its blocks and the blocks of its neighbours. A cold chunk does not change
  after that. In Minecraft, the data layer of a dimension has a generation stage and a light
  stage of its own. They run the same code as the stages of the dimension, and own no section
  of the dimension.
- In hot space, the owner runs the light stage after each change of blocks. It sends the
  changes and the halo it needs. The result applies at the start of a later tick.

A stage is a kernel in the sense of the sharding document: plain Rust, with no Bevy and no
thread of its own. The link gives it a thread. A stage that keeps state between steps, such as
the light of the chunks it has seen, keeps it inside its kernel.

---

# 9. The engine and the game

The engine knows worlds, chunks, cells, tickets, links, stages, the Muxer and the data layer.
It does not know blocks, light, Minecraft packets or Anvil.

Minecraft supplies what the engine leaves open:

| The engine asks for | Minecraft gives |
|---|---|
| a chunk type and its key | the section, keyed by `SectionPos` |
| the minimum leaf size on each axis | the world height on Y |
| the encoding of a chunk version and of a change | the chunk and block update packets |
| the unit a client receives | a column: all sections at one `ColumnPos` |
| the disk store | Anvil region files |
| the stages | world generation and lighting |
| the domains inside a cell | entities, redstone, fluids, block ticks |
| the tickets | simulation distance, spawn, chunk loaders, ender pearls |

The engine crates use the prefix `mcrs_engine_`, without `minecraft`:

```text
mcrs_engine_link     links, placements, the stable order of results
mcrs_engine_cell     the cell tree, tickets, the tick and its sync point, the halo, split and merge
mcrs_engine_muxer    sessions, view and simulation interests, version streams, encode once
mcrs_engine_store    versioned chunks, the cache and the disk store trait
```

Each crate works without Bevy. A Bevy driver is a feature, as for the kernels.

---

# 10. The simplest server

A user can drop cells, the Muxer, the Coordinator and the data layer, and write a server on one
thread. This is a requirement on the layering, not an extra mode:

- The game logic depends on kernels and on the world view trait (sharding document, section 13).
  It never depends on cells, links, the Muxer or the data layer.
- A stage is a plain kernel. A simple server calls it directly, inside its tick.
- The protocol, the network and the game data (layers 1 to 3) work without the engine crates.

The simple server is then one world, one loop and the connections:

```text
loop:
    read the packets of the connections
    tick the world: the domains, then the stages inline
    send each client the chunks and the changes in its view
```

Between the two ends there is a middle: the full engine with one cell for each world and every
link `Inline`. It runs on one thread, it is deterministic, and it is the exact-parity mode of the
sharding document (decision 20). It is also the mode of the tests.

---

# 11. Where the code is today

| This document | Today |
|---|---|
| Cell | one dimension sub-app in `mcrs_engine_cell::Cells` (`DimCells`). A world has one cell by default, or a static division by region over the count of the server's `cells` field (`WorldCells`, `mcrs_engine_cell::BoxPartition`). Each cell owns the columns of its regions (`OwnedColumns`) and holds a halo of the columns around them. The coordinator splits the input of a world by the owner: a player to the cell of the column it arrives in, its packets to the cell that holds it, a column request to the owner of the column. It merges the outputs of the cells of a world into one output for the muxer, and hands the messages between the cells (`cell_mail`: tickets, halo changes, writes, players that cross) to their next tick. The cells of all worlds tick in lockstep after the host's `Last`, placed `Inline` or on a `Thread` by the server's `worlds` field. It loads only the columns of its hot tickets and the two rings around them. It reports the view of each player, publishes a version of each column that the muxer wants, and publishes each change of the blocks and the light of a column that the muxer subscribes to (`column_streams`, `block_update`, `emit_light_updates`). Without a data layer, a wanted column loads with a ticket until the dimension publishes it |
| Muxer | for entities, `mcrs_engine_muxer::MuxerStage` on a link from the host, placed by the server's `muxer` field (`Thread` by default): tiles, subscriptions, delta bases, spawn bundles and the player list, with the Minecraft codec of `mcrs_minecraft_replication`; for the rest, the `muxer` module tree of `mcrs_minecraft_engine` in the host world: connections and the bridge, sessions and their placement, the input it routes to the owner dimension, the orders and the command handoffs it routes to the dimension of a player or of a handoff, the host side of entity replication, and the view of each player (`muxer::column_views`): the columns of the view and their order by distance, the batches and their pace by the answer of the client, the center, the radius and the forgets, and the version stream of each column, whose bytes the muxer sends to every client that takes the version, with the changes after it |
| Coordinator | the `coordinator` module tree of `mcrs_minecraft_engine` in the host world: the dimension worlds and their lockstep tick, the spawn and despawn queues, game time and world clocks, the operator list, the ranges of global entity ids that it grants to each dimension world, the dimension of a player that joins or changes dimension, the running dimensions and the players that each dimension reports. It and the muxer share one world but talk only through the messages of `to_muxer` and `to_coordinator` |
| Link | `mcrs_engine_link` for light and generation; the host and each dimension exchange one `DimInput` and one `DimOutput` a tick over the cell's link: the coordinator puts the input that the muxer routed into the `DimInput`, and answers a request for entity ids in a `DimOutput` with a range in the next `DimInput`, and hands the packets and the tile deltas of each `DimOutput` to the muxer, which sends the tile deltas on to the entity muxer over its link. The requests of the muxer for column versions go in the `DimInput`, and the views of the players, the column versions and the column changes come back in the `DimOutput`. A command runs inside the tick of one dimension: the muxer routes the command line to the dimension of the player, an effect on a player of another dimension leaves in the `DimOutput` as an order that the muxer routes to the dimension of the player, a stage that reads another dimension leaves as a handoff to that dimension, and a clock change leaves for the coordinator, which sends the clocks to every dimension in the next `DimInput`. Each `DimOutput` reports the players of its dimension, and the coordinator puts the players of the other dimensions in the `DimInput` of a dimension that runs commands. No part lends a world; only tests read a world between ticks |
| Data layer | `mcrs_engine_store::StoreStage` on a link from each dimension world, placed on a `Thread`, with the column store of `mcrs_minecraft_engine` (`column_store`, `cold_columns`, `stored_columns_landing`, `hot_ticket`) and no disk under it. `ColumnProduction` in the dimension world drives the stages of cold space: a second generation stage (`cold_generation` in `mcrs_minecraft_vanilla_worldgen`) and a second light stage (`cold_light` in `mcrs_minecraft_vanilla_light`), each on a `Thread`. The link to the data layer and `ColumnProduction` stay in the dimension world: the dimension publishes a cold column that the muxer wants in its `DimOutput`. Anvil is read only by the generation stages, for the columns of a save that the data layer does not hold |
| Stage | the light engine and the world generator, each as a `Stage` on a `Link` |

---

# 12. Migration

Each step leaves a working server and passes the determinism test.

1. Add `mcrs_engine_link`. Move light and generation onto links and stages. The words actor, inbox,
   handoff and delivery leave the code.
2. Split the host into the Muxer and the Coordinator, still in one process. Make the host to
   dimension channels a link.
3. Add `mcrs_engine_store` with the in-memory data layer. Serve the view from it. Keep only hot columns
   in the dimension. Done, with these parts left:
   - The disk store of Minecraft: Anvil region files that the data layer reads and writes, a
     `Flush` at each autosave and at shutdown, and an eviction of the columns that the disk
     holds. Check: a column that a player changed is the same after a restart.
   - The stages in cold space. Done: the data layer generates and lights each column that a
     view needs and that the dimension does not light, and a view loads no column into the
     dimension. Parts left:
     - The world writes a column that goes cold with the light of the dimension. A block change
       in the last tick before the column goes cold can thus be stored with light that lags.
       A column of the outer ring can also go cold with the light of an edge. The data layer
       lights such a column again before a view gets it, but the version stays in the cache
       with the old light until then.
     - A cold column keeps the light it had when the data layer made it. A later block change in
       a neighbour near the seam does not change it. The light stage of cold space computes the
       new light, but the data layer does not send it.
     - A column whose block changes in the dimension after its packet went out gets the change,
       but not the light of the change, while the column is in the outer ring.
     - A section that a ticket loads treats `Unreadable` as `Missing` and is generated again, so
       the next write replaces the version on the disk. The data layer does not keep a column
       that it made for a view after `Unreadable`.
     - The light stage of cold space publishes the light of each epoch, and the data layer drops
       it. Only the light of a settled column goes back.
   - The view in the Muxer. Done: the view of each player and the version stream of each
     column are in the muxer tree of the host, and the dimension world holds no view and no
     waiter. Parts left:
     - The view runs in the host world, on the server thread. It is not on the `MuxerStage`
       and its link yet, so the `muxer` placement does not move it to a thread. The engine
       crate does not know column streams: a stream generic over a chunk codec, as
       `ReplicationCodec` is for entities, is the next step. On a thread, the Muxer can also
       take the encode from the dimension.
     - The muxer reaches the data layer through the dimension world, which holds the link and
       `ColumnProduction`. A link of the muxer to the data layer, and a dimension that sends the
       data layer its blocks, remove that path.
     - The dimension makes the view square of a player from its position. The muxer takes the
       square as it is.
   - The scheduled ticks of a section, which a cold column does not keep yet. A column that the
     data layer generates drops the ticks that generation scheduled.
4. Let a cell own a box, not a whole world. Add the halo and the messages between cells. Divide
   statically, for example by region files. Test one cell against many cells. Done: a world
   divides by region over a fixed count of cells, and a test runs one scenario with one cell
   and with two cells (`world_cells` in the tests of `mcrs_minecraft_server`). Section 4 lists
   the rule of each domain. Parts left:
   - The data layer is a link of each cell. A cell writes only its own columns to it, but it
     reads only its own data layer. A column of the halo that its owner wrote and let go cold
     loads in a neighbour from generation, without the changes of the owner. Check: a block
     that a player changed near a boundary is in the halo of the neighbour after the column
     went cold and came back.
   - The cold production of a column at the edge of a box takes the blocks of its neighbours
     from the halo only while the cell holds them. Else it takes generated blocks. Check: the
     light of a cold column next to a changed column of another cell has no seam.
   - A cell marks the ticking sections of its own players only. A remote ticket loads columns
     but does not tick them. Check: when a system reads `BlockTicking`, a section near a
     player of another cell ticks as in a world of one cell.
   - Neighbour and shape updates, redstone, fluids, scheduled ticks, pistons and explosions
     across a boundary. They are not in the dimension yet. Check: a redstone line across a
     boundary gives the same states as in a world of one cell, a tick later at most.
   - A command handoff runs in the first cell of its world. Check: a stage that reads blocks
     of another cell reads the blocks of their owner.
   - A cell that ticks no player and holds no remote ticket still ticks. Check: the tick of a
     world of four cells with no players costs no more than that of one cell.
5. Split and merge in the Coordinator, with moves at the tick boundary.
6. `Placement::Remote`: a cell in another process.
