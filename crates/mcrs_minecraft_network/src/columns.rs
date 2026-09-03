use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use bevy_app::App;
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::prelude::{On, Query, ResMut, Resource};
use bevy_tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use log::error;
use mcrs_minecraft_protocol::ColumnPos;
use mcrs_minecraft_protocol::chunk::{
    ChunkData, LightChunk, LightData, Palette, PalettedContainer,
};
use mcrs_minecraft_protocol::light_codec::{RowLight, unpack_light_data};
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundForgetLevelChunk, ClientboundLevelChunkWithLight, ClientboundLogin,
};
use mcrs_minecraft_protocol::section::{Biomes, Blocks, NetworkSectionKind};
use mcrs_minecraft_protocol::{Decode, Packet};
use mcrs_voxel_storage::unpack_into;

use crate::ConnectionState;
use crate::client::{ReceivedRegistries, ReceivedRegistry};
use crate::event::ReceivedPacketEvent;

pub const SECTION_SIZE: usize = 16;
pub const SECTION_VOLUME: usize = SECTION_SIZE * SECTION_SIZE * SECTION_SIZE;
pub const BIOME_CELLS: usize = 64;

/// Block state 0. The network palette is the server's global one, so no remap
/// stands between a stored value and the block catalog.
pub const AIR: u16 = 0;

/// Sky nibble full, block nibble dark: what a block outside every resident
/// column is lit by.
const OPEN_SKY: u8 = 0x0f;

/// The dimension's vertical reach in sections, from its `dimension_type`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Extent {
    pub min_section_y: i32,
    pub sections: usize,
}

pub struct Section {
    pub blocks: Box<[u16; SECTION_VOLUME]>,
    pub biomes: Box<[u8; BIOME_CELLS]>,
}

pub struct Column {
    min_section_y: i32,
    sections: Vec<Option<Section>>,
    /// One row per section plus the padding row the light packet carries at
    /// each end, so row `i` holds section `min_section_y + i - 1`.
    light: Vec<Option<Box<[u8; SECTION_VOLUME]>>>,
}

/// The columns the server has sent and not taken back, keyed by position.
/// Cloning hands an async mesh task the same sections, not a copy of them.
#[derive(Resource, Default, Clone)]
pub struct ColumnStore {
    extent: Option<Extent>,
    columns: HashMap<ColumnPos, Arc<Column>>,
}

impl ColumnStore {
    pub fn extent(&self) -> Option<Extent> {
        self.extent
    }

    /// A column placed against one extent cannot be read against another, so
    /// entering a dimension of a different shape drops what is resident.
    pub fn enter(&mut self, extent: Extent) {
        if self.extent != Some(extent) {
            self.columns.clear();
            self.extent = Some(extent);
        }
    }

    pub fn insert(&mut self, pos: ColumnPos, column: Column) {
        self.columns.insert(pos, Arc::new(column));
    }

    pub fn remove(&mut self, pos: ColumnPos) {
        self.columns.remove(&pos);
    }

    pub fn holds(&self, pos: ColumnPos) -> bool {
        self.columns.contains_key(&pos)
    }

    pub fn positions(&self) -> impl Iterator<Item = ColumnPos> + '_ {
        self.columns.keys().copied()
    }

    pub fn len(&self) -> usize {
        self.columns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    #[inline]
    pub fn block(&self, x: i32, y: i32, z: i32) -> u16 {
        match self.section(section_of(x), section_of(y), section_of(z)) {
            Some(section) => section.blocks[cell_index(x, y, z)],
            None => AIR,
        }
    }

    #[inline]
    pub fn light(&self, x: i32, y: i32, z: i32) -> u8 {
        match self.column(section_of(x), section_of(z)) {
            Some(column) => column.light(section_of(y), cell_index(x, y, z)),
            None => OPEN_SKY,
        }
    }

    #[inline]
    pub fn section(&self, sx: i32, sy: i32, sz: i32) -> Option<&Section> {
        self.column(sx, sz)?.section(sy)
    }

    pub fn biome(&self, sx: i32, sy: i32, sz: i32, cell: usize) -> u8 {
        match self.section(sx, sy, sz) {
            Some(section) => section.biomes[cell],
            None => 0,
        }
    }

    #[inline]
    fn column(&self, sx: i32, sz: i32) -> Option<&Column> {
        self.columns.get(&ColumnPos::new(sx, sz)).map(Arc::as_ref)
    }
}

