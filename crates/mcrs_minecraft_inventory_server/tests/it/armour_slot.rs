use std::borrow::Cow;

use mcrs_minecraft_engine_item::GateQuery;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::SlotPolicy;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_inventory_transaction::Filter;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::EntityTypeId;
use mcrs_minecraft_item_stack::EntityTypeSet;
use mcrs_minecraft_item_stack::EntityTypeTagId;
use mcrs_minecraft_item_stack::Equippable;
use mcrs_minecraft_item_stack::EquippableSlot;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::ResourceLocation;

use crate::click::joined;
use crate::fixture::STONE;
use crate::fixture::app;

const HEAD: u16 = 39;

fn location(text: &str) -> ResourceLocation<'_> {
    ResourceLocation::read(text).expect("a valid identifier")
}

struct EntityTypeTags {
    with_player: EntityTypeTagId,
    without_player: EntityTypeTagId,
}

fn entity_type_tags(names: &RegistryNames<'_>, registries: &CurrentRegistries) -> EntityTypeTags {
    let player = registries
        .id_of(
            location("minecraft:entity_type"),
            location("minecraft:player"),
        )
        .expect("the player has an entity type id");
    let tags: Vec<_> = registries
        .tags_of(location("minecraft:entity_type"))
        .expect("the entity type registry has tags")
        .collect();
    let tag_id = |contains: bool| {
        let (name, _) = tags
            .iter()
            .find(|(_, members)| members.contains(&player) == contains)
            .expect("the data pack has such a tag");
        let id = names.id(NameTable::EntityTypeTag, (*name).into());
        EntityTypeTagId(id.expect("the tag has an id"))
    };
    EntityTypeTags {
        with_player: tag_id(true),
        without_player: tag_id(false),
    }
}

#[test]
fn inv_slt_003_the_head_slot_takes_a_stack_whose_equippable_admits_the_player() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);
    let late = world
        .remove_resource::<CurrentRegistries>()
        .expect("the fixture has registries");
    world.insert_resource(late);
    let registries = world.resource::<CurrentRegistries>();
    let names = RegistryNames::new(registries, world.resource::<ItemTable>());
    let tags = entity_type_tags(&names, registries);
    let entity_type = |name: &str| {
        let id = names.id(NameTable::EntityType, location(name));
        EntityTypeId(id.expect("a known entity type"))
    };
    let list = |entity_types: Vec<EntityTypeId>| {
        Some(EntityTypeSet::EntityTypes(Cow::Owned(entity_types)))
    };
    let cases = [
        (
            "no allowed entities",
            Some(EquippableSlot::Head),
            None,
            Filter::Permits,
        ),
        (
            "a list with the player",
            Some(EquippableSlot::Head),
            list(vec![
                entity_type("minecraft:zombie"),
                entity_type("minecraft:player"),
            ]),
            Filter::Permits,
        ),
        (
            "a list without the player",
            Some(EquippableSlot::Head),
            list(vec![entity_type("minecraft:zombie")]),
            Filter::Refuses,
        ),
        (
            "an empty list",
            Some(EquippableSlot::Head),
            list(vec![]),
            Filter::Refuses,
        ),
        (
            "a tag with the player",
            Some(EquippableSlot::Head),
            Some(EntityTypeSet::Tag(tags.with_player)),
            Filter::Permits,
        ),
        (
            "a tag without the player",
            Some(EquippableSlot::Head),
            Some(EntityTypeSet::Tag(tags.without_player)),
            Filter::Refuses,
        ),
        (
            "the chest slot",
            Some(EquippableSlot::Chest),
            None,
            Filter::Refuses,
        ),
        ("no equippable", None, None, Filter::Refuses),
    ];
    let place = world
        .get::<SlotPolicy>(player)
        .expect("a player has a slot policy")
        .access(HEAD)
        .place;
    for (name, slot, allowed_entities, expected) in cases {
        let mut stack = ItemStack::new(STONE, Count::new(1).expect("a count above 0"));
        if let Some(slot) = slot {
            let mut equippable = Equippable::new(slot);
            equippable.allowed_entities = allowed_entities;
            stack = stack.set(equippable);
        }
        let stack = world.commands().spawn_stack(stack).id();
        world.flush();
        let query = GateQuery {
            slot: SlotRef {
                holder: player,
                index: HEAD,
            },
            actor: player,
            stack,
        };
        assert_eq!(place.answer(world, &query), expected, "{name}");
        world.commands().entity(stack).despawn();
        world.flush();
    }
}
