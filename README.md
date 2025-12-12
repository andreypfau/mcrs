# mcrs

A voxel engine written in Rust, built on [Bevy](https://bevyengine.org) ECS, inspired
by [Spout](https://github.com/spoutdev/Spout), with a Minecraft gameplay implementation.

## Structure

- **mcrs_engine** - Voxel engine core
- **mcrs_minecraft** - Minecraft gameplay implementation
- **mcrs_protocol** - Minecraft protocol implementation
- **mcrs_nbt** - Minecraft NBT serialization implementation

## Status

Early development.

## Build

```bash
cargo build --release
cargo run --release
```
