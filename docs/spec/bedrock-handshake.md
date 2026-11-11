# Bedrock connection handshake: behavioural specification

| Field | Value |
|-------|-------|
| Edition | Bedrock |
| Protocol version | `729` |
| Game version | `1.21.30` (see `bedrock-wire.md`, Q1) |

This document states the packets and the order of the Bedrock Edition connection handshake at
protocol 729. The handshake starts with the first batch of the client. It ends when the client
accepts the resource pack stack. The game packets that follow are in `bedrock-game-packets.md`.

This document does not state these items. A later specification states each one:

- the check of the identity chain in `Login`, and Xbox Live authentication;
- the key exchange and the cipher layer;
- the transfer of resource pack data in chunks.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **vanilla client**, **packet**, **payload**,
  **field**, **decode error**: as `bedrock-wire.md`, section 0.1, defines them.
- **Batch**, **switch**, **marker**, **threshold**, **algorithm**: as `bedrock-batch.md`,
  section 0.1, defines them.
- **Clientbound**, **serverbound**: from the server to the client, and from the client to the
  server.
- **Protocol version**: the integer that names the packet set of a game version. This document
  states protocol 729.
- **Connection request**: the opaque data of `Login` that holds the identity chain and the client
  data (BEH-LOGIN-001).
- **Pack id**: the UUID of a pack in its text form, for example
  `0fba4063-dba1-4281-9b89-ff9390653530`.
- **Pack reference**: a pack id, the character `_`, and the pack version, for example
  `0fba4063-dba1-4281-9b89-ff9390653530_1.0.0`.
- **Add-on pack**: a behaviour pack. The protocol puts it in the add-on list of
  `ResourcePackStack`.
- **Experiment**: a named toggle of the world that turns on game content that is not final.

### 0.2 Notation and provenance

The notation, the wire types, the provenance labels, the sources and the experiment set-up are
as `bedrock-wire.md`, sections 0.2 to 0.4, defines them. Each rule has `Edition: bedrock`. A
field table lists the fields in wire order. The header (`bedrock-wire.md`, BEW-HDR-001) comes
before the first field and is not in the table.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `BEH-PKT` | packet ids and directions |
| `BEH-RNS` | `RequestNetworkSettings` |
| `BEH-NS` | `NetworkSettings` |
| `BEH-LOGIN` | `Login` |
| `BEH-PS` | `PlayStatus` |
| `BEH-DC` | `Disconnect` |
| `BEH-RPI` | `ResourcePacksInfo` |
| `BEH-RPS` | `ResourcePackStack` |
| `BEH-RPR` | `ResourcePackClientResponse` |
| `BEH-CFG` | server inputs |
| `BEH-SEQ` | the order of the handshake |

## 1. Packets

### BEH-PKT-001: Packet ids

| Packet | Id | Direction |
|--------|----|-----------|
| `Login` | `0x01` | serverbound |
| `PlayStatus` | `0x02` | clientbound |
| `Disconnect` | `0x05` | clientbound |
| `ResourcePacksInfo` | `0x06` | clientbound |
| `ResourcePackStack` | `0x07` | clientbound |
| `ResourcePackClientResponse` | `0x08` | serverbound |
| `NetworkSettings` | `0x8f` | clientbound |
| `RequestNetworkSettings` | `0xc1` | serverbound |

- **Edge cases:** a packet in the wrong direction is an unexpected packet (BEH-SEQ-006).

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEH-RNS-001: `RequestNetworkSettings`

| Field | Wire type | Notes |
|-------|-----------|-------|
| protocol version | `i32be` | 729 at this version |

- **Fixtures:** `c1 01 00 00 02 d9` is the packet with protocol 729.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.
Experiment: capture the first batch of a vanilla client of 1.21.30. Compare it with
`fe 06 c1 01 00 00 02 d9`.

### BEH-NS-001: `NetworkSettings`

