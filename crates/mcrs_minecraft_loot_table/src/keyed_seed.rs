use mcrs_minecraft_loot::loot::LootTable;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::BlockPos;

/// The key of the stream of one evaluation of a block table (LOOT-RND-001).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyedSeed {
    pub world: u64,
    pub table: Id<LootTable>,
    pub origin: BlockPos,
    pub tick: u64,
    /// The value that tells the cause of the evaluation apart from each other cause of the same
    /// table, origin block and tick.
    pub cause: u64,
}

/// The value of a table name in the key, so that the stream does not change when a data pack
/// adds a table and the ids move.
pub(crate) fn name_key(name: &str) -> u64 {
    // FNV-1a.
    name.bytes().fold(0xCBF2_9CE4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01B3)
    })
}

pub(crate) fn stream(seed: KeyedSeed, name_key: u64) -> XoroshiroRandom {
    let key = [
        name_key,
        u64::from(seed.origin.x.cast_unsigned()),
        u64::from(seed.origin.y.cast_unsigned()),
        u64::from(seed.origin.z.cast_unsigned()),
        seed.tick,
        seed.cause,
    ]
    .into_iter()
    .fold(mix(seed.world), |state, word| mix(state ^ word));
    XoroshiroRandom::new(key)
}

fn mix(value: u64) -> u64 {
    // SplitMix64 finaliser.
    let value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}
