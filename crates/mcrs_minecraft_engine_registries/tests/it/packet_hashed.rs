use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_engine_registries::read_packet_hashed;
use mcrs_minecraft_engine_registries::test_item_table;
use mcrs_minecraft_engine_registries::write_packet_hashed;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::HashScratch;
use mcrs_minecraft_item_stack::HashedStack;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::Rarity;
use mcrs_minecraft_item_stack::hash_stack;
use mcrs_minecraft_resource::ResourceLocation;

fn item(names: &RegistryNames<'_>, name: &str) -> Kind {
    let name = ResourceLocation::read(name).expect("the item name parses");
    Kind(names.id(NameTable::Item, name).expect("the item is named"))
}

fn round_trip(names: &RegistryNames<'_>, stack: &ItemStack) -> Option<HashedStack> {
    let mut bytes = Vec::new();
    let packet = write_packet_hashed(Some(stack), names, &mut HashScratch::default(), &mut bytes);
    read_packet_hashed(packet.as_ref(), names, &mut bytes).expect("the packet stack reads")
}

#[test]
fn a_hashable_stack_crosses_the_packet_unchanged() {
    let items = test_item_table();
    let names = RegistryNames::new(test_registries(), &items);
    let count = Count::new(3).expect("3 is a count");
    let stack = ItemStack::new(item(&names, "minecraft:stone"), count).set(Rarity::Epic);
    let expected =
        hash_stack(Some(&stack), &names, &mut HashScratch::default()).expect("the stack hashes");
    assert!(
        expected.is_some(),
        "a present stack hashes to a present value"
    );
    assert_eq!(round_trip(&names, &stack), expected);
}

#[test]
fn an_unhashable_stack_becomes_one_air() {
    let items = test_item_table();
    let names = RegistryNames::new(test_registries(), &items);
    let count = Count::new(5).expect("5 is a count");
    let stack = ItemStack::new(Kind(u16::MAX), count);
    let air = HashedStack {
        kind: item(&names, "minecraft:air"),
        count: 1,
        values: Vec::new(),
        removals: Vec::new(),
    };
    assert_eq!(round_trip(&names, &stack), Some(air));
}
