# The LOGIN protocol: behavioural specification

| Field | 26.3 | 26.4 |
|-------|------|------|
| Game version | `26.3` (release) | `26.4-snapshot-5` (Minecraft Java Edition, 26.4 Snapshot 5) |
| Protocol version | `777` | `1073742167` |
| Switch mode | immediate switch | acknowledged switch |

This document states the LOGIN state of the game protocol for two versions, 26.3
and 26.4. mcrs supports both versions on one server and on one client. The rules
for 26.4 come from snapshot 5. The LOGIN changes of 26.4 first occur in snapshot
5. Snapshots 1 to 4 of 26.4 have the LOGIN packets of 26.3.

This document also states the handshake that leads to LOGIN. The frame format,
compression and the cipher layer are in `compression.md`. This document states
when each side switches them on.

This document states only what a client, a server or a packet capture can see.
It does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**: the two sides of one connection.
- **Vanilla server**, **vanilla client**: the game's own server and client.
- **Direction**, **clientbound**, **serverbound**, **VarInt**, **frame**,
  **threshold**, **compression**, **cipher layer**: as `compression.md`, section
  0.1, defines them.
- **State**: the packet table that applies to a direction. The states in this
  document are HANDSHAKE, LOGIN and CONFIGURATION.
- **Handshake**: the first serverbound packet of a connection, `intention`, in the
  HANDSHAKE state.
- **Intent**: the field of `intention` that selects the next state.
- **Online mode**: a login in which the server authenticates the player with the
  session server. It switches the cipher layer on.
- **Offline mode**: a login in which the server does not authenticate the player.
  It does not switch the cipher layer on.
- **Session server**: the public web service that authenticates a player's
  session. The client calls it to join a server. The server calls it to check
  that join.
- **In-process connection**: a connection between a client and a server in one
  process. It has no socket. The integrated server of single-player uses it for
  the host player.
- **Integrated server**: the server that runs inside a client, for single-player
  and for a world open to LAN.
- **Owner**: the player of the client that runs the integrated server.
- **Shared secret**: a random 16-byte key that the client makes in online mode.
- **Announcement**: a packet that tells the client about a new layer state:
  `login_compression` (26.3), `hello` with key data (26.3), `setup_compression`,
  `apply_options` or `login_finished` (26.4).
- **Acknowledgement**: a packet that the client sends after it applies an
  announcement in 26.4: `options_applied` or `login_acknowledged`.
- **Switch mode**: the rule that sets when each side switches compression and the
  cipher layer on.
- **Immediate switch**: the switch mode of 26.3. Each side switches a layer on for
  both directions directly when it sends or receives the related packet.
- **Acknowledged switch**: the switch mode of 26.4. The server announces a layer.
  The client switches its inbound direction, sends an acknowledgement, and then
  switches its outbound direction. The server switches both directions when it
  receives the acknowledgement.
- **Option flags**: the one-byte bit set of 26.4 that names layers (LOGIN-PKT-004).
- **Last plain frame**, **first encrypted frame**: the frames on the two sides of
  the cipher switch point in one direction.
- **Last uncompressed frame**, **first compressed frame**: the frames on the two
  sides of the compression switch point in one direction.
- **Terminal packet**: a packet after which the receiver decodes no more frames
  in the current state (LOGIN-PKT-005).
- **Tick**: one step of the server game loop. A server that is not overloaded runs
  20 ticks in one second.

### 0.2 Wire types

| Type | Form |
|------|------|
| VarInt | `compression.md`, section 0.1 |
| String (n) | a VarInt byte count, then UTF-8 bytes. The byte count is at most `3 × n`. The text holds at most `n` UTF-16 code units |
| Identifier | a String (32767) that holds `namespace:path` |
| UUID | 16 bytes: the high 64 bits, then the low 64 bits, both big-endian |
| Boolean | 1 byte: `0x00` false, `0x01` true |
| Byte | 1 signed byte |
| Unsigned Short | 2 bytes, big-endian |
| Byte Array | a VarInt byte count, then the bytes. The count is at most the number of bytes that remain in the packet |
| Optional X | a Boolean, then an X when the Boolean is true |
| Rest | all the bytes that remain in the packet |

### 0.3 Provenance and confidence

This document uses the provenance labels of `registry.md`, section 0.3. None of
the experiments in this document has been run. Each one is a proposal.

### 0.4 Artefacts cited

- `assets/mcrs/reports/packets.json`: the packet id report of `26.4-snapshot-4`.
  Its LOGIN section gives the packet ids of the 26.3 table (LOGIN-PKT-001,
  LOGIN-PKT-002). No artefact in the repository gives the 26.4 snapshot 5 table.

### 0.5 ID prefixes

| Prefix | Subject |
|--------|---------|
| `LOGIN-VER` | protocol versions and the switch mode of a connection |
| `LOGIN-HS` | the handshake that leads to LOGIN |
| `LOGIN-PKT` | packet ids and field layouts |
| `LOGIN-CFG` | server inputs |
| `LOGIN-SEQ` | the order of packets in a login |
| `LOGIN-SW` | the switch points of compression and the cipher layer |
| `LOGIN-OTH` | disconnect, custom queries, cookies, timeouts |
| `LOGIN-ERR` | errors and edge cases |

