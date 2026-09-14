use mcrs_minecraft_chunk::{PalettedContainer, SectionKind, VoxelId, pack_from, packed_len};
use mcrs_minecraft_protocol::chunk::{ChunkData, ChunkSection};
use mcrs_minecraft_protocol::light_codec::{RowLight, unpack_light_data};
use mcrs_minecraft_protocol::section::{Biomes, Blocks};
use mcrs_minecraft_protocol::{Decode, Encode, VarInt};
use std::borrow::Cow;

type BlockContainer = PalettedContainer<VoxelId, { Blocks::SIZE }>;
type BiomeContainer = PalettedContainer<u8, { Biomes::SIZE }>;

fn encoded<T: Encode>(value: &T) -> Vec<u8> {
    let mut buf = Vec::new();
    value.encode(&mut buf).expect("encode");
    buf
}

fn blocks(ids: &[u32]) -> BlockContainer {
    let cells: Vec<VoxelId> = ids.iter().map(|&id| VoxelId(id as u16)).collect();
    PalettedContainer::from_cells(&cells)
}

fn biomes(ids: &[u32]) -> BiomeContainer {
    let cells: Vec<u8> = ids.iter().map(|&id| id as u8).collect();
    PalettedContainer::from_cells(&cells)
}

fn block_ids(distinct: u32) -> Vec<u32> {
    (0..Blocks::ENTRY_COUNT as u32)
        .map(|i| i % distinct)
        .collect()
}

fn biome_ids(distinct: u32) -> Vec<u32> {
    (0..Biomes::ENTRY_COUNT as u32)
        .map(|i| i % distinct)
        .collect()
}

fn round_trip<C>(container: C)
where
    C: Encode + for<'a> Decode<'a> + PartialEq + std::fmt::Debug,
{
    let bytes = encoded(&container);
    let mut r = bytes.as_slice();
    let decoded = C::decode(&mut r).expect("decode container");
    assert!(r.is_empty(), "{} bytes left unread", r.len());
    assert_eq!(decoded, container);
    assert_eq!(encoded(&decoded), bytes);
}

#[test]
fn every_palette_form_round_trips_byte_for_byte() {
    round_trip(blocks(&block_ids(1)));
    round_trip(blocks(&block_ids(9)));
    round_trip(blocks(&block_ids(400)));

    round_trip(biomes(&biome_ids(1)));
    round_trip(biomes(&biome_ids(3)));
    round_trip(biomes(&biome_ids(40)));
}

#[test]
fn the_three_forms_are_the_ones_under_test() {
    assert_eq!(
        encoded(&blocks(&block_ids(1))),
        [0, 0],
        "a single value and no packed longs"
    );

    let indirect = encoded(&blocks(&block_ids(9)));
    assert_eq!(indirect[..2], [4, 9], "four bits, then a palette of nine");
    assert_eq!(
        indirect.len(),
        2 + 9 + 8 * packed_len(4, Blocks::ENTRY_COUNT)
    );

    let direct = encoded(&blocks(&block_ids(400)));
    assert_eq!(direct[0], 15);
    assert_eq!(
        direct.len(),
        1 + 8 * packed_len(15, Blocks::ENTRY_COUNT),
        "no palette list"
    );

    assert_eq!(encoded(&biomes(&biome_ids(40)))[0], 7);
}

#[test]
fn a_palette_index_past_the_palette_is_an_error() {
    let mut indices = vec![0u16; Blocks::ENTRY_COUNT];
    indices[17] = 5;
    let mut bytes = Vec::new();
    4u8.encode(&mut bytes).unwrap();
    for id in [2, 0, 1] {
        VarInt(id).encode(&mut bytes).unwrap();
    }
    for word in pack_from(4, &indices, |&index| index as u32).iter() {
        word.encode(&mut bytes).unwrap();
    }
    assert!(BlockContainer::decode(&mut bytes.as_slice()).is_err());
}

