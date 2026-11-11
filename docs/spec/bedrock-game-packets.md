# Bedrock game packets: behavioural specification

| Field | Value |
|-------|-------|
| Edition | Bedrock |
| Protocol version | `729` |
| Game version | `1.21.30` (see `bedrock-wire.md`, Q1) |

This document states the first game packets of Bedrock Edition at protocol 729: the start of
the world, chat text, the player movement in both directions, the time, one block change and
the chunk column. It also states the order of the packets from the end of the handshake to the
spawn of the player.

This document does not state these items. A later specification states each one:

- the layout of the sub-chunk data and of the biome data in the chunk column payload;
- the block palette and the runtime ids of the block states;
- the item table and the user data of an item;
- the meaning of each item stack request and the answer of the server;
- the movement correction of the server (server authoritative movement).

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **vanilla client**, **packet**, **payload**,
  **field**, **decode error**: as `bedrock-wire.md`, section 0.1, defines them.
- **Clientbound**, **serverbound**, **experiment**: as `bedrock-handshake.md`, section 0.1,
  defines them.
- **Unique id**: the `varint64` that names an entity for the life of the world. It does not
  change when the entity unloads and loads again.
- **Runtime id**: the `varuint64` that names an entity for the life of the connection.
- **Block runtime id**: the `varuint32` that names a block state for the life of the connection.
  The block palette (a later specification) gives the values.
- **Client tick**: the count of simulation frames of the client. The client increments it by 1
  for each `PlayerAuthInput`.
- **Eye position**: the feet position of the player plus 1.62 on the y axis.
- **Chunk column**: the blocks of a 16 × 16 column of the world, from the bottom to the top of
  the dimension.
- **Sub-chunk**: a 16 × 16 × 16 part of a chunk column. Sub-chunk index 0 is the lowest one.
- **Blob**: a part of a chunk column that the client keeps in its cache. A 64-bit hash names it.
- **Input flags**: the `varuint64` bit set of `PlayerAuthInput` (BEG-PAI-002).

### 0.2 Notation and provenance

The notation, the wire types, the provenance labels, the sources and the experiment set-up are
as `bedrock-wire.md`, sections 0.2 to 0.4, defines them. Each rule has `Edition: bedrock`. A
field table lists the fields in wire order. The header (`bedrock-wire.md`, BEW-HDR-001) comes
before the first field and is not in the table. A field with a condition is on the wire only
when the condition is true.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `BEG-PKT` | packet ids and directions |
| `BEG-SG` | `StartGame` |
| `BEG-TXT` | `Text` |
| `BEG-MOVE` | `MovePlayer` |
| `BEG-PAI` | `PlayerAuthInput` |
| `BEG-TIME` | `SetTime` |
| `BEG-BLK` | `UpdateBlock` |
| `BEG-CHUNK` | the chunk column packet |
| `BEG-SEQ` | the order from the handshake to the spawn |

## 1. Packets

### BEG-PKT-001: Packet ids

| Packet | Id | Direction |
|--------|----|-----------|
| `Text` | `0x09` | both |
| `SetTime` | `0x0a` | clientbound |
| `StartGame` | `0x0b` | clientbound |
| `MovePlayer` | `0x13` | both (BEG-MOVE-002) |
| `UpdateBlock` | `0x15` | clientbound |
| chunk column packet | `0x3a` | clientbound |
| `PlayerAuthInput` | `0x90` | serverbound |

- **Behavior:** the header of `PlayerAuthInput` is the two bytes `90 01`.
  `bedrock-wire.md`, BEW-HDR-001, gives the reason.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

## 2. `StartGame`

### BEG-SG-001: Fields of the player

| Field | Wire type | Notes |
|-------|-----------|-------|
| entity unique id | `varint64` | the unique id of the player |
| entity runtime id | `varuint64` | the runtime id of the player |
| player game type | `varint32` | 0 survival, 1 creative, 2 adventure, 5 default, 6 spectator |
| position | `vec3` | the eye position of the player |
| rotation | `vec2` | pitch, then yaw, in degrees |
| level settings | the fields of BEG-SG-002 | |
| level id | `string` | an opaque id of the world |
| level name | `string` | the name that the pause screen shows |
| template content identity | `string` | empty when the world is not from a template |
| trial | `bool` | |
| movement settings | the fields of BEG-SG-004 | |
| current level time | `i64le` | the time of day in ticks |
| enchantment seed | `varint32` | |
| block properties | `list<varuint32, T>` | T is BEG-SG-005 |
| item table | `list<varuint32, T>` | T is BEG-SG-006 |
| multiplayer correlation id | `string` | |
| item stack net manager | `bool` | true: the client sends item stack requests |
| server version | `string` | a free text, for example `mcrs` |
| player property data | `nbt` | a compound; an empty compound when the server has no player properties |
| block registry checksum | `u64le` | BEG-SG-007 |
| world template id | `uuid` | the zero UUID when the world is not from a template |
| client side generation | `bool` | |
| block network ids are hashes | `bool` | BEG-SG-007 |
| server authoritative sound | `bool` | |

