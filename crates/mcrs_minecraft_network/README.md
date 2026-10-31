# mcrs_minecraft_network

## LAN announcement

A server that listens beyond loopback announces itself on the local network, so a vanilla client lists it under its LAN worlds. The announcement is the game's datagram, `[MOTD]<motd>[/MOTD][AD]<port>[/AD]`, sent to the multicast group `224.0.2.60` on port `4445` every 1.5 seconds, the first one at once. The MOTD is the one the status response carries and the port is the one the server listens on. The sending socket is bound to the IPv4 address the server listens on, or to any IPv4 address when it listens on every interface. The game announces over IPv4 only and its client joins the address the datagram came from, so a server listening on one IPv6 address does not announce, and logs that once at startup.

The server plugin's `announce_on_lan` turns the announcement off; no socket is opened then. The `mcrs` server binary sets it from `MCRS_LAN_ANNOUNCE` (`off`, `0`, `false` or `no` in any case). A server that listens on loopback never announces, whatever the setting says. The announcement ends when the server is dropped.

It differs from the game on purpose:

- The game announces only a singleplayer world that was opened to LAN. A dedicated MCRS server announces itself by default.
- The game stops announcing for good after one failed send. This server keeps trying on every interval, and logs once when sending starts to fail and once when it recovers.
- A MOTD that the client's parser cannot read, one that contains `[/MOTD]`, is not sent. The reason is logged once. The datagram length is not checked: the MOTD is a constant far below the 1024 bytes the client reads.

## Transfers

A client that a transfer packet sends to the server opens its connection with the transfer intent. The server plugin's `accepts_transfers` lets such a connection log in as a direct one, with the same version check. When it is off, which is the default, the client gets `multiplayer.disconnect.transfers_disabled`. The `mcrs` server binary turns it on from `MCRS_ACCEPT_TRANSFERS` (`on`, `1`, `true` or `yes` in any case).

## Status

The status response follows [docs/spec/status.md](../../docs/spec/status.md). The server plugin's `hide_online_players` leaves the sample of online players out of it, while the online and maximum counts stay. It is off by default, and the `mcrs` server binary turns it on from `MCRS_HIDE_ONLINE_PLAYERS` (`on`, `1`, `true` or `yes` in any case). With `enable_status` off, the server closes a status connection after the handshake without sending a packet; a login is not affected. It is on by default, and the binary turns it off from `MCRS_ENABLE_STATUS` (`off`, `0`, `false` or `no` in any case).