### 0.6 Scope

In scope:

- the handshake for intents 2 and 3;
- all LOGIN packets in both directions;
- the login sequences and the switch points;
- the server inputs that change them.

Out of scope:

- the STATUS state, and the CONFIGURATION state after its first frame;
- the HTTP form of the session server calls;
- the content of a custom query.

### 0.7 Names that appear in this document

The document names data, never code. It uses these names of the game:

- packet names from the packet report, for example `login_finished`;
- protocol type names (`VarInt`);
- keys of `server.properties`, for example `network-compression-threshold`;
- translation keys of disconnect reasons, for example
  `multiplayer.disconnect.slow_login`.

---

## 1. Versions and switch modes

### LOGIN-VER-001: Protocol versions

| Version | Protocol version | Hexadecimal |
|---------|------------------|-------------|
| 26.3 | `777` | `0x00000309` |
| 26.4 (snapshot 5) | `1073742167` | `0x40000157` |
| 26.4 (snapshot 4) | `1073742166` | `0x40000156` |

- **Behavior:** the client writes its protocol version in the handshake
  (LOGIN-HS-001).
- **Edge cases:** snapshot 4 of 26.4 uses the LOGIN packets and the switch mode
  of 26.3. This document does not require support for it.

Provenance: `source_suggested`. The snapshot 4 value is `black_box_confirmed`
(`assets/minecraft/version.json`). Confidence: high. Experiment: capture the
handshake of each vanilla client.

### LOGIN-VER-002: Switch mode of a connection

- **Inputs:** the protocol version in the handshake.
- **Behavior:**
  - For `777`, the connection uses the 26.3 packet table and the immediate switch.
  - For `1073742167`, the connection uses the 26.4 packet table and the acknowledged
    switch.
  - For another value, the server rejects the login (LOGIN-HS-003).
- **Behavior:** the server selects the mode one time, when it reads the handshake.
  The mode does not change for the life of the connection.
- **Behavior:** a client uses the mode of the protocol version that it sent.
- **Invariants:** a vanilla server accepts only its own protocol version. A server
  that accepts both versions selects the mode per connection.

Provenance: `source_suggested`. Confidence: high. Experiment: connect a 26.3
client to a 26.4 vanilla server. The server sends `login_disconnect` with
`multiplayer.disconnect.incompatible`.

## 2. Handshake

### LOGIN-HS-001: The `intention` packet

The handshake is the serverbound packet `intention`, id `0x00`, in the HANDSHAKE
state.

| Field | Type | 26.3 | 26.4 |
|-------|------|------|------|
| protocol version | VarInt | | |
| host | String | at most 255 on read | at most 1024 |
| port | Unsigned Short | | |
| intent | VarInt | `1`, `2` or `3` | `1`, `2` or `3` |

- **Behavior:** the intent values are the same in both versions:

| Value | Intent | Next state |
|-------|--------|------------|
| `1` | status | STATUS |
| `2` | login | LOGIN |
| `3` | transfer | LOGIN, with the transfer mark |

- **Edge cases:**
  - Another intent value is a decode error. The server closes the connection.
  - A 26.3 server rejects a host of more than 255 characters. A 26.4 server
    accepts up to 1024 characters.

Provenance: `source_suggested`. Confidence: high. Experiment: send a vanilla
server a handshake with intent `4`. The connection closes.

### LOGIN-HS-002: Transfer intent

- **Behavior:** for intent `3`, the server first checks the `accepts-transfers`
  property.
  - When it is false, the server sends `login_disconnect` in the LOGIN state with
    the reason `multiplayer.disconnect.transfers_disabled`. Then it closes the
    connection.
  - When it is true, the server continues as for intent `2`, with the transfer
    mark.
- **Behavior:** the transfer mark does not change the LOGIN packets or the switch
  points.

Provenance: `source_suggested`. Confidence: high. Experiment: send intent `3` to a
vanilla server with `accepts-transfers=false`.

### LOGIN-HS-003: Version check

- **Behavior:** for intent `2` or `3`, the server enters the LOGIN state for the
  clientbound direction. Then it checks the protocol version.
- **Behavior:** when the server does not accept the version, it sends
  `login_disconnect` and closes the connection. The reason is:
  - `multiplayer.disconnect.outdated_client` when the version is less than `754`;
  - `multiplayer.disconnect.incompatible` in all other cases.
- **Behavior:** both reasons have one argument, the name of the server's game
  version.
- **Behavior:** when the server accepts the version, the serverbound direction
  also enters the LOGIN state.

Provenance: `source_suggested`. Confidence: high. Experiment: send a vanilla
26.3 server a handshake with version `753`, then with version `778`.

### LOGIN-HS-004: Connection id filter (26.4 only)

- **Inputs:** the host field, and the server property `allowed-connection-ids`.
- **Behavior:** in 26.4, the host field can hold a query part after the first
  `?`. The query part is a list of `key=value` items, with `&` between them. Keys
  and values use URL form escapes in UTF-8. An item without `=` has an empty value.
