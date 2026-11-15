/// The id of a gate function that [`register_slot_gate`](crate::ItemAppExt::register_slot_gate)
/// gave. It is valid in the world of that registration only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GateId(pub(crate) u16);
