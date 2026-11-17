# PROXY protocol: behavioural specification

This document states how the TCP listener of the server reads a PROXY protocol header. A load
balancer or a TCP proxy in front of the server sends the header before the first Minecraft
byte. The header carries the address of the player. The server then uses that address for the
connection, also for the per-IP accept limit.

The PROXY protocol is not a part of the game. Its source is the public document of HAProxy,
<https://www.haproxy.org/download/2.9/doc/proxy-protocol.txt>. In this document, "PP 2.1" means
section 2.1 of that document. No game source applies.

## 0. Conventions

### 0.1 Terms

- **Upstream**: a load balancer or a proxy that connects to the server for a player.
- **Trusted upstream**: an IP address in the configured set of PXP-CFG-001.
- **Socket address**: the IP address and the port of the remote end of the TCP connection.
- **Client address**: the IP address and the port that the server uses as the address of the
  connection. Without a header, the client address is the socket address.
- **Header**: a PROXY protocol header, version 1 or version 2.
- **v1 header**: the text header of PP 2.1.
- **v2 header**: the binary header of PP 2.2.
- **Source address**: the first address and the first port of a header. PP 2.1 and PP 2.2 call
  them the source address and the source port.
- **Minecraft stream**: the bytes of the connection after the header. The first frame of the
  Minecraft stream is the handshake.
- **Close**: the server closes the TCP connection and sends no Minecraft packet.

### 0.2 Provenance

This document uses the labels of `registry.md`, section 0.3, with one change. A rule that the
PROXY protocol document states has the label `documentation`. A rule where that document lets
the receiver select a behaviour has the label `design`, and the rule states the reason. No rule
comes from source code.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `PXP-CFG` | server inputs and the match of addresses |
| `PXP-CON` | the start of a connection, timing and failure |
| `PXP-V1` | the v1 header |
| `PXP-V2` | the v2 header |
| `PXP-USE` | the client address and the per-IP accept limit |

### 0.4 Scope

In scope: the TCP listener of `mcrs_minecraft_network`, for each intent of the handshake.

Out of scope:

- the send of a header by mcrs to another server;
- the WebTransport listener and each UDP listener;
- Velocity modern forwarding (`proxy-forwarding.md`);
- the meaning of the TLVs of PP 2.2.1 to PP 2.2.7, the SSL TLV included;
- address ranges (CIDR) in the configuration.

---

## 1. Server inputs

### PXP-CFG-001: Trusted upstreams

- **Inputs:** a set of IP addresses, IPv4 and IPv6. The default is the empty set. The user
  configures the set through the server network plugin.
- **Behavior:** each entry is one exact address. An entry has no port, no prefix length and no
  zone.
- **Behavior:** when the set is empty, the listener reads no header on any connection.
- **Behavior:** the server compares the IP address of the socket address with the set. The port
  of the socket address has no effect.

Provenance: `documentation` for the access filter (PP 2, PP 5: accept the header only from known
sources). `design` for exact addresses: a deployment has few upstreams, and a list of exact
addresses has no ambiguity. Kind: semantic. Confidence: high.

### PXP-CFG-002: IPv4-mapped addresses

- **Behavior:** an IPv6 address in `::ffff:0:0/96` (RFC 4291, section 2.5.5.2) is equal to the
  IPv4 address in its last 32 bits. The server uses the IPv4 form:
  - when it compares the socket address with the trusted upstreams;
  - for a source address that it takes from a header.
- **Behavior:** no other IPv6 address is equal to an IPv4 address.
- **Example:** the socket address `[::ffff:192.0.2.10]:40000` matches the trusted upstream
  `192.0.2.10`. A v1 `TCP6` header with the source address `::ffff:203.0.113.7` gives the client
  address `203.0.113.7`.

