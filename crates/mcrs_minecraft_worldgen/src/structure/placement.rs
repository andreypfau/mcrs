use std::f64::consts::PI;

use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;

use crate::jmath::floor_div;
use crate::structure::{FrequencyReduction, SpreadType, StructurePlacement};

pub const BIOME_SEARCH_QUARTS: i32 = 112 >> 2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpreadPlacement {
    pub salt: i32,
    pub frequency: f32,
    pub reduction: FrequencyReduction,
    pub exclusion_chunks: Option<i32>,
    pub spacing: i32,
    pub separation: i32,
    pub spread_type: SpreadType,
}

impl SpreadPlacement {
    pub fn of(placement: &StructurePlacement) -> Option<Self> {
        let StructurePlacement::RandomSpread {
            spreading,
            spacing,
            separation,
            spread_type,
        } = placement
        else {
            return None;
        };
        Some(Self {
            salt: spreading.salt.0,
            frequency: spreading.frequency.0 as f32,
            reduction: spreading.frequency_reduction_method,
            exclusion_chunks: spreading
                .exclusion_zone
                .as_ref()
                .map(|zone| zone.chunk_count.0),
            spacing: spacing.0,
            separation: separation.0,
            spread_type: *spread_type,
        })
    }

    pub fn potential_chunk(&self, seed: i64, x: i32, z: i32) -> (i32, i32) {
        let grid_x = floor_div(x, self.spacing);
        let grid_z = floor_div(z, self.spacing);
        let mut rng = LegacyRandom::large_feature_with_salt(seed, grid_x, grid_z, self.salt);
        let limit = self.spacing - self.separation;
        let spread = |rng: &mut LegacyRandom| match self.spread_type {
            SpreadType::Linear => rng.next_i32_bound(limit),
            SpreadType::Triangular => (rng.next_i32_bound(limit) + rng.next_i32_bound(limit)) / 2,
        };
        let offset_x = spread(&mut rng);
        let offset_z = spread(&mut rng);
        (
            grid_x * self.spacing + offset_x,
            grid_z * self.spacing + offset_z,
        )
    }

    pub fn is_structure_chunk(
        &self,
        seed: i64,
        x: i32,
        z: i32,
        excluded: impl FnMut(i32, i32) -> bool,
    ) -> bool {
        self.potential_chunk(seed, x, z) == (x, z)
            && frequency_gate(seed, self.salt, self.frequency, self.reduction, x, z)
            && !self
                .exclusion_chunks
                .is_some_and(|range| excluded_in_range(range, x, z, excluded))
    }
}

pub fn frequency_gate(
    seed: i64,
    salt: i32,
    frequency: f32,
    method: FrequencyReduction,
    x: i32,
    z: i32,
) -> bool {
    if frequency >= 1.0 {
        return true;
    }
    match method {
        FrequencyReduction::Default => {
            LegacyRandom::large_feature_with_salt(seed, salt, x, z).next_f32() < frequency
        }
        FrequencyReduction::LegacyType1 => {
            let region_x = x >> 4;
            let region_z = z >> 4;
            let mut rng = LegacyRandom::new(((region_x ^ (region_z << 4)) as i64 ^ seed) as u64);
            rng.next_i32();
            rng.next_i32_bound((1.0f32 / frequency) as i32) == 0
        }
        FrequencyReduction::LegacyType2 => {
            LegacyRandom::large_feature_with_salt(seed, x, z, 10387320).next_f32() < frequency
        }
        FrequencyReduction::LegacyType3 => {
            LegacyRandom::large_feature(seed, x, z).next_f64() < frequency as f64
        }
    }
}

pub fn excluded_in_range(
    range: i32,
    x: i32,
    z: i32,
    mut excluded: impl FnMut(i32, i32) -> bool,
) -> bool {
    (x - range..=x + range)
        .any(|test_x| (z - range..=z + range).any(|test_z| excluded(test_x, test_z)))
}

pub fn select_with_removal<T: Copy>(
    seed: i64,
    x: i32,
    z: i32,
    entries: &[(T, i32)],
    mut try_start: impl FnMut(T) -> bool,
) -> Option<T> {
    if let [(only, _)] = entries {
        return try_start(*only).then_some(*only);
    }
    let mut options = entries.to_vec();
    let mut rng = LegacyRandom::large_feature(seed, x, z);
    let mut total: i32 = options.iter().map(|(_, weight)| weight).sum();
    while !options.is_empty() {
        let mut choice = rng.next_i32_bound(total);
        let index = options
            .iter()
            .position(|(_, weight)| {
                choice -= weight;
                choice < 0
            })
            .expect("a draw below the total lands on an entry");
        let (selected, weight) = options[index];
        if try_start(selected) {
            return Some(selected);
        }
        options.remove(index);
        total -= weight;
    }
    None
}

// Math.round(double): ties go toward +∞, and `floor(v + 0.5)` is not it —
// that rounds 0.49999999999999994 up.
fn java_round(value: f64) -> i32 {
    let floor = value.floor();
    (if value - floor >= 0.5 {
        floor + 1.0
    } else {
        floor
    }) as i32
}

