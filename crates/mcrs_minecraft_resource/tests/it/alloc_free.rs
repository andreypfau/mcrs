use mcrs_minecraft_resource::ResourceLocation;
use mcrs_test_alloc::{CountingAllocator, allocations};

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

#[test]
fn reading_a_borrowed_location_allocates_nothing() {
    for text in ["stone", "minecraft:stone", "mcrs:block/alt", ":stone"] {
        let before = allocations();
        let location = ResourceLocation::read(text).unwrap();
        assert_eq!(allocations() - before, 0, "reading {text} allocates");
        assert!(!location.path().is_empty());
    }
}
