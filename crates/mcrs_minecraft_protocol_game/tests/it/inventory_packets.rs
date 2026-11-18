use crate::common::encoded;
use std::collections::HashMap;
use std::sync::LazyLock;

use mcrs_minecraft_container_click::{Click, ClickKind, Clicked};
use mcrs_minecraft_core::codec::Bounded as Range;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_menu_key::menu_keys::MenuType;
use mcrs_minecraft_nbt::{NbtBudget, NbtQuotaExceeded};
use mcrs_minecraft_protocol::{Bounded, Decode, Encode, VarInt};
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_protocol_game::clientbound::*;
use mcrs_minecraft_protocol_game::item::Text as ItemText;
use mcrs_minecraft_protocol_game::item::{
    ComponentMap, ComponentPatch, CustomName, Damage, DelimitedStack, HashedStack, ItemCost,
    MaxStackSize, MerchantOffer, QuickCraftButton, QuickCraftKind, QuickCraftStage, Unbreakable,
};
use mcrs_minecraft_protocol_game::serverbound::*;
use mcrs_minecraft_protocol_game::text::{RawText, Text};
use mcrs_minecraft_resource::Id;
use mcrs_voxel::BlockPos;

use crate::item::harness::{WireCodec, lookup, wire_form};
use mcrs_minecraft_registries::{FromWire, Wire};

static GOLDEN: LazyLock<String> =
    LazyLock::new(|| mcrs_minecraft_fixtures::fixture("inventory_packets").text());

struct Fixture {
    ids: HashMap<(String, String), u16>,
    packets: HashMap<String, Vec<u8>>,
}

fn fixture() -> Fixture {
    let mut fixture = Fixture {
        ids: HashMap::new(),
        packets: HashMap::new(),
    };
    for line in GOLDEN.lines() {
        if let Some(rest) = line.strip_prefix("id ") {
            let [registry, name, id] = rest.split(' ').collect::<Vec<_>>()[..] else {
                panic!("malformed id line: {line}");
            };
            let id: u16 = id.parse().unwrap();
            fixture.ids.insert((registry.into(), name.into()), id);
        } else if let Some((name, hex)) = line.split_once(' ') {
            fixture.packets.insert(name.into(), crate::common::hex(hex));
        }
    }
    fixture
}

fn item(fixture: &Fixture, path: &str) -> Id<Item> {
    let id = Item::read(&format!("minecraft:{path}")).unwrap().id();
    assert_eq!(
        fixture
            .ids
            .get(&("item".into(), format!("minecraft:{path}"))),
        Some(&id.number())
    );
    id
}

fn decode<'a, P: Decode<'a>>(fixture: &'a Fixture, name: &str) -> (P, &'a [u8]) {
    let bytes = &fixture.packets[name][..];
    let mut r = bytes;
    let packet = P::decode(&mut r).unwrap_or_else(|e| panic!("{name}: {e:#}"));
    assert!(r.is_empty(), "{name}: {} trailing bytes", r.len());
    (packet, bytes)
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
    patch.set(CustomName(ItemText::text("named")));
    patch.set(Unbreakable);
    ProtoStack::new(item(fixture, "diamond_sword"), 1, patch)
}