- **Behavior:** the 26.4 client adds these keys:
  - `_id`, when the player writes the server address as `id@host`;
  - `_o`, with the text `host:port` that the player wrote, when name resolution
    gives a different host or port.
- **Behavior:** when `allowed-connection-ids` is not empty, the 26.4 server splits
  it at `,`, trims each item, and ignores empty items. It accepts the connection only
  when the value of `_id` is equal to one of these items.
- **Behavior:** when it does not accept the connection, it closes the connection
  without a packet. The server applies this check before it acts on the intent.
  So it applies to every intent.
- **Edge cases:** a 26.3 server treats the query part as a part of the host. A
  26.3 client never sends a query part.

Provenance: `source_suggested`. Confidence: medium. Experiment: set
`allowed-connection-ids=abc` on a 26.4 vanilla server. Connect with the address
`abc@host` and with the address `host`. Only the first connection logs in.

## 3. Packets

### LOGIN-PKT-001: Clientbound packet ids

| Id | 26.3 | 26.4 |
|----|------|------|
| `0x00` | `login_disconnect` | `login_disconnect` |
| `0x01` | `hello` | `key` |
| `0x02` | `login_finished` | `login_finished` (with `options`) |
| `0x03` | `login_compression` | `setup_compression` |
| `0x04` | `custom_query` | `custom_query` |
| `0x05` | `cookie_request` | `cookie_request` |
| `0x06` | — | `apply_options` |

- **Invariants:** the 26.4 `key` has the layout of the 26.3 clientbound `hello`.
  The 26.4 `setup_compression` has the layout of the 26.3 `login_compression`.
  Only the names changed.

Provenance: 26.3 ids `source_suggested_black_box_confirmed`
(`assets/mcrs/reports/packets.json`, from snapshot 4, which has the 26.3 table).
26.4 ids `source_suggested`. Confidence: high. Experiment: run the packet report
of the 26.4 snapshot 5 server, and compare its LOGIN section with this table.

### LOGIN-PKT-002: Serverbound packet ids

| Id | 26.3 | 26.4 |
|----|------|------|
| `0x00` | `hello` | `hello` |
| `0x01` | `key` | `key` |
| `0x02` | `custom_query_answer` | `custom_query_answer` |
| `0x03` | `login_acknowledged` | `login_acknowledged` |
| `0x04` | `cookie_response` | `cookie_response` |
| `0x05` | — | `options_applied` |

Provenance: as LOGIN-PKT-001. Confidence: high.

### LOGIN-PKT-003: Field layouts

The layouts are the same in both versions, except where a row names a version.

| Packet | Direction | Fields |
|--------|-----------|--------|
| `login_disconnect` | clientbound | reason: String (262144) that holds a JSON text component |
| `hello` (26.3), `key` (26.4) | clientbound | server id: String (20); public key: Byte Array; challenge: Byte Array; authenticate: Boolean |
| `login_finished` 26.3 | clientbound | profile; session id: UUID |
| `login_finished` 26.4 | clientbound | profile; session id: UUID; options: Byte (LOGIN-PKT-004) |
| `login_compression` (26.3), `setup_compression` (26.4) | clientbound | threshold: VarInt |
| `custom_query` | clientbound | transaction id: VarInt; channel: Identifier; data: Rest, at most 1048576 bytes |
| `cookie_request` | clientbound | key: Identifier |
| `apply_options` (26.4) | clientbound | options: Byte (LOGIN-PKT-004) |
| `hello` | serverbound | name: String (16); profile id: UUID |
| `key` | serverbound | encrypted shared secret: Byte Array; encrypted challenge: Byte Array |
| `custom_query_answer` | serverbound | transaction id: VarInt; answer: Optional Rest |
| `login_acknowledged` | serverbound | no field |
| `cookie_response` | serverbound | key: Identifier; payload: Optional Byte Array of at most 5120 bytes |
| `options_applied` (26.4) | serverbound | no field |

The profile in `login_finished` has this layout:

| Field | Type |
|-------|------|
| id | UUID |
| name | String (16) |
| property count | VarInt, at most 16 |
| each property: name | String (64) |
| each property: value | String (32767) |
| each property: signature | Optional String (1024) |

- **Edge cases:**
  - A vanilla server reads the answer of `custom_query_answer` as opaque bytes
    after the transaction id. The presence Boolean is one of these bytes. More than
    1048576 such bytes is a decode error.
  - A vanilla client reads the data of `custom_query` as opaque bytes and discards
    them.
  - A vanilla server ignores the profile id of the serverbound `hello`.

Provenance: `source_suggested`. Confidence: high. Experiment: capture an offline
login and an online login of each version. Decode each packet with this table,
and find no byte that remains.

### LOGIN-PKT-004: Option flags (26.4)

The `options` field of `apply_options` and of `login_finished` is a bit set in one
byte.

| Bit | Value | Layer |
|-----|-------|-------|
| 0 | `1` | the cipher layer |
| 1 | `2` | compression |

- **Behavior:** the vanilla client ignores bits 2 to 7.
- **Behavior:** a vanilla server sends only these values:
  - `apply_options`: always `1`;
  - `login_finished`: `2` when it sent `setup_compression` in this login, and `0`
    in all other cases. It never sets bit 0 in `login_finished`.
