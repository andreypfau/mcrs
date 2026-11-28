# Proxy forwarding: behavioural specification

This document states how a backend server accepts a player that the Velocity proxy forwards in
the "modern" forwarding mode. The backend gets the profile and the address of the player in a
signed answer to a LOGIN custom query. The backend does not authenticate the player. The proxy
authenticates the player only when the proxy runs in online mode. The signature proves that
the data comes from the proxy. It does not prove the identity of the player.

Velocity is not the game. It is an open-source proxy. The sources of this document are the
public Velocity documentation, the public source of Velocity, and the public source of the
Paper server, which is a backend that Velocity supports.

This document extends `login.md`. It uses the packets, the wire types and the login sequence of
that document. It states only the differences that forwarding makes.

The legacy BungeeCord forwarding mode, which puts the data in the handshake host field, is out
of scope.

## 0. Conventions

### 0.1 Terms

- **Proxy**: a Velocity instance between the player's client and the backend. The backend sees
  the proxy as its client.
- **Backend**: the server behind the proxy. In this document, the backend is mcrs.
- **Forwarding**: the mode in which the backend takes the profile and the address of the player
  from the proxy. This document states only the "modern" forwarding mode of Velocity.
- **Secret**: a text that the proxy and the backend share. The proxy configuration calls it the
  forwarding secret.
- **Key**: the UTF-8 bytes of the secret. The key is the HMAC key.
- **Forwarding query**: the `custom_query` that the backend sends to get the forwarded data.
- **Forwarding answer**: the `custom_query_answer` that has the transaction id of the forwarding
  query.
- **Payload**: the bytes of the forwarding answer after its presence Boolean.
- **Signature**: the first 32 bytes of the payload.
- **Signed data**: all the bytes of the payload after the signature.
- **Forwarding version**: the first field of the signed data. It selects the layout of the
  signed data.
- **Requested version**: the highest forwarding version that the backend accepts. The
  forwarding query carries it.
- **Forwarded profile**: the profile in the signed data: an id, a name and properties.
- **Forwarded address**: the client address in the signed data.
- **Direct client**: a client that connects to the backend without the proxy.

Other terms are as `login.md`, section 0.1, defines them.

### 0.2 Wire types

The wire types are those of `login.md`, section 0.2. In addition:

| Type | Form |
|------|------|
| HMAC-SHA256 | the 32-byte HMAC of RFC 2104 with the hash SHA-256 (FIPS 180-4) |

### 0.3 Provenance and confidence

This document uses the labels of `registry.md`, section 0.3, with one change. For this
document, "the reference" is the public source of Velocity and of Paper, not the game. A rule
that only that source supports has the label `source_suggested`. None of the experiments in
this document has been run. Each one is a proposal.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `FWD-CFG` | server inputs |
| `FWD-REQ` | the forwarding query |
| `FWD-ANS` | the layout of the forwarding answer |
| `FWD-CHK` | the checks of the forwarding answer and their results |
| `FWD-USE` | the use of the forwarded data after a successful check |

### 0.5 Scope

In scope:

- the LOGIN state of a network connection when forwarding is on;
- the forwarding versions 1 and 4.

Out of scope:

- the legacy BungeeCord forwarding mode, and the BungeeGuard variant of it;
- the STATUS state. A status request through the proxy uses the socket address;
- the forwarding versions 2 and 3. The proxy uses them only for clients older than protocol
  `761` (game version 1.19.3). The backend rejects them (FWD-CHK-003).

---

## 1. Server inputs

### FWD-CFG-001: Forwarding switch and secret

- **Inputs:** a forwarding switch, on or off. The default is off. A secret, as text.
- **Behavior:** when the switch is off, the login is that of `login.md`. The backend sends no
  forwarding query.
- **Behavior:** when the switch is on, every login on a network connection uses forwarding.
- **Behavior:** the key is the UTF-8 bytes of the secret. The backend does not trim the
  secret and does not change it in any other way.
- **Behavior:** when the switch is on and the secret is empty, the server does not start. The
  library returns a configuration error.
- **Note:** the proxy reads its secret from a file. It joins the lines of the file and removes
  each line terminator. So the secret of the backend must not contain a line terminator.

Provenance: `documentation` for the shared secret, `source_suggested` for the key bytes and the
line terminators. `design` for the empty secret: an empty key lets each party that knows the
format forge a login. Kind: semantic. Confidence: high.

### FWD-CFG-002: Forwarding excludes online mode

