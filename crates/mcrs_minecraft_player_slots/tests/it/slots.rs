use mcrs_minecraft_inventory_plan::Filter;
use mcrs_minecraft_player_slots::ArmourLock;
use mcrs_minecraft_player_slots::ArmourSlot;
use mcrs_minecraft_player_slots::EquipSlot;
use mcrs_minecraft_player_slots::Holder;
use mcrs_minecraft_player_slots::OffHandRule;
use mcrs_minecraft_player_slots::OffHandTag;
use mcrs_minecraft_player_slots::Offer;
use mcrs_minecraft_player_slots::PlayerMode;
use mcrs_minecraft_player_slots::SlotFilter;
use mcrs_minecraft_player_slots::SlotRule;
use mcrs_minecraft_player_slots::swap_button_slot;

fn rule(limit: u8, filter: SlotFilter) -> Option<SlotRule> {
    Some(SlotRule { limit, filter })
}

#[test]
fn inv_slt_001_002_the_holders_their_slots_and_slot_limits() {
    let any = rule(99, SlotFilter::Any);
    let cases = [
        (Holder::Inventory, 0, any),
        (Holder::Inventory, 8, any),
        (Holder::Inventory, 35, any),
        (
            Holder::Inventory,
            36,
            rule(1, SlotFilter::Armour(ArmourSlot::Feet)),
        ),
        (
            Holder::Inventory,
            37,
            rule(1, SlotFilter::Armour(ArmourSlot::Legs)),
        ),
        (
            Holder::Inventory,
            38,
            rule(1, SlotFilter::Armour(ArmourSlot::Chest)),
        ),
        (
            Holder::Inventory,
            39,
            rule(1, SlotFilter::Armour(ArmourSlot::Head)),
        ),
        (Holder::Inventory, 40, rule(99, SlotFilter::OffHand)),
        (Holder::Inventory, 41, None),
        (Holder::Cursor, 0, any),
        (Holder::Cursor, 1, None),
        (Holder::CraftGrid, 3, any),
        (Holder::CraftGrid, 4, None),
        (Holder::CraftResult, 0, rule(99, SlotFilter::CraftResult)),
        (Holder::CraftResult, 1, None),
    ];
    for (holder, index, expected) in cases {
        assert_eq!(holder.slot(index), expected, "{holder:?} slot {index}");
    }
}

#[test]
fn inv_slt_003_005_006_the_place_filters() {
    let plain = Offer::default();
    let helmet = Offer {
        wear: Some(EquipSlot::Armour(ArmourSlot::Head)),
        ..Offer::default()
    };
    let tagged = Offer {
        off_hand_tag: OffHandTag::Member,
        ..Offer::default()
    };
    let head = SlotFilter::Armour(ArmourSlot::Head);
    let chest = SlotFilter::Armour(ArmourSlot::Chest);
    let cases = [
        (head, helmet, OffHandRule::AnyItem, Filter::Permits),
        (chest, helmet, OffHandRule::AnyItem, Filter::Refuses),
        (head, plain, OffHandRule::AnyItem, Filter::Refuses),
        (
            SlotFilter::OffHand,
            plain,
            OffHandRule::default(),
            Filter::Permits,
        ),
        (
            SlotFilter::OffHand,
            plain,
            OffHandRule::TaggedItems,
            Filter::Refuses,
        ),
        (
            SlotFilter::OffHand,
            tagged,
            OffHandRule::TaggedItems,
            Filter::Permits,
        ),
        (
            SlotFilter::CraftResult,
            plain,
            OffHandRule::AnyItem,
            Filter::Refuses,
        ),
        (
            SlotFilter::Any,
            helmet,
            OffHandRule::TaggedItems,
            Filter::Permits,
        ),
    ];
    for (filter, offer, off_hand, expected) in cases {
        assert_eq!(
            filter.place(offer, off_hand),
            expected,
            "{filter:?} {offer:?} {off_hand:?}"
        );
    }
}

#[test]
fn inv_slt_004_005_the_take_filters() {
    let head = SlotFilter::Armour(ArmourSlot::Head);
    let cases = [
        (
            head,
            ArmourLock::Locked,
            PlayerMode::NotCreative,
            Filter::Refuses,
        ),
        (
            head,
            ArmourLock::Locked,
            PlayerMode::Creative,
            Filter::Permits,
        ),
        (
            head,
            ArmourLock::Free,
            PlayerMode::NotCreative,
            Filter::Permits,
        ),
        (
            SlotFilter::OffHand,
            ArmourLock::Locked,
            PlayerMode::NotCreative,
            Filter::Permits,
        ),
    ];
    for (filter, lock, mode, expected) in cases {
        assert_eq!(
            filter.take(lock, mode),
            expected,
            "{filter:?} {lock:?} {mode:?}"
        );
    }
}

#[test]
fn inv_mnu_003_swap_buttons_name_the_hotbar_and_the_off_hand() {
    let cases = [
        (0, Some(0)),
        (8, Some(8)),
        (9, None),
        (39, None),
        (40, Some(40)),
        (41, None),
    ];
    for (button, slot) in cases {
        assert_eq!(swap_button_slot(button), slot, "button {button}");
    }
}
