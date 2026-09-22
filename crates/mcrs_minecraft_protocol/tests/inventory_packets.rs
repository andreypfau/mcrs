//! Bytes written by the vanilla 26.3-snapshot-10 packet stream codecs; `id`
//! lines are the registry ids the capture session had.

#[allow(dead_code)]
mod common;

use std::collections::HashMap;

use mcrs_minecraft_core::codec::Bounded as Range;
use mcrs_minecraft_core::{BlockPos, ResourceKey, ResourceLocation};
use mcrs_minecraft_protocol::item::{
    ComponentMap, ComponentPatch, ContainerInput, CustomName, Damage, HashedStack, ItemCost,
    MaxStackSize, MerchantOffer, QuickCraftButton, QuickCraftKind, QuickCraftStage,
    RawDelimitedStack, RawMerchantOffer, RawStack, Unbreakable,
};
use mcrs_minecraft_protocol::packets::game::clientbound::*;
use mcrs_minecraft_protocol::packets::game::serverbound::*;
use mcrs_minecraft_protocol::text::Text;
use mcrs_minecraft_protocol::{Bounded, Decode, Encode, ProtoStack, VarInt};
use mcrs_minecraft_registry::{ItemId, RegistryLookup};

const GOLDEN: &str = include_str!("fixtures/inventory_packets_26_3_snapshot_10.txt");

struct Fixture {
    ids: HashMap<(String, String), u32>,
    names: HashMap<(String, u32), ResourceLocation>,
    packets: HashMap<String, Vec<u8>>,
}

impl RegistryLookup for Fixture {
    fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u32> {
        self.ids.get(&(registry.into(), name.to_string())).copied()
    }

    fn name(&self, registry: &str, id: u32) -> Option<&ResourceLocation> {
        self.names.get(&(registry.into(), id))
    }
}

fn fixture() -> Fixture {
    let mut fixture = Fixture {
        ids: HashMap::new(),
        names: HashMap::new(),
        packets: HashMap::new(),
    };
    for line in GOLDEN.lines() {
        if let Some(rest) = line.strip_prefix("id ") {
            let [registry, name, id] = rest.split(' ').collect::<Vec<_>>()[..] else {
                panic!("malformed id line: {line}");
            };
            let id: u32 = id.parse().unwrap();
            fixture.ids.insert((registry.into(), name.into()), id);
            fixture.names.insert(
                (registry.into(), id),
                ResourceLocation::parse(name).unwrap(),
            );
        } else if let Some((name, hex)) = line.split_once(' ') {
            fixture.packets.insert(name.into(), common::hex(hex));
        }
    }
    fixture
}

fn item(fixture: &Fixture, path: &str) -> ItemId {
    ItemId(
        fixture
            .id("item", &ResourceLocation::minecraft(path))
            .unwrap() as u16,
    )
}

fn decode<'a, P: Decode<'a>>(fixture: &'a Fixture, name: &str) -> (P, &'a [u8]) {
    let bytes = &fixture.packets[name][..];
    let mut r = bytes;
    let packet = P::decode(&mut r).unwrap_or_else(|e| panic!("{name}: {e:#}"));
    assert!(r.is_empty(), "{name}: {} trailing bytes", r.len());
    (packet, bytes)
}

fn encoded<P: Encode>(packet: &P) -> Vec<u8> {
    let mut out = Vec::new();
    packet.encode(&mut out).unwrap();
    out
}

fn check<'a, P: Encode + Decode<'a> + PartialEq + std::fmt::Debug>(
    fixture: &'a Fixture,
    name: &str,
    expected: P,
) {
    let (packet, bytes) = decode::<P>(fixture, name);
    assert_eq!(packet, expected, "{name}");
    assert_eq!(encoded(&expected), bytes, "{name}");
}

fn sword(fixture: &Fixture) -> ProtoStack {
    let mut patch = ComponentPatch::EMPTY;
    patch.set(Damage(Range(7)));
    patch.set(CustomName(Text::text("named")));
    patch.set(Unbreakable);
    ProtoStack::new(item(fixture, "diamond_sword"), 1, patch)
}

fn raw(fixture: &Fixture, slot: ProtoStack) -> RawStack {
    RawStack::from_stack(&slot, fixture).unwrap()
}

