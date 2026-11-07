use crate::BirthKey;
use crate::BirthSource;

const FNV_OFFSET: u128 = 0x6C62_272E_07BB_0142_62B8_2175_6295_C58D;
const FNV_PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013B;
const KEY: u64 = 0x5BE0_CD19_137E_2179;

/// The UUID that an entity gets at its birth, from the stored key of its parent (or its source),
/// the birth tick and the sequence. It breaks a tie of two stored keys, so it uses FNV-1a over
/// 128 bits with its own key, a function independent of [`BirthKey::digest`].
#[must_use]
pub fn birth_uuid(tick: u64, source: &BirthSource, sequence: u32) -> u128 {
    let origin = match source {
        BirthSource::Entity(parent) => parent.words(),
        other => BirthKey::new(0, other.kind(), other.packed(), 0).words(),
    };
    let mut hash = FNV_OFFSET;
    for word in [KEY, tick, u64::from(sequence)].into_iter().chain(origin) {
        for byte in word.to_le_bytes() {
            hash = (hash ^ u128::from(byte)).wrapping_mul(FNV_PRIME);
        }
    }
    hash ^= hash >> 67;
    hash = hash.wrapping_mul(FNV_PRIME);
    hash ^= hash >> 59;
    // Version 4 and the variant of RFC 9562, so the client takes it as a random UUID.
    hash & !(0xF << 76) & !(0x3 << 62) | 0x4 << 76 | 0x2 << 62
}
