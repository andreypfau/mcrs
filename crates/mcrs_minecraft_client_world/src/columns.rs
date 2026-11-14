use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use bevy::app::{App, Plugin, Update};
use bevy::ecs::change_detection::DetectChangesMut;
use bevy::ecs::message::{Message, MessageWriter};
use bevy::ecs::prelude::{
    Commands, Entity, IntoScheduleConfigs, Local, On, Res, ResMut, Resource, Single,
};
use bevy::ecs::schedule::SystemSet;
use bevy::log::{error, warn};
use bevy::math::IVec3;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_block::block_entity_type_keys::BlockEntityType;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block_entity::{SignTexts, SignUpdate};
use mcrs_minecraft_chunk::section::Biomes;
use mcrs_minecraft_chunk::{LightStorage, PalettedContainer, SectionKind};
use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_entity_visual::TickingArea;
use mcrs_minecraft_light_engine::block_light::block_light_registry;
use mcrs_minecraft_protocol::{Decode, WritePacket};
use mcrs_minecraft_protocol_game::ColumnPos;
use mcrs_minecraft_protocol_game::chunk::{ChunkData, ChunkDataBlockEntity, LightChunk, LightData};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundBlockChangedAck, ClientboundBlockEntityData, ClientboundBlockUpdate,
    ClientboundChunkBatchFinished, ClientboundChunkBatchStart, ClientboundForgetLevelChunk,
    ClientboundLevelChunkWithLight, ClientboundLightUpdate, ClientboundLogin, ClientboundRespawn,
    ClientboundSectionBlocksUpdate,
};
use mcrs_minecraft_protocol_game::light_codec::{ColumnLight, RowLight, unpack_light_data};
use mcrs_minecraft_protocol_game::player::PlayerSpawnInfo;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChunkBatchReceived;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::{BlockPos, BlockStateId, LocalPos, SectionPos};

use crate::block_prediction::BlockPredictions;
use crate::client_light;
use crate::joined_game::CurrentDimension;
use crate::sign::sign_block;
use mcrs_minecraft_network::Instant;
use mcrs_minecraft_network::client::{
    ClientConnection, ClientNetworkSystems, DeferredRows, close_connection_later,
};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_registries::{CurrentRegistries, Registries};
use mcrs_minecraft_volume::{
    Access, BiomeVolume, BlockVolume, DenseSection, LightVolume, SectionVolume, Unavailable, Volume,
};

use crate::registries::{SessionSet, entered_configuration};

/// The network palette is the server's global one, so no remap stands between a
/// stored state and the block catalog.
pub const AIR: BlockStateId = BlockStateId::AIR;

/// Sky nibble full, block nibble dark: what a block outside every resident
/// column is lit by.
const OPEN_SKY: u8 = 0x0f;

/// The dimension's vertical reach in sections, from its `dimension_type`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Extent {
    pub min_section_y: i32,
    pub sections: usize,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Dimension {
    pub name: String,
    pub extent: Extent,
    /// The dimension type has sky light, so the client lights the sky layer of a block change.
    pub sky_light: bool,
}

/// A dimension with no name and with sky light, for a store that only ever holds one.
impl From<Extent> for Dimension {
    fn from(extent: Extent) -> Self {
        Dimension {
            name: String::new(),
            extent,
            sky_light: true,
        }
    }
}

#[derive(Clone)]
pub struct Section {
    pub blocks: Box<[BlockStateId; SectionPos::VOLUME]>,
    pub biomes: PalettedContainer<u8, { Biomes::SIZE }>,
    /// Every state the blocks hold. A block change adds its state and removes none, so a state
    /// can stay here after the last block of it is gone.
    pub states: Vec<BlockStateId>,
}

impl Section {
    /// The biomes of a section the server sent empty are lost, so the first biome stands in.
    fn air() -> Section {
        Section {
            blocks: Box::new([AIR; SectionPos::VOLUME]),
            biomes: PalettedContainer::Homogeneous(0),
            states: vec![AIR],
        }
    }
}

#[derive(Clone)]
pub struct Column {
    min_section_y: i32,
    sections: Vec<Option<Section>>,
    /// One row per section plus the padding row the light packet carries at
    /// each end, so row `i` holds section `min_section_y + i - 1`.
    light: Vec<Option<Box<[u8; SectionPos::VOLUME]>>>,
    signs: Vec<SignEntity>,
}

/// The block entity of a sign or a hanging sign: where it is and the text of its two sides.
#[derive(Clone, Debug, PartialEq)]
pub struct SignEntity {
    pub pos: BlockPos,
    pub texts: SignTexts,
}

/// A column the server sent or took back, in the order it happened. A column sent again
/// departs and arrives in that order, and one dropped by a change of extent departs too.
/// The store stays the truth: a reader that keeps state decides from what it finds there.
#[derive(Message, Clone)]
pub enum ColumnChange {
    Arrived(ColumnPos, Arc<Column>),
    Departed(ColumnPos, Arc<Column>),
    /// The section rows a light update rewrote, by section y.
    Relit(ColumnPos, Vec<i32>),
    /// A section the server sent empty now holds a block. It comes before the `Reblocked` of
    /// that block.
    Filled(SectionPos),
    /// The block at the position now holds the state.
    Reblocked(BlockPos, BlockStateId),
    /// The sign entity at the position was set or dropped.
    Signed(BlockPos),
}

/// The columns the server has sent and not taken back, keyed by position, and the changes
/// nobody has drained yet.
#[derive(Resource, Default, Clone)]
pub struct ColumnStore {
    dimension: Option<Dimension>,
    columns: HashMap<ColumnPos, Arc<Column>>,
    changes: Vec<ColumnChange>,
    light_checks: Vec<LightCheck>,
    unlit_light: UnlitColumnLight,
    /// The resident columns that came in without the light `unlit_light` computes, for the
    /// light step to take. Empty while the option is off.
    unlit_arrivals: HashSet<ColumnPos>,
}

/// What the client lights in a column that arrives without light data, from its blocks. For a
/// program that embeds the client as a viewer and has no server to send light. The networked
/// client leaves it off (CBK-LGT-013). Set it on `ColumnCachePlugin`.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnlitColumnLight {
    /// No local light for a new column: the light is the server's.
    #[default]
    Off,
    /// Block light for a column with no block light. The sky layer stays as the program set it.
    Block,
    /// Block light and sky light for a column with no light of either layer.
    BlockAndSky,
}

impl ColumnStore {
    /// A column placed against one extent cannot be read against another, and a column of one
    /// dimension is not a column of another, so entering any other dimension drops what is
    /// resident.
    pub fn enter(&mut self, dimension: impl Into<Dimension>) {
        let dimension = dimension.into();
        if self.dimension.as_ref() != Some(&dimension) {
            for (pos, column) in self.columns.drain() {
                self.changes.push(ColumnChange::Departed(pos, column));
            }
            self.dimension = Some(dimension);
            self.light_checks.clear();
            self.unlit_arrivals.clear();
        }
    }

    pub fn clear(&mut self) {
        for (pos, column) in self.columns.drain() {
            self.changes.push(ColumnChange::Departed(pos, column));
        }
        self.dimension = None;
        self.light_checks.clear();
        self.unlit_arrivals.clear();
    }

    pub fn dimension(&self) -> Option<&Dimension> {
        self.dimension.as_ref()
    }

    fn holds_nothing(&self) -> bool {
        self.columns.is_empty() && self.dimension.is_none()
    }

    pub fn set_unlit_light(&mut self, mode: UnlitColumnLight) {
        self.unlit_light = mode;
        if mode == UnlitColumnLight::Off {
            self.unlit_arrivals.clear();
        }
    }

    pub fn unlit_light(&self) -> UnlitColumnLight {
        self.unlit_light
    }

    pub fn insert(&mut self, pos: ColumnPos, column: Column) {
        // A column that replaces another is judged on its own light, and the one it replaces
        // no longer waits.
        self.unlit_arrivals.remove(&pos);
        if column.arrives_unlit(self.unlit_light) {
            self.unlit_arrivals.insert(pos);
        }
        let column = Arc::new(column);
        if let Some(old) = self.columns.insert(pos, column.clone()) {
            self.changes.push(ColumnChange::Departed(pos, old));
        }
        self.changes.push(ColumnChange::Arrived(pos, column));
    }

    /// A light update for a column nobody holds is not an error: the client may
    /// have forgotten the column between the server's decision and the packet.
    pub fn relight(&mut self, pos: ColumnPos, update: &ColumnLight) {
        if let Some(column) = self.columns.get_mut(&pos) {
            let rows = Arc::make_mut(column).relight(update);
            if !rows.is_empty() {
                self.changes.push(ColumnChange::Relit(pos, rows));
            }
        }
    }

    pub fn remove(&mut self, pos: ColumnPos) {
        self.unlit_arrivals.remove(&pos);
        if let Some(old) = self.columns.remove(&pos) {
            self.changes.push(ColumnChange::Departed(pos, old));
        }
    }

    /// Writes one block of a resident column. Answers the state it replaced, or `None` when the
    /// column is not resident, the position is outside the extent, or the block already holds
    /// `state`.
    pub fn set_block(&mut self, at: BlockPos, state: BlockStateId) -> Option<BlockStateId> {
        let section_pos = SectionPos::from(at);
        let column_pos = ColumnPos::from(at);
        let cell = LocalPos::from(at).index();
        let column = self.columns.get_mut(&column_pos)?;
        let index = usize::try_from(section_pos.y - column.min_section_y).ok()?;
        let old = match column.sections.get(index)? {
            Some(section) => *section.blocks.get(cell)?,
            None => AIR,
        };
        if old == state {
            return None;
        }
        let slot = Arc::make_mut(column).sections.get_mut(index)?;
        if slot.is_none() {
            self.changes.push(ColumnChange::Filled(section_pos));
        }
        let section = slot.get_or_insert_with(Section::air);
        *section.blocks.get_mut(cell)? = state;
        if !section.states.contains(&state) {
            section.states.push(state);
        }
        self.changes.push(ColumnChange::Reblocked(at, state));
        self.light_checks.push(LightCheck {
            pos: at,
            old,
            new: state,
        });
        Some(old)
    }

    pub(crate) fn has_unlit_arrivals(&self) -> bool {
        !self.unlit_arrivals.is_empty()
    }

    /// Whether `pos` is a resident column that arrived without light and has not been taken.
    pub(crate) fn is_unlit_arrival(&self, pos: ColumnPos) -> bool {
        self.unlit_arrivals.contains(&pos)
    }

    /// Moves the columns that arrived without light since the last call into `into`.
    pub(crate) fn take_unlit_arrivals(&mut self, into: &mut Vec<ColumnPos>) {
        into.extend(self.unlit_arrivals.drain());
    }

    /// Moves those of `named` that arrived without light and have not been taken into `into`.
    pub(crate) fn take_unlit_among(
        &mut self,
        named: impl IntoIterator<Item = ColumnPos>,
        into: &mut Vec<ColumnPos>,
    ) {
        into.extend(
            named
                .into_iter()
                .filter(|pos| self.unlit_arrivals.remove(pos)),
        );
    }

    /// A side `update` leaves out keeps its stored text. `set_block` does not drop the sign: the
    /// caller uses [`ColumnStore::remove_sign`] when the new block is no sign.
    pub fn set_sign(&mut self, at: BlockPos, update: SignUpdate) -> bool {
        let Some(column) = self.columns.get_mut(&ColumnPos::from(at)) else {
            return false;
        };
        let index = usize::try_from(SectionPos::from(at).y - column.min_section_y);
        if !index.is_ok_and(|index| index < column.sections.len()) {
            return false;
        }
        let column = Arc::make_mut(column);
        match column.signs.iter_mut().find(|sign| sign.pos == at) {
            Some(sign) => sign.texts = update.applied_to(std::mem::take(&mut sign.texts)),
            None => column.signs.push(SignEntity {
                pos: at,
                texts: update.applied_to(SignTexts::default()),
            }),
        }
        self.changes.push(ColumnChange::Signed(at));
        true
    }

