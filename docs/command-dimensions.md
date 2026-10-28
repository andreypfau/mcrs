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

A handler does not change a player. It writes an effect to the outbox through `CommandOutput`:
a teleport or a game mode, for a host anchor. After the call, the host takes the outbox back
and applies the effects in order. Each effect applies in the world that holds its player:

- A game mode sets the component of the player. Its world sends the packets on its next tick.
- A teleport in the same dimension sets the transform and sends `player_position`.
- A teleport to another dimension moves the player from one world to the other
  (`change_dimension`). The old world runs the eviction of a disconnect, so observers receive
  `remove_entities`. The player takes its own state to the new world. Its view and its trackers
  start new there. The host places the session in the new dimension. Then the host sends the
  packets of DIM-CHG-003.

After each effect, the host updates the entry of the player in `CommandTargets`. Thus a later
effect, a later stage and a later line see the move. Then the host sends the messages of the
call, so a player that a command moves hears of the move after the move.

## Why the result is synchronous

The search, the effects and the messages of one call run in one host step, and no other system
runs between them. A player that the search finds is in its world when its effect applies. Thus
the handler knows each outcome when it writes the effect, and it returns the result and writes
the messages at once. The host applies the effects before it reads the next source or the next
line. This is the barrier at the end of each call.

## Failures

- An effect that names no player of `CommandTargets`, or a player that its world does not hold,
  is an internal error. The host logs it and applies the other effects.
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
| `CommandTargets` | materialized derived state of the players of all worlds | the host; filled at the start of a pass, changed only by the effects of the pass, dropped at its end |
| `CommandOutbox` | temporal fact: the effects and messages of one call | the host; emptied after each call |
| `DimensionLabel` | truth: the name of the dimension of a world | the sub-app builder |
| `Carried` | truth of the player, moved, never copied | the world that holds the player |

## Cost model

- An idle tick costs one look at `CommandQueue` in the host. Nothing else runs.
- A pass with lines costs one query of the players of each dimension and one `CommandTargets`
  with a copy of each name. The lend of a parse or a call swaps four resources. Nothing else is
  copied.
- A call allocates its list of found players and its effects. The buffers of the host
  (`CommandSeam`) keep their capacity from one pass to the next.
- A change of dimension moves the components of one player and runs the outbound flush of the
  host once, so the packets already stamped for the player leave before its epoch changes.

## Options not taken

- Hand the rest of the chain to the target dimension over a channel. The move would end on a
  later tick, so the result and a later search could not see it.
- Return a placeholder result and finish on a later tick. This breaks `execute store` and the
  order of CMD-TP-014.
- Lend the other worlds to the handler. Then a handler moves a player into the world that runs
  it, through `Commands`, and each command must know the transfer.
