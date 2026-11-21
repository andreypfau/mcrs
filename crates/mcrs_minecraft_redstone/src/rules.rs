#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WireEvaluator {
    #[default]
    Default,
    Experimental,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RedstoneRules {
    pub quasi_connectivity: bool,
    pub sticky_piston_short_pulse: bool,
    pub pistons_move_block_entities: bool,
    pub dust_connects_to_pistons: bool,
    pub wire_evaluator: WireEvaluator,
}

impl RedstoneRules {
    pub const JAVA: Self = Self {
        quasi_connectivity: true,
        sticky_piston_short_pulse: true,
        pistons_move_block_entities: false,
        dust_connects_to_pistons: false,
        wire_evaluator: WireEvaluator::Default,
    };

    pub const BEDROCK: Self = Self {
        quasi_connectivity: false,
        sticky_piston_short_pulse: false,
        pistons_move_block_entities: true,
        dust_connects_to_pistons: true,
        wire_evaluator: WireEvaluator::Default,
    };
}

impl Default for RedstoneRules {
    fn default() -> Self {
        Self::JAVA
    }
}
