# Frames, compression and the cipher layer: behavioural specification

| Field | 26.3 | 26.4 |
|-------|------|------|
| Game version | `26.3` (release) | `26.4-snapshot-5` (Minecraft Java Edition, 26.4 Snapshot 5) |
| Protocol version | `777` | `1073742167` |

This document states how the game cuts a TCP byte stream into frames, how it
compresses a frame, and how it encrypts the stream. These rules are the same in
26.3 and in 26.4. The rules for 26.4 come from snapshot 5.

The rules here apply in every connection state. The LOGIN state turns compression
and encryption on. The moment of that switch differs between the versions, and
`login.md` states it (LOGIN-SW-001 to LOGIN-SW-006).

## 0. Conventions

### 0.1 Terms

- **Direction**: one of the two byte streams of a connection. The clientbound
  direction goes from the server to the client. The serverbound direction goes from
  the client to the server.
- **Sender**, **receiver**: the side that writes a direction and the side that
  reads it.
- **VarInt**: a signed 32-bit integer in 1 to 5 bytes. Each byte holds 7 data bits,
  the low group first. The high bit of a byte is set when one more byte follows.
- **Packet**: a VarInt packet id, then the fields of the packet.
- **Packet size**: the number of bytes of a packet, the packet id included.
- **Frame**: a length prefix, then the frame body.
- **Length prefix**: a VarInt that gives the number of bytes of the frame body.
- **Frame body**: the bytes that the length prefix counts.
- **Compression**: a state of one direction. When compression is on, each frame
  body has the compressed form of FRAME-005. When compression is off, the frame
  body is the packet.
- **Threshold**: a signed integer that the server announces. It sets which packets
  the sender compresses (FRAME-006).
- **Data length**: the VarInt at the start of a frame body when compression is on.
- **zlib stream**: a compressed byte sequence in the format of RFC 1950, with
  DEFLATE data (RFC 1951) inside it.
- **Cipher layer**: a state of one direction. When the cipher layer is on, all
  bytes of the direction go through AES-128 in CFB8 mode (FRAME-010).
- **Strict check**, **lenient check**: the two sets of receiver checks for a
  compressed frame (FRAME-007, FRAME-008).

### 0.2 Provenance and confidence

This document uses the provenance labels of `registry.md`, section 0.3. None of
the experiments in this document has been run. Each one is a proposal.

### 0.3 Artefacts cited

No game-produced artefact in this repository shows a frame. A packet capture of a
vanilla connection confirms each rule here. Section 4 lists the captures to make.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `FRAME` | frame layout, compression, the cipher layer |

### 0.5 Names that appear in this document

The document names data, never code. It uses protocol type names (`VarInt`),
packet names from the packet report (for example `login_finished`), and the names
of public formats (RFC 1950, AES, CFB8).

---

## 1. Layers

### FRAME-001: Order of the layers

- **Behavior:** a sender applies these steps to each packet, in this order:
  1. It writes the packet (FRAME-002).
  2. When compression is on, it makes the compressed frame body (FRAME-005,
     FRAME-006).
  3. It writes the length prefix before the frame body (FRAME-003).
  4. When the cipher layer is on, it encrypts the bytes of the frame (FRAME-010).
- **Behavior:** a receiver applies the inverse steps in the inverse order.
- **Invariants:**
  - The cipher layer encrypts the length prefix too. A receiver decrypts the stream
    before it reads a length prefix.
  - Compression and the cipher layer are independent. Each direction has its own
    compression state and its own cipher state.
  - A side switches a layer on only between two frames. A frame is never part
    plain and part encrypted, or part compressed and part uncompressed.

Provenance: `source_suggested`. Confidence: high. Experiment: capture an online
login with a threshold of 256. Decrypt the stream after the switch point of
LOGIN-SW-003, and find a length prefix at the start of each decrypted frame.

### FRAME-002: Packet

- **Behavior:** a packet is a VarInt packet id, then the fields of that packet in
  the current state and direction.
