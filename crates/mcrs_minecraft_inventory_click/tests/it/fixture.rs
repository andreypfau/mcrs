use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_inventory_click::ClickError;
use mcrs_minecraft_inventory_click::ClickPlayer;
use mcrs_minecraft_inventory_click::Drag;
use mcrs_minecraft_inventory_click::Menu;
use mcrs_minecraft_inventory_click::Operations;
use mcrs_minecraft_inventory_click::Slots;
use mcrs_minecraft_inventory_menu::MenuEntry;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_inventory_player::ArmourLock;
use mcrs_minecraft_inventory_player::ArmourSlot;
use mcrs_minecraft_inventory_player::EquipSlot;
use mcrs_minecraft_inventory_player::Holder;
use mcrs_minecraft_inventory_player::Offer;
use mcrs_minecraft_inventory_player::PlayerMode;
use mcrs_minecraft_inventory_transaction::Filter;
use mcrs_minecraft_inventory_transaction::Loaded;
use mcrs_minecraft_inventory_transaction::Op;
use mcrs_minecraft_inventory_transaction::Plan;
use mcrs_minecraft_inventory_transaction::Planned;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub components: u8,
    pub equip: Option<EquipSlot>,
    pub lock: ArmourLock,
    pub damaged: bool,
}

pub type Stack = Option<Planned<Item>>;
pub type Operation = Op<HeldSlot, Planned<Item>>;

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

#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    inventory: [Stack; 41],
    cursor: Stack,
    grid: [Stack; 4],
    result: Stack,
    pub mode: PlayerMode,
    pub selected: u8,
    refusing: Vec<HeldSlot>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HeldSlot {
    pub holder: Holder,
    pub index: u16,
}

impl HeldSlot {
    pub const fn new(holder: Holder, index: u16) -> Self {
        Self { holder, index }
    }
}

pub const CURSOR: HeldSlot = HeldSlot::new(Holder::Cursor, 0);

fn held(entry: MenuEntry) -> Option<HeldSlot> {
    let holder = match entry.part {
        0 => Holder::Inventory,
        1 => Holder::CraftGrid,
        2 => Holder::CraftResult,
        _ => return None,
    };
    Some(HeldSlot::new(holder, entry.index))
}

fn menu_slot(index: u16) -> Option<HeldSlot> {
    MenuLayout::INVENTORY.entry(index).and_then(held)
}

pub fn entry(index: u16) -> HeldSlot {
    menu_slot(index).expect("the test names an entry of the inventory menu")
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
            refusing: Vec::new(),
        }
    }

    pub fn with(mut self, index: u16, stack: Stack) -> Self {
        *self.stack_mut(entry(index)) = stack;
        self
    }

    pub fn refusing(mut self, index: u16) -> Self {
        self.refusing.push(entry(index));
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

    pub fn stack(&self, slot: HeldSlot) -> Option<Stack> {
        let index = usize::from(slot.index);
        match slot.holder {
            Holder::Inventory => self.inventory.get(index).copied(),
            Holder::Cursor => (index == 0).then_some(self.cursor),
            Holder::CraftGrid => self.grid.get(index).copied(),
            Holder::CraftResult => (index == 0).then_some(self.result),
        }
    }

    fn stack_mut(&mut self, slot: HeldSlot) -> &mut Stack {
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
    type Slot = HeldSlot;
    type Stack = Item;

    fn cursor(&self) -> HeldSlot {
        CURSOR
    }

    fn inventory(&self, index: u16) -> Option<HeldSlot> {
        (index < Holder::Inventory.slot_count()).then_some(HeldSlot::new(Holder::Inventory, index))
    }

    fn load(&self, slot: HeldSlot) -> Option<Loaded<Item>> {
        let rule = slot.holder.slot(slot.index)?;
        let stack = self.stack(slot)?;
        let lock = stack.map_or(ArmourLock::Free, |stack| stack.stack.lock);
        Some(Loaded {
            stack,
            limit: rule.limit,
            take: rule.filter.take(lock, self.mode),
        })
    }

    fn place(&self, slot: HeldSlot, stack: Item) -> Filter {
        let Some(rule) = slot.holder.slot(slot.index) else {
            return Filter::Refuses;
        };
        if self.refusing.contains(&slot) {
            return Filter::Refuses;
        }
        let offer = Offer { wear: stack.equip };
        rule.filter.place(offer)
    }

    fn same_holder(&self, left: HeldSlot, right: HeldSlot) -> bool {
        left.holder == right.holder
    }

    fn equal(&self, left: Item, right: Item) -> bool {
        left == right
    }

    fn can_stack(&self, stack: Planned<Item>) -> bool {
        stack.max > 1 && !stack.stack.damaged
    }
}

pub struct Window<'a>(pub &'a Player);

impl Menu for Window<'_> {
    type Slot = HeldSlot;

    fn entry_count(&self) -> u16 {
        MenuLayout::INVENTORY.len()
    }

    fn slot(&self, index: u16) -> Option<HeldSlot> {
        menu_slot(index)
    }

    fn quick_move_destinations(
        &self,
        source: u16,
        holds_empty: impl Fn(u16) -> bool,
    ) -> impl Iterator<Item = u16> {
        let player = self.0;
        let equip = menu_slot(source)
            .and_then(|slot| player.stack(slot).flatten())
            .and_then(|stack| stack.stack.equip);
        MenuLayout::INVENTORY
            .routes(source)
            .find(|route| route.when.holds(equip, &holds_empty))
            .into_iter()
            .flat_map(|route| route.to.iter())
    }

    fn permits_pick_up_all(&self, index: u16) -> bool {
        MenuLayout::INVENTORY.permits_pick_up_all(index)
    }

    fn permits_drag(&self, index: u16) -> bool {
        MenuLayout::INVENTORY.permits_drag(index)
    }
}

pub fn click(
    player: &Player,
    kind: ClickKind,
    button: u8,
    clicked: Clicked,
) -> Result<Vec<Operation>, ClickError> {
    send(player, &mut Drag::new(), kind, button, clicked)
}

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

pub fn apply(player: &Player, ops: &[Operation]) -> Player {
    let mut scratch = Plan::new();
    let result = mcrs_minecraft_inventory_transaction::plan(
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

pub fn split(from: HeldSlot, to: HeldSlot, count: u8) -> Operation {
    Op::Split { from, to, count }
}

pub fn swap(left: HeldSlot, right: HeldSlot) -> Operation {
    Op::Swap { left, right }
}

pub fn drop_from(from: HeldSlot, count: u8) -> Operation {
    Op::Drop { from, count }
}

pub struct Case {
    pub name: &'static str,
    pub player: Player,
    pub button: u8,
    pub clicked: Clicked,
    pub ops: Vec<Operation>,
}

pub fn check(kind: ClickKind, cases: Vec<Case>) {
    for case in cases {
        let ops = click(&case.player, kind, case.button, case.clicked);
        assert_eq!(ops, Ok(case.ops), "{}", case.name);
    }
}
