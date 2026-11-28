# Loopback operators: behavioural specification

| Field | Value |
|-------|-------|
| Status | `ready` |

This document states when a player who joins from a loopback address becomes an operator with
no entry in the op list. This rule is a convenience of mcrs for a game on one machine. The
vanilla game has no such rule (section 5).

A public server can have a proxy or a tunnel on the same machine. Then each player arrives from
a loopback address. So the server gives the automatic grant only on the local profile. On the
public profile, only the op list makes a player an operator.

This document extends `login.md` and `command-permissions.md`. It uses the client address of
`proxy-forwarding.md` and of `proxy-protocol.md`.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **login**: as `login.md`, section 0.1, defines them.
- **Operator**, **level**: as `command-permissions.md` defines them. The level names are those
  of PERM-LEVEL-001.
- **Op list**: the operators that the server holds in memory.
- **Ops file**: the file `ops.json` in the directory of the server. The server reads the op
  list from it at start. After each `op` or `deop`, it writes each entry of the op list to the
  file, except the entries of automatic operators (LOOP-FILE-001).
- **Listed player**: a player whose profile id is on the op list, with the level of that entry,
  and who is not an automatic operator.
- **Client address**: the address of the client of a connection, after the changes of
  `proxy-protocol.md`, PXP-USE-001, and of `proxy-forwarding.md`, FWD-USE-002. Without these
  changes, it is the address of the socket.
- **Loopback address**: an IPv4 address in `127.0.0.0/8`, the IPv6 address `::1`, or an
  IPv4-mapped IPv6 address `::ffff:a.b.c.d` whose IPv4 part is in `127.0.0.0/8`.
- **Loopback join**: a login that the server accepts, when its client address is a loopback
  address.
- **Profile**: an input of the server network plugin with two values, `local` and `public`.
- **Automatic operator**: a player whom LOOP-GRANT-001 makes an operator.

### 0.2 Provenance and confidence

This document uses the labels of `registry.md`, section 0.3. The rules of sections 1 to 4 are
decisions of mcrs. They have the label `design`. No observer of the vanilla game
can confirm them. Section 5 states the vanilla behaviour and its sources.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `LOOP-CFG` | server inputs |
| `LOOP-ADDR` | the address that the check reads |
| `LOOP-GRANT` | the result of a loopback join per profile |
| `LOOP-FILE` | the ops file and the life of an automatic operator |

### 0.4 Scope

In scope: the operator status and the level of a player after a loopback join.

Out of scope:

- the owner of an integrated server (`login.md`, LOGIN-CFG-002). The host application puts the
  owner on the op list. That entry is a listed player for this document.
- the `op` and `deop` commands, except for their effect on an automatic operator.

## 1. Server inputs

### LOOP-CFG-001: The profile

- **Behavior:** the host application sets the profile through a field of the server network
  plugin. The profile is either `local` or `public`.
- **Behavior:** the default of the plugin is `public`. The plugin for an embedded server also
  has the default `public`.
- **Behavior:** the engine and its plugins do not read an environment variable for the
  profile. An environment variable that the server binary reads does not change the profile.
- **Behavior:** the profile changes only the result of a loopback join (section 3). It does
  not change other settings of the server.
- **Postconditions:** a library user who adds the plugin with its default gets no automatic
  operator.

This document does not state the name or the type of the field.

Provenance: `design`. Kind: semantic. Confidence: high.

### LOOP-CFG-002: The profile of the server binary

- **Behavior:** the `mcrs` server binary uses the `public` profile, unless its configuration
  selects `local` explicitly.
- **Behavior:** an environment variable does not select `local` (LOOP-CFG-001). The binary
  has no other source of settings, so it always uses `public`.

Provenance: `design` (the owner selected `public` for the binary in the review of 2026-11-16).
Kind: semantic. Confidence: high.

## 2. The address of the check

### LOOP-ADDR-001: The client address decides

- **Behavior:** the server reads the client address of the connection at the time it accepts
  the login. It does not read the address of the socket.
- **Behavior:** a login that Velocity forwards (`proxy-forwarding.md`) is a loopback join only
  when its forwarded address is a loopback address. The address of the proxy does not count.
- **Behavior:** a connection with a PROXY protocol header (`proxy-protocol.md`) is a loopback
  join only when the client address of PXP-USE-001 is a loopback address. A header that keeps
  the socket address keeps the socket address for this check too.
- **Behavior:** an IPv4-mapped IPv6 address counts as its IPv4 address.

Examples:

