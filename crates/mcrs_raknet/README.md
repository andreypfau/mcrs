# mcrs_raknet

A sans-I/O implementation of the RakNet protocol (version 10/11) with an optional `tokio` UDP driver.

## Overview
- **Layer 2 Transport**: Provides offline status (MOTD), MTU discovery, reliability and ordering channels, fragmentation, and ACK/NACK mechanics.
- **Sans-I/O Architecture**: The core protocol state machine operates purely on byte buffers without coupling to network sockets or system clocks.
- **Bounded State**: Receive windows, reassembly buffers (per session and per server), reorder queues and in-flight datagrams have fixed ceilings. The steady path allocates; it is not allocation-free.
- **Receipts**: The ack-receipt reliability modes are encoded on the wire, but a receipt result is not reported to the caller.
- **Transport Decoupling**: Upper session layers consume abstract payload slices/bytes without depending on RakNet-specific wire types.
