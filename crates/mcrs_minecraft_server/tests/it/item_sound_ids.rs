use mcrs_minecraft_assets::{RegistryAccess, RegistrySnapshotErased};
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_protocol::item::{
    ComponentPatch, Consumable, Holder, ItemStackValue, ItemUseAnimation, ProtoStack, RawStack,
};
use mcrs_minecraft_registry::{ChainLookup, RegistryLookup};
use mcrs_minecraft_server::world::item::item_lookups;

use crate::support::{registry_set, standalone_corpus};

fn drink() -> ResourceLocation {
    ResourceLocation::minecraft("entity.generic.drink")
}

fn misnumbered_sounds() -> RegistryAccess {
    let names = [
        "entity.generic.drink",
        "entity.generic.eat",
        "block.wood.break",
        "block.wood.fall",
        "block.wood.hit",
        "block.wood.place",
        "block.wood.step",
        "block.stone.break",
        "block.stone.fall",
        "block.stone.hit",
        "block.stone.place",
        "block.stone.step",
        "intentionally_empty",
    ];
    let mut access = RegistryAccess::default();
    access.register(RegistrySnapshotErased::from_entries(
        "minecraft:sound_event",
        names
            .iter()
            .map(|name| (ResourceLocation::minecraft(*name).into(), None))
            .collect(),
        None,
    ));
    access
}

fn drinkable() -> ItemStackValue {
    let mut components = ComponentPatch::EMPTY;
    components.set(Consumable {
        consume_seconds: 1.6,
        animation: ItemUseAnimation::Drink,
        sound: Holder::reference(drink()),
        has_consume_particles: true,
        on_consume_effects: Vec::new(),
    });
    ItemStackValue {
        item: ResourceKey::from_location(ResourceLocation::minecraft("potion")),
        count: Bounded(1),
        components,
    }
}

#[test]
fn a_sound_in_a_stack_is_sent_with_its_report_id() {
    let access = misnumbered_sounds();
    let (blocks, _) = standalone_corpus();
    let lookups = item_lookups(registry_set(), &access, &blocks.0);
    let chain = ChainLookup(&lookups);

    let report_id = registry_set()
        .id("sound_event", &drink())
        .expect("the report numbers it");
    assert_eq!(
        access.id("sound_event", &drink()),
        Some(0),
        "the fixture must number the sound differently from the report"
    );
    assert_eq!(chain.id("sound_event", &drink()), Some(report_id));

    let stack = ProtoStack::from_value(&drinkable(), &chain).unwrap();
    let sent = RawStack::from_stack(&stack, &chain).unwrap();
    let received = sent.resolve(registry_set()).unwrap();
    let sound = &received.components.get::<Consumable>().unwrap().sound;
    assert_eq!(sound, &Holder::reference(drink()));
}
