# RFD 6: The host and dimension boundary: shared state, edits, entity parcels, tickets

- Authors: andreypfau
- State: prediscussion
- Labels: engine

This page proposes one boundary between the host and the worlds that tick the dimensions. It
covers four things that cross that boundary: server-wide data, an edit to that data, an entity
and a ticket. Read it if you add server-wide state, move an entity between dimensions, or keep
chunks loaded.

The maintainer decided the direction of each part. Section "Determination" lists those
decisions. The agent that wrote this page checked them against the code and worked out the
details. The text marks each detail that is the agent's proposal. The state is prediscussion,
so the text can still change.

This RFD supersedes two earlier texts. Issue #154 asks for a shared snapshot behind an `Arc`.
Issue #1110 asks, as its step 0, for a design note under `docs/`. This page is that design
note, and it replaces the `Arc` snapshot with a copy by version, as #566 requires.

## Terms

- **Host**: the main app. It owns the server-wide data, the sessions and the network.
- **Cell**: one world that ticks a dimension, or a part of a dimension. `docs/cells.md`
  describes it. A cell can run on a thread of its own.
- **Boundary**: the input that the host moves into a cell, and the output that the cell
  returns. Nothing else crosses.
- **Datum**: one server-wide value, such as the world clocks, the game rules or a scoreboard.
- **Edit**: a request from a cell to change a datum.
- **Parcel**: an entity in the form that crosses a boundary.
- **Ticket**: a request to keep an area of chunks loaded, simulated, or both.

## The problem

A consumer of the engine cannot add a datum. The input and the output of a cell have one
crate-private field for each datum. The control enum of a cell is closed. The mail of edits has
one vector for each edit kind. A scoreboard, a command storage or the state of a minigame has
no way in.

Only a player crosses a dimension. The transfer type carries a player snapshot and nothing
else. A mob, an item, a vehicle with its riders and a leashed animal cannot cross. No type
names an entity of another cell, so a reference cannot say "not loaded".

Four ticket types exist, and each serves one holder. None has an owner outside its cell, an
expiry, a flag for persistence or a level that only loads. The host cannot ask a dimension to
prepare a destination before a mover leaves.

The vocabulary of the boundary sits in the wrong place. The root modules of
`mcrs_minecraft_engine` import the coordinator module for the game time, the operator list and
the default game mode. The coordinator imports those modules back, so the crate has a module
cycle.

## What is known

Each statement below is a read of the code at `origin/main`, commit `c7bdbad0b`.

- `DimInput` and `DimOutput` in `crates/mcrs_minecraft_engine/src/dim_tick.rs` hold one
  `pub(crate)` field for each datum. The private struct `Globals` in `coordinator/dim_cells.rs`
  repeats those fields on the host side.
- `dim_mail.rs` has five functions `edit_clock`, `edit_whitelist`, `edit_bans`, `edit_ops` and
  `edit_server`. Each applies the edit in the cell first and then pushes it to `DimMail`.
- `ToDim` in `channel_types.rs` has five variants. `Transfer` carries a `PlayerTransfer`.
- `Versioned<T>` in `mcrs_minecraft_core` holds `Arc<T>` and a version.
- `SubAppWorld<I, O>` and `Cells<W>` in `mcrs_engine_cell` are generic already.
- `DimCells` is a resource without a type parameter. Its field `worlds` holds the cells.
- `DimensionList` holds `Arc<[ResourceKeyBuf<Dimension>]>` and a parallel vector of types.
  `RunningDimensions` holds the same set in a second form. The module `dimension_order` ranks
  a key for the dimension order.
- The ticket types are `SimulationTicket` and `HeldSimulationArea` in
  `mcrs_minecraft_engine_world`, and `HotTicket` and `RemoteTicket` in `mcrs_minecraft_engine`.
- `StoredColumn` in `mcrs_minecraft_region_disk` has no entity part.
- `docs/spec/dimension-change.md` states the dimension change of a player and of an entity
  (DIM-CHG-001 to DIM-CHG-009). `docs/spec/argument-entity.md` states the dimension order
  (ARG-ENT-019). DIM-CHG-009 states that each passenger of a mover gets its own teleport.
- `docs/spec/command-forceload.md` and `docs/spec/block-simulation.md` name a ticket. No
  specification has tickets, the passenger tree or the leash as its subject.