| Field | Wire type | Notes |
|-------|-----------|-------|
| threshold | `u16le` | `bedrock-batch.md`, BEB-CMP-003 |
| algorithm | `u16le` | `0x0000` raw deflate, `0x0001` Snappy, `0xffff` none |
| client throttle enabled | `bool` | |
| client throttle threshold | `u8` | |
| client throttle scalar | `f32le` | |

- **Behavior:**
  - The server writes false, 0 and 0.0 in the three client throttle fields.
- **Edge cases:**
  - Another algorithm value is a decode error in the client.
- **Fixtures:** `8f 01 00 02 00 00 00 00 00 00 00 00` is the packet with threshold 512,
  algorithm raw deflate and no client throttle. *(constructed)*

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high (fields), low
(throttle meaning). Edition: bedrock. Experiment: capture the `NetworkSettings` packet of BDS
1.21.30 with the default `server.properties`. Record the five values.

### BEH-LOGIN-001: `Login`

| Field | Wire type | Notes |
|-------|-----------|-------|
| protocol version | `i32be` | 729 at this version |
| connection request | `bytes` | the layout below |

The connection request has this layout:

| Part | Wire type | Notes |
|------|-----------|-------|
| chain length | `i32le` | the byte length of the chain JSON |
| chain JSON | bytes | UTF-8 JSON: an object with the key `chain`, an array of JWT strings |
| client data length | `i32le` | the byte length of the client data |
| client data | bytes | one JWT in its compact form |

- **Edge cases:**
  - A negative length, or a length larger than the bytes that remain, is a decode error.
  - Bytes after the client data are a decode error.
- **Fixtures:**
  `01 00 00 02 d9 14 0c 00 00 00 7b 22 63 68 61 69 6e 22 3a 5b 5d 7d 00 00 00 00` is a packet
  with protocol 729, the chain JSON `{"chain":[]}` and empty client data. It is a fixture of the
  layout only. A server does not accept this identity. *(constructed)*

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEH-PS-001: `PlayStatus`

| Field | Wire type | Notes |
|-------|-----------|-------|
| status | `i32be` | the table below |

| Status | Meaning |
|--------|---------|
| 0 | the login is accepted |
| 1 | the login fails: the client is older than the server |
| 2 | the login fails: the server is older than the client |
| 3 | the player spawns: the client can show the world |
| 4 | the login fails: the tenant is not valid (Education Edition) |
| 5 | the login fails: an Education Edition client joins a server that is not Education Edition |
| 6 | the login fails: a client that is not Education Edition joins an Education Edition server |
| 7 | the login fails: the server is full for a sub-client |
| 8 | the login fails: an Editor client joins a server that is not an Editor server |
| 9 | the login fails: a client that is not an Editor client joins an Editor server |

- **Fixtures:** `02 00 00 00 00` is status 0. `02 00 00 00 03` is status 3.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEH-DC-001: `Disconnect`

| Field | Wire type | Condition | Notes |
|-------|-----------|-----------|-------|
| reason | `varint32` | | the table below |
| hide screen | `bool` | | |
| message | `string` | hide screen is false | the text that the client shows |
| filtered message | `string` | hide screen is false | the message after a text filter |

- **Behavior:**
  - When hide screen is false, the client shows a disconnect screen with the message.
  - When hide screen is true, the packet ends after that field. The client shows no disconnect
    screen.
  - The server writes an empty filtered message (BEH-DC-002).
  - The client closes the connection after it receives the packet.
- **Values:** the reason values that a server of mcrs sends:

| Reason | Meaning |
|--------|---------|
| 0 | unknown |
| 25 | the server is full |
| 34 | the server is older than the client |
| 35 | the client is older than the server |
| 41 | disconnected |
| 42 | the player is not valid |
| 43 | the player logged in from another location |
| 45 | not allowed |
| 46 | not authenticated |
| 48 | unknown packet |
| 49 | unexpected packet |
| 55 | kicked |
| 57 | kicked because the player was idle |
| 58 | a resource pack problem |
| 68 | the server shuts down |

  The public documentation lists the values 0 to 74. A receiver accepts each `varint32` value.
