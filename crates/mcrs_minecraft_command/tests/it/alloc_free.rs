use crate::block::{read as read_block, suggest_block};
use crate::entity::{Mob, world};
use crate::{OP, read};
use mcrs_command::{Suggestion, SuggestionBuilder};
use mcrs_minecraft_command::{BlockPosArgument, EntityArgument, Vec3Argument};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|count| count.set(count.get() + 1));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCATIONS.with(|count| count.set(count.get() + 1));
        unsafe { System.realloc(ptr, layout, size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[test]
fn a_warm_read_and_suggestion_of_the_vanilla_arguments_allocates_nothing() {
    let blocks = [
        "chest{Items:[{id:\"minecraft:stone\",Slot:0b,count:1}],x:-12,a:[I;1,2],b:1.5e3f,c:'\\N{SNOWMAN}\\uD83D\\uDE00',d:uuid(\"00000001-0002-0003-0004-000000000005\"),e:bool(1)}",
        "#logs[axis=y]{a:[B;300]}",
        "stone{a:1",
        "oak_stairs[facing=nor",
        "#logs[axis=",
    ];
    let selectors = [
        "@e[type=#minecraft:skeletons,nbt={Tags:[\"a\"],Health:20f},distance=..5,sort=nearest]",
        "@a[gamemode=!creative,limit=2]",
        "@e[type=!cow,type=pig]",
    ];
    let coordinates = ["~1 ^ 3", "^ ^1 ^-2.5", "1 2 3"];
    let entity_inputs = ["@e[type=!cow,type=armadi", "@a[gamemode=", "@e["];
    assert_eq!(read_block(blocks[0]).0.map(drop), Ok(()));
    assert_eq!(read_block(blocks[1]).0.map_err(|e| e.position), Err(23));
    assert_eq!(read_block(blocks[2]).0.map_err(|e| e.position), Err(9));
    assert!(read(&EntityArgument::ENTITIES, selectors[0], &OP).0.is_ok());
    assert!(read(&EntityArgument::ENTITIES, selectors[1], &OP).0.is_ok());
    assert_eq!(
        read(&EntityArgument::ENTITIES, selectors[2], &OP)
            .0
            .map(drop)
            .map_err(|e| e.position),
        Err(18)
    );
    assert!(read(&Vec3Argument, coordinates[0], &OP).0.is_err());
    assert!(read(&Vec3Argument, coordinates[1], &OP).0.is_ok());
    assert!(read(&BlockPosArgument, coordinates[2], &OP).0.is_ok());

    let world = world();
    let mut found: Vec<&Mob> = Vec::new();
    let mut block_out: Vec<Suggestion<'_, &'static str>> = Vec::new();
    let mut entity_out: Vec<Suggestion<'_, &'static str>> = Vec::new();
    let mut counts = (0, 0, 0);
    let mut run = || {
        counts = (0, 0, 0);
        for input in blocks {
            let (value, cursor) = read_block(input);
            std::hint::black_box((value.is_ok(), cursor));
            block_out.clear();
            suggest_block(input, &mut block_out);
            counts.0 += block_out.len();
        }
        for input in selectors {
            let (selector, cursor) = read(&EntityArgument::ENTITIES, input, &OP);
            std::hint::black_box(cursor);
            if let Ok(selector) = selector {
                let result =
                    selector.select(&OP, world.first(), world.iter(), &mut |_| 0, &mut found);
                assert_eq!(std::hint::black_box(result), Ok(()));
                counts.1 += found.len();
            }
        }
        for input in coordinates {
            let _ = std::hint::black_box(read(&Vec3Argument, input, &OP));
            let _ = std::hint::black_box(read(&BlockPosArgument, input, &OP));
        }
        for input in entity_inputs {
            entity_out.clear();
            EntityArgument::ENTITIES
                .suggest(&OP, &mut SuggestionBuilder::new(input, 0, &mut entity_out));
            counts.2 += entity_out.len();
        }
    };
    run();
    let before = ALLOCATIONS.with(Cell::get);
    run();
    assert_eq!(ALLOCATIONS.with(Cell::get) - before, 0);
    let (block_suggestions, selected, entity_suggestions) = counts;
    assert!(block_suggestions > 0 && selected > 0 && entity_suggestions > 0);
}
