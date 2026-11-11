# Bedrock wire primitives: behavioural specification

| Field | Value |
|-------|-------|
| Edition | Bedrock |
| Protocol version | `729` |
| Game version | `1.21.30` (see Q1) |

This document states the primitive types of the Bedrock Edition game protocol at protocol 729.
It also states the header of one game packet. The batch that carries packets is in
`bedrock-batch.md`. The packets of the connection handshake are in `bedrock-handshake.md`. The
game packets are in `bedrock-game-packets.md`.

This document states only what a capture of the bytes can show. It does not prescribe an
architecture.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**: the two sides of one connection.
- **Vanilla server**: the Bedrock Dedicated Server (BDS) of the game version in the table above.
- **Vanilla client**: the Bedrock Edition client of that game version.
- **Sender**, **receiver**: the side that writes a value, and the side that reads it.
- **Packet**: one game packet. It is a header (BEW-HDR-001) and a payload.
- **Payload**: the bytes of a packet after its header.
- **Field**: one value in a payload, at a fixed position in the field order.
- **Decode error**: a receiver cannot read a value from the bytes. BEW-ERR-001 states the result.
- **varuint32**, **varuint64**: an unsigned LEB128 integer of 32 or 64 bits (BEW-VAR-001).
- **varint32**, **varint64**: a signed integer of 32 or 64 bits that a zigzag map converts to a
  varuint of the same width (BEW-VAR-002).
- **Count**: the number of elements of a list. Its wire type depends on the list (BEW-STR-003).
- **Network NBT**: the NBT form of this protocol, with variable-length integers (BEW-NBT-001).
- **Block position**: three integers x, y and z. The y value is a varuint32 (BEW-GEO-003).
- **Signed block position**: three integers x, y and z. Each one is a varint32 (BEW-GEO-003).
- **Sub-client**: one of four players that share one connection on a split-screen console. The
  primary player is sub-client 0.

### 0.2 Notation

Each byte fixture is hexadecimal, with one space between two bytes. A fixture that a capture
or a public source did not confirm has the mark *(constructed)*. A constructed fixture follows
the rules of this document. A test can assert it, but a capture can still prove it wrong.

The wire types of the field tables in all four Bedrock documents are these:

| Wire type | Size | Rule |
|-----------|------|------|
| `u8` | 1 byte | BEW-FIX-001 |
| `bool` | 1 byte | BEW-FIX-002 |
| `i16le`, `u16le` | 2 bytes, little-endian | BEW-FIX-001 |
| `i32le`, `u32le` | 4 bytes, little-endian | BEW-FIX-001 |
| `i64le`, `u64le` | 8 bytes, little-endian | BEW-FIX-001 |
| `i32be` | 4 bytes, big-endian | BEW-FIX-003 |
| `f32le` | 4 bytes, IEEE 754 binary32, little-endian | BEW-FIX-004 |
| `varuint32`, `varuint64` | 1 to 5, 1 to 10 bytes | BEW-VAR-001 |
| `varint32`, `varint64` | 1 to 5, 1 to 10 bytes | BEW-VAR-002 |
| `string` | varuint32 length, then UTF-8 bytes | BEW-STR-001 |
| `bytes` | varuint32 length, then bytes | BEW-STR-002 |
| `vec2`, `vec3` | 2 or 3 `f32le` | BEW-GEO-001 |
| `blockpos` | `varint32`, `varuint32`, `varint32` | BEW-GEO-003 |
| `sblockpos` | 3 `varint32` | BEW-GEO-003 |
| `uuid` | 16 bytes | BEW-GEO-004 |
| `nbt` | network NBT, one root compound | BEW-NBT-001 |
| `optional<T>` | `bool`, then a `T` when the `bool` is true | BEW-FIX-005 |
| `list<C, T>` | a count of wire type C, then that number of T | BEW-STR-003 |

### 0.3 Provenance and confidence

This document uses the provenance labels of `registry.md`, section 0.3, with these sources:

- `documentation`: the public Bedrock protocol documentation of Mojang, at the revision for
  protocol 729 (repository `Mojang/bedrock-protocol-docs`, release r21u3).
- `source_suggested`: the open-source Bedrock servers and libraries that issue #656 allows. This
  document used gophertunnel 1.41.0 (protocol 729), the CloudburstMC protocol codec that GeyserMC
  uses (codecs up to 729), and GeyserMC. These sources agree with each other, except where a rule
  says otherwise.

None of the experiments in this document has been run. Each one is a proposal. Each rule has
`Edition: bedrock`.

### 0.4 Experiment set-up

The experiments of the four Bedrock documents use this set-up:

1. Run BDS 1.21.30 on a local machine.
2. Put a capture proxy between a vanilla client of 1.21.30 and BDS.
3. Make the proxy log the bytes of each batch before the cipher layer and after decompression.
4. Turn off `online-mode` and encryption where the proxy cannot follow the key exchange.

