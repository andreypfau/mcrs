use mcrs_minecraft_inventory_click::Click;
use mcrs_minecraft_inventory_click::ClickError;
use mcrs_minecraft_inventory_click::ClickKind;
use mcrs_minecraft_inventory_click::ClickPlayer;
use mcrs_minecraft_inventory_click::Clicked;
use mcrs_minecraft_inventory_click::Drag;
use mcrs_minecraft_inventory_click::Menu;
use mcrs_minecraft_inventory_click::Operations;
use mcrs_minecraft_inventory_click::Slots;
use mcrs_minecraft_inventory_plan::Filter;
use mcrs_minecraft_inventory_plan::Loaded;
use mcrs_minecraft_inventory_plan::Op;
use mcrs_minecraft_inventory_plan::Plan;
use mcrs_minecraft_inventory_plan::Planned;
use mcrs_minecraft_menu::InventoryMenu;
use mcrs_minecraft_menu::MenuRange;
use mcrs_minecraft_menu::MenuSlot;
use mcrs_minecraft_player_slots::ArmourLock;
use mcrs_minecraft_player_slots::ArmourSlot;
use mcrs_minecraft_player_slots::EquipSlot;
use mcrs_minecraft_player_slots::Holder;
use mcrs_minecraft_player_slots::OffHandRule;
use mcrs_minecraft_player_slots::Offer;
use mcrs_minecraft_player_slots::PlayerMode;

/// The handle of a stack: the parts that the filters and the equality read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub components: u8,
    pub equip: Option<EquipSlot>,
    pub lock: ArmourLock,
    pub damaged: bool,
}

pub type Stack = Option<Planned<Item>>;
pub type Operation = Op<MenuSlot, Planned<Item>>;

const PLAIN: Item = Item {
    components: 0,
    equip: None,
    lock: ArmourLock::Free,
    damaged: false,
};
const HEAD: Option<EquipSlot> = Some(EquipSlot::Armour(ArmourSlot::Head));

fn stack(kind: u16, item: Item, count: u8, max: u8) -> Stack {
    Some(Planned {
        stack: item,
        kind,
        count,
        max,
    })
}

pub fn stone(count: u8) -> Stack {
    stack(1, PLAIN, count, 64)
}

pub fn dirt(count: u8) -> Stack {
    stack(2, PLAIN, count, 64)
}

pub fn sword() -> Stack {
    stack(3, PLAIN, 1, 1)
}

pub fn helmet() -> Stack {
    stack(
        4,
        Item {
            equip: HEAD,
            ..PLAIN
        },
        1,
        1,
    )
}

/// A helmet with an enchantment that prevents an armour change.
pub fn locked_helmet() -> Stack {
    stack(
        4,
        Item {
            equip: HEAD,
            lock: ArmourLock::Locked,
            ..PLAIN
        },
        1,
        1,
    )
}

/// A stackable item that the head slot accepts; `components` tells two such stacks apart.
pub fn pumpkin(components: u8, count: u8) -> Stack {
    stack(
        5,
        Item {
            components,
            equip: HEAD,
            ..PLAIN
        },
        count,
        64,
    )
}

/// The slots of one player, addressed by the entries of the inventory menu.
#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    inventory: [Stack; 41],
    cursor: Stack,
    grid: [Stack; 4],
    result: Stack,
    pub mode: PlayerMode,
    pub selected: u8,
}

pub const CURSOR: MenuSlot = MenuSlot::new(Holder::Cursor, 0);

/// The slot of the menu index `index`. Panics for an index that is not an entry.
pub fn entry(index: u16) -> MenuSlot {
    InventoryMenu::slot(index).expect("the test names an entry of the inventory menu")
}

impl Player {
    pub fn new() -> Self {
        Self {
            inventory: [None; 41],
            cursor: None,
            grid: [None; 4],
            result: None,
            mode: PlayerMode::NotCreative,
            selected: 0,
        }
    }

    pub fn with(mut self, index: u16, stack: Stack) -> Self {
        *self.stack_mut(entry(index)) = stack;
        self
    }

    pub fn carrying(mut self, stack: Stack) -> Self {
        self.cursor = stack;
        self
    }

    pub fn creative(mut self) -> Self {
        self.mode = PlayerMode::Creative;
        self
    }

    pub fn stack(&self, slot: MenuSlot) -> Option<Stack> {
        let index = usize::from(slot.index);
        match slot.holder {
            Holder::Inventory => self.inventory.get(index).copied(),
            Holder::Cursor => (index == 0).then_some(self.cursor),
            Holder::CraftGrid => self.grid.get(index).copied(),
            Holder::CraftResult => (index == 0).then_some(self.result),
        }
    }

    /// Panics for a slot that the player does not have.
    fn stack_mut(&mut self, slot: MenuSlot) -> &mut Stack {
        let index = usize::from(slot.index);
        match slot.holder {
            Holder::Inventory => &mut self.inventory[index],
            Holder::Cursor => &mut self.cursor,
            Holder::CraftGrid => &mut self.grid[index],
            Holder::CraftResult => &mut self.result,
        }
    }

    pub fn at(&self, index: u16) -> Stack {
        self.stack(entry(index)).flatten()
    }

