use crate::EquipSlot;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Offer {
    pub wear: Option<EquipSlot>,
}
