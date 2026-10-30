use crate::block::{read, suggest_block};
use mcrs_command::Suggestion;
use mcrs_test_alloc::{CountingAllocator, allocations};

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

#[test]
fn a_warm_read_and_suggestion_of_a_block_predicate_allocates_nothing() {
    let blocks = [
        "chest{Items:[{id:\"minecraft:stone\",Slot:0b,count:1}],x:-12,a:[I;1,2],b:1.5e3f,c:'\\N{SNOWMAN}\\uD83D\\uDE00',d:uuid(\"00000001-0002-0003-0004-000000000005\"),e:bool(1)}",
        "#logs[axis=y]{a:[B;300]}",
        "stone{a:1",
        "oak_stairs[facing=nor",
        "#logs[axis=",
    ];
    assert_eq!(read(blocks[0]).0.map(drop), Ok(()));
    assert_eq!(read(blocks[1]).0.map_err(|e| e.position), Err(23));
    assert_eq!(read(blocks[2]).0.map_err(|e| e.position), Err(9));

    let mut out: Vec<Suggestion<'_, &'static str>> = Vec::new();
    let mut run = |trace: &mut [usize; 15]| {
        for (index, input) in blocks.into_iter().enumerate() {
            let (value, cursor) = read(input);
            let ok = std::hint::black_box(value.is_ok());
            out.clear();
            suggest_block(input, &mut out);
            trace[index * 3..][..3].copy_from_slice(&[usize::from(ok), cursor, out.len()]);
        }
    };
    let mut warm = [0; 15];
    let mut measured = [0; 15];
    run(&mut warm);
    let before = allocations();
    run(&mut measured);
    assert_eq!(allocations() - before, 0);
    assert_eq!(
        measured, warm,
        "the measured run does the work of the warm run"
    );
    let ok_pattern = [warm[0], warm[3], warm[6], warm[9], warm[12]];
    assert_eq!(
        ok_pattern,
        [1, 0, 0, 0, 0],
        "only the first input is a valid predicate"
    );
    assert!(
        warm.chunks(3).any(|entry| entry[2] > 0),
        "at least one input has suggestions"
    );
}