- **Conformance:** a server that conforms sends only the values of the vanilla
  server.

Provenance: `source_suggested`. Confidence: high. Experiment: capture a 26.4
online login with a threshold of 256, and with a threshold of -1. Read
`apply_options` = `0x01`, and `options` of `login_finished` = `0x02` and then
`0x00`.

### LOGIN-PKT-005: Terminal packets

- **Behavior:** `login_finished` is the last clientbound LOGIN packet.
  `login_acknowledged` is the last serverbound LOGIN packet.
- **Behavior:** after it receives a terminal packet, a receiver decodes no
  more frames of that direction in the LOGIN state. It keeps the bytes and decodes
  them in the CONFIGURATION state.
- **Behavior:** after it sends a terminal packet, a sender sends no more LOGIN
  packets in that direction. A disconnect after this point closes the connection
  without a packet.

Provenance: `source_suggested`. Confidence: medium. Experiment: a scripted client
sends `login_acknowledged` and the first CONFIGURATION packet in one TCP segment.
A vanilla server decodes the second packet as CONFIGURATION.

## 4. Server inputs

### LOGIN-CFG-001: Compression threshold

- **Behavior:** a dedicated server reads the threshold from the property
  `network-compression-threshold`. The default is `256`.
- **Behavior:** an integrated server uses `256`.
- **Behavior:** the server turns compression on for a login when both conditions
  are true:
  - the threshold is `0` or more;
  - the connection is not an in-process connection.
- **Behavior:** a negative threshold turns compression off. The server then sends
  no `login_compression` or `setup_compression`.
- **Behavior:** a 26.4 server reads the threshold one time, when the login starts.
  A 26.3 server reads it when it sends `login_compression`.

Provenance: `source_suggested`, `documentation` for the property and its default.
Confidence: high. Experiment: set `network-compression-threshold=-1` and log in.
The capture has no compression packet.

### LOGIN-CFG-002: Online mode

- **Behavior:** the server uses online mode for a login when all these conditions
  are true:
  - the property `online-mode` is true (the default);
  - the connection is not an in-process connection;
  - the name in `hello` is not equal to the owner's name. The comparison ignores
    case.
- **Behavior:** in all other cases, the server uses offline mode.
- **Behavior:** when the name is equal to the owner's name, the server uses the
  owner's profile. Otherwise, offline mode uses an offline profile (LOGIN-SEQ-001).

Provenance: `source_suggested`, `documentation` for the property. Confidence:
high. Experiment: start a dedicated server with `online-mode=false`, and capture a
login without `hello` from the server.

### LOGIN-CFG-003: In-process connection

- **Behavior:** an in-process connection uses no compression and no cipher layer.
- **Behavior:** in 26.4, `login_finished.options` is `0` on an in-process connection.
- **Behavior:** a 26.3 client ignores `login_compression` on an in-process
  connection. A vanilla server does not send it there.

Provenance: `source_suggested`. Confidence: medium. Experiment: not possible
with a capture, because an in-process connection has no socket. Confirm it with a
modified client that logs the options byte in single-player.

## 5. Sequences

### LOGIN-SEQ-001: Start of a login

- **Behavior:** the client sends `hello` as its first LOGIN packet.
- **Behavior:** the server checks the name. A name with more than 16 characters is an
  error (LOGIN-ERR-001). A character of code 32 or less, or of code 127 or more,
  is also an error.
- **Behavior:** in offline mode, the server makes the offline profile:
  - the id is the version 3 UUID of the UTF-8 bytes of `OfflinePlayer:` and the
    name;
  - the name is the name from `hello`;
  - the profile has no properties.
- **Behavior:** in online mode, the server sends the key data (LOGIN-SEQ-005).

Provenance: `source_suggested`, `documentation` for the offline UUID.
Confidence: high. Experiment: log in offline as `Steve`, and compare the UUID in
`login_finished` with the version 3 UUID of `OfflinePlayer:Steve`.

### LOGIN-SEQ-002: Verification and the end of LOGIN

- **Behavior:** after the server knows the profile, it does these steps at its
  next tick:
  1. It checks whether the player can join: ban lists, the whitelist and the
     player limit. On a failure, it sends `login_disconnect` with the reason of
     that check, and closes the connection.
  2. When compression is on (LOGIN-CFG-001), it sends `login_compression` (26.3)
     or `setup_compression` (26.4) with the threshold.
  3. When a player with the same profile id is already on the server, the server
     disconnects that player. It waits until that player has left.
  4. It sends `login_finished` with the profile and its session id.
- **Behavior:** the client answers `login_finished` with `login_acknowledged`.
  Then both directions enter the CONFIGURATION state (LOGIN-PKT-005).
- **Invariants:** the compression packet comes before `login_finished`. Step 3
  can put a delay between them.

Provenance: `source_suggested`. Confidence: high. Experiment: log in two times
with the same offline name. The first player leaves. The capture of the second
login shows the compression packet, a delay, and then `login_finished`.

### LOGIN-SEQ-003: Offline login

The table shows a login with compression on. The 26.3 and the 26.4 columns give
the state of each frame. "P" is plain, "C" is compressed, "U" is uncompressed.