Provenance: `design`. A listener on `[::]` reports an IPv4 peer in the mapped form. A user
writes the IPv4 form of the upstream. One form for each client also gives one per-IP bucket and
one ban match for each client. Kind: semantic. Confidence: high.

## 2. Start of a connection

### PXP-CON-001: Connection from another address

- **Inputs:** an accepted TCP connection whose socket address is not a trusted upstream.
- **Behavior:** the first byte of the connection is the first byte of the Minecraft stream. The
  server does not look for a header.
- **Behavior:** a header from such a connection is not a valid handshake frame. The server
  closes the connection as for each other malformed handshake. The client address stays the
  socket address.
- **Behavior:** the per-IP accept limit charges the socket address when the server accepts the
  connection. This is the behaviour without this feature.

Provenance: `documentation` (PP 2: the receiver must not guess whether a header is present).
Kind: semantic. Confidence: high.

### PXP-CON-002: Connection from a trusted upstream

- **Inputs:** an accepted TCP connection whose socket address is a trusted upstream.
- **Behavior:** the connection must start with a header. The server reads the header before
  it reads the handshake.
- **Behavior:** the first bytes select the version:
  - the 12 bytes of the v2 signature (PXP-V2-001) start a v2 header;
  - the 6 bytes `PROXY ` (`50 52 4f 58 59 20`) start a v1 header;
  - each other start is malformed.
- **Behavior:** a connection that starts with a handshake frame and no header is malformed.
- **Behavior:** a malformed header gives a close (PXP-CON-004).

Provenance: `documentation` (PP 2, PP 2.2: detection by signature; the connection must be
dropped when no signature matches). Kind: wire. Confidence: high.

### PXP-CON-003: The header ends where the Minecraft stream starts

- **Behavior:** the server consumes exactly the bytes of the header. The first byte after the
  header is the first byte of the Minecraft stream. This is also true when the header and the
  handshake arrive in one TCP segment.
- **Behavior:** the server finds the end of the header from the bytes of the header only. It
  does not wait for a byte after the end of the header before it continues.
- **Behavior:** the header can arrive in more than one TCP segment, at each byte boundary.

Provenance: `documentation` (PP 2.2: the length field gives the header size; PP 9: the sample
receiver consumes only the header). `design` for the rule on the wait: an upstream can send only
the header, for example for a health check, and then wait. Kind: wire. Confidence: high.

### PXP-CON-004: Malformed header

- **Behavior:** when the header from a trusted upstream is malformed, the server closes the
  connection. It does not read the Minecraft stream. It does not use a part of the header.
- **Behavior:** the per-IP accept limit charges no address for such a connection.

Provenance: `documentation` (PP 2.1: a sequence that does not match must abort the connection;
PP 2.2: partial headers must be rejected). `design` for the charge. Kind: semantic.
Confidence: high.

### PXP-CON-005: Time limit

- **Behavior:** the header counts in the time limit of the connection. The limit is 5 seconds.
  It starts when the server accepts the connection, and it also covers the handshake.
- **Behavior:** when the header is not complete at the end of the limit, the server closes the
  connection.

Provenance: `design`. PP 2 lets the receiver apply a short timeout, and recommends at least 3
seconds. The limit of 5 seconds already exists for the handshake. Kind: semantic.
Confidence: high.

## 3. The header

### PXP-V1-001: Fields of a v1 header

A v1 header is one line of US-ASCII bytes. Each separator is exactly one space (`0x20`).

| # | Field | Value |
|---|-------|-------|
| 1 | signature | `PROXY` |
| 2 | family | `TCP4`, `TCP6` or `UNKNOWN` |
| 3 | source address | PXP-V1-002 |
| 4 | destination address | PXP-V1-002 |
| 5 | source port | PXP-V1-002 |
| 6 | destination port | PXP-V1-002 |
| 7 | end of line | CR LF (`0d 0a`) |

- **Behavior:** the comparison of the signature and of the family is exact. It does not
  ignore case.
