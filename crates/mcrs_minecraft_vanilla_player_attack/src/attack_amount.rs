const WEAK_SHARE: f32 = 0.2;
const STRENGTH_SHARE: f32 = 0.8;

#[must_use]
pub fn attack_amount(attack_damage: f64, strength: f32) -> Option<f32> {
    let amount = attack_damage as f32 * (WEAK_SHARE + strength * strength * STRENGTH_SHARE);
    (amount > 0.0).then_some(amount)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_of_attack() {
        let cases = [
            (1.0, 1.0, Some(1.0)),
            (1.0, 0.1, Some(0.208)),
            (7.0, 0.955, Some(6.50734)),
            (0.0, 1.0, None),
        ];
        for (attack_damage, strength, expected) in cases {
            let amount = attack_amount(attack_damage, strength);
            let close = match (amount, expected) {
                (Some(amount), Some(expected)) => (amount - expected).abs() < 1.0E-5,
                (None, None) => true,
                _ => false,
            };
            assert!(close, "{amount:?} != {expected:?}");
        }
    }
}