| # | Direction | Packet | 26.3 | 26.4 |
|---|-----------|--------|------|------|
| 1 | serverbound | `intention` (HANDSHAKE) | P, U | P, U |
| 2 | serverbound | `hello` | P, U | P, U |
| 3 | clientbound | `login_compression` / `setup_compression` | P, U | P, U |
| 4 | clientbound | `login_finished` | P, **C** | P, U |
| 5 | serverbound | `login_acknowledged` | P, **C** | P, U |
| 6 | both | first CONFIGURATION packets | P, C | P, **C** |

- **Behavior:** with compression off, rows 3 to 6 are all U. Row 3 is absent.
  In 26.4, `login_finished.options` is `0`.

Provenance: `source_suggested`. Confidence: high. Experiment: capture an offline
login of each version with a threshold of 256.

### LOGIN-SEQ-004: Online login, 26.3

| # | Direction | Packet | Frame state |
|---|-----------|--------|-------------|
| 1 | serverbound | `intention` | plain, uncompressed |
| 2 | serverbound | `hello` | plain, uncompressed |
| 3 | clientbound | `hello` (key data) | plain, uncompressed |
| 4 | — | the client calls the session server to join | — |
| 5 | serverbound | `key` | plain, uncompressed |
| 6 | — | the server checks the challenge, then calls the session server | — |
| 7 | clientbound | `login_compression` | **encrypted**, uncompressed |
| 8 | clientbound | `login_finished` | encrypted, **compressed** |
| 9 | serverbound | `login_acknowledged` | encrypted, compressed |

- **Behavior:** the cipher layer is on before step 6. The session server check
  occurs while the connection is encrypted.

Provenance: `source_suggested`. Confidence: high. Experiment: a scripted 26.3
client with a known shared secret logs in to a vanilla server. Capture the
stream, and decrypt it from the frame after `key`.

### LOGIN-SEQ-005: Key exchange (both versions)

- **Behavior:** the server sends the key data in `hello` (26.3) or in `key` (26.4):
  - server id: the empty string;
  - public key: the server's RSA public key of 1024 bits, as a DER
    SubjectPublicKeyInfo structure;
  - challenge: 4 random bytes, new for each login;
  - authenticate: true.
- **Behavior:** the client makes a shared secret. It encrypts the secret and the
  challenge with the public key, with the RSA scheme `RSAES-PKCS1-v1_5`. It sends both
  in the serverbound `key`.
- **Behavior:** when authenticate is true, the client calls the session server to
  join before it sends `key`. The hash is the SHA-1 digest of these bytes, in
  this order:
  1. the server id, in ISO 8859-1;
  2. the shared secret;
  3. the public key in DER form.
- **Behavior:** the hash text is the digest as a signed two's complement
  integer in lower-case hexadecimal. A negative value has a `-` sign.
- **Behavior:** the server decrypts both values. When the challenge is not equal
  to its own challenge, or when a decryption fails, the login fails
  (LOGIN-ERR-005).
- **Behavior:** the server calls the session server with the name from `hello`
  and the same hash. When `prevent-proxy-connections` is true, it also sends the
  client's IP address.
- **Edge cases:** when the client's join call fails and the server is a LAN
  world, the client logs a message and continues. For other servers, the client
  closes the connection.

Provenance: `documentation`, `source_suggested`. Confidence: high. Experiment:
a scripted client logs in to a vanilla server. Compare its hash with the hash
that the server sends to a stub session server.

### LOGIN-SEQ-006: Online login, 26.4

| # | Direction | Packet | Frame state |
|---|-----------|--------|-------------|
| 1 | serverbound | `intention` | plain, uncompressed |
| 2 | serverbound | `hello` | plain, uncompressed |
| 3 | clientbound | `key` | plain, uncompressed |
| 4 | — | the client calls the session server to join | — |
| 5 | serverbound | `key` | plain, uncompressed |
| 6 | — | the server checks the challenge, then calls the session server | — |
| 7 | clientbound | `apply_options` (`0x01`) | plain, uncompressed |
| 8 | serverbound | `options_applied` | plain, uncompressed |
| 9 | clientbound | `setup_compression` | **encrypted**, uncompressed |
| 10 | clientbound | `login_finished` (`0x02`) | encrypted, uncompressed |
| 11 | serverbound | `login_acknowledged` | **encrypted**, uncompressed |
| 12 | both | first CONFIGURATION packets | encrypted, **compressed** |

- **Behavior:** the client does not switch the cipher layer on after it sends
  `key`. It keeps the shared secret until `apply_options` arrives.
- **Behavior:** the session server check occurs while the connection is plain.
- **Behavior:** the server sends `apply_options` only after a successful
  session server check. Then it waits for `options_applied`. It sends no other
  packet while it waits, except `login_disconnect` (LOGIN-ERR-003).
- **Behavior:** after `options_applied`, the server continues with the
  verification of LOGIN-SEQ-002.

Provenance: `source_suggested`. Confidence: high. Experiment: a scripted 26.4
client with a known shared secret logs in to a vanilla server. Capture the
stream, and decrypt the clientbound direction from the frame after
`apply_options`.

## 6. Switch points

