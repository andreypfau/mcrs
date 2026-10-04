# mcrs_minecraft_protocol

A protocol library for _Minecraft: Java Edition_. Use this to build clients, servers, proxies, or something novel!

`mcrs_minecraft_protocol` is primarily concerned with defining all of Minecraft's [network packets](packets) and the process for encoding and decoding them. To encode and decode packets, use the [`PacketEncoder`] and [`PacketDecoder`] types.

```rust
use mcrs_minecraft_protocol::packets::login::clientbound::LoginCompression;
use mcrs_minecraft_protocol::{Decode, Packet, PacketDecoder, PacketEncoder, VarInt};

let mut encoder = PacketEncoder::new();

let packet = LoginCompression {
    threshold: VarInt(256),
};

// Encode our packet struct into a frame.
encoder.append_packet(&packet).unwrap();

// Take our encoded frame(s) out of the encoder.
let bytes = encoder.take();

let mut decoder = PacketDecoder::new();

// Put them in the decoder.
decoder.queue_bytes(bytes);

// Get the next packet "frame" from the decoder. A frame holds the packet's id and the
// bytes after it, so it can be thought of as a type-erased packet.
let frame = decoder.try_next_packet().unwrap().unwrap();
assert_eq!(frame.id, LoginCompression::ID);

// Decode the body into the packet struct we started from.
let decoded = LoginCompression::decode(&mut &frame.body[..]).unwrap();
assert_eq!(decoded.threshold, packet.threshold);
```

## Supported Minecraft Versions

`mcrs_minecraft_protocol` targets exactly one game version, the one read from `mcrs_minecraft_core::VERSION`. There is no support for any other version, and a new game version often entails a major version bump, since breaking changes to packet definitions are frequent.

## Differences from the game

A frame is compressed when its packet is at or above the compression threshold, a length prefix has at most three bytes, a zero length is refused, and an uncompressed frame may be longer than the threshold. These rules are shared with the game. Where MCRS differs, it differs on purpose:

- A compressed frame is checked on both sides. One whose declared size is below the threshold or above 8,388,608 bytes is refused whether MCRS is the server or the client. The game checks this on the server only, so MCRS as a client is stricter than a vanilla client.
- A compressed frame whose data inflates to more than its declared size, or which carries bytes after the end of its stream, is refused. The game inflates the declared count and ignores the rest.
- An empty packet is refused on write. With compression on, the game would write a frame that holds only a data length of zero; MCRS writes no such frame, because every packet starts with its id.
