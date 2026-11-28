use mcrs_engine_muxer::ViewerId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerInput {
    Latency { viewer: ViewerId, latency: i32 },
    Backlog { viewer: ViewerId, backlog: Backlog },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Backlog {
    #[default]
    Clear,
    Behind,
}