pub fn ring_positions(
    seed: i64,
    distance: i32,
    spread: i32,
    count: i32,
    mut find: impl FnMut(i32, i32, &mut LegacyRandom) -> Option<(i32, i32)>,
) -> Vec<(i32, i32)> {
    let mut rng = LegacyRandom::new(seed as u64);
    let mut angle = rng.next_f64() * PI * 2.0;
    let mut position_in_circle = 0;
    let mut circle = 0;
    let mut spread = spread;
    let mut positions = Vec::with_capacity(count as usize);
    for i in 0..count {
        let dist = (4 * distance + distance * circle * 6) as f64
            + (rng.next_f64() - 0.5) * (distance as f64 * 2.5);
        let initial_x = java_round(angle.cos() * dist);
        let initial_z = java_round(angle.sin() * dist);
        let mut fork = rng.fork();
        positions.push(find(initial_x, initial_z, &mut fork).unwrap_or((initial_x, initial_z)));
        angle += PI * 2.0 / spread as f64;
        position_in_circle += 1;
        if position_in_circle == spread {
            circle += 1;
            position_in_circle = 0;
            spread += 2 * spread / (circle + 1);
            spread = spread.min(count - i);
            angle += rng.next_f64() * PI * 2.0;
        }
    }
    positions
}

pub fn scan_biome_window(
    initial_x: i32,
    initial_z: i32,
    fork: &mut LegacyRandom,
    admits: impl FnOnce(i32, i32, i32) -> Vec<bool>,
) -> Option<(i32, i32)> {
    let side = 2 * BIOME_SEARCH_QUARTS + 1;
    let first_x = (((initial_x << 4) + 8) >> 2) - BIOME_SEARCH_QUARTS;
    let first_z = (((initial_z << 4) + 8) >> 2) - BIOME_SEARCH_QUARTS;
    let cells = admits(first_x, first_z, side);
    debug_assert_eq!(cells.len(), (side * side) as usize);
    let mut result = None;
    let mut found = 0;
    for row in 0..side {
        for column in 0..side {
            if !cells[(row * side + column) as usize] {
                continue;
            }
            if result.is_none() || fork.next_i32_bound(found + 1) == 0 {
                result = Some((first_x + column, first_z + row));
            }
            found += 1;
        }
    }
    result.map(|(quart_x, quart_z)| ((quart_x << 2) >> 4, (quart_z << 2) >> 4))
}

