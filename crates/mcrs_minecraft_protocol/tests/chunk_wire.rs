use mcrs_minecraft_protocol::chunk::{ChunkData, ChunkSection, Palette, PalettedContainer};
use mcrs_minecraft_protocol::light_codec::{RowLight, unpack_light_data};
use mcrs_minecraft_protocol::section::{Biomes, Blocks, NetworkSectionKind, PaletteForm};
use mcrs_minecraft_protocol::{BlockStateId, Decode, Encode};
use mcrs_voxel_storage::{SectionKind, pack_from};
use std::borrow::Cow;

fn encoded<T: Encode>(value: &T) -> Vec<u8> {
    let mut buf = Vec::new();
    value.encode(&mut buf).expect("encode");
    buf
}

fn container<K: NetworkSectionKind, V: Copy>(
    ids: &[u32],
    value: impl Fn(u32) -> V,
) -> PalettedContainer<V> {
    let mut palette: Vec<u32> = Vec::new();
    for &id in ids {
        if !palette.contains(&id) {
            palette.push(id);
        }
    }
    match K::network_form(palette.len()) {
        PaletteForm::Single => PalettedContainer {
            bits_per_entry: 0,
            palette: Palette::Single(value(palette[0])),
            packed_data: Box::new([]),
        },
        PaletteForm::Indirect { bits } => PalettedContainer {
            bits_per_entry: bits as u8,
            palette: Palette::Indirect(palette.iter().map(|&id| value(id)).collect()),
            packed_data: pack_from(bits, ids, |id| {
                palette.iter().position(|p| p == id).expect("indexed") as u32
            }),
        },
        PaletteForm::Direct { bits } => PalettedContainer {
            bits_per_entry: bits as u8,
            palette: Palette::Direct,
            packed_data: pack_from(bits, ids, |&id| id),
        },
    }
}

fn blocks(ids: &[u32]) -> PalettedContainer<BlockStateId> {
    container::<Blocks, _>(ids, |id| BlockStateId(id as u16))
}

fn biomes(ids: &[u32]) -> PalettedContainer<u8> {
    container::<Biomes, _>(ids, |id| id as u8)
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

fn round_trip<V>(container: PalettedContainer<V>)
where
    V: Copy + std::fmt::Debug + PartialEq,
    PalettedContainer<V>: Encode + for<'a> Decode<'a>,
{
    let bytes = encoded(&container);
    let mut r = bytes.as_slice();
    let decoded = PalettedContainer::<V>::decode(&mut r).expect("decode container");
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
    assert_eq!(blocks(&block_ids(1)).bits_per_entry, 0);
    assert!(matches!(
        blocks(&block_ids(9)).palette,
        Palette::Indirect(_)
    ));
    assert_eq!(blocks(&block_ids(400)).bits_per_entry, 15);
    assert!(matches!(blocks(&block_ids(400)).palette, Palette::Direct));
    assert_eq!(biomes(&biome_ids(40)).bits_per_entry, 7);
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
