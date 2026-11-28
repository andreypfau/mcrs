use crate::Disconnect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Forward,
    EndTick(u64),
    Drop,
    Disconnect(Disconnect),
}