pub fn fixed_biome_window(initial_x: i32, initial_z: i32, fork: &mut LegacyRandom) -> (i32, i32) {
    let radius = BIOME_SEARCH_QUARTS << 2;
    let x = (initial_x << 4) + 8 - radius + fork.next_i32_bound(radius * 2 + 1);
    let z = (initial_z << 4) + 8 - radius + fork.next_i32_bound(radius * 2 + 1);
    (x >> 4, z >> 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: i64 = -4_172_144_997_902_289_642;

    fn linear(spacing: i32, separation: i32, spread_type: SpreadType) -> SpreadPlacement {
        SpreadPlacement {
            salt: 14357617,
            frequency: 1.0,
            reduction: FrequencyReduction::Default,
            exclusion_chunks: None,
            spacing,
            separation,
            spread_type,
        }
    }

    #[test]
    fn a_negative_coordinate_rounds_its_cell_toward_negative_infinity() {
        let placement = linear(34, 8, SpreadType::Linear);
        let (x, z) = placement.potential_chunk(SEED, -1, -35);
        assert!((-34..-8).contains(&x), "{x}");
        assert!((-68..-42).contains(&z), "{z}");
        for (other_x, other_z) in [(-34, -68), (-9, -35), (-20, -50)] {
            assert_eq!(placement.potential_chunk(SEED, other_x, other_z), (x, z));
        }
        assert_ne!(placement.potential_chunk(SEED, 0, 0), (x, z));
    }

    #[test]
    fn a_triangular_spread_is_the_integer_mean_of_two_linear_draws() {
        let placement = linear(20, 11, SpreadType::Triangular);
        let mut replay = LegacyRandom::large_feature_with_salt(SEED, 3, -2, placement.salt);
        let (a, b, c, d) = (
            replay.next_i32_bound(9),
            replay.next_i32_bound(9),
            replay.next_i32_bound(9),
            replay.next_i32_bound(9),
        );
        assert_eq!(
            placement.potential_chunk(SEED, 65, -40),
            (60 + (a + b) / 2, -40 + (c + d) / 2)
        );
    }

    #[test]
    fn a_start_chunk_is_the_one_its_cell_names() {
        let placement = linear(34, 8, SpreadType::Linear);
        let (x, z) = placement.potential_chunk(SEED, 100, 100);
        assert!(placement.is_structure_chunk(SEED, x, z, |_, _| false));
        assert!(!placement.is_structure_chunk(SEED, x + 1, z, |_, _| false));
        let neighbour = SpreadPlacement {
            exclusion_chunks: Some(2),
            ..placement
        };
        assert!(!neighbour.is_structure_chunk(SEED, x, z, |tx, tz| (tx, tz) == (x - 2, z + 2)));
        assert!(neighbour.is_structure_chunk(SEED, x, z, |tx, tz| (tx, tz) == (x - 3, z)));
    }

    #[test]
    fn a_full_frequency_never_draws() {
        for method in [
            FrequencyReduction::Default,
            FrequencyReduction::LegacyType1,
            FrequencyReduction::LegacyType2,
            FrequencyReduction::LegacyType3,
        ] {
            assert!(frequency_gate(SEED, 1, 1.0, method, 7, -3));
        }
    }

    #[test]
    fn the_pillager_outpost_gate_keys_on_the_region() {
        let (region_x, region_z) = (7 >> 4, -3 >> 4);
        let mut rng = LegacyRandom::new(((region_x ^ (region_z << 4)) as i64 ^ SEED) as u64);
        rng.next_i32();
        let expected = rng.next_i32_bound(5) == 0;
        assert_eq!(
            frequency_gate(SEED, 1, 0.2, FrequencyReduction::LegacyType1, 7, -3),
            expected
        );
    }

    #[test]
    fn removal_shifts_the_draw_to_the_remaining_entries() {
        let entries = [('a', 3), ('b', 5), ('c', 2)];
        let mut tried = Vec::new();
        let picked = select_with_removal(SEED, 4, 9, &entries, |entry| {
            tried.push(entry);
            false
        });
        assert_eq!(picked, None);
        let last = *tried.last().unwrap();
        tried.sort_unstable();
        assert_eq!(tried, ['a', 'b', 'c']);

        let mut tried = Vec::new();
        let picked = select_with_removal(SEED, 4, 9, &entries, |entry| {
            tried.push(entry);
            entry == last
        });
        assert_eq!(picked, Some(last));
        assert_eq!(tried.len(), 3);

        let mut tried = Vec::new();
        assert_eq!(
            select_with_removal(SEED, 4, 9, &entries[..1], |entry| {
                tried.push(entry);
                true
            }),
            Some('a')
        );
        assert_eq!(tried, ['a']);
    }

    #[test]
    fn rings_grow_by_two_thirds_and_the_last_takes_what_remains() {
        let positions = ring_positions(SEED, 32, 3, 128, |_, _, _| None);
        assert_eq!(positions.len(), 128);
        let mut per_ring = [0usize; 8];
        for (x, z) in positions {
            let radius = ((x * x + z * z) as f64).sqrt();
            let ring = ((radius - 4.0 * 32.0) / (6.0 * 32.0)).round();
            assert!((0.0..8.0).contains(&ring), "({x}, {z}) radius {radius}");
            let centre = 4.0 * 32.0 + 6.0 * 32.0 * ring;
            assert!(
                (radius - centre).abs() <= 1.25 * 32.0 + 1.5,
                "({x}, {z}) radius {radius} ring {ring}"
            );
            per_ring[ring as usize] += 1;
        }
        assert_eq!(per_ring, [3, 6, 10, 15, 21, 28, 36, 9]);
    }

    #[test]
    fn a_snapped_ring_position_is_the_reservoir_pick_in_chunks() {
        let mut window = (0, 0);
        let positions = ring_positions(SEED, 32, 3, 1, |x, z, fork| {
            scan_biome_window(x, z, fork, |first_x, first_z, side| {
                window = (first_x, first_z);
                (0..side * side).map(|cell| cell == 3 * side + 10).collect()
            })
        });
        let (quart_x, quart_z) = (window.0 + 10, window.1 + 3);
        assert_eq!(positions, vec![((quart_x << 2) >> 4, (quart_z << 2) >> 4)]);
    }

    #[test]
    fn a_fixed_source_draws_one_block_offset_per_axis() {
        let mut replay = LegacyRandom::new(SEED as u64);
        let angle = replay.next_f64() * PI * 2.0;
        let dist = 4.0 * 32.0 + (replay.next_f64() - 0.5) * 80.0;
        let (ix, iz) = (
            java_round(angle.cos() * dist),
            java_round(angle.sin() * dist),
        );
        let mut fork = replay.fork();
        let (dx, dz) = (fork.next_i32_bound(225), fork.next_i32_bound(225));
        let positions = ring_positions(SEED, 32, 3, 1, |x, z, fork| {
            Some(fixed_biome_window(x, z, fork))
        });
        assert_eq!(
            positions,
            vec![(
                ((ix << 4) + 8 - 112 + dx) >> 4,
                ((iz << 4) + 8 - 112 + dz) >> 4
            )]
        );
    }

    #[test]
    fn java_round_breaks_ties_upward() {
        assert_eq!(java_round(-2.5), -2);
        assert_eq!(java_round(2.5), 3);
        assert_eq!(java_round(-2.6), -3);
        assert_eq!(java_round(0.49999999999999994), 0);
    }
}
