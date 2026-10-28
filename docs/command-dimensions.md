# Commands across dimensions

This note states how a command finds and changes players in every dimension. The rules are in
`docs/spec/argument-entity.md` (ARG-ENT-018 to ARG-ENT-021), `docs/spec/dimension-change.md`,
`docs/spec/command-teleport.md` (CMD-TP-013 to CMD-TP-015), `docs/spec/command-execute.md`
(CMD-EX-024) and `docs/spec/command-gamemode.md` (CMD-GM-009).

## The problem

Each dimension runs in its own world, a sub-app of the host. A system of one world cannot read
or write another world. A command can select players of all dimensions and move a player from
one world to another. CMD-TP-014 also requires a synchronous result. The victims move one
after another. A later search sees each earlier move. The feedback comes after the last move.
The result is known when the command returns.

## Where a command runs

The host receives each command line and each suggestion request. It keeps them in its
`CommandQueue` resource. The host parses and runs the lines between two ticks, in
`run_pending_commands`. At that point no schedule runs, and the host holds each dimension world
by a mutable reference. This is a seam: it drives the sub-apps, so it can take `&mut World` of
each dimension.

The lines run one after another, in the order that they arrived. This is the ordering policy.
A line ends, with all its effects, before the next line starts. After the last line, the host
answers the latest suggestion request of each player, in the order of the first request of
that player in the pass (CMDP-SUG-003).

The host owns the one command graph, `CommandTree`. A dimension world holds only the
`commands` packets of each command level, which it sends to a player that joins it.

## The source carries its dimension

A source holds the label of its dimension, the host entity that names the sub-app. The source
entity is the host anchor of the player. A player keeps its host anchor in every dimension, so
a source keeps its player after a change of dimension (DIM-CHG-001). The source dimension and
the dimension of the source entity can differ (ARG-ENT-021).

One source list can hold sources of several dimensions. The executor runs each stage call in
the world of the dimension of its source. `execute in` changes the dimension of the source and
scales x and z. No source leaves the run, so the stages keep the order of CMD-EX-030 across
all dimensions.

## Where a line parses and a selector is resolved

The host fills `CommandTargets` before the first line of a pass. It holds the dimensions in the
dimension order and the online players of all dimensions in the order of the player list. A
player is online when its session places it in the world that holds it. A player that leaves
has no place, so no command finds it, and its lines and requests do not parse.

A line parses in the world of the dimension of its source player. The source takes the
position and the rotation of the player from `CommandTargets`, and the level of the player from
that world. Before the parse, the host lends the graph, `CommandTargets` and the outbox to that
world. A lend is a swap of the values of two resources. A parse error goes to the outbox, and
the line does not run. A suggestion request parses in the same way.

Before a stage call, the host lends the graph, `CommandTargets`, the line and the outbox to the
world of the call. The handler reads its arguments from the graph, and resolves the selector in
that world against `CommandTargets`. The library search (`Selector::select`) applies the world
limit of ARG-ENT-018 and the order of ARG-ENT-019. The distance uses the coordinates alone
(ARG-ENT-020).

The names of the players and the dimensions that a parse suggests come from `CommandTargets`.
The dimensions that a command accepts are the dimensions that run (ARG-RES-004). A source whose
dimension `CommandTargets` does not hold is an internal error: the line fails with
`command.failed`.

## How an effect reaches its world

A handler does not change a player. It writes an effect through `CommandOutput::effect`, with
the `EffectQueue` of the effect type. The value goes to the queue of the world of the call. The
outbox records one function pointer for each effect, in the order of writing. After the call,
the host takes the outbox back. It calls the recorded functions in order. Each function takes
the oldest value of its queue and gives it to the applier of its type. Thus the effects of one
call apply in the order of writing, across all effect types.

A plugin registers an effect type with `AppCommands::add_command_effect`. The registration
gives each dimension world the queue of the type and the applier, a `fn` pointer. The applier
receives `Worlds`: the host, each dimension world in the dimension order, and
`CommandTargets`. `VanillaCommandsPlugin` registers the vanilla effects:

- `SetGameMode` sets the component of the player. Its world sends the packets on its next tick.
- `TeleportPlayer` in the same dimension sets the transform and sends `player_position`.
- `TeleportPlayer` to another dimension calls `change_dimension`, the one operation that moves
  a player between worlds. See "Change of dimension".
- `EditClock` changes a clock on the host and extracts the clocks into each world.

An applier changes a player only through `Worlds::set_transform`, `Worlds::change_dimension`
and `Worlds::set_game_mode`. Each operation also updates the entry of the player in
`CommandTargets`: its dimension, its entity, its transform and its game mode. Thus a later
effect, a later stage and a later line see the change, and the host does not collect the
players again. After the effects, the host sends the messages of the call, so a player that a
command moves hears of the move after the move.

## Change of dimension