- **Fixtures:**
  - `05 6e 00 03 62 79 65 00` is reason 55, the message `bye` and an empty filtered message.
  - `05 6e 01` is reason 55 with hide screen true.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high (fields), medium
(what the client shows). Edition: bedrock. Experiment: from BDS, kick a player with
`/kick <name> bye`. Capture the `Disconnect` packet. Then send the client a `Disconnect` with
hide screen true, and record the screen that the client shows.

### BEH-DC-002: Filtered message of `Disconnect`

- **Behavior:**
  - The server writes the message in the message field.
  - The server writes an empty string in the filtered message field, because mcrs has no text
    filter.

Provenance: `design`. Kind: semantic. Confidence: medium. Edition: bedrock. Reason: mcrs has no
text filter, and the libraries write an empty filtered message. See Q2.

### BEH-RPI-001: `ResourcePacksInfo`

| Field | Wire type | Notes |
|-------|-----------|-------|
| packs required | `bool` | the client must accept the packs to join |
| has add-on packs | `bool` | |
| has scripts | `bool` | |
| resource packs | `list<u16le, entry>` | the entry layout below |
| CDN URLs | `list<varuint32, url>` | the url layout below |

An entry has this layout:

| Field | Wire type | Notes |
|-------|-----------|-------|
| pack id | `string` | the UUID in text form |
| version | `string` | for example `1.0.0` |
| size | `u64le` | the size of the pack archive in bytes |
| content key | `string` | the key of an encrypted pack, or empty |
| sub-pack name | `string` | |
| content identity | `string` | |
| has scripts | `bool` | |
| is add-on pack | `bool` | the pack is part of an add-on |
| ray tracing capable | `bool` | |

A url has this layout:

| Field | Wire type | Notes |
|-------|-----------|-------|
| pack reference | `string` | |
| URL | `string` | the address where the client can download the pack |

- **Behavior:**
  - At protocol 729, the packet has no behaviour pack list. A behaviour pack goes only in
    `ResourcePackStack`.
- **Fixtures:**
  - `06 00 00 00 00 00 00` is the packet with no pack and no URL.
  - This is the packet with packs required and one resource pack of 1024 bytes:
    `06 01 00 00 01 00 24 30 66 62 61 34 30 36 33 2d 64 62 61 31 2d 34 32 38 31 2d 39 62 38 39
    2d 66 66 39 33 39 30 36 35 33 35 33 30 05 31 2e 30 2e 30 00 04 00 00 00 00 00 00 00 00 00
    00 00 00 00`. *(constructed)*

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEH-RPS-001: `ResourcePackStack`

| Field | Wire type | Notes |
|-------|-----------|-------|
| packs required | `bool` | |
| add-on packs | `list<varuint32, stack entry>` | behaviour packs |
| resource packs | `list<varuint32, stack entry>` | |
| base game version | `string` | the game version of the vanilla packs, or `*` |
| experiments | `list<u32le, experiment>` | |
| experiments previously toggled | `bool` | true when the world ever had an experiment on |
| include editor packs | `bool` | true only for an Editor world |

A stack entry has this layout:

| Field | Wire type |
|-------|-----------|
| pack id | `string` |
| version | `string` |
| sub-pack name | `string` |

An experiment has this layout:

| Field | Wire type | Notes |
|-------|-----------|-------|
| name | `string` | for example `gametest` |
| enabled | `bool` | |

- **Behavior:**
  - The client applies the packs of each list in the order of the list.
  - The base game version `*` selects the vanilla packs of the client version.
  - The server writes false in include editor packs.