- **Behavior:**
  - Each `nbt` field of `StartGame` is in the network form of `bedrock-wire.md`, BEW-NBT-001.
  - Game type 5 tells the client to use the world game type of BEG-SG-002.
- **Fixtures:** the constructed packet of 187 bytes below has these values:
  unique id 1, runtime id 1, survival, position (0.5, 65.62, 0.5), rotation (0, 0), seed 0,
  default biome, overworld, generator 1, survival world, easy, world spawn (0, 64, 0),
  multiplayer and LAN broadcast on, Xbox and platform broadcast 4, commands on, one game rule
  `dodaylightcycle` = true, no experiment, member permission, chunk tick range 4, base game
  version `*`, new nether on, level name `world`, movement authority 1, time 0, empty block and
  item lists, item stack net manager on, server version `mcrs`, empty player property data,
  checksum 0, zero template id, and the three last flags false. *(constructed)*

```
0b 02 01 00 00 00 00 3f 71 3d 83 42 00 00 00 3f
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 02 00 00 02 00 40 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 01 01 08 08
01 00 01 0f 64 6f 64 61 79 6c 69 67 68 74 63 79
63 6c 65 00 01 01 00 00 00 00 00 00 00 02 04 00
00 00 00 00 00 00 00 00 00 00 00 00 01 2a 00 00
00 00 00 00 00 00 01 00 00 00 00 00 00 00 00 00
05 77 6f 72 6c 64 00 00 02 00 00 00 00 00 00 00
00 00 00 00 00 00 00 01 04 6d 63 72 73 0a 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00
```

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high (order and types),
medium (meaning of the flags). Edition: bedrock. Experiment: capture the `StartGame` packet of
BDS 1.21.30 with a new world and the default `server.properties`. Decode it with this table. The
decoder must use all bytes. Record each value.

### BEG-SG-002: Level settings

| Field | Wire type | Notes |
|-------|-----------|-------|
| seed | `u64le` | |
| spawn biome type | `i16le` | 0 default, 1 user defined |
| user defined biome name | `string` | empty when the type is 0 |
| dimension | `varint32` | 0 overworld, 1 nether, 2 end |
| generator | `varint32` | 0 legacy, 1 overworld, 2 flat, 3 nether, 4 end, 5 void |
| world game type | `varint32` | the values of the player game type, without 5 |
| hardcore | `bool` | |
| difficulty | `varint32` | 0 peaceful, 1 easy, 2 normal, 3 hard |
| world spawn | `blockpos` | |
| achievements disabled | `bool` | |
| editor world type | `varint32` | 0 not an editor world, 1 project, 2 test level, 3 realms upload |
| created in editor | `bool` | |
| exported from editor | `bool` | |
| day cycle stop time | `varint32` | |
| education offer | `varint32` | 0 none, 1 rest of world, 2 China |
| education features enabled | `bool` | |
| education product id | `string` | |
| rain level | `f32le` | 0.0 to 1.0 |
| lightning level | `f32le` | 0.0 to 1.0 |
| confirmed platform locked content | `bool` | |
| multiplayer game intent | `bool` | |
| LAN broadcast intent | `bool` | |
| Xbox Live broadcast mode | `varint32` | 0 none, 1 invite only, 2 friends only, 3 friends of friends, 4 public |
| platform broadcast mode | `varint32` | the values of the Xbox Live broadcast mode |
| commands enabled | `bool` | |
| texture packs required | `bool` | |
| game rules | `list<varuint32, T>` | T is BEG-SG-003 |
| experiments | `list<u32le, T>` | T is a `string` name and a `bool` enabled |
| experiments previously toggled | `bool` | |
| bonus chest | `bool` | |
| start with map | `bool` | |
| player permission | `varint32` | 0 visitor, 1 member, 2 operator, 3 custom |
| server chunk tick range | `i32le` | in chunks |
| locked behaviour pack | `bool` | |
| locked resource pack | `bool` | |
| from locked template | `bool` | |
| only Microsoft account gamertags | `bool` | |
| from world template | `bool` | |
| world template option locked | `bool` | |
| only old villagers | `bool` | |
| persona disabled | `bool` | |
| custom skins disabled | `bool` | |
| emote chat muted | `bool` | |
| base game version | `string` | `*` for the newest version |
| limited world width | `i32le` | 0 for an infinite world |
| limited world depth | `i32le` | 0 for an infinite world |
| new nether | `bool` | |
| education shared resource button name | `string` | |
| education shared resource link | `string` | |
| force experimental gameplay | `optional<bool>` | |
| chat restriction level | `u8` | 0 none, 1 dropped, 2 disabled |
| player interactions disabled | `bool` | |
| server id | `string` | |
| world id | `string` | |
| scenario id | `string` | |

- **Edge cases:**
  - A value outside a value set of this table is a decode error.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high (order and types),
medium (value sets of the editor world type and the education offer). Edition: bedrock.
Experiment: the experiment of BEG-SG-001.

### BEG-SG-003: Game rules in `StartGame`

