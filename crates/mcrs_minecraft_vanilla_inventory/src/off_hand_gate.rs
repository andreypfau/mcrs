use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::Gate;
use mcrs_minecraft_engine_item::GateQuery;
use mcrs_minecraft_inventory_player::OffHandRule;
use mcrs_minecraft_inventory_player::OffHandTag;
use mcrs_minecraft_inventory_player::Offer;
use mcrs_minecraft_inventory_player::SlotFilter;
use mcrs_minecraft_inventory_transaction::Filter;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_resource::TagKey;
use mcrs_minecraft_resource::rl;

const OFFHAND_ALLOWED: TagKey<'static, Item> = TagKey::new(rl!("mcrs:offhand_allowed"));

/// The place gate of the off-hand slot of each player of the world (INV-SLT-005).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OffHandGate(pub(crate) Gate);

/// Permits a stack whose item is in the item tag `mcrs:offhand_allowed`. A world without the
/// registries or without the tag refuses.
pub(crate) fn tagged_item_gate(world: &World, query: &GateQuery) -> Filter {
    let off_hand_tag = if in_tag(world, query.stack) == Some(true) {
        OffHandTag::Member
    } else {
        OffHandTag::NotMember
    };
    let offer = Offer {
        off_hand_tag,
        ..Offer::default()
    };
    SlotFilter::OffHand.place(offer, OffHandRule::TaggedItems)
}

fn in_tag(world: &World, stack: Entity) -> Option<bool> {
    let kind = world.get::<Kind>(stack)?;
    let registries = world.get_resource::<CurrentRegistries>()?;
    let items = registries.entries::<Item>()?;
    let tags = registries.tags::<Item>()?;
    let tag = tags.id(OFFHAND_ALLOWED)?;
    let item = Id::checked(kind.0, items.len())?;
    Some(tags.contains(tag, item))
}