### 0.5 ID prefixes

| Prefix | Subject |
|--------|---------|
| `BEW-VAR` | variable-length integers |
| `BEW-FIX` | fixed-size values |
| `BEW-STR` | strings, byte arrays and lists |
| `BEW-GEO` | vectors, positions and UUIDs |
| `BEW-NBT` | network NBT |
| `BEW-HDR` | the packet header |
| `BEW-ERR` | decode errors |

## 1. Variable-length integers

### BEW-VAR-001: Unsigned varint

- **Inputs:** an unsigned integer of 32 bits (varuint32) or 64 bits (varuint64).
- **Behavior:**
  - The sender writes the value in groups of 7 bits. The least significant group comes first.
  - Each byte holds one group in bits 0 to 6. Bit 7 is 1 when another byte follows, and 0 in
    the last byte.
  - A varuint32 has 1 to 5 bytes. A varuint64 has 1 to 10 bytes.
  - The sender writes the shortest form. It does not write a last byte of 0 after another byte.
- **Edge cases:**
  - A receiver accepts a form that is longer than the shortest form, inside the byte limit.
  - A varuint32 with bit 7 set in its fifth byte is a decode error.
  - A varuint64 with bit 7 set in its tenth byte is a decode error.
  - In the fifth byte of a varuint32, bits 4 to 6 are outside 32 bits. A receiver must reject a
    fifth byte larger than `0x0f` (BEW-ERR-001). See D1.
  - In the tenth byte of a varuint64, a receiver must reject a value larger than `0x01`.

| Value | varuint32 bytes |
|-------|-----------------|
| 0 | `00` |
| 1 | `01` |
| 127 | `7f` |
| 128 | `80 01` |
| 300 | `ac 02` |
| 4294967295 | `ff ff ff ff 0f` |

The varuint64 of 18446744073709551615 is `ff ff ff ff ff ff ff ff ff 01`.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEW-VAR-002: Signed varint (zigzag)

- **Inputs:** a signed integer n of 32 bits (varint32) or 64 bits (varint64).
- **Behavior:**
  - The sender maps n to an unsigned integer z of the same width. A value of n that is 0 or
    more gives z = 2n. A negative n gives z = -2n - 1.
  - The sender writes z as a varuint of the same width (BEW-VAR-001).
  - The receiver reads z. An even z gives n = z / 2. An odd z gives n = -(z + 1) / 2.

| Value | varint32 bytes |
|-------|----------------|
| 0 | `00` |
| -1 | `01` |
| 1 | `02` |
| -2 | `03` |
| 2147483647 | `fe ff ff ff 0f` |
| -2147483648 | `ff ff ff ff 0f` |

The varint64 of -9223372036854775808 is `ff ff ff ff ff ff ff ff ff 01`. The varint64 of
9223372036854775807 is `fe ff ff ff ff ff ff ff ff 01`.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

## 2. Fixed-size values

### BEW-FIX-001: Little-endian integers

- **Behavior:**
  - A fixed-size integer of 2, 4 or 8 bytes is little-endian. The least significant byte comes
    first.
  - A signed fixed-size integer is two's complement.
  - A `u8` is one byte with no sign.
- **Fixtures:** the `i16le` of -2 is `fe ff`. The `u32le` of `0x01020304` is `04 03 02 01`.
  The `u64le` of 1 is `01 00 00 00 00 00 00 00`.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEW-FIX-002: Boolean

- **Behavior:**
  - The sender writes `00` for false and `01` for true.
  - The receiver reads `00` as false. It reads each other byte value as true.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Edition: bedrock. Experiment:
none can show what the vanilla client does with `02`, because a vanilla server never sends it.
The rule follows the libraries.

### BEW-FIX-003: Big-endian integers

- **Behavior:**
  - The protocol uses a big-endian `i32be` in exactly three fields. All other fixed-size integers
    are little-endian.
  - The three fields are these:

| Packet | Field |
|--------|-------|
| `RequestNetworkSettings` | protocol version |
| `Login` | protocol version |
| `PlayStatus` | status |

- **Fixtures:** the `i32be` of 729 is `00 00 02 d9`.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEW-FIX-004: Floating-point values

- **Behavior:** an `f32le` is an IEEE 754 binary32 value, little-endian. The protocol of the
  packets in the four Bedrock documents has no binary64 field outside network NBT.
- **Fixtures:** the `f32le` of 1.0 is `00 00 80 3f`. The `f32le` of 64.0 is `00 00 80 42`.
- **Edge cases:** the receiver reads a NaN or an infinity as a value. The rule of each field
  states whether such a value is valid.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEW-FIX-005: Optional value

- **Behavior:** an `optional<T>` is a `bool`. When the `bool` is true, a `T` follows it. When it
  is false, no byte follows it.
