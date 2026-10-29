use crate::entity::{Mob, world};
use crate::{OP, read};
use mcrs_command::{Suggestion, SuggestionBuilder};
use mcrs_minecraft_command_entity::EntityArgument;
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
fn a_warm_read_search_and_suggestion_of_a_selector_allocates_nothing() {
    let selectors = [
        "@e[type=#minecraft:skeletons,nbt={Tags:[\"a\"],Health:20f},distance=..5,sort=nearest]",
        "@a[gamemode=!creative,limit=2]",
        "@e[type=!cow,type=pig]",
    ];
    let inputs = ["@e[type=!cow,type=armadi", "@a[gamemode=", "@e["];
    assert!(read(&EntityArgument::ENTITIES, selectors[0], &OP).0.is_ok());
    assert!(read(&EntityArgument::ENTITIES, selectors[1], &OP).0.is_ok());
    assert_eq!(
        read(&EntityArgument::ENTITIES, selectors[2], &OP)
            .0
            .map(drop)
            .map_err(|e| e.position),
        Err(18)
    );

    let world = world();
    let mut found: Vec<&Mob> = Vec::new();
    let mut out: Vec<Suggestion<'_, &'static str>> = Vec::new();
    let mut counts = (0, 0);
    let mut run = || {
        counts = (0, 0);
        for input in selectors {
            let (selector, cursor) = read(&EntityArgument::ENTITIES, input, &OP);
            std::hint::black_box(cursor);
            if let Ok(selector) = selector {
                let result =
                    selector.select(&OP, world.first(), world.iter(), &mut |_| 0, &mut found);
                assert_eq!(std::hint::black_box(result), Ok(()));
                counts.0 += found.len();
            }
        }
        for input in inputs {
            out.clear();
            EntityArgument::ENTITIES.suggest(&OP, &mut SuggestionBuilder::new(input, 0, &mut out));
            counts.1 += out.len();
        }
    };
    run();
    let before = ALLOCATIONS.with(Cell::get);
    run();
    assert_eq!(ALLOCATIONS.with(Cell::get) - before, 0);
    let (selected, suggestions) = counts;
    assert!(selected > 0 && suggestions > 0);
}
