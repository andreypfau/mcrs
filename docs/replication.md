# Entity Replication at Scale

This document describes how the Muxer sends the entities of a world to its clients. The target is
10,000 players in one world, with the vanilla client and the vanilla protocol. The design changes
only the server.

It extends `cells.md`. The Muxer, the cells, the links and the placements stay as that document
defines them. The engine does not know Minecraft: the game supplies the codec (section 7.3).

---

# 1. Terms

```text
Viewer
    A client that receives entities from the Muxer.

Subject
    An entity that a viewer can receive. Each player is a subject for the other players.

Visible set
    The subjects that a viewer has on its client at one time.

Track radius
    The distance from a viewer inside which a subject can enter the visible set.

Tile
    A square of columns. The tile is the unit of entity interest. One cell owns each tile.

Tile delta
    The entity events of one tile in one tick: spawn, move, data, enter, leave, despawn.

Tile stream
    The snapshot of a tile and the tile deltas after the snapshot, in tick order.

Tier
    A rate of updates. A tier sends the move of a subject every n ticks.

Layer
    A part of the subjects of a tile. A rank that is the same for all viewers sorts the subjects
    into layers.

Cap
    The maximum number of subjects in a visible set. This document calls it K.

Budget
    The maximum number of bytes per second that the Muxer sends to one viewer for entities.

Delta base
    The last position that a tier sent for a subject. A relative move starts from it.

Keyframe
    The absolute positions of the subjects of a tile, for one tier.

Spawn bundle
    The packets that make a subject appear on a client, in one bundle.

Profile entry
    The entry of a player in the player list of a client. The client needs it to show the player.

Edge
    One process of the Muxer. An edge holds a part of the connections.
```

---

# 2. The problem

The entity traffic of a world is:

```text
bytes per second = sum over viewers of (visible subjects × updates per second × bytes per update)
```

When players spread out, each visible set is small, and the total grows with N. When players
stand together, each viewer sees almost all other players, and the total grows with N². No
protocol removes this term. A client that shows N players must receive N players.

The Minecraft protocol adds four costs:

- The encryption keeps a stream state for each connection. Two connections never share
  encrypted bytes.
- The protocol has no packet for many entities. Each move is one packet of about 15 bytes.
- The reference server sends the latency of all N players to all N players at a fixed interval.
  It also sends the whole player list to each new player.
- The client shows a player only after it has the profile entry of that player.

The table gives estimates from a model of packet sizes, not measurements. The model assumes that
70% of the players move, a view distance of 10, a session of 30 minutes and K = 200.

| Case | Reference: per viewer / total | This design: per viewer / total |
|---|---|---|
| each viewer sees 50 players | 41.5 KB/s / 415 MB/s | 2.4 KB/s / 24 MB/s |
| each viewer sees 200 players | 71 KB/s / 714 MB/s | 9.5 KB/s / 95 MB/s |
| each viewer sees 1,000 players | 231 KB/s / 2.3 GB/s | 23 KB/s / 229 MB/s |
| all 10,000 players in one place | 1.9 MB/s / 19.3 GB/s | 32–51 KB/s / 315–512 MB/s |

Even with 50 visible players, the player list of the reference server costs about 30 KB/s for
each viewer. This traffic does not depend on the visible set.

Two limits do not change with any design:

- The downlink of one client. 1.9 MB/s is more than many home connections give.
- The draw cost on the client. The client draws each player that the server spawns. Near 1,000
  visible players, the frame rate of the client falls before the network fails.

Thus the server must cap the visible set. All large games do this (section 3).

---

# 3. What other games do

- **EVE Online** serializes an update one time for each group of ships, and copies it to each
  observer. The cost fell from O(n·m) to about O(n + m). Time dilation slows a node to 10% as the
  last guard. One battle held 6,739 pilots in one system.
- **Improbable** sends 10 updates per second for about 500 players of high priority, and 2 for
  the other 9,500. Their own estimate: 10 times the players give 100 times the data.
- **Unreal Engine** (Replication Graph, Iris) keeps prebuilt lists for each cell of a spatial
  grid. It uses a priority accumulator, a budget for each connection and shared serialization.
- **Glenn Fiedler** puts the top 64 objects by accumulated priority into each packet, under a
  fixed bandwidth.
