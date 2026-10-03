# mcrs_minecraft_network

## LAN announcement

A server that listens beyond loopback announces itself on the local network, so a vanilla client lists it under its LAN worlds. The announcement is the game's datagram, `[MOTD]<motd>[/MOTD][AD]<port>[/AD]`, sent to the multicast group `224.0.2.60` on port `4445` every 1.5 seconds, the first one at once. The MOTD is the one the status response carries and the port is the one the server listens on. The sending socket is bound to the IPv4 address the server listens on, or to any IPv4 address when it listens on every interface. The game announces over IPv4 only and its client joins the address the datagram came from, so a server listening on one IPv6 address does not announce, and logs that once at startup.

`MCRS_LAN_ANNOUNCE=off` turns the announcement off; `0`, `false` and `no` do the same, and any other value leaves it on. No socket is opened then. A server that listens on loopback never announces, whatever the setting says. The announcement ends when the server is dropped.

It differs from the game on purpose:

- The game announces only a singleplayer world that was opened to LAN. A dedicated MCRS server announces itself by default.
- The game stops announcing for good after one failed send. This server keeps trying on every interval, and logs once when sending starts to fail and once when it recovers.
- A MOTD that the client's parser cannot read, one that contains `[/MOTD]` or that makes the datagram longer than the 1024 bytes the client reads, is not sent. The reason is logged once.