- **Fixtures:**
  - `07 00 00 00 01 2a 00 00 00 00 00 00` is the packet with no pack, the base game version `*`
    and no experiment.
  - `07 00 00 00 07 31 2e 32 31 2e 33 30 01 00 00 00 08 67 61 6d 65 74 65 73 74 01 01 00` is
    the packet with the base game version `1.21.30` and the experiment `gametest` on.
    *(constructed)*

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high (fields), medium
(pack order). Edition: bedrock. Experiment: start BDS with one behaviour pack and one resource
pack. Capture `ResourcePackStack`. Record the base game version and the order of the lists.

### BEH-RPR-001: `ResourcePackClientResponse`

| Field | Wire type | Notes |
|-------|-----------|-------|
| response | `u8` | the table below |
| pack references | `list<u16le, string>` | the packs that the client asks for |

| Response | Meaning |
|----------|---------|
| 1 | refused: the player does not accept the packs |
| 2 | send packs: the client asks for the packs of the list |
| 3 | have all packs: the client has each pack of `ResourcePacksInfo` |
| 4 | completed: the client applied the stack |

- **Edge cases:**
  - Another response value is a decode error.
  - The pack references list is empty for responses 1, 3 and 4.
- **Fixtures:**
  - `08 04 00 00` is response 4.
  - `08 02 01 00 2a 30 66 62 61 34 30 36 33 2d 64 62 61 31 2d 34 32 38 31 2d 39 62 38 39 2d 66
    66 39 33 39 30 36 35 33 35 33 30 5f 31 2e 30 2e 30` is response 2 for one pack.
    *(constructed)*

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

## 2. Server inputs

### BEH-CFG-001: Compression settings

- **Inputs:** the threshold and the algorithm of `NetworkSettings`.
- **Behavior:**
  - The server takes both values from its configuration.
  - The default threshold is 1. The default algorithm is raw deflate.

Provenance: `source_suggested`. Kind: unspecified. Confidence: medium. Edition: bedrock.
Experiment: read the `compression-threshold` and `compression-algorithm` defaults of the
`server.properties` file of BDS 1.21.30.

### BEH-CFG-002: Server packs

- **Inputs:** the resource packs and the behaviour packs of the server, and the packs required
  flag.
- **Behavior:** the server fills `ResourcePacksInfo` and `ResourcePackStack` from these inputs.
  With no pack, both lists of each packet are empty.

Provenance: `design`. Kind: unspecified. Confidence: high. Edition: bedrock.

## 3. Order of the handshake

### BEH-SEQ-001: Network settings

- **Behavior:**
  1. The client sends `RequestNetworkSettings` in its first batch.
  2. The server checks the protocol version (BEH-SEQ-002).
  3. The server sends `NetworkSettings` (BEH-NS-001).
  4. Both sides make the switch (`bedrock-batch.md`, BEB-CMP-001).
- **Edge cases:**
  - Each packet other than `RequestNetworkSettings` before step 3 is an unexpected packet
    (BEH-SEQ-006).

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEH-SEQ-002: Version check

- **Inputs:** the protocol version of `RequestNetworkSettings`.
- **Behavior:**
  - A version equal to 729 passes.
  - For a version less than 729, the server sends `PlayStatus` with status 1. Then it closes
    the connection.
  - For a version more than 729, the server sends `PlayStatus` with status 2. Then it closes
    the connection.
  - The server sends that `PlayStatus` before the switch, in a batch with no marker.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Edition: bedrock. Experiment:
connect a vanilla client of 1.21.20 (protocol 712) to BDS 1.21.30. Capture the answer of BDS.
Record the screen of the client.

### BEH-SEQ-003: Login

- **Behavior:**
  1. After the switch, the client sends `Login`.
  2. The server checks that the protocol version of `Login` is 729. Another version has the
     result of BEH-SEQ-002, but after the switch.
  3. The server checks the identity (a later specification).
  4. When the cipher layer is on, the key exchange occurs here (a later specification).
  5. The server sends `PlayStatus` with status 0.
  6. The server sends `ResourcePacksInfo` (BEH-RPI-001).
