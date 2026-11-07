use crate::{Disconnect, Reports, Serverbound};

/// What layer 0 makes of one serverbound packet. It never drops a packet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict {
    Pass(Checked),
    Disconnect(Disconnect),
}

/// A packet that passes layer 0, with the findings it reports to layer 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Checked {
    pub packet: Serverbound,
    pub reports: Reports,
}