- **Edge cases:**
  - A receiver rejects a packet with bytes after the last field. It closes the
    connection (FRAME-009).
  - A receiver rejects a packet id that the current state and direction do not
    define (FRAME-009).

Provenance: `source_suggested`. Confidence: high. Experiment: send a vanilla
server a LOGIN `hello` with one extra byte after the UUID. The server closes the
connection.

## 2. Frames

### FRAME-003: Length prefix

- **Inputs:** the bytes of one direction, after the cipher layer.
- **Behavior:**
  - The length prefix is a VarInt of 1, 2 or 3 bytes. The largest frame body is
    therefore 2 097 151 bytes.
  - The receiver reads the frame body when all its bytes have arrived. Until then,
    it keeps the bytes and reads nothing.
- **Edge cases:**
  - A length prefix with the continuation bit set in its third byte is an error.
    The receiver closes the connection (FRAME-009).
  - A length prefix of 0 is an error. The receiver closes the connection.
  - A sender does not write a frame body of more than 2 097 151 bytes. The send
    fails, and the sender closes the connection.

Provenance: `source_suggested`. Confidence: high. Experiment: send the bytes
`0x00` to a vanilla server as the first frame. The server closes the connection.
Then send `0x80 0x80 0x80 0x01`, which also closes it.

### FRAME-004: Frame when compression is off

- **Behavior:** the frame body is the packet, with no other byte.

| Field | Type | Notes |
|-------|------|-------|
| length | VarInt (1 to 3 bytes) | the packet size |
| packet | bytes | FRAME-002 |

Provenance: `documentation`, `source_suggested`. Confidence: high. Experiment:
capture the `intention` packet of a vanilla client. The first byte gives the
size of the rest.

### FRAME-005: Frame when compression is on

- **Behavior:** the frame body starts with the data length.

| Field | Type | Notes |
|-------|------|-------|
| length | VarInt (1 to 3 bytes) | the size of the data length field and of the data, together |
| data length | VarInt | 0, or the packet size before compression |
| data | bytes | when data length is 0: the packet, not compressed. Otherwise: a zlib stream of the packet |

- **Invariants:** the data length counts the bytes of the packet before
  compression. The length prefix counts the bytes after compression.

Provenance: `documentation`, `source_suggested`. Confidence: high. Experiment:
capture the first CONFIGURATION frames after a login with a threshold of 256. A
short packet starts with a data length of 0. A large packet, for example
`registry_data`, has a data length that is equal to its inflated size.

### FRAME-006: What the sender compresses

- **Inputs:** the packet size `n`, and the threshold `t` of the direction.
- **Behavior:**
  - When `n` is equal to `t` or more than `t`, the sender writes `n` as the data
    length. Then it writes a zlib stream of the packet.
  - When `n` is less than `t`, the sender writes a data length of 0. Then it writes
    the packet.
- **Edge cases:**
  - A threshold of 0 compresses every packet.
  - A packet size of more than 8 388 608 bytes is an error on the sender. The
    send fails, and the sender closes the connection.
  - A compressed packet that makes a frame body of more than 2 097 151 bytes also
    fails (FRAME-003).
  - The game uses the default compression level of zlib. A receiver must not
    depend on the level. Any valid zlib stream is correct.

Provenance: `source_suggested`. Confidence: high. Experiment: set
`network-compression-threshold=64` on a vanilla server. Capture packets of sizes
63 and 64 in CONFIGURATION. The first has a data length of 0. The second has a
data length of 64.

### FRAME-007: Strict check of a compressed frame (server receiver)

- **Inputs:** a frame body with a data length `d`, and the threshold `t`.
- **Behavior:** a vanilla server applies these checks to each serverbound frame:
  1. When `d` is 0, the rest of the frame body is the packet. The receiver does
     not compare its size with `t`. An uncompressed packet can be larger than `t`.
  2. When `d` is not 0 and `d` is less than `t`, the frame is an error.
  3. When `d` is more than 8 388 608, the frame is an error.
  4. Otherwise, the receiver inflates the zlib stream and keeps the first `d`
     bytes.
  5. When the zlib stream gives fewer than `d` bytes, the frame is an error.