- **Behavior:** when forwarding is on, the backend never uses online mode. It never sends the
  key data of `login.md`, LOGIN-SEQ-005, and it never calls the session server.
- **Behavior:** a configuration that turns on both forwarding and online mode is a
  configuration error. The server does not start.
- **Reason:** the proxy, not the backend, authenticates the player when it runs in online
  mode. When a backend sends key data, the proxy ends the connection to the backend. The
  player cannot join.

Provenance: `documentation` (the backend must have online mode off), `source_suggested` for
the result in the proxy. `design` for the configuration error. Kind: semantic. Confidence: high.

## 2. The forwarding query

### FWD-REQ-001: When the backend sends the forwarding query

- **Inputs:** a serverbound `hello`, on a connection with forwarding on.
- **Behavior:** the backend checks the name in `hello` as `login.md`, LOGIN-SEQ-001, states.
  When the name is valid, the backend sends the forwarding query as its next packet.
- **Behavior:** the backend sends no other LOGIN packet before the forwarding answer arrives,
  except `login_disconnect`.
- **Behavior:** the backend does not make the offline profile of LOGIN-SEQ-001. The profile of
  the login is the forwarded profile (FWD-USE-001).
- **Postconditions:** the forwarding query is a plain, uncompressed frame. No layer is on at
  this point.
- **Edge cases:** when the backend does not send the forwarding query before
  `login_finished`, the proxy disconnects the player with this text: "Your server did not send
  a forwarding request to the proxy. Make sure the server is configured for Velocity
  forwarding."

Provenance: `source_suggested`. Kind: wire. Confidence: medium.

### FWD-REQ-002: Content of the forwarding query

| Field | Value |
|-------|-------|
| transaction id | a VarInt that the backend selects |
| channel | `velocity:player_info` |
| data | one byte: the requested version, `0x04` |

- **Behavior:** the requested version is `4` for every protocol version that `login.md`,
  LOGIN-VER-001, accepts.
- **Behavior:** the proxy selects the forwarding version from the requested version and the
  protocol version of the player's client. For a client of protocol `761` or later, it
  selects:
  - `4` when the requested version is `4` or more;
  - `1` in all other cases.
- **Reason:** every protocol version that mcrs accepts is later than `761`. So the proxy sends
  version `1` or `4`, and the two layouts are equal (FWD-ANS-001). Version `4` tells the proxy
  that the backend expects the chat session of the player in the PLAY state, and no key data
  in LOGIN. Paper requests the same version, so the proxy uses a path that many servers use.
- **Behavior:** the transaction id value is unspecified. The backend recognizes only the value
  that it sent in this login.
- **Edge cases:** when the data of a query is not exactly one byte, the proxy uses the
  requested version `1`.

Provenance: `source_suggested` for the channel, the data byte and the selection of the proxy.
`design` for the version `4`. Kind: wire, except the transaction id value, which is
unspecified. Confidence: medium.

## 3. The forwarding answer

### FWD-ANS-001: Layout of the payload

The payload of a forwarding answer has this layout for the forwarding versions 1 and 4:

| # | Field | Type | Limit that the backend applies |
|---|-------|------|--------------------------------|
| 1 | signature | 32 bytes | exactly 32 bytes |
| 2 | forwarding version | VarInt | `1` or `4` (FWD-CHK-003) |
| 3 | client address | String | at most 32767 bytes; an IP address literal |
| 4 | profile id | UUID | |
| 5 | profile name | String | String (16) |
| 6 | property count | VarInt | from `0` to `16` |
| 7 | each property: name | String | String (64) |
| 8 | each property: value | String | String (32767) |
| 9 | each property: signature | Optional String | String (1024) |

- **Behavior:** the signature is the HMAC-SHA256 of the signed data, with the key. The signed
  data is fields 2 to 9.
- **Behavior:** a String is a VarInt count of UTF-8 bytes, then the bytes, as in `login.md`,
  section 0.2.
- **Behavior:** the client address is the text form of the IP address of the player's
  connection to the proxy: four decimal numbers with dots for IPv4, or the RFC 5952 or RFC 4291
  text form for IPv6. It is never a host name and has no port.
- **Behavior:** the proxy writes the signature Boolean of a property as false when the
  property has no signature or an empty signature.
- **Behavior:** the payload ends after the last property. The layout of version 4 has no field
  that version 1 does not have.
- **Note:** the limits of rows 5 to 9 are those of the profile in `login_finished`
  (`login.md`, LOGIN-PKT-003). A profile that is in these limits can go to every vanilla
  client.

