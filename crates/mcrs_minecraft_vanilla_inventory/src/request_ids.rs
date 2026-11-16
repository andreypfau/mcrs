use mcrs_minecraft_engine_item::RequestId;

/// The request of a click in the inventory menu (ITX-JAV-001).
pub const CLICK_REQUEST: RequestId = RequestId(0);
/// The request of a drop key (INV-SEL-004).
pub const HAND_DROP_REQUEST: RequestId = RequestId(1);
/// The request of the hand swap (INV-SEL-005).
pub const HAND_SWAP_REQUEST: RequestId = RequestId(2);
/// The request of a close of the inventory menu (INV-CLS-001).
pub const CLOSE_REQUEST: RequestId = RequestId(3);
/// The request of a creative set (INV-CRE-004).
pub const CREATIVE_REQUEST: RequestId = RequestId(4);