- **Behavior:** for `TCP4` and `TCP6`, the line has all seven fields, with no other byte.
- **Behavior:** for `UNKNOWN`, the line is one of these two forms. The server ignores the bytes
  between `UNKNOWN ` and CR LF.
  - `PROXY UNKNOWN` and then CR LF;
  - `PROXY UNKNOWN`, a space, other bytes, and then CR LF.
- **Behavior:** each other family is malformed. For `TCP4` and `TCP6`, an empty field, two
  spaces in a row, and a space before CR LF are malformed. These rules do not apply to the
  ignored bytes of an `UNKNOWN` line.

Provenance: `documentation` (PP 2.1). Kind: wire. Confidence: high.

### PXP-V1-002: Addresses and ports of a v1 header

- **Behavior:** for `TCP4`, each address is four decimal numbers from `0` to `255`, with one
  dot between two numbers.
- **Behavior:** for `TCP6`, each address is an IPv6 address in one of the three text forms of
  RFC 4291, section 2.2. A group has 1 to 4 hexadecimal digits, upper or lower case. The address
  has at most one `::` and exactly 128 bits.
- **Behavior:** a port is a decimal number from `0` to `65535`.
- **Behavior:** a decimal number has no leading zero. The number `0` is the digit `0` alone.
  This applies to the ports, to the numbers of an IPv4 address, and to the numbers of the IPv4
  part of a `TCP6` address.
- **Behavior:** each other address or port is malformed. Examples: an IPv4 address after
  `TCP6`, an IPv6 address after `TCP4`, a zone (`%`), brackets, a sign, `01`, `256`, `65536`.
- **Behavior:** the server accepts each address value that has a valid form. It does not
  reject an address because of its class, for example a multicast or an unspecified address.

Provenance: `documentation` (PP 2.1) for the IPv4 form, the ports and the leading zeros.
`design` for the RFC 4291 forms: PP 2.1 describes groups of 4 digits, but a sender writes an
address in the standard text form (RFC 5952). That form drops the leading zeros of a group, and
it writes a mapped address with an IPv4 part. Kind: wire. Confidence: high.

### PXP-V1-003: End of a v1 header

- **Behavior:** the header ends at the first LF. The byte before it must be CR.
- **Behavior:** a CR that is not followed by LF is malformed. An LF that does not follow a CR
  is malformed.
- **Behavior:** the header is at most 107 bytes, CR LF included. When the first 107 bytes of
  the connection contain no end of the header, the header is malformed. The server reads at
  most 107 bytes for a v1 header.

Provenance: `documentation` (PP 2.1: the receiver must not accept a single CR or LF as the end
of the line; a line without CR LF in its first 107 characters is invalid). Kind: wire.
Confidence: high.

### PXP-V2-001: Fixed part of a v2 header

| Bytes | Field | Value |
|-------|-------|-------|
| 1–12 | signature | `0d 0a 0d 0a 00 0d 0a 51 55 49 54 0a` |
| 13, high 4 bits | version | `2` |
| 13, low 4 bits | command | `0` LOCAL, `1` PROXY |
| 14 | family and transport | PXP-V2-002 |
| 15–16 | length | unsigned 16-bit, big-endian |

- **Behavior:** the header is exactly 16 + length bytes.
- **Behavior:** a version other than `2` is malformed. A command other than `0` and `1` is
  malformed.
- **Behavior:** for LOCAL, the server ignores byte 14 and the length bytes after byte 16. The
  client address stays the socket address.

Provenance: `documentation` (PP 2.2). Kind: wire. Confidence: high.

### PXP-V2-002: Family and transport of a PROXY command

| Byte 14 | Meaning | Result of the PROXY command |
|---------|---------|-----------------------------|
| `11` | TCP over IPv4 | the source address of PXP-V2-003 |
| `21` | TCP over IPv6 | the source address of PXP-V2-003 |
| `00` | UNSPEC | the client address stays the socket address |
| `12`, `22` | UDP over IPv4, UDP over IPv6 | the client address stays the socket address |
| `31`, `32` | UNIX stream, UNIX datagram | the client address stays the socket address |
| each other value | | malformed |

