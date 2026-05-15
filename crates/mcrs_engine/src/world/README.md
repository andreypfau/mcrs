# `world/` — voxel-world spatial hierarchy

This module owns the engine-wide spatial vocabulary. All voxel-coordinate
types and the ECS scaffolding that gives chunks, columns, and dimensions
their identity live here.

## Three spatial levels

| Level     | Type                                                    | Geometry              | Purpose                                                                 |
|-----------|---------------------------------------------------------|-----------------------|-------------------------------------------------------------------------|
| Chunk     | [`storage::chunk::Chunk`] marker on a chunk entity      | 16 × 16 × 16 blocks   | Unit of voxel storage, light propagation, and palette compression.      |
| Column    | [`storage::column::Column`] marker on a column entity   | 16 × Y × 16 blocks    | Vertical stack of chunks; owns the heightmaps and the chunk index used for the network wire format. |
| Dimension | [`dimension::Dimension`] marker on a dimension entity   | unbounded             | Top-level world container; owns the column index and chunk index for all chunks belonging to that dimension. |

A chunk is **not** a "section" — the cube of blocks is the canonical
chunk. A column is **not** a kind of chunk — it is the parent stack
that aggregates chunks for streaming and heightmap maintenance.

## Submodules

```
world/
├── block.rs          re-export of geometry::BlockPos
├── chunk.rs          backward-compat façade over storage::chunk + lifecycle::markers + lifecycle::ticket + storage::palette
├── column.rs         backward-compat façade over storage::column
├── dimension.rs      Dimension, DimensionId, DimensionTypeConfig, InDimension, DimensionBundle, HasSkyLight
├── lighting.rs       backward-compat re-export of lifecycle::ticket::LightTicket
├── region.rs         re-export of geometry::RegionPos
├── lifecycle/
│   ├── markers.rs    sparse-marker lifecycle states (ChunkLoaded, ChunkLoading, ChunkGenerating, ChunkUnloading, ChunkUnloaded)
│   └── ticket.rs     chunk-ticket plumbing and the engine-wide LightTicket marker
└── storage/
    ├── chunk.rs      Chunk, ChunkBundle, ChunkIndex, ChunkPlugin
    ├── column.rs     Column, ColumnBundle, ColumnChunks, ChunkLookup, Heightmaps, PackedBitStorage, ColumnIndex, ColumnSlot, InColumn, ColumnLifecycleSet, ColumnPlugin
    └── palette.rs    PalettedContainer, AbstractCube, HeterogeneousPaletteData
```

## Position types

All position types live one level up, in `mcrs_engine::geometry`:

- `BlockPos` — 3D world-block position
- `ChunkPos` — 3D chunk position (cubic, `IVec3`)
- `ColumnPos` — 2D column position (x, z)
- `RegionPos` — 3D region position (16³ chunks per region)

They are pure coordinate-arithmetic types with `From` conversions
between adjacent levels; they carry no ECS state. Both `mcrs_engine`
and `mcrs_protocol` import `ColumnPos` from the same source.