| Client address | Loopback address |
|----------------|------------------|
| `127.0.0.1` | yes |
| `127.255.0.9` | yes |
| `::1` | yes |
| `::ffff:127.0.0.1` | yes |
| `192.168.1.2`, `10.0.0.1`, `0.0.0.0`, `::`, `::ffff:10.0.0.1`, `fe80::1` | no |

Provenance: `design`. Kind: semantic. Confidence: high.

## 3. The result per profile

### LOOP-GRANT-001: Local profile

- **Behavior:** on the `local` profile, a loopback join of a player who is not a listed player
  makes the player an operator at the level `owners`.
- **Behavior:** the automatic operator does not get the exception to the player limit that an
  entry of the ops file can give.
- **Behavior:** a loopback join of a listed player does not change the entry. The player keeps
  the level of the entry.
- **Behavior:** a join whose client address is not a loopback address does not make an
  operator.
- **Postconditions:** the client receives the level of the player as `command-permissions.md`,
  PERM-LEVEL-003, states. For an automatic operator, that is the `entity_event` 28.

Provenance: `design` ("as it does today"). Kind: semantic. Confidence: high.

### LOOP-GRANT-002: Public profile

- **Behavior:** on the `public` profile, a loopback join is the same as a join from each other
  address. Only the op list makes the player an operator.
- **Behavior:** a listed player is an operator at the level of the entry.
- **Behavior:** a player who is not a listed player has the level `all`
  (`command-permissions.md`, PERM-LEVEL-002).
- **Postconditions:** a proxy or a tunnel on the machine of the server does not make an
  operator.

Provenance: `design`. Kind: semantic. Confidence: high.

### LOOP-GRANT-003: A refused login makes no operator

- **Behavior:** the grant of LOOP-GRANT-001 occurs only after the server accepts the login. A
  login that the ban list, the whitelist or a failed forwarding check refuses makes no operator.
- **Behavior:** the automatic grant does not let a player pass the whitelist check of the same
  login.

Provenance: `design`. Kind: semantic. Confidence: medium.

## 4. The ops file

### LOOP-FILE-001: The ops file does not hold an automatic operator

- **Behavior:** the server does not write an automatic operator to the ops file. A later
  write of the ops file, after an `op` or a `deop` of another player, also leaves the automatic
  operator out.
- **Behavior:** the automatic operator stays an operator until the process ends or until a
  `deop` of that player. An `op` of that player does not change this (LOOP-FILE-002).
- **Behavior:** during this time, a join of that player from another address finds the player
  an operator. So that join also passes the whitelist check, as for each operator
  (`whitelist.md`, WL-JOIN-004).
- **Behavior:** after a `deop`, the next loopback join of that player on the `local` profile
  makes the player an operator again.
- **Postconditions:** after a restart of the server, the op list holds only the entries of the
  ops file.

Provenance: `design` (the behaviour of mcrs before this rule). Kind: semantic. Confidence:
high.

### LOOP-FILE-002: `op` on an automatic operator

- **Behavior:** an `op` of an automatic operator fails with `commands.op.failed`, as for each
  other operator.
- **Behavior:** the `op` does not write the ops file. The player stays an automatic operator.

Provenance: `design` (the owner kept the behaviour of mcrs before this rule, in the review of
2026-11-16). Kind: semantic. Confidence: high.

## 5. The vanilla server

This section is informative. It states no rule, and no rule of this document depends on it.
Its sources are the public pages on `ops.json`, `server.properties` and "Open to LAN".

The vanilla dedicated server reads the operators only from `ops.json`. The `op` and `deop`
commands and the console edit that file. No rule of the vanilla dedicated server reads the
client address to give an operator level. A player who joins from `127.0.0.1` without an entry
has the level `all`. So the `public` profile agrees with the vanilla dedicated server.

The vanilla integrated server gives the level of its owner from the option that allows
commands in the world. A player who joins a world that is open to LAN gets no level from the
address, also when the player joins from the same machine. The `local` profile has no vanilla
counterpart.

## 7. Intentional deviations

- **Rule:** LOOP-GRANT-001.
  - **Reference:** the vanilla game gives no operator level from the client address.
  - **Reason:** a developer who runs the server and the client on one machine gets the
    commands with no edit of `ops.json`. The `public` profile has no such grant.
  - **Visible to the client:** yes, on the `local` profile only. A player from a loopback
    address receives the level `owners` with no entry in `ops.json`.
  - **Bug reference:** none.
  
## 8. Unresolved

None.
