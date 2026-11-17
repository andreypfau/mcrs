use crate::ButtonBinding;
use crate::ClickInput;
use crate::ClickKind;
use crate::Clicked;
use crate::Gesture;
use crate::StackPresence;

const LEFT_BUTTON: u8 = 0;
const RIGHT_BUTTON: u8 = 1;
const OFFHAND_SWAP_BUTTON: u8 = 40;
const HOTBAR_KEYS: u8 = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Click {
    pub kind: ClickKind,
    pub button: u8,
    pub clicked: Clicked,
}

impl Click {
    #[must_use]
    pub fn from_input(clicked: Clicked, input: ClickInput) -> Option<Self> {
        let click = |kind, button| {
            Some(Self {
                kind,
                button,
                clicked,
            })
        };
        match input.gesture {
            Gesture::Press { button, binding } => {
                if input.cursor == StackPresence::Stack || clicked == Clicked::Nowhere {
                    return None;
                }
                if binding == ButtonBinding::PickItem && input.creative {
                    return click(ClickKind::Clone, button);
                }
                if button != LEFT_BUTTON && button != RIGHT_BUTTON {
                    return None;
                }
                match clicked {
                    Clicked::Outside => click(ClickKind::Throw, button),
                    _ if input.shift => click(ClickKind::QuickMove, button),
                    _ => click(ClickKind::PickUp, button),
                }
            }
            Gesture::Release { button, binding } => {
                if input.cursor == StackPresence::Empty {
                    return None;
                }
                match clicked {
                    _ if binding == ButtonBinding::PickItem => click(ClickKind::Clone, button),
                    Clicked::Outside => click(ClickKind::PickUp, button),
                    _ if input.shift => click(ClickKind::QuickMove, button),
                    _ => click(ClickKind::PickUp, button),
                }
            }
            Gesture::HotbarKey(key) => match key.checked_sub(1) {
                Some(button) if button < HOTBAR_KEYS => Self::swap(clicked, input, button),
                _ => None,
            },
            Gesture::OffhandKey => Self::swap(clicked, input, OFFHAND_SWAP_BUTTON),
            Gesture::DropKey => {
                let button = if input.control {
                    RIGHT_BUTTON
                } else {
                    LEFT_BUTTON
                };
                Self::on_target_stack(clicked, input, ClickKind::Throw, button)
            }
            Gesture::PickKey if input.creative => {
                Self::on_target_stack(clicked, input, ClickKind::Clone, LEFT_BUTTON)
            }
            Gesture::PickKey => None,
        }
    }

    fn swap(clicked: Clicked, input: ClickInput, button: u8) -> Option<Self> {
        match clicked {
            Clicked::Entry(_) if input.cursor == StackPresence::Empty => Some(Self {
                kind: ClickKind::Swap,
                button,
                clicked,
            }),
            _ => None,
        }
    }

    fn on_target_stack(
        clicked: Clicked,
        input: ClickInput,
        kind: ClickKind,
        button: u8,
    ) -> Option<Self> {
        match clicked {
            Clicked::Entry(_) if input.target == StackPresence::Stack => Some(Self {
                kind,
                button,
                clicked,
            }),
            _ => None,
        }
    }
}