- **Hyperion**, a Minecraft server in Rust, sends one message for each broadcast to its proxies.
  The proxies find the players of the region with a BVH and do the encryption. Hyperion limits the
  draw to 400–700 players.
- **Minestom** keeps one buffer of framed packets for each chunk in a tick. Each viewer gets
  slices of that buffer. The buffer keeps the byte ranges of each player, so a player never gets
  its own packets.
- **Folia, MultiPaper and server meshing** divide the simulation work. They do not reduce the
  bytes to the clients. In Folia, a dense crowd becomes one region on one thread.

These techniques do not transfer:

- Level of detail and impostors on the client. They need client code.
- Snapshot deltas with acknowledgement, as in Quake 3 and Source. They need datagrams.
- Layers of a world, as in World of Warcraft. Players on different layers do not see each other.

---

# 4. Principles

1. The Muxer decides what each viewer receives. The tick never reads the Muxer.
2. The work of the Muxer in a tick is proportional to the changes and the subscriptions. It is
   not proportional to subjects × viewers.
3. The Muxer encodes each tile delta one time for each tier and layer. All viewers of that tier
   and layer receive the same bytes.
4. Only the encryption and the socket write occur once for each connection.
5. A budget limits the bytes for each viewer. A cap limits the visible set.
6. A subject that does not change costs zero bytes.
7. Entity replication has the policy `semantic` (sharding document, section 2). Each subject in
   the track radius reaches the viewer when the cap and the budget allow it. The update rate can
   differ from the reference. The cap, the impostors and time dilation are
   `intentional_deviation`.

---

# 5. Client behaviour that the design uses

Each line states a behaviour of the vanilla client. Each line is an assumption until a rule in
`docs/spec/` states it (`clean-room.md`). An implementation must not depend on an assumption
that has no rule.

| ID | Behaviour | Use in this design |
|---|---|---|
| C1 | The client ignores a move, data, motion, sync or remove packet for an entity id that it does not know. | A viewer can receive a tile blob with subjects that it does not have. |
| C2 | The client does not create a player entity without the profile entry of that UUID. It logs a warning. | The profile entry goes before the spawn, in the same bundle. |
| C3 | The client draws a player whose profile entry is not listed. | Profile entries of the visible set use `listed = false`. |
| C4 | The player list shows a maximum of 80 entries. | The Muxer lists a maximum of 80 players. |
| C5 | The client interpolates each move over a fixed number of client ticks. A path of steps with tick offsets keeps the motion continuous. With one step each 4 ticks, the subject does not stop. With one step each 10 ticks, it stops about half of the time. | Tiers 1 to 3 send paths of steps. |
| C6 | A relative move changes each axis by a maximum of about 8 blocks for each step. A larger change needs an absolute position sync. | The Muxer sends a keyframe when a delta overflows. |
| C7 | The client draws players up to some hundreds of blocks. Only the server decides which players a client shows. | The cap is the only limit of the draw cost. |
| C8 | The client applies all packets of a bundle in one client tick. A bundle holds a maximum of 4,096 packets. | Spawn bundles are atomic. |
| C9 | A spawn with an id that the client knows replaces the entity. | A spawn bundle can repeat without a remove. |
| C10 | The client turns the head of an entity over a short fixed time. At a low rate, the head turns in jumps. | Far tiers send head rotation at a low rate only. |
| C11 | The ticking state packet changes the tick rate of the client. | Time dilation (section 7.9). This assumption has no confirmation. |
| C12 | The protocol compresses each packet alone. Packets below the threshold go without compression. | Compression gives nothing for moves. It matters for chunks. |

---

# 6. Where the code is today

