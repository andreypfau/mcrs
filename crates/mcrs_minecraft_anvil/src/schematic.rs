//! Sponge schematics, versions 2 and 3: a box of block states with the block entities in
//! it, read into a volume.

use std::io::{Cursor, Read};

use flate2::read::GzDecoder;
use mcrs_minecraft_core::{BlockPos, BlockStateId, BoundingBox};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_volume::{BoxVolume, VoxelStoreMut};

use crate::palette::{BlockStateList, PaletteLookup};

/// The blocks hold the box from `(0, 0, 0)`; `offset` is where the box's corner sat
/// when the schematic was saved.
#[derive(Clone, Debug)]
pub struct Schematic {
    pub blocks: BoxVolume,
    pub offset: BlockPos,
    pub block_entities: Vec<SchematicBlockEntity>,
}

/// A block entity's kind and its saved data, at a position inside the box.
#[derive(Clone, Debug, PartialEq)]
pub struct SchematicBlockEntity {
    pub pos: BlockPos,
    pub id: String,
    pub data: NbtCompound,
}

#[derive(Debug, thiserror::Error)]
pub enum SchematicError {
    #[error("reading the schematic: {0}")]
    Io(#[from] std::io::Error),
    #[error("decoding the schematic: {0}")]
    Nbt(#[from] mcrs_minecraft_nbt::Error),
    #[error("schematic version {0} is not 2 or 3")]
    Version(i32),
    #[error("the schematic has no {0}")]
    Missing(&'static str),
    #[error("the schematic is {0}×{1}×{2}, which holds no block")]
    Empty(i32, i32, i32),
    #[error("block data names palette index {0}, which the palette lacks")]
    Index(u32),
    #[error("block data ends before the box is full")]
    Short,
}

/// Reads a schematic file, gzip-compressed or not. A palette entry `blocks` cannot
/// resolve reads as air.
pub fn read_schematic(
    bytes: &[u8],
    blocks: &impl PaletteLookup<BlockStateId>,
) -> Result<Schematic, SchematicError> {
    let mut raw = Vec::new();
    let bytes = if bytes.starts_with(&[0x1f, 0x8b]) {
        GzDecoder::new(bytes).read_to_end(&mut raw)?;
        raw.as_slice()
    } else {
        bytes
    };
    let root: NbtCompound = mcrs_minecraft_nbt::from_bytes(Cursor::new(bytes))?;
    let schematic = root.get_compound("Schematic").unwrap_or(&root);
    let version = schematic
        .get_int("Version")
        .ok_or(SchematicError::Missing("Version"))?;
    let (container, palette_key, data_key) = match version {
        3 => (
            schematic
                .get_compound("Blocks")
                .ok_or(SchematicError::Missing("Blocks"))?,
            "Palette",
            "Data",
        ),
        2 => (schematic, "Palette", "BlockData"),
        other => return Err(SchematicError::Version(other)),
    };

    let size = |key: &'static str| {
        schematic
            .get_short(key)
            .map(|n| i32::from(n as u16))
            .ok_or(SchematicError::Missing(key))
    };
    let (width, height, length) = (size("Width")?, size("Height")?, size("Length")?);
    if width == 0 || height == 0 || length == 0 {
        return Err(SchematicError::Empty(width, height, length));
    }
    let offset = match schematic.get_int_array("Offset") {
        Some(&[x, y, z]) => BlockPos::new(x, y, z),
        _ => BlockPos::new(0, 0, 0),
    };

    let palette = container
        .get_compound(palette_key)
        .ok_or(SchematicError::Missing("Palette"))?;
    let mut states: Vec<BlockStateId> = Vec::new();
    for (key, index) in &palette.child_tags {
        let NbtTag::Int(index) = index else {
            continue;
        };
        let Ok(index) = usize::try_from(*index) else {
            continue;
        };
        let mut list = BlockStateList::default();
        let (name, properties) = split_state(key);
        list.push(name, properties);
        let state = blocks
            .resolve(list.name(0), list.properties(0))
            .unwrap_or(BlockStateId::AIR);
        if states.len() <= index {
            states.resize(index + 1, BlockStateId::AIR);
        }
        states[index] = state;
    }

    let data = match container.get(data_key) {
        Some(NbtTag::ByteArray(data)) => data,
        _ => return Err(SchematicError::Missing("block data")),
    };
    let bounds = BoundingBox {
        min: BlockPos::new(0, 0, 0),
        max: BlockPos::new(width - 1, height - 1, length - 1),
    };
    let mut volume = BoxVolume::filled(bounds, BlockStateId::AIR)
        .ok_or(SchematicError::Empty(width, height, length))?;
    let mut indices = varints(data);
    for y in 0..height {
        for z in 0..length {
            for x in 0..width {
                let index = indices.next().ok_or(SchematicError::Short)?;
                let state = *states
                    .get(index as usize)
                    .ok_or(SchematicError::Index(index))?;
                volume.set_voxel(BlockPos::new(x, y, z), state);
            }
        }
    }

    let block_entities = container
        .get_list("BlockEntities")
        .or_else(|| schematic.get_list("BlockEntities"))
        .unwrap_or_default()
        .iter()
        .filter_map(|entry| {
            let NbtTag::Compound(entry) = entry else {
                return None;
            };
            block_entity(entry, version)
        })
        .collect();

    Ok(Schematic {
        blocks: volume,
        offset,
        block_entities,
    })
}

/// Version 3 keeps the data under `Data`; version 2 keeps it beside `Pos` and `Id`.
fn block_entity(entry: &NbtCompound, version: i32) -> Option<SchematicBlockEntity> {
    let &[x, y, z] = entry.get_int_array("Pos")? else {
        return None;
    };
    let id = entry.get_string("Id")?.to_owned();
    let data = if version == 3 {
        entry.get_compound("Data").cloned().unwrap_or_default()
    } else {
        NbtCompound {
            child_tags: entry
                .child_tags
                .iter()
                .filter(|(key, _)| key != "Pos" && key != "Id")
                .cloned()
                .collect(),
        }
    };
    Some(SchematicBlockEntity {
        pos: BlockPos::new(x, y, z),
        id,
        data,
    })
}

/// `minecraft:oak_stairs[facing=east,half=top]` into its name and properties.
fn split_state(key: &str) -> (&str, Vec<(&str, &str)>) {
    let Some((name, rest)) = key.split_once('[') else {
        return (key, Vec::new());
    };
    let properties = rest
        .trim_end_matches(']')
        .split(',')
        .filter_map(|pair| pair.split_once('='))
        .collect();
    (name, properties)
}

/// Unsigned LEB128, seven bits a byte.
fn varints(data: &[u8]) -> impl Iterator<Item = u32> + '_ {
    let mut bytes = data.iter();
    std::iter::from_fn(move || {
        let mut value = 0u32;
        for shift in (0..35).step_by(7) {
            let byte = *bytes.next()?;
            value |= u32::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    })
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_volume::BlockVolume;

    use super::*;
    use crate::palette::Properties;

    /// Air, stone, and stairs that face east at palette index 130, past one varint byte.
    struct Table;

    impl PaletteLookup<BlockStateId> for Table {
        fn resolve(&self, name: &str, properties: Properties<'_>) -> Option<BlockStateId> {
            let number = match (name, properties.get("facing")) {
                ("minecraft:air", _) => 0,
                ("minecraft:stone", _) => 1,
                ("minecraft:oak_stairs", Some("east")) => 2,
                _ => return None,
            };
            BlockStateId::checked(number, 3)
        }
    }

    fn state(number: u16) -> BlockStateId {
        BlockStateId::checked(number, 3).expect("a state of the table")
    }

    fn v3() -> Vec<u8> {
        let mut palette = NbtCompound::new();
        palette.put_int("minecraft:air", 0);
        palette.put_int("minecraft:stone", 1);
        palette.put_int("minecraft:oak_stairs[facing=east,half=bottom]", 130);
        let mut chest = NbtCompound::new();
        chest.put("Pos", NbtTag::IntArray(vec![1, 0, 1]));
        chest.put_string("Id", "minecraft:chest".to_owned());
        let mut data = NbtCompound::new();
        data.put_string("LootTable", "minecraft:chests/igloo_chest".to_owned());
        chest.put_component("Data", data.clone());
        let mut blocks = NbtCompound::new();
        blocks.put_component("Palette", palette);
        // x fastest, then z: (0,0,0) air, (1,0,0) stone, (0,0,1) stairs, (1,0,1) stone.
        blocks.put("Data", NbtTag::ByteArray(Box::new([0, 1, 0x82, 0x01, 1])));
        blocks.put_list("BlockEntities", vec![NbtTag::Compound(chest)]);
        let mut schematic = NbtCompound::new();
        schematic.put_int("Version", 3);
        schematic.put_short("Width", 2);
        schematic.put_short("Height", 1);
        schematic.put_short("Length", 2);
        schematic.put("Offset", NbtTag::IntArray(vec![10, 64, -3]));
        schematic.put_component("Blocks", blocks);
        let mut root = NbtCompound::new();
        root.put_component("Schematic", schematic);
        let mut bytes = Vec::new();
        mcrs_minecraft_nbt::to_bytes(&root, &mut bytes).expect("an encodable schematic");
        bytes
    }

    #[test]
    fn a_version_three_schematic_reads_into_a_volume_with_its_block_entities() {
        let schematic = read_schematic(&v3(), &Table).expect("a valid schematic");

        let block = |x, z| schematic.blocks.block(BlockPos::new(x, 0, z));
        assert_eq!(block(0, 0), Ok(state(0)));
        assert_eq!(block(1, 0), Ok(state(1)));
        assert_eq!(block(0, 1), Ok(state(2)));
        assert_eq!(block(1, 1), Ok(state(1)));
        assert_eq!(schematic.offset, BlockPos::new(10, 64, -3));
        let [chest] = schematic.block_entities.as_slice() else {
            panic!("{:?}", schematic.block_entities);
        };
        assert_eq!(chest.pos, BlockPos::new(1, 0, 1));
        assert_eq!(chest.id, "minecraft:chest");
        assert_eq!(
            chest.data.get_string("LootTable"),
            Some("minecraft:chests/igloo_chest")
        );
    }
}
