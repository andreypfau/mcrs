# Lag compensation and reach: design

Design note for what a client applied (#93) and for reach against what the client saw (#95).
Behaviour comes from `docs/spec/client-knowledge.md` (CK), `docs/spec/reach.md` (RCH) and
`docs/spec/movement-checks.md` (MVC). This note states the scheme that the code follows now.

## 1. The acknowledged tick

Each connection has one number: the tick that the client acknowledged. `PlayPings` keeps it, and
it is the one owner of the value. The tick of `PlayPings` counts the end flushes of the connection
in the play state, from 0. Thus it is a counter of one connection, not the server tick.

The bridge writes the play pings. The id of a ping is its tick.

- In a tick with packets, the bridge writes one ping before the first packet of that tick.
- The first play tick gets no ping, so the login packet stays the first packet.
- At the end of an empty tick that follows a tick with packets, the bridge writes one closing ping.
- Another empty tick gets no ping.

`answer` takes the id of a `pong`. A known id acknowledges that ping and each older ping, and
`answer` returns `true`. The bridge then drops the `pong`. An unknown id returns `false`, and the
move guard reports it and drops the `pong`. `PingLimits` bounds the pings without an answer: at
most 2400, and none older than 60 s. Past a bound, the bridge disconnects the client with
`disconnect.timeout`.

## 2. The state of a change

The ping of tick `t` comes before the packets of tick `t`. Thus for a change that the server sends
in tick `t` and an acknowledged tick `a`:

| Comparison | State |
|------------|-------|
| `a > t` | applied |
| `a = t` | window: applied or not |
| `a < t` | not applied |

`MoveGuard::serverbound` takes `Acknowledged { tick, sent_at }` from the bridge for each play
packet. `sent_at` is the send time of the acknowledged ping. The guard keeps the tick of each change
of the predicted state that it has not seen as applied. It removes a change at the end of a client
tick when the change is applied.

## 3. The value on a packet and the order of packets

`InboundPlayerPacket` has `acknowledged_tick` and `arrival`. The bridge copies both at the arrival
of the packet. Before a dimension gets the packets of one tick, the router sorts them by arrival,
with the session as the tie break (PHY-ORD-018). The handlers of the dimension see the packets in
that order.

The hits of client packets apply at the packet apply point, before the entity pass, in arrival
order. The hits of the pass apply after the pass, in sorted order. `DamageSet` of
`mcrs_minecraft_engine_damage` has the two points; `docs/damage.md` section 4 states them.

## 4. Classification of the state

| Value | Kind | Owner and cost |
|-------|------|----------------|
| `PlayPings` on a connection | authoritative truth: the tick, the pings without an answer (tick, send time), the acknowledged ping | the bridge; O(1) for each ping; at most 2400 entries of 24 bytes |
| `acknowledged_tick`, `arrival` of `InboundPlayerPacket` | temporal facts, copied at arrival | the inbound bridge; 24 bytes for each packet |
| the change ticks of `MoveGuard` | temporal facts: the tick of each change that is not applied | the outbound bridge pushes, the guard trims at each client tick end |
| `Acknowledged`, the state of a change | derived on read | none |

## 5. Crate map

| Crate | Layer | Content |
|-------|-------|---------|
| `mcrs_minecraft_network` | 2 | `PlayPings`, `PingLimits` |
| `mcrs_minecraft_move_validate` | 3, rules | `MoveGuard`, `Acknowledged`, `ChangeState`, `ChangeView` |
| `mcrs_minecraft_engine` | 4 | the bridge: pings in the flush, `answer` at arrival, the values on `InboundPlayerPacket`, the order of the dimension input |
| `mcrs_minecraft_engine_damage` | 4 | `DamageSet` with the packet apply point and the pass apply point |
| `mcrs_minecraft_vanilla_player_attack` | 5 | the attack, the baseline reach check, the knockback |

## 6. Decisions

- One number for each connection. No probe for each change, and no knowledge state for each pair
  of a viewer and an entity.
- No knowledge crate and no generic change log. Each consumer keeps its own small log of the ticks
  of its changes. `MoveGuard` is the first consumer.
- The packets of one tick apply in the order of arrival.
- The server does not predict the movement of a player yet (#94). Thus the client knowledge of #93
  does not block the knockback of a player.

## 7. Rejected

| Idea | Reason |
|------|--------|
| a separate crate for ping tracking | the pings are network state; they belong to the network crate |
| a generic knowledge type | it has no second user |
| order by the time that the client saw | more state than the arrival order, for a result that an observer cannot tell apart |

## 8. Open points

- The acknowledged tick is a counter of one connection. Two connections cannot compare it. A
  consumer uses the number of ticks without an acknowledgement, with a limit (#95).
- The value does not reach a handler inside the dimension yet. The event of a handler does not
  carry it (#95).
- The reach check uses the baseline: the eye to the target box, the range plus 3.0. It does not use
  the positions that the client saw (#95).
- The client knowledge of applied changes other than the predicted state (#93).
- The ping state does not reset when the connection goes back to the configuration state
  (CK-BRK-010).
- The bridge matches a `pong` by its id, not by the order of the connection. A ping of another
  sender gets no answer (CK-BRK-003, #93).
- Experiments of CK and RCH wait for their tools (#1103).
- Some handlers, for example the inventory input, still take exclusive world access through a
  queued closure (#1099).
