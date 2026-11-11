# Bedrock batch and compression: behavioural specification

| Field | Value |
|-------|-------|
| Edition | Bedrock |
| Protocol version | `729` |
| Game version | `1.21.30` (see `bedrock-wire.md`, Q1) |

This document states the batch of the Bedrock Edition game protocol at protocol 729. A batch
carries one or more packets in one transport message. This document also states the compression
of a batch and the switch to compression. The primitive types and the packet header are in
`bedrock-wire.md`. The packet that sets compression is in `bedrock-handshake.md`.

The transport below the batch is RakNet. The cipher layer is between the batch and RakNet. This
document states neither of them. Section 4 states only where the cipher layer starts.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **sender**, **receiver**, **packet**, **varuint32**, **decode error**:
  as `bedrock-wire.md`, section 0.1, defines them.
- **Transport message**: one complete message that the RakNet layer gives to the receiver, or
  that the sender gives to the RakNet layer.
- **Batch**: a transport message whose first byte is `0xfe`.
- **Batch body**: the bytes of a batch after the byte `0xfe` and after the cipher layer.
- **Packet list**: a sequence of packets, each with a varuint32 length before it (BEB-FRAME-002).
- **Marker**: the first byte of a batch body after the switch. It selects the compression of
  the batch (BEB-CMP-002).
- **Switch**: the point after which each batch of a connection has a marker (BEB-CMP-001).
- **Algorithm**: the compression algorithm that `NetworkSettings` selects for the connection.
- **Threshold**: the value of `NetworkSettings` that sets the smallest packet list to compress.
- **Raw deflate**: the DEFLATE format of RFC 1951, with no zlib header and no checksum.
- **Snappy block**: the block format of Snappy, with no frame format.
- **Inflate limit**: the largest packet list that a receiver makes from one compressed batch.

### 0.2 Notation and provenance

The notation, the provenance labels, the sources and the experiment set-up are as
`bedrock-wire.md`, sections 0.2 to 0.4, defines them. Each rule has `Edition: bedrock`.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `BEB-FRAME` | the batch and the packet list |
| `BEB-CMP` | the marker, the compression and the switch |
| `BEB-ERR` | errors |

## 1. The batch

### BEB-FRAME-001: Batch byte

- **Behavior:**
  - Each transport message of the game protocol starts with the byte `0xfe`.
  - The rest of the transport message is the batch body, after the cipher layer when it is on.
- **Edge cases:**
  - A transport message with another first byte is not a batch. A receiver closes the
    connection (BEB-ERR-001).
  - An empty transport message is not a batch. A receiver discards it.