### LOGIN-SW-001: Immediate switch, cipher layer (26.3)

- **Behavior:** the client switches the cipher layer on for both directions after
  it writes the serverbound `key`, and before it writes the next frame.
- **Behavior:** the server switches the cipher layer on for both directions when
  it reads `key`. It does this before it reads or writes the next frame.
- **Postconditions:** in each direction, `key` and the clientbound `hello` are the
  last plain frames. Every later frame is encrypted.

Provenance: `source_suggested`. Confidence: high. Experiment: as LOGIN-SEQ-004.

### LOGIN-SW-002: Immediate switch, compression (26.3)

- **Behavior:** the server switches compression on for both directions after it
  writes `login_compression`, and before it writes the next frame.
- **Behavior:** the client switches compression on for both directions when it
  reads `login_compression`. It does this before it reads or writes the next
  frame.
- **Postconditions:** `login_compression` is the last uncompressed clientbound
  frame. `login_finished` is compressed. `login_acknowledged` is compressed.
- **Note:** the vanilla server switches when the socket write of
  `login_compression` completes. When the socket is slow, the next frame can
  pass the compression step before that moment. A server that conforms must switch
  at the frame boundary, with no such race.

Provenance: `source_suggested`. Confidence: high. Experiment: as LOGIN-SEQ-003,
26.3 column.

### LOGIN-SW-003: Acknowledged switch, cipher layer (26.4)

- **Behavior:** the server sends `apply_options` with bit 0 set, as a plain frame.
- **Behavior:** when the client reads `apply_options`, it does these steps in
  this order:
  1. It switches the cipher layer on for the clientbound direction.
  2. It writes `options_applied` as a plain frame.
  3. It switches the cipher layer on for the serverbound direction.
- **Behavior:** the server switches the cipher layer on for both directions when
  it reads `options_applied`. It does this before it reads or writes the next
  frame.
- **Postconditions:** `apply_options` is the last plain clientbound frame.
  `options_applied` is the last plain serverbound frame.

Provenance: `source_suggested`. Confidence: high. Experiment: as LOGIN-SEQ-006.

### LOGIN-SW-004: Acknowledged switch, compression (26.4)

- **Behavior:** the client stores the threshold of `setup_compression`. It does
  not switch compression on.
- **Behavior:** the server sends `login_finished` with bit 1 set, as an
  uncompressed frame.
- **Behavior:** when the client reads `login_finished`, it does these steps in
  this order:
  1. It switches compression on for the clientbound direction, with the stored
     threshold.
  2. It writes `login_acknowledged` as an uncompressed frame.
  3. It switches compression on for the serverbound direction, with the stored
     threshold.
- **Behavior:** the server switches compression on for both directions when it
  reads `login_acknowledged`. It does this before it reads or writes the next
  frame.
- **Postconditions:** `login_finished` is the last uncompressed clientbound frame.
  `login_acknowledged` is the last uncompressed serverbound frame. The first
  CONFIGURATION frame in each direction is compressed.

Provenance: `source_suggested`. Confidence: high. Experiment: as LOGIN-SEQ-003,
26.4 column.

### LOGIN-SW-005: Summary of the switch points

| Version | Layer | Direction | Last frame without the layer | Client switches | Server switches |
|---------|-------|-----------|------------------------------|-----------------|-----------------|
| 26.3 | cipher | clientbound | `hello` | after it writes `key` | when it reads `key` |
| 26.3 | cipher | serverbound | `key` | after it writes `key` | when it reads `key` |
| 26.3 | compression | clientbound | `login_compression` | when it reads `login_compression` | after it writes `login_compression` |
| 26.3 | compression | serverbound | the last frame before the client reads `login_compression` | when it reads `login_compression` | after it writes `login_compression` |
| 26.4 | cipher | clientbound | `apply_options` | when it reads `apply_options` | when it reads `options_applied` |
| 26.4 | cipher | serverbound | `options_applied` | after it writes `options_applied` | when it reads `options_applied` |
| 26.4 | compression | clientbound | `login_finished` | when it reads `login_finished` | when it reads `login_acknowledged` |
| 26.4 | compression | serverbound | `login_acknowledged` | after it writes `login_acknowledged` | when it reads `login_acknowledged` |

Provenance: `source_suggested`. Confidence: high.

### LOGIN-SW-006: What the 26.4 client accepts

- **Behavior:** the client applies the option flags of `apply_options` and of
  `login_finished` with the same three steps. For each set bit, it switches the
  clientbound direction first. Then it writes its acknowledgement. Then it
  switches the serverbound direction.
- **Behavior:** the client applies compression with the last threshold that it
  stored. When no `setup_compression` arrived, that threshold is `0`.
- **Behavior:** the client treats a compressed clientbound frame with the lenient
  check of `compression.md`, FRAME-008. The server uses the strict check,
  FRAME-007.
- **Edge cases:**
  - Bit 0 without a shared secret, for example in offline mode, is an error. The
    client closes the connection.
  - Bit 0 when the cipher layer is already on is an error on the vanilla client.
- **Conformance:** a server that conforms never sends these cases (LOGIN-PKT-004).