- **Fixtures:** an absent `optional<bool>` is `00`. A present `optional<bool>` with the value
  false is `01 00`.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

## 3. Strings, byte arrays and lists

### BEW-STR-001: String

- **Behavior:**
  - A `string` is a varuint32 byte length, then that number of bytes.
  - The bytes are UTF-8. There is no terminator byte.
  - The length counts bytes, not characters.
- **Edge cases:**
  - A length that is larger than the number of bytes that remain in the payload is a decode
    error. The receiver checks the length before it allocates memory.
  - Bytes that are not valid UTF-8 are a decode error in a `string` field. See D2.
- **Fixtures:** `"abc"` is `03 61 62 63`. `"é"` is `02 c3 a9`. The empty string is `00`.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEW-STR-002: Byte array

- **Behavior:** a `bytes` value has the form of a `string`. The bytes are opaque. The receiver
  does not check them as UTF-8.
- **Edge cases:** the length check of BEW-STR-001 applies.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEW-STR-003: List counts

- **Behavior:**
  - A list is a count, then that number of elements. The elements have no separator.
  - Most lists use a `varuint32` count. The lists of this table use another count type:

| List | Count type |
|------|------------|
| `ResourcePacksInfo`: resource packs | `u16le` |
| `ResourcePackClientResponse`: pack ids | `u16le` |
| `ResourcePackStack`: experiments | `u32le` |
| `StartGame`: experiments | `u32le` |
| `PlayerAuthInput`: block actions | `varint32` |
| network NBT: list, byte array and int array | `varint32` (BEW-NBT-001) |

  - Each list of the four Bedrock documents that is not in this table uses a `varuint32` count.
    The field tables state the count type of each list.
- **Edge cases:**
  - A negative `varint32` count is a decode error.
  - A count that is larger than the number of bytes that remain in the payload is a decode
    error. The receiver checks the count before it allocates memory.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

## 4. Vectors, positions and UUIDs

### BEW-GEO-001: Vectors

- **Behavior:**
  - A `vec3` is three `f32le` values in the order x, y, z.
  - A `vec2` is two `f32le` values. A rotation `vec2` holds pitch first, then yaw, in degrees.
    A movement `vec2` holds x first, then z.
- **Fixtures:** the `vec3` of (0, 64, 0) is `00 00 00 00 00 00 80 42 00 00 00 00`.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEW-GEO-002: Chunk position

- **Behavior:** a chunk position is two `varint32` values, x first, then z.
- **Fixtures:** the chunk position (1, -1) is `02 01`.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEW-GEO-003: Block positions

- **Behavior:**
  - A `blockpos` is a `varint32` x, a `varuint32` y and a `varint32` z.
  - A `sblockpos` is a `varint32` x, a `varint32` y and a `varint32` z.
  - Each field table names the type that a field uses.
- **Edge cases:**
  - In a `blockpos`, a negative y is the varuint32 of its two's complement 32-bit form. Thus a y
    of -64 is `c0 ff ff ff 0f`. A receiver converts the varuint32 back to a signed 32-bit value.
- **Fixtures:**
  - The `blockpos` of (1, 64, -1) is `02 40 01`.
  - The `sblockpos` of (1, -64, -1) is `02 7f 01`.

Provenance: `documentation`, `source_suggested`. Kind: wire. Confidence: high (form), medium (a
negative y in a `blockpos`). Edition: bedrock. Experiment: in an overworld of BDS 1.21.30, place
a block at y = -60. Capture the `UpdateBlock` packet. Record the bytes of its y.

### BEW-GEO-004: UUID

- **Behavior:**
  - A `uuid` is two `u64le` values. The first holds the 64 most significant bits of the UUID.
    The second holds the 64 least significant bits.
- **Fixtures:** the UUID `01234567-89ab-cdef-0011-223344556677` is
  `ef cd ab 89 67 45 23 01 77 66 55 44 33 22 11 00`.