fn plain(f: &Fixture, path: &str, n: i32) -> RawStack {
    raw(f, ProtoStack::new(item(f, path), n, ComponentPatch::EMPTY))
}

#[test]
fn clientbound_container_packets() {
    let f = &fixture();
    check(
        f,
        "container_set_slot",
        ClientboundContainerSetSlot {
            container_id: VarInt(3),
            state_seqno: VarInt(7),
            slot: 5,
            item: raw(f, sword(f)),
        },
    );
    check(
        f,
        "container_set_slot_empty",
        ClientboundContainerSetSlot {
            container_id: VarInt(0),
            state_seqno: VarInt(1),
            slot: -1,
            item: RawStack::EMPTY,
        },
    );
    check(
        f,
        "set_cursor_item",
        ClientboundSetCursorItem {
            contents: plain(f, "stone", 64),
        },
    );
    check(
        f,
        "set_player_inventory",
        ClientboundSetPlayerInventory {
            slot: VarInt(36),
            contents: plain(f, "apple", 3),
        },
    );
    check(
        f,
        "open_screen",
        ClientboundOpenScreen {
            container_id: VarInt(1),
            menu_type: VarInt(
                f.id("menu", &ResourceLocation::minecraft("generic_9x3"))
                    .unwrap() as i32,
            ),
            title: Text::text("Chest"),
        },
    );
    check(
        f,
        "container_close",
        ClientboundContainerClose {
            container_id: VarInt(2),
        },
    );
    check(
        f,
        "container_set_data",
        ClientboundContainerSetData {
            container_id: VarInt(1),
            id: 2,
            value: 300,
        },
    );
    check(
        f,
        "set_held_slot",
        ClientboundSetHeldSlot { slot: VarInt(4) },
    );
    check(
        f,
        "take_item_entity",
        ClientboundTakeItemEntity {
            item_id: VarInt(10),
            player_id: VarInt(20),
            amount: VarInt(3),
        },
    );
}

fn key(path: &str) -> ResourceKey<mcrs_minecraft_protocol::item::ItemReg> {
    ResourceKey::from_location(ResourceLocation::minecraft(path))
}

fn offers(f: &Fixture) -> Vec<MerchantOffer> {
    let mut sell_patch = ComponentPatch::EMPTY;
    sell_patch.set(Damage(Range(3)));
    vec![
        MerchantOffer {
            cost_a: ItemCost {
                item: key("emerald"),
                count: 3,
                components: ComponentMap::default(),
            },
            result: ProtoStack::new(item(f, "apple"), 2, ComponentPatch::EMPTY),
            cost_b: None,
            uses: 1,
            max_uses: 12,
            xp: 5,
            special_price_diff: -1,
            price_multiplier: 0.05,
            demand: 2,
        },
        MerchantOffer {
            cost_a: ItemCost {
                item: key("diamond"),
                count: 1,
                components: ComponentMap(vec![
                    MaxStackSize(Range(16)).into(),
                    CustomName(Text::text("x")).into(),
                ]),
            },
            result: ProtoStack::new(item(f, "diamond_sword"), 1, sell_patch),
            cost_b: Some(ItemCost {
                item: key("stone"),
                count: 4,
                components: ComponentMap::default(),
            }),
            uses: 4,
            max_uses: 4,
            xp: 0,
            special_price_diff: 0,
            price_multiplier: 0.2,
            demand: 0,
        },
    ]
}

#[test]
fn clientbound_container_set_content() {
    let f = &fixture();
    let mut player = vec![RawStack::EMPTY; 46];
    player[9] = plain(f, "apple", 3);
    player[36] = raw(f, sword(f));
    player[45] = plain(f, "stone", 16);
    check(
        f,
        "container_set_content",
        ClientboundContainerSetContent {
            container_id: VarInt(0),
            state_seqno: VarInt(5),
            slot_data: player,
            carried_item: plain(f, "stone", 64),
        },
    );
    let mut chest = vec![RawStack::EMPTY; 63];
    chest[0] = plain(f, "diamond", 5);
    chest[26] = plain(f, "emerald", 1);
    chest[27] = plain(f, "apple", 2);
    chest[62] = plain(f, "stone", 1);
    check(
        f,
        "container_set_content_chest",
        ClientboundContainerSetContent {
            container_id: VarInt(1),
            state_seqno: VarInt(2),
            slot_data: chest,
            carried_item: RawStack::EMPTY,
        },
    );
}