    /// Drops the sign at `at`. Answers whether there was one.
    pub fn remove_sign(&mut self, at: BlockPos) -> bool {
        let Some(column) = self.columns.get_mut(&ColumnPos::from(at)) else {
            return false;
        };
        if !column.signs.iter().any(|sign| sign.pos == at) {
            return false;
        }
        Arc::make_mut(column).signs.retain(|sign| sign.pos != at);
        self.changes.push(ColumnChange::Signed(at));
        true
    }

    pub fn has_sign(&self, at: BlockPos) -> bool {
        self.columns
            .get(&ColumnPos::from(at))
            .is_some_and(|column| column.signs.iter().any(|sign| sign.pos == at))
    }

    pub(crate) fn has_light_checks(&self) -> bool {
        !self.light_checks.is_empty()
    }

    /// The block changes since the last call, for the light step to check.
    pub(crate) fn take_light_checks(&mut self) -> Vec<LightCheck> {
        std::mem::take(&mut self.light_checks)
    }

    /// The light of a section of a resident column. A section the server sent no light for is
    /// dark in both layers.
    pub(crate) fn section_light(&self, at: SectionPos) -> Option<SectionLight> {
        let column = self.columns.get(&ColumnPos::from(at))?;
        let row = column.light.get(column.light_row(at.y)?)?;
        let Some(cells) = row else {
            return Some(SectionLight::default());
        };
        let mut layer = Box::new([0u8; SectionPos::VOLUME]);
        for (out, cell) in layer.iter_mut().zip(cells.iter()) {
            *out = cell & 0x0f;
        }
        let sky = LightStorage::from_field(&layer);
        for (out, cell) in layer.iter_mut().zip(cells.iter()) {
            *out = cell >> 4;
        }
        Some(SectionLight {
            sky,
            block: LightStorage::from_field(&layer),
        })
    }

    /// Replaces the layers of a section of a resident column that `light` holds.
    pub(crate) fn set_section_light(&mut self, at: SectionPos, light: SectionLightChange) {
        let pos = ColumnPos::from(at);
        let Some(column) = self.columns.get_mut(&pos) else {
            return;
        };
        let Some(index) = column.light_row(at.y) else {
            return;
        };
        let Some(row) = Arc::make_mut(column).light.get_mut(index) else {
            return;
        };
        let cells = row.get_or_insert_with(|| Box::new([0u8; SectionPos::VOLUME]));
        let mut layer = Box::new([0u8; SectionPos::VOLUME]);
        for (storage, shift) in [(light.sky, 0), (light.block, 4)] {
            let Some(storage) = storage else { continue };
            storage.write_field(&mut layer);
            let keep = !(0x0f << shift);
            for (cell, value) in cells.iter_mut().zip(layer.iter()) {
                *cell = (*cell & keep) | (value << shift);
            }
        }
        self.changes.push(ColumnChange::Relit(pos, vec![at.y]));
    }

    /// Moves every change since the last drain to the end of `into`.
    pub fn drain_changes(&mut self, into: &mut Vec<ColumnChange>) {
        into.append(&mut self.changes);
    }

    /// The column at `origin` and the eight around it, held alive for a task that reads
    /// across their edges.
    pub fn around(&self, origin: ColumnPos) -> Neighbourhood {
        Neighbourhood {
            extent: self.extent(),
            origin,
            columns: std::array::from_fn(|index| {
                let (dx, dz) = (index as i32 % 3 - 1, index as i32 / 3 - 1);
                self.columns
                    .get(&ColumnPos::new(origin.x + dx, origin.z + dz))
                    .cloned()
            }),
        }
    }

    pub fn get(&self, pos: ColumnPos) -> Option<&Arc<Column>> {
        self.columns.get(&pos)
    }

    pub fn holds(&self, pos: ColumnPos) -> bool {
        self.columns.contains_key(&pos)
    }

    /// The resident columns with the handle each one is held by. A light update
    /// rewrites a handle nobody else holds in place, so a reader telling a column
    /// it has already looked at from one it has not keeps a `Weak` to the handle:
    /// that moves the rewrite onto a new one.
    pub fn resident(&self) -> impl Iterator<Item = (ColumnPos, &Arc<Column>)> + '_ {
        self.columns.iter().map(|(pos, column)| (*pos, column))
    }

    pub fn len(&self) -> usize {
        self.columns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }
}

impl Columns for ColumnStore {
    fn extent(&self) -> Option<Extent> {
        self.dimension.as_ref().map(|dimension| dimension.extent)
    }

    #[inline]
    fn column(&self, sx: i32, sz: i32) -> Option<&Column> {
        self.columns.get(&ColumnPos::new(sx, sz)).map(Arc::as_ref)
    }
}

impl TickingArea for ColumnStore {
    fn holds(&self, column: ColumnPos) -> bool {
        ColumnStore::holds(self, column)
    }
}

pub struct Neighbourhood {
    extent: Option<Extent>,
    origin: ColumnPos,
    columns: [Option<Arc<Column>>; 9],
}

impl Columns for Neighbourhood {
    fn extent(&self) -> Option<Extent> {
        self.extent
    }

    #[inline]
    fn column(&self, sx: i32, sz: i32) -> Option<&Column> {
        let (dx, dz) = (sx - self.origin.x + 1, sz - self.origin.z + 1);
        if !(0..3).contains(&dx) || !(0..3).contains(&dz) {
            return None;
        }
        self.columns.get((dz * 3 + dx) as usize)?.as_deref()
    }
}

/// The columns a reader holds, by section column, within the dimension's extent.
pub trait Columns {
    fn extent(&self) -> Option<Extent>;

    fn column(&self, sx: i32, sz: i32) -> Option<&Column>;

    /// The section's stored blocks; `None` for one the server sent empty or never sent.
    #[inline]
    fn stored_section(&self, at: SectionPos) -> Option<&Section> {
        self.column(at.x, at.z)?.section(at.y)
    }
}

fn column_bounds(extent: Option<Extent>) -> BoundingBox {
    let (min_y, max_y) = extent.map_or((0, -1), |extent| {
        let min_y = extent.min_section_y * SectionPos::SIZE as i32;
        (
            min_y,
            min_y + (extent.sections * SectionPos::SIZE) as i32 - 1,
        )
    });
    BoundingBox {
        min: BlockPos::new(i32::MIN, min_y, i32::MIN),
        max: BlockPos::new(i32::MAX, max_y, i32::MAX),
    }
}

/// Every column the reader holds is loaded whole: a section the server sent empty is air,
/// and only a missing column is unloaded. Light reaches one row past the extent at each
/// end, and past that it is open sky.
macro_rules! column_volume {
    ($store:ty) => {
        impl Volume for $store {
            fn bounds(&self) -> BoundingBox {
                column_bounds(Columns::extent(self))
            }

            fn access(&self, at: SectionPos) -> Access {
                let inside = Columns::extent(self).is_some_and(|extent| {
                    (extent.min_section_y..extent.min_section_y + extent.sections as i32)
                        .contains(&at.y)
                });
                if !inside {
                    Access::Outside
                } else if Columns::column(self, at.x, at.z).is_some() {
                    Access::Read
                } else {
                    Access::Unloaded
                }
            }
        }

        impl BlockVolume for $store {
            type Section<'a> = DenseSection<'a>;

            #[inline]
            fn section(&self, at: SectionPos) -> Result<DenseSection<'_>, Unavailable> {
                self.access(at).check_read()?;
                Ok(self
                    .stored_section(at)
                    .map_or(DenseSection::air(), |section| DenseSection(&section.blocks)))
            }
        }

        impl SectionVolume for $store {}

        impl LightVolume for $store {
            #[inline]
            fn light(&self, at: BlockPos) -> Result<u8, Unavailable> {
                let section = SectionPos::from(at);
                let column =
                    Columns::column(self, section.x, section.z).ok_or(Unavailable::Unloaded)?;
                Ok(column.light(section.y, LocalPos::from(at).index()))
            }
        }

        impl BiomeVolume for $store {
            fn biome_at(&self, at: BlockPos) -> Result<u16, Unavailable> {
                let section = SectionPos::from(at);
                self.access(section).check_read()?;
                let cell = LocalPos::from(at);
                Ok(self
                    .stored_section(section)
                    .map_or(0, |stored| u16::from(stored.biomes.get_local(cell))))
            }
        }
    };
}

column_volume!(ColumnStore);
column_volume!(Neighbourhood);

impl Column {
    /// A column nobody has lit: every block in it reads as dark.
    pub fn unlit(min_section_y: i32, sections: Vec<Option<Section>>) -> Column {
        let light = (0..sections.len() + 2).map(|_| None).collect();
        Column {
            min_section_y,
            sections,
            light,
            signs: Vec::new(),
        }
    }

    /// Whether the column holds none of the light that `mode` computes: no light row at all for
    /// both layers, no lit block cell for block light alone.
    fn arrives_unlit(&self, mode: UnlitColumnLight) -> bool {
        match mode {
            UnlitColumnLight::Off => false,
            UnlitColumnLight::Block => !self
                .light
                .iter()
                .flatten()
                .any(|row| row.iter().any(|cell| cell >> 4 != 0)),
            UnlitColumnLight::BlockAndSky => self.light.iter().all(Option::is_none),
        }
    }

    /// The column with these sign entities, for an embedder that builds columns without packets.
    /// The positions are block positions of the world, not of the column.
    #[must_use]
    pub fn with_signs(mut self, signs: Vec<SignEntity>) -> Column {
        self.signs = signs;
        self
    }