| Field | Wire type | Condition | Notes |
|-------|-----------|-----------|-------|
| name | `string` | | lower case, for example `dodaylightcycle` |
| editable | `bool` | | true: the player can change the rule in the settings screen |
| type | `varuint32` | | 1 boolean, 2 integer, 3 float |
| value | `bool` | type is 1 | |
| value | `varuint32` | type is 2 | |
| value | `f32le` | type is 3 | |

- **Behavior:**
  - The name of a game rule on the Bedrock wire is in lower case.
  - The server sends the rules that differ from the default of the client. It can also send
    all rules.
- **Edge cases:**
  - Another type is a decode error.
  - A negative integer value is not possible on this wire. BEG-SG-003 has open question Q1.
- **Fixtures:** `01 0f 64 6f 64 61 79 6c 69 67 68 74 63 79 63 6c 65 00 01 01` is a list of one
  rule: `dodaylightcycle`, not editable, boolean true. *(constructed)*

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high (layout), medium
(integer as `varuint32`). Edition: bedrock. Experiment: set `randomtickspeed` to 5 in BDS,
capture `StartGame`, and record the bytes of the rule.

### BEG-SG-004: Movement settings

| Field | Wire type | Notes |
|-------|-----------|-------|
| movement authority | `varint32` | 0 client, 1 server, 2 server with rewind |
| rewind history size | `varint32` | in client ticks; used only with authority 2 |
| server authoritative block breaking | `bool` | |

- **Behavior:**
  - With authority 0, the client sends `MovePlayer` and does not send `PlayerAuthInput`.
  - With authority 1 or 2, the client sends `PlayerAuthInput` once for each client tick and does
    not send `MovePlayer`.
  - The server sends authority 1, rewind history size 0 and server authoritative block breaking
    false.

Provenance: `documentation`, `source_suggested` (fields); `design` (values). Kind: wire,
semantic. Confidence: high (fields), medium (the packet that each authority selects). Edition:
bedrock. Reason for the values: `PlayerAuthInput` carries the client tick that `MovePlayer`
returns (BEG-MOVE-001). Rewind and block breaking authority need the correction of the server,
which a later specification states. Experiment: send `StartGame` with authority 0, then with
authority 1. Record the movement packet that the client sends in each case.

### BEG-SG-005: Block properties

| Field | Wire type | Notes |
|-------|-----------|-------|
| name | `string` | the namespaced name of a custom block |
| properties | `nbt` | a compound; the network form |

- **Behavior:** the server sends an empty list when it has no custom block.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEG-SG-006: Item table

| Field | Wire type | Notes |
|-------|-----------|-------|
| name | `string` | the namespaced name of the item, for example `minecraft:stick` |
| network id | `i16le` | the id that the item fields of other packets carry |
| component based | `bool` | true for a custom item with components |

- **Behavior:**
  - The table gives the item network ids for the connection.
  - The table holds each item that the client knows, and each custom item.
- **Edge cases:**
  - An item that is not in the table shows as an unknown item in the client. Open question Q2
    states the source of the vanilla table.

Provenance: `documentation`, `source_suggested`. Kind: wire, semantic. Confidence: high (layout),
medium (a missing item). Edition: bedrock. Experiment: capture the item table of BDS 1.21.30.
Record the count and the ids.

### BEG-SG-007: Block registry checksum and hashed ids

- **Behavior:**
  - The server writes 0 in the block registry checksum. The client then does not compare the
    block registry.
  - The server writes false in "block network ids are hashes". The block runtime ids are then
    the indices of the block palette (a later specification).

Provenance: `source_suggested` (checksum 0), `design` (hashes false). Kind: semantic.
Confidence: medium. Edition: bedrock. Reason: the palette order is the subject of a later
specification, and an index is the smaller value on the wire. Experiment: send `StartGame` with
checksum 0 to a vanilla client, and confirm that the client spawns.

## 3. `Text`

### BEG-TXT-001: Fields

| Field | Wire type | Condition | Notes |
|-------|-----------|-----------|-------|
| type | `u8` | | BEG-TXT-002 |
| needs translation | `bool` | | true: the client translates the message and the parameters |
| source name | `string` | type is 1, 7 or 8 | the name of the sender |
| message | `string` | | |
| parameters | `list<varuint32, string>` | type is 2, 3 or 4 | the arguments of the translation key |
| sender XUID | `string` | | the Xbox user id; empty for the server |
| platform chat id | `string` | | empty except on some consoles |
| filtered message | `string` | | BEG-TXT-003 |

- **Fixtures:**
  - `09 00 00 02 68 69 00 00 00` is a raw message `hi`. *(constructed)*
  - `09 01 00 05 53 74 65 76 65 02 68 69 00 00 00` is a chat message `hi` from `Steve`.
    *(constructed)*
  - `09 02 01 02 25 61 01 01 78 00 00 00` is a translation of the key `%a` with one parameter
    `x`. *(constructed)*

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.
Experiment: type `hi` in the chat of a vanilla client connected to BDS 1.21.30. Capture the
serverbound and the clientbound `Text`. Compare them with the second fixture.

### BEG-TXT-002: Types

