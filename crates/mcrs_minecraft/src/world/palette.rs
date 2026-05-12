use bevy_ecs::component::Component;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk;
use mcrs_engine::world::chunk::palette::PalettedContainer::{Heterogeneous, Homogeneous};
use mcrs_engine::world::chunk::palette::{PalettedContainer, encompassing_bits};
use mcrs_protocol::BlockStateId;

impl BiomePalette {
    pub fn convert_network(&self) -> mcrs_protocol::chunk::PalettedContainer<u8> {
        match &self.0 {
            Homogeneous(registry_id) => mcrs_protocol::chunk::PalettedContainer {
                bits_per_entry: 0,
                palette: mcrs_protocol::chunk::Palette::Single(*registry_id),
                packed_data: Box::new([]),
            },
            Heterogeneous(data) => {
                let raw_bits_per_entry = encompassing_bits(data.counts.len());
                if raw_bits_per_entry > BIOME_NETWORK_MAX_MAP_BITS {
                    let bits_per_entry = BIOME_NETWORK_MAX_BITS;
                    let values_per_i64 = 64 / bits_per_entry;
                    let packed_data = data
                        .cube
                        .as_flattened()
                        .as_flattened()
                        .chunks(values_per_i64 as usize)
                        .map(|chunk| {
                            chunk.iter().enumerate().fold(0, |acc, (index, value)| {
                                debug_assert!((1 << bits_per_entry) > *value);
                                let packed_offset_index =
                                    (*value as u64) << (bits_per_entry as u64 * index as u64);
                                acc | packed_offset_index as i64
                            })
                        })
                        .collect();

                    mcrs_protocol::chunk::PalettedContainer {
                        bits_per_entry,
                        palette: mcrs_protocol::chunk::Palette::Direct,
                        packed_data,
                    }
                } else {
                    let bits_per_entry = raw_bits_per_entry.max(BIOME_NETWORK_MIN_MAP_BITS);
                    let (palette, packed) = self.0.to_palette_and_packed_data(bits_per_entry);

                    mcrs_protocol::chunk::PalettedContainer {
                        bits_per_entry,
                        palette: mcrs_protocol::chunk::Palette::Indirect(palette),
                        packed_data: packed,
                    }
                }
            }
        }
    }
}

impl BlockPalette {
    pub fn convert_network(&self) -> mcrs_protocol::chunk::PalettedContainer<BlockStateId> {
        match &self.0 {
            Homogeneous(registry_id) => mcrs_protocol::chunk::PalettedContainer {
                bits_per_entry: 0,
                palette: mcrs_protocol::chunk::Palette::Single(*registry_id),
                packed_data: Box::new([]),
            },
            Heterogeneous(data) => {
                let raw_bits_per_entry = encompassing_bits(data.counts.len());
                if raw_bits_per_entry > BLOCK_NETWORK_MAX_MAP_BITS {
                    let bits_per_entry = BLOCK_NETWORK_MAX_BITS;
                    let values_per_i64 = 64 / bits_per_entry;
                    let packed_data = data
                        .cube
                        .as_flattened()
                        .as_flattened()
                        .chunks(values_per_i64 as usize)
                        .map(|chunk| {
                            chunk.iter().enumerate().fold(0, |acc, (index, value)| {
                                // debug_assert!((1 << bits_per_entry) > *value);

                                let packed_offset_index =
                                    (**value as i64) << (bits_per_entry as u64 * index as u64);
                                acc | packed_offset_index
                            })
                        })
                        .collect();

                    mcrs_protocol::chunk::PalettedContainer {
                        bits_per_entry,
                        palette: mcrs_protocol::chunk::Palette::Direct,
                        packed_data,
                    }
                } else {
                    let bits_per_entry = raw_bits_per_entry.max(BLOCK_NETWORK_MIN_MAP_BITS);
                    let (palette, packed) = self.0.to_palette_and_packed_data(bits_per_entry);

                    mcrs_protocol::chunk::PalettedContainer {
                        bits_per_entry,
                        palette: mcrs_protocol::chunk::Palette::Indirect(palette),
                        packed_data: packed,
                    }
                }
            }
        }
    }

    // Coupling: this method assumes `BlockStateId(0)` is the air state, which
    // is the current vanilla convention but is not enforced by `BlockPalette`
    // itself. Reordering the static block registry so air ends up at a
    // different ID would silently break this count. The principled fix is to
    // consult the `IS_NOT_AIR` bit from `BlockLightTable.flags_for`, but that
    // requires plumbing the table through `non_air_block_count`'s callers in
    // `column_view`. Tracked as a follow-up.
    pub fn non_air_block_count(&self) -> u16 {
        match &self.0 {
            Homogeneous(registry_id) => {
                if **registry_id != 0 {
                    chunk::BLOCKS::VOLUME as u16
                } else {
                    0
                }
            }
            Heterogeneous(data) => data
                .palette
                .iter()
                .zip(data.counts.iter())
                .filter_map(|(registry_id, count)| {
                    if **registry_id != 0 {
                        Some(*count)
                    } else {
                        None
                    }
                })
                .sum(),
        }
    }

    pub fn fill<B: Into<BlockStateId>>(&mut self, block: B) {
        self.0 = Homogeneous(block.into());
    }

    pub fn get<I: Into<BlockPos>>(&self, pos: I) -> BlockStateId {
        let pos = pos.into();
        self.0.get(
            pos.x as usize & chunk::BLOCKS::MASK,
            pos.y as usize & chunk::BLOCKS::MASK,
            pos.z as usize & chunk::BLOCKS::MASK,
        )
    }

    pub fn set<I: Into<BlockPos>, B: Into<BlockStateId>>(
        &mut self,
        pos: I,
        block: B,
    ) -> BlockStateId {
        let pos = pos.into();
        self.0.set(
            pos.x as usize & chunk::BLOCKS::MASK,
            pos.y as usize & chunk::BLOCKS::MASK,
            pos.z as usize & chunk::BLOCKS::MASK,
            block.into(),
        )
    }

    /// Invoke `f` once for each distinct `BlockStateId` present in the
    /// container. A homogeneous container yields exactly one state.
    /// A heterogeneous container yields every entry in its palette without
    /// duplicates.
    pub fn for_each_distinct_state<F: FnMut(BlockStateId)>(&self, mut f: F) {
        match &self.0 {
            Homogeneous(state) => f(*state),
            Heterogeneous(data) => {
                for state in data.palette.iter() {
                    f(*state);
                }
            }
        }
    }
}

// According to the wiki, palette serialization for disk and network is different. Disk
// serialization always uses a palette if greater than one entry. Network serialization packs ids
// directly instead of using a palette above a certain bits-per-entry

#[derive(Component, Debug, Clone, Default)]
pub struct BlockPalette(PalettedContainer<BlockStateId, 16>);
const BLOCK_DISK_MIN_BITS: u8 = 4;
const BLOCK_NETWORK_MIN_MAP_BITS: u8 = 4;
const BLOCK_NETWORK_MAX_MAP_BITS: u8 = 8;
pub(crate) const BLOCK_NETWORK_MAX_BITS: u8 = 15;

#[derive(Component, Debug, Clone, Default)]
pub struct BiomePalette(PalettedContainer<u8, 4>);
const BIOME_DISK_MIN_BITS: u8 = 0;
const BIOME_NETWORK_MIN_MAP_BITS: u8 = 1;
const BIOME_NETWORK_MAX_MAP_BITS: u8 = 3;
pub(crate) const BIOME_NETWORK_MAX_BITS: u8 = 7;
