use mcrs_minecraft_brain::KeyedRandom;

/// What the mob is in, for the float goal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Immersion {
    Dry,
    /// The height of the fluid of the tag `entity_floatable` at the mob.
    Fluid(f64),
    Lava,
}

/// The float goal is a reflex of the hot tick (GOAL-FLT-002): it holds `jump` while this is true.
#[must_use]
pub fn float_holds(immersion: Immersion, eye_height: f32) -> bool {
    let threshold = if eye_height >= 0.4 { 0.4 } else { 0.0 };
    match immersion {
        Immersion::Dry => false,
        Immersion::Fluid(height) => height > threshold,
        Immersion::Lava => true,
    }
}

/// The jump of the float goal in one tick, from the stream of the part
/// [`FLOAT_JUMP`](crate::FLOAT_JUMP) of that tick.
#[must_use]
pub fn float_jumps(random: &mut KeyedRandom) -> bool {
    random.unit_f32() < 0.8
}