impl Column {
    /// A column nobody has lit: every block in it reads as dark.
    pub fn unlit(min_section_y: i32, sections: Vec<Option<Section>>) -> Column {
        let light = (0..sections.len() + 2).map(|_| None).collect();
        Column {
            min_section_y,
            sections,
            light,
        }
    }

    pub fn decode(data: &ChunkData<'_>, light: &LightData<'_>, extent: Extent) -> Result<Column> {
        let sections = data
            .sections(extent.sections)?
            .iter()
            .map(|section| {
                if section.non_empty_block_count == 0 {
                    return Ok(None);
                }
                let mut blocks = Box::new([AIR; SECTION_VOLUME]);
                expand::<Blocks, _>(&section.blocks, |id| id.0, blocks.as_mut_slice())
                    .context("blocks")?;
                let mut cells = [0u16; BIOME_CELLS];
                expand::<Biomes, _>(&section.biomes, u16::from, &mut cells).context("biomes")?;
                let mut biomes = Box::new([0u8; BIOME_CELLS]);
                for (out, cell) in biomes.iter_mut().zip(cells) {
                    *out = cell as u8;
                }
                Ok(Some(Section { blocks, biomes }))
            })
            .collect::<Result<Vec<_>>>()?;

        let unpacked = unpack_light_data(light, extent.sections + 2)?;
        let light = unpacked
            .sky
            .iter()
            .zip(&unpacked.block)
            .map(|(sky, block)| merge_light(sky, block))
            .collect();

        Ok(Column {
            min_section_y: extent.min_section_y,
            sections,
            light,
        })
    }

    #[inline]
    fn section(&self, sy: i32) -> Option<&Section> {
        let index = usize::try_from(sy - self.min_section_y).ok()?;
        self.sections.get(index)?.as_ref()
    }

    #[inline]
    fn light(&self, sy: i32, cell: usize) -> u8 {
        let row = sy - self.min_section_y + 1;
        if row < 0 {
            return 0;
        }
        match self.light.get(row as usize) {
            Some(Some(light)) => light[cell],
            Some(None) => 0,
            None => OPEN_SKY,
        }
    }
}

#[inline]
fn section_of(coordinate: i32) -> i32 {
    coordinate.div_euclid(SECTION_SIZE as i32)
}

#[inline]
fn cell_index(x: i32, y: i32, z: i32) -> usize {
    let local = |coordinate: i32| coordinate.rem_euclid(SECTION_SIZE as i32) as usize;
    (local(y) * SECTION_SIZE + local(z)) * SECTION_SIZE + local(x)
}

fn expand<K: NetworkSectionKind, V: Copy>(
    container: &PalettedContainer<V>,
    id: impl Fn(V) -> u16,
    out: &mut [u16],
) -> Result<()> {
    let bits = K::wire_storage_bits(container.bits_per_entry);
    let unpack = |out: &mut [u16]| {
        unpack_into(bits, &container.packed_data, out).map_err(|length| {
            anyhow!(
                "{bits} bits per entry needs {} packed longs, got {}",
                length.expected,
                length.found
            )
        })
    };
    match &container.palette {
        Palette::Single(value) => out.fill(id(*value)),
        Palette::Indirect(entries) => {
            unpack(out)?;
            for cell in out.iter_mut() {
                let entry = entries
                    .get(*cell as usize)
                    .with_context(|| format!("palette index {cell} of {}", entries.len()))?;
                *cell = id(*entry);
            }
        }
        Palette::Direct => unpack(out)?,
    }
    Ok(())
}

fn merge_light(sky: &RowLight, block: &RowLight) -> Option<Box<[u8; SECTION_VOLUME]>> {
    let filled = |row: &RowLight| match row {
        RowLight::Filled(chunk) => Some(*chunk),
        _ => None,
    };
    let (sky, block) = (filled(sky), filled(block));
    if sky.is_none() && block.is_none() {
        return None;
    }
    let mut out = Box::new([0u8; SECTION_VOLUME]);
    if let Some(sky) = sky {
        write_nibbles(&sky, &mut out, 0);
    }
    if let Some(block) = block {
        write_nibbles(&block, &mut out, 4);
    }
    Some(out)
}

