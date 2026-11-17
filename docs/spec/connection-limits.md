# Connection limits: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states four limits of a server that a public network can reach:

- the player limit, which refuses a login when the server is full;
- the per-IP join cap, which limits the joined connections of one address;
- the join rate, which limits how fast one address can join;
- the packet rate, which ends a connection that sends too many packets.

This document extends `login.md`. It states only what a client, a player or a user of the library
can see. It does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **vanilla client**, **tick**: as `login.md`, section
  0.1, defines them.
- **Client address**: the address of the connection after the PROXY protocol header
  (`proxy-protocol.md`, PXP-USE-001) and after Velocity forwarding (`proxy-forwarding.md`,
  FWD-USE-002).
- **Address**: the IP address of the client address, without the port. An IPv4-mapped IPv6
  address is the IPv4 address in its last 32 bits (PXP-CFG-002).
- **Loopback address**: an address in `127.0.0.0/8`, or `::1`.
- **Verification**: step 1 of `login.md`, LOGIN-SEQ-002. The server knows the profile and the
  client address of the login. It has not sent `login_finished`.
- **Joined connection**: a connection that passed each check of verification. It stays a joined
  connection until it closes. This includes a connection that waits for the departure of an older
  session (LOGIN-SEQ-002, step 3), a connection in CONFIGURATION and a connection in PLAY.
- **Refusal**: the server sends `login_disconnect` with a reason, and then closes the connection.
- **Plain text**: a text component with only the field `text`, and no style.
- **Plugin**: the server network plugin. Each input of this document is an input of this plugin.

### 0.2 Provenance

This document uses the labels of `registry.md`, section 0.3. A `black_box_confirmed` rule cites
a captured result or a file of the client jar of the version in the header. A `design` rule states
the reason for the choice. No rule comes only from source code.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `LIM-CFG` | plugin inputs and their defaults |
| `LIM-JOIN` | the order of the checks at verification |
| `LIM-FULL` | the player limit |
| `LIM-IP` | the per-IP join cap |
| `LIM-RATE` | the join rate |
| `LIM-PKT` | the packet rate |

### 0.4 Scope

In scope: each connection of the server, on each listener. Out of scope:

- the per-IP accept limit at the TCP accept (PXP-USE-002). This document does not change it.
  LIM-RATE-002 states how the two limits relate;