| Type | Meaning | Fields after "needs translation" |
|------|---------|-----------------------------------|
| 0 | raw text | message |
| 1 | chat | source name, message |
| 2 | translation | message, parameters |
| 3 | pop-up | message, parameters |
| 4 | jukebox pop-up | message, parameters |
| 5 | tip | message |
| 6 | system | message |
| 7 | whisper | source name, message |
| 8 | announcement | source name, message |
| 9 | JSON whisper | message |
| 10 | JSON | message |
| 11 | JSON announcement | message |

- **Behavior:**
  - For types 9 to 11, the message is a JSON text object with the key `rawtext`.
  - A client sends only type 1.
- **Edge cases:**
  - A type above 11 is a decode error.
  - A serverbound `Text` of another type than 1 is a decode error in the server.

Provenance: `documentation`, `source_suggested`. Kind: wire, semantic. Confidence: high (layout),
medium (client sends only type 1). Edition: bedrock.

### BEG-TXT-003: Filtered message

- **Behavior:**
  - The filtered message is the message after the text filter of the server.
  - The server writes an empty filtered message. The client then shows the message.
  - The server ignores the filtered message of a serverbound `Text`.

Provenance: `design`. Kind: semantic. Confidence: medium. Edition: bedrock. Reason: the server
has no text filter. Experiment: send a `Text` with a message and a different filtered message,
with the profanity filter of the client on, then off. Record the text that the client shows.

### BEG-TXT-004: Chat from the client

- **Behavior:**
  - The server takes the message of a serverbound chat `Text` as the chat input of the player.
  - The server ignores the source name and the sender XUID of the packet. It uses the name and
    the XUID of the connection.
  - The server sends the chat to each player as `Text` type 1 with the name of the sender.

Provenance: `design`. Kind: semantic. Confidence: high. Edition: bedrock. Reason: a client can
write any name in the source field.

## 4. `MovePlayer`

### BEG-MOVE-001: Fields

| Field | Wire type | Condition | Notes |
|-------|-----------|-----------|-------|
| runtime id | `varuint64` | | the runtime id of the player that moves |
| position | `vec3` | | the eye position |
| pitch | `f32le` | | degrees |
| yaw | `f32le` | | degrees |
| head yaw | `f32le` | | degrees |
| mode | `u8` | | 0 normal, 1 reset, 2 teleport, 3 head rotation only |
| on ground | `bool` | | |
| ridden runtime id | `varuint64` | | 0 when the player rides nothing |
| teleport cause | `i32le` | mode is 2 | 0 unknown, 1 projectile, 2 chorus fruit, 3 command, 4 behaviour |
| teleport source entity type | `i32le` | mode is 2 | the entity type id of the source; 0 for none |
| tick | `varuint64` | | BEG-MOVE-003 |

- **Fixtures:**
  - `13 01 00 00 00 3f 71 3d 83 42 00 00 00 3f 00 00 00 00 00 00 b4 42 00 00 b4 42 00 01 00 64`
    is runtime id 1 at (0.5, 65.62, 0.5), pitch 0, yaw 90, head yaw 90, mode normal, on ground,
    tick 100. *(constructed)*
  - `13 01 00 00 00 00 00 00 80 42 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00 00 03 00 00 00 00 00 00 00 00`
    is runtime id 1, a teleport to (0, 64, 0) by a command, tick 0. *(constructed)*

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.
Experiment: run `/tp @s 0 64 0` on BDS 1.21.30 and capture the clientbound `MovePlayer`. Compare
it with the second fixture.

### BEG-MOVE-002: Direction

- **Behavior:**
  - The server sends `MovePlayer` to move the player of the client, and to show the movement of
    other players.
  - With movement authority 0, the client sends `MovePlayer` for its own movement.
  - With movement authority 1 or 2, a serverbound `MovePlayer` is an unexpected packet. The
    server ignores it.
  - Mode 2 and mode 1 move the player of the client to the position. Mode 0 is the movement of
    another player.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Edition: bedrock.

### BEG-MOVE-003: Tick

- **Behavior:**
  - In a clientbound `MovePlayer` for the player of the client, the tick is the client tick of
    the last `PlayerAuthInput` that the server processed.
  - When the server processed no `PlayerAuthInput`, the tick is 0.
  - For another player, the tick is 0.
  - The client uses the tick to replay its inputs after the position. A wrong tick causes the
    client to jump back.

Provenance: `documentation`, `source_suggested`. Kind: semantic. Confidence: medium. Edition:
bedrock. Experiment: send a teleport with the tick of the last `PlayerAuthInput`, then with a
tick 20 ticks older, while the player walks. Record the position of the client after each.

## 5. `PlayerAuthInput`

### BEG-PAI-001: Fields