fn plain(f: &Fixture, path: &str, n: i32) -> ProtoStack<Wire> {
    wire_form(&ProtoStack::new(item(f, path), n, ComponentPatch::EMPTY))
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one table of the game's bytes for every container packet, so each packet reads against its capture"
)]
fn container_packets_are_the_games_bytes() {
    let f = &fixture();
    check(
        f,
        "container_set_slot",
        ClientboundContainerSetSlot {
            container_id: VarInt(3),
            state_seqno: VarInt(7),
            slot: 5,
            item: wire_form(&sword(f)),
        },
    );
    check(
        f,
        "container_set_slot_empty",
        ClientboundContainerSetSlot {
            container_id: VarInt(0),
            state_seqno: VarInt(1),
            slot: -1,
            item: ProtoStack::default(),
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
            menu_type: MenuType::Generic9x3,
            title: RawText::from_text(&Text::text("Chest")).unwrap(),
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
    let mut player = vec![ProtoStack::default(); 46];
    player[9] = plain(f, "apple", 3);
    player[36] = wire_form(&sword(f));
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
    let mut chest = vec![ProtoStack::default(); 63];
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
            carried_item: ProtoStack::default(),
        },
    );
    check(
        f,
        "set_creative_mode_slot",
        ServerboundSetCreativeModeSlot {
            slot: 36,
            item: DelimitedStack(sword(f).to_wire(&lookup()).unwrap()),
        },
    );
    let (packet, _) = decode::<ServerboundSetCreativeModeSlot>(f, "set_creative_mode_slot");
    assert_eq!(packet.item.check(&lookup()).unwrap(), sword(f));
    check(
        f,
        "set_creative_mode_slot_empty",
        ServerboundSetCreativeModeSlot {
            slot: -1,
            item: DelimitedStack::default(),
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

fn offers(f: &Fixture) -> Vec<MerchantOffer> {
    let mut sell_patch = ComponentPatch::EMPTY;
    sell_patch.set(Damage(Range(3)));
    vec![
        MerchantOffer {
            cost_a: ItemCost {
                item: item(f, "emerald"),
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
                item: item(f, "diamond"),
                count: 1,
                components: ComponentMap(vec![
                    MaxStackSize(Range(16)).into(),
                    CustomName(ItemText::text("x")).into(),
                ]),
            },
            result: ProtoStack::new(item(f, "diamond_sword"), 1, sell_patch),
            cost_b: Some(ItemCost {
                item: item(f, "stone"),
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
                .map(|offer| offer.to_wire(&lookup()).unwrap())
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
        .into_iter()
        .map(|wire| MerchantOffer::from_wire(wire, &lookup()).unwrap())
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
    let error = offer
        .encode_checked(&lookup(), &mut Vec::new())
        .unwrap_err();
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
fn container_click_carries_hashed_slots() {
    let f = &fixture();
    let (packet, bytes) = decode::<ServerboundContainerClick>(f, "container_click");
    assert_eq!(encoded(&packet), bytes);
    assert_eq!(packet.container_id, VarInt(1));
    assert_eq!(packet.state_seqno, VarInt(2));
    assert_eq!(
        packet.click,
        Click {
            kind: ClickKind::PickUp,
            button: 0,
            clicked: Clicked::Entry(3),
        }
    );
    let [(4, None), (3, Some(changed))] = &packet.changed_slots.0[..] else {
        panic!("{:?}", packet.changed_slots);
    };
    assert!(changed.matches(&sword(f), &lookup()));
    assert_eq!(
        packet.carried_item,
        HashedStack::create(
            &ProtoStack::new(item(f, "apple"), 1, ComponentPatch::EMPTY),
            &lookup()
        )
        .unwrap()
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
    packet.click.encode(&mut bytes).unwrap();
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
fn click_reads_the_slot_the_button_and_the_kind_id() {
    let cases = [
        (3i16, 0, ClickKind::PickUp, Clicked::Entry(3)),
        (-999, 1, ClickKind::QuickMove, Clicked::Outside),
        (-1, 2, ClickKind::Swap, Clicked::Nowhere),
        (-5, 3, ClickKind::Clone, Clicked::Nowhere),
        (0, 4, ClickKind::Throw, Clicked::Entry(0)),
        (-999, 5, ClickKind::Drag, Clicked::Outside),
        (7, 6, ClickKind::PickUpAll, Clicked::Entry(7)),
    ];
    for (slot, id, kind, clicked) in cases {
        let mut bytes = Vec::new();
        slot.encode(&mut bytes).unwrap();
        1u8.encode(&mut bytes).unwrap();
        VarInt(id).encode(&mut bytes).unwrap();
        let click = Click::decode(&mut &bytes[..]).unwrap();
        let expected = Click {
            kind,
            button: 1,
            clicked,
        };
        assert_eq!(click, expected, "slot {slot}, kind id {id}");
    }
    let mut unknown = Vec::new();
    0i16.encode(&mut unknown).unwrap();
    0u8.encode(&mut unknown).unwrap();
    VarInt(7).encode(&mut unknown).unwrap();
    assert!(Click::decode(&mut &unknown[..]).is_err());
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
fn edit_book_refuses_one_page_past_the_limit_on_decode() {
    let book = |pages: usize| {
        let mut bytes = vec![0, u8::try_from(pages).expect("below 128 pages")];
        for _ in 0..pages {
            bytes.extend_from_slice(&[1, b'x']);
        }
        bytes.push(0);
        bytes
    };
    assert!(
        ServerboundEditBook::decode(&mut book(MAX_BOOK_PAGES).as_slice()).is_ok(),
        "a book at the page limit decodes"
    );
    assert!(
        ServerboundEditBook::decode(&mut book(MAX_BOOK_PAGES + 1).as_slice()).is_err(),
        "a book past the page limit is refused"
    );
}

const CUSTOM_DATA: usize = 0;
const CUSTOM_NAME: usize = 6;
const WRITTEN_BOOK_CONTENT: usize = 57;
const CONTAINER: usize = 77;

fn var_int(value: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    VarInt(i32::try_from(value).expect("the test lengths fit in 32 bits"))
        .encode(&mut bytes)
        .expect("a Vec takes every byte");
    bytes
}

fn byte_array_under_a(length: usize) -> Vec<u8> {
    let declared = i32::try_from(length).expect("the test lengths fit in 32 bits");
    let mut nbt = vec![10, 7, 0, 1, b'a'];
    nbt.extend_from_slice(&declared.to_be_bytes());
    nbt.resize(nbt.len() + length, 0);
    nbt.push(0);
    nbt
}

fn with_component(kind: usize, value: &[u8]) -> Vec<u8> {
    [var_int(1), var_int(0), var_int(kind), value.to_vec()].concat()
}

fn creative_slot(kind: usize, value: &[u8]) -> Vec<u8> {
    let component = [var_int(value.len()), value.to_vec()].concat();
    [
        36i16.to_be_bytes().to_vec(),
        var_int(1),
        var_int(1),
        with_component(kind, &component),
    ]
    .concat()
}

fn container_of_two(length: usize) -> Vec<u8> {
    let template = [
        vec![1],
        var_int(1),
        var_int(1),
        with_component(0, &byte_array_under_a(length)),
    ]
    .concat();
    [var_int(2), template.clone(), template].concat()
}

fn text(content: &str) -> Vec<u8> {
    let length = u16::try_from(content.len()).expect("the text fits a String");
    [
        vec![8],
        length.to_be_bytes().to_vec(),
        content.as_bytes().to_vec(),
    ]
    .concat()
}

fn book_of(pages: usize, page: &str, filtered: bool) -> Vec<u8> {
    let text = if filtered {
        [text(page), vec![1], text(page)].concat()
    } else {
        [text(page), vec![0]].concat()
    };
    let mut book = [
        var_int(1),
        b"t".to_vec(),
        vec![0],
        var_int(1),
        b"a".to_vec(),
    ]
    .concat();
    book.extend([var_int(0), var_int(pages)].concat());
    for _ in 0..pages {
        book.extend_from_slice(&text);
    }
    book.push(0);
    book
}

#[test]
fn a_creative_slot_packet_shares_one_nbt_budget_across_its_stack() {
    let cases = [
        (
            creative_slot(CUSTOM_DATA, &byte_array_under_a(2_097_148)),
            true,
        ),
        (
            creative_slot(CUSTOM_DATA, &byte_array_under_a(2_097_149)),
            false,
        ),
        (creative_slot(CONTAINER, &container_of_two(1_048_572)), true),
        (
            creative_slot(CONTAINER, &container_of_two(1_048_573)),
            false,
        ),
        (
            creative_slot(
                WRITTEN_BOOK_CONTENT,
                &book_of(100, &"x".repeat(10_485), false),
            ),
            true,
        ),
        (
            creative_slot(
                WRITTEN_BOOK_CONTENT,
                &book_of(100, &"x".repeat(10_486), false),
            ),
            false,
        ),
        (
            creative_slot(WRITTEN_BOOK_CONTENT, &book_of(262_145, "", false)),
            false,
        ),
    ];
    let budget = NbtBudget {
        bytes: 2_097_152,
        tags: 262_144,
    };
    for (index, (bytes, passes)) in cases.iter().enumerate() {
        let read = mcrs_minecraft_nbt::with_nbt_budget(budget, || {
            ServerboundSetCreativeModeSlot::decode(&mut bytes.as_slice()).map(|_| ())
        });
        if *passes {
            assert!(matches!(read, Ok(Ok(()))), "{index}: {read:?}");
        } else {
            assert!(matches!(read, Err(NbtQuotaExceeded)), "{index}: {read:?}");
        }
    }
}

fn named_patch(name: Option<&str>, kind: usize, value: &[u8]) -> Vec<u8> {
    match name {
        Some(name) => [
            var_int(2),
            var_int(0),
            var_int(CUSTOM_NAME),
            text(name),
            var_int(kind),
            value.to_vec(),
        ]
        .concat(),
        None => with_component(kind, value),
    }
}

fn shulker_of_books(books: usize, name: Option<&str>) -> Vec<u8> {
    let page = "\u{3042}".repeat(1024);
    let book = [
        vec![1],
        var_int(1),
        var_int(1),
        named_patch(name, WRITTEN_BOOK_CONTENT, &book_of(100, &page, true)),
    ]
    .concat();
    [var_int(books), book.repeat(books)].concat()
}

fn at_default_budget(read: impl FnOnce() -> bool) -> Result<bool, NbtQuotaExceeded> {
    mcrs_minecraft_nbt::with_nbt_budget(NbtBudget::default(), read)
}

#[test]
fn a_survival_shulker_of_books_passes_and_a_long_list_is_refused_at_the_default_budget() {
    let creative = |bytes: &[u8]| ServerboundSetCreativeModeSlot::decode(&mut &*bytes).is_ok();
    let thirteen_books = creative_slot(CONTAINER, &shulker_of_books(13, None));
    assert_eq!(
        at_default_budget(|| creative(&thirteen_books)),
        Ok(true),
        "NBQ-VEC-018"
    );

    let mut list = vec![10, 9, 0, 1, b'a', 1];
    list.extend_from_slice(&300_000i32.to_be_bytes());
    list.resize(list.len() + 300_000, 0);
    list.push(0);
    let long_list = creative_slot(CUSTOM_DATA, &list);
    assert_eq!(
        at_default_budget(|| creative(&long_list)),
        Err(NbtQuotaExceeded),
        "NBQ-VEC-019"
    );

    let name = "\u{3042}".repeat(50);
    let full_shulker = [
        var_int(1),
        var_int(1),
        named_patch(Some(&name), CONTAINER, &shulker_of_books(27, Some(&name))),
    ]
    .concat();
    assert_eq!(
        at_default_budget(|| ProtoStack::decode(&mut full_shulker.as_slice()).is_ok()),
        Ok(true),
        "NBQ-VEC-020"
    );
    let one_short = NbtBudget {
        bytes: 33_185_999,
        tags: 262_144,
    };
    assert_eq!(
        mcrs_minecraft_nbt::with_nbt_budget(one_short, || {
            ProtoStack::decode(&mut full_shulker.as_slice()).is_ok()
        }),
        Err(NbtQuotaExceeded),
        "the charge of NBQ-VEC-020 is 33 186 000"
    );
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