- `Versioned<T>` has two users outside tests: the registry snapshot and the boundary.

Two things are not known. No measurement exists for the cost of a copy of a datum into each
cell. No search has shown whether a caller relies on an edit that applies in the same tick.

## Determination

The maintainer decided these points. They are the proposal of this page. They become the
determination when the state is published.

1. A datum reaches a cell as `Shared<T>`: a value and a version. The host copies the value
   into a cell only when the version changes.
2. The input and the output are `DimInput<X>` and `DimOutput<X>` with `X: SharedSet`. A
   `SharedSet` is a tuple of `Shared<T>`. The application composes it.
3. An edit comes back as an `Edit<T>` row in the output. One writer in the host applies it.
4. `DimCells` keeps no type parameter. Its field `worlds` moves into a new resource
   `DimWorlds<X>`. `X` has no default type.
5. One `EntityParcel` carries any entity across a boundary. A player is a parcel and a
   session part. `RemovalCause` replaces the cause that only a player has today.
6. One `Ticket` replaces the four ticket types. A request for a ticket and its answer cross
   the boundary.
7. `GameTime`, the birth clock, `TickPace` and one `Dimensions` table live in a vocabulary
   crate below `mcrs_minecraft_engine_world`. The default game mode and the operator level
   become configuration of the vanilla player crate.

   The code says that the vocabulary cannot sit below `mcrs_minecraft_engine_world`. RFD 7,
   determination 5, gives the one map of the shared crates. `Shared`, `Edit` and `SharedSet`
   sit below `mcrs_minecraft_engine_world`, in `mcrs_engine_shared`. `GameTime` and `Dimensions`
   sit above it, in `mcrs_minecraft_engine_shared`. The birth clock and `TickPace` stay in
   `mcrs_minecraft_engine_world`, because it reads them itself.
8. Three wants stay out of scope, and each keeps an extension point. The first is a dimension
   that opens or closes at run time. The second is an override of a rule or of the weather for
   one dimension. The third is a leash across dimensions.
9. A specification for tickets, entity transfer, passengers and the leash comes before the
   code of each.

## Proposal

### Crates and layers

The crate names and their content are the agent's proposal. A later RFD gives the full crate
map of the engine. This page names only the crates that the boundary needs.

| Crate | Layer | Content | Bevy | `no_std` |
|---|---|---|---|---|
| `mcrs_engine_shared` | 4 | `Shared<T>`, `Edit<T>`, `SharedSet`, the datum trait | `bevy_ecs` only | yes, with `alloc` |
| `mcrs_minecraft_engine_shared` | 4 | `GameTime`, `Dimensions`, `EntityParcel`, `EntityRef`, `RemovalCause`, `Ticket` and its request and answer, and the vocabulary of RFD 7 | yes | no |
| `mcrs_minecraft_vanilla_player` | 5 | The whitelist, the ban list, the operator list, the login gate, the default game mode, the default operator level | plugin | no |

`mcrs_engine_shared` holds no game type, so it takes the prefix `mcrs_`. It sits beside
`mcrs_engine_cell` and depends on nothing of the game. `mcrs_minecraft_engine_world` depends on
it. The vocabulary crate depends on the data crates of layer 3, on `mcrs_engine_shared` and on
`mcrs_minecraft_engine_world`. The code says that the vocabulary must sit above
`mcrs_minecraft_engine_world`: it reads `PlayerSession`, `DimensionList` and
`AddDimensionPlugins` of that crate. `BirthClock` and `TickPace` stay in
`mcrs_minecraft_engine_world`, because its `server_loop` and `entity_births` read them. RFD 7,
determination 5, records this map.

RFD 7 gives the vocabulary crate the bus messages and their plugins. The types of this page in
that crate hold no system. The systems that use a parcel or a
ticket stay in the crates that hold them today, until the crate map moves them.

### Shared state

A datum is a type with one owner in the host. The owner holds it as `Shared<T>`.

```rust
pub struct Shared<T> {
    value: T,
    version: u64,
}

pub trait Datum: Clone + Send + Sync + 'static {
    type Change: Send + 'static;
    fn apply(&mut self, change: Self::Change);
}

pub struct Edit<T: Datum>(pub T::Change);
```

