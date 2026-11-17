use mcrs_minecraft_brain::KeyedRandom;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Immersion {
    Dry,
    Fluid(f64),
    Lava,
}

#[must_use]
pub fn float_holds(immersion: Immersion, eye_height: f32) -> bool {
    let threshold = if eye_height >= 0.4 { 0.4 } else { 0.0 };
    match immersion {
        Immersion::Dry => false,
        Immersion::Fluid(height) => height > threshold,
        Immersion::Lava => true,
    }
}

#[must_use]
pub fn float_jumps(random: &mut KeyedRandom) -> bool {
    random.unit_f32() < 0.8
}
