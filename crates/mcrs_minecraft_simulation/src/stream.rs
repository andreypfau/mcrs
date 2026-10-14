use crate::scalar::GameTick;
use crate::sequence::SubTick;
use mcrs_minecraft_core::{BlockPos, SectionPos};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StreamKey {
    Tick {
        pos: BlockPos,
        sequence: SubTick,
    },
    Event {
        pos: BlockPos,
        sequence: SubTick,
    },
    Chain {
        pos: BlockPos,
        origin: SubTick,
        step: u32,
    },
    Section {
        section: SectionPos,
    },
    Visit {
        anchor: SectionPos,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StreamSeed(u64);

impl StreamSeed {
    pub fn new(world_seed: i64, dimension: &str) -> Self {
        let mut dimension_random = LegacyRandom::new(world_seed as u64).fork_hash(dimension);
        Self(dimension_random.next_java_long() as u64)
    }

    pub fn stream(&self, now: GameTick, key: StreamKey) -> LegacyRandom {
        let seed = mix(self.0, now.0 as u64);
        let seed = match key {
            StreamKey::Tick { pos, sequence } => {
                let [xz, y] = pos_words(pos);
                mix_all(seed, 1, [xz, y, sequence.get() as u64])
            }
            StreamKey::Event { pos, sequence } => {
                let [xz, y] = pos_words(pos);
                mix_all(seed, 2, [xz, y, sequence.get() as u64])
            }
            StreamKey::Chain { pos, origin, step } => {
                let [xz, y] = pos_words(pos);
                mix_all(seed, 3, [xz, y, origin.get() as u64, u64::from(step)])
            }
            StreamKey::Section { section } => mix_all(seed, 4, section_words(section)),
            StreamKey::Visit { anchor } => mix_all(seed, 5, section_words(anchor)),
        };
        LegacyRandom::new(seed)
    }
}

fn words(x: i32, y: i32, z: i32) -> [u64; 2] {
    [
        (x as u32 as u64).wrapping_shl(32) | u64::from(z as u32),
        y as u32 as u64,
    ]
}

fn pos_words(pos: BlockPos) -> [u64; 2] {
    words(pos.x, pos.y, pos.z)
}

fn section_words(section: SectionPos) -> [u64; 2] {
    words(section.x, section.y, section.z)
}

fn mix_all<const N: usize>(seed: u64, tag: u64, words: [u64; N]) -> u64 {
    words.into_iter().fold(mix(seed, tag), mix)
}

fn mix(seed: u64, word: u64) -> u64 {
    let mut z = (seed ^ word).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ z.wrapping_shr(30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ z.wrapping_shr(27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ z.wrapping_shr(31)
}