#[test]
fn merchant_offers() {
    let f = &fixture();
    let offers = offers(f);
    check(
        f,
        "merchant_offers",
        ClientboundMerchantOffers {
            container_id: VarInt(5),
            offers: offers
                .iter()
                .map(|offer| RawMerchantOffer::from_offer(offer, f).unwrap())
                .collect(),
            villager_level: VarInt(2),
            villager_xp: VarInt(15),
            show_progress: true,
            can_restock: false,
        },
    );
    let (packet, _) = decode::<ClientboundMerchantOffers>(f, "merchant_offers");
    let resolved: Vec<MerchantOffer> = packet
        .offers
        .iter()
        .map(|raw| raw.resolve(f).unwrap())
        .collect();
    assert_eq!(resolved, offers);
    assert!(!resolved[0].is_out_of_stock());
    assert!(resolved[1].is_out_of_stock());
    check(
        f,
        "merchant_offers_empty",
        ClientboundMerchantOffers {
            container_id: VarInt(1),
            offers: vec![],
            villager_level: VarInt(1),
            villager_xp: VarInt(0),
            show_progress: false,
            can_restock: true,
        },
    );
}

#[test]
fn an_offer_never_sells_an_empty_stack() {
    let f = &fixture();
    let mut offer = offers(f).remove(0);
    offer.result = ProtoStack::EMPTY;
    let error = RawMerchantOffer::from_offer(&offer, f).unwrap_err();
    assert!(error.to_string().contains("Empty ItemStack not allowed"));
    let mut bytes = f.packets["merchant_offers"].clone();
    let result_count = 6;
    assert_eq!(bytes[result_count], 2);
    bytes[result_count] = 0;
    let mut r = &bytes[..];
    let error = ClientboundMerchantOffers::decode(&mut r).unwrap_err();
    assert!(format!("{error:#}").contains("Empty ItemStack not allowed"));
}

#[test]
fn serverbound_container_packets() {
    let f = &fixture();
    check(
        f,
        "set_creative_mode_slot",
        ServerboundSetCreativeModeSlot {
            slot: 36,
            item: RawDelimitedStack::from_stack(&sword(f), f).unwrap(),
        },
    );
    let (packet, _) = decode::<ServerboundSetCreativeModeSlot>(f, "set_creative_mode_slot");
    assert_eq!(packet.item.resolve(f).unwrap(), sword(f));
    check(
        f,
        "set_creative_mode_slot_empty",
        ServerboundSetCreativeModeSlot {
            slot: -1,
            item: RawDelimitedStack::EMPTY,
        },
    );
    check(
        f,
        "sb_container_close",
        ServerboundContainerClose {
            container_id: VarInt(3),
        },
    );
    check(
        f,
        "container_button_click",
        ServerboundContainerButtonClick {
            container_id: VarInt(1),
            button_id: VarInt(2),
        },
    );
    check(
        f,
        "container_slot_state_changed",
        ServerboundContainerSlotStateChanged {
            slot_id: VarInt(5),
            container_id: VarInt(1),
            new_state: true,
        },
    );
    check(
        f,
        "rename_item",
        ServerboundRenameItem { name: "Excalibur" },
    );
    check(
        f,
        "select_trade",
        ServerboundSelectTrade { item: VarInt(1) },
    );
    check(
        f,
        "edit_book",
        ServerboundEditBook {
            slot: VarInt(0),
            pages: Bounded(vec![Bounded("page one"), Bounded("page two")]),
            title: Some(Bounded("Title")),
        },
    );
    check(
        f,
        "edit_book_untitled",
        ServerboundEditBook {
            slot: VarInt(1),
            pages: Bounded(vec![]),
            title: None,
        },
    );
    check(
        f,
        "pick_item_from_block",
        ServerboundPickItemFromBlock {
            pos: BlockPos::new(1, -2, 3),
            include_data: true,
        },
    );
    check(
        f,
        "pick_item_from_entity",
        ServerboundPickItemFromEntity {
            id: VarInt(42),
            include_data: false,
        },
    );
}