Provenance: `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEB-FRAME-002: Packet list

- **Behavior:**
  - A packet list is one or more entries. An entry is a varuint32 length, then that number of
    bytes.
  - The bytes of an entry are one packet: its header and its payload (`bedrock-wire.md`,
    BEW-HDR-001).
  - The entries end at the last byte of the packet list. There is no count and no terminator.
  - The receiver handles the packets of one packet list in their order.
- **Edge cases:**
  - An entry length of 0 is a decode error.
  - An entry length that is larger than the number of bytes that remain is a decode error.
  - A packet list with no entry is a decode error.
- **Fixtures:**
  - A packet list with one `PlayStatus` packet (status 0) is `05 02 00 00 00 00`.
  - A packet list with two `PlayStatus` packets (status 0, then status 3) is
    `05 02 00 00 00 00 05 02 00 00 00 03`.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEB-FRAME-003: Packets in one batch

- **Behavior:**
  - A sender can put more than one packet in one batch.
  - A sender must put the packets of one batch in the order in which the receiver must handle
    them.
  - A sender must not put a packet after `NetworkSettings` in the same batch (BEB-CMP-001).

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Edition: bedrock.

## 2. Compression

### BEB-CMP-001: The switch

- **Behavior:**
  - Before the switch, a batch body is a packet list with no marker and no compression.
  - The server sends `NetworkSettings` in a batch before the switch. That batch holds no other
    packet.
  - The server makes the switch for both directions directly after it sends that batch.
  - The client makes the switch for both directions directly after it receives that batch.
  - After the switch, each batch body in each direction starts with a marker (BEB-CMP-002).
- **Fixtures:**
  - The first batch of a client, before the switch, is `fe 06 c1 01 00 00 02 d9`. It holds
    `RequestNetworkSettings` with protocol 729.
  - The batch of `NetworkSettings` with threshold 512 and algorithm 0 is
    `fe 0c 8f 01 00 02 00 00 00 00 00 00 00 00`. *(constructed)*

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.
Experiment: capture the first four batches of a login to BDS 1.21.30. The first two batches
have no marker. The third and the fourth batches start with a marker.

### BEB-CMP-002: Marker

- **Behavior:**
  - The marker is one byte. The rest of the batch body depends on its value:

| Marker | Rest of the batch body |
|--------|------------------------|
| `0x00` | a raw deflate stream of the packet list (BEB-CMP-004) |
| `0x01` | a Snappy block of the packet list (BEB-CMP-005) |
| `0xff` | the packet list with no compression |

  - A receiver accepts each of the three markers in each batch after the switch. It does not
    require the marker of the algorithm.
  - A sender writes only the marker of the algorithm or the marker `0xff` (BEB-CMP-003).
- **Edge cases:**
  - Another marker value is a decode error. The receiver closes the connection (BEB-ERR-001).
- **Fixtures:**
  - `fe ff 05 02 00 00 00 00` is a batch with marker `0xff` and one `PlayStatus` packet.
  - `fe 00 63 65 62 00 02 00` is the same packet list, in raw deflate at level 6.
    *(constructed)*
  - `fe 01 06 14 05 02 00 00 00 00` is the same packet list, in a Snappy block with one literal.
    *(constructed)*

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEB-CMP-003: Threshold

- **Inputs:** the threshold T and the algorithm A of `NetworkSettings`, and the size S in bytes
  of the packet list of one batch.
- **Behavior:**
  - The sender compresses the packet list with A when S is T or more. The marker is the marker
    of A.
  - The sender does not compress the packet list when S is less than T. The marker is `0xff`.
  - A threshold of 0 turns compression off. The sender writes the marker `0xff` on each batch.
  - A threshold of 1 makes the sender compress each batch.
  - The algorithm value `0xffff` turns compression off. The sender writes the marker `0xff` on
    each batch.
  - The threshold and the algorithm apply to both directions.

| Algorithm value | Algorithm | Marker |
|-----------------|-----------|--------|
| `0x0000` | raw deflate | `0x00` |
| `0x0001` | Snappy | `0x01` |
| `0xffff` | none | `0xff` |

- **Edge cases:**
  - A server selects the threshold and the algorithm. The value is a server input of the
    handshake (`bedrock-handshake.md`, BEH-CFG-001).

Provenance: `documentation`. Kind: wire. Confidence: medium. Edition: bedrock. Experiment: set
`compression-threshold` of BDS to 256. Capture a login. Each batch of fewer than 256 bytes from
BDS has the marker `0xff`. Each larger batch has the marker `0x00`. Repeat the capture with the
threshold 0.

### BEB-CMP-004: Raw deflate

- **Behavior:**
  - The algorithm with the value 0 is raw deflate. The public documentation names it "ZLib", but
    the stream has no zlib header and no Adler-32 checksum.
  - A sender can use any compression level. A receiver accepts each valid raw deflate stream.
  - The stream must end with a final block. Bytes after the final block are a decode error.
- **Edge cases:**
  - A stream with a zlib header (for example `78 9c`) is not raw deflate. It is a decode error.

Provenance: `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock. Experiment:
capture one batch of BDS with the marker `0x00`. Inflate the rest of the batch body as raw
deflate. The result is a valid packet list.

### BEB-CMP-005: Snappy

- **Behavior:**
  - The algorithm with the value 1 is a Snappy block: a varuint32 of the uncompressed size, then
    the Snappy elements.
  - The batch body has no Snappy frame header and no checksum.