- **Edge cases:**
  - The server can send step 5 and step 6 in one batch, in this order.

Provenance: `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock. Experiment:
capture an offline login to BDS 1.21.30. Record the packets after `Login` and their batches.

### BEH-SEQ-004: Pack negotiation

- **Behavior:**
  - The client answers `ResourcePacksInfo` with `ResourcePackClientResponse`.
  - For response 2, the server sends the data of each pack of the list (a later
    specification). The client then sends response 3.
  - For response 3, the server sends `ResourcePackStack` (BEH-RPS-001).
  - For response 4, the handshake ends. The server starts the world with `StartGame`
    (`bedrock-game-packets.md`, BEG-SEQ-001).
  - For response 1, the server sends `Disconnect` with reason 58 when packs required is true.
    Else, it sends `ResourcePackStack` with no resource pack.
- **Edge cases:**
  - With no pack, the client sends response 3 directly after `ResourcePacksInfo`.
  - Response 4 before `ResourcePackStack` is an unexpected packet (BEH-SEQ-006).

| Step | Direction | Packet |
|------|-----------|--------|
| 1 | serverbound | `RequestNetworkSettings` |
| 2 | clientbound | `NetworkSettings` |
| 3 | serverbound | `Login` |
| 4 | clientbound | `PlayStatus` (0) |
| 5 | clientbound | `ResourcePacksInfo` |
| 6 | serverbound | `ResourcePackClientResponse` (3) |
| 7 | clientbound | `ResourcePackStack` |
| 8 | serverbound | `ResourcePackClientResponse` (4) |

The table is the order of a login with no pack and no cipher layer.

Provenance: `source_suggested`. Kind: semantic. Confidence: high (order), medium (response 1).
Edition: bedrock. Experiment: capture a login to BDS 1.21.30 with no pack. Compare the packets
with the table. Then start BDS with a required pack, refuse it in the client, and capture the
answer.

### BEH-SEQ-005: Spawn

- **Behavior:** the server sends `PlayStatus` with status 3 after `StartGame` and after the
  chunk columns around the spawn position (`bedrock-game-packets.md`, BEG-SEQ-001).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Edition: bedrock.

### BEH-SEQ-006: Unexpected packet

- **Behavior:**
  - During the handshake, a packet that this section does not expect is an unexpected packet.
  - The server sends `Disconnect` with reason 49 and closes the connection.
  - Before the switch, the server closes the connection with no packet.

Provenance: `design`. Kind: semantic. Confidence: medium. Edition: bedrock. Reason: the
handshake has one valid order. Experiment: send BDS a `Text` packet before `Login`. Record the
answer.

## 4. Intentional deviations

None.

## 5. Edition differences

**E1.**

- **Rule**: BEH-SEQ-001.
- **Java**: the client selects the protocol in the `intention` packet, and the server enables
  compression during LOGIN (`login.md`).
- **Bedrock**: the client asks for the network settings before `Login`, and compression starts
  before `Login`.
- **Decision**: each edition keeps its own order.
- **Selected by**: client edition.
- **Visible to the client**: yes.
- **Prediction risk**: none.
- **Parity list**: none.

## 6. Open questions

- **Q1.** The meaning of the three client throttle fields of `NetworkSettings` is not known. The
  server writes false, 0 and 0.0.
- **Q2.** It is not known which of the two messages of `Disconnect` the client shows when its
  text filter setting is on. Experiment: send the client a `Disconnect` with two different
  messages. Turn the profanity filter of the client on, then off. Record the text that the
  client shows each time.
- **Q3.** It is not known whether BDS sends `Disconnect` or `PlayStatus` for a protocol mismatch
  at `Login`. The experiment of BEH-SEQ-002 decides it.
