use mcrs_minecraft_engine_item::Gate;
use mcrs_minecraft_engine_item::SlotAccess;
use mcrs_minecraft_engine_item::SlotPolicy;
use mcrs_minecraft_inventory_player::Holder;
use mcrs_minecraft_inventory_player::SlotFilter;
use mcrs_minecraft_inventory_player::SlotRule;

pub(crate) fn player_policy(holder: Holder, armour_place: Gate) -> SlotPolicy {
    (0..holder.slot_count())
        .filter_map(|index| holder.slot(index))
        .map(|rule| slot_access(rule, armour_place))
        .collect()
}

fn slot_access(rule: SlotRule, armour_place: Gate) -> SlotAccess {
    let place = match rule.filter {
        SlotFilter::Any => Gate::Permits,
        SlotFilter::Armour(_) => armour_place,
        SlotFilter::CraftResult => Gate::Refuses,
    };
    SlotAccess {
        limit: rule.limit,
        take: Gate::Permits,
        place,
    }
}