| Field | Wire type | Condition | Notes |
|-------|-----------|-----------|-------|
| pitch | `f32le` | | degrees |
| yaw | `f32le` | | degrees |
| position | `vec3` | | the eye position |
| move vector | `vec2` | | x is the side input, y is the forward input; each from -1 to 1 |
| head yaw | `f32le` | | degrees |
| input flags | `varuint64` | | BEG-PAI-002 |
| input mode | `varuint32` | | 1 mouse, 2 touch, 3 gamepad, 4 motion controller |
| play mode | `varuint32` | | BEG-PAI-003 |
| interaction model | `varuint32` | | 0 touch, 1 crosshair, 2 classic |
| gaze direction | `vec3` | play mode is 4 | |
| client tick | `varuint64` | | |
| position delta | `vec3` | | the velocity of the client |
| item use transaction | BEG-PAI-004 | input flag 34 | |
| item stack request | BEG-PAI-006 | input flag 36 | |
| block actions | BEG-PAI-007 | input flag 35 | |
| vehicle rotation | `vec2` | input flag 45 | |
| predicted vehicle unique id | `varint64` | input flag 45 | |
| analog move vector | `vec2` | | |

- **Behavior:**
  - The conditional parts come in the order of the table. The block actions come after the item
    stack request, not in the order of the flag bits.
- **Fixtures:**
  - The bytes below are pitch 0, yaw 90, position (0.5, 65.62, 0.5), move vector (0, 1), head
    yaw 90, input flag 10 (up), mouse, normal play mode, crosshair, client tick 5, delta 0 and
    analog move vector (0, 1). *(constructed)*

    ```
    90 01 00 00 00 00 00 00 b4 42 00 00 00 3f 71 3d
    83 42 00 00 00 3f 00 00 00 00 00 00 80 3f 00 00
    b4 42 80 08 01 00 01 05 00 00 00 00 00 00 00 00
    00 00 00 00 00 00 00 00 00 00 80 3f
    ```

  - The bytes below are all angles, positions and vectors 0, input flag 35, mouse, normal play
    mode, crosshair, client tick 6, and one block action: start destroy at (1, 64, -1) on face 1.
    *(constructed)*

    ```
    90 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00
    00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
    00 00 80 80 80 80 80 01 01 00 01 06 00 00 00 00
    00 00 00 00 00 00 00 00 02 00 02 80 01 01 02 00
    00 00 00 00 00 00 00
    ```

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.
Experiment: capture the `PlayerAuthInput` packets of a vanilla client that walks forward, then
breaks a block, then places a block, then moves an item in the inventory. Decode each packet
with this section. The decoder must use all bytes.

### BEG-PAI-002: Input flags

| Bit | Meaning | Bit | Meaning |
|-----|---------|-----|---------|
| 0 | ascend | 27 | start sneaking |
| 1 | descend | 28 | stop sneaking |
| 2 | north jump (no longer used) | 29 | start swimming |
| 3 | jump key down | 30 | stop swimming |
| 4 | sprint key down | 31 | start jumping |
| 5 | change height | 32 | start gliding |
| 6 | jumping | 33 | stop gliding |
| 7 | auto jumping in water | 34 | perform item interaction |
| 8 | sneaking | 35 | perform block actions |
| 9 | sneak key down | 36 | perform item stack request |
| 10 | up | 37 | handled teleport |
| 11 | down | 38 | emoting |
| 12 | left | 39 | missed swing |
| 13 | right | 40 | start crawling |
| 14 | up and left | 41 | stop crawling |
| 15 | up and right | 42 | start flying |
| 16 | want up | 43 | stop flying |
| 17 | want down | 44 | client acknowledged server data |
| 18 | want down slow | 45 | in client predicted vehicle |
| 19 | want up slow | 46 | paddling left |
| 20 | sprinting | 47 | paddling right |
| 21 | ascend block | 48 | block breaking delay enabled |
| 22 | descend block | 49 | horizontal collision |
| 23 | sneak toggle down | 50 | vertical collision |
| 24 | persist sneak | 51 | down and left |
| 25 | start sprinting | 52 | down and right |
| 26 | stop sprinting | | |

- **Behavior:**
  - Bit 34, bit 35, bit 36 and bit 45 add a part to the packet (BEG-PAI-001).
  - The other bits add no field.
  - The client sets bit 37 in the first `PlayerAuthInput` after it applies a teleport of the
    server.
- **Edge cases:**
  - A bit above 52 is not a decode error. The server ignores it.

Provenance: `documentation`, `source_suggested`; `design` (a bit above 52). Kind: wire.
Confidence: high (bits), medium (bit 37). Edition: bedrock.

### BEG-PAI-003: Play mode

| Value | Meaning |
|-------|---------|
| 0 | normal |
| 1 | teaser |
| 2 | screen |
| 3 | viewer |
| 4 | reality (virtual reality) |
| 5 | placement |
| 6 | living room |
| 7 | exit level |
| 8 | exit level living room |

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEG-PAI-004: Item use transaction

| Field | Wire type | Condition | Notes |
|-------|-----------|-----------|-------|
| legacy request id | `varint32` | | |
| legacy slots | `list<varuint32, T>` | legacy request id is not 0 | T is a `u8` container id and a `bytes` of slot indices |
| actions | `list<varuint32, T>` | | T is BEG-PAI-005 |
| action type | `varuint32` | | 0 click block, 1 click air, 2 break block |
| trigger type | `varuint32` | | 0 unknown, 1 player input, 2 simulation tick |
| block position | `blockpos` | | |
| block face | `varint32` | | 0 down, 1 up, 2 north, 3 south, 4 west, 5 east |
| hotbar slot | `varint32` | | |
| held item | item with net id (BEG-PAI-008) | | |
| player position | `vec3` | | |
| click position | `vec3` | | relative to the block |
| block runtime id | `varuint32` | | |
| client interact prediction | `varuint32` | | 0 failure, 1 success |

