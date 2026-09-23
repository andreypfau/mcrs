# `world/` — voxel-world spatial hierarchy

This module owns the ECS scaffolding that gives sections, columns, and
dimensions their identity.

## Three spatial levels

| Level     | Type                                                     | Geometry            | Purpose                                                                 |
|-----------|----------------------------------------------------------|---------------------|-------------------------------------------------------------------------|
| Section   | [`storage::section::Section`] marker on a section entity | 16 × 16 × 16 blocks | Unit of voxel storage, light propagation, and palette compression.      |
| Column    | [`storage::column::Column`] marker on a column entity    | 16 × Y × 16 blocks  | Vertical stack of sections; owns the heightmaps and the section slots used for the network wire format. |
| Dimension | [`dimension::Dimension`] marker on a dimension entity    | unbounded           | Top-level world container; owns the column index and section index for everything in that dimension. |

The reference calls the column `ChunkPos` and the cube `SectionPos`; only the
second name is taken, because a bare "chunk" does not say which of the two it
means.

## Submodules

```
world/
├── channels.rs       host ↔ dimension channel types
├── dimension.rs      Dimension, DimensionTypeConfig, InDimension, DimensionBundle, HasSkyLight
├── in_flight.rs      moves between dimensions awaiting their destination's acknowledgement
├── sub_app.rs        per-dimension sub-app labels and spawn/despawn queues
├── lifecycle/
│   ├── level.rs      FullStatus, the incremental 3D level field, SectionLevels
│   ├── stage.rs      SectionStage, the SectionStageChanged message every transition writes, SectionStages
│   ├── ticket.rs     SectionTickets, simulation tickets, level propagation, spawn and despawn of section entities
│   └── trace.rs      per-column stage trace for diagnostics
└── storage/
    ├── block_entity.rs  InSection, SectionBlockEntities, BlockEntityPos
    ├── column.rs        Column, ColumnBundle, ColumnSections, SectionLookup, ColumnIndex, ColumnSlot, ColumnLifecycleSet, ColumnPlugin
    └── section.rs       Section, SectionBundle, SectionIndex, SectionPlugin
```

## Position types

`BlockPos`, `SectionPos` and `ColumnPos` live in `mcrs_minecraft_core`. They
are pure coordinate arithmetic with `From` conversions between adjacent levels
and carry no ECS state.