    /// The signs of a chunk packet's block entities, as [`Column::with_signs`] holds them. An
    /// entry that is no sign, or whose compound does not read, is skipped.
    #[must_use]
    pub fn with_chunk_signs(self, at: ColumnPos, entries: &[ChunkDataBlockEntity<'_>]) -> Column {
        let origin = IVec3::new(at.x, 0, at.z) * SectionPos::SIZE as i32;
        let signs = entries
            .iter()
            .filter(|entry| {
                matches!(
                    entry.kind,
                    BlockEntityType::Sign | BlockEntityType::HangingSign
                )
            })
            .filter_map(|entry| {
                let packed = entry.packed_xz as u8;
                let pos = BlockPos::new(
                    origin.x + i32::from(packed >> 4),
                    i32::from(entry.y),
                    origin.z + i32::from(packed & 15),
                );
                let texts = SignTexts::from_compound(&entry.data).ok()?;
                Some(SignEntity { pos, texts })
            })
            .collect();
        self.with_signs(signs)
    }

    /// The sign entities of the column.
    pub fn signs(&self) -> &[SignEntity] {
        &self.signs
    }

    pub fn decode(
        data: &ChunkData<'_>,
        light: &LightData<'_>,
        extent: Extent,
        block_state_count: usize,
        biome_count: usize,
    ) -> Result<Column> {
        let sections = data
            .sections(extent.sections, block_state_count, biome_count)?
            .into_iter()
            .map(|section| {
                if section.non_empty_block_count == 0 {
                    return Ok(None);
                }
                // A server whose block corpus differs from ours can name a state past our
                // table; it reads as air rather than taking the frame down.
                let state = |id: mcrs_minecraft_chunk::VoxelId| {
                    BlockStateId::checked(id.0, block_state_count).unwrap_or(AIR)
                };
                let mut blocks = Box::new([AIR; SectionPos::VOLUME]);
                expand(&section.blocks, blocks.as_mut_slice(), state);
                let states = match &section.blocks {
                    PalettedContainer::Homogeneous(id) => vec![state(*id)],
                    PalettedContainer::Heterogeneous(data) => {
                        data.palette.iter().map(|id| state(*id)).collect()
                    }
                };
                Ok(Some(Section {
                    blocks,
                    biomes: biome_ids(&section.biomes, biome_count)?,
                    states,
                }))
            })
            .collect::<Result<_>>()?;

        let mut column = Column::unlit(extent.min_section_y, sections);
        column.relight(&unpack_light_data(light, extent.sections + 2)?);
        Ok(column)
    }

    /// Rows the update leaves unchanged keep the nibbles they have, per layer.
    /// Returns the section y of every row it rewrote.
    fn relight(&mut self, update: &ColumnLight) -> Vec<i32> {
        let min_section_y = self.min_section_y;
        let mut touched = Vec::new();
        let rows = update.sky.iter().zip(&update.block);
        for (index, (row, (sky, block))) in self.light.iter_mut().zip(rows).enumerate() {
            let out = match row {
                Some(out) => out,
                None => {
                    if !matches!(sky, RowLight::Filled(_)) && !matches!(block, RowLight::Filled(_))
                    {
                        continue;
                    }
                    row.insert(Box::new([0u8; SectionPos::VOLUME]))
                }
            };
            if matches!(sky, RowLight::Unchanged) && matches!(block, RowLight::Unchanged) {
                continue;
            }
            apply_layer(sky, out, 0);
            apply_layer(block, out, 4);
            touched.push(min_section_y + index as i32 - 1);
        }
        touched
    }

    /// The index of the light row of section `sy`, within the rows or past them.
    fn light_row(&self, sy: i32) -> Option<usize> {
        usize::try_from(sy - self.min_section_y + 1).ok()
    }

    #[inline]
    pub fn section(&self, sy: i32) -> Option<&Section> {
        let index = usize::try_from(sy - self.min_section_y).ok()?;
        self.sections.get(index)?.as_ref()
    }

    /// Every section slot the column spans, by section y.
    pub fn sections(&self) -> impl Iterator<Item = (i32, Option<&Section>)> {
        self.sections
            .iter()
            .enumerate()
            .map(|(index, section)| (self.min_section_y + index as i32, section.as_ref()))
    }

    #[inline]
    fn light(&self, sy: i32, cell: usize) -> u8 {
        let row = sy - self.min_section_y + 1;
        if row < 0 {
            return 0;
        }
        match self.light.get(row as usize) {
            Some(Some(light)) => light.get(cell).copied().unwrap_or(0),
            Some(None) => 0,
            None => OPEN_SKY,
        }
    }
}

fn expand<V: Hash + Eq + Copy + Default, const DIM: usize, T>(
    container: &PalettedContainer<V, DIM>,
    out: &mut [T],
    id: impl Fn(V) -> T,
) {
    let mut cells = out.iter_mut();
    container.for_each(|value| {
        if let Some(cell) = cells.next() {
            *cell = id(value);
        }
    });
}

fn apply_layer(row: &RowLight, out: &mut [u8; SectionPos::VOLUME], shift: u32) {
    match row {
        RowLight::Unchanged => {}
        RowLight::Empty => {
            let keep = !(0x0f << shift);
            for byte in out.iter_mut() {
                *byte &= keep;
            }
        }
        RowLight::Filled(chunk) => write_nibbles(chunk, out, shift),
    }
}

fn write_nibbles(source: &LightChunk, out: &mut [u8; SectionPos::VOLUME], shift: u32) {
    let keep = !(0x0f << shift);
    let (pairs, _) = out.as_chunks_mut::<2>();
    for ([low, high], byte) in pairs.iter_mut().zip(source.as_bytes()) {
        *low = (*low & keep) | ((byte & 0x0f) << shift);
        *high = (*high & keep) | ((byte >> 4) << shift);
    }
}

fn biome_ids(
    sent: &PalettedContainer<u8, { Biomes::SIZE }>,
    biome_count: usize,
) -> Result<PalettedContainer<u8, { Biomes::SIZE }>> {
    let local = |number: u8| -> Result<u8> {
        if usize::from(number) < biome_count {
            Ok(number)
        } else {
            Err(anyhow!(
                "biome {number} is not one the session registries hold"
            ))
        }
    };
    match sent {
        PalettedContainer::Homogeneous(number) => {
            Ok(PalettedContainer::Homogeneous(local(*number)?))
        }
        PalettedContainer::Heterogeneous(_) => {
            let mut cube = Box::new([[[0u8; Biomes::SIZE]; Biomes::SIZE]; Biomes::SIZE]);
            let mut slots = cube.as_flattened_mut().as_flattened_mut().iter_mut();
            let mut refused = Ok(());
            sent.for_each(|number| match (slots.next(), local(number)) {
                (Some(slot), Ok(id)) => *slot = id,
                (None, Ok(_)) => {}
                (_, Err(error)) => refused = Err(error),
            });
            refused?;
            Ok(PalettedContainer::from_cube(cube))
        }
    }
}

fn dimension_of(registries: &Registries, current: &CurrentDimension) -> Option<Dimension> {
    let dimension_types = registries.registry::<DimensionTypeFile>()?;
    let dimension_type = dimension_types.get(Id::checked(
        current.dimension_type.number(),
        dimension_types.len(),
    )?)?;
    Some(Dimension {
        name: current.key.to_string(),
        extent: Extent {
            min_section_y: dimension_type.min_y >> SectionPos::BITS,
            sections: usize::try_from(dimension_type.height).ok()? / SectionPos::SIZE,
        },
        sky_light: dimension_type.has_skylight,
    })
}

/// Keeps the columns the server sends. `unlit_light` is off by default, so the networked client
/// takes its light from the server alone.
#[derive(Default)]
pub struct ColumnCachePlugin {
    pub unlit_light: UnlitColumnLight,
}

/// Columns reach the store after the frame's packets are read, and whatever builds on the
/// store runs after them, so a column is taken up in the frame it arrived.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClientTerrainSet {
    Settle,
    Build,
}

impl Plugin for ColumnCachePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ColumnStore>();
        app.world_mut()
            .resource_mut::<ColumnStore>()
            .set_unlit_light(self.unlit_light);
        app.init_resource::<Arrivals>();
        app.init_resource::<BlockPredictions>();
        app.add_message::<ColumnChange>();
        app.add_observer(on_login);
        app.add_observer(on_respawn);
        app.add_observer(on_level_chunk);
        app.add_observer(on_light_update);
        app.add_observer(on_block_update);
        app.add_observer(on_block_entity_data);
        app.add_observer(on_section_blocks_update);
        app.add_observer(on_block_changed_ack);
        app.add_observer(on_forget);
        app.add_observer(on_batch_start);
        app.add_observer(on_batch_finished);
        // A column is large; its observer decodes it off the main thread.
        app.world_mut()
            .get_resource_or_init::<DeferredRows>()
            .defer::<ClientboundLevelChunkWithLight<'static>>();
        app.configure_sets(
            Update,
            (
                ClientTerrainSet::Settle
                    .after(ClientNetworkSystems::Receive)
                    .before(ClientNetworkSystems::Flush),
                ClientTerrainSet::Build.after(ClientTerrainSet::Settle),
            ),
        );
        app.add_systems(
            Update,
            (
                clear_columns
                    .in_set(SessionSet::Clear)
                    .before(settle_columns)
                    .run_if(entered_configuration),
                settle_columns.in_set(ClientTerrainSet::Settle),
            ),
        );
    }
}

fn clear_columns(
    mut store: ResMut<ColumnStore>,
    mut arrivals: ResMut<Arrivals>,
    mut predictions: ResMut<BlockPredictions>,
) {
    if !store.holds_nothing() {
        store.clear();
    }
    predictions.clear();
    if !arrivals.holds_nothing() {
        arrivals.clear();
    }
}

/// Column packets in the order they came, each one decoding on the compute pool. A column's
/// forget can only be applied once the column itself has landed, so the queue is drained from
/// the front and stops at the first decode still running.
#[derive(Resource)]
pub struct Arrivals {
    queue: VecDeque<Arrival>,
    extent: Option<Extent>,
    batch_started_at: Option<Instant>,
    nanos_per_column: f64,
    old_samples_weight: u32,
}

impl Default for Arrivals {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            extent: None,
            batch_started_at: None,
            nanos_per_column: START_NANOS_PER_COLUMN,
            old_samples_weight: 1,
        }
    }
}

enum Arrival {
    Enter(Dimension),
    /// `None` for a frame that decodes but has no dimension or biome registry to land in.
    Column(Task<Result<Option<(ColumnPos, Column)>>>),
    Light(ColumnPos, ColumnLight),
    Blocks(Vec<BlockChange>),
    Sign(BlockPos, SignUpdate),
    Acknowledge(i32),
    Forget(ColumnPos),
    BatchStart,
    BatchEnd(u32),
}

/// A block change that the light step checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LightCheck {
    pub(crate) pos: BlockPos,
    pub(crate) old: BlockStateId,
    pub(crate) new: BlockStateId,
}

/// The two light layers of one section.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SectionLight {
    pub(crate) sky: LightStorage,
    pub(crate) block: LightStorage,
}

/// The layers of one section that a light calculation changed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SectionLightChange<'a> {
    pub(crate) sky: Option<&'a LightStorage>,
    pub(crate) block: Option<&'a LightStorage>,
}

/// A block state the server sent for one position.
struct BlockChange {
    pos: BlockPos,
    state: BlockStateId,
}

/// The rate answered to the server is measured over a batch: not from the packets landing, but
/// from its columns finishing their decode, so a client that has fallen behind asks for less.
const START_NANOS_PER_COLUMN: f64 = 2_000_000.0;
const MAX_OLD_SAMPLES_WEIGHT: u32 = 49;
const CLAMP_COEFFICIENT: f64 = 3.0;

/// The share of a tick a client is willing to spend taking columns. Vanilla keeps 7 ms of the
/// tick for chunks because it decodes them on the thread it renders from; ours decode on the
/// compute pool, so the budget is most of the tick and the ceiling is what binds while the
/// pipeline keeps up.
const NANOS_PER_TICK_ON_COLUMNS: f64 = 35_000_000.0;

impl Arrivals {
    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// Drops what the connection sent for the dimension it is leaving. The decodes still
    /// running are cancelled with their tasks. The measured decode rate stays: it describes the
    /// machine, not the session.
    fn clear(&mut self) {
        self.queue.clear();
        self.extent = None;
        self.batch_started_at = None;
    }

    fn holds_nothing(&self) -> bool {
        self.queue.is_empty() && self.extent.is_none() && self.batch_started_at.is_none()
    }

    fn enter(&mut self, registries: &Registries, spawn: &PlayerSpawnInfo) {
        let current = CurrentDimension::of(spawn);
        match dimension_of(registries, &current) {
            Some(dimension) => {
                self.extent = Some(dimension.extent);
                self.queue.push_back(Arrival::Enter(dimension));
            }
            None => error!(
                "dimension type {} has no extent: columns have nowhere to sit",
                spawn.dimension_type_id.number()
            ),
        }
    }

    /// Folds a finished batch into the running average and answers with the columns a tick it
    /// implies.
    fn rate_after_batch(&mut self, batch_size: u32) -> f32 {
        if let (Some(started), true) = (self.batch_started_at.take(), batch_size > 0) {
            let measured = started.elapsed().as_nanos() as f64 / batch_size as f64;
            let clamped = measured.clamp(
                self.nanos_per_column / CLAMP_COEFFICIENT,
                self.nanos_per_column * CLAMP_COEFFICIENT,
            );
            let weight = self.old_samples_weight as f64;
            self.nanos_per_column = (self.nanos_per_column * weight + clamped) / (weight + 1.0);
            self.old_samples_weight = (self.old_samples_weight + 1).min(MAX_OLD_SAMPLES_WEIGHT);
        }
        (NANOS_PER_TICK_ON_COLUMNS / self.nanos_per_column) as f32
    }
}