- **Behavior:**
  - The trigger type is 1 when the input starts in this tick. It is 2 when the player holds the
    input from an earlier tick.
- **Edge cases:**
  - Open question Q3 states a conflict about the condition of the legacy slots.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high (layout), medium
(condition of the legacy slots). Edition: bedrock. Experiment: place a block with a vanilla
client and capture the packet. Record the legacy request id and the bytes after it.

### BEG-PAI-005: Inventory action

| Field | Wire type | Condition | Notes |
|-------|-----------|-----------|-------|
| source type | `varuint32` | | 0 container, 1 global, 2 world interaction, 3 creative, 99999 untracked interaction |
| container id | `varint32` | source type is 0 or 99999 | |
| flags | `varuint32` | source type is 2 | |
| slot | `varuint32` | | |
| old item | item with net id (BEG-PAI-008) | | |
| new item | item with net id (BEG-PAI-008) | | |

- **Edge cases:** another source type is a decode error.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high (types 0 to 3),
medium (type 99999). Edition: bedrock.

### BEG-PAI-006: Item stack request

| Field | Wire type | Notes |
|-------|-----------|-------|
| request id | `varint32` | |
| actions | `list<varuint32, T>` | T is a `u8` action type and the body of the table below |
| strings to filter | `list<varuint32, string>` | for example the new name of an item in an anvil |
| filter origin | `i32le` | |

A slot reference has this layout:

| Field | Wire type | Notes |
|-------|-----------|-------|
| container | `u8` | |
| dynamic container id | `optional<u32le>` | |
| slot | `u8` | |
| stack network id | `varint32` | |

| Type | Action | Body |
|------|--------|------|
| 0 | take | count `u8`, source slot reference, destination slot reference |
| 1 | place | count `u8`, source slot reference, destination slot reference |
| 2 | swap | source slot reference, destination slot reference |
| 3 | drop | count `u8`, source slot reference, randomly `bool` |
| 4 | destroy | count `u8`, source slot reference |
| 5 | consume | count `u8`, source slot reference |
| 6 | create | result slot `u8` |
| 7 | place in item container | count `u8`, source slot reference, destination slot reference |
| 8 | take from item container | count `u8`, source slot reference, destination slot reference |
| 9 | lab table combine | no body |
| 10 | beacon payment | primary effect `varint32`, secondary effect `varint32` |
| 11 | mine block | hotbar slot `varint32`, predicted durability `varint32`, stack network id `varint32` |
| 12 | craft recipe | recipe network id `varuint32`, number of crafts `u8` |
| 13 | auto craft recipe | recipe network id `varuint32`, number of crafts `u8`, times crafted `u8`, ingredients (below) |
| 14 | craft creative | creative item network id `varuint32`, number of crafts `u8` |
| 15 | craft recipe optional | recipe network id `varuint32`, filter string index `i32le` (Q4) |
| 16 | craft grindstone | recipe network id `varuint32`, number of crafts `u8`, repair cost `varint32` |
| 17 | craft loom | pattern id `string`, times crafted `u8` (Q4) |
| 18 | craft non-implemented | no body |
| 19 | craft results (no longer used) | `list<varuint32, item without net id>`, times crafted `u8` |

The ingredients of type 13 are a `u8` count, then for each ingredient an item descriptor and a
`varint32` count (Q4). An item descriptor is a `u8` descriptor type and a body:

| Descriptor type | Body |
|-----------------|------|
| 0 | none |
| 1 | item network id `i16le`; then metadata `i16le` when the id is not 0 |
| 2 | molang expression `string`, molang version `u8` |
| 3 | item tag `string` |
| 4 | full name `string`, metadata `i16le` |
| 5 | complex alias name `string` |

- **Behavior:**
  - The client sends item stack requests in this part only when "item stack net manager" of
    `StartGame` is true.
  - The answer of the server is the subject of a later specification.
- **Edge cases:**
  - An unknown action type is a decode error. The rest of the packet cannot be decoded.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high (types 0 to 12 and
14), low (bodies of types 13, 15, 17 and 19). Edition: bedrock. Experiment: in a vanilla client
connected to BDS 1.21.30, use the recipe book (type 13), a loom (type 17), a grindstone (type
16) and an anvil with a new name (type 15). Capture each packet and decode the bodies.

### BEG-PAI-007: Block actions

| Field | Wire type | Condition | Notes |
|-------|-----------|-----------|-------|
| count | `varint32` | | the number of actions |
| action | `varint32` | | for each action; the table below |
| position | `sblockpos` | action is 0, 1, 18, 26 or 27 | |
| face | `varint32` | action is 0, 1, 18, 26 or 27 | |