Provenance: `source_suggested`. Confidence: medium. Experiment: a scripted 26.4
server sends `login_finished` with `0x02` and no `setup_compression`. The vanilla
client then compresses every serverbound packet.

## 7. Other LOGIN packets

These rules are the same in both versions.

### LOGIN-OTH-001: `login_disconnect`

- **Behavior:** the server sends `login_disconnect` with a reason, and then closes
  the connection. The client shows the reason and closes the connection.
- **Behavior:** the frame of `login_disconnect` uses the layers that are on for
  the clientbound direction at that moment.

Provenance: `source_suggested`. Confidence: high. Experiment: log in to a
whitelisted vanilla server with a name that is not on the whitelist.

### LOGIN-OTH-002: Custom query

- **Behavior:** a server can send `custom_query` with a transaction id, a channel
  and data. The vanilla server never sends it.
- **Behavior:** the vanilla client answers each `custom_query` with
  `custom_query_answer`. The answer has the same transaction id and no data (the
  Boolean is false).
- **Behavior:** when the vanilla server receives `custom_query_answer`, it sends
  `login_disconnect` with `multiplayer.disconnect.unexpected_query_response`.

Provenance: `source_suggested`. Confidence: high. Experiment: a scripted server
sends `custom_query` with id 7 to a vanilla client. The answer is `0x07 0x00`
after the packet id.

### LOGIN-OTH-003: Cookies

- **Behavior:** a server can send `cookie_request` with a key. The vanilla server
  never sends it in LOGIN.
- **Behavior:** the vanilla client answers with `cookie_response`. The payload is
  the cookie that the client stored for that key in an earlier transfer. When it
  has no such cookie, the payload is absent.
- **Behavior:** when the vanilla server receives `cookie_response` in LOGIN, it
  sends `login_disconnect` with `multiplayer.disconnect.unexpected_query_response`.

Provenance: `source_suggested`. Confidence: high. Experiment: a scripted server
sends `cookie_request` for `a:b` to a vanilla client that has no cookie. The
answer has the key `a:b` and the Boolean false.

### LOGIN-OTH-004: Login timeout

- **Behavior:** the server counts its ticks from the start of the LOGIN state.
  At tick 600, when the login has not ended, it sends `login_disconnect` with
  `multiplayer.disconnect.slow_login`. Then it closes the connection.
- **Behavior:** the count includes the time of the session server check and of the
  wait for a duplicate player.
- **Behavior:** independent of this count, each side closes the connection when it
  reads no byte for 30 seconds. It sends no packet in that case. The reason is
  `disconnect.timeout`.
- **Note:** 600 ticks is 30 seconds when the server is not overloaded.

Provenance: `source_suggested`. Confidence: high. Experiment: a scripted client
sends `hello` to an online vanilla server, and then sends nothing. Measure the
time to `login_disconnect`.

## 8. Errors and edge cases

### LOGIN-ERR-001: Packet in the wrong state

- **Behavior:** the vanilla server accepts each serverbound LOGIN packet only at
  one point of the login:

| Packet | Accepted when |
|--------|---------------|
| `hello` | it is the first LOGIN packet |
| `key` | the server sent the key data and waits for `key` |
| `options_applied` (26.4) | the server sent `apply_options` and waits for the acknowledgement |
| `login_acknowledged` | the server sent `login_finished` |
| `custom_query_answer`, `cookie_response` | never (LOGIN-OTH-002, LOGIN-OTH-003) |

- **Behavior:** at any other point, the packet is an error. The server sends
  `login_disconnect` with `disconnect.genericReason` and one argument. Then it
  closes the connection.
- **Behavior:** an invalid name in `hello` (LOGIN-SEQ-001) has the same result.
- **Edge cases:** the 26.4 server switches compression on before it checks
  `login_acknowledged` (LOGIN-ERR-002).

Provenance: `source_suggested`. Confidence: high. Experiment: send `key` to an
offline vanilla server after `hello`.

### LOGIN-ERR-002: Premature `login_acknowledged` (26.4)

- **Behavior:** when the 26.4 server receives `login_acknowledged`, it first
  applies the compression bit of its `login_finished` options. This occurs before
  it checks the point of the login. The value of the bit does not depend on
  whether it sent `login_finished`.
- **Behavior:** so, when compression is on for this login and `login_finished` is
  not sent yet, the server does these steps:
  1. It switches compression on for both directions, with the server threshold.
  2. It finds the error of LOGIN-ERR-001.
  3. It sends `login_disconnect` as a compressed frame.
  4. It closes the connection.
- **Postconditions:** the client did not switch compression on. It cannot read the
  `login_disconnect` correctly. It sees an error or a closed connection.
- **Edge cases:** when compression is off for the login, the server switches
  nothing. The result is that of LOGIN-ERR-001. A 26.3 server switches nothing
  either.
- **Conformance:** a server that conforms must check the point of the login first. It
  must reject a premature `login_acknowledged` as in LOGIN-ERR-001, with a frame
  that the client can read. The client cannot see the difference, except for a
  readable reason.

Provenance: `source_suggested`. Confidence: medium. Experiment: a scripted 26.4
client sends `hello` and then `login_acknowledged` at once to an offline vanilla
server with threshold 256. Capture the clientbound frame. It starts with a data
length field.