- the login timeout and the removal of idle buckets (issue #305);
- the operator rights of a loopback client (issue #913);
- the server property `rate-limit` of a vanilla server (`movement-checks.md`, MVC-VAN-019). mcrs
  has no `server.properties`. LIM-PKT-001 states the packet rate of mcrs.

---

## 1. Plugin inputs

### LIM-CFG-001: Inputs and defaults

| Input | Default | Value that turns the limit off |
|-------|---------|--------------------------------|
| player limit | `20` | none: the limit is always on |
| per-IP join cap | `5` joined connections | no value |
| join rate | a bucket of `3` tokens, `1` token back in each `4` seconds | no value |
| packet rate limit | `500` packets per second | `0` |

- **Behavior:** the user sets each input through the plugin (`README.md`, section "Requirements
  that each specification inherits"). The server reads no input from an environment variable or
  from a file.
- **Behavior:** the player limit is also the maximum count that the status response and the
  `login` packet of PLAY show. This is the behaviour without this document.
- **Behavior:** each input applies to each connection. An input does not change while the server
  runs.

Provenance: `design`. The player limit of `20` is the default that mcrs already has. The packet
rate limit of `500` is the documented default of the packet limiter of the Paper server: a vanilla
client in normal play sends much less. The other values are small numbers that let a household
join together and stop one address from a flood of logins. Section 8 lists the defaults that the
owner can change. Kind: semantic. Confidence: high.

## 2. Checks at verification

### LIM-JOIN-001: Order of the checks

At verification, the server does these checks in this order. The first check that fails gives a
refusal with its reason. A login that passes all checks continues with step 2 of LOGIN-SEQ-002.

| # | Check | Rule |
|---|-------|------|
| 1 | the ban checks and the whitelist check | `bans.md`, `whitelist.md` |
| 2 | the player limit | LIM-FULL-001 |
| 3 | the per-IP join cap | LIM-IP-001 |
| 4 | the join rate | LIM-RATE-001 |

- **Behavior:** the order inside row 1 is as `bans.md` and `whitelist.md` state.
- **Behavior:** the checks use the client address after the forwarding answer. A forwarded login
  thus gets the checks with the address of the player, not the address of the proxy.
- **Behavior:** the checks of rows 2 to 4 apply also to a login with the transfer intent.

Provenance: `black_box_confirmed` for the whitelist before the player limit (`whitelist.md`,
WL-JOIN-006). `design` for the address ban before the player limit: a ban reason tells
the player more than a full server. `design` for rows 3 and 4: a limit that protects the server
comes after the checks that tell the player why the player cannot join. Kind: semantic.
Confidence: high.

## 3. The player limit

### LIM-FULL-001: Full server

- **Inputs:** the player limit `M`. The count `N` of joined connections, except each joined
  connection with the profile id of the login.
- **Behavior:** when `N` is equal to `M` or more, the login gets a refusal with this reason:

```json
{"translate": "multiplayer.disconnect.server_full"}
```

- **Behavior:** the reason has no argument and no style. The vanilla client shows
  "The server is full!".
- **Behavior:** no player can join past the limit. An operator has no exception. A loopback
  address has no exception.
- **Behavior:** a player limit of `0` refuses each login.
- **Postconditions:** the number of joined connections is never more than `M` after the check.
  Two logins that the server verifies in the same tick count in the order of their verification.

Provenance: `black_box_confirmed` for the key and the packet and for the English text (`assets/minecraft/lang/en_us.json` in the client jar). `design` for the
count of joined connections and for the exception of the same profile id: see section 7. Kind:
semantic. Confidence: high.

### LIM-FULL-002: Status count

- **Behavior:** the status response counts only the online players (`status.md`). A joined
  connection in CONFIGURATION counts in LIM-FULL-001, but not in the status response.
- **Example:** with `M = 20`, 19 players in PLAY and one connection in CONFIGURATION, the status
  response shows 19 of 20. A new login gets the refusal of LIM-FULL-001.

Provenance: `design`. A slot belongs to a player from verification on, so a player that downloads
a resource pack does not lose the slot. Kind: semantic. Confidence: high.

## 4. The per-IP join cap

### LIM-IP-001: Cap of joined connections for one address

- **Inputs:** the per-IP join cap `C`. The count `K` of joined connections with the address of the
  login, except each joined connection with the profile id of the login.
- **Behavior:** when `K` is equal to `C` or more, the login gets a refusal with this reason, as
  plain text:

```text
Too many connections from your address
```

- **Behavior:** the server compares two addresses as exact values. Two IPv6 addresses in one
  `/64` network are different addresses.
- **Behavior:** a login from a loopback address gets no check. Its joined connection does not
  count in `K`.
- **Behavior:** a STATUS connection and a connection that has not reached verification do not
  count in `K`.
- **Behavior:** with no value for the input, the server does not do this check.

Provenance: `design`. One address with many joined connections is a common sign of a bot. The
exception of the same profile id lets a player reconnect while the old session ends. The loopback
exception is the same as in PXP-USE-002: an integrated server and a local test connect many
clients from loopback. Kind: semantic. Confidence: high.

### LIM-IP-002: A proxy that does not give the client address

- **Behavior:** when the server is behind a proxy with no Velocity forwarding and no PROXY
  protocol header, each connection has the address of the proxy. All players then share one
  address, so LIM-IP-001 refuses the player after the cap, and LIM-RATE-001 refuses a quick
  login after the capacity of the bucket.
- **Behavior:** such a setup must turn off the per-IP join cap and the join rate, with no value
  for the two inputs of LIM-CFG-001. The server does not detect such a setup.

Provenance: `design`. The server cannot tell a proxy from one busy address; only the operator
knows the setup. Kind: semantic. Confidence: high.

## 5. The join rate

### LIM-RATE-001: Join bucket of one address

- **Inputs:** the join rate: a capacity `B` and a period `P`. One bucket of tokens for each
  address.
- **Behavior:** a new bucket holds `B` tokens. A bucket gains tokens continuously, `1` token in
  each period `P`, up to `B`. A token count can be a fraction.
- **Behavior:** at row 4 of LIM-JOIN-001, the server looks at the bucket of the address:
  - when the bucket holds `1` token or more, the server removes `1` token, and the login passes;
  - else the login gets a refusal with this reason, as plain text. The bucket does not change.

```text
Too many logins from your address. Wait a few seconds and try again.
```

- **Behavior:** a login that a check before row 4 refuses takes no token.
- **Behavior:** a login from a loopback address gets no check and takes no token.
- **Behavior:** with no value for the input, the server does not do this check.

Provenance: `design`. A bucket lets a household join together and limits a flood from one
address to a low steady rate. The public documentation of the Velocity proxy and of the Spigot
server describe a similar limit for each address. Kind: semantic. Confidence: high.

### LIM-RATE-002: The join rate and the per-IP accept limit

- **Behavior:** the per-IP accept limit of PXP-USE-002 stays as it is: a bucket of `5` tokens for
  each address, with `0.5` token back each second. It charges each TCP connection when the server
  accepts it, or after the PROXY protocol header. A refused connection closes with no packet.
- **Behavior:** a connection that the accept limit closes never reaches verification. It takes no
  token of the join bucket.
- **Behavior:** the two buckets are independent. A login that both limits pass takes one token of
  each bucket.
- **Behavior:** when Velocity forwarding is on, the accept limit is off, because the server sees
  only the address of the proxy at the accept. The join rate then is the only rate limit for each
  address. It uses the forwarded address.

Provenance: `design`. The accept limit stops a flood of TCP connections before it costs a login.
The join rate gives a reason to the player, and it also works behind a proxy. Kind: semantic.
Confidence: high.

## 6. The packet rate

### LIM-PKT-001: Count and average

- **Inputs:** the packet rate limit `L`. The time of arrival of each serverbound packet.
- **Behavior:** the server counts each serverbound packet that it reads from the connection, in
  each state from LOGIN on: LOGIN, CONFIGURATION and PLAY. A packet that the server ignores also
  counts. The handshake packet and the packets of a STATUS connection do not count.
- **Behavior:** the seconds of a connection start at the arrival of its first counted packet. Second `k`
  is the time from `t0 + k` seconds to `t0 + k + 1` seconds, where `t0` is that arrival. The
  seconds follow the arrival times, not the ticks of the server.
- **Behavior:** the connection has an average `a`, at first `0`. At the end of each second with
  `c` packets, `a` becomes `c + 0.75 × (a − c)`. A second with no packet multiplies `a` by `0.75`.
- **Behavior:** the server compares `a` with `L` when the first packet of a later second arrives.
  It compares the average of the last second that had a packet, before the empty seconds reduce
  it. When `a` is more than `L`, the result of LIM-PKT-002 occurs.
- **Behavior:** a limit of `0` turns the count off.

Provenance: `design`. The average lets a short burst pass, for example the packets at the start of
CONFIGURATION, and it stops a steady flood within a few seconds. The arrival time does not punish
a client when the server is slow and reads a backlog at one time. The vanilla server has a similar
property, `rate-limit`, with the same reason text. Kind: semantic. Confidence: high.

### LIM-PKT-002: Disconnect past the packet rate

- **Behavior:** the server does not handle the packet whose arrival gave the result, and no later
  packet of the connection.
- **Behavior:** the server sends the disconnect packet of the current state with this reason, and
  then closes the connection:

```json
{"translate": "disconnect.exceeded_packet_rate"}
```

| State | Packet |
|-------|--------|
| LOGIN | `login_disconnect` |
| CONFIGURATION | `disconnect` of CONFIGURATION |
| PLAY | `disconnect` of PLAY |

- **Behavior:** the vanilla client shows "Kicked for exceeding packet rate limit".
- **Behavior:** a loopback address has no exception.

Provenance: `black_box_confirmed` for the English text (`assets/minecraft/lang/en_us.json` in the
client jar). `design` for the use of the key: the vanilla client translates it. Kind: semantic.
Confidence: high.

## 7. Intentional deviations

- **Rule:** LIM-FULL-001, the count of joined connections.
  - **Reference:** the vanilla server counts only the players in PLAY. It checks the limit again
    at the end of CONFIGURATION, and refuses there with the `disconnect` packet of PLAY.
  - **Reason:** one check at verification gives the reason before the client downloads a resource
    pack, and the slot stays with the player that got it.
  - **Visible to the client:** yes, only when two logins compete for the last slot. The second
    login gets the refusal at LOGIN, not at the end of CONFIGURATION.
  - **Bug reference:** none.
  - **Test:** a differential test skips a login while another connection is in CONFIGURATION.
- **Rule:** LIM-FULL-001, the exception of the same profile id.
  - **Reference:** the vanilla server counts the old session of the player. A player that
    reconnects to a full server before the old session ends gets
    `multiplayer.disconnect.server_full`.
  - **Reason:** the new login ends the old session (LOGIN-SEQ-002, step 3), so the player takes
    the same slot again.
  - **Visible to the client:** yes. The player joins in place of the refusal.
  - **Bug reference:** none.
  - **Test:** a differential test skips a login with the profile id of a joined connection.
- **Rule:** LIM-IP-001, LIM-RATE-001 and the default of LIM-PKT-001.
  - **Reference:** the vanilla server has no per-IP join cap and no join rate. Its packet rate
    limit is off by default (`rate-limit=0`).
  - **Reason:** a public server must slow down a flood of connections .
  - **Visible to the client:** yes, only for a client past a limit. It gets the refusal or the
    disconnect of the rule.
  - **Bug reference:** none.
  - **Test:** a differential test turns these limits off.

## 8. Unresolved

Each entry has a default. The default applies until the owner decides.

Each entry is a `design` rule with a default. The default applies until the owner decides. An
entry is not an open question for an implementer.

1. **Slots in CONFIGURATION** (LIM-FULL-001, LIM-FULL-002). Default: a joined connection in
   CONFIGURATION holds a slot. The other choice: count only the players in PLAY, and check the limit
   again at the end of CONFIGURATION.
2. **A rejoin to a full server** (LIM-FULL-001, LIM-IP-001). Default: a joined connection with
   the profile id of the login does not count.
3. **A player that can pass the player limit.** Default: none. mcrs has no operator file, so no
   operator flag can give this right.
4. **The per-IP join cap** (LIM-IP-001). Default: `5`, exact addresses. The other choice for IPv6:
   one count for each `/64` network.
5. **A global join rate.** Default: none. Only the bucket of each address applies. A global limit
   also refuses real players during a flood from many addresses.
6. **The texts of LIM-IP-001 and LIM-RATE-001.** Default: English plain text. The other choice:
   translation keys of mcrs, as in `proxy-forwarding.md`, FU-03.
7. **The packet rate limit** (LIM-PKT-001). Default: `500`.