Provenance: `source_suggested` for the layout. `design` for the limits of rows 5 to 9 and for
the end of the payload: see "Intentional deviations". Kind: wire. Confidence: medium.


## 4. Checks and results

### FWD-CHK-001: Order of the checks

When the backend waits for the forwarding answer and receives a `custom_query_answer`, it
does these checks in this order. The first check that fails gives the result of its row. Then
the backend closes the connection.

| # | Check | Result when the check fails |
|---|-------|-----------------------------|
| 1 | the transaction id is equal to the id of the forwarding query | `login_disconnect` with `multiplayer.disconnect.unexpected_query_response` |
| 2 | the answer has a payload (the presence Boolean is true) | `login_disconnect` with the text "This server requires you to connect with Velocity." |
| 3 | the payload has at least 32 bytes, and the signature is correct (FWD-CHK-002) | `login_disconnect` with the text "Unable to verify player details" |
| 4 | the forwarding version is `1` or `4` (FWD-CHK-003) | `login_disconnect` with the text "Unsupported forwarding version" |
| 5 | the signed data obeys the layout and the limits of FWD-ANS-001 | `login_disconnect` with the text "Malformed player details" |

- **Behavior:** each text in the table is a plain text component with no style. A row with a
  translation key has no argument.
- **Behavior:** when all checks pass, the backend continues as FWD-USE-003 states.
- **Behavior:** the proxy shows the text of the `login_disconnect` to the player. The proxy
  can add its own text around it.

Provenance: `source_suggested` for rows 1 to 3 and their texts. `design` for the texts of rows
4 and 5: see "Intentional deviations". Kind: semantic. Confidence: medium.

### FWD-CHK-002: Signature check

- **Behavior:** the backend computes the HMAC-SHA256 of the signed data with its key. The
  signature is correct when it is equal to the result in all 32 bytes.
- **Behavior:** the backend checks the signature before it decodes a field of the signed
  data. A wrong signature gives row 3 of FWD-CHK-001, also when the signed data is malformed.
- **Behavior:** the time of the comparison does not depend on the position of the first byte
  that is different.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

### FWD-CHK-003: Forwarding version check

- **Behavior:** the backend accepts the forwarding versions `1` and `4`. It rejects every other
  value with row 4 of FWD-CHK-001. This includes `0`, a negative value, `2`, `3`, and each
  value above the requested version.
- **Reason:** the versions 2 and 3 carry key data that the proxy sends only for clients older
  than protocol `761`. The backend does not accept such a client (`login.md`, LOGIN-HS-003).

Provenance: `source_suggested` for the bound by the requested version. `design` for the
rejection of `2` and `3`. Kind: semantic. Confidence: medium.

### FWD-CHK-004: Direct client

- **Inputs:** a vanilla client that connects to the backend without the proxy, while
  forwarding is on.
- **Behavior:** the vanilla client answers the forwarding query with the same transaction id
  and no payload (`login.md`, LOGIN-OTH-002).
- **Behavior:** the backend sends `login_disconnect` with the text "This server requires you
  to connect with Velocity." (row 2 of FWD-CHK-001). The vanilla client shows this text.
- **Behavior:** a proxy in a mode other than modern forwarding also answers with no payload,
  with the same result.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

### FWD-CHK-005: No answer

- **Behavior:** the backend has no separate time limit for the forwarding answer. The login
  timeout of `login.md`, LOGIN-OTH-004, applies. The wait for the answer is a part of the 600
  ticks.
- **Behavior:** at the end of the timeout, the backend sends `login_disconnect` with
  `multiplayer.disconnect.slow_login`, as a plain, uncompressed frame. Then it closes the
  connection.
- **Behavior:** the 30-second read limit of LOGIN-OTH-004 can close the connection first,
  without a packet.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### FWD-CHK-006: Other packets during the wait

- **Behavior:** while the backend waits for the forwarding answer, every other serverbound
  LOGIN packet is an error as in `login.md`, LOGIN-ERR-001.
- **Behavior:** a `custom_query_answer` when the backend does not wait for the forwarding
  answer gives `multiplayer.disconnect.unexpected_query_response` (LOGIN-OTH-002). This
  includes a second forwarding answer.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

## 5. Use of the forwarded data

### FWD-USE-001: Forwarded profile

- **Behavior:** after a successful check, the profile of the player is the forwarded profile:
  - the id is the profile id of the payload;
  - the name is the profile name of the payload;
  - the properties are the properties of the payload, in their order, with their values and
    signatures.
