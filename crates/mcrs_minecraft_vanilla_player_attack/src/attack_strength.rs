const TICKS_PER_SECOND: f64 = 20.0;
const HALF_TICK: f32 = 0.5;

#[must_use]
pub fn attack_strength(counter: u32, attack_speed: f64) -> f32 {
    let delay = (1.0 / attack_speed * TICKS_PER_SECOND) as f32;
    ((counter as f32 + HALF_TICK) / delay).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT_SPEED: f64 = 4.0;

    #[test]
    fn strength_of_counter() {
        let cases = [
            (0, DEFAULT_SPEED, 0.1),
            (4, DEFAULT_SPEED, 0.9),
            (5, DEFAULT_SPEED, 1.0),
            (6, DEFAULT_SPEED, 1.0),
        ];
        for (counter, attack_speed, expected) in cases {
            assert_eq!(
                attack_strength(counter, attack_speed),
                expected,
                "counter {counter}"
            );
        }
    }
}