| Place | Today | Cost |
|---|---|---|
| Tile deltas, `crates/mcrs_minecraft_engine/src/publish_player_tiles.rs` | Each dimension keeps the tile of each player. A tile is 2 × 2 columns and the full height of the world. At the end of a tick, the dimension publishes the spawn, the move, the change of tile, the change of game mode, the messages and the despawn of its players, sorted by tile and by player, in its `DimOutput`. | O(players that change) each tick. |
| Muxer, `crates/mcrs_engine_muxer` | The host sends the tile deltas of each dimension to one `MuxerStage` over a `Link`. The `muxer` field of the server plugin places the link: `Thread` by default, or `Inline`. The stage keeps one `Muxer` for each dimension, with `MuxerSettings`: the tile width, the cap K of 200 and the budget of 32 KiB/s. A player subscribes to the tiles within 3 tiles of its own tile, about 96 blocks on X and Z. The host changes the subscription when the player spawns, changes tile or despawns. In each tick, the planner (`planner.rs`, a pure function) gives each layer of each tile a tier, as section 7.4 states. The distance comes from the position of the own subject of the viewer, on X and Z. A tile that the cap leaves out after K gets layer 0 only, at tier 3. The budget is a moving average of 16 ticks. Above the budget, the planner lowers the last layer by one tier each 10 ticks. Below 70%, it raises the lowered layer with the largest accumulator each 40 ticks. The writer queue does not reach the budget. There are no pinned subjects. | One encode for each tile and tier that is due, has events to send and has viewers. All viewers of a tier share the bytes, cut by layer. The planner costs O(tiles in view) for each viewer and tick. With 300 bots together, a session gets 1.4 KB of muxer bytes in each tick (27 KB/s), and 4.2 KB without tiers. With about 750 bots, it gets 1.7 KB, and 6.3 KB without tiers. |
| Codec, `crates/mcrs_minecraft_replication/src/player_codec.rs` | The codec keeps one delta base for each subject and tier, and the poses of the last 20 ticks. Tier 0 sends one relative move. Tiers 1 to 3 send a path with a step at most each 4 ticks. A step carries the ticks after the step before it. This is an assumption (C5). A step of more than about 8 blocks, a move after 100 ticks, or a subject that enters the tile or changes layer sends an absolute position sync. A viewer that changes tier gets a keyframe of the tile at the bases of the new tier. The muxer caches the keyframe by tile, tier and tick. A spawn bundle places the subject at the base of the tier of the viewer. The bundle is the same at all tiers: the profile entry and the spawn. The codec has no equipment and no entity data yet (section 7.7). The codec caches the spawn packet of each tier until that base moves. After the muxer unsubscribes a viewer, the codec sends it nothing more. | One encode of a spawn for each subject, tier and move, and one profile entry for each viewer that does not have it. |
| Remove packets | A maximum of two remove packets with a list for each viewer and tick: one for the tile deltas, one for the changes of the plan. | None. |
| The host, `crates/mcrs_minecraft_engine/src/muxer/entity_replication.rs` | The host takes the bytes of each viewer from the muxer at the end of the time between ticks, and appends them to the outbound packets of its session. The muxer bytes are records: the length of each packet, then the packet with its id. | The muxer thread works while the host does the rest of the time between ticks. |
| `crates/mcrs_minecraft_engine/src/muxer/bridge.rs` | The flush runs on one thread. It encodes a packet for many recipients one time, and all recipients share the bytes. It appends the records of the muxer without a copy. | The writer of each connection frames and compresses the records of the muxer. A frame for each compression threshold is not shared yet. |
| `crates/mcrs_minecraft_network/src/outbound.rs` | The flush frames and compresses a shared packet one time for each compression threshold. The writer sends this `Framed` part as it is. The records of the muxer go as a `Packets` part. | One deflate for each chunk version in a flush. |
| Profile entries, `crates/mcrs_minecraft_replication/src/player_list.rs` | The codec of each dimension keeps the entries of each viewer in that dimension. Section 7.5 gives the rules. A change of game mode goes only to the viewers that have the entry. | The policy reads all players of the dimension: O(N) for each viewer that it checks. |
| Chat, `crates/mcrs_minecraft_chat_server/src/chat_plugin.rs`, `crates/mcrs_minecraft_engine/src/publish_player_tiles.rs` | Global chat is system chat. The join and leave messages are data events of the tile deltas. The codec sends them to the viewers whose client holds the player. | One encode for each message. |
| Entity ids, `crates/mcrs_minecraft_engine/src/network_id.rs`, `crates/mcrs_minecraft_engine/src/entity_ids.rs` | The `EntityId` of a player comes from its session and is odd. An entity that is not a player gets an even `GlobalEntityId` when it spawns. The Coordinator gives each dimension world a range of 4,096 even ids when the world spawns. A world asks for the next range in its `DimOutput` when fewer than 2,048 ids remain. The grant comes in the `DimInput` of the next tick, so a world can take 2,048 ids in each tick. The Coordinator answers the requests in the order of the cells, so Inline and Thread placement give the same ids. An entity that a cell takes in keeps its `GlobalEntityId`. The wire id is the low 32 bits of the `EntityId`, so it depends on that id only, and players and other entities never share a wire id. | No cost when no entity spawns. One range of ids for each 4,096 entities that spawn. |
| Encryption | None. The server runs in offline mode only. | No cost yet. |

