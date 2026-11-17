use mcrs_minecraft_engine_item::Gate;
use mcrs_minecraft_engine_item::SlotAccess;
use mcrs_minecraft_engine_item::SlotPolicy;
use mcrs_minecraft_inventory_player::Holder;
use mcrs_minecraft_inventory_player::SlotFilter;
use mcrs_minecraft_inventory_player::SlotRule;

use crate::armour_gates::ArmourGates;

pub(crate) fn player_policy(holder: Holder, armour: ArmourGates) -> SlotPolicy {
    (0..holder.slot_count())
        .filter_map(|index| holder.slot(index))
        .map(|rule| slot_access(rule, armour))
        .collect()
}

fn slot_access(rule: SlotRule, armour: ArmourGates) -> SlotAccess {
    let (take, place) = match rule.filter {
        SlotFilter::Any => (Gate::Permits, Gate::Permits),
        SlotFilter::Armour(_) => (armour.take, armour.place),
        SlotFilter::CraftResult => (Gate::Permits, Gate::Refuses),
    };
    SlotAccess {
        limit: rule.limit,
        take,
        place,
    }
}