fn column(section_count: usize) -> Vec<ChunkSection> {
    (0..section_count)
        .map(|index| ChunkSection {
            non_empty_block_count: index as u16 * 7,
            fluid_count: 0,
            blocks: blocks(&block_ids(1 + index as u32 * 60)),
            biomes: biomes(&biome_ids(1 + index as u32 * 5)),
        })
        .collect()
}

/// The server writes the blob field by field rather than through
/// `ChunkSection`, so the two orders have to agree byte for byte.
fn write_column_the_way_the_server_does(sections: &[ChunkSection]) -> Vec<u8> {
    let mut data = Vec::new();
    for section in sections {
        section.non_empty_block_count.encode(&mut data).unwrap();
        section.fluid_count.encode(&mut data).unwrap();
        section.blocks.encode(&mut data).unwrap();
        section.biomes.encode(&mut data).unwrap();
    }
    data
}

#[test]
fn a_multi_section_blob_decodes_back_to_its_sections() {
    for section_count in [1, 24, 40] {
        let sections = column(section_count);
        let data = write_column_the_way_the_server_does(&sections);
        let chunk = ChunkData {
            data: &data,
            ..Default::default()
        };

        let decoded = chunk.sections(section_count).expect("decode sections");
        assert_eq!(decoded, sections);
        assert_eq!(write_column_the_way_the_server_does(&decoded), data);
    }
}

#[test]
fn a_truncated_blob_is_an_error() {
    let sections = column(3);
    let data = write_column_the_way_the_server_does(&sections);
    for cut in [1, 8, 100, data.len() - 1] {
        let truncated = &data[..data.len() - cut];
        let chunk = ChunkData {
            data: truncated,
            ..Default::default()
        };
        assert!(
            chunk.sections(3).is_err(),
            "{cut} bytes short decoded anyway"
        );
    }
}

#[test]
fn an_over_long_blob_is_an_error() {
    let sections = column(3);
    let mut data = write_column_the_way_the_server_does(&sections);
    data.push(0);
    let chunk = ChunkData {
        data: &data,
        ..Default::default()
    };
    assert!(chunk.sections(3).is_err());
    assert!(
        chunk.sections(2).is_err(),
        "a short section count leaves the tail unread"
    );
}

#[test]
fn light_rows_come_back_split_by_mask() {
    use mcrs_minecraft_protocol::chunk::{LightChunk, LightData};

    let data = LightData {
        sky_light_mask: Cow::Owned(vec![0b0100]),
        empty_sky_light_mask: Cow::Owned(vec![0b0001]),
        block_light_mask: Cow::Borrowed(&[]),
        empty_block_light_mask: Cow::Owned(vec![0b1000]),
        sky_light_arrays: Cow::Owned(vec![LightChunk([0x21; 2048])]),
        block_light_arrays: Cow::Borrowed(&[]),
    };

    let light = unpack_light_data(&data, 4).expect("unpack");
    assert_eq!(
        light.sky,
        vec![
            RowLight::Empty,
            RowLight::Unchanged,
            RowLight::Filled(LightChunk([0x21; 2048])),
            RowLight::Unchanged,
        ]
    );
    assert_eq!(
        light.block,
        vec![
            RowLight::Unchanged,
            RowLight::Unchanged,
            RowLight::Unchanged,
            RowLight::Empty,
        ]
    );

    assert!(
        unpack_light_data(&data, 3).is_err(),
        "a mask bit past the column must not be ignored"
    );

    let missing_payload = LightData {
        sky_light_arrays: Cow::Borrowed(&[]),
        ..data.clone()
    };
    assert!(unpack_light_data(&missing_payload, 4).is_err());

    let overlapping = LightData {
        empty_sky_light_mask: Cow::Owned(vec![0b0101]),
        ..data
    };
    assert!(unpack_light_data(&overlapping, 4).is_err());
}