fn on_login(
    event: On<Received<ClientboundLogin>>,
    registries: Option<Res<CurrentRegistries>>,
    mut arrivals: ResMut<Arrivals>,
) {
    if let (Some(login), Some(registries)) = (event.packet(), registries.as_deref()) {
        arrivals.enter(registries, &login.player_spawn_info);
    }
}

fn on_respawn(
    event: On<Received<ClientboundRespawn>>,
    registries: Option<Res<CurrentRegistries>>,
    mut arrivals: ResMut<Arrivals>,
) {
    if let (Some(respawn), Some(registries)) = (event.packet(), registries.as_deref()) {
        arrivals.enter(registries, &respawn.player_spawn_info);
    }
}

/// The column decodes off the main thread, from the frame the event holds. The dispatcher
/// defers the check of this row, so a frame that does not decode drops the connection here.
fn on_level_chunk(
    event: On<Received<ClientboundLevelChunkWithLight<'static>>>,
    blocks: Res<Blocks>,
    registries: Option<Res<CurrentRegistries>>,
    mut arrivals: ResMut<Arrivals>,
) {
    let extent = arrivals.extent;
    let biome_count = registries
        .as_deref()
        .and_then(|registries| registries.registry::<BiomeFile>())
        .map(|biomes| biomes.len());
    let data = event.data().clone();
    let block_state_count = blocks.state_count();
    let task = AsyncComputeTaskPool::get().spawn(async move {
        let mut bytes = &data[..];
        let packet = ClientboundLevelChunkWithLight::decode(&mut bytes)
            .map_err(|error| anyhow!("chunk packet: {error:?}"))?;
        anyhow::ensure!(
            bytes.is_empty(),
            "chunk packet: {} bytes left over",
            bytes.len()
        );
        let Some(extent) = extent else {
            return Ok(None);
        };
        let Some(biome_count) = biome_count else {
            error!("a column arrived before the server's biome registry was built");
            return Ok(None);
        };
        let column = Column::decode(
            &packet.chunk_data,
            &packet.light_data,
            extent,
            block_state_count,
            biome_count,
        )
        .with_context(|| format!("column {:?}", packet.pos))?;
        let column = column.with_chunk_signs(packet.pos, &packet.chunk_data.block_entities);
        Ok(Some((packet.pos, column)))
    });
    arrivals.queue.push_back(Arrival::Column(task));
}

fn on_light_update(
    event: On<Received<ClientboundLightUpdate<'static>>>,
    mut arrivals: ResMut<Arrivals>,
) {
    let (Some(update), Some(extent)) = (event.packet(), arrivals.extent) else {
        return;
    };
    let pos = ColumnPos::new(update.x.0, update.z.0);
    match unpack_light_data(&update.light_data, extent.sections + 2) {
        Ok(light) => arrivals.queue.push_back(Arrival::Light(pos, light)),
        Err(error) => error!("light update for column {pos:?}: {error:#}"),
    }
}

/// A block update waits in the queue behind the column it changes, which may still decode.
fn on_block_update(
    event: On<Received<ClientboundBlockUpdate>>,
    blocks: Res<Blocks>,
    mut arrivals: ResMut<Arrivals>,
) {
    if let Some(update) = event.packet() {
        let change = BlockChange {
            pos: update.block_pos,
            state: sent_state(update.block_state_id.0, &blocks),
        };
        arrivals.queue.push_back(Arrival::Blocks(vec![change]));
    }
}

/// A sign entity update waits in the queue behind the column it lands in, as a block update does.
fn on_block_entity_data(
    event: On<Received<ClientboundBlockEntityData>>,
    mut arrivals: ResMut<Arrivals>,
) {
    let Some(update) = event.packet() else {
        return;
    };
    let Some((kind, data)) = &update.data else {
        warn!(
            "block entity update at {:?} that this client cannot read",
            update.pos
        );
        return;
    };
    if !matches!(kind, BlockEntityType::Sign | BlockEntityType::HangingSign) {
        return;
    }
    match SignUpdate::from_compound(data) {
        Ok(texts) => arrivals.queue.push_back(Arrival::Sign(update.pos, texts)),
        Err(error) => error!("sign update at {:?}: {error}", update.pos),
    }
}

/// A block that stops being a sign takes its entity with it, as the server removes it.
fn drop_replaced_sign(
    store: &mut ColumnStore,
    blocks: Option<&Blocks>,
    at: BlockPos,
    state: BlockStateId,
) {
    if store.has_sign(at) && !blocks.is_some_and(|blocks| sign_block(blocks, state).is_some()) {
        store.remove_sign(at);
    }
}

fn on_section_blocks_update(
    event: On<Received<ClientboundSectionBlocksUpdate<'static>>>,
    blocks: Res<Blocks>,
    mut arrivals: ResMut<Arrivals>,
) {
    let Some(update) = event.packet() else {
        return;
    };
    let origin = update.chunk_pos.0 * SectionPos::SIZE as i32;
    let changes = update
        .blocks
        .iter()
        .map(|entry| BlockChange {
            pos: BlockPos::from(
                origin
                    + IVec3::new(
                        i32::from(entry.off_x()),
                        i32::from(entry.off_y()),
                        i32::from(entry.off_z()),
                    ),
            ),
            state: sent_state(entry.block_state(), &blocks),
        })
        .collect();
    arrivals.queue.push_back(Arrival::Blocks(changes));
}

fn on_block_changed_ack(
    event: On<Received<ClientboundBlockChangedAck>>,
    mut arrivals: ResMut<Arrivals>,
) {
    if let Some(ack) = event.packet() {
        arrivals
            .queue
            .push_back(Arrival::Acknowledge(ack.sequence.0));
    }
}

/// A state past our table reads as air, as it does in a column.
fn sent_state(number: u16, blocks: &Blocks) -> BlockStateId {
    BlockStateId::checked(number, blocks.state_count()).unwrap_or(AIR)
}

fn on_forget(event: On<Received<ClientboundForgetLevelChunk>>, mut arrivals: ResMut<Arrivals>) {
    if let Some(forget) = event.packet() {
        arrivals
            .queue
            .push_back(Arrival::Forget(ColumnPos::new(forget.x, forget.z)));
    }
}

fn on_batch_start(_: On<Received<ClientboundChunkBatchStart>>, mut arrivals: ResMut<Arrivals>) {
    arrivals.queue.push_back(Arrival::BatchStart);
}

fn on_batch_finished(
    event: On<Received<ClientboundChunkBatchFinished>>,
    mut arrivals: ResMut<Arrivals>,
) {
    if let Some(finished) = event.packet() {
        arrivals
            .queue
            .push_back(Arrival::BatchEnd(finished.batch_size.0.max(0) as u32));
    }
}

/// The light of one `light_update`, held back until the local light of the frame is done.
struct ServerLight {
    pos: ColumnPos,
    update: ColumnLight,
}

/// The local light of each block change comes before the server light of the same frame, so
/// the server light wins over it. A column that arrives or leaves settles both first, so each
/// lands on the column it was meant for.
///
/// The columns that arrived without light wait for the end of the frame, so that all of them
/// are lit in one step. Only a column that the server light of the frame names is lit sooner,
/// so that the server light still wins over the local one (CBK-LGT-014).
struct LightStep<'a> {
    blocks: Option<&'a Blocks>,
    server: Vec<ServerLight>,
    /// The columns taken for the step, kept between frames so a step allocates nothing.
    due: &'a mut Vec<ColumnPos>,
}

/// When a `LightStep::settle` runs, which decides what it lights of the columns that arrived
/// without light.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Settling {
    /// Before a column or a dimension change: only the columns the server light names.
    NamedByServer,
    /// The end of the frame: every column that is still waiting.
    FrameEnd,
}

impl LightStep<'_> {
    fn settle(&mut self, store: &mut ResMut<ColumnStore>, when: Settling) {
        if store.has_light_checks() {
            let store = store.as_mut();
            let checks = store.take_light_checks();
            if let Some(blocks) = self.blocks {
                client_light::relight(store, &block_light_registry(blocks), &checks);
            }
        }
        // The columns are taken only when the light can run, or they would stay dark for good.
        let can_light = self.blocks.is_some() && store.dimension().is_some();
        let named = when == Settling::NamedByServer;
        if can_light && store.has_unlit_arrivals() && !(named && self.server.is_empty()) {
            let store = store.as_mut();
            if named {
                store.take_unlit_among(self.server.iter().map(|server| server.pos), self.due);
            } else {
                store.take_unlit_arrivals(self.due);
            }
            if let (Some(blocks), false) = (self.blocks, self.due.is_empty()) {
                let mode = store.unlit_light();
                client_light::light_arrived(store, &block_light_registry(blocks), self.due, mode);
            }
            self.due.clear();
        }
        for server in self.server.drain(..) {
            store.relight(server.pos, &server.update);
        }
    }
}

/// Lands what has finished decoding, in packet order. The store is only borrowed mutably when
/// something actually lands, so its change tick means a column moved.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and writer is its own parameter"
)]
fn settle_columns(
    mut arrivals: ResMut<Arrivals>,
    mut store: ResMut<ColumnStore>,
    mut predictions: ResMut<BlockPredictions>,
    blocks: Option<Res<Blocks>>,
    mut changes: MessageWriter<ColumnChange>,
    mut drained: Local<Vec<ColumnChange>>,
    mut due: Local<Vec<ColumnPos>>,
    connection: Option<Single<(Entity, &mut ClientConnection)>>,
    mut commands: Commands,
) {
    let mut connection = connection.map(Single::into_inner);
    let arrivals = arrivals.bypass_change_detection();
    let mut light = LightStep {
        blocks: blocks.as_deref(),
        server: Vec::new(),
        due: &mut due,
    };
    while let Some(arrival) = arrivals.queue.pop_front() {
        match arrival {
            Arrival::Enter(dimension) => {
                light.settle(&mut store, Settling::NamedByServer);
                if store.dimension.as_ref() != Some(&dimension) {
                    predictions.clear();
                }
                store.enter(dimension);
            }
            Arrival::Forget(pos) => {
                light.settle(&mut store, Settling::NamedByServer);
                store.remove(pos);
            }
            Arrival::Light(pos, update) => light.server.push(ServerLight { pos, update }),
            Arrival::Sign(at, texts) => {
                store.set_sign(at, texts);
            }
            Arrival::Blocks(changes) => {
                for change in changes {
                    if let Some(state) = predictions.server_update(change.pos, change.state) {
                        store.set_block(change.pos, state);
                        drop_replaced_sign(&mut store, blocks.as_deref(), change.pos, state);
                    }
                }
            }
            Arrival::Acknowledge(sequence) => {
                for settled in predictions.acknowledge(sequence) {
                    store.set_block(settled.pos, settled.server_state);
                    drop_replaced_sign(
                        &mut store,
                        blocks.as_deref(),
                        settled.pos,
                        settled.server_state,
                    );
                }
            }
            Arrival::BatchStart => arrivals.batch_started_at = Some(Instant::now()),
            Arrival::BatchEnd(batch_size) => {
                let desired_chunks_per_tick = arrivals.rate_after_batch(batch_size);
                if let Some((_, connection)) = connection.as_mut() {
                    connection.write_packet(&ServerboundChunkBatchReceived {
                        desired_chunks_per_tick,
                    });
                }
            }
            Arrival::Column(mut task) => match check_ready(&mut task) {
                None => {
                    arrivals.queue.push_front(Arrival::Column(task));
                    break;
                }
                Some(Ok(Some((pos, column)))) => {
                    light.settle(&mut store, Settling::NamedByServer);
                    store.insert(pos, column);
                }
                Some(Ok(None)) => {}
                Some(Err(error)) => {
                    if let Some((entity, _)) = &connection {
                        close_connection_later(
                            &mut commands,
                            *entity,
                            format!("Internal Exception: {error:#}"),
                        );
                    }
                    arrivals.queue.clear();
                    break;
                }
            },
        }
    }
    light.settle(&mut store, Settling::FrameEnd);
    store.bypass_change_detection().drain_changes(&mut drained);
    changes.write_batch(drained.drain(..));
}

