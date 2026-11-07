use mcrs_minecraft_physics::Birth;
use mcrs_minecraft_physics::BirthKey;
use mcrs_minecraft_physics::BirthSource;
use mcrs_minecraft_physics::birth_uuid;
use mcrs_voxel::BlockPos;

fn from_block(x: i32, y: i32, z: i32) -> BirthKey {
    BirthKey::born(7, &BirthSource::Block(BlockPos::new(x, y, z)), 0)
}

#[test]
fn cmp_013_block_sources_compare_by_y_then_z_then_x_across_signs() {
    let ordered = [
        from_block(100, -64, 100),
        from_block(5, -1, -3),
        from_block(-9, 0, -1),
        from_block(-9, 0, 0),
        from_block(-1, 0, 1),
        from_block(3, 0, 1),
        from_block(-100, 2, -100),
    ];
    for pair in ordered.windows(2) {
        assert!(pair[0] < pair[1], "{:?} before {:?}", pair[0], pair[1]);
    }
}

#[test]
fn cmp_013_children_of_one_parent_keep_their_sequence_and_follow_other_kinds() {
    let parent = from_block(0, 0, 0);
    let first = BirthKey::born(9, &BirthSource::Entity(parent), 0);
    let second = BirthKey::born(9, &BirthSource::Entity(parent), 1);
    let from_player = BirthKey::born(9, &BirthSource::Player { order: 5 }, 0);
    let from_block_later = BirthKey::born(9, &BirthSource::Block(BlockPos::new(0, 300, 0)), 0);
    assert!(first < second);
    assert!(from_player < from_block_later && from_block_later < first);
    assert_eq!(first.source(), parent.digest());
    assert_ne!(
        parent.digest(),
        from_block(1, 0, 0).digest(),
        "the digest reads the full key"
    );
}

#[test]
fn cmp_016_the_phase_offset_covers_zero_to_three_and_is_kept() {
    let mut seen = [false; 4];
    for order in 0..64 {
        let key = BirthKey::born(0, &BirthSource::Player { order }, 0);
        let birth = Birth::from(key);
        assert_eq!(birth.phase, key.phase_offset());
        seen[usize::from(birth.phase)] = true;
    }
    assert_eq!(seen, [true; 4]);
}

#[test]
fn cmp_024_children_with_equal_stored_keys_differ_by_the_uuid_of_their_parent() {
    // Two parents whose digests collide give their children equal stored keys.
    let child = BirthKey::born(9, &BirthSource::Entity(from_block(0, 0, 0)), 0);
    let uuid = birth_uuid(&child, Some(1));
    assert_ne!(uuid, birth_uuid(&child, Some(2)));
    assert_ne!(uuid, birth_uuid(&child, None));
    assert_ne!(
        birth_uuid(&child, None),
        birth_uuid(
            &BirthKey::born(9, &BirthSource::Entity(from_block(0, 0, 0)), 1),
            None
        ),
        "the uuid reads the stored key"
    );
    assert_eq!((uuid >> 76) & 0xF, 4, "version 4");
    assert_eq!((uuid >> 62) & 0x3, 2, "the variant of RFC 9562");
}
