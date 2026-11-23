# Lag compensation and reach: design

Design note for what a client applied (#93) and for reach against what the client saw (#95).
Behaviour comes from `docs/spec/client-knowledge.md` (CK), `docs/spec/reach.md` (RCH) and
`docs/spec/movement-checks.md` (MVC). A name that does not exist in the code yet is planned. Each
part of CK-PART-001 and RCH-PART-001 is a separate piece: a user takes one and pays for no other.

## 1. The mark of a change

The server tick is the mark of each tracked change. The bridge writes one probe at the end of each
server tick on each connection (MVC-LAT-002). Thus the group of tick `t` is each packet between the
end probes of ticks `t − 1` and `t`, and the probe after one group is the probe before the next
(CK-BRK-002). A record of a tracked change holds its tick and no probe id. The knowledge at a
knowledge point is `Knowledge { applied: Mark, seen: Mark }`. A change of a mark up to `applied`
is required, a change up to `seen` is possible, and a later change is not applied (CK-BRK-005).
All the viewers of one block change or one entity move get it in the same tick. Thus its record
is shared, and each viewer reads it with its own `Knowledge` (MVC-UNC-007).

## 2. Classification of the state

| Value | Kind | Owner and cost |
|-------|------|----------------|
| `Brackets` on a connection | authoritative truth: the pings without an answer (wire id, send time, own or foreign), the probe of each mark without an answer, the count of answers | the outbound bridge writes probes, the inbound bridge writes answers; both are in the bridge of `mcrs_minecraft_engine`; O(1) for each packet |
| `Knowledge` of a serverbound packet | temporal fact: copied into `InboundPlayerPacket` at its arrival | `bridge_inbound`; 16 bytes for each packet |
| `KnowledgeHistory` on a player | temporal facts: the `Knowledge` of the last `history_ticks` client ticks | one system at `client_tick_end` |
| `Known<T>` on a player | temporal facts: each value that the server sent with its mark, and the newest required value | the system that queues the packet of that value |
| `ChangeLog<Change>` on a player | temporal facts: velocity, impulse, abilities, attributes and effects with their marks | the system that queues the packet; trimmed `replay_depth` client ticks after the applied state |
| `BlockLog` per dimension | temporal facts: position, old state, new state and mark of each block change that a viewer gets | the system that sends block changes (`VoxelUpdateSet::NetworkSync`); 24 bytes for each change, kept `max_kept_ticks` ticks |
| `SentPositions` on an entity | temporal facts: each entity move that the server sends, with its kind, its mark and each step position as the client decodes it (none for `move_entity_rot`) | the motion publisher; one entry for each sent entity move, at most 42 entries |
| `ViewLog` on a player | temporal facts: the loads and unloads of chunks, the subscriptions of tiles at tier 0, the adds and removes of mobs, with their marks | the system that changes the subscription; one entry for each change |
| entity view `H(k)`, eye set, look set, target region | derived on read by `entity_hull` and the reach engine, into `Local` buffers | none; no storage |
| `TickActions` on a player | authoritative truth: the actions that wait for the end of their client tick (RCH-TCK-002) | the action intake; at most `max_actions_per_tick` entries |
| `CheckedActions` | work queue of the tick; empty after `TickSet::WorldWrite` | the reach engine fills it, reactions change it, consumers drain it |
| `ActionChecked`, `KnowledgeMismatch`, `MovementChecked` | temporal facts (messages) | the reach engine, the knowledge engine, the movement engine of #94 |

No pair of a viewer and an entity has a stored value. A reader computes `H(k)` of CK-ENT-003 only
for the target of an action or a possible pusher. It reads `SentPositions` of that entity, the
`ViewLog` and the `KnowledgeHistory` of the viewer. One read visits at most 42 entries and
`history_ticks` client ticks, an estimated 1 µs. Thus the work follows the actions, at most 10
targets and 24 pushers for each player and tick, not the count of viewers times entities.

Memory for each player: `Brackets` uses at most 38.4 KB (`max_pending_probes` × 16 bytes), about
130 bytes at 200 ms. `KnowledgeHistory` uses 640 bytes. `max_kept_bytes` bounds `Known`,
`ChangeLog` and `ViewLog`, a few hundred bytes in normal play. Blocks use no memory for each
player: the world is the current value, and `BlockLog` holds the changes that a client can undo.

## 3. Crate map

| Crate | Layer | New or existing | Content |
|-------|-------|-----------------|---------|
| `mcrs_minecraft_bracket` (`no_std`, feature `bevy` for derives) | 2 | new | `Brackets`, `BracketLimits`, `Mark`, `Probe`, `Answer`, `Knowledge`, `ChangeState`, `BracketError` |
| `mcrs_minecraft_client_knowledge` (`no_std`, feature `bevy` for derives) | 3, data | new | `Known<T>`, `ChangeLog<T>`, `BlockLog`, `SentPositions`, `ViewLog`, `KnowledgeHistory`, `entity_hull`, `Hull`, `KnowledgeLimits` |
| `mcrs_minecraft_reach` (`no_std`) | 3, rules | new | `check_attack`, `check_interact`, `check_dig`, `check_use_item_on`, `first_entity`, `baseline`, `ReachInput`, `ReachRange`, `Tolerances`, `ReachVerdict`, `ReachOutcome`, `ReachReason`, `Basis` |
| `mcrs_minecraft_move_validate` | 3, rules | existing | `MoveGuard` loses `Probes` and the brackets; it takes `Knowledge`; its constants become `GuardLimits`; `Finding::SkippedAnswer` and `Finding::UnknownAnswer` become `Answer` |
| `mcrs_minecraft_move_predict` | 3, rules | existing | `KeptChanges` becomes `ChangeLog<Change>` and `ChangeStates` becomes `Knowledge`, both borrowed; `Pusher` comes from `Hull`; `Outcome::setback` stays data |
| `mcrs_minecraft_engine` | 4 | existing | `BracketPlugin`; `InboundPlayerPacket` gets `knowledge`; `muxer/probes.rs` and `payload_changes_prediction` are deleted |
| `mcrs_minecraft_engine_client_knowledge` | 4 | new | `ClientKnowledgePlugin`, `KnowledgeSet`, `KnowledgeMismatch`; the record systems and `KnowledgeHistory` |
| `mcrs_minecraft_engine_reach` | 4 | new | `ReachPlugin`, `ReachSettings`, `ActionSet`, `ClientAction`, `CheckedAction`, `CheckedActions`, `ActionChecked`; the intake of `attack`, `interact`, `player_action` and `use_item_on` moves here from `block_break.rs` and `use_item_on_request.rs` |
| `mcrs_minecraft_vanilla_client_reaction` | 5 | new | `DropRefusedEntityActionPlugin`, `RevertRefusedBlockActionPlugin`, `SetbackPlugin` |
| `mcrs_minecraft_client_world` | 4, client | existing | answers each play ping in order (CK-CLI-001, CK-CLI-002); headless, so a bot has it |
| `mcrs_minecraft_network` | 2 | existing | the client answers each configuration ping (CK-CLI-004) |

`block_break::reach` and `block_break::in_reach` are deleted; `mcrs_minecraft_reach::baseline`
replaces them (RCH-VAN-001 to RCH-VAN-004). `TeleportState` keeps the pending id and gets the mark
of the teleport. The movement engine plugin and `MovementChecked` belong to #94.

## 4. Kernel signatures

```rust
impl Brackets {
    pub fn new(limits: BracketLimits) -> Self;
    pub fn wrote(&mut self);
    pub fn foreign_ping(&mut self, wire: u32, now: Duration) -> Result<(), BracketError>;
    pub fn close(&mut self, mark: Mark, now: Duration) -> Result<Option<Probe>, BracketError>;
    pub fn answer(&mut self, wire: u32) -> Answer;
    pub fn knowledge(&self) -> Knowledge;
    pub fn overdue(&self, now: Duration) -> bool;
}

pub fn check_attack(input: &ReachInput<'_>, tolerances: &Tolerances) -> ReachVerdict;
```

- `Answer` is `Own`, `Foreign`, `Skipped` or `Unknown` (CK-BRK-003, CK-BRK-006). The caller
  forwards a `Foreign` answer to its sender. `Probe { wire: u32 }` is the id that the edge encodes.
  The caller gives the time; the crate reads no clock.
- `ReachInput { eyes: &[DVec3], looks: &[Vec3], region: &[Aabb], range: ReachRange }` borrows
  caller buffers. `ReachVerdict { outcome, basis, distance: Option<f64>, bound: f64 }` is `Copy`.
  `ReachOutcome` is `Accepted` or `Refused(ReachReason)`. The check does nothing else.
- `entity_hull(positions: &SentPositions, history: &KnowledgeHistory, view: &ViewLog,
  tick: u64, settle: u32) -> Hull` gives `Hull::Box(Aabb)` or `Hull::Unknown` (CK-ENT-003,
  CK-ENT-004).

## 5. The pipeline in the tick

1. Main app, `FixedPostUpdate`, `OutboundFlush`: the bridge writes the queued packets, then
   `close(Mark(tick))`. The end probe follows each packet that was queued until this flush, also
   a packet in the backlog. A ping of another sender goes through `foreign_ping`.
2. Main app, `bridge_inbound`: each `pong` goes to `answer`. Each other packet gets
   `knowledge()` and goes to its dimension. `overdue` disconnects (CK-BRK-007).
3. Dimension, `TickSet::ClientInput`: `KnowledgeSet::Ingest` pushes `KnowledgeHistory` at
   `client_tick_end`, trims the logs and writes `KnowledgeMismatch` for a skipped teleport
   (CK-TP-002). Then the movement reader of #94 runs, and then `ActionSet::Intake`, `Check` and
   `Filter`, chained. After a position packet, `Check` reads the position that the movement reader
   accepted. After a silent tick, it reads the ball of CK-SIL-001 (RCH-EYE-001).
4. Dimension, `TickSet::WorldWrite`: consumers drain `CheckedActions`. The attack plugin reads it
   in `DamageSet::Collect`. Dig and place write `BlockActionRequest` and `UseItemOnRequest`.
5. Dimension, `TickSet::Output`: each system that queues a tracked packet pushes its record with
   the current tick.

Ordering contract:

- The bridge never writes a packet of tick `t` after the end probe of tick `t`.
- The muxer writes the block changes and the tier 0 moves of tick `t` to each subscribed viewer
  in tick `t`. A tile that it holds back is a `ViewLog` entry, and its values are unknown values.
- A reaction never writes a knowledge record. A setback is a teleport through the normal send
  path, so it gets a mark (CK-RCT-003).
- `CheckedActions` is sorted by the `EntityUuid` of the player, then by the order of its actions.

A refused attack is removed in `ActionSet::Filter`, not in `DamageSet::Filter`. It must play no
sound, keep the attack counter and skip the steps of the attacker (RCH-RCT-001). The attack plugin
does these steps before it writes a `DamageRequest`, and `interact`, dig and place write no
`DamageRequest`. `DamageSet::Filter` keeps the rules of a hit, for example friendly fire.

## 6. Parameters and editions

Each plugin takes its parameters as a value; no environment variable and no fixed constant sets
them. `BracketPlugin { limits: BracketLimits }` holds `max_pending_probes` and `max_ack_delay`.
`ClientKnowledgePlugin { limits: KnowledgeLimits }` holds `max_kept_bytes`, `entity_settle_ticks`,
`max_kept_ticks` and `history_ticks`. `ReachPlugin { settings: ReachSettings }` holds the values of
RCH section 0.5 and `outlines`.

A Bedrock edge keeps each part and gives other values (CK section 13, RCH section 15):

| Difference | Parameter |
|------------|-----------|
| the probe and the answer | the edge encodes `Probe::wire` as the latency probe of the edition |
| the knowledge point | the edge reads `knowledge()` for each input frame |
| the prediction tolerance | `Tolerances::edition`, never for an attack on a player (RCH-INV-001) |
| the outline region | `ReachSettings::outlines`: `Java`, `Bedrock` or `Union` |
| the reaction to a refusal | the edge sends its own revert in place of `RevertRefusedBlockActionPlugin` |

## 7. Use cases

| User | Parts | How the design serves it |
|------|-------|--------------------------|
| Proxy | `mcrs_minecraft_bracket`, `mcrs_minecraft_reach` | It calls `close` at its own marks among the frames that it forwards, `foreign_ping` for each ping of the server, and `answer` for each `pong` (MVC-LAY-007). It removes `Own` answers. It calls `check_attack` with its own eye and target boxes. No Bevy, no tokio. |
| Anticheat plugin | `ActionChecked`, `MovementChecked`, `KnowledgeMismatch`, `CheckedActions` | It reads the verdicts. It replaces a reaction plugin with its own system in `ActionSet::Filter` that reads each `CheckedAction`, for example a count of refusals and a kick. |
| Bot | `mcrs_minecraft_client_world`, `mcrs_minecraft_reach` | It answers each ping in order with no window. It calls `check_attack` with its own view before it attacks, so the server accepts the attack. |
| Replay analyser | `mcrs_minecraft_bracket`, `mcrs_minecraft_client_knowledge`, `mcrs_minecraft_reach` | It feeds a recording to `Brackets` and the logs, then calls the checks offline (RCH-PART-002, CK-INV-002). No Bevy. |
| Minigame | `ReachPlugin`, the reaction plugins | Without `ClientKnowledgePlugin`, `ReachPlugin` uses `baseline`, the vanilla checks. It omits `DropRefusedEntityActionPlugin` in a lobby, or sets `Tolerances`. |
| Bedrock edge (#655) | all parts | Section 6. No fork. |
| Server | all plugins | The default plugin set of `mcrs_minecraft_server` adds each plugin. |

## 8. Migration path

Each step is one pull request. It stays under about 3k added lines, leaves the workspace green and
leaves no second path.

1. Refs #93: `mcrs_minecraft_bracket`; `MoveGuard` keeps its methods and uses `Brackets`.
   `Probes` of `move_validate` is deleted; the constants become `GuardLimits`.
2. Refs #93: `BracketPlugin`: one probe for each tick, foreign pings, `Knowledge` on
   `InboundPlayerPacket`. `muxer/probes.rs`, `payload_changes_prediction` and the bracket methods
   of `MoveGuard` are deleted.
3. Refs #93: the client answers in the apply step and in the configuration state. The
   conformance test of section 10 of `client-knowledge.md` confirms it.
4. Refs #93: `mcrs_minecraft_client_knowledge` with `Known`, `ChangeLog`, `KnowledgeHistory`;
   `move_predict` borrows them; `KeptChanges` and `ChangeStates` are deleted.
5. Refs #93: `mcrs_minecraft_engine_client_knowledge`: the records of velocity, abilities,
   attributes, effects and teleports. The velocity of a player becomes a tracked change, so a
   knockback of a player is possible. A test replays a knockback at 1000 ms.
6. Refs #93: `BlockLog` and the chunk entries of `ViewLog`; rules first, then the engine.
7. Closes #93: `SentPositions`, the tile and mob entries of `ViewLog`, `entity_hull`, `Pusher`.
8. Refs #95: `mcrs_minecraft_reach`, with a table test for each comparison of RCH section 8.
9. Refs #95: `mcrs_minecraft_engine_reach`; the old intake and `block_break::reach` are deleted.
10. Closes #95: the reaction crate. `SetbackPlugin` joins here when #94 has its movement plugin,
    else in the pull request of #94.
11. Refs #106: `mcrs_minecraft_vanilla_player_attack` (`docs/damage.md`, which closes #106).

## 9. Open points

1. CK-BRK-009 bounds the records of one connection, but `BlockLog` and `SentPositions` are shared.
   Recommended: CK-BRK-009 allows a shared record with the age bound `max_kept_ticks`.
2. MVC-LAT-001 defines a group as one flush. Recommended: a group is the packets of one server
   tick; the knowledge is the same (section 1).
3. A target outside the tier 0 tiles of a viewer has no shared positions. Recommended: its
   positions are unknown values, and RCH-VRD-004 gives the baseline.
4. The contract on the muxer in section 5 has no test. Recommended: a muxer test in step 7.
5. #94 states that the setback is not optional. Recommended: the server adds `SetbackPlugin` by
   default, and a user can replace it.
6. `mcrs_minecraft_client_world` answers a ping at its arrival, in an observer. Recommended:
   answer in the apply step of the client tick (CK-CLI-002), and confirm it with the conformance
   test of section 10 of `client-knowledge.md`.