The names `Shared<T>`, `Edit<T>` and `SharedSet` are the maintainer's. The trait `Datum` and
the signatures above are the agent's sketch, and the first code pull request can change them.

A write to a `Shared<T>` in the host increases the version. The host keeps, for each cell, the
version that the cell holds. Before a tick, the host compares the two versions for each datum.
When they differ, the host clones the value and moves the copy into the input of that cell.
When they are equal, the host moves nothing for that datum.

The cell inserts each copy as a resource of the type `T`. A system of a dimension reads
`Res<T>` and never names `X`. A cell has no write access to the host value.

`SharedSet` is a trait that the engine implements for tuples of `Shared<T>` and for tuples of
sets. A library exports its own set as a type alias. The application lists the sets that its
plugins need.

```rust
pub type VanillaShared = (Shared<WorldClocks>, Shared<GameRules>, Shared<TickPace>);
pub type MinigameShared = (Shared<Scoreboard>, Shared<Arena>);
pub type ServerShared = (VanillaShared, MinigameShared);
```

The type `X` appears in the files that build and tick the cells, and nowhere else. `DimCells`
stays a resource without a type parameter, because other crates name it. The cells move from
its field `worlds` into `DimWorlds<X>`. `X` has no default type, so each use states its set.

A plugin cannot add a datum by itself. The maintainer decided that a missing datum is caught
in the `finish` step of the plugin build, and not by the compiler. The agent proposes the
mechanism: a plugin states the data that it needs, and the engine compares that list with `X`.
A missing datum stops the start with an error that names the type.

A datum that changes on every tick, such as `GameTime`, is copied on every tick. That is
correct for a small `Copy` value. A large datum that changes often pays one clone for each
cell and each change. Section "What would reopen this" names the measurement that decides it.

The agent proposes that `Shared<T>` replaces `Versioned<T>`. Two types for one concept are a
second model. Step 4 of the migration deletes `Versioned<T>`.

The agent proposes that a `SharedSet` can save and load its data. The world state writer of
the host calls it, so a datum of a consumer is part of an autosave. A datum states whether it
persists.

### Edits

A cell that wants a change writes an `Edit<T>`. The output of the cell carries the rows of each
datum. After the cells finish a tick, the host merges the outputs in the fixed order of the
cells. One system in the host applies the rows of `T` to `Shared<T>` in that order.

A cell sees its own edit one tick later. The cell writes the edit in tick N. The host applies
it after the cells of tick N finish. The copy reaches each cell with the input of tick N + 1.
Each option in section "Options" has this delay.

This is a change of behaviour. Today a cell applies an edit to its own copy at once and then
sends it. The proposal removes the local write, because a local write gives the datum a second
writer. A caller that reads its own edit in the same tick must change.

An effect of one dimension on a server-wide value takes the same path. A vote of sleepers that
moves a clock is an edit. A command that sets a rule is an edit.

### The shape of the input and the output

The field lists below are the agent's proposal.

```rust
pub struct DimInput<X: SharedSet> {
    shared: X::Copies,
    mail: Vec<ToDim>,
    packets: Vec<InboundPlayerPacket>,
    columns: Vec<ColumnStoreOutput>,
    cell_mail: CellMail,
}

pub struct DimOutput<X: SharedSet> {
    edits: X::Edits,
    client: ClientOutput,
    transfers: Vec<EntityTransfer>,
    ticket_answers: Vec<TicketAnswer>,
    reports: Reports,
    cell_mail: CellMail,
    columns: Vec<ColumnStoreInput>,
}
```

`ToDim` stays a closed enum of the engine. It carries the things that only the engine routes:
a spawn, a despawn, a transfer, an order, a command and a ticket request. Data of a consumer
goes through `X` and never through `ToDim`.

A report is a message with data. The flag that says "a player save failed" becomes a message
that names the session and the error.

### Entity parcels

An `EntityParcel` is an entity in the form that crosses a boundary. It holds:

- the saved record of the entity, which is the entity save compound of
  `docs/spec/persistence.md`;
- the UUID and the kind of the entity;
- the passenger tree, as a list of parcels;
- each reference to another entity, such as a leash holder or an owner, as `EntityRef`.

One parcel type serves each case in which an entity leaves a cell:

