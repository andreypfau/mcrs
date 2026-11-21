use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::KeyedStream;
use mcrs_voxel::BlockPos;

const OVERWORLD: &str = "minecraft:overworld";

fn from_block(x: i32, y: i32, z: i32) -> EntityUuid {
    EntityUuid::born(
        7,
        OVERWORLD,
        3,
        &BirthSource::Block(BlockPos::new(x, y, z)),
        0,
    )
}

fn command(spawn: BlockPos, source: BlockPos) -> EntityUuid {
    EntityUuid::born(7, OVERWORLD, 3, &BirthSource::Command { spawn, source }, 0)
}

#[test]
fn cmp_013_a_new_uuid_is_a_random_uuid_of_rfc_9562() {
    for order in 0_u32..64 {
        let uuid = EntityUuid::born(
            order.into(),
            OVERWORLD,
            9,
            &BirthSource::Command {
                spawn: BlockPos::new(order.cast_signed(), 0, 0),
                source: BlockPos::new(0, 0, 0),
            },
            order,
        )
        .as_u128();
        assert_eq!((uuid >> 76) & 0xF, 4, "version 4");
        assert_eq!((uuid >> 62) & 0x3, 2, "the variant of RFC 9562");
    }
}

#[test]
fn cmp_013_each_birth_input_changes_the_new_uuid() {
    let parent = from_block(0, 0, 0);
    let child = EntityUuid::born(7, OVERWORLD, 3, &BirthSource::Entity(parent), 0);
    assert_eq!(
        child,
        EntityUuid::born(7, OVERWORLD, 3, &BirthSource::Entity(parent), 0),
        "the same inputs give the same UUID"
    );
    let others = [
        EntityUuid::born(8, OVERWORLD, 3, &BirthSource::Entity(parent), 0),
        EntityUuid::born(7, OVERWORLD, 4, &BirthSource::Entity(parent), 0),
        EntityUuid::born(
            7,
            OVERWORLD,
            3,
            &BirthSource::Entity(from_block(1, 0, 0)),
            0,
        ),
        EntityUuid::born(7, OVERWORLD, 3, &BirthSource::Entity(parent), 1),
        EntityUuid::born(
            7,
            "minecraft:the_nether",
            3,
            &BirthSource::Entity(parent),
            0,
        ),
        EntityUuid::born(
            7,
            "minecraft:overworld2",
            3,
            &BirthSource::Entity(parent),
            0,
        ),
        command(BlockPos::new(0, 0, 0), BlockPos::new(0, 0, 0)),
        command(BlockPos::new(0, 0, 0), BlockPos::new(1, 0, 0)),
        command(BlockPos::new(1, 0, 0), BlockPos::new(0, 0, 0)),
        from_block(0, 0, 0),
        from_block(0, 1, 0),
        from_block(0, 0, 1),
        from_block(-1, 0, 0),
    ];
    for (index, uuid) in others.iter().enumerate() {
        assert_ne!(*uuid, child, "input {index} changes the UUID");
        for other in &others[index + 1..] {
            assert_ne!(uuid, other, "two sources give two UUIDs");
        }
    }
}

#[test]
fn cmp_016_the_phase_offset_of_new_uuids_covers_zero_to_three() {
    let mut seen = [false; 4];
    for order in 0_u32..64 {
        let uuid = EntityUuid::born(
            0,
            OVERWORLD,
            0,
            &BirthSource::Command {
                spawn: BlockPos::new(order.cast_signed(), 0, 0),
                source: BlockPos::new(0, 0, 0),
            },
            0,
        );
        seen[usize::from(uuid.phase_offset())] = true;
    }
    assert_eq!(seen, [true; 4]);
}

fn bins(values: impl Iterator<Item = f64>) -> [u32; 10] {
    let mut bins = [0; 10];
    for value in values {
        assert!((0.0..1.0).contains(&value), "a unit value: {value}");
        if let Some(bin) = bins.get_mut((value * 10.0) as usize) {
            *bin += 1;
        }
    }
    bins
}

#[test]
fn dev_013_a_keyed_stream_draws_uniform_values_over_uuids_and_over_draws() {
    let over_uuids =
        bins((0..1000).map(|order| KeyedStream::new(&from_block(order, 0, 0), 3).unit(0)));
    let stream = KeyedStream::new(&from_block(0, 0, 0), 3);
    let over_draws = bins((0..1000).map(|draw| stream.unit(draw)));
    for bins in [over_uuids, over_draws] {
        assert!(
            bins.iter().all(|count| (60..=140).contains(count)),
            "a uniform distribution puts about 100 values in each bin: {bins:?}"
        );
    }
}
