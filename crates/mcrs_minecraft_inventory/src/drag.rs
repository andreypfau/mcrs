use mcrs_minecraft_protocol::item::{QuickCraftButton, QuickCraftKind, QuickCraftStage};

use crate::plan::Click;
use crate::slot::MenuSnapshot;

/// A quick-craft drag in progress: the kind chosen by the header and the menu
/// indices admitted so far, in admission order with no duplicates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drag {
    pub kind: QuickCraftKind,
    pub indices: Vec<usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Feed {
    Pending,
    Reset,
    Complete(Drag),
}

impl Drag {
    /// Feeds one `QuickCraft` click into the drag state machine, porting
    /// vanilla's stage transition, empty-cursor and kind-validity checks.
    pub fn feed(drag: &mut Option<Drag>, click: Click, snapshot: &MenuSnapshot) -> Feed {
        let Ok(button) = QuickCraftButton::try_from(click.button) else {
            *drag = None;
            return Feed::Reset;
        };
        let legal = match drag {
            None => button.stage == QuickCraftStage::Header,
            Some(_) => matches!(button.stage, QuickCraftStage::Slot | QuickCraftStage::End),
        };
        if !legal {
            *drag = None;
            return Feed::Reset;
        }
        let Some(carried) = snapshot.get(snapshot.carried()).cloned() else {
            *drag = None;
            return Feed::Reset;
        };
        match button.stage {
            QuickCraftStage::Header => {
                if button.kind == QuickCraftKind::Full && !click.creative {
                    *drag = None;
                    return Feed::Reset;
                }
                *drag = Some(Drag {
                    kind: button.kind,
                    indices: Vec::new(),
                });
                Feed::Pending
            }
            QuickCraftStage::Slot => {
                let current = drag.as_mut().expect("Slot stage only legal with a drag");
                if let Some(index) = usize::try_from(click.slot)
                    .ok()
                    .filter(|index| *index < snapshot.layout.len())
                    && !current.indices.contains(&index)
                {
                    let slot = snapshot.layout[index];
                    let admit = snapshot.can_quick_replace(slot, &carried)
                        && snapshot.may_place(slot, &carried)
                        && (current.kind == QuickCraftKind::Full
                            || usize::from(carried.count) > current.indices.len());
                    if admit {
                        current.indices.push(index);
                    }
                }
                Feed::Pending
            }
            QuickCraftStage::End => {
                let current = drag.take().expect("End stage only legal with a drag");
                if current.indices.is_empty() {
                    Feed::Reset
                } else {
                    Feed::Complete(current)
                }
            }
        }
    }
}

/// The vanilla per-slot new counts for an admitted set and the cursor count
/// left over, re-checking every slot against the current snapshot.
pub fn quick_craft_counts(
    kind: QuickCraftKind,
    indices: &[usize],
    snapshot: &MenuSnapshot,
) -> (Vec<(usize, u8)>, u8) {
    let Some(carried) = snapshot.get(snapshot.carried()).cloned() else {
        return (Vec::new(), 0);
    };
    if indices.is_empty() {
        return (Vec::new(), carried.count);
    }
    let n = indices.len();
    let place = match kind {
        QuickCraftKind::Split => carried.count / n as u8,
        QuickCraftKind::Single => 1,
        QuickCraftKind::Full => carried.max,
    };
    let mut remaining = i32::from(carried.count);
    let mut placed = Vec::new();
    for &index in indices {
        let Some(&slot) = snapshot.layout.get(index) else {
            continue;
        };
        let admit = snapshot.can_quick_replace(slot, &carried)
            && snapshot.may_place(slot, &carried)
            && (kind == QuickCraftKind::Full || usize::from(carried.count) >= n);
        if !admit {
            continue;
        }
        let had = snapshot.count(slot);
        let max_size = carried
            .max
            .min(snapshot.slot_max(slot, &carried).expect("may_place held"));
        let new_count = (u16::from(place) + u16::from(had)).min(u16::from(max_size)) as u8;
        remaining -= i32::from(new_count) - i32::from(had);
        placed.push((index, new_count));
    }
    (placed, remaining.max(0) as u8)
}