| Case | Where the parcel goes |
|---|---|
| A change of dimension (DIM-CHG-001 to DIM-CHG-009) | To the host, then to a cell of the destination |
| A crossing between two cells of one dimension | To the neighbour cell |
| A chunk that unloads | Into the entity part of the stored column |
| A player that leaves | Into the player save |
| A player in no dimension | It stays in the host |

A player is a parcel and a session part. The session part holds what a non-player entity does
not have. That is the session, the view that the client holds and the state of an open
teleport. A
player in no dimension keeps both parts alive in the host. The return does not go through a
save and a load.

One parcel can carry a passenger tree, so a tree can be saved or moved as one unit. This page
does not decide when a tree stays together. DIM-CHG-002 and DIM-CHG-009 state that a mover
leaves its vehicle and that each passenger gets its own teleport. The passengers
specification states the other cases: a save, an unload and a crossing between cells.

A transfer has an answer. The destination accepts the parcel or returns it. On a return, the
source places the entity again. No tick has the entity in two cells.

`EntityRef(Uuid)` names an entity that can be in another cell. A cell never holds the body of
an entity of another cell. A lookup of an `EntityRef` has three answers:

- `Here(Entity)`: the entity is in this cell.
- `NotLoaded`: the entity exists, and this cell does not hold it now.
- `Gone`: the entity does not exist.

`RemovalCause` says why an entity leaves a cell. It replaces `CaptureCause`, which only a
player snapshot has. The cause tells "saved with the chunk" from "saved with the player", and
it tells a death from an unload. The specification of entity transfer lists its values.

### Tickets

One `Ticket` type states each request for chunks.

| Field | Meaning |
|---|---|
| `owner` | Who holds the ticket and can release it: a session, an entity, or an id of a plugin |
| `area` | The chunks that the ticket covers |
| `level` | What the ticket asks for: load only, simulate blocks, or simulate entities |
| `expiry` | A number of ticks, or none |
| `persists` | Whether a save writes the ticket |
| `keeps_active` | Whether the ticket keeps its dimension active |

The field names are the maintainer's. The type of each field is the agent's proposal, and the
ticket specification settles it. A flag that reads as a bare `true` at a call site becomes an
enum, as the style rules require.

A ticket can come from outside its dimension. The host sends a `TicketRequest` to the
dimension through `ToDim`. The dimension answers in its output with `TicketReady` or
`TicketRefused`. The tick does not wait for the answer.

This pair prepares a destination. The host asks the destination for a ticket before a mover
leaves the source. The mover leaves when the answer is `TicketReady`. On `TicketRefused`, the
mover stays. A join uses the same pair for the chunks of its spawn point.

A plugin raises a ticket through the same request. It needs no component in a cell and no
field of its own in the input.

### Time and the dimension table

`GameTime` moves to the vocabulary crate without a change of meaning. `BirthClock` and
`TickPace` stay in `mcrs_minecraft_engine_world`. The host stays the one writer of each.

One `Dimensions` table replaces the three forms of the dimension set. A row holds the key of
the dimension, its dimension type and its place in the dimension order (ARG-ENT-019). The index
of a row is dense and names the dimension in the engine. The table is a datum in `X`, so a
change of the set reaches each cell as a new version.

`DefaultGameMode` and the default operator level leave the engine. They are vanilla policy,
and the vanilla player crate takes them as configuration of its plugin.

### Extension points

These three wants are out of scope. The maintainer decided that each keeps an extension point,
so that a later RFD adds it without a new boundary. Where each point sits is the agent's
proposal.

- **A dimension that opens or closes at run time.** `Dimensions` is a versioned datum, so a
  new row is a new version. The steps that move or save the content of a closed dimension are
  not designed here.
- **An override of a rule or of the weather for one dimension.** A read of a rule resolves in
  one place. That place returns the value of the dimension, else the value of the server.
  Today it returns the value of the server.
- **A leash across dimensions.** `EntityRef` answers `NotLoaded` for a holder in another
  dimension. What the leash does then is a question for its specification.

## Prerequisites

Four specifications come before the code that they describe. Each follows
`docs/spec/README.md`, and each takes its facts about the game from an experiment or from an
existing rule.

