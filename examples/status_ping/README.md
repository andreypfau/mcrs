# Status ping

Query a Java Edition server and print its version, description, online and
maximum player counts, and ping/pong round-trip time in milliseconds.

```sh
cargo run -p mcrs_minecraft_status_ping -- localhost 25565
```

The first argument is a hostname, IPv4 address, or IPv6 address without brackets.
The optional second argument is the port, which defaults to 25565. The whole
exchange has a five-second timeout, including name resolution and connection.
The description follows the string format in `docs/spec/status.md`; an omitted
description prints as empty. A malformed response or mismatched pong is an error.

The executable uses the protocol and network crates without Bevy. Its integration
test starts an embedded `NetworkPlugin` server on a free loopback port and checks
the executable's output against the plugin settings and
`mcrs_minecraft_core::VERSION`. Bevy is only a development dependency.

```sh
cargo nextest run -p mcrs_minecraft_status_ping
cargo nextest run -p xtask
```

The exchange follows STATUS-SEQ-001 and STATUS-JSON-001 through STATUS-JSON-005
in `docs/spec/status.md`.