#[cfg(test)]
mod tests {
    fn state(number: u16) -> BlockStateId {
        BlockStateId::checked(number, 1 << 16).expect("every u16 names a state")
    }

    fn block(store: &impl BlockVolume, x: i32, y: i32, z: i32) -> BlockStateId {
        store.block(BlockPos::new(x, y, z)).unwrap_or(AIR)
    }

    fn light(store: &impl LightVolume, x: i32, y: i32, z: i32) -> u8 {
        store.light(BlockPos::new(x, y, z)).unwrap_or(OPEN_SKY)
    }

    fn biome(store: &impl BiomeVolume, sx: i32, sy: i32, sz: i32, cell: (i32, i32, i32)) -> u16 {
        let size = SectionPos::SIZE as i32;
        let at = BlockPos::new(sx * size + cell.0, sy * size + cell.1, sz * size + cell.2);
        store.biome_at(at).unwrap_or(0)
    }

    use super::*;
    use mcrs_minecraft_chunk::VoxelId;
    use mcrs_minecraft_protocol::{Decode, Encode, VarInt};
    use mcrs_minecraft_protocol_game::chunk::{ChunkSection, encode_section};
    use mcrs_minecraft_protocol_game::section::{biome_direct_bits, block_direct_bits};
    use mcrs_minecraft_resource::ResourceKeyBuf;
    use mcrs_minecraft_resource::ResourceLocationBuf;
    use std::borrow::Cow;

    const EXTENT: Extent = Extent {
        min_section_y: -1,
        sections: 2,
    };

    fn encoded<T: Encode>(value: &T) -> Vec<u8> {
        let mut bytes = Vec::new();
        value.encode(&mut bytes).expect("encode");
        bytes
    }

    const BLOCK_STATE_COUNT: usize = 40_000;
    const BIOME_REGISTRY_LEN: usize = 16;

