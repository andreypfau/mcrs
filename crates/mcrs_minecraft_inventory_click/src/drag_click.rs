use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_inventory_player::PlayerMode;
use mcrs_minecraft_inventory_transaction::Op;
use mcrs_minecraft_inventory_transaction::Planned;

use crate::ClickError;
use crate::ClickPlayer;
use crate::Drag;
use crate::DragEntry;
use crate::DragKind;
use crate::DragSplit;
use crate::DragStage;
use crate::EntryPreview;
use crate::Menu;
use crate::Slots;
use crate::pick_up;
use crate::pick_up::MouseButton;
use crate::stays_in_drag;
use crate::view::Ops;
use crate::view::Seat;
use crate::view::View;

#[derive(Clone, Copy)]
enum Step {
    Start,
    Slot(DragKind),
    End(DragKind),
}

impl Step {
    fn of(button: u8, drag: &Drag) -> Option<Self> {
        match (DragStage::from_button(button)?, drag.kind()) {
            (DragStage::Start, None) => Some(Self::Start),
            (DragStage::Entry, Some(kind)) => Some(Self::Slot(kind)),
            (DragStage::End, Some(kind)) => Some(Self::End(kind)),
            _ => None,
        }
    }
}

pub(crate) fn drag_click<M: Menu<Slot = S::Slot>, S: Slots>(
    view: &View<'_, M, S>,
    click: Click,
    player: ClickPlayer,
    drag: &mut Drag,
    ops: &mut Ops<S>,
) -> Result<(), ClickError> {
    let Some(step) = Step::of(click.button, drag) else {
        drag.reset();
        return Ok(());
    };
    let cursor = view.cursor()?;
    let Some(carried) = cursor.stack else {
        drag.reset();
        return Ok(());
    };
    match step {
        Step::Start => match DragKind::from_button(click.button) {
            Some(DragKind::Full) if player.mode != PlayerMode::Creative => drag.reset(),
            Some(kind) => drag.open(kind),
            None => drag.reset(),
        },
        Step::Slot(kind) => {
            let Clicked::Entry(index) = click.clicked else {
                return Ok(());
            };
            let target = view.entry(index)?;
            let room = kind == DragKind::Full || u32::from(carried.count) > drag.len();
            if room && admits(view, index, &target, carried) {
                drag.insert(index);
            }
        }
        Step::End(kind) => {
            let ended = *drag;
            drag.reset();
            if kind == DragKind::Full && player.mode != PlayerMode::Creative {
                return Ok(());
            }
            return end(view, &ended, kind, &cursor, carried, ops);
        }
    }
    Ok(())
}

fn admits<M: Menu<Slot = S::Slot>, S: Slots>(
    view: &View<'_, M, S>,
    index: u16,
    target: &Seat<S>,
    carried: Planned<S::Stack>,
) -> bool {
    stays_in_drag::drag_entry(view, index, target, carried).is_some() && view.place(target, carried)
}

pub(crate) fn holds<M: Menu<Slot = S::Slot>, S: Slots>(
    view: &View<'_, M, S>,
    target: &Seat<S>,
    carried: Planned<S::Stack>,
) -> bool {
    match target.stack {
        None => true,
        Some(held) => view.same(held, carried) && held.count <= carried.max,
    }
}

fn end<M: Menu<Slot = S::Slot>, S: Slots>(
    view: &View<'_, M, S>,
    drag: &Drag,
    kind: DragKind,
    cursor: &Seat<S>,
    carried: Planned<S::Stack>,
    ops: &mut Ops<S>,
) -> Result<(), ClickError> {
    let size = drag.len();
    let split = DragSplit::from_carried(kind, &carried, size);
    match (size, kind) {
        (0, _) | (1, DragKind::Full) => Ok(()),
        (1, DragKind::Split) => pick_up_single(view, drag, MouseButton::Primary, ops),
        (1, DragKind::Single) => pick_up_single(view, drag, MouseButton::Secondary, ops),
        (_, DragKind::Split | DragKind::Single) if u32::from(carried.count) < size => Ok(()),
        (_, DragKind::Split | DragKind::Single) => spread(view, drag, split, cursor, carried, ops),
        (_, DragKind::Full) => fill(view, drag, split, cursor, carried, ops),
    }
}

fn pick_up_single<M: Menu<Slot = S::Slot>, S: Slots>(
    view: &View<'_, M, S>,
    drag: &Drag,
    button: MouseButton,
    ops: &mut Ops<S>,
) -> Result<(), ClickError> {
    for index in drag.entries() {
        if let Some(op) = pick_up::on_entry(view, index, button)? {
            ops.push(op)?;
        }
    }
    Ok(())
}

fn spread<M: Menu<Slot = S::Slot>, S: Slots>(
    view: &View<'_, M, S>,
    drag: &Drag,
    split: DragSplit,
    cursor: &Seat<S>,
    carried: Planned<S::Stack>,
    ops: &mut Ops<S>,
) -> Result<(), ClickError> {
    for index in drag.entries() {
        let target = view.entry(index)?;
        if !admits(view, index, &target, carried) {
            continue;
        }
        let held = target.stack.map_or(0, |held| held.count);
        let cap = target.cap(carried);
        let filled = split
            .entry(DragEntry { held, cap })
            .map_or(held, EntryPreview::count);
        let count = filled.saturating_sub(held);
        if count > 0 {
            ops.push(Op::Split {
                from: cursor.slot,
                to: target.slot,
                count,
            })?;
        }
    }
    Ok(())
}

fn fill<M: Menu<Slot = S::Slot>, S: Slots>(
    view: &View<'_, M, S>,
    drag: &Drag,
    split: DragSplit,
    cursor: &Seat<S>,
    carried: Planned<S::Stack>,
    ops: &mut Ops<S>,
) -> Result<(), ClickError> {
    let mut left = i32::from(carried.count);
    for index in drag.entries() {
        let target = view.entry(index)?;
        if !admits(view, index, &target, carried) {
            continue;
        }
        let held = target.stack.map_or(0, |held| held.count);
        let cap = target.cap(carried);
        let full = split
            .entry(DragEntry { held, cap })
            .map_or(held, EntryPreview::count);
        if held == full {
            continue;
        }
        ops.push(Op::Set {
            slot: target.slot,
            stack: Some(Planned {
                count: full,
                ..carried
            }),
            count: full,
        })?;
        left -= i32::from(full) - i32::from(held);
    }
    let count = u8::try_from(left.max(0)).unwrap_or(u8::MAX);
    ops.push(Op::Set {
        slot: cursor.slot,
        stack: Some(Planned { count, ..carried }),
        count,
    })
}