- **Behavior:** the backend ignores the name and the profile id of the serverbound `hello`
  for the profile.
- **Behavior:** the owner profile of `login.md`, LOGIN-CFG-002, does not apply. A forwarded
  login with the owner's name gets the forwarded profile.
- **Behavior:** a property keeps its signature. A property without a signature stays without
  a signature.
- **Behavior:** the backend uses the forwarded profile in `login_finished` and in every later
  use of the player's profile: the ban list, the whitelist, the check for a player with the
  same id (`login.md`, LOGIN-SEQ-002), and the player entries that other clients receive.
- **Postconditions:** the `textures` property reaches each vanilla client unchanged. So each
  client shows the skin of the player.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

### FWD-USE-002: Forwarded address

- **Behavior:** after a successful check, the client address of the connection is the
  forwarded address. The port is the port of the socket.
- **Behavior:** the backend uses the forwarded address in place of the socket address in every
  use of the client address. This includes the address ban check of `bans.md`, BAN-JOIN-005,
  and the address of a player that `ban-ip` with a player name bans.
- **Behavior:** before a successful check, the client address is the socket address. A failed
  check does not change it.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

### FWD-USE-003: Sequence of a forwarded login

| # | Direction | Packet | Note |
|---|-----------|--------|------|
| 1 | serverbound | `intention` | |
| 2 | serverbound | `hello` | the proxy sends the player's name |
| 3 | clientbound | `custom_query` | FWD-REQ-002 |
| 4 | serverbound | `custom_query_answer` | FWD-ANS-001 |
| 5 | — | the checks of FWD-CHK-001 | |
| 6 | clientbound | `login_compression` (26.3) or `setup_compression` (26.4) | when compression is on |
| 7 | clientbound | `login_finished` | the forwarded profile |
| 8 | serverbound | `login_acknowledged` | |

- **Behavior:** after step 5, the backend continues with the verification of `login.md`,
  LOGIN-SEQ-002, at its next tick.
- **Behavior:** the cipher layer stays off. In 26.4, the backend sends no `apply_options`.
  The options of `login_finished` are as LOGIN-PKT-004 states for a login without the cipher
  layer.
- **Behavior:** rows 1 to 6 are plain, uncompressed frames. The compression of rows 7 and 8 is
  as `login.md`, LOGIN-SEQ-003, states.

Provenance: `source_suggested`. Kind: wire. Confidence: medium.

## 8. Intentional deviations

These entries compare mcrs with Paper as a backend. The vanilla game has no forwarding.

- **Rule:** FWD-CHK-001, row 4, and FWD-CHK-003.
  - **Reference:** Paper rejects only a version above `4`. It signals the error as an internal
    error, and the connection ends with a generic text. It accepts `2` and `3` and ignores
    their key data.
  - **Visible to the client:** no. The proxy never sends such a version for a client that
    mcrs accepts.
  - **Test:** a differential test skips this case.
- **Rule:** FWD-CHK-001, row 5, and FWD-ANS-001, the limits of rows 5 to 9 and the end of the
  payload.
  - **Reference:** Paper reads the property strings with a limit of 32767 characters and the
    property count with no limit. It ignores bytes after the last property. A decode error
    ends the connection with a generic text.
  - **Visible to the client:** no. A proxy that the player uses sends profiles of the session
    server, which are in the limits.
  - **Test:** a differential test skips this case.
- **Rule:** FWD-CFG-002.
  - **Reference:** Paper accepts forwarding together with online mode. It then sends the key
    data, and the proxy ends the connection.
  - **Visible to the client:** yes, at login. With mcrs, the server does not start. With Paper,
    the player cannot join through the proxy.
  - **Test:** none. The difference is in the configuration.

## 9. Unresolved

- **FU-01: name characters.** `login.md`, LOGIN-SEQ-001, rejects a name in `hello` with a
  character of code 32 or less, or 127 or more. It is not decided whether the backend applies
  the same check to the forwarded name. Paper does not. A proxy in offline mode can forward
  such a name. Question to the owner.
- **FU-02: IPv6 zone.** The proxy can write an IPv6 address with a zone suffix (`%` and a zone
  name) for a link-local client. It is not decided whether such an address is malformed.
- **FU-03: texts.** It is not decided whether the texts of FWD-CHK-001 must be translation
  keys of mcrs. The proxy shows them to a player in any language.
