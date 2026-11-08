use mcrs_minecraft_brain::OrderKey;
use mcrs_minecraft_physics::BirthKey;

/// The birth key packed into 256 bits in the order of its comparison: the tick (64 bits), the
/// source kind (8), the source value (128) and the sequence (32), so the order of the order keys
/// is the order of the birth keys.
#[must_use]
pub fn order_key(key: &BirthKey) -> OrderKey {
    let tick = u128::from(key.tick());
    let kind = u128::from(key.kind() as u8);
    let source = key.source();
    OrderKey::new(
        tick << 64 | kind << 56 | source >> 72,
        source << 56 | u128::from(key.sequence()),
    )
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_physics::SourceKind;

    use super::*;

    #[test]
    fn the_order_keys_keep_the_order_of_the_birth_keys() {
        let keys = [
            BirthKey::new(3, SourceKind::Load, u128::MAX, u32::MAX),
            BirthKey::new(4, SourceKind::Load, 0, 0),
            BirthKey::new(4, SourceKind::Command, 0, 7),
            BirthKey::new(4, SourceKind::Command, 1 << 100, 0),
            BirthKey::new(4, SourceKind::Command, (1 << 100) + 1, 0),
            BirthKey::new(4, SourceKind::Command, (1 << 100) + 1, 1),
            BirthKey::new(4, SourceKind::Entity, 0, 0),
        ];
        for pair in keys.windows(2) {
            assert!(pair[0] < pair[1], "the birth keys of the case are sorted");
            assert!(
                order_key(&pair[0]) < order_key(&pair[1]),
                "{:?} before {:?}",
                pair[0],
                pair[1]
            );
        }
    }
}