- **Edge cases:**
  - When the zlib stream gives more than `d` bytes, the receiver does not read
    them. It keeps the first `d` bytes and does not report an error.
  - Bytes after the end of the zlib stream are not read and are not an error.
  - A negative `d` is less than every threshold of 0 or more, so check 2 rejects
    it.
- **Postconditions:** an error closes the connection (FRAME-009).

Provenance: `source_suggested`. Confidence: medium. Experiment: log in with
threshold 256. Then send a vanilla server a `client_information` packet with data
length 20 and a zlib stream. The server closes the
connection. Send the same packet with data length 0, and it accepts the packet.

### FRAME-008: Lenient check of a compressed frame (client receiver)

- **Behavior:** a vanilla client applies only steps 1, 4 and 5 of FRAME-007 to
  each clientbound frame.
- **Edge cases:**
  - A vanilla client accepts a compressed packet with a data length less than the
    threshold.
  - A vanilla client does not compare the data length with 8 388 608.
- **Conformance:** a client that conforms can apply the strict check of FRAME-007. A
  vanilla server never sends a frame that the strict check rejects.

Provenance: `source_suggested`. Confidence: medium. Experiment: a scripted server
with threshold 256 sends the vanilla client a short CONFIGURATION packet with a
data length of 10 and a zlib stream. The client accepts it.

### FRAME-009: Result of a frame error

- **Behavior:** a frame error, a packet error of FRAME-002, or a send error of
  FRAME-003 or FRAME-006 closes the connection.
- **Behavior:** when the server finds the error in the LOGIN state, it first tries
  to send `login_disconnect`. The reason is the translation key
  `disconnect.genericReason` with one argument. The text of that argument is not
  part of this specification.
- **Edge cases:** after the server has sent `login_finished`, it sends no more
  LOGIN packet (LOGIN-PKT-005). It then closes the connection with no packet.

Provenance: `source_suggested`. Confidence: medium. Experiment: send a vanilla
server a LOGIN `hello` with a 17-character name. Capture a `login_disconnect`
before the connection closes.

## 3. Cipher layer

### FRAME-010: Cipher

- **Inputs:** the shared secret, a 16-byte AES key that the client makes
  (LOGIN-SEQ-005).
- **Behavior:**
  - The cipher is AES-128 in CFB8 mode. The mode adds no bytes.
  - The key is the shared secret. The initial vector is also the shared secret.
  - Each direction has one cipher state. That state continues from the first
    encrypted byte until the connection closes.
  - The two directions start from the same key and the same initial vector. Their
    cipher states are independent.
- **Invariants:** the cipher layer does not change the number of bytes.

Provenance: `documentation`, `source_suggested`. Confidence: high. Experiment:
use a scripted client with a known shared secret. Decrypt the clientbound stream
after the switch point, and read a valid length prefix and packet.

### FRAME-011: Switch of a layer

- **Behavior:** a side switches compression or the cipher layer for one direction
  at a frame boundary. The first frame after the boundary uses the new state. The
  last frame before it uses the old state.
- **Behavior:** when a side changes the threshold of a direction that has
  compression on, the new threshold applies from the next frame.
- **Behavior:** the 26.3 client turns compression off for both directions when it
  receives a negative threshold in `login_compression`. A vanilla server never
  sends a negative threshold.

Provenance: `source_suggested`. Confidence: medium. Experiment: a scripted server
sends `login_compression` two times, with 256 and then 64. Capture the next
serverbound frames of sizes from 64 to 255. They are compressed.

---

## 4. Open questions

- **FQ-01: Captures** (all rules). No capture in the repository confirms these
  rules. Make these captures with a vanilla client and a vanilla server for each
  version:
  - an offline login with a threshold of 256;
  - an online login with a threshold of 256;
  - an offline login with a threshold of -1.

  Store them as artefacts, and change the labels to confirmed labels.
- **FQ-02: Sender behaviour at the limits** (FRAME-003, FRAME-006). The exact
  bytes that a vanilla sender writes before it closes the connection on a send
  error are not known. An implementation must not depend on them.