---

# 7. Design

## 7.1 The boundary

A cell does not build packets. At the end of a tick, a cell publishes one batch of tile deltas:

```rust
pub struct TileDeltas { pub tick: Tick, pub tiles: Vec<TileDelta> }                      // by TileKey
pub struct TileDelta { pub tile: TileKey, pub count: u32, pub events: Vec<EntityEvent> } // by EntityId
pub enum EntityEvent { Spawn(..), Move(..), Data(..), Enter { from: TileKey }, Leave { to: TileKey }, Despawn, Effect(..) }
```

- The cell publishes only subjects that changed: a new transform, a spawn, a despawn, a move to
  another tile.
- `count` is the number of subjects in the tile. The Muxer uses it for the cap and the track
  radius, and does not read the events for that.
- All other entity packets become events of the tile: equipment, animation, sound, passengers,
  damage. An entity packet outside the tile delta brings back the encode for each viewer.
- The tracker leaves the tick. `sync_entities`, `update_tracked_by` and their components go
  away. Subscriptions in the Muxer replace them.

## 7.2 Tiles and tile streams

A tile is a square of columns, 2 × 2 by default. Tile edges align with the minimum leaf of the
cell tree, so one cell owns each tile. The tile size is a parameter. A large tile sends more
subjects that the viewer does not need. A small tile gives more subscriptions.

A tile stream works as the version stream of a chunk (`cells.md`, section 6). It is a snapshot
and the deltas after it, merged by tick. A new subscriber gets the spawn bundles of the tile, then
its deltas.

Each entity has one global `EntityId`. The Coordinator gives each cell a range of ids. The wire
id of an entity comes from its `EntityId`. A player keeps the wire id of its session.

## 7.3 Encode once

The game supplies the codec:

```rust
pub trait ReplicationCodec: Send + 'static {
    type Event;
    fn encode(&mut self, tile: TileKey, tier: Tier, layer: Layer, events: &[Self::Event]) -> TileBlob;
    fn spawn_bundle(&mut self, id: EntityId) -> Bytes;
}

pub struct TileBlob { bytes: Bytes, ranges: SmallVec<[(EntityId, Range<u32>); 4]> }
```

- The Muxer keeps one delta base for each subject and tier. Thus all viewers of a tier receive
  the same relative moves.
- The Muxer sends a keyframe of the tile for a tier in three cases:
  - a relative move overflows (C6);
  - a subject enters the tile;
  - a slow fixed interval ends.
- `ranges` mark the bytes of each subject. A player receives the blob of its own tile as two
  slices around its own range. It never receives its own move.
- `bytes` holds framed packets. The Muxer keeps one buffer for each compression threshold in use.
  The writer sends the buffer as it is. This needs a new outbound part, `Framed`, next to
  `Shared` in `outbound.rs`.
- The Muxer compresses each chunk version one time. All connections send the same compressed
  frame.
- The Muxer caches the spawn bundle of a subject by subject and data version.

## 7.4 Interest: tiers, layers, the cap and the budget

For each viewer, the Muxer does these steps in each tick:

1. Sort the tiles by distance from the viewer.
2. Give each tile a tier by distance:

   | Tier | Distance | One move every | Updates per second |
   |---|---|---|---|
   | 0 | 32 blocks or less | 2 ticks | 10 |
   | 1 | 64 blocks or less | 4 ticks | 5 |
   | 2 | 128 blocks or less | 10 ticks | 2 |
   | 3 | track radius or less | 20 ticks | 1 |

3. Add the `count` of each tile, in order, until the sum reaches K. Give each tile after that
   point only layer 0, or tier 3.
4. When the cap binds inside tier 0, give lower tiers to tiles and layers by their order, not by
   distance. In a dense crowd, all K nearest subjects stand inside 32 blocks.

Rules for tiers and layers:

- Tiers 1 to 3 send paths of steps with tick offsets (C5). The tick of a tile for a tier is
  `hash(tile) mod n`, so the work spreads over the ticks.
- The rank inside a tile is the same for all viewers: players first, then by `EntityId`. Layer 0
  holds the first 32 ranks, layer 1 the next 64, layer 2 the rest.
- A tile enters a tier at the tier distance. It leaves at the tier distance plus 8 blocks.
- The cap admits a tile while the sum is K or less. It removes a tile when the sum is more than
  1.25 × K.
- Each viewer can have a maximum of 16 pinned subjects: its target, its attacker, its party, its
  vehicle. The Muxer encodes them for that viewer only. The cost is O(16 × N).

Rules for the budget:

- The Muxer measures the bytes of each viewer as a moving average. It also reads the state of the
  writer queue.
- Above 100% of the budget, or when the writer falls behind, the Muxer lowers one tile or layer
  by one tier.
- Below 70% of the budget for 2 seconds, the Muxer raises one tile or layer by one tier.
- The Muxer never drops one delta from a subscription, because a lost delta breaks the delta
  base. It changes only subscriptions.
- A change of tier sends the keyframe of the tile for the new tier. The Muxer caches keyframes by
  tile, tier and tick. All viewers that change in the same tick share one keyframe.
- A priority accumulator stops starvation. For each pair of viewer and lowered tile,
  `acc += weight × dt`. Spare budget sends a keyframe of the tile with the largest `acc`, and
  sets that `acc` to zero.

## 7.5 Profile entries and the player list

- The profile entries of a viewer are the players of its visible set, with `listed = false`, and
  a maximum of 80 listed players (C3, C4). A policy selects the listed players: party, nearest,
  operators.
- The Muxer sends the profile entry of a subject in the bundle of its spawn, before the spawn
  (C2, C8).
- The Muxer encodes a profile entry one time for each player and profile version.
- An entry stays in a cache with hysteresis after its subject leaves the visible set. Thus a
  subject at the edge of the cap does not send its skin data again and again.
- The Muxer sends latency only for listed entries, a maximum of one time in 60 seconds.
- A player that joins or leaves changes only the viewers whose set changes.
- The header or the footer of the list shows the number of players online.

The implementation today has these values and limits:

- The default policy lists the nearest players. `ListPolicy` holds a function, and the
  `list_policy` field of `ServerCorePlugin` sets it, so a party or an operator policy can replace it.
- The candidates of the policy are the players of the dimension of the viewer. The reference lists
  all players of the server. When a player changes dimension, the viewers of the old dimension
  remove its entry, and it removes their entries. This is `intentional_deviation`.
- The own entry of a viewer comes with its login. It counts as one of the 80 listed entries.
- A viewer checks its listed entries a maximum of one time in 20 ticks. It checks them only after
  a player joins, leaves or changes section in the dimension. Thus players that do not move cost
  no work and no bytes.
- An entry that is not listed and not in the visible set stays for 600 ticks. Then the muxer
  sends `player_info_remove`. A disconnect removes the entry at the end of the tick of the
  despawn.
- The profile entry is in the spawn bundle only when the client does not have the entry. An entry
  that the policy lists first goes before the change of the listing, outside a bundle.
- The host measures the latency from the keep-alive on the wire and sends it to the muxer. A
  viewer gets the latency of its listed entries a maximum of one time in 1,200 ticks, and only
  the values that changed. An entry that becomes listed also brings its latency.
- The host sends the header when the number changes. A player that a dimension attaches also
  gets it.
- All intervals count the ticks of the tile deltas of the dimension. No interval reads the wall
  clock.

## 7.6 Chat and other broadcasts

- Signed player chat needs the profile entry of the sender at each recipient. Each recipient
  also has its own message index. Thus a signed chat to all players costs O(N²) bytes.
- Global chat goes out as system chat. Local chat in a radius stays signed when the sender is in
  the visible set of the recipient. This is `intentional_deviation`.
- Messages about a player that joins or leaves go only to the players near that player.
- A broadcast with the same bytes for all players, such as the time, is encoded one time. Each
  edge gets one copy.

The implementation today has these limits:

- The server does not sign chat. All chat goes out as system chat, also local chat.
- The join message goes to the player and to the viewers that hold it at the end of the tick of
  its spawn. The leave message goes to the viewers that hold it. A change of dimension sends
  neither message.