### LOGIN-ERR-003: Absent `options_applied` (26.4)

- **Behavior:** after `apply_options`, the server waits for `options_applied`. It
  does not verify the player and does not send `setup_compression` or
  `login_finished` in this time.
- **Behavior:** when `options_applied` does not arrive, the login timeout of
  LOGIN-OTH-004 ends the login. The server sends `login_disconnect` as a plain
  frame, because its cipher layer is still off.
- **Edge cases:** a client that already switched its clientbound cipher layer on
  cannot read this `login_disconnect`. It sees an error or a closed connection.
- **Edge cases:** the 30-second read limit of LOGIN-OTH-004 can close the
  connection first, without a packet.

Provenance: `source_suggested`. Confidence: medium. Experiment: a scripted 26.4
client completes the key exchange and does not answer `apply_options`. Measure
the time to the close, and capture the plain `login_disconnect`.

### LOGIN-ERR-004: Disconnect between the sides of a switch (26.4)

- **Behavior:** between `apply_options` and `options_applied`, the server sends a
  `login_disconnect` plain (LOGIN-ERR-003).
- **Behavior:** after the server sends `login_finished`, it sends no more LOGIN
  packets (LOGIN-PKT-005). A timeout in this time closes the connection without
  a packet.
- **Postconditions:** in 26.4, no compressed frame reaches a client that did not
  switch compression on, except in LOGIN-ERR-002.

Provenance: `source_suggested`. Confidence: medium. Experiment: a scripted 26.4
client reads `login_finished` and does not answer. The connection closes after
the timeout with no further frame.

### LOGIN-ERR-005: Key and authentication failures

| Failure | Reason in `login_disconnect` | 26.3 frame | 26.4 frame |
|---------|------------------------------|------------|------------|
| the challenge does not match, or a decryption fails | `disconnect.genericReason` | encrypted | plain |
| the session server does not confirm the join | `multiplayer.disconnect.unverified_username` | encrypted | plain |
| the session server is not available | `multiplayer.disconnect.authservers_down` | encrypted | plain |

- **Behavior:** an integrated server does not reject the last two failures. It
  continues with the offline profile of the name.
- **Behavior:** in 26.4, the integrated server then sends `apply_options` as for a
  successful check.

Provenance: `source_suggested`. Confidence: medium. Experiment: point a vanilla
server at a stub session server that answers with no content. Capture the
`login_disconnect`, and check whether it is encrypted.

---

## 9. Configuration points (non-normative)

This section suggests configuration points for mcrs. It is not a rule.

- **Compression threshold.** One integer per server, with the same function as
  `network-compression-threshold`. The default is `256`. A negative value turns
  compression off for all versions.
- **Accepted versions.** A set of protocol versions that the server accepts. The
  default is `{777, 1073742167}`. The server selects the switch mode from the
  handshake (LOGIN-VER-002). A version outside the set gets the reject of
  LOGIN-HS-003.
- **Version name in a reject.** The text for the argument of
  `multiplayer.disconnect.incompatible`. One example is `26.3 or 26.4`.
- **Client version.** A client configuration point that selects the protocol version that it
  sends, `777` or `1073742167`. The client then uses the switch mode of that
  version.
- **Online mode.** One boolean, with the same function as `online-mode`.
- **Connection ids.** One list, with the same function as `allowed-connection-ids`.
  It applies only to 26.4 connections, because a 26.3 client sends no `_id`.

## 10. Open questions

- **LQ-01: 26.4 packet report** (LOGIN-PKT-001, LOGIN-PKT-002). Run the packet
  report of the 26.4 snapshot 5 server, and store it as an artefact. Then change
  the labels of the 26.4 ids.
- **LQ-02: Handshake decode errors in 26.3** (LOGIN-HS-001). It is not confirmed
  whether a 26.3 server sends any bytes before it closes the connection on an
  unknown intent. A 26.4 server sends none. Experiment: capture the clientbound
  bytes after a handshake with intent `4` to a 26.3 server.
- **LQ-03: Owner shortcut on a LAN world** (LOGIN-CFG-002). The name check ignores
  case. Confirm that a LAN guest with the owner's name in other case gets the
  owner's profile without encryption.
- **LQ-04: Captures** (all rules). Capture the sequences of LOGIN-SEQ-003,
  LOGIN-SEQ-004 and LOGIN-SEQ-006 with vanilla programs, and store them as
  artefacts.

## 11. Conflicts with `crates/mcrs_minecraft_protocol/README.md`

- The README says that the crate supports exactly one game version. This
  document requires two packet tables and two switch modes on one server and on
  one client (LOGIN-VER-002).
- The README example uses the packet name `login_compression` only. In 26.4 the
  name of packet `0x03` is `setup_compression` (LOGIN-PKT-001). The layout did
  not change.
- The README says that the game "inflates the declared count and ignores the
  rest". The game also rejects a zlib stream that gives fewer bytes than the data
  length (`compression.md`, FRAME-007, step 5). This is not a conflict with the
  mcrs rule, which is stricter.
- The README states that mcrs applies the strict check on the client as well.
  That is a permitted difference (`compression.md`, FRAME-008).
