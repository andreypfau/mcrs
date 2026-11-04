use std::borrow::Cow;

use bytes::{BufMut, Bytes, BytesMut};
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_engine_world::palette::{BiomePalette, ChunkBlocks};
use mcrs_minecraft_heightmap::ColumnHeightmapSet;
use mcrs_minecraft_protocol::{Packet, VarInt};
use mcrs_minecraft_protocol_game::ColumnPos;
use mcrs_minecraft_protocol_game::chunk::{
    ChunkData, ChunkDataBlockEntity, LightChunk, LightData, encode_section,
};
use mcrs_minecraft_protocol_game::clientbound::ClientboundLevelChunkWithLight;
use mcrs_minecraft_protocol_game::section::{biome_direct_bits, block_direct_bits};
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_volume::{PaletteBuffer, PalettedSection, SectionBlocks};
use mcrs_voxel::BlockStateId;
use tracing::warn;

use crate::block_entity_packet::packet_entry;
use crate::heightmap::client_heightmaps;
use crate::light_codec::{build_fullbright_light_data, build_stored_light_data};
use crate::stored_column::StoredColumn;

/// The bits a palette entry takes when a section is too varied for a palette.
#[derive(Clone, Copy)]
pub(crate) struct DirectBits {
    block: u32,
    biome: u32,
}

impl DirectBits {
    pub(crate) fn of(blocks: &Blocks, registries: &CurrentRegistries) -> Self {
        Self {
            block: block_direct_bits(blocks.state_count()),
            biome: biome_direct_bits(
                registries
                    .registry::<BiomeFile>()
                    .map_or(0, |biomes| biomes.len()),
            ),
        }
    }
}

/// A light array and the length ahead of it on the wire.
const WIRE_LIGHT_ARRAY_BYTES: usize = size_of::<LightChunk>() + 3;

/// Room for the column's position, masks, heightmaps and block entities.
const PACKET_SLACK_BYTES: usize = 4 * 1024;

/// Appends the wire form of each section to `data`, or gives `None` when one does not encode.
pub(crate) fn encode_sections<'a>(
    column: ColumnPos,
    sections: impl IntoIterator<Item = (&'a ChunkBlocks, &'a BiomePalette)>,
    direct_bits: DirectBits,
    data: &mut Vec<u8>,
) -> Option<()> {
    for (blocks, biomes) in sections {
        let non_air = PalettedSection(&blocks.0.0)
            .palette(&mut PaletteBuffer::default())
            .count(|state| state != BlockStateId::AIR);
        // A section cut short shifts every section after it, so the client would
        // read the rest of the column as garbage.
        if let Err(err) = encode_section(
            non_air,
            0,
            &blocks.0.0,
            &biomes.0,
            direct_bits.block,
            direct_bits.biome,
            &mut *data,
        ) {
            warn!(
                %err,
                col_x = column.x,
                col_z = column.z,
                "encoding a chunk section for the wire"
            );
            return None;
        }
    }
    Some(())
}

pub(crate) fn push_block_entity(
    entry: &GeneratedBlockEntity,
    block_entities: &mut Vec<ChunkDataBlockEntity<'static>>,
) {
    match packet_entry(entry) {
        Ok(entry) => block_entities.push(entry),
        Err(err) => warn!(%err, "encoding a block entity for the wire"),
    }
}

/// The parts of a `level_chunk_with_light` packet besides its sections.
pub(crate) struct ColumnParts {
    pub(crate) heightmaps: Vec<(VarInt, Vec<u64>)>,
    pub(crate) block_entities: Vec<ChunkDataBlockEntity<'static>>,
    pub(crate) light_data: LightData<'static>,
}

/// Encodes the whole `level_chunk_with_light` packet with its id.
pub(crate) fn encode_packet(column: ColumnPos, data: &[u8], parts: ColumnParts) -> Option<Bytes> {
    let ColumnParts {
        heightmaps,
        block_entities,
        light_data,
    } = parts;
    let light_arrays = light_data.sky_light_arrays.len() + light_data.block_light_arrays.len();
    let mut packet = BytesMut::with_capacity(
        data.len() + light_arrays * WIRE_LIGHT_ARRAY_BYTES + PACKET_SLACK_BYTES,
    );
    let encoded = ClientboundLevelChunkWithLight {
        pos: column,
        chunk_data: ChunkData {
            heightmaps: heightmaps
                .into_iter()
                .map(|(kind, longs)| (kind, Cow::Owned(longs)))
                .collect(),
            data,
            block_entities: Cow::Owned(block_entities),
        },
        light_data,
    }
    .encode_with_id((&mut packet).writer());
    if let Err(err) = encoded {
        warn!(
            %err,
            col_x = column.x,
            col_z = column.z,
            "encoding a column for the wire"
        );
        return None;
    }
    Some(packet.freeze())
}

/// How a column from the data layer puts its light on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StoredLight {
    /// The world lights its columns, and the column holds the light it had.
    Kept { has_sky_light: bool },
    /// The world does not light its columns, so the column goes out at full sky light.
    Fullbright,
}

/// Encodes the packet of a column from the data layer.
pub(crate) fn encode_stored_column(
    column: ColumnPos,
    stored: &StoredColumn,
    direct_bits: DirectBits,
    light: StoredLight,
) -> Option<Bytes> {
    let mut data = Vec::with_capacity(16 * 1024);
    encode_sections(
        column,
        stored
            .sections
            .iter()
            .map(|section| (&section.blocks, &section.biomes)),
        direct_bits,
        &mut data,
    )?;
    let mut block_entities = Vec::new();
    for entry in &stored.block_entities {
        push_block_entity(entry, &mut block_entities);
    }
    let light_data = match light {
        StoredLight::Kept { has_sky_light } => build_stored_light_data(stored, has_sky_light),
        StoredLight::Fullbright => build_fullbright_light_data(stored.sections.len() + 2),
    };
    let heightmaps = stored
        .heightmaps
        .as_ref()
        .map(
            |ColumnHeightmapSet {
                 surface,
                 motion,
                 no_leaves,
                 ..
             }| { client_heightmaps(surface, motion, no_leaves) },
        )
        .unwrap_or_default();
    encode_packet(
        column,
        &data,
        ColumnParts {
            heightmaps,
            block_entities,
            light_data,
        },
    )
}
