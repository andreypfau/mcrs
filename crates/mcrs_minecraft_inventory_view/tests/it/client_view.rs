use mcrs_minecraft_inventory_view::Agreement;
use mcrs_minecraft_inventory_view::ClientView;
use mcrs_minecraft_inventory_view::Mismatches;
use mcrs_minecraft_inventory_view::SlotView;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::HashScratch;
use mcrs_minecraft_item_stack::HashedStack;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::hash_stack;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_test_alloc::allocations;

const ITEMS: [&str; 3] = ["air", "stone", "apple"];
const STONE: u16 = 1;
const APPLE: u16 = 2;

struct Names;

impl NameLookup for Names {
    fn id(&self, table: NameTable, name: ResourceLocation<'_>) -> Option<u16> {
        if table != NameTable::Item || name.namespace() != "minecraft" {
            return None;
        }
        let index = ITEMS.iter().position(|item| *item == name.path())?;
        u16::try_from(index).ok()
    }

    fn name(&self, table: NameTable, id: u16) -> Option<ResourceLocation<'_>> {
        if table != NameTable::Item {
            return None;
        }
        let path = ITEMS.get(usize::from(id))?;
        ResourceLocation::new("minecraft", path).ok()
    }
}

fn stack(kind: u16, count: u8) -> ItemStack {
    let count = Count::new(count).expect("the test counts are not zero");
    ItemStack::new(Kind(kind), count)
}

fn hashed(kind: u16, count: u8) -> HashedStack {
    hash_stack(
        Some(&stack(kind, count)),
        &Names,
        &mut HashScratch::default(),
    )
    .expect("the test items are known")
    .expect("the test stacks are not empty")
}

/// Each pair is the client view of an entry and the stack that the server holds.
fn menu() -> (ClientView, Vec<Option<ItemStack>>) {
    let rows: [(SlotView, Option<ItemStack>); 7] = [
        (SlotView::Unknown, None),
        (
            SlotView::Stack(Some(stack(STONE, 1))),
            Some(stack(STONE, 1)),
        ),
        (
            SlotView::Stack(Some(stack(STONE, 1))),
            Some(stack(STONE, 2)),
        ),
        (
            SlotView::Hashed(Some(hashed(STONE, 2))),
            Some(stack(STONE, 2)),
        ),
        (
            SlotView::Hashed(Some(hashed(STONE, 2))),
            Some(stack(APPLE, 2)),
        ),
        (SlotView::Stack(None), None),
        (SlotView::Hashed(None), None),
    ];
    let mut view = ClientView::new(rows.len());
    let mut stacks = Vec::new();
    for (index, (slot, held)) in rows.into_iter().enumerate() {
        *view.entry_mut(index).expect("the view has each row") = slot;
        stacks.push(held);
    }
    stacks.push(Some(stack(STONE, 1)));
    (view, stacks)
}

#[test]
fn inv_syn_002_003_comparison_lists_each_entry_and_the_cursor_that_differ() {
    let (mut view, stacks) = menu();
    let cursor = stack(APPLE, 3);
    let mut scratch = HashScratch::default();
    let mut mismatches = Mismatches::default();
    let cases: [(SlotView, Agreement); 4] = [
        (SlotView::Unknown, Agreement::Differs),
        (SlotView::Stack(Some(stack(APPLE, 3))), Agreement::Matches),
        (SlotView::Hashed(Some(hashed(APPLE, 3))), Agreement::Matches),
        (SlotView::Hashed(Some(hashed(APPLE, 4))), Agreement::Differs),
    ];
    for (cursor_view, expected) in cases {
        *view.cursor_mut() = cursor_view.clone();
        view.compare(
            stacks.iter().map(Option::as_ref),
            Some(&cursor),
            &Names,
            &mut scratch,
            &mut mismatches,
        );
        assert_eq!(
            mismatches.entries(),
            [0, 2, 4, 7],
            "cursor view {cursor_view:?}"
        );
        assert_eq!(mismatches.cursor(), expected, "cursor view {cursor_view:?}");
    }
}

#[test]
fn inv_syn_003_comparison_with_a_warm_buffer_makes_no_allocation() {
    let (view, stacks) = menu();
    let mut scratch = HashScratch::default();
    let mut mismatches = Mismatches::default();
    let mut compare = |mismatches: &mut Mismatches| {
        view.compare(
            stacks.iter().map(Option::as_ref),
            None,
            &Names,
            &mut scratch,
            mismatches,
        );
    };
    compare(&mut mismatches);
    let before = allocations();
    compare(&mut mismatches);
    assert_eq!(
        allocations() - before,
        0,
        "the buffer already holds the largest result"
    );
}