- **Tickets**: the kinds, the levels, the expiry, what persists and what keeps a dimension
  active.
- **Entity transfer**: the removal causes, the identity of an entity across a move, and the
  answer of a failed arrival. It extends `docs/spec/dimension-change.md`.
- **Passengers**: the tree, its save, and its move as one unit.
- **The leash**: the reference to a holder, the wait for a holder that is not loaded, and the
  behaviour across dimensions.

Steps 1 to 5 of the migration need none of them. Steps 7 to 11 wait for them.

## Migration

The step order is the agent's proposal. Each step is one pull request that leaves `main` green.
The step that moves the last caller of a path deletes the path.

The project allows three open migrations. Step 1 starts when a place is free.

| Step | Content | Issues |
|---|---|---|
| 1 | Create `mcrs_minecraft_engine_shared`, or use it if step 8 of RFD 7 made it. Move `GameTime` into it. Add `Dimensions` and move each reader to it. | #154 |
| 2 | Create `mcrs_engine_shared` below `mcrs_minecraft_engine_world`. Split `DimWorlds<X>` from `DimCells`. Make `DimInput` and `DimOutput` generic. Move the clocks, the tick pace, the weather, the game rules and the three lists to `Shared<T>` and `Edit<T>`. Update section "Crossing a boundary" of `CLAUDE.md` and `docs/crates.md`. | #154, #1110 step 1 |
| 3 | Create `mcrs_minecraft_vanilla_player`. Move the three lists, the login gate, the default game mode and the default operator level into it. The engine keeps a plugin point for the login gate. | #1110 step 2 |
| 4 | Move the registry snapshot to `Shared<T>` by value. | #1112 step 7 |
| 5 | Turn each report of a cell into a message with data. | none |
| 6 | Write the four specifications of section "Prerequisites". | none |
| 7 | Add `EntityParcel` and `RemovalCause`. Move the dimension change and the cell crossing of a player to the parcel and the session part. | none |
| 8 | Carry a non-player entity and its `EntityRef` values across a dimension and across a cell. Carry a passenger tree as the passengers specification states. | none |
| 9 | Add the entity part to the stored column. Save a parcel with its chunk and load it with the chunk. | none |
| 10 | Add `Ticket`. Move each holder of a ticket to it. | none |
| 11 | Add the ticket request and its answer to the boundary. Move the join and the transfer to a prepared destination. | none |

Issue #1162 replaces `GameRules` with `GameRuleValues`. It runs after step 2 and puts
`GameRuleValues` into the set of the vanilla crates.

Each row below names an old type and the step that deletes it.

| Old type or path | Replaced by | Deleted in |
|---|---|---|
| `DimensionList`, `RunningDimensions`, the module `dimension_order` | `Dimensions` | step 1 |
| The fields of `DimInput` and `DimOutput` for each datum | `X::Copies`, `X::Edits` | step 2 |
| `Globals` in `coordinator/dim_cells.rs` | The versions that the host keeps for each cell | step 2 |
| `DimMail` | The edit rows of the output | step 2 |
| `EditClock`, `EditWhitelist`, `EditBans`, `EditOps`, `ServerEdit` | `Edit<T>` | step 2 |
| `edit_clock`, `edit_whitelist`, `edit_bans`, `edit_ops`, `edit_server` | One host system for each datum | step 2 |
| `Whitelist`, `BanList`, `OpList` in the engine | One list type in the vanilla player crate | step 3 |
| `DefaultGameMode`, `DefaultOpLevel` in the engine | Configuration of the vanilla player plugin | step 3 |
| `Versioned<T>` | `Shared<T>` | step 4 |
| The flag `player_save_failed` of `DimOutput` | A message with the session and the error | step 5 |
| `PlayerTransfer`, `Departure`, `CarriedPlayer`, `Crossing` | `EntityParcel` and the session part | step 7 |
| `CaptureCause` | `RemovalCause` | step 7 |
| `SimulationTicket`, `HeldSimulationArea`, `HotTicket` | `Ticket` | step 10 |
| `RemoteTicket`, `RemoteTickets`, `TicketUpdate`, `TicketMail` | The ticket request and its answer | step 11 |
| `GameRules` | `GameRuleValues` | #1162 |

## Options

The maintainer considered these options for server-wide data and rejected them.

