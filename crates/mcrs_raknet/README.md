# mcrs_raknet

A sans-I/O implementation of the RakNet protocol (version 10/11) with an optional `tokio` UDP driver.

## Overview
- **Layer 2 Transport**: Provides offline status (MOTD), MTU discovery, reliability and ordering channels, fragmentation, and ACK/NACK mechanics.
- **Sans-I/O Architecture**: The core protocol state machine operates purely on byte buffers without coupling to network sockets or system clocks.
- **Minimal Steady-State Overhead**: Sequence sliding windows, bounded reassembly buffers, and frame packaging operate over caller-provided scratch buffers.
- **Transport Decoupling**: Upper session layers consume abstract payload slices/bytes without depending on RakNet-specific wire types.