## 7.7 Spawn bundles by distance

- Tiers 0 and 1 get the full spawn bundle: spawn, data, attributes, equipment, passengers.
- Tiers 2 and 3 get the spawn and the minimum data, about 70 bytes instead of about 450 bytes.
  The equipment follows when the subject enters tier 1.
- This decreases the cost of subjects that enter and leave at the edge of the cap by about 80%
  (estimate). The artifact: a far player shows without armour for a short time.

## 7.8 Edges

The Muxer runs as M processes, the edges. A load balancer gives each new connection to an edge.
The PROXY protocol v2 keeps the address of the client.

- Each edge subscribes to the union of the tiles that its viewers need. It uses a link with
  `Placement::Remote`.
- A cell sends each tile delta to each edge that subscribes: O(active tiles × M).
- An edge encodes O(subscribed tiles × tiers × layers) blobs. It copies
  O(local viewers × subscriptions) references.
- No term is N². The density hits the cap, not N.
- The Coordinator keeps the map from session to edge to owner cell. The edge sends the input of a
  player to the owner cell. The input applies at the sync point (`cells.md`, section 4).
- Random placement of connections makes each edge subscribe to almost all active tiles. This is
  acceptable up to about 32 edges. Above that, place connections by region or add relays.

Encryption runs on the edge, in the writer of each connection:

- CFB8 encrypts one block of AES for each byte, and each stream is serial. The throughput is
  50–300 MB/s for each core. This is an assumption. Measure it before step 9.
- At 500 MB/s of egress, the edges need about 2 to 10 cores for encryption. The reference
  traffic of 19.3 GB/s would need 64 to 400 cores.
- A possible gain: encrypt many connections at one time in wide AES instructions. Each stream
  stays serial, but the streams are independent. This is a hypothesis.

## 7.9 Density control

When 2,000 players or more stand in one place, the Muxer uses these controls in this order:

1. **Track radius from density.** `r = min(view distance, sqrt(K / (π × ρ)))`. The density `ρ`
   comes from the `count` of the tiles. The function reads only tile headers, so it is
   deterministic.
2. **The cap and the pinned subjects only.**
3. **Impostors.** This control is optional and is `intentional_deviation`. One cheap entity for
   each tile shows the number of hidden players, for example a text display "+1,870". Its id comes
   from a reserved range. The Muxer encodes it one time.
4. **Time dilation.** This is the last guard. A cell ticks slower in wall time, and bytes per
   second fall with the tick rate. The logical tick does not use wall time, so the world stays
   deterministic.

Limits of time dilation:

- A message from tick T applies at tick T + 1. Thus a connected group of cells runs at the speed
  of its slowest cell. Time dilation applies to the group, not to one cell.
- The Muxer tells the viewers of the group the new tick rate (C11).
- The movement checks of the server must count logical ticks, not wall time.

## 7.10 Determinism and tests

- Tile deltas are a pure output of the tick. The world hash of
  `crates/mcrs_minecraft_server/tests/it/tick_determinism.rs` includes them.
- For one cell against many cells, the tile deltas, sorted by tile key, must be equal.
- The Muxer gives the tick only player input and tickets. Entity interest is not a ticket.
- The planner of the Muxer is a pure function:
  `plan(&ViewerState, &TileHeaders, Measured) -> Vec<SubscriptionChange>`. Tests run it on
  recorded tile streams with a virtual clock.
- A client model decodes the bytes of a viewer with `mcrs_minecraft_client_world`. It checks
  these conditions:
  - the viewer never gets a spawn of its own id;
  - the profile entry of a player comes before its spawn;
  - the visible set is equal to the expected set;
  - no tile starves for more than a limit.

---

# 8. Steps

Each step leaves a working server and passes the determinism test. The second column shows where
the step goes in the migration of `cells.md`, section 12.