| Action | Meaning |
|--------|---------|
| 0 | start destroy block |
| 1 | abort destroy block |
| 2 | stop destroy block |
| 18 | crack block |
| 26 | predict destroy block |
| 27 | continue destroy block |

- **Behavior:**
  - The count is a signed `varint32`, not a `varuint32` (`bedrock-wire.md`, BEW-STR-003).
  - Other action values are possible. They carry no position and no face.
- **Edge cases:**
  - A negative count is a decode error.
  - Open question Q5 states a conflict about action 2.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high (actions 0, 1, 18,
26, 27), low (action 2). Edition: bedrock. Experiment: break a block in survival with a vanilla
client and capture the packets until the block breaks. Record whether a position follows
action 2.

### BEG-PAI-008: Item with net id

| Field | Wire type | Condition | Notes |
|-------|-----------|-----------|-------|
| network id | `varint32` | | 0 is the empty item; no field follows |
| count | `u16le` | id is not 0 | |
| metadata | `varuint32` | id is not 0 | |
| has stack network id | `bool` | id is not 0 | |
| stack network id | `varint32` | has stack network id is true | |
| block runtime id | `varint32` | id is not 0 | 0 for an item that is not a block |
| user data | `bytes` | id is not 0 | opaque in this document |

An item without net id has the same layout without "has stack network id" and "stack network
id".

- **Fixtures:** `00` is the empty item.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

## 6. `SetTime`

### BEG-TIME-001: Fields

| Field | Wire type | Notes |
|-------|-----------|-------|
| time | `varint32` | the time of day in ticks |

- **Behavior:**
  - The client sets its time of day to the value.
  - When the time of the world is outside the range of `i32`, the server sends the time modulo
    192000. This keeps the time of day (period 24000) and the moon phase (period 8 days).
- **Fixtures:** `0a e0 5d` is time 6000. *(constructed)*

Provenance: `documentation`, `source_suggested` (field); `design` (modulo). Kind: wire, semantic.
Confidence: high (field). Edition: bedrock. Reason for the modulo: a cast of a large value to
`i32` gives a wrong time of day. Experiment: send time 6000 and record the position of the sun
in the client.

## 7. `UpdateBlock`

### BEG-BLK-001: Fields

| Field | Wire type | Notes |
|-------|-----------|-------|
| position | `blockpos` | |
| block runtime id | `varuint32` | |
| flags | `varuint32` | bit 0 neighbours, bit 1 network, bit 2 no graphics, bit 3 priority |
| layer | `varuint32` | 0 the normal layer, 1 the liquid layer |

- **Behavior:**
  - The client sets the block at the position and the layer to the block state.
  - The server writes flags 3 (neighbours and network).
- **Fixtures:** `15 02 40 01 2a 03 00` is the block runtime id 42 at (1, 64, -1), flags 3,
  layer 0. *(constructed)*

Provenance: `documentation`, `source_suggested` (fields and flags 3); `design` (the server always
writes 3). Kind: wire, semantic. Confidence: high (fields), medium (flags). Edition: bedrock.
Experiment: place a block in BDS 1.21.30 and capture `UpdateBlock`. Record the flags.

## 8. The chunk column packet

### BEG-CHUNK-001: Fields

| Field | Wire type | Condition | Notes |
|-------|-----------|-----------|-------|
| chunk x | `varint32` | | |
| chunk z | `varint32` | | |
| dimension | `varint32` | | 0 overworld, 1 nether, 2 end |
| sub-chunk count | `varuint32` | | BEG-CHUNK-002 |
| highest sub-chunk | `u16le` | sub-chunk count is `0xfffffffe` | |
| cache enabled | `bool` | | BEG-CHUNK-003 |
| blob ids | `list<varuint32, u64le>` | cache enabled is true | |
| payload | `bytes` | | the layout is a later specification |

- **Fixtures:**
  - `3a 00 00 00 ff ff ff ff 0f 00 00` is the column (0, 0) of the overworld, the client
    requests all sub-chunks, no cache, empty payload. *(constructed)*
  - `3a 02 01 00 fe ff ff ff 0f 04 00 00 00` is the column (1, -1), the client requests the
    sub-chunks up to index 4, no cache, empty payload. *(constructed)*
  - `3a 00 00 00 01 01 01 ef cd ab 89 67 45 23 01 01 00` is the column (0, 0) with 1 sub-chunk,
    cache on, one blob id `0x0123456789abcdef`, and the payload `00`. *(constructed)*

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEG-CHUNK-002: Sub-chunk count

| Value | Meaning |
|-------|---------|
| 0 to `0xfffffffd` | the payload holds this number of sub-chunks |
| `0xffffffff` | the payload holds no sub-chunk; the client requests all sub-chunks |
| `0xfffffffe` | the payload holds no sub-chunk; the client requests the sub-chunks up to the highest sub-chunk |

- **Behavior:**
  - With a request mode (`0xffffffff` or `0xfffffffe`), the client sends sub-chunk requests (a
    later specification). The payload then holds only the biome data and the border blocks.
  - The server sends the sub-chunks in the payload with a count from 0 to `0xfffffffd`.

