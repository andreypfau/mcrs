use crate::{OP, read};
use mcrs_minecraft_command_argument::{BlockPosArgument, Vec3Argument};
use mcrs_test_alloc::{CountingAllocator, allocations};

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

#[test]
fn a_warm_read_of_the_coordinates_allocates_nothing() {
    let coordinates = ["~1 ^ 3", "^ ^1 ^-2.5", "1 2 3"];
    assert!(read(&Vec3Argument, coordinates[0], &OP).0.is_err());
    assert!(read(&Vec3Argument, coordinates[1], &OP).0.is_ok());
    assert!(read(&BlockPosArgument, coordinates[2], &OP).0.is_ok());

    let run = |trace: &mut [usize; 12]| {
        for (index, input) in coordinates.into_iter().enumerate() {
            let (vec3, vec3_cursor) = std::hint::black_box(read(&Vec3Argument, input, &OP));
            let (block, block_cursor) = std::hint::black_box(read(&BlockPosArgument, input, &OP));
            trace[index * 4..][..4].copy_from_slice(&[
                usize::from(vec3.is_ok()),
                vec3_cursor,
                usize::from(block.is_ok()),
                block_cursor,
            ]);
        }
    };
    let mut warm = [0; 12];
    let mut measured = [0; 12];
    run(&mut warm);
    let before = allocations();
    run(&mut measured);
    assert_eq!(allocations() - before, 0);
    assert_eq!(
        measured, warm,
        "the measured run does the work of the warm run"
    );
    assert!(
        warm.iter().any(|&value| value > 0),
        "the reads advance the cursor"
    );
}