- **Behavior:** for the rows that keep the socket address, the server ignores the length bytes
  after byte 16.

Provenance: `documentation` (PP 2.2: other values must be rejected; a receiver can treat a valid
combination that it does not use as UNSPEC). `design` for UNSPEC with PROXY, which PP 2.2 lets
the receiver accept or reject: the upstream is trusted, so the server accepts the connection.
Kind: wire. Confidence: high.

### PXP-V2-003: Address block

| Byte 14 | Block size | Layout, in network byte order |
|---------|------------|-------------------------------|
| `11` | 12 bytes | source address (4), destination address (4), source port (2), destination port (2) |
| `21` | 36 bytes | source address (16), destination address (16), source port (2), destination port (2) |

- **Behavior:** the address block starts at byte 17.
- **Behavior:** when the length is less than the block size, the header is malformed.

Provenance: `documentation` (PP 2.2). Kind: wire. Confidence: high.

### PXP-V2-004: Bytes after the address block

- **Behavior:** the server skips the bytes after the address block, up to the end of the
  header. It does not decode them and does not check them. This includes the TLVs.
- **Behavior:** the server does not check the CRC32C TLV (PP 2.2.3). A header with a wrong
  CRC32C value is not malformed.

Provenance: `documentation` (PP 2.2: a receiver can skip the TLVs; PP 2.2.3: the check applies
only when the receiver supports it). `design` for the CRC32C TLV: TCP already protects the
bytes, and the upstream is trusted. Kind: wire. Confidence: high.

## 4. Use of the header

### PXP-USE-001: Client address

- **Behavior:** after a v1 `TCP4` or `TCP6` header, or a v2 PROXY header of family `11` or
  `21`, the client address is the source address and the source port (PXP-CFG-002). The server
  never uses the destination address or the destination port.
- **Behavior:** after a v1 `UNKNOWN` header, a v2 LOCAL header, or a row of PXP-V2-002 that
  keeps the socket address, the client address is the socket address.
- **Behavior:** the client address applies to the whole connection, from the handshake on.
  The server uses it in each use of the client address. This includes the status and login
  intents, the address ban check of `bans.md`, BAN-JOIN-005, the address that the library gives
  for the connection, and the log lines.
- **Behavior:** when Velocity forwarding is on, FWD-USE-002 of `proxy-forwarding.md` replaces
  the client address after a successful check.

Provenance: `documentation` (PP 2.1, PP 2.2: the receiver uses the source address of the header;
it uses the real endpoints for UNKNOWN and LOCAL). Kind: semantic. Confidence: high.

### PXP-USE-002: Per-IP accept limit

This rule applies only when the per-IP accept limit is on. The limit and its bucket are as
mcrs has them without this feature.

- **Behavior:** a connection from a trusted upstream gets no charge when the server accepts it.
- **Behavior:** after a header that gives a source address, the limit charges the IP address of
  the client address. When the bucket of that address refuses, the server closes the
  connection before it reads the handshake.
- **Behavior:** a connection whose client address stays the socket address of a trusted
  upstream gets no charge.
- **Behavior:** a client address that is a loopback address gets no charge. This is the same
  rule as for a socket address.

Provenance: `design`. One upstream carries the connections of many players, so a charge to the
upstream throttles all of them. A LOCAL or `UNKNOWN` connection comes from the upstream itself,
for example a health check. A trusted address gets no protection from a charge, and a refused
health check marks the server as down. Kind: semantic. Confidence: high.

## 7. Intentional deviations

None. Where the PROXY protocol document lets the receiver select a behaviour, the rule states the
selection and the reason.

## 8. Unresolved

None.
