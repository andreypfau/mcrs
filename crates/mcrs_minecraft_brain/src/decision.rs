use crate::Intents;
use crate::MobId;
use crate::OrderKey;

/// The result of one full evaluation of one mob in the AI stage. It writes only the guidance state
/// of its own mob.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decision {
    pub mob: MobId,
    pub key: OrderKey,
    pub issue_tick: u64,
    pub intents: Intents,
}