/// Vanilla hashes the patch into a hash map, so the entry order on the wire
/// is not the insertion order; the decoded map is compared by `matches`.
#[test]
fn container_click_carries_hashed_slots() {
    let f = &fixture();
    let (packet, bytes) = decode::<ServerboundContainerClick>(f, "container_click");
    assert_eq!(encoded(&packet), bytes);
    assert_eq!(packet.container_id, VarInt(1));
    assert_eq!(packet.state_seqno, VarInt(2));
    assert_eq!(packet.slot_index, 3);
    assert_eq!(packet.button, 0);
    assert_eq!(packet.container_input, ContainerInput::Pickup);
    let [(4, None), (3, Some(changed))] = &packet.changed_slots.0[..] else {
        panic!("{:?}", packet.changed_slots);
    };
    assert!(changed.matches(&sword(f)));
    assert_eq!(
        packet.carried_item,
        HashedStack::create(&ProtoStack::new(item(f, "apple"), 1, ComponentPatch::EMPTY)).unwrap()
    );
}

#[test]
fn container_click_bounds_the_changed_slots() {
    let f = &fixture();
    let (packet, _) = decode::<ServerboundContainerClick>(f, "container_click");
    let mut oversized = packet.clone();
    oversized.changed_slots = Bounded(vec![(0, None); MAX_CHANGED_SLOTS + 1]);
    assert!(oversized.encode(&mut Vec::new()).is_err());
    let mut bytes = Vec::new();
    packet.container_id.encode(&mut bytes).unwrap();
    packet.state_seqno.encode(&mut bytes).unwrap();
    packet.slot_index.encode(&mut bytes).unwrap();
    packet.button.encode(&mut bytes).unwrap();
    packet.container_input.encode(&mut bytes).unwrap();
    VarInt(MAX_CHANGED_SLOTS as i32 + 1)
        .encode(&mut bytes)
        .unwrap();
    for _ in 0..=MAX_CHANGED_SLOTS {
        (0u16, None::<HashedStack>).encode(&mut bytes).unwrap();
    }
    packet.carried_item.encode(&mut bytes).unwrap();
    assert!(ServerboundContainerClick::decode(&mut &bytes[..]).is_err());
}

#[test]
fn edit_book_bounds_pages_and_title() {
    let mut out = Vec::new();
    let long = "x".repeat(MAX_BOOK_PAGE_CHARS + 1);
    let packet = ServerboundEditBook {
        slot: VarInt(0),
        pages: Bounded(vec![Bounded(long.as_str())]),
        title: None,
    };
    assert!(packet.encode(&mut out).is_err());
    let packet = ServerboundEditBook {
        slot: VarInt(0),
        pages: Bounded(vec![Bounded("ok"); MAX_BOOK_PAGES + 1]),
        title: None,
    };
    assert!(packet.encode(&mut out).is_err());
    let title = "t".repeat(MAX_BOOK_TITLE_CHARS + 1);
    let packet = ServerboundEditBook {
        slot: VarInt(0),
        pages: Bounded(vec![]),
        title: Some(Bounded(title.as_str())),
    };
    assert!(packet.encode(&mut out).is_err());
}

#[test]
fn quick_craft_button_round_trips_every_valid_mask() {
    let valid = [
        (0, QuickCraftKind::Split, QuickCraftStage::Header),
        (1, QuickCraftKind::Split, QuickCraftStage::Slot),
        (2, QuickCraftKind::Split, QuickCraftStage::End),
        (4, QuickCraftKind::Single, QuickCraftStage::Header),
        (5, QuickCraftKind::Single, QuickCraftStage::Slot),
        (6, QuickCraftKind::Single, QuickCraftStage::End),
        (8, QuickCraftKind::Full, QuickCraftStage::Header),
        (9, QuickCraftKind::Full, QuickCraftStage::Slot),
        (10, QuickCraftKind::Full, QuickCraftStage::End),
    ];
    for (byte, kind, stage) in valid {
        let button = QuickCraftButton::try_from(byte).unwrap();
        assert_eq!(button.kind, kind, "{byte}");
        assert_eq!(button.stage, stage, "{byte}");
        assert_eq!(u8::from(button), byte);
    }

    for byte in [3, 7, 12, 15] {
        assert_eq!(QuickCraftButton::try_from(byte), Err(byte));
    }

    let button = QuickCraftButton::try_from(0b0001_0110).unwrap();
    assert_eq!(button.kind, QuickCraftKind::Single);
    assert_eq!(button.stage, QuickCraftStage::End);
}