    fn written(section: &ChunkSection) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode_section(
            section.non_empty_block_count,
            section.fluid_count,
            &section.blocks,
            &section.biomes,
            block_direct_bits(BLOCK_STATE_COUNT),
            biome_direct_bits(BIOME_REGISTRY_LEN),
            &mut bytes,
        )
        .expect("write section");
        bytes
    }

    fn air_section() -> ChunkSection {
        ChunkSection {
            non_empty_block_count: 0,
            fluid_count: 0,
            blocks: PalettedContainer::Homogeneous(VoxelId(0)),
            biomes: PalettedContainer::Homogeneous(3),
        }
    }

    fn section_of_states(ids: &[u32]) -> ChunkSection {
        let cells: Vec<VoxelId> = ids.iter().map(|&id| VoxelId(id as u16)).collect();
        ChunkSection {
            non_empty_block_count: ids.iter().filter(|&&id| id != 0).count() as u16,
            fluid_count: 0,
            blocks: PalettedContainer::from_cells(&cells).unwrap(),
            biomes: PalettedContainer::Homogeneous(3),
        }
    }

    fn one_lit_cell(cell: usize, level: u8) -> LightChunk {
        let mut nibbles = [0u8; 2048];
        nibbles[cell / 2] = level << ((cell % 2) * 4);
        LightChunk::new(nibbles)
    }

    #[test]
    fn a_relight_moves_a_remembered_column_to_a_new_handle() {
        let pos = ColumnPos::new(0, 0);
        let mut store = ColumnStore::default();
        store.insert(
            pos,
            Column::unlit(EXTENT.min_section_y, vec![None; EXTENT.sections]),
        );
        store.drain_changes(&mut Vec::new());
        let remembered = Arc::downgrade(store.get(pos).expect("resident"));

        let rows = EXTENT.sections + 2;
        let mut sky: Vec<RowLight> = (0..rows).map(|_| RowLight::Unchanged).collect();
        sky[1] = RowLight::Filled(one_lit_cell(0, 15));
        let block = (0..rows).map(|_| RowLight::Unchanged).collect();
        store.relight(pos, &ColumnLight { sky, block });

        assert_ne!(
            remembered.as_ptr(),
            Arc::as_ptr(store.get(pos).expect("resident"))
        );
    }

    #[test]
    fn a_decoded_packet_reads_back_where_it_put_its_blocks_and_its_light() {
        let cell = LocalPos::new(3, 5, 7).index();
        let mut states = vec![0u32; SectionPos::VOLUME];
        states[cell] = 42;
        let blob = [air_section(), section_of_states(&states)]
            .iter()
            .flat_map(written)
            .collect::<Vec<u8>>();

        let lit_row = 1u64 << 2;
        let packet = ClientboundLevelChunkWithLight {
            pos: ColumnPos::new(1, -2),
            chunk_data: ChunkData {
                data: &blob,
                ..Default::default()
            },
            light_data: LightData {
                sky_light_mask: Cow::Owned(vec![lit_row]),
                block_light_mask: Cow::Owned(vec![lit_row]),
                sky_light_arrays: Cow::Owned(vec![one_lit_cell(cell, 7)]),
                block_light_arrays: Cow::Owned(vec![one_lit_cell(cell, 2)]),
                ..Default::default()
            },
        };

        let bytes = encoded(&packet);
        let mut reader = bytes.as_slice();
        let packet = ClientboundLevelChunkWithLight::decode(&mut reader).expect("decode packet");
        assert!(reader.is_empty(), "{} bytes left unread", reader.len());

        let mut store = ColumnStore::default();
        store.enter(EXTENT);
        let column = Column::decode(
            &packet.chunk_data,
            &packet.light_data,
            EXTENT,
            BLOCK_STATE_COUNT,
            BIOME_REGISTRY_LEN,
        )
        .expect("decode the column");
        store.insert(packet.pos, column);

        let (x, z) = (
            (SectionPos::SIZE as i32) + 3,
            -2 * SectionPos::SIZE as i32 + 7,
        );
        assert_eq!(
            block(&store, x, 5, z),
            state(42),
            "the state the packet carried"
        );
        assert_eq!(block(&store, x + 1, 5, z), AIR, "its neighbour");
        assert_eq!(block(&store, x + 64, 5, z), AIR, "a column nobody sent");
        assert_eq!(light(&store, x, 5, z), 0x27, "sky 7 low, block 2 high");
        assert_eq!(
            light(&store, x, 5, z + 1),
            0x00,
            "an unlit cell of a lit section"
        );
        assert_eq!(light(&store, x, 5 + 32, z), OPEN_SKY, "above the dimension");
        assert!(
            store.stored_section(SectionPos::new(1, -1, -2)).is_none(),
            "a section of nothing but air is not resident"
        );
        assert!(store.stored_section(SectionPos::new(1, 0, -2)).is_some());
        assert_eq!(biome(&store, 1, 0, -2, (0, 0, 0)), 3);

        store.remove(packet.pos);
        assert!(store.is_empty(), "the forget packet takes the column back");
    }

    fn stone_with(biomes: PalettedContainer<u8, { Biomes::SIZE }>) -> ChunkSection {
        ChunkSection {
            non_empty_block_count: 1,
            fluid_count: 0,
            blocks: PalettedContainer::Homogeneous(VoxelId(1)),
            biomes,
        }
    }

    fn decoded(sections: &[ChunkSection], biomes: usize) -> Result<Column> {
        let blob = sections.iter().flat_map(written).collect::<Vec<u8>>();
        Column::decode(
            &ChunkData {
                data: &blob,
                ..Default::default()
            },
            &LightData::default(),
            EXTENT,
            BLOCK_STATE_COUNT,
            biomes,
        )
    }

    fn biomes_with_one_cell(base: u8, cell: (usize, usize, usize), value: u8) -> ChunkSection {
        let mut cells = vec![base; Biomes::ENTRY_COUNT];
        cells[Biomes::index(cell.0, cell.1, cell.2)] = value;
        stone_with(PalettedContainer::from_cells(&cells).unwrap())
    }

    #[test]
    fn a_decoded_column_holds_the_session_ids_of_the_biome_numbers_it_carries() {
        let biomes = BIOME_REGISTRY_LEN;

        let column = decoded(
            &[
                stone_with(PalettedContainer::Homogeneous(3)),
                biomes_with_one_cell(3, (3, 2, 1), 0),
            ],
            biomes,
        )
        .expect("decode the column");
        let pos = ColumnPos::new(1, -2);
        let mut store = ColumnStore::default();
        store.enter(EXTENT);
        store.insert(pos, column);

        assert_eq!(biome(&store, 1, -1, -2, (0, 0, 0)), 3);
        assert_eq!(biome(&store, 1, 0, -2, (0, 0, 0)), 3);
        assert_eq!(biome(&store, 1, 0, -2, (3, 2, 1)), 0);
    }

    #[test]
    fn a_column_naming_a_biome_the_session_registries_lack_is_refused() {
        let biomes = 8;

        let held = stone_with(PalettedContainer::Homogeneous(7));
        decoded(&[held.clone(), held.clone()], biomes).expect("every biome is held");

        let lacking = stone_with(PalettedContainer::Homogeneous(12));
        let Err(error) = decoded(&[held.clone(), lacking], biomes) else {
            panic!("a biome the session registries lack was accepted");
        };
        assert!(format!("{error:#}").contains("biome 12"), "{error:#}");

        let lacking_in_one_cell = biomes_with_one_cell(7, (0, 3, 2), 12);
        assert!(decoded(&[held, lacking_in_one_cell], biomes).is_err());
    }

    #[test]
    fn a_column_is_refused_when_the_session_has_no_biomes() {
        let held = stone_with(PalettedContainer::Homogeneous(0));
        assert!(decoded(&[held.clone(), held], 0).is_err());
    }

    /// A column with sections -1 and 0 lit at one cell: sky 6 / block 1 below,
    /// sky 7 / block 2 above.
    fn lit_store() -> (ColumnStore, ColumnPos) {
        let blob = [air_section(), air_section()]
            .iter()
            .flat_map(written)
            .collect::<Vec<u8>>();
        let rows = (1u64 << 1) | (1u64 << 2);
        let light = LightData {
            sky_light_mask: Cow::Owned(vec![rows]),
            block_light_mask: Cow::Owned(vec![rows]),
            sky_light_arrays: Cow::Owned(vec![
                one_lit_cell(lit_cell(), 6),
                one_lit_cell(lit_cell(), 7),
            ]),
            block_light_arrays: Cow::Owned(vec![
                one_lit_cell(lit_cell(), 1),
                one_lit_cell(lit_cell(), 2),
            ]),
            ..Default::default()
        };
        let column = Column::decode(
            &ChunkData {
                data: &blob,
                ..Default::default()
            },
            &light,
            EXTENT,
            BLOCK_STATE_COUNT,
            BIOME_REGISTRY_LEN,
        )
        .expect("decode the column");
        let pos = ColumnPos::new(1, -2);
        let mut store = ColumnStore::default();
        store.enter(EXTENT);
        store.insert(pos, column);
        (store, pos)
    }

    fn lit_cell() -> usize {
        LocalPos::new(3, 5, 7).index()
    }

    /// The world coordinates of `lit_cell` in section `sy` of the lit column.
    fn lit_at(sy: i32) -> (i32, i32, i32) {
        (19, sy * SectionPos::SIZE as i32 + 5, -25)
    }

    fn light_update(pos: ColumnPos, light_data: LightData<'_>) -> ColumnLight {
        let bytes = encoded(&ClientboundLightUpdate {
            x: VarInt(pos.x),
            z: VarInt(pos.z),
            light_data,
        });
        let mut reader = bytes.as_slice();
        let packet = ClientboundLightUpdate::decode(&mut reader).expect("decode packet");
        assert!(reader.is_empty(), "{} bytes left unread", reader.len());
        assert_eq!(ColumnPos::new(packet.x.0, packet.z.0), pos);
        unpack_light_data(&packet.light_data, EXTENT.sections + 2).expect("unpack")
    }

    #[test]
    fn an_update_rewrites_the_rows_it_carries_and_leaves_the_rest() {
        let (mut store, pos) = lit_store();
        let row = 1u64 << 2;
        let update = light_update(
            pos,
            LightData {
                sky_light_mask: Cow::Owned(vec![row]),
                block_light_mask: Cow::Owned(vec![row]),
                sky_light_arrays: Cow::Owned(vec![one_lit_cell(lit_cell(), 3)]),
                block_light_arrays: Cow::Owned(vec![one_lit_cell(lit_cell(), 5)]),
                ..Default::default()
            },
        );
        store.relight(pos, &update);

        let (x, y, z) = lit_at(0);
        assert_eq!(
            light(&store, x, y, z),
            0x53,
            "sky 7 fell to 3, which an or-merge could not do"
        );
        let (x, y, z) = lit_at(-1);
        assert_eq!(light(&store, x, y, z), 0x16, "the row the update left out");
    }

    #[test]
    fn an_empty_row_darkens_that_layer_only() {
        let (mut store, pos) = lit_store();
        let update = light_update(
            pos,
            LightData {
                empty_sky_light_mask: Cow::Owned(vec![1u64 << 2]),
                ..Default::default()
            },
        );
        store.relight(pos, &update);

        let (x, y, z) = lit_at(0);
        assert_eq!(light(&store, x, y, z), 0x20, "sky dark, block untouched");
        let (x, y, z) = lit_at(-1);
        assert_eq!(light(&store, x, y, z), 0x16);
    }

    fn drained(store: &mut ColumnStore) -> Vec<ColumnChange> {
        let mut changes = Vec::new();
        store.drain_changes(&mut changes);
        changes
    }

    fn relit_rows(changes: &[ColumnChange]) -> Vec<(ColumnPos, Vec<i32>)> {
        changes
            .iter()
            .filter_map(|change| match change {
                ColumnChange::Relit(pos, rows) => Some((*pos, rows.clone())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn an_update_reports_the_rows_it_rewrote_so_the_renderer_can_follow() {
        let (mut store, pos) = lit_store();
        drained(&mut store);

        let update = light_update(
            pos,
            LightData {
                block_light_mask: Cow::Owned(vec![1u64 << 2]),
                block_light_arrays: Cow::Owned(vec![one_lit_cell(lit_cell(), 5)]),
                empty_sky_light_mask: Cow::Owned(vec![1u64 << 3]),
                ..Default::default()
            },
        );
        store.relight(pos, &update);

        assert_eq!(
            relit_rows(&drained(&mut store)),
            vec![(pos, vec![0])],
            "row 2 is section 0; darkening the already dark top padding row changes nothing"
        );

        let update = light_update(pos, LightData::default());
        store.relight(pos, &update);
        assert!(
            relit_rows(&drained(&mut store)).is_empty(),
            "an update that rewrites nothing is not a change"
        );
    }

    #[test]
    fn an_update_for_a_column_nobody_holds_is_dropped() {
        let (mut store, pos) = lit_store();
        let update = light_update(
            ColumnPos::new(9, 9),
            LightData {
                empty_sky_light_mask: Cow::Owned(vec![1u64 << 2]),
                ..Default::default()
            },
        );
        store.relight(ColumnPos::new(9, 9), &update);

        let (x, y, z) = lit_at(0);
        assert_eq!(
            light(&store, x, y, z),
            0x27,
            "the resident column is untouched"
        );
        assert!(store.holds(pos));
    }

    #[derive(Resource, Default)]
    struct Seen<const READER: usize>(Vec<(&'static str, ColumnPos, Vec<i32>)>);

    fn observed(change: &ColumnChange) -> (&'static str, ColumnPos, Vec<i32>) {
        match change {
            ColumnChange::Arrived(pos, _) => ("arrived", *pos, Vec::new()),
            ColumnChange::Departed(pos, _) => ("departed", *pos, Vec::new()),
            ColumnChange::Relit(pos, rows) => ("relit", *pos, rows.clone()),
            ColumnChange::Filled(at) => ("filled", ColumnPos::from(*at), Vec::new()),
            ColumnChange::Reblocked(at, _) => ("reblocked", ColumnPos::from(*at), Vec::new()),
            ColumnChange::Signed(at) => ("signed", ColumnPos::from(*at), Vec::new()),
        }
    }

    fn read_into<const READER: usize>(
        mut changes: bevy::ecs::message::MessageReader<ColumnChange>,
        mut seen: ResMut<Seen<READER>>,
    ) {
        seen.0.extend(changes.read().map(observed));
    }

    fn sign_compound(color: &str) -> mcrs_minecraft_nbt::compound::NbtCompound {
        let mut front = mcrs_minecraft_nbt::compound::NbtCompound::new();
        front.put_list(
            "messages",
            vec![mcrs_minecraft_nbt::tag::NbtTag::String(String::new()); 4],
        );
        front.put_string("color", color.to_owned());
        let mut compound = mcrs_minecraft_nbt::compound::NbtCompound::new();
        compound.put(
            "front_text",
            mcrs_minecraft_nbt::tag::NbtTag::Compound(front),
        );
        compound
    }

    #[test]
    fn a_sign_entity_comes_with_its_column_and_follows_its_updates() {
        let mut store = ColumnStore::default();
        store.enter(EXTENT);
        let at = BlockPos::new(17, 3, -2);
        let entries = [
            ChunkDataBlockEntity {
                packed_xz: 0x1e,
                y: 3,
                kind: BlockEntityType::Sign,
                data: Cow::Owned(sign_compound("red")),
            },
            ChunkDataBlockEntity {
                packed_xz: 0,
                y: 0,
                kind: BlockEntityType::Chest,
                data: Cow::Owned(Default::default()),
            },
        ];
        let column = Column::unlit(EXTENT.min_section_y, vec![None, None])
            .with_chunk_signs(ColumnPos::from(at), &entries);
        assert_eq!(column.signs().len(), 1);
        assert_eq!(column.signs()[0].pos, at);
        assert_eq!(column.signs()[0].texts.front_text.color, "red");
        store.insert(ColumnPos::from(at), column);
        assert!(store.has_sign(at));

        let texts = SignUpdate::from_compound(&sign_compound("blue")).unwrap();
        assert!(store.set_sign(at, texts.clone()));
        assert!(!store.set_sign(BlockPos::new(500, 3, 0), texts.clone()));
        assert!(!store.set_sign(BlockPos::new(17, 200, -2), texts));
        let sign = &store.get(ColumnPos::from(at)).unwrap().signs()[0];
        assert_eq!(sign.texts.front_text.color, "blue");

        assert!(store.remove_sign(at));
        assert!(!store.remove_sign(at));
        assert!(!store.has_sign(at));
    }

    #[test]
    fn cbk_prd_007_a_predicted_block_takes_the_server_state_only_at_its_acknowledgement() {
        let mut app = App::new();
        app.init_resource::<ColumnStore>()
            .init_resource::<Arrivals>()
            .init_resource::<BlockPredictions>()
            .add_message::<ColumnChange>()
            .add_systems(Update, settle_columns);
        let (stone, placed) = (state(1), state(2));
        let (broken, beside) = (BlockPos::new(3, 4, 5), BlockPos::new(4, 4, 5));
        let mut store = app.world_mut().resource_mut::<ColumnStore>();
        store.enter(EXTENT);
        store.insert(
            ColumnPos::new(0, 0),
            Column::unlit(EXTENT.min_section_y, vec![None; EXTENT.sections]),
        );
        store.set_block(broken, stone);
        let old = store
            .set_block(broken, AIR)
            .expect("the column holds stone");
        let mut predictions = app.world_mut().resource_mut::<BlockPredictions>();
        let sequence = predictions.next_sequence();
        predictions.record(sequence, broken, old);

        let mut arrivals = app.world_mut().resource_mut::<Arrivals>();
        let changes = [broken, beside].map(|pos| BlockChange { pos, state: placed });
        arrivals.queue.push_back(Arrival::Blocks(changes.into()));
        app.update();
        let store = app.world().resource::<ColumnStore>();
        assert_eq!(block(store, 3, 4, 5), AIR, "the prediction holds the block");
        assert_eq!(block(store, 4, 4, 5), placed);

        let mut arrivals = app.world_mut().resource_mut::<Arrivals>();
        arrivals.queue.push_back(Arrival::Acknowledge(sequence));
        app.update();
        let store = app.world().resource::<ColumnStore>();
        assert_eq!(block(store, 3, 4, 5), placed);
    }

    fn corpus_state(name: &str) -> BlockStateId {
        mcrs_minecraft_engine_registries::test_corpus()
            .0
            .default_state(name)
            .expect("the corpus holds the block")
    }

    /// A store of one column: stone up to `y = 15` with one cell of cave at `(5, 14, 5)`, and
    /// open sky above, with the sky light that the server sends for that.
    fn cave_app() -> App {
        let mut app = App::new();
        app.init_resource::<ColumnStore>()
            .init_resource::<Arrivals>()
            .init_resource::<BlockPredictions>()
            .insert_resource(mcrs_minecraft_engine_registries::test_corpus().0.clone())
            .add_message::<ColumnChange>()
            .add_systems(Update, settle_columns);
        let stone = corpus_state("minecraft:stone");
        let mut ground = Section::air();
        ground.blocks.fill(stone);
        ground.states = vec![stone];
        let mut store = app.world_mut().resource_mut::<ColumnStore>();
        store.enter(Extent {
            min_section_y: 0,
            sections: 2,
        });
        store.insert(
            ColumnPos::new(0, 0),
            Column::unlit(0, vec![Some(ground), None]),
        );
        store.set_block(BlockPos::new(5, 14, 5), AIR);
        for sy in [1, 2] {
            store.set_section_light(
                SectionPos::new(0, sy, 0),
                SectionLightChange {
                    sky: Some(&LightStorage::Uniform(15)),
                    block: None,
                },
            );
        }
        store.take_light_checks();
        app
    }

    fn sky_and_block(app: &App, pos: BlockPos) -> (u8, u8) {
        let light = app
            .world()
            .resource::<ColumnStore>()
            .light(pos)
            .expect("the column is resident");
        (light & 0x0f, light >> 4)
    }

    fn server_sets(app: &mut App, pos: BlockPos, state: BlockStateId) {
        app.world_mut()
            .resource_mut::<Arrivals>()
            .queue
            .push_back(Arrival::Blocks(vec![BlockChange { pos, state }]));
    }

    #[test]
    fn cbk_lgt_003_a_broken_roof_lets_the_sky_in_within_the_frame() {
        let mut app = cave_app();
        let cave = BlockPos::new(5, 14, 5);
        app.update();
        assert_eq!(sky_and_block(&app, cave), (0, 0));

        server_sets(&mut app, BlockPos::new(5, 15, 5), AIR);
        app.update();
        assert_eq!(sky_and_block(&app, cave), (15, 0));
    }

    #[test]
    fn cbk_lgt_001_a_placed_light_source_lights_and_its_break_darkens() {
        let mut app = cave_app();
        let source = BlockPos::new(5, 20, 5);
        server_sets(&mut app, source, corpus_state("minecraft:glowstone"));
        app.update();
        assert_eq!(sky_and_block(&app, source + IVec3::Y).1, 14);
        assert_eq!(sky_and_block(&app, BlockPos::new(5, 20, 15)).1, 5);

        server_sets(&mut app, source, AIR);
        app.update();
        assert_eq!(sky_and_block(&app, source + IVec3::Y), (15, 0));
    }

    #[test]
    fn cbk_lgt_009_server_light_of_the_frame_wins_over_the_local_light() {
        let mut app = cave_app();
        server_sets(&mut app, BlockPos::new(5, 15, 5), AIR);
        let rows = 4;
        let mut sky: Vec<RowLight> = (0..rows).map(|_| RowLight::Unchanged).collect();
        sky[1] = RowLight::Empty;
        let block = (0..rows).map(|_| RowLight::Unchanged).collect();
        app.world_mut()
            .resource_mut::<Arrivals>()
            .queue
            .push_back(Arrival::Light(
                ColumnPos::new(0, 0),
                ColumnLight { sky, block },
            ));
        app.update();
        assert_eq!(sky_and_block(&app, BlockPos::new(5, 14, 5)), (0, 0));
    }

    fn unlit_app(mode: UnlitColumnLight) -> App {
        let mut app = App::new();
        app.init_resource::<ColumnStore>()
            .init_resource::<Arrivals>()
            .init_resource::<BlockPredictions>()
            .init_resource::<Seen<0>>()
            .insert_resource(mcrs_minecraft_engine_registries::test_corpus().0.clone())
            .add_message::<ColumnChange>()
            .add_systems(
                Update,
                (settle_columns, read_into::<0>.after(settle_columns)),
            );
        let mut store = app.world_mut().resource_mut::<ColumnStore>();
        store.set_unlit_light(mode);
        store.enter(Extent {
            min_section_y: 0,
            sections: 2,
        });
        app
    }

    /// A column of air with no light, with a glowstone block at `lamp` when it is given.
    fn unlit_column(lamp: Option<BlockPos>) -> Column {
        let upper = lamp.map(|at| {
            let glow = corpus_state("minecraft:glowstone");
            let mut section = Section::air();
            section.blocks[LocalPos::from(at).index()] = glow;
            section.states.push(glow);
            section
        });
        Column::unlit(0, vec![None, upper])
    }

    /// Puts an unlit column into the store, as an embedder does.
    fn arrive(app: &mut App, column: ColumnPos, lamp: Option<BlockPos>) {
        app.world_mut()
            .resource_mut::<ColumnStore>()
            .insert(column, unlit_column(lamp));
    }

    /// A light update with the same chunk in every row of one layer and nothing in the other.
    fn layer_light(chunk: LightChunk, block: bool) -> ColumnLight {
        let rows = 4;
        let filled = || (0..rows).map(|_| RowLight::Filled(chunk)).collect();
        let none = || (0..rows).map(|_| RowLight::Unchanged).collect();
        if block {
            ColumnLight {
                sky: none(),
                block: filled(),
            }
        } else {
            ColumnLight {
                sky: filled(),
                block: none(),
            }
        }
    }

    const LAMP: BlockPos = BlockPos::new(5, 20, 5);

    #[test]
    fn cbk_lgt_014_a_column_without_light_is_lit_from_its_blocks() {
        let mut app = unlit_app(UnlitColumnLight::Block);
        arrive(&mut app, ColumnPos::new(0, 0), Some(LAMP));
        app.update();
        assert_eq!(sky_and_block(&app, LAMP + IVec3::Y).1, 14);
        assert_eq!(sky_and_block(&app, BlockPos::new(5, 20, 15)).1, 5);
        assert_eq!(sky_and_block(&app, BlockPos::new(15, 20, 15)).1, 0);
    }

    #[test]
    fn cbk_lgt_015_the_form_for_block_light_leaves_the_sky_alone() {
        let above = BlockPos::new(5, 25, 5);
        let mut app = unlit_app(UnlitColumnLight::Block);
        arrive(&mut app, ColumnPos::new(0, 0), Some(LAMP));
        app.update();
        assert_eq!(sky_and_block(&app, above).0, 0);

        let mut app = unlit_app(UnlitColumnLight::BlockAndSky);
        arrive(&mut app, ColumnPos::new(0, 0), Some(LAMP));
        app.update();
        assert_eq!(sky_and_block(&app, above), (15, 10));
    }

    #[test]
    fn cbk_lgt_013_with_the_option_off_nothing_is_calculated() {
        let mut app = unlit_app(UnlitColumnLight::Off);
        arrive(&mut app, ColumnPos::new(0, 0), Some(LAMP));
        app.update();
        assert_eq!(sky_and_block(&app, LAMP + IVec3::Y), (0, 0));
        assert!(!app.world().resource::<ColumnStore>().has_unlit_arrivals());
    }

    #[test]
    fn cbk_lgt_016_light_crosses_into_a_held_column_and_out_of_it() {
        let edge = BlockPos::new(15, 20, 5);
        let mut app = unlit_app(UnlitColumnLight::Block);
        arrive(&mut app, ColumnPos::new(1, 0), None);
        app.update();
        arrive(&mut app, ColumnPos::new(0, 0), Some(edge));
        app.update();
        assert_eq!(sky_and_block(&app, BlockPos::new(16, 20, 5)).1, 14);
        assert_eq!(sky_and_block(&app, BlockPos::new(17, 20, 5)).1, 13);

        // The lamp is held first and the dark column arrives beside it.
        let mut app = unlit_app(UnlitColumnLight::Block);
        arrive(&mut app, ColumnPos::new(0, 0), Some(edge));
        app.update();
        arrive(&mut app, ColumnPos::new(1, 0), None);
        app.update();
        assert_eq!(sky_and_block(&app, BlockPos::new(16, 20, 5)).1, 14);
    }

    #[test]
    fn cbk_lgt_016_a_held_column_whose_light_does_not_move_is_not_relit() {
        let mut app = unlit_app(UnlitColumnLight::Block);
        arrive(
            &mut app,
            ColumnPos::new(0, 0),
            Some(BlockPos::new(15, 20, 5)),
        );
        app.update();
        app.world_mut().resource_mut::<Seen<0>>().0.clear();
        // The light of the lamp falls to nothing before the column at x = -1.
        arrive(&mut app, ColumnPos::new(-1, 0), None);
        app.update();
        let seen = &app.world().resource::<Seen<0>>().0;
        assert!(
            !seen
                .iter()
                .any(|(kind, pos, _)| *kind == "relit" && *pos == ColumnPos::new(0, 0)),
            "{seen:?}"
        );
    }

    #[test]
    fn cbk_lgt_017_columns_that_arrive_in_one_frame_are_lit_in_one_step() {
        let mut app = unlit_app(UnlitColumnLight::Block);
        let pool = AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::new);
        // One tile of 4 x 4 columns, with a lamp at the border of two of them.
        for x in 0..4 {
            for z in 0..4 {
                let pos = ColumnPos::new(x, z);
                let lamp = match (x, z) {
                    (1, 1) => Some(BlockPos::new(31, 20, 21)),
                    (2, 1) => Some(BlockPos::new(40, 20, 21)),
                    _ => None,
                };
                let column = unlit_column(lamp);
                let task = pool.spawn(async move { Ok(Some((pos, column))) });
                app.world_mut()
                    .resource_mut::<Arrivals>()
                    .queue
                    .push_back(Arrival::Column(task));
            }
        }
        // Every decode has finished before the frame, so the frame lands all of them.
        while (app.world().resource::<Arrivals>().queue.iter())
            .any(|arrival| matches!(arrival, Arrival::Column(task) if !task.is_finished()))
        {
            std::thread::yield_now();
        }
        app.update();
        assert!(app.world().resource::<Arrivals>().queue.is_empty());
        assert_each_section_relit_once(&app);
        // Light of the lamp at x = 31 crosses into the column at x = 2 and meets the other.
        assert_eq!(sky_and_block(&app, BlockPos::new(32, 20, 21)).1, 14);
        assert_eq!(sky_and_block(&app, BlockPos::new(36, 20, 21)).1, 11);
    }

    /// Panics when a section of a column was announced as relit more than once.
    fn assert_each_section_relit_once(app: &App) {
        let seen = &app.world().resource::<Seen<0>>().0;
        let mut rows: Vec<(ColumnPos, i32)> = seen
            .iter()
            .filter(|(kind, ..)| *kind == "relit")
            .flat_map(|(_, pos, rows)| rows.iter().map(|&row| (*pos, row)))
            .collect();
        let relit = rows.len();
        rows.sort_by_key(|(pos, row)| (pos.x, pos.z, *row));
        rows.dedup();
        assert_eq!(
            rows.len(),
            relit,
            "a section was lit more than once: {seen:?}"
        );
    }

    #[test]
    fn cbk_lgt_014_a_column_taken_while_the_light_cannot_run_stays_for_later() {
        let mut app = unlit_app(UnlitColumnLight::Block);
        let blocks = app
            .world_mut()
            .remove_resource::<Blocks>()
            .expect("the app holds the blocks");
        arrive(&mut app, ColumnPos::new(0, 0), Some(LAMP));
        app.update();
        assert!(app.world().resource::<ColumnStore>().has_unlit_arrivals());
        assert_eq!(sky_and_block(&app, LAMP + IVec3::Y).1, 0);

        app.insert_resource(blocks);
        app.update();
        assert_eq!(sky_and_block(&app, LAMP + IVec3::Y).1, 14);
        assert!(!app.world().resource::<ColumnStore>().has_unlit_arrivals());
    }

    #[test]
    fn cbk_lgt_017_a_column_named_early_does_not_light_the_ones_still_waiting() {
        let lamp_a = BlockPos::new(15, 20, 5);
        let lamp_b = BlockPos::new(17, 20, 5);
        let mut app = unlit_app(UnlitColumnLight::Block);
        arrive(&mut app, ColumnPos::new(0, 0), Some(lamp_a));
        arrive(&mut app, ColumnPos::new(1, 0), Some(lamp_b));
        // The server light of the first column lights it before the later arrival, and the
        // second column waits for the end of the frame.
        let rows = 4;
        let unchanged = || (0..rows).map(|_| RowLight::Unchanged).collect::<Vec<_>>();
        let mut arrivals = app.world_mut().resource_mut::<Arrivals>();
        arrivals.queue.push_back(Arrival::Light(
            ColumnPos::new(0, 0),
            ColumnLight {
                sky: unchanged(),
                block: unchanged(),
            },
        ));
        arrivals
            .queue
            .push_back(Arrival::Forget(ColumnPos::new(9, 9)));
        app.update();
        assert_each_section_relit_once(&app);
        assert_eq!(sky_and_block(&app, BlockPos::new(16, 20, 5)).1, 14);
        assert_eq!(sky_and_block(&app, BlockPos::new(18, 20, 5)).1, 14);
    }

    #[test]
    fn cbk_lgt_017_a_block_change_in_a_column_that_waits_is_lit_once() {
        let mut app = unlit_app(UnlitColumnLight::Block);
        arrive(&mut app, ColumnPos::new(0, 0), Some(LAMP));
        let glow = corpus_state("minecraft:glowstone");
        server_sets(&mut app, BlockPos::new(10, 20, 10), glow);
        app.update();
        assert_each_section_relit_once(&app);
        assert_eq!(sky_and_block(&app, LAMP + IVec3::Y).1, 14);
        assert_eq!(sky_and_block(&app, BlockPos::new(10, 21, 10)).1, 14);
    }

    #[test]
    fn cbk_lgt_015_a_dimension_without_sky_keeps_the_sky_of_a_held_column() {
        let mut app = unlit_app(UnlitColumnLight::BlockAndSky);
        app.world_mut()
            .resource_mut::<ColumnStore>()
            .enter(Dimension {
                name: String::new(),
                extent: Extent {
                    min_section_y: 0,
                    sections: 2,
                },
                sky_light: false,
            });
        let mut held = unlit_column(None);
        held.relight(&layer_light(LightChunk::new([0xff; 2048]), false));
        app.world_mut()
            .resource_mut::<ColumnStore>()
            .insert(ColumnPos::new(1, 0), held);
        app.update();
        arrive(
            &mut app,
            ColumnPos::new(0, 0),
            Some(BlockPos::new(15, 20, 5)),
        );
        app.update();
        assert_eq!(sky_and_block(&app, BlockPos::new(16, 20, 5)), (15, 14));
        assert_eq!(sky_and_block(&app, BlockPos::new(20, 5, 5)).0, 15);
    }

    #[test]
    fn cbk_lgt_016_a_column_that_replaces_a_held_one_leaves_no_light_of_the_old_one() {
        let edge = BlockPos::new(15, 20, 5);
        let mut app = unlit_app(UnlitColumnLight::Block);
        arrive(&mut app, ColumnPos::new(1, 0), None);
        arrive(&mut app, ColumnPos::new(0, 0), Some(edge));
        app.update();
        assert_eq!(sky_and_block(&app, BlockPos::new(16, 20, 5)).1, 14);

        arrive(&mut app, ColumnPos::new(0, 0), None);
        app.update();
        for pos in [edge, BlockPos::new(14, 20, 5), BlockPos::new(16, 20, 5)] {
            assert_eq!(sky_and_block(&app, pos).1, 0, "{pos:?}");
        }
    }

    #[test]
    fn cbk_lgt_014_a_column_replaced_or_removed_before_the_step_is_not_lit() {
        let sky_cell = BlockPos::new(0, 0, 0);
        let mut app = unlit_app(UnlitColumnLight::BlockAndSky);
        arrive(&mut app, ColumnPos::new(0, 0), Some(LAMP));
        arrive(&mut app, ColumnPos::new(5, 5), Some(LAMP));
        let mut store = app.world_mut().resource_mut::<ColumnStore>();
        let mut provided = unlit_column(Some(LAMP));
        provided.relight(&layer_light(one_lit_cell(0, 15), false));
        store.insert(ColumnPos::new(0, 0), provided);
        store.remove(ColumnPos::new(5, 5));
        app.update();
        // The light that came with the column stays, and nothing was calculated over it.
        assert_eq!(sky_and_block(&app, sky_cell), (15, 0));
        assert_eq!(sky_and_block(&app, LAMP + IVec3::Y), (0, 0));
        assert!(!app.world().resource::<ColumnStore>().has_unlit_arrivals());
    }

    #[test]
    fn cbk_lgt_014_the_form_for_block_light_lights_a_column_that_came_with_sky_light() {
        let above = BlockPos::new(5, 25, 5);
        let mut app = unlit_app(UnlitColumnLight::Block);
        let mut column = unlit_column(Some(LAMP));
        column.relight(&layer_light(LightChunk::new([0xff; 2048]), false));
        app.world_mut()
            .resource_mut::<ColumnStore>()
            .insert(ColumnPos::new(0, 0), column);
        app.update();
        assert_eq!(sky_and_block(&app, above), (15, 10));

        // A column that came with block light of its own keeps it.
        let mut column = unlit_column(Some(LAMP));
        column.relight(&layer_light(one_lit_cell(0, 7), true));
        app.world_mut()
            .resource_mut::<ColumnStore>()
            .insert(ColumnPos::new(0, 0), column);
        app.update();
        assert_eq!(sky_and_block(&app, LAMP + IVec3::Y).1, 0);
    }

    #[test]
    fn cbk_lgt_017_a_batch_over_a_few_frames_lights_each_column_once_and_settles() {
        let mut app = unlit_app(UnlitColumnLight::BlockAndSky);
        let columns: Vec<ColumnPos> = (0..10)
            .flat_map(|x| (0..10).map(move |z| ColumnPos::new(x, z)))
            .collect();
        for frame in columns.chunks(25) {
            for &column in frame {
                let lamp = (column.x + column.z) % 7 == 0;
                let at = BlockPos::new(column.x * 16 + 8, 20, column.z * 16 + 8);
                arrive(&mut app, column, lamp.then_some(at));
            }
            app.update();
        }
        app.world_mut().resource_mut::<Seen<0>>().0.clear();
        for _ in 0..3 {
            app.update();
        }
        let seen = &app.world().resource::<Seen<0>>().0;
        assert!(seen.is_empty(), "the world kept changing: {seen:?}");
        assert!(!app.world().resource::<ColumnStore>().has_unlit_arrivals());
        // The lamps of two columns far apart both lit their surroundings.
        assert_eq!(sky_and_block(&app, BlockPos::new(8, 21, 8)).1, 14);
        assert_eq!(sky_and_block(&app, BlockPos::new(8 + 16 * 7, 21, 8)).1, 14);
    }

    #[test]
    fn two_readers_each_see_every_change_in_order() {
        use bevy::ecs::change_detection::DetectChanges;

        let mut app = App::new();
        app.init_resource::<ColumnStore>()
            .init_resource::<Arrivals>()
            .init_resource::<BlockPredictions>()
            .init_resource::<Seen<0>>()
            .init_resource::<Seen<1>>()
            .add_message::<ColumnChange>()
            .add_systems(
                Update,
                (
                    settle_columns,
                    (read_into::<0>, read_into::<1>).after(settle_columns),
                ),
            );
        let pos = ColumnPos::new(4, -7);
        let column = || Column::unlit(EXTENT.min_section_y, vec![None; EXTENT.sections]);

        app.world_mut()
            .resource_mut::<ColumnStore>()
            .insert(pos, column());
        app.update();

        let rows = EXTENT.sections + 2;
        let mut sky: Vec<RowLight> = (0..rows).map(|_| RowLight::Unchanged).collect();
        sky[1] = RowLight::Filled(one_lit_cell(0, 15));
        let block = (0..rows).map(|_| RowLight::Unchanged).collect();
        let mut store = app.world_mut().resource_mut::<ColumnStore>();
        store.relight(pos, &ColumnLight { sky, block });
        store.insert(pos, column());
        store.remove(pos);
        app.update();

        let expected = vec![
            ("arrived", pos, Vec::new()),
            ("relit", pos, vec![-1]),
            ("departed", pos, Vec::new()),
            ("arrived", pos, Vec::new()),
            ("departed", pos, Vec::new()),
        ];
        assert_eq!(app.world().resource::<Seen<0>>().0, expected);
        assert_eq!(app.world().resource::<Seen<1>>().0, expected);

        let changed = app.world().resource_ref::<ColumnStore>().last_changed();
        app.update();
        assert_eq!(
            app.world().resource::<Seen<0>>().0,
            expected,
            "a frame where nothing lands writes nothing"
        );
        assert_eq!(app.world().resource::<Seen<1>>().0, expected);
        assert_eq!(
            app.world().resource_ref::<ColumnStore>().last_changed(),
            changed,
            "settling nothing leaves the store's change tick where it was"
        );
    }

    #[test]
    fn clearing_departs_every_column_and_drops_what_was_on_its_way() {
        let column = || Column::unlit(EXTENT.min_section_y, vec![None; EXTENT.sections]);
        let (a, b) = (ColumnPos::new(0, 0), ColumnPos::new(1, 0));
        let mut store = ColumnStore::default();
        store.enter(EXTENT);
        store.insert(a, column());
        store.insert(b, column());
        drained(&mut store);

        store.clear();
        let mut departed: Vec<_> = drained(&mut store).iter().map(observed).collect();
        departed.sort_by_key(|(_, pos, _)| *pos);
        assert_eq!(
            departed,
            vec![("departed", a, Vec::new()), ("departed", b, Vec::new())]
        );
        assert!(store.is_empty());
        assert!(store.extent().is_none());
        assert!(store.holds_nothing());

        let mut arrivals = Arrivals {
            extent: Some(EXTENT),
            ..Arrivals::default()
        };
        arrivals.queue.push_back(Arrival::BatchStart);
        arrivals.queue.push_back(Arrival::Forget(a));
        assert!(!arrivals.holds_nothing());
        arrivals.clear();
        assert_eq!(arrivals.pending(), 0);
        assert!(arrivals.extent.is_none());
        assert!(arrivals.holds_nothing());
    }

    #[test]
    fn a_new_dimension_of_the_same_height_departs_every_column() {
        let dimension = |name: &str| Dimension {
            name: name.to_owned(),
            extent: EXTENT,
            sky_light: true,
        };
        let column = || Column::unlit(EXTENT.min_section_y, vec![None; EXTENT.sections]);
        let (a, b) = (ColumnPos::new(0, 0), ColumnPos::new(1, 0));
        let mut store = ColumnStore::default();
        store.enter(dimension("minecraft:overworld"));
        store.insert(a, column());
        store.insert(b, column());
        drained(&mut store);

        store.enter(dimension("minecraft:the_nether"));
        let mut changes: Vec<_> = drained(&mut store).iter().map(observed).collect();
        changes.sort_by_key(|(_, pos, _)| *pos);
        assert_eq!(
            changes,
            vec![("departed", a, Vec::new()), ("departed", b, Vec::new())]
        );
        assert!(store.is_empty());

        store.insert(a, column());
        drained(&mut store);
        store.enter(dimension("minecraft:the_nether"));
        assert!(
            drained(&mut store).is_empty(),
            "a respawn into the same dimension keeps its columns"
        );
        assert!(store.holds(a));
    }

    #[test]
    fn the_vertical_extent_comes_from_the_dimension_type_the_login_named() {
        use mcrs_minecraft_dimension::Dimension;
        use mcrs_minecraft_dimension::dimension_keys::{OVERWORLD, THE_NETHER};
        use mcrs_minecraft_game_mode::GameMode;

        let registries = mcrs_minecraft_game_registries::registry_load::test_registries();
        let types = registries.registry::<DimensionTypeFile>().unwrap();
        let named = |key: &str, dimension: ResourceKeyBuf<Dimension>| {
            let id = types
                .by_name(&ResourceLocationBuf::read(key).unwrap())
                .unwrap();
            CurrentDimension::of(&PlayerSpawnInfo::new(
                Id::checked(id.number(), types.len()).unwrap(),
                dimension,
                GameMode::Survival,
            ))
        };

        let overworld = named("minecraft:overworld", OVERWORLD.into());
        let reach = |current| {
            dimension_of(registries, current)
                .map(|dimension| (dimension.extent, dimension.sky_light))
        };
        assert_eq!(
            reach(&overworld),
            Some((
                Extent {
                    min_section_y: -4,
                    sections: 24,
                },
                true
            ))
        );
        let nether = named("minecraft:the_nether", THE_NETHER.into());
        assert_eq!(
            reach(&nether),
            Some((
                Extent {
                    min_section_y: 0,
                    sections: 16,
                },
                false
            ))
        );
        let past = CurrentDimension::of(&PlayerSpawnInfo::new(
            Id::checked(types.len() as u16, types.len() + 1).unwrap(),
            OVERWORLD.into(),
            GameMode::Survival,
        ));
        assert_eq!(reach(&past), None, "no such dimension type");
    }
}
