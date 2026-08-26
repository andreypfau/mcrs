use bevy_ecs::component::Component;
use mcrs_voxel_math::chunk_pos;
use mcrs_voxel_math::BlockPos;
use mcrs_engine::world::storage::palette::PalettedContainer;
use mcrs_engine::world::storage::palette::PalettedContainer::{Heterogeneous, Homogeneous};
use mcrs_palette::{PaletteForm, SectionKind};
use mcrs_protocol::BlockStateId;

impl BiomePalette {
    /// Set the biome id for a 4x4x4 biome cell within this section.
    /// `cell_x`, `cell_y`, `cell_z` are biome-cell indices (0..4).
    /// `id` is the u8 network registry id for the biome.
    pub fn set_cell(&mut self, cell_x: usize, cell_y: usize, cell_z: usize, id: u8) {
        self.0.set(cell_x, cell_y, cell_z, id);
    }

    pub fn convert_network(&self) -> mcrs_protocol::chunk::PalettedContainer<u8> {
        match &self.0 {
            Homogeneous(registry_id) => mcrs_protocol::chunk::PalettedContainer {
                bits_per_entry: 0,
                palette: mcrs_protocol::chunk::Palette::Single(*registry_id),
                packed_data: Box::new([]),
            },
            Heterogeneous(data) => match mcrs_palette::Biomes::network_form(data.counts.len()) {
                PaletteForm::Single => unreachable!("a heterogeneous container has two entries"),
                PaletteForm::Indirect { bits } => {
                    let (palette, packed) = self.0.to_palette_and_packed_data(bits as u8);
                    mcrs_protocol::chunk::PalettedContainer {
                        bits_per_entry: bits as u8,
                        palette: mcrs_protocol::chunk::Palette::Indirect(palette),
                        packed_data: packed,
                    }
                }
                PaletteForm::Direct { bits } => {
                    let cells = data.cube.as_flattened().as_flattened();
                    mcrs_protocol::chunk::PalettedContainer {
                        bits_per_entry: bits as u8,
                        palette: mcrs_protocol::chunk::Palette::Direct,
                        packed_data: mcrs_palette::pack_from(bits, cells, |&id| id as u32),
                    }
                }
            },
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
            Heterogeneous(data) => match mcrs_palette::Blocks::network_form(data.counts.len()) {
                PaletteForm::Single => unreachable!("a heterogeneous container has two entries"),
                PaletteForm::Indirect { bits } => {
                    let (palette, packed) = self.0.to_palette_and_packed_data(bits as u8);
                    mcrs_protocol::chunk::PalettedContainer {
                        bits_per_entry: bits as u8,
                        palette: mcrs_protocol::chunk::Palette::Indirect(palette),
                        packed_data: packed,
                    }
                }
                PaletteForm::Direct { bits } => {
                    let cells = data.cube.as_flattened().as_flattened();
                    mcrs_protocol::chunk::PalettedContainer {
                        bits_per_entry: bits as u8,
                        palette: mcrs_protocol::chunk::Palette::Direct,
                        packed_data: mcrs_palette::pack_from(bits, cells, |id| id.0 as u32),
                    }
                }
            },
        }
    }

    // Coupling: this method assumes `BlockStateId(0)` is the air state, which
    // is the current vanilla convention but is not enforced by `BlockPalette`
    // itself. Reordering the static block registry so air ends up at a
    // different ID would silently break this count. The principled fix is to
    // consult the `IS_NOT_AIR` bit from `BlockStateLightTable.flags_for`, but that
    // requires plumbing the table through `non_air_block_count`'s callers in
    // `column_view`. Tracked as a follow-up.
    pub fn non_air_block_count(&self) -> u16 {
        match &self.0 {
            Homogeneous(registry_id) => {
                if **registry_id != 0 {
                    chunk_pos::BLOCKS::VOLUME as u16
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
            pos.x as usize & chunk_pos::BLOCKS::MASK,
            pos.y as usize & chunk_pos::BLOCKS::MASK,
            pos.z as usize & chunk_pos::BLOCKS::MASK,
        )
    }

    pub fn set<I: Into<BlockPos>, B: Into<BlockStateId>>(
        &mut self,
        pos: I,
        block: B,
    ) -> BlockStateId {
        let pos = pos.into();
        self.0.set(
            pos.x as usize & chunk_pos::BLOCKS::MASK,
            pos.y as usize & chunk_pos::BLOCKS::MASK,
            pos.z as usize & chunk_pos::BLOCKS::MASK,
            block.into(),
        )
    }

    /// Fill the box `[x0, x1) x [y0, y1) x [z0, z1)` (section-local coords)
    /// with `block`. Produces output identical to per-block `set` calls over
    /// the same box, with bulk-optimized palette bookkeeping.
    pub fn fill_box<B: Into<BlockStateId>>(
        &mut self,
        x0: usize,
        x1: usize,
        y0: usize,
        y1: usize,
        z0: usize,
        z1: usize,
        block: B,
    ) {
        self.0.fill_box(x0, x1, y0, y1, z0, z1, block.into());
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

#[derive(Component, Debug, Clone, Default)]
pub struct BiomePalette(PalettedContainer<u8, 4>);
