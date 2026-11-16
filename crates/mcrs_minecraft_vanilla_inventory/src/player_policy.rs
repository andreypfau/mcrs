use mcrs_minecraft_engine_item::Gate;
use mcrs_minecraft_engine_item::SlotAccess;
use mcrs_minecraft_engine_item::SlotPolicy;
use mcrs_minecraft_inventory_player::Holder;
use mcrs_minecraft_inventory_player::SlotFilter;
use mcrs_minecraft_inventory_player::SlotRule;

/// The slot limits and the gates of `holder` by its slot rules, with `off_hand` as the place gate
/// of the off-hand slot.
pub(crate) fn player_policy(holder: Holder, off_hand: Gate) -> SlotPolicy {
    (0..holder.slot_count())
        .filter_map(|index| holder.slot(index))
        .map(|rule| slot_access(rule, off_hand))
        .collect()
}

// chisle: no stack has the `minecraft:equippable` component or an enchantment effect yet, so the
// armour slots refuse each place and permit each take; armour gates over these components lift it.
fn slot_access(rule: SlotRule, off_hand: Gate) -> SlotAccess {
    let place = match rule.filter {
        SlotFilter::Any => Gate::Permits,
        SlotFilter::OffHand => off_hand,
        SlotFilter::Armour(_) | SlotFilter::CraftResult => Gate::Refuses,
    };
    SlotAccess {
        limit: rule.limit,
        take: Gate::Permits,
        place,
    }
}
