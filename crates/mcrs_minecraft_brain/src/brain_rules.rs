#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrainRules {
    pub player_range: f64,
    pub idle_limit: u32,
    pub idle_reset_range: f64,
}
