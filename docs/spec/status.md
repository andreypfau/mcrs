# The STATUS protocol: behavioural specification

This document states the STATUS state of the game protocol. A client uses this
state to get the server list entry of a server. The entry holds the message of
the day, the player counts, a sample of player names and the game version. The
client also uses the state to measure the round-trip time.

This document states only what a client, a server or a packet capture can see.
It does not prescribe an architecture. Every rule comes from the responses of a
vanilla server. Section 7 lists the experiments.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **vanilla client**, **direction**,
  **clientbound**, **serverbound**, **VarInt**, **frame**, **state**,
  **handshake**, **intent**, **offline mode**: as `login.md`, section 0.1,
  defines them.
- **Status exchange**: one connection that starts with a handshake with intent
  `1`.
- **Status request**: the serverbound packet `status_request`.
- **Status response**: the clientbound packet `status_response`.
- **Response document**: the JSON text in the status response.
- **Ping**: the serverbound packet `ping_request`.
- **Pong**: the clientbound packet `pong_response`.
- **Payload**: the 8-byte value in a ping or a pong.
- **Online player**: a player that the server shows as joined in its log
  (`<name> joined the game`) and that has not disconnected.
- **Join order**: the order of the online players by the time at which they
  joined, the first player first. Positions in the join order start at `0`.
- **Sample**: the array `players.sample` of the response document.
- **Sample entry**: one object of the sample.
- **Idle time**: the time since the server received the last serverbound packet
  of the connection. The handshake is a serverbound packet.
- **Probe**: the test client of section 7. It writes frames and logs each frame
  that it receives, with the time.

### 0.2 Wire types

| Type | Form |
|------|------|
| VarInt | `compression.md`, section 0.1 |
| String | a VarInt byte count, then UTF-8 bytes |
| Long | 8 bytes, big-endian |

### 0.3 Provenance and confidence

This document uses the provenance labels of `registry.md`, section 0.3. A rule
with the label `black_box_confirmed` cites the experiment of section 7 that
shows it.

### 0.4 Artefacts cited

- `assets/mcrs/reports/packets.json`: the packet id report. Its `status` section
  gives the packet ids of STATUS-PKT-001.
- `assets/minecraft/version.json`: the version name and the protocol version of
  STATUS-JSON-005.

### 0.5 ID prefixes

| Prefix | Subject |
|--------|---------|
| `STATUS-PKT` | packet ids and field layouts |
| `STATUS-SEQ` | the order of packets and the end of the connection |
| `STATUS-JSON` | the fields of the response document |
| `STATUS-CFG` | server inputs |
| `STATUS-DEV` | intentional deviations from the reference |

### 0.6 Scope

In scope:

- the STATUS packets in both directions, after a handshake with intent `1`;
- the response document;
- the server properties that change the response document or the exchange.

Out of scope:

- the handshake itself (`login.md`, section 2);
- the old status form of clients before the handshake existed, which starts
  with the byte `0xFE`;
- the LAN announcement;
- the content of a server icon (section 6).

### 0.7 Names that appear in this document

The document names data, never code. It uses these names of the game:

- packet names from the packet report, for example `status_response`;
- keys of `server.properties`, for example `hide-online-players`;
- keys of the response document, for example `players.sample`;
- text that the vanilla server writes to its log.

---

## 1. Packets

### STATUS-PKT-001: Packet ids

| Direction | Id | Packet |
|-----------|----|--------|
| serverbound | `0x00` | `status_request` |
| serverbound | `0x01` | `ping_request` |
| clientbound | `0x00` | `status_response` |
| clientbound | `0x01` | `pong_response` |

Provenance: `black_box_confirmed` (`assets/mcrs/reports/packets.json`, E1).
Kind: wire. Confidence: high.

### STATUS-PKT-002: Field layouts

| Packet | Fields |
|--------|--------|
| `status_request` | no fields |
| `status_response` | one String: the response document |
| `ping_request` | one Long: the payload |
| `pong_response` | one Long: the payload |

- **Behavior:** the response document is JSON text with no white space between
  tokens.
- **Behavior:** the server sends the frames of a status exchange without
  compression. The STATUS state has no compression and no cipher layer.

Provenance: `black_box_confirmed` (E1). Kind: wire. Confidence: high.

## 2. Sequence

### STATUS-SEQ-001: Normal exchange

1. The client sends the handshake with intent `1`.
2. The client sends a status request.
3. The server sends one status response.
4. The client sends a ping.
5. The server sends one pong.
6. The server closes the connection.

- **Behavior:** the pong holds the same 8 bytes as the payload of the ping. The
  server does not interpret the payload. `0xffffffffffffffff` comes back as
  `0xffffffffffffffff`.