    pub fn cursor_stack(&self) -> Stack {
        self.cursor
    }
}

impl Slots for Player {
    type Slot = MenuSlot;
    type Stack = Item;

    fn cursor(&self) -> MenuSlot {
        CURSOR
    }

    fn inventory(&self, index: u16) -> Option<MenuSlot> {
        (index < Holder::Inventory.slot_count()).then_some(MenuSlot::new(Holder::Inventory, index))
    }

    fn load(&self, slot: MenuSlot) -> Option<Loaded<Item>> {
        let rule = slot.holder.slot(slot.index)?;
        let stack = self.stack(slot)?;
        let lock = stack.map_or(ArmourLock::Free, |stack| stack.stack.lock);
        Some(Loaded {
            stack,
            limit: rule.limit,
            take: rule.filter.take(lock, self.mode),
        })
    }

    fn place(&self, slot: MenuSlot, stack: Item) -> Filter {
        let Some(rule) = slot.holder.slot(slot.index) else {
            return Filter::Refuses;
        };
        let offer = Offer {
            wear: stack.equip,
            ..Offer::default()
        };
        rule.filter.place(offer, OffHandRule::AnyItem)
    }

    fn equal(&self, left: Item, right: Item) -> bool {
        left == right
    }

    fn can_stack(&self, stack: Planned<Item>) -> bool {
        stack.max > 1 && !stack.stack.damaged
    }
}

/// The inventory menu over the slots of `Player`.
pub struct Window<'a>(pub &'a Player);

impl Menu for Window<'_> {
    type Slot = MenuSlot;

    fn entry_count(&self) -> u16 {
        InventoryMenu::LEN
    }

    fn slot(&self, index: u16) -> Option<MenuSlot> {
        InventoryMenu::slot(index)
    }

    fn quick_move_destinations(&self, source: u16) -> impl Iterator<Item = u16> {
        let player = self.0;
        let equip = InventoryMenu::slot(source)
            .and_then(|slot| player.stack(slot).flatten())
            .and_then(|stack| stack.stack.equip);
        let holds_empty = |index: u16| {
            InventoryMenu::slot(index).is_some_and(|slot| player.stack(slot) == Some(None))
        };
        InventoryMenu::quick_move_destinations(source, equip, holds_empty)
            .into_iter()
            .flatten()
            .flat_map(MenuRange::iter)
    }

    fn permits_pick_up_all(&self, index: u16) -> bool {
        InventoryMenu::permits_pick_up_all(index)
    }

    fn permits_drag(&self, index: u16) -> bool {
        InventoryMenu::permits_drag(index)
    }
}

/// The operations of `kind` with `button` on `clicked`. Panics when the request that the click
/// gives does not apply, because a request of a click applies by construction (ITX-JAV-003).
pub fn click(
    player: &Player,
    kind: ClickKind,
    button: u8,
    clicked: Clicked,
) -> Result<Vec<Operation>, ClickError> {
    send(player, &mut Drag::new(), kind, button, clicked)
}

/// [`click`] with the drag state `drag` of the menu.
pub fn send(
    player: &Player,
    drag: &mut Drag,
    kind: ClickKind,
    button: u8,
    clicked: Clicked,
) -> Result<Vec<Operation>, ClickError> {
    let mut ops = Operations::new();
    let input = Click {
        kind,
        button,
        clicked,
    };
    let clicker = ClickPlayer {
        mode: player.mode,
        selected: player.selected,
    };
    mcrs_minecraft_inventory_click::request(
        input,
        clicker,
        &Window(player),
        player,
        drag,
        &mut ops,
    )?;
    let ops: Vec<Operation> = ops.iter().collect();
    apply(player, &ops);
    Ok(ops)
}

/// The slots after `ops`. Panics when the request has a rejection.
pub fn apply(player: &Player, ops: &[Operation]) -> Player {
    let mut scratch = Plan::new();
    let result = mcrs_minecraft_inventory_plan::plan(
        ops.iter().copied(),
        &mut scratch,
        |slot| player.load(slot),
        |slot, stack| player.place(slot, stack),
        |left, right| left == right,
    );
    if let Err(rejected) = result {
        panic!("the request of a click applies, but {rejected} for {ops:?}");
    }
    let mut after = player.clone();
    for change in scratch.changes() {
        *after.stack_mut(change.slot) = change.after;
    }
    after
}

pub fn split(from: MenuSlot, to: MenuSlot, count: u8) -> Operation {
    Op::Split { from, to, count }
}

pub fn swap(left: MenuSlot, right: MenuSlot) -> Operation {
    Op::Swap { left, right }
}

pub fn drop_from(from: MenuSlot, count: u8) -> Operation {
    Op::Drop { from, count }
}

pub struct Case {
    pub name: &'static str,
    pub player: Player,
    pub button: u8,
    pub clicked: Clicked,
    pub ops: Vec<Operation>,
}

/// Checks the operations of each case for the click kind `kind`.
pub fn check(kind: ClickKind, cases: Vec<Case>) {
    for case in cases {
        let ops = click(&case.player, kind, case.button, case.clicked);
        assert_eq!(ops, Ok(case.ops), "{}", case.name);
    }
}
