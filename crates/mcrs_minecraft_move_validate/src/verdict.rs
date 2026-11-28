use crate::{Disconnect, Reports, Serverbound};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict {
    Pass(Checked),
    Disconnect(Disconnect),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Checked {
    pub packet: Serverbound,
    pub reports: Reports,
}
