use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use glam::DVec3;
use mcrs_minecraft_block::definition::FluidState;
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_predicate::{LocationPredicate, PredicateScratch};
use mcrs_minecraft_registries::Resolve;
use mcrs_voxel::BlockPos;

use crate::grid::{Grid, definitions, registries, smoke, state};

thread_local! {
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

struct CountingAllocator;

fn count() {
    let _ = ALLOCATIONS.try_with(|allocations| allocations.set(allocations.get() + 1));
}

// SAFETY: each method forwards to `System` with the arguments it received.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: the caller upholds the contract of `GlobalAlloc::alloc`.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: the caller upholds the contract of `GlobalAlloc::alloc_zeroed`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count();
        // SAFETY: the caller upholds the contract of `GlobalAlloc::realloc`.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller upholds the contract of `GlobalAlloc::dealloc`.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn allocations_during<T>(run: impl FnOnce() -> T) -> (T, usize) {
    ALLOCATIONS.with(|allocations| allocations.set(0));
    let value = run();
    (value, ALLOCATIONS.with(Cell::get))
}

#[test]
fn a_match_that_reads_no_block_entity_data_allocates_nothing_after_warm_up() {
    let json = r#"{
        "position": {"y": {"min": 64.0}},
        "block": {"blocks": "minecraft:campfire", "state": {"lit": "true"}},
        "fluid": {"fluids": "minecraft:water"},
        "light": {"light": {"min": 10}}
    }"#;
    let named = serde_json::from_str(json).unwrap();
    let predicate = LocationPredicate::resolve(named, registries())
        .unwrap()
        .bake(registries(), definitions())
        .unwrap();
    let world = Grid {
        blocks: vec![(
            BlockPos::new(0, 64, 0),
            state("minecraft:campfire", &[("lit", "true")]),
        )],
        fluid: Some(FluidState::source_of(Fluid::Water.id())),
        block_light: Some(12),
        sky_light: Some(0),
        sky_light_level: Some(15.0),
        ..Grid::default()
    };
    let at = DVec3::new(0.5, 64.5, 0.5);
    let smoke = smoke();
    let mut scratch = PredicateScratch::default();
    assert!(
        predicate.matches(&world, at, smoke, &mut scratch),
        "the world meets each test"
    );

    let (passes, allocations) =
        allocations_during(|| predicate.matches(&world, at, smoke, &mut scratch));

    assert!(passes, "the world meets each test");
    assert_eq!(allocations, 0, "a match allocated after warm-up");
}