| Step | `cells.md` §12 | Work | Check |
|---|---|---|---|
| 1 | before step 2 | Add a crowd mode to the `tick_load` example and the `bot_load` bench, with bots together and apart. Count encode calls, shared appends, packets by kind and bytes for each session. | A baseline at 100, 300 and 1,000 bots. |
| 2 | before step 2 | Replace the two trackers with one tracker on a grid of sections, with hash sets and one radius. Send relative moves instead of absolute syncs. Send the profile entry before the spawn of a player. Send one remove packet with a list. | Work per tick grows with N × visible, not with the view distance. The client model sees the other players. |
| 3 | before step 2 | Share bytes on the bus. Add the `Framed` part. Compress each chunk version one time. Remove the deep clone for each recipient. Send one channel message for each drain. | Encode calls per tick are equal to distinct packets. One deflate for each chunk version. |
| 4 | before step 2 | The player list (section 7.5) and chat (section 7.6). | 1,000 bots that do not move cost about 0 bytes for each viewer. |
| 5 | with step 2 | Add `mcrs_engine_muxer` (tiles, subscriptions, fan out, a `Stage` on a `Link`). Add `mcrs_minecraft_replication` (the codec, delta bases, spawn bundles). The dimension publishes tile deltas. The tracker leaves the tick. | Encode calls per tick are about active tiles × tiers. Bytes for each viewer against step 3. |
| 6 | after step 5 here, before step 3 | Tiers, layers, the cap, the budget, the accumulator, spawn bundles by distance. | 2,000 bots in one place: bytes for each viewer stay in the budget. No tile starves for more than 2 seconds. The frame rate of the vanilla client at 200, 500 and 1,000 visible players. |
| 7 | before step 4 | A global `EntityId` from the Coordinator. | A subject that changes its owner cell causes no despawn and spawn on the client. |
| 8 | before step 4 | A parallel flush: groups of connections on many threads. | CPU time for each 1,000 viewers. |
| 9 | after step 6 | M edges through `Placement::Remote`. Encryption on the edges. | The egress of a cell is the sum of tiles × M. 10,000 bots at M = 8. |
| 10 | after step 9 here | Density control (section 7.9). | Tick time and bytes with 2,000 players in one tile. |

---

# 9. Open questions

- C1 to C12 need rules in `docs/spec/`, from a specification author.
- The sharding document, section 2, needs a row for entity replication with the policy
  `semantic`.
- Keyframe storm: many viewers change their tier in the same tick.
- The encode can run on each edge (M encodes for each tile delta, and the cell stays free of the
  protocol) or in a stage beside the cell (one encode). Start on the edge and measure.
- The tile size.
- The default K and the default budget. Measure the frame rate of the vanilla client first.
- The encryption throughput for each core, and the gain of many streams in wide instructions.
- The wire id of a player wraps after 2³¹ sessions. The wire id of an entity that is not a player
  wraps after 2³¹ granted ids, because a range never returns to the Coordinator.

---

# 10. Sources

- EVE Online, serialization for each group of ships: https://www.eveonline.com/news/view/fixing-lag-drakes-of-destiny-part-2-1
- EVE Online, time dilation: https://wiki.eveuniversity.org/Time_dilation
- EVE Online, 6,739 pilots in one system: https://www.eveonline.com/news/view/the-second-timer-in-m2-xfe
- Improbable, tiers for 10,000 players: https://ims.improbable.io/insights/intimacy-at-scale-building-an-architecture-for-density/
- Unreal Engine, Replication Graph: https://dev.epicgames.com/documentation/en-us/unreal-engine/replication-graph-in-unreal-engine
- Unreal Engine, Iris prioritization: https://dev.epicgames.com/documentation/en-us/unreal-engine/iris-prioritization-in-unreal-engine
- Glenn Fiedler, state synchronization: https://gafferongames.com/post/state_synchronization/
- Hyperion, proxy architecture: https://github.com/hyperion-mc/hyperion/blob/85abe336514e507b72a38e351b5df5a641a4bf30/docs/architecture/proxy.md
- Minestom, shared packet buffers: https://github.com/Minestom/Minestom/blob/704be783b63cfb0666f38f527f90f5d118cffae3/src/main/java/net/minestom/server/utils/PacketViewableUtils.java
- Folia, test with 1,000 players: https://cubxity.dev/blog/folia-test-june-2023
- Second Life, client cost of many avatars: https://community.secondlife.com/forums/topic/500570-the-era-of-150-avatars-in-a-region-is-here-we-have-a-performance-problem-viewer-side/
- Protocol packet formats: https://minecraft.wiki/w/Java_Edition_protocol/Packets
- Protocol encryption: https://minecraft.wiki/w/Java_Edition_protocol/Encryption