- **Behavior:** the server closes the connection directly after the pong. In
  E1, the close came less than 50 ms after the ping.

Provenance: `black_box_confirmed` (E1, E2). Kind: wire. Confidence: high.

### STATUS-SEQ-002: One status request per connection

- **Behavior:** the server answers only the first status request of a
  connection.
- **Edge cases:** when the client sends a second status request, the server
  sends no packet and closes the connection.

Provenance: `black_box_confirmed` (E3). Kind: semantic. Confidence: high.

### STATUS-SEQ-003: Ping without a status request

- **Behavior:** the client can send a ping before a status request. The server
  sends the pong and then closes the connection.
- **Edge cases:** after a pong, the connection is closed. The client cannot send
  a status request or a second ping on it.

Provenance: `black_box_confirmed` (E4, E5). Kind: semantic. Confidence: high.

### STATUS-SEQ-004: Idle timeout

- **Behavior:** the server closes the connection when the idle time is 30 s. It
  sends no packet before the close.
- **Behavior:** each serverbound packet sets the idle time to zero. A status
  request at 20 s moves the close to 50 s.
- **Edge cases:** this rule applies also after the status response, while the
  server waits for a ping.

Provenance: `black_box_confirmed` (E6). Kind: semantic. Confidence: high. The
measured close times were 30.002 s, 30.002 s and 50.007 s.

### STATUS-SEQ-005: Protocol version of the handshake

- **Behavior:** for the protocol versions `5` and `1073742166` in the
  handshake, the server answers the status exchange.
- **Behavior:** for both versions, the response document is the same.
  `version.protocol` is the version of the server, not the version of the
  client.

Provenance: `black_box_confirmed` (E1, E7). Kind: semantic. Confidence: high
for these two versions. No experiment used other versions (section 6).

## 3. Response document

### STATUS-JSON-001: Top-level shape

```json
{"description":"A Minecraft Server","players":{"max":20,"online":0},"version":{"name":"26.4 Snapshot 4","protocol":1073742166}}
```

| Key | Type | Present |
|-----|------|---------|
| `description` | string | when the `motd` property is not empty (STATUS-JSON-002) |
| `players` | object | always (STATUS-JSON-003) |
| `version` | object | always (STATUS-JSON-005) |

- **Behavior:** the vanilla server writes the keys in the order of the table.
- **Behavior:** the vanilla server does not write the keys `favicon`,
  `enforcesSecureChat` or `preventsChatReports` in any experiment of this
  document. No experiment had a `server-icon.png` file. All experiments had
  `online-mode=false` and `enforce-secure-profile=false` (section 7).

Provenance: `black_box_confirmed` (E1, E8). Kind: wire. Confidence: high for the
keys that the experiments show. A file `server-icon.png` or other values of
`online-mode` and `enforce-secure-profile` can add a key (section 6).

### STATUS-JSON-002: `description`

- **Inputs:** the `motd` property.
- **Behavior:** `description` is a JSON string with the text of `motd`. It is
  not a text component object.
- **Behavior:** the server does not change the text. A formatting code stays in
  the string as the character `§` (U+00A7), followed by its code character.
- **Behavior:** a line feed in the property text stays a line feed in the
  string. In `server.properties`, the escape `\n` writes a line feed.
- **Behavior:** the server writes `§` as UTF-8 bytes `0xC2 0xA7`, not as a JSON
  escape.
- **Edge cases:** when `motd` is empty, the response document has no key
  `description`. The server does not send an empty string.
- **Edge cases:** when `server.properties` has no `motd` key, the value is
  `A Minecraft Server`.

Provenance: `black_box_confirmed` (E1, E8, E9, E10). Kind: wire. Confidence:
high.

### STATUS-JSON-003: `players`

| Key | Type | Present |
|-----|------|---------|
| `max` | integer | always |
| `online` | integer | always |
| `sample` | array of sample entries | when there is at least one online player and `hide-online-players` is `false` (STATUS-JSON-004, STATUS-CFG-003) |

- **Inputs:** the `max-players` property, and the online players.
- **Behavior:** `max` is the value of `max-players`. The value `0` gives `0`.
- **Behavior:** `online` is the number of online players.
- **Behavior:** when there are no online players, the object has no key
  `sample`. The server does not send an empty array.
- **Edge cases:** `hide-online-players` does not change `online`
  (STATUS-CFG-003).

Provenance: `black_box_confirmed` (E1, E8, E11, E12, E14). Kind: wire.
Confidence: high.

### STATUS-JSON-004: `players.sample`

Each sample entry has these keys, in this order:

| Key | Type | Value |
|-----|------|-------|
| `id` | string | the UUID of the player, in lowercase hexadecimal with hyphens (8-4-4-4-12) |
| `name` | string | the name of the player |

- **Inputs:** the online players in the join order. `n` is the number of online
  players.
- **Behavior:** when `n` is 12 or less, the sample holds all online players.
- **Behavior:** when `n` is more than 12, the sample holds 12 of the online
  players. The selection of the 12 players is unspecified. A parity test must
  not compare it.
- **Reference:** the reference selects a start position `s` at random, from `0`
  to `n − 12`, for each new response document (STATUS-JSON-006). It takes the
  players at the positions `s` to `s + 11` of the join order. The window does
  not wrap from the end of the join order to the start.
- **Behavior:** the order of the entries in the sample is unspecified. The
  reference puts the entries in a random order.
- **Behavior:** each online player occurs in the sample at most one time.
- **Behavior:** a client must not give a meaning to the order of the sample.
- **Edge cases:** in offline mode, `id` is the name-based UUID of version 3 for
  the text `OfflinePlayer:<name>`. For `bot02`, it is
  `b7fedf1d-4755-3933-8b95-94af66bb8fba`. This UUID comes from the login, not
  from the STATUS state.

Provenance: `black_box_confirmed` (E11, E12, E13). Kind: semantic for the
limit of 12, unspecified for the selection and the order. Confidence: high for
the limit of 12 and for the adjacent window of the reference. The evidence is
the 12 response documents of E12 and the six captures of E11, all with 15
online players. Confidence: medium for the uniform choice of `s`. E12 showed
the start positions 0, 1, 2 and 3 with the counts 3, 5, 2 and 2.

### STATUS-JSON-005: `version`

| Key | Type | Value |
|-----|------|-------|
| `name` | string | the name of the game version: `26.4 Snapshot 4` |
| `protocol` | integer | the protocol version of the server: `1073742166` |

- **Behavior:** both values are the `name` and `protocol_version` of
  `assets/minecraft/version.json`.
- **Behavior:** no server property changes them.

Provenance: `black_box_confirmed` (`assets/minecraft/version.json`, E1, E7).
Kind: wire. Confidence: high.

### STATUS-JSON-006: Refresh of the response document

- **Behavior:** when no player joins or leaves, the server sends the same
  response document in all status exchanges in a period of 5 s.
- **Behavior:** after 5 s, the server makes a new response document. The new
  sample has a new start position and a new order (STATUS-JSON-004), also when
  the online players are the same.
- **Behavior:** when a player joins or leaves, the next status exchange gets a
  new response document. `online` and the sample show the change at once. In
  E13 and E14, the change was visible in less than 0.5 s.
- **Behavior:** a join also starts a new period of 5 s.

Provenance: `black_box_confirmed` (E12, E13, E14). Kind: semantic. Confidence:
medium. The probe has a resolution of about 1 s. The measured periods were
4.5 s to 5.3 s. No experiment measured if a leave starts a new period. No
experiment shows if the server takes the response document at the handshake or
at the status request (section 6).

### STATUS-JSON-007: `favicon`

- **Behavior:** by default, the server writes the key `favicon`. Its value is
  `data:image/png;base64,` and then the Base64 text of the PNG file
  `assets/mcrs/server-icon.png`, an image of 64 × 64 pixels.
- **Behavior:** a server that has no icon omits the key.

Provenance: `documentation`. Kind: wire. Confidence: medium. STATUS-DEV-001 records
the difference from the reference.
No experiment of this document captured the key from a vanilla server (section 6).

## 4. Server inputs

### STATUS-CFG-001: `motd`

- **Behavior:** the value sets `description` (STATUS-JSON-002).
- **Behavior:** the server reads the value when it starts. A change of the file
  has an effect only after a restart.

Provenance: `black_box_confirmed` (E8, E9, E10). Kind: semantic. Confidence:
high for the first behavior. Confidence: medium for the second behavior,
because no experiment changed the file while the server ran.

### STATUS-CFG-002: `max-players`

- **Behavior:** the value sets `players.max` (STATUS-JSON-003). The value `7`
  gives `7`, and the value `0` gives `0`.
- **Edge cases:** the default value is `20`.

Provenance: `black_box_confirmed` (E1, E8, E15). Kind: semantic. Confidence:
high.

### STATUS-CFG-003: `hide-online-players`

- **Behavior:** when the value is `true`, the response document has no key
  `players.sample`, also with online players.
- **Behavior:** `players.online` and `players.max` stay the same as with the
  value `false`.

Provenance: `black_box_confirmed` (E16). Kind: semantic. Confidence: high.

### STATUS-CFG-004: `enable-status`

