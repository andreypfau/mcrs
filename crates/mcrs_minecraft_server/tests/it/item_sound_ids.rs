use mcrs_minecraft_assets::{RegistryAccess, SyncedRegistry};
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation, rl};
use mcrs_minecraft_protocol::item::{
    ComponentPatch, Consumable, Holder, ItemStackValue, ItemUseAnimation, ProtoStack, RawStack,
};
use mcrs_minecraft_registry::{ChainLookup, RegistryLookup};
use mcrs_minecraft_server::world::item::item_lookups;
use mcrs_minecraft_sound::keys::sound_event;

use crate::support::{registry_set, standalone_corpus};

fn drink() -> ResourceLocation {
    rl!("minecraft:entity.generic.drink").to_arc()
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
    access.register(SyncedRegistry::from_names(
        "minecraft:sound_event",
        names
            .iter()
            .map(|name| ResourceLocation::minecraft(name).unwrap()),
    ));
    access
}

fn drinkable() -> ItemStackValue {
    let mut components = ComponentPatch::EMPTY;
    components.set(Consumable {
        consume_seconds: 1.6,
        animation: ItemUseAnimation::Drink,
        sound: Holder::Reference(sound_event::ENTITY_GENERIC_DRINK.id()),
        has_consume_particles: true,
        on_consume_effects: Vec::new(),
    });
    ItemStackValue {
        item: ResourceKey::from_location(rl!("minecraft:potion").to_arc()),
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
    assert_eq!(
        sound,
        &Holder::Reference(sound_event::ENTITY_GENERIC_DRINK.id())
    );
}