- **Edge cases:**
  - An uncompressed size that is larger than the inflate limit is a decode error. The receiver
    checks it before it allocates memory.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Edition: bedrock. Experiment:
set the compression algorithm of BDS to Snappy (`compression-algorithm=snappy`). Capture one
batch with the marker `0x01`. Decode it as a Snappy block.

### BEB-CMP-006: Inflate limit

- **Inputs:** the inflate limit L. The default of L is 10 485 760 bytes (10 MiB).
- **Behavior:**
  - A receiver stops the decompression of a batch when the output reaches L bytes and more
    output remains.
  - That batch is a decode error. The receiver closes the connection (BEB-ERR-001).
  - The receiver does not allocate more than L bytes for the output of one batch.
- **Edge cases:**
  - A packet list of exactly L bytes is valid.
  - The limit applies to raw deflate and to Snappy. It does not apply to the marker `0xff`,
    because the transport message already holds that packet list.

Provenance: `design`. Kind: semantic. Confidence: medium. Edition: bedrock. Reason: a small
compressed batch can inflate to a very large packet list. The default is the default of the
CloudburstMC codec that GeyserMC uses. The limit of BDS is not known (Q1).

## 3. Errors

### BEB-ERR-001: Result of a batch error

- **Behavior:**
  - A batch error is one of these:
    - a first byte that is not `0xfe`;
    - an unknown marker;
    - a compressed packet list that does not decode;
    - an output larger than the inflate limit;
    - a packet list error of BEB-FRAME-002.
  - A receiver that finds a batch error does not handle any packet of that batch.
  - A server that finds a batch error closes the connection.

Provenance: `design`. Kind: semantic. Confidence: medium. Edition: bedrock. Reason: a batch error
leaves no reliable packet boundary. Experiment: send BDS a batch with the marker `0x02` after the
switch. Record whether BDS closes the connection and what the client shows.

## 4. The cipher layer boundary

### BEB-FRAME-004: Order of the layers

- **Behavior:**
  - A sender makes a batch in this order:
    1. It makes the packet list.
    2. After the switch, it compresses the packet list and puts the marker before it
       (BEB-CMP-003).
    3. When the cipher layer is on, it encrypts the result of step 2.
    4. It puts the byte `0xfe` before the result.
  - Thus the byte `0xfe` is never encrypted, and the marker is encrypted when the cipher layer
    is on.
  - A receiver undoes the steps in the reverse order.

Provenance: `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock. A later
specification states the cipher layer.

## 5. Intentional deviations

**D1.**

- **Rule**: BEB-CMP-003.
- **Reference**: the libraries compress each batch after the switch and do not use the
  threshold.
- **Visible to the client**: no. The client accepts both markers.
- **Test**: a differential test compares the packet list, not the marker.

## 6. Edition differences

**E1.**

- **Rule**: BEB-CMP-002.
- **Java**: compression has a size field in each frame, and the format is zlib with a header and
  a checksum (`compression.md`).
- **Bedrock**: compression has a marker byte in each batch after the switch, and the format is
  raw deflate or Snappy.
- **Decision**: each edition keeps its own form.
- **Selected by**: client edition.
- **Visible to the client**: yes. A wrong form gives a disconnect.
- **Prediction risk**: none.
- **Parity list**: none.

## 7. Open questions

- **Q1.** The inflate limit of BDS and of the vanilla client is not known. Experiment: send BDS a
  raw deflate batch that inflates to 16 MiB of valid entries. Record whether BDS closes the
  connection.
- **Q2.** A library limits a batch to 812 packets. It is not known whether BDS has a limit on the
  number of packets in one batch. Experiment: send BDS one batch with 1000 `Text` packets.
- **Q3.** It is not known whether the threshold compares with the size of the packet list or with
  the size of one packet. This document uses the packet list (BEB-CMP-003). The experiment of
  BEB-CMP-003 decides it.
