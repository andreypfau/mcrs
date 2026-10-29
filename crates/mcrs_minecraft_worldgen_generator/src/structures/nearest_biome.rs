use bevy_math::IVec3;

const RADIUS: i32 = 200;
const STEP: i32 = 32;
const Y_STEP: i32 = 64;

/// The nearest sample point with one of the `wanted` biomes, searched ring by
/// ring so the search stops once no farther ring can be nearer. `None` for a dimension with no
/// height.
// chisle: the search runs on the caller's thread, as long as a few seconds for a biome that is
// absent; a task the tick polls lifts it.
pub fn nearest_biome(
    mut biome_at: impl FnMut(IVec3) -> Option<u16>,
    wanted: &[u16],
    origin: IVec3,
    min_y: i32,
    height: i32,
) -> Option<(IVec3, u16)> {
    let low = min_y.saturating_add(1);
    let high = min_y.saturating_add(height);
    if high < low {
        return None;
    }
    let centre = origin.y.clamp(low, high);
    let first = centre - (centre - low) / Y_STEP * Y_STEP;
    let mut best: Option<(i64, IVec3, u16)> = None;
    for ring in 0..=RADIUS {
        let nearest_possible = i64::from(ring * STEP).pow(2);
        if best.is_some_and(|(distance, ..)| distance <= nearest_possible) {
            break;
        }
        for i in -ring..=ring {
            let step = if i.abs() == ring { 1 } else { 2 * ring };
            for j in (-ring..=ring).step_by(step as usize) {
                let x = origin.x.saturating_add(STEP * i);
                let z = origin.z.saturating_add(STEP * j);
                for y in (first..=high).step_by(Y_STEP as usize) {
                    let at = IVec3::new(x, y, z);
                    let distance = (at.as_i64vec3() - origin.as_i64vec3()).length_squared();
                    if best.is_some_and(|(nearest, ..)| nearest <= distance) {
                        continue;
                    }
                    if let Some(biome) = biome_at(at).filter(|biome| wanted.contains(biome)) {
                        best = Some((distance, at, biome));
                    }
                }
            }
        }
    }
    best.map(|(_, at, biome)| (at, biome))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_search_finds_the_nearest_sample_and_skips_an_empty_dimension() {
        let east = |at: IVec3| Some(if at.x >= 100 { 7 } else { 1 });
        let origin = IVec3::new(0, 70, 0);
        assert_eq!(
            nearest_biome(east, &[7], origin, -64, 384),
            Some((IVec3::new(128, 70, 0), 7))
        );
        assert_eq!(nearest_biome(east, &[9], origin, -64, 384), None);
        assert_eq!(nearest_biome(east, &[7], origin, 0, 0), None);
    }

    #[test]
    fn a_nearer_point_on_a_later_ring_wins_over_a_corner_of_an_earlier_ring() {
        let two_columns = |at: IVec3| {
            Some(match (at.x, at.z) {
                (96, 96) | (128, 0) => 7,
                _ => 1,
            })
        };
        assert_eq!(
            nearest_biome(two_columns, &[7], IVec3::new(0, 70, 0), -64, 384),
            Some((IVec3::new(128, 70, 0), 7))
        );
    }
}