`change_dimension` moves a player from one world to another. It takes the host world and the two
dimension worlds, so only a seam between two ticks can call it. The steps are:

1. The host runs its outbound flush. The packets that already have the old epoch leave first.
2. The old world runs the eviction of a disconnect, so observers receive `remove_entities`.
3. The new world spawns the player with its UUID, its host anchor and its owner. Its view and
   its trackers start new there.
4. Each component in `CarriedComponents` of the old world moves to the new entity. The old
   entity is despawned.
5. The host places the session in the new dimension.
6. The host sends `respawn` and then the packets that a join sends after `login`
   (`enter_level_packets`), in the order of DIM-CHG-003.

A plugin of a dimension world adds a component of the player to `CarriedComponents` with
`carry_on_dimension_change`. The list holds one function pointer for each component type. A
component that is not in the list stays in the old world and is despawned with the old entity.
The dimension worlds read the state of a player in each tick, so this state stays on the entity
in the dimension world and does not move to the host.

A system of a dimension world cannot reach another world. A move that a dimension decides in its
tick, for example a portal, writes the message `ChangeDimensionRequest`. Between two ticks, after
the pending commands, the host applies the requests of each world through `change_dimension`
(`apply_dimension_change_requests`). The order is the dimension order and then the order of
writing. A request that names no running dimension, or the dimension of its own world, is an
error in the log.

## Why the result is synchronous

The search, the effects and the messages of one call run in one host step, and no other system
runs between them. A player that the search finds is in its world when its effect applies. Thus
the handler knows each outcome when it writes the effect, and it returns the result and writes
the messages at once. The host applies the effects before it reads the next source or the next
line. This is the barrier at the end of each call.

## Failures

- An effect that names no player of `CommandTargets`, or a player that its world does not hold,
  is an internal error. The applier logs it, and the host applies the other effects.
- A handler that names an `EffectQueue` of a type that no plugin registered does not run. The
  call fails with `command.failed`.
- A player that leaves before the pass is not online, so the search does not find it
  (CMD-TP-015).
- A world without the command resources fails the call with `command.failed`.

## Network ids

A player takes its network id from its host anchor (`player_network_id`). Thus it keeps the id
in every dimension (DIM-CHG-001). Every other entity takes an id from its own index. Players
take the odd ids and other entities the even ids, so the ids of two allocators never meet.

## Stored values

| Value | Kind | Owner |
|-------|------|-------|
| `SourceState::dimension` | truth of the run | the run of the line |
| `CommandQueue` | temporal fact: lines and the latest suggestion request of each player, not parsed | the host; emptied by each pass |
| `CommandTree` | truth: the command graph | the host; lent to one world at a time during a pass |
| `CommandTreePackets` | materialized derived state: the encoded graph of each command level | built once when the graph freezes; each dimension holds handles to the same bytes |
| `CommandTargets` | materialized derived state of the players of all worlds | the host; filled at the start of a pass, changed only by the `Worlds` operations of the effects of the pass |
| `CommandOutbox` | temporal fact: the order of the effects and the messages of one call | the host; emptied after each call |
| `EffectQueue<E>` | temporal fact: the effects of one type that a call wrote | each dimension world; emptied by the effects of the call |
| `DimensionLabel` | truth: the name of the dimension of a world | the sub-app builder |
| Carried components of a player | truth of the player, moved, never copied | the world that holds the player |
| `CarriedComponents` | truth: the component types that a player takes to another dimension | each dimension world; filled when its plugins build |
| `ChangeDimensionRequest` | temporal fact: a move that a dimension decided in its tick | the dimension world that writes it; emptied by the host between two ticks |

## Cost model

- An idle tick costs one look at `CommandQueue` in the host. Nothing else runs.
- An idle tick costs one look at the `ChangeDimensionRequest` messages of each dimension world.
- A pass with lines collects the players once: one iteration of the players of each dimension,
  and one copy of each name into the capacity of the entry of the last pass. The queries are
  built once for each world and kept in `CommandSeam`.
- After an effect, the host updates only the entry of the player that the effect changed. A
  search for a player by its host anchor is a lookup in a hash map of `CommandTargets`. Thus
  `execute as @a run tp @s ...` with N players costs O(N) work in the seam, not O(N²).
- The lend of a parse or a call swaps four resources. Nothing else is copied.
- A call allocates its list of found players. The effects, the outbox and the buffers of the
  host keep their capacity from one call and one pass to the next.
- A change of dimension moves the components of one player and runs the outbound flush of the
  host once, so the packets already stamped for the player leave before its epoch changes.

## Options not taken

- Hand the rest of the chain to the target dimension over a channel. The move would end on a
  later tick, so the result and a later search could not see it.
- Return a placeholder result and finish on a later tick. This breaks `execute store` and the
  order of CMD-TP-014.
- Lend the other worlds to the handler. Then a handler moves a player into the world that runs
  it, through `Commands`, and each command must know the transfer.