fn write_nibbles(source: &LightChunk, out: &mut [u8; SECTION_VOLUME], shift: u32) {
    for (index, byte) in source.as_bytes().iter().enumerate() {
        out[index * 2] |= (byte & 0x0f) << shift;
        out[index * 2 + 1] |= (byte >> 4) << shift;
    }
}

fn extent_of(registries: &[ReceivedRegistry], dimension_type_id: i32) -> Option<Extent> {
    let data = registries
        .iter()
        .find(|registry| registry.registry == "minecraft:dimension_type")?
        .entries
        .get(usize::try_from(dimension_type_id).ok()?)?
        .data
        .as_ref()?;
    Some(Extent {
        min_section_y: section_of(data.get_int("min_y")?),
        sections: usize::try_from(data.get_int("height")?).ok()? / SECTION_SIZE,
    })
}

pub(crate) fn build(app: &mut App) {
    app.init_resource::<ColumnStore>();
    app.init_resource::<Arrivals>();
    app.add_observer(receive_column_packets);
}

/// Column packets in the order they came, each one decoding on the compute pool. A column's
/// forget can only be applied once the column itself has landed, so the queue is drained from
/// the front and stops at the first decode still running.
#[derive(Resource, Default)]
pub struct Arrivals {
    queue: VecDeque<Arrival>,
    extent: Option<Extent>,
}

enum Arrival {
    Enter(Extent),
    Column(Task<Result<(ColumnPos, Column)>>),
    Forget(ColumnPos),
}

impl Arrivals {
    pub fn pending(&self) -> usize {
        self.queue.len()
    }
}

fn receive_column_packets(
    event: On<ReceivedPacketEvent>,
    connections: Query<(&ConnectionState, &ReceivedRegistries)>,
    mut arrivals: ResMut<Arrivals>,
) {
    let Ok((state, registries)) = connections.get(event.entity) else {
        return;
    };
    if *state != ConnectionState::Game {
        return;
    }

    if let Some(login) = event.decode::<ClientboundLogin>() {
        let dimension_type = login.player_spawn_info.dimension_type_id.0;
        match extent_of(&registries.0, dimension_type) {
            Some(extent) => {
                arrivals.extent = Some(extent);
                arrivals.queue.push_back(Arrival::Enter(extent));
            }
            None => error!(
                "dimension type {dimension_type} carries no min_y and height: \
                 columns have nowhere to sit"
            ),
        }
    } else if event.id == ClientboundLevelChunkWithLight::ID {
        let Some(extent) = arrivals.extent else {
            return;
        };
        let data = event.data.clone();
        let task = AsyncComputeTaskPool::get().spawn(async move {
            let mut bytes = &data[..];
            let packet = ClientboundLevelChunkWithLight::decode(&mut bytes)
                .map_err(|error| anyhow!("chunk packet: {error:?}"))?;
            let column = Column::decode(&packet.chunk_data, &packet.light_data, extent)
                .with_context(|| format!("column {:?}", packet.pos))?;
            Ok((packet.pos, column))
        });
        arrivals.queue.push_back(Arrival::Column(task));
    } else if let Some(forget) = event.decode::<ClientboundForgetLevelChunk>() {
        arrivals
            .queue
            .push_back(Arrival::Forget(ColumnPos::new(forget.x, forget.z)));
    }
}