- **Closed fields, as today.** Each datum is a field of the input and of the output. It is the
  least code for the engine. A crate outside the engine cannot add a field, so it fails the
  first requirement.
- **An erased slot.** The input holds a map from a type id to a `Box<dyn Any>`. A plugin
  registers its datum by itself, and no type parameter exists. It breaks the rule against
  trait objects of #566. The `#[expect]` exception of that rule is for a real seam between
  threads, such as a callback of a GPU map. A map of data is no such seam.
- **Typed byte slots.** Each datum has an id, and the input carries its bytes. A `fn` pointer
  decodes them in the cell. It has no trait object. It puts a codec on each datum, which is a
  second model beside the value.
- **A lend, as an extract does.** The host lends its world to each cell for a read. A cell can
  run on a thread, so a tick can meet a world that is lent out and fail.
- **A shared snapshot behind an `Arc`.** Issue #154 proposed it. A change costs one allocation
  and no copy for each cell. It is shared ownership, which #566 removes from library code.

The maintainer decided on one parcel type and one ticket type. The reasons below are the
agent's. A player type beside a second type for other entities would state the passenger tree,
the references and the causes two times. One ticket type for each holder is the state of
today, and each new holder adds a type and a mail path.

## What would reopen this

- A measurement shows that the copy of a datum into each cell costs a visible part of the tick
  budget. The first candidate is a large datum that changes often, such as a scoreboard. The
  pull request of step 2 carries the first measurement for the lists.
- A caller needs its own edit in the same tick, and no change of the caller removes that need.
- The list of data in the application grows until an application cannot keep it correct.
- A cell moves to another process, and a datum cannot cross as a copy.
- A specification of section "Prerequisites" states a behaviour that one parcel or one ticket
  cannot carry.

## Objections

These are the strongest objections that the text does not already answer. A second reader made
a pass on the shared state part of the first sketch, and its objections are in this list.
No second reader has reviewed this page as a whole. That pass comes before the state
discussion.

- A plugin cannot register its datum by itself. The application must list each datum of each
  library. The answer is the nested set: a library exports one alias, and the application
  names that alias once. It stands for an application that composes many libraries.
- A missing datum fails at the `finish` step and not at compile time. It stands. The failure
  comes before the first tick and names the type, so it cannot reach a running server.
- A cell reads its own edit one tick late. It stands for each option, because a cell can run
  on a thread and has no write access to the host.
- The type parameter `X` can spread through the engine. The answer is the split of
  `DimWorlds<X>` from `DimCells`. A system of a dimension reads `Res<T>`, so it does not name
  `X`.
- The function that builds a dimension holds boxed closures today. This page does not remove
  them. They belong to the builder of a cell and not to the boundary.

## Open questions

1. Does `Shared<T>` replace `Versioned<T>`, as this page proposes, or do the registries keep a
   type of their own until #1112 ends?
2. Who answers `Gone` for an `EntityRef` of another dimension? A cell knows only its own
   entities. The answer needs an index of UUIDs in the host, or a request to each dimension.
3. The player roster reaches a cell as a field of the input, and each cell returns its rows.
   It has many writers, so it is no `Shared<T>`. Does it stay a field, or become a datum that
   the host builds from reports?
4. Does a `SharedSet` save and load its data, or does each persistent datum register with the
   world state writer by itself?
5. Resolved: RFD 7, determination 5, sets the names and the places of the two shared crates.
   The name of the vanilla player crate stays open.
6. Which crate owns the plugin point for the login gate after step 3?
7. Does an entity keep its network id when it crosses between two cells of one dimension? The
   specification of entity transfer must state it.

## References

- [RFD 1: Requests for Discussion](0001-requests-for-discussion.md).
- [RFD 2: Mission, principles and values](0002-mission-principles-values.md).
- `CLAUDE.md`: Layers, ECS, Crossing a boundary, Ownership, locks, dispatch and allocation.
- `docs/cells.md`: cells, the order of a merge and the delay of a crossing.
- `docs/spec/dimension-change.md`: DIM-CHG-001 to DIM-CHG-009.
- `docs/spec/argument-entity.md`: ARG-ENT-019.
- `docs/spec/persistence.md`: the saved form of a player.
- Issues #154, #566, #1110, #1112 and #1162.