Provenance: `documentation`, `source_suggested` (values); `design` (the server does not use the
request modes). Kind: wire, semantic. Confidence: high (values), medium (payload in request
mode). Edition: bedrock. Reason: the sub-chunk request packets are not specified yet.

### BEG-CHUNK-003: Blob cache

- **Behavior:**
  - The server writes "cache enabled" true only when the client allowed the blob cache (a later
    specification).
  - With the cache, each blob id names one part of the column, in the order of the parts. The
    client asks for the blobs that it does not have.
  - The server writes "cache enabled" false.

Provenance: `documentation`, `source_suggested` (layout); `design` (no cache). Kind: wire,
semantic. Confidence: high (layout), low (the hash function, Q6). Edition: bedrock.

## 9. Order from the handshake to the spawn

### BEG-SEQ-001: Start of the world

| Step | Direction | Packet |
|------|-----------|--------|
| 1 | serverbound | `ResourcePackClientResponse` (4) (`bedrock-handshake.md`, BEH-SEQ-004) |
| 2 | clientbound | `StartGame` |
| 3 | clientbound | the registries of the world: items, creative content, biomes, entity identifiers (later specifications) |
| 4 | serverbound | the request of a chunk radius (`0x45`) |
| 5 | clientbound | the chunk radius of the server (`0x46`) |
| 6 | clientbound | the centre and the radius of the chunk publisher (`0x79`) |
| 7 | clientbound | the chunk column packets around the spawn position (BEG-CHUNK-001) |
| 8 | clientbound | `PlayStatus` (3) (`bedrock-handshake.md`, BEH-SEQ-005) |
| 9 | serverbound | the local player is initialized (`0x71`) |

- **Behavior:**
  - The server sends `StartGame` first after response 4.
  - The server sends `PlayStatus` 3 after the columns in the radius of the chunk publisher
    around the spawn position.
  - The client shows the world after `PlayStatus` 3. It sends `PlayerAuthInput` from then.
  - The server sends `SetTime` and `Text` only after `StartGame`.
- **Edge cases:**
  - A `PlayerAuthInput` before `PlayStatus` 3 is not an error. The server ignores it.

Provenance: `source_suggested`. Kind: semantic. Confidence: high (steps 1, 2, 8), medium (steps 3
to 7 and 9). Edition: bedrock. Experiment: capture the packets of a login to BDS 1.21.30 from
response 4 to the packet `0x71`. Record the order and the count of the chunk column packets.

## 10. Intentional deviations

**D1.**

- **Rule**: BEG-CHUNK-002, BEG-CHUNK-003.
- **Reference**: BDS uses the sub-chunk request mode and the blob cache when the client allows
  them.
- **Visible to the client**: no; the world is the same.
- **Test**: a column packet of the server has a count from 0 to `0xfffffffd` and cache false.

**D2.**

- **Rule**: BEG-TIME-001.
- **Reference**: the time of the world wraps at the `i32` limit.
- **Visible to the client**: no, before the time passes the `i32` limit.
- **Test**: a time of `192000 × 20000 + 6000`, above the `i32` limit, gives the same time of day and moon phase as 6000.

## 11. Edition differences

**E1.**

- **Rule**: BEG-MOVE-003, BEG-PAI-001.
- **Java**: the client sends its position; the server corrects with a teleport id that the
  client confirms.
- **Bedrock**: the client sends its input with a client tick for each tick; the server corrects
  with the tick in `MovePlayer`.
- **Decision**: each edition keeps its own correction.
- **Selected by**: client edition.
- **Visible to the client**: yes.
- **Prediction risk**: medium; a wrong tick makes the client jump back.
- **Parity list**: none.

**E2.**

- **Rule**: BEG-TXT-002.
- **Java**: a system message and a player chat message are two packets, and chat is signed.
- **Bedrock**: one packet with a type; chat is not signed.
- **Decision**: each edition keeps its own form.
- **Selected by**: client edition.
- **Visible to the client**: yes.
- **Prediction risk**: none.
- **Parity list**: none.

## 12. Open questions

- **Q1.** The type of an integer game rule value in `StartGame` is `varuint32` in the sources.
  It is not known how the client reads a negative rule value. Experiment: BEG-SG-003.
- **Q2.** The item table must hold each vanilla item with the network id of BDS 1.21.30. The
  data source for the table in this repository is not decided.
- **Q3.** The documentation sends the legacy slots when the legacy request id is not 0. Some
  libraries send them only when the id is below -1 and even. Experiment: BEG-PAI-004.
- **Q4.** The sources do not agree on the bodies of item stack actions 13, 15 and 17. One
  source adds a `u8` to type 15 and removes the `u8` of type 17. The count of the ingredients of
  type 13 is `u8` in one source and `varuint32` in another. Experiment: BEG-PAI-006.
- **Q5.** The documentation gives a position and a face for block action 2. The libraries do
  not. Experiment: BEG-PAI-007.
- **Q6.** The hash function of a blob id is not confirmed. The server does not use the cache
  (D1).
- **Q7.** The ids of steps 3 to 7 and 9 of BEG-SEQ-001 and the packets of step 3 are not in this
  document. A later specification states them.