/// Lands what has finished decoding, in packet order. The store is only borrowed mutably when
/// something actually lands, so its change tick means a column moved.
pub(crate) fn settle_columns(mut arrivals: ResMut<Arrivals>, mut store: ResMut<ColumnStore>) {
    let arrivals = arrivals.bypass_change_detection();
    while let Some(arrival) = arrivals.queue.pop_front() {
        match arrival {
            Arrival::Enter(extent) => store.enter(extent),
            Arrival::Forget(pos) => store.remove(pos),
            Arrival::Column(mut task) => match check_ready(&mut task) {
                None => {
                    arrivals.queue.push_front(Arrival::Column(task));
                    break;
                }
                Some(Ok((pos, column))) => store.insert(pos, column),
                Some(Err(error)) => error!("{error:#}"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::RegistryEntry;
    use mcrs_minecraft_nbt::compound::NbtCompound;
    use mcrs_minecraft_protocol::chunk::ChunkSection;
    use mcrs_minecraft_protocol::{BlockStateId, Decode, Encode};
    use mcrs_voxel_storage::pack_from;
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

    fn single_biome(id: u8) -> PalettedContainer<u8> {
        PalettedContainer {
            bits_per_entry: 0,
            palette: Palette::Single(id),
            packed_data: Box::new([]),
        }
    }

    fn air_section() -> ChunkSection {
        ChunkSection {
            non_empty_block_count: 0,
            fluid_count: 0,
            blocks: PalettedContainer {
                bits_per_entry: 0,
                palette: Palette::Single(BlockStateId(AIR)),
                packed_data: Box::new([]),
            },
            biomes: single_biome(3),
        }
    }

    fn section_of_states(ids: &[u32]) -> ChunkSection {
        let mut palette: Vec<u32> = Vec::new();
        for &id in ids {
            if !palette.contains(&id) {
                palette.push(id);
            }
        }
        ChunkSection {
            non_empty_block_count: ids.iter().filter(|&&id| id != 0).count() as u16,
            fluid_count: 0,
            blocks: PalettedContainer {
                bits_per_entry: 4,
                palette: Palette::Indirect(
                    palette.iter().map(|&id| BlockStateId(id as u16)).collect(),
                ),
                packed_data: pack_from(4, ids, |id| {
                    palette
                        .iter()
                        .position(|entry| entry == id)
                        .expect("interned") as u32
                }),
            },
            biomes: single_biome(3),
        }
    }

    fn one_lit_cell(cell: usize, level: u8) -> LightChunk {
        let mut nibbles = [0u8; 2048];
        nibbles[cell / 2] = level << ((cell % 2) * 4);
        LightChunk::new(nibbles)
    }

    #[test]
    fn a_decoded_packet_reads_back_where_it_put_its_blocks_and_its_light() {
        let cell = cell_index(3, 5, 7);
        let mut states = vec![u32::from(AIR); SECTION_VOLUME];
        states[cell] = 42;
        let blob = [air_section(), section_of_states(&states)]
            .iter()
            .flat_map(encoded)
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
        let column = Column::decode(&packet.chunk_data, &packet.light_data, EXTENT)
            .expect("decode the column");
        store.insert(packet.pos, column);

        let (x, z) = (1 * SECTION_SIZE as i32 + 3, -2 * SECTION_SIZE as i32 + 7);
        assert_eq!(store.block(x, 5, z), 42, "the state the packet carried");
        assert_eq!(store.block(x + 1, 5, z), AIR, "its neighbour");
        assert_eq!(store.block(x + 64, 5, z), AIR, "a column nobody sent");
        assert_eq!(store.light(x, 5, z), 0x27, "sky 7 low, block 2 high");
        assert_eq!(
            store.light(x, 5, z + 1),
            0x00,
            "an unlit cell of a lit section"
        );
        assert_eq!(store.light(x, 5 + 32, z), OPEN_SKY, "above the dimension");
        assert!(
            store.section(1, -1, -2).is_none(),
            "a section of nothing but air is not resident"
        );
        assert!(store.section(1, 0, -2).is_some());
        assert_eq!(store.biome(1, 0, -2, 0), 3);

        store.remove(packet.pos);
        assert!(store.is_empty(), "the forget packet takes the column back");
    }

    #[test]
    fn the_vertical_extent_comes_from_the_dimension_type_the_login_named() {
        let mut overworld = NbtCompound::new();
        overworld.put_int("min_y", -64);
        overworld.put_int("height", 384);
        let registries = vec![
            ReceivedRegistry {
                registry: "minecraft:biome".to_owned(),
                entries: Vec::new(),
            },
            ReceivedRegistry {
                registry: "minecraft:dimension_type".to_owned(),
                entries: vec![
                    RegistryEntry {
                        id: "minecraft:the_nether".to_owned(),
                        data: None,
                    },
                    RegistryEntry {
                        id: "minecraft:overworld".to_owned(),
                        data: Some(overworld),
                    },
                ],
            },
        ];

        assert_eq!(
            extent_of(&registries, 1),
            Some(Extent {
                min_section_y: -4,
                sections: 24,
            })
        );
        assert_eq!(
            extent_of(&registries, 0),
            None,
            "an entry sent without data"
        );
        assert_eq!(extent_of(&registries, 9), None, "no such dimension type");
    }
}
