use std::borrow::Cow;
use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use mcrs_minecraft_chunk::{PalettedContainer, SectionKind, VoxelId};
use mcrs_minecraft_protocol::chunk::{LightChunk, encode_container};
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundLevelChunkWithLight, ClientboundMoveEntityPosRot, VecDelta,
};
use mcrs_minecraft_protocol::section::{Biomes, Blocks, biome_direct_bits, block_direct_bits};
use mcrs_minecraft_protocol::{
    ByteAngle, ChunkData, ColumnPos, CompressionThreshold, Encode, LightData, PacketDecoder,
    PacketEncoder, VarInt,
};

const BLOCK_STATE_COUNT: usize = 40_000;
const BIOME_REGISTRY_LEN: usize = 100;
const SECTIONS: usize = 24;
const LIGHT_SECTIONS: usize = SECTIONS + 2;
const BLOCK_LIGHT_SECTIONS: usize = 10;
const MIXED_SECTION_PALETTES: [u32; 8] = [2, 5, 12, 40, 90, 200, 600, 1500];

fn scatter(index: u32, distinct: u32) -> u32 {
    index.wrapping_mul(2_654_435_761).rotate_left(11) % distinct
}

fn blocks(distinct: u32) -> PalettedContainer<VoxelId, { Blocks::SIZE }> {
    let cells: Vec<VoxelId> = (0..Blocks::ENTRY_COUNT as u32)
        .map(|index| VoxelId(scatter(index, distinct) as u16))
        .collect();
    PalettedContainer::from_cells(&cells)
}

fn biomes(distinct: u32) -> PalettedContainer<u8, { Biomes::SIZE }> {
    let cells: Vec<u8> = (0..Biomes::ENTRY_COUNT as u32)
        .map(|index| scatter(index, distinct) as u8)
        .collect();
    PalettedContainer::from_cells(&cells)
}

fn column_blob() -> Vec<u8> {
    let mut data = Vec::new();
    for section in 0..SECTIONS {
        let (block_palette, biome_palette) = match MIXED_SECTION_PALETTES.get(section) {
            Some(&distinct) => (distinct, 1 + section as u32 % 4),
            None => (1, 1),
        };
        (section as u16 * 7).encode(&mut data).unwrap();
        0u16.encode(&mut data).unwrap();
        encode_container(
            &blocks(block_palette),
            block_direct_bits(BLOCK_STATE_COUNT),
            &mut data,
        )
        .unwrap();
        encode_container(
            &biomes(biome_palette),
            biome_direct_bits(BIOME_REGISTRY_LEN),
            &mut data,
        )
        .unwrap();
    }
    data
}

fn light_array(section: usize, sky: bool) -> LightChunk {
    if sky && section >= MIXED_SECTION_PALETTES.len() {
        return LightChunk([0xFF; 2048]);
    }
    LightChunk(std::array::from_fn(|byte| {
        (byte as u8).wrapping_mul(31).wrapping_add(section as u8)
    }))
}

fn light_data() -> LightData<'static> {
    LightData {
        sky_light_mask: Cow::Owned(vec![(1 << LIGHT_SECTIONS) - 1]),
        block_light_mask: Cow::Owned(vec![(1 << BLOCK_LIGHT_SECTIONS) - 1]),
        empty_sky_light_mask: Cow::Owned(vec![0]),
        empty_block_light_mask: Cow::Owned(vec![0]),
        sky_light_arrays: Cow::Owned(
            (0..LIGHT_SECTIONS)
                .map(|section| light_array(section, true))
                .collect(),
        ),
        block_light_arrays: Cow::Owned(
            (0..BLOCK_LIGHT_SECTIONS)
                .map(|section| light_array(section, false))
                .collect(),
        ),
    }
}

fn heightmaps() -> Vec<(VarInt, Cow<'static, [u64]>)> {
    (1..=2)
        .map(|kind| {
            let longs = (0..36u64)
                .map(|long| long.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(kind))
                .collect();
            (VarInt(kind as i32), Cow::Owned(longs))
        })
        .collect()
}

fn encoder_at(threshold: i32) -> PacketEncoder {
    let mut encoder = PacketEncoder::new();
    encoder.set_compression(CompressionThreshold(threshold));
    encoder
}

macro_rules! frame_cells {
    ($group:expr, $name:literal, $packet:expr) => {{
        let packet = $packet;
        for (suffix, threshold) in [("off", -1), ("256", 256)] {
            $group.bench_function(format!("{}_encode_{suffix}", $name), |b| {
                b.iter(|| {
                    let mut encoder = encoder_at(threshold);
                    encoder.append_packet(black_box(&packet)).unwrap();
                    black_box(encoder.take())
                });
            });

            let wire = {
                let mut encoder = encoder_at(threshold);
                encoder.append_packet(&packet).unwrap();
                encoder.take()
            };
            $group.bench_function(format!("{}_decode_{suffix}", $name), |b| {
                b.iter_batched(
                    || wire.clone(),
                    |input| {
                        let mut decoder = PacketDecoder::new();
                        decoder.set_compression(CompressionThreshold(threshold));
                        decoder.queue_bytes(input);
                        black_box(decoder.try_next_packet().unwrap().expect("a whole frame"))
                    },
                    BatchSize::LargeInput,
                );
            });
        }
    }};
}

fn bench_frame(c: &mut Criterion) {
    let blob = column_blob();
    let mut group = c.benchmark_group("frame");

    frame_cells!(
        group,
        "chunk",
        ClientboundLevelChunkWithLight {
            pos: ColumnPos { x: 11, z: -7 },
            chunk_data: ChunkData {
                heightmaps: heightmaps(),
                data: &blob,
                block_entities: Cow::Borrowed(&[]),
            },
            light_data: light_data(),
        }
    );
    frame_cells!(
        group,
        "movement",
        ClientboundMoveEntityPosRot {
            entity_id: VarInt(913),
            delta: VecDelta::Linear([312, -87, 1204]),
            y_rot: ByteAngle(97),
            x_rot: ByteAngle(12),
            on_ground: true,
        }
    );

    group.finish();
}

criterion_group!(benches, bench_frame);
criterion_main!(benches);
