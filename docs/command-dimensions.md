# Commands across dimensions

This note states how a command finds and changes players in every dimension. The rules are in
`docs/spec/argument-entity.md` (ARG-ENT-018 to ARG-ENT-021), `docs/spec/dimension-change.md`,
`docs/spec/command-teleport.md` (CMD-TP-013 to CMD-TP-015), `docs/spec/command-execute.md`
(CMD-EX-024) and `docs/spec/command-gamemode.md` (CMD-GM-009).

## The problem

Each dimension runs in its own world, a cell of the coordinator. No part reads or writes the
state of another part (`cells.md`, section 2). A command can select players of all dimensions
and move a player from one world to another. Thus a command runs in one world, and its effects
on other worlds are messages.

## Where a command runs

The muxer routes each serverbound packet of a player to the dimension of the player. A command
line and a suggestion request are such packets. The world raises them on the entity of the
player at the sync point, and the command engine keeps them in the `CommandQueue` of the world.

The world runs its commands in `run_commands`, in `FixedPreUpdate`, after the input of the tick
reached the players. The order is:

1. The runs that other worlds handed over (`CommandHandoff`), in the order they came.
2. The command lines of the players of this world, in the order they arrived.
3. The latest suggestion request of each player, in the order of the first request of that
   player in the tick (CMDP-SUG-003).

A line ends, with all its effects in this world, before the next line starts. This is the
ordering policy.

The host keeps the command graph that it froze, `CommandTree`. Each dimension world holds a
copy of the graph, the handlers and the `commands` packets of each command level.

## The source carries its dimension

A source holds the label of its dimension, the host entity that names the world. A source keeps
its player by the session, which names the player in every dimension (DIM-CHG-001). The source
dimension and the dimension of the source entity can differ (ARG-ENT-021).

One source list can hold sources of several dimensions. `execute in` changes the dimension of
the source and scales x and z.

## Where a stage call runs

Each handler has a world (`HandlerWorld`):

- `Any`: the handler reads only the targets, the registries and the copies of global state
  that every world holds. It runs in the world of the run. A node gets it with
  `NodeRef::in_any_world`.
- `SourceDimension`: the handler reads the blocks, the chunks or the type of the source
  dimension. This is the default.

A call whose handler has `SourceDimension` and whose source is in another dimension does not run
here. Its source goes into a `CommandHandoff` for the world of that dimension, with the stage,
the cost so far and the source the line parsed for. The world sends the handoffs in its output.
The coordinator hands them to the muxer, and the muxer puts each one in the input of its world
for the next tick. That world parses the line again for the same source, and runs the chain on
from the stage (`mcrs_command::resume`).

The vanilla `execute` modifiers, the conditions `entity` and `dimension`, `teleport` and
`gamemode` run in any world. The conditions `block` and `loaded`, `positioned over`, `time` and
`locate` run in the source dimension.

## Where a line parses and a selector is resolved

Before the commands of a tick, the world fills `CommandTargets`:

- the running dimensions in the dimension order, from the copy of `RunningDimensions` that the
  coordinator sends when the set changes;
- the players of this world, as they are now;
- the players of the other worlds, from `ForeignPlayers`.

Each world reports its players in the output of each tick. The coordinator keeps the reports in
`PlayerRoster`. When the input of a world carries a command line, a suggestion request or a
handoff, the coordinator adds the players of the other worlds to that input. Thus a player of
another world is where its world left it at the end of the last tick. A player that leaves a
world in a tick is in no report for that tick.

A line parses in the world of its player. A parse error goes to the player, and the line does
not run. A suggestion request parses in the same way. The handler reads its arguments from the
graph, and resolves the selector against `CommandTargets`. The library search
(`Selector::select`) applies the world limit of ARG-ENT-018 and the order of ARG-ENT-019. The
distance uses the coordinates alone (ARG-ENT-020).

## How an effect reaches its player

A handler does not change a player. It writes an effect through `CommandOutput::effect`, with
the `EffectQueue` of the effect type. After the call, the world applies the effects of the call
in the order of writing, across all effect types. Then it sends the messages of the call, so a
player that a command moves hears of the move after the move.

An applier receives `Worlds`: the world of the run and `CommandTargets`. It changes a player
only through `Worlds::teleport` and `Worlds::set_game_mode`. Each operation updates the entry of
the player in `CommandTargets`, so a later effect, a later stage and a later line of the tick
see the change. Then:

- A player of this world changes at once. A teleport in this dimension sets the transform and
  sends `player_position`. A teleport to another dimension asks this world to let the player go
  after the tick (`DepartureRequests`). A later move in the same tick replaces the request.
- A player of another world gets a `PlayerOrder`. The world sends the order in its output. The
  muxer puts the order in the input of the dimension of the session, after it routed every
  departure of the tick. That world applies the order at its sync point, after the arrivals of
  the tick.

`Worlds::edit_clock` changes the copy of the clocks in this world at once, so the rest of the
tick reads the new time. The world sends the change in its output. The coordinator applies it to
the clocks that it owns, sends `set_time` to every player, and sends the clocks to every world
in the next input.