Provenance: `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

## 5. Network NBT

### BEW-NBT-001: Network NBT form

- **Behavior:**
  - An `nbt` field holds one named tag. In each field of the four Bedrock documents, the tag is
    a compound (type 10).
  - A tag is a type byte, then a name, then a value. A tag of type 0 (end) has no name and no
    value.
  - A name is a `string` (BEW-STR-001).
  - The values have these forms:

| Type | Tag | Value form |
|------|-----|------------|
| 1 | byte | 1 byte |
| 2 | short | `i16le` |
| 3 | int | `varint32` |
| 4 | long | `varint64` |
| 5 | float | `f32le` |
| 6 | double | binary64, little-endian |
| 7 | byte array | `varint32` count, then the bytes |
| 8 | string | `string` |
| 9 | list | the element type byte, a `varint32` count, then the values with no name |
| 10 | compound | tags, then a tag of type 0 |
| 11 | int array | `varint32` count, then `varint32` values |
| 12 | long array | `varint32` count, then `varint64` values |

- **Edge cases:**
  - An empty list can have the element type 0.
  - A negative count is a decode error.
  - The game item data inside a `bytes` field uses another NBT form. This document does not state
    it.
- **Fixtures:**
  - The empty root compound with an empty name is `0a 00 00`.
  - A root compound with one int tag `a` = -1 is `0a 00 03 01 61 01 00`.

Provenance: `documentation` (types 1 to 11), `source_suggested` (type 12). Kind: wire.
Confidence: high. Edition: bedrock.

## 6. Packet header

### BEW-HDR-001: Header

- **Inputs:** a packet id, a sender sub-client id and a target sub-client id.
- **Behavior:**
  - The header is one varuint32. It is the first field of each packet.
  - Bits 0 to 9 hold the packet id. Bits 10 and 11 hold the sender sub-client id. Bits 12 and 13
    hold the target sub-client id.
  - A sender writes 0 in bits 14 to 31.
  - Packet ids are from 0 to 1023. One id table applies to both directions.
- **Fixtures:**
  - The header of packet id `0x09`, from sub-client 1 to sub-client 2, is `89 48`.
  - The header of packet id `0x90` with both sub-client ids 0 is `90 01`.
  - The header of packet id `0xc1` with both sub-client ids 0 is `c1 01`.

Provenance: `source_suggested`. Kind: wire. Confidence: high. Edition: bedrock.

### BEW-HDR-002: Sub-client ids

- **Behavior:**
  - A server that does not support split-screen sub-clients writes 0 in both sub-client fields.
  - Such a server discards a packet with a sender sub-client id that is not 0. It does not
    close the connection.
  - A receiver ignores bits 14 to 31 of the header.

Provenance: `design`. Kind: unspecified. Confidence: medium. Edition: bedrock. Reason: mcrs has
no split-screen support. The discard keeps a split-screen client of a console connected for its
primary player. Experiment: connect a split-screen console client to BDS. Capture the header of
each packet of the second player.

## 7. Decode errors

### BEW-ERR-001: Result of a decode error

- **Behavior:**
  - A decode error in one packet makes the receiver stop the decode of that packet.
  - A server that has a decode error in a serverbound packet closes the connection. It does not
    apply the packet.
  - Bytes that remain in the payload after the last field are a decode error. See D3.
  - An unknown packet id is not a decode error. The receiver discards the packet and reads the
    next packet of the batch.

Provenance: `design`. Kind: semantic. Confidence: medium. Edition: bedrock. Reason: a strict
decoder finds a field table error at the first capture. Experiment: send BDS a `Text` packet
with one extra byte after its last field. Record whether BDS keeps the connection.

## 8. Intentional deviations

**D1.**

- **Rule**: BEW-VAR-001.
- **Reference**: the libraries ignore the bits of the last byte of a varuint that are outside
  its width.
- **Visible to the client**: no. A vanilla client writes the shortest form.
- **Test**: a differential test does not send such a varuint.

**D2.**

- **Rule**: BEW-STR-001.
- **Reference**: the libraries keep the bytes of a `string` as they are.
- **Visible to the client**: no. A vanilla client writes valid UTF-8.
- **Test**: a differential test does not send such a string.

**D3.**

- **Rule**: BEW-ERR-001.
- **Reference**: not known for BDS. The libraries report extra bytes as an error.
- **Visible to the client**: no.
- **Test**: a differential test records the result and does not compare it.

## 9. Edition differences

**E1.**

- **Rule**: BEW-VAR-002.
- **Java**: a signed variable-length integer is the two's complement value as an unsigned LEB128. -1 has 5 bytes.
- **Bedrock**: a signed varint uses the zigzag map. -1 has 1 byte.
- **Decision**: each edition keeps its own form.
- **Selected by**: client edition.
- **Visible to the client**: yes. A wrong form gives a decode error in the client.
- **Prediction risk**: none.
- **Parity list**: none.

**E2.**

- **Rule**: BEW-FIX-001.
- **Java**: fixed-size integers are big-endian.
- **Bedrock**: fixed-size integers are little-endian, except the three fields of BEW-FIX-003.
- **Decision**: each edition keeps its own form.
- **Selected by**: client edition.
- **Visible to the client**: yes.
- **Prediction risk**: none.
- **Parity list**: none.

## 10. Open questions

- **Q1.** The issue and the crate comments name game version 1.21.40 for protocol 729. The
  public documentation and the libraries give protocol 729 for 1.21.30, and protocol 748 for
  1.21.40. This document follows protocol 729. The owner must confirm the pin.
- **Q2.** The largest `string` length that BDS accepts is not known. Experiment: send BDS a
  `Text` packet with a message of 1 MiB. Record the result.
