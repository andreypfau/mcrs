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

The Muxer hides moves between cells. An entity keeps one network ID in the whole world, so a
client sees no despawn and spawn when the entity changes its owner.

The Muxer sends the input of a player to the cell that owns the player.

In a cold chunk, entities are visible but do not move, as in vanilla. A cold snapshot holds its
entities. A cell publishes its entities within their tracking range.

---

# 7. The data layer

The data layer holds the versioned chunks of cold space. It reads and writes the store on disk.

Simplest now: an in-memory cache in the process of the Muxer, over the disk store.

Later: a service of its own, scaled apart from the Muxer.

The Muxer, the cells and the stages reach the data layer only through a link, by chunk key and
version. The move to a service changes only the placement of that link.

---

# 8. Stages

A chunk climbs a ladder of levels. In Minecraft: requested, generated, lit, active. An active
chunk is hot or can become hot, and the client can draw it.

A stage takes the data of a chunk, and of the neighbours it needs, and gives the data of the
next level. It runs off the tick, with the placement of its link.

The same stage code serves cold and hot space:

- In cold space, the data layer runs the stages. After generation, the light stage lights the
  new chunk from its blocks and the blocks of its neighbours. A cold chunk does not change
  after that.
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
| Cell | one dimension sub-app: a tree with one leaf for each world |
| Muxer and Coordinator | the host world: bridge, sessions, column waiters, game time |
| Link | `mcrs_engine_link` for light and generation; `ToDim` and `FromDim`, each with its own channels |
| Data layer | none apart: cold columns live in the dimension until they unload |
| Stage | the light engine and the world generator, each as a `Stage` on a `Link` |

---

# 12. Migration

Each step leaves a working server and passes the determinism test.

1. Add `mcrs_engine_link`. Move light and generation onto links and stages. The words actor, inbox,
   handoff and delivery leave the code.
2. Split the host into the Muxer and the Coordinator, still in one process. Make the host to
   dimension channels a link.
3. Add `mcrs_engine_store` with the in-memory data layer. Serve the view from it. Keep only hot columns
   in the dimension.
4. Let a cell own a box, not a whole world. Add the halo and the messages between cells. Divide
   statically, for example by region files. Test one cell against many cells.
5. Split and merge in the Coordinator, with moves at the tick boundary.
6. `Placement::Remote`: a cell in another process.