- **Behavior:** when the value is `false`, the server sends no packet in a
  status exchange. It does not send a status response or a pong.
- **Behavior:** when the value is `false`, the server closes the connection.
- **Edge cases:** the default value is `true`.

Provenance: `black_box_confirmed` (E17). Kind: semantic. Confidence: high. E17
sends its first packet directly after the handshake. It does not show if the
close comes after the handshake or after that packet (section 6).

## 6. Unresolved

- **Server icon.** A file `server-icon.png` in the server directory can add a
  key to the response document. No experiment of this document used one.
  Proposed experiment: add a PNG file of 64 × 64 pixels and repeat E1. The form
  of the value in STATUS-JSON-007 comes from public documentation, and this
  experiment can confirm it. The server does not read a `server-icon.png` file.
- **Per-player opt-out.** The vanilla client has an option that hides its
  player from server listings. The probe client did not change it. Proposed
  experiment: join one vanilla client with the option off and one with it on,
  then repeat E11.
- **Unknown packet.** No experiment sent a packet id other than `0x00` or
  `0x01`, or a packet with extra bytes.
- **Long text.** No experiment measured a limit on the length of `motd` or of
  the response document. Proposed experiment: set a `motd` of 40000 characters
  and repeat E1.
- **Other protocol versions.** E7 used only the version `5`. Proposed
  experiment: repeat E7 with `0`, a negative version and `1073742167`.
- **Join order after a leave.** No experiment made a player leave from the
  middle of the join order and then read the sample.
- **Secure chat.** All experiments had `online-mode=false` and
  `enforce-secure-profile=false`. Other values of these properties can add the
  key `enforcesSecureChat`. Proposed experiment: set both properties to `true`,
  then repeat E1.
- **Time of the response document.** In E13 and E14, the probe sent each status
  request directly after the handshake. In E6 (c), no player joined between the
  handshake and the status request. No experiment shows if the server takes the
  response document at the handshake or at the status request.
  Proposed experiment: send the handshake, join a player, wait 1 s, and then
  send a status request.
- **Close with `enable-status=false`.** E17 does not show if the server closes
  the connection after the handshake or after the first packet. Proposed
  experiment: set `enable-status=false`, send only the handshake, and read 5 s.
- **Part of a frame.** No experiment sent a frame in parts with a pause between
  them. It is not known if each received byte sets the idle time to zero, or
  only a complete packet.
- **Server start.** All experiments started after the log line `Done`. No
  experiment sent a status exchange while the server prepared its world.

## 7. Experiments

The probe is a TCP client that sends frames without compression and logs each frame. The server runs with
`online-mode=false`, `enforce-secure-profile=false` and `white-list=false`. The properties column lists the
other properties.


| Id | Properties added | Players | Probe steps |
|----|------------------|---------|-------------|
| E1 | none | 0 | status request, read 5 s; ping `0x0123456789abcdef`, read 5 s; read 5 s |
| E2 | none | 0 | status request, read; ping `0xffffffffffffffff`, read |
| E3 | none | 0 | status request, read; status request, read; read |
| E4 | none | 0 | ping `0x2a`, read; read |
| E5 | none | 0 | status request, read; ping `1`, read; ping `2`, read |
| E6 | none | 0 | (a) read 60 s; (b) status request, read, read 60 s; (c) read 20 s, status request, read, read 60 s |
| E7 | none | 0 | status request, read; with protocol version `5` in the handshake |
| E8 | `motd=Hello mcrs`, `max-players=7` | 0 | status request, read |
| E9 | `motd=§cRed §lbold§r plain\nsecond line` | 0 | status request, read |
| E10 | `motd=` | 0 | status request, read |
| E11 | none | 15 | status request, read; repeat six times, 2 s apart |
| E12 | none | 15 | status request, read; repeat 90 times, 0.5 s apart; log the sample names and the missing names when the document changes |
| E13 | none | 0, then 3 | start 3 players; status request, read; repeat 40 times, 0.3 s apart |
| E14 | none | 3, then 0 | stop the 3 players; status request, read; repeat six times, 0.2 s apart |
| E15 | `max-players=0` | 0 | status request, read |
| E16 | `hide-online-players=true` | 3 | status request, read |
| E17 | `enable-status=false` | 0 | (a) as E1; (b) as E4 |

## 9. Intentional deviations

### STATUS-DEV-001: The default icon

- **Rule:** STATUS-JSON-007.
- **Reference:** a vanilla server without a file `server-icon.png` omits the key
  `favicon` (STATUS-JSON-001).
- **Visible to the client:** yes. The server list shows the icon of mc.rs and not
  the default icon of the client.
- **Test:** a differential test ignores the key `favicon`.