The admin message of a command is an `OperatorBroadcast` event in the world of the run. The
server observes it there. It sends the message to each player of `CommandTargets` that the copy
of the operator list names, but the source.

## Change of dimension

A player changes dimension as a `Departure`. A departure holds the session, the key of the new
dimension, the arrival and the carried state of the player (`CarriedPlayer`). The carried state
is a plain struct of the components that go with the player. A component that is not in it stays
in the old world and is despawned with the old entity.

A portal writes `ChangeDimensionRequest`, and a command writes a departure request. At the end
of tick T, the old world takes the carried state of each player that a request names. Then it
despawns the player and puts the departure in its output, after its packets. A game mode that
changed in tick T reaches the viewers before the despawn.

The host reads the departures when it takes the outputs, in the dimension order. For each
departure, the steps are:

1. The host runs its outbound flush. The packets that already have the old epoch leave first.
2. The host places the session in the new dimension, which changes its epoch.
3. The host claims the teleport id of the arrival from the carried state.
4. The host sends `respawn`, the packets that a join sends after `login`
   (`enter_level_packets`), in the order of DIM-CHG-003, and then the `player_look_at` of the
   arrival.
5. The host puts `ToDim::Transfer(PlayerTransfer::Arrive)` in the input of the new dimension
   for tick T+1.

In tick T+1, the new world spawns the player before the serverbound packets and the orders of
the tick reach it. A departure that names no other running dimension, or whose session is no
longer placed in the old dimension, goes back to the old world as `PlayerTransfer::Return`.

## Delays across worlds

An effect in the world of the run applies in the same tick, as in vanilla. An effect on another
world is late:

| What | Delay |
|------|-------|
| A call of a `SourceDimension` handler for a source in another dimension | it runs 1 tick later |
| An order for a player of another world | it applies 1 tick later |
| A player of this world that a command sends to another dimension | it is there 1 tick later |
| A player of another world that a command sends to a third dimension or to this one | it is there 2 ticks later |
| A clock change, for the other worlds | they read it 1 tick later |

Thus CMD-TP-014 holds in the world of the run: the result counts the victims, and a later
search of the same tick in that world finds each victim in its destination. A search in a later
tick does not find a victim that is between two worlds.

The sequence limit and the fork limit count in each world. A chain whose sources reach another
dimension before the run stage can therefore run more than the sequence limit in total.

## Failures

- An effect that names no player of `CommandTargets` is an internal error. The applier logs it,
  and the world applies the other effects.
- An order that names a player that no dimension holds is an error in the log of the muxer. An
  order that reaches a world after its player left is an error in the log of that world.
- A handler that names an `EffectQueue` of a type that no plugin registered does not run. The
  call fails with `command.failed`.
- A source whose dimension does not run fails the call with `command.failed`.

## Network ids

A player takes its network id from its session (`player_network_id`). Thus it keeps the id in
every dimension (DIM-CHG-001). Every other entity takes an id from its own index. Players take
the odd ids and other entities the even ids, so the ids of two allocators never meet.

## Stored values

| Value | Kind | Owner |
|-------|------|-------|
| `SourceState::dimension` | truth of the run | the run of the line |
| `CommandQueue` | temporal fact: lines and the latest suggestion request of each player, not parsed | each dimension world; emptied by its commands of the tick |
| `CommandTree` | truth: the command graph | the host freezes it; each dimension world holds a copy |
| `CommandTreePackets` | materialized derived state: the encoded graph of each command level | built once when the graph freezes; each dimension holds handles to the same bytes |
| `CommandTargets` | materialized derived state of the players of all worlds | each dimension world; filled before its commands of a tick, changed only by the `Worlds` operations |
| `PlayerRoster` | materialized derived state: the players that each world reported | the coordinator; replaced by each report |
| `ForeignPlayers` | copy of the rows of `PlayerRoster` for the other worlds | each dimension world; replaced by each input |
| `RunningDimensions` | truth: the dimensions that run | the coordinator; each world holds a copy |
| `DepartureRequests` | temporal fact: the players that the commands of a tick send away | each dimension world; emptied after the tick |
| `DimMail` | temporal fact: the orders, clock changes and handoffs of a tick | each dimension world; emptied into the output of the tick |
| `CommandOutbox` | temporal fact: the order of the effects and the messages of one call | each dimension world; emptied after each call |
| `EffectQueue<E>` | temporal fact: the effects of one type that a call wrote | each dimension world; emptied by the effects of the call |
| `DimensionLabel` | truth: the name of the dimension of a world | the sub-app builder |
| Carried components of a player | truth of the player, moved, never copied | the world that holds the player |

## Cost model

- An idle tick costs one run condition in each world: a look at `CommandQueue` and at the
  handoffs. Nothing else runs.
- Each world reports its players at the end of each tick: one row for each player, with a copy
  of its name.
- A tick with commands collects the targets once: one iteration of the players of the world and
  one copy of each row of the other worlds into the capacity of the entries of the last tick.
- The coordinator copies the rows of the other worlds only into the input of a world that runs
  commands in that tick.
- A handoff copies the line, the origin source and the sources of one stage, and the world of
  the handoff parses the line again.
