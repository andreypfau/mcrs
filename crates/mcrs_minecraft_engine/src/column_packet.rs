use std::borrow::Cow;

use bytes::{BufMut, Bytes, BytesMut};
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_heightmap::ColumnHeightmapSet;
use mcrs_minecraft_protocol::Packet;
use mcrs_minecraft_protocol_game::ColumnPos;
use mcrs_minecraft_protocol_game::chunk::{
    ChunkData, ChunkDataBlockEntity, LightChunk, encode_section,
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
use mcrs_minecraft_region_disk::stored_column::StoredColumn;

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
    for section in &stored.sections {
        let non_air = PalettedSection(&section.blocks.0.0)
            .palette(&mut PaletteBuffer::default())
            .count(|state| state != BlockStateId::AIR);
        // A section cut short shifts every section after it, so the client would
        // read the rest of the column as garbage.
        if let Err(err) = encode_section(
            non_air,
            0,
            &section.blocks.0.0,
            &section.biomes.0,
            direct_bits.block,
            direct_bits.biome,
            &mut data,
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
    let block_entities: Vec<ChunkDataBlockEntity<'static>> = stored
        .block_entities
        .iter()
        .filter_map(|entry| match packet_entry(entry) {
            Ok(entry) => Some(entry),
            Err(err) => {
                warn!(%err, "encoding a block entity for the wire");
                None
            }
        })
        .collect();
    let light_data = match light {
        StoredLight::Kept { has_sky_light } => build_stored_light_data(stored, has_sky_light),
        StoredLight::Fullbright => build_fullbright_light_data(stored.sections.len() + 2),
    };
    let ColumnHeightmapSet {
        surface,
        motion,
        no_leaves,
        ..
    } = &stored.heightmaps;
    let heightmaps = client_heightmaps(surface, motion, no_leaves);
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
            data: &data,
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
