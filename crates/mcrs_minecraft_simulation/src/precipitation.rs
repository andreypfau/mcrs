use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_voxel::ColumnPos;

/// Each precipitation pick of a column hits with the probability `1 / PRECIPITATION_GATE`.
pub const PRECIPITATION_GATE: u32 = 48;

/// The width in columns of the square tile whose columns draw their precipitation picks
/// together. A tile never crosses the boundary of a region, so its group does not depend on the
/// partition.
pub const PRECIPITATION_TILE: i32 = 4;

/// The trials of one inversion draw. `(47 / 48)^4096` is about `1e-38`, far from the end of
/// the f64 range.
const BINOMIAL_CHUNK: u32 = 4096;

/// A precipitation pick that hit the block column `(x, z)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrecipitationHit {
    pub x: i32,
    pub z: i32,
}

/// Draws the precipitation picks of one group of `columns`, each with `speed` picks, from
/// `random`, into `hits`.
///
/// The group has `columns.len() × speed` slots. The count of hits is binomial over the slots
/// with the probability of one gate, and the hits are distinct slots, uniform among all sets of
/// that size. This is the distribution of one independent gate for each slot, so a column never
/// gets more than `speed` hits. Each hit takes a uniform position in its column.
pub fn precipitation_hits(
    columns: &[ColumnPos],
    speed: u32,
    random: &mut LegacyRandom,
    hits: &mut Vec<PrecipitationHit>,
) {
    let Some(slots) = u32::try_from(columns.len())
        .ok()
        .and_then(|count| count.checked_mul(speed))
    else {
        return;
    };
    let count = binomial(slots, random);
    let mut chosen: Vec<u32> = Vec::with_capacity(count as usize);
    // Floyd's sampling: each set of `count` distinct slots out of `slots` is equally likely.
    for top in slots.saturating_sub(count)..slots {
        let slot = random.next_u32_bound(top.saturating_add(1));
        chosen.push(if chosen.contains(&slot) { top } else { slot });
    }
    chosen.sort_unstable();
    for slot in chosen {
        let Some(column) = slot
            .checked_div(speed)
            .and_then(|index| columns.get(index as usize))
        else {
            continue;
        };
        let x = random.next_u32_bound(16) as i32;
        let z = random.next_u32_bound(16) as i32;
        // A column of the world holds block coordinates that fit an i32.
        hits.push(PrecipitationHit {
            x: column.x.wrapping_mul(16).wrapping_add(x),
            z: column.z.wrapping_mul(16).wrapping_add(z),
        });
    }
}

/// The count of successes of `trials` gates of `1 / PRECIPITATION_GATE`.
fn binomial(trials: u32, random: &mut LegacyRandom) -> u32 {
    let mut left = trials;
    let mut successes = 0u32;
    while left > 0 {
        let chunk = left.min(BINOMIAL_CHUNK);
        left = left.saturating_sub(chunk);
        successes = successes.saturating_add(binomial_by_inversion(chunk, random));
    }
    successes
}

/// Inverts the cumulative distribution at one uniform value.
fn binomial_by_inversion(trials: u32, random: &mut LegacyRandom) -> u32 {
    let hit = 1.0 / f64::from(PRECIPITATION_GATE);
    let miss = 1.0 - hit;
    let mut probability = miss.powi(trials as i32);
    let mut cumulative = probability;
    let uniform = random.next_f64();
    let mut successes = 0;
    while uniform >= cumulative && successes < trials {
        let next = successes.saturating_add(1);
        probability *= hit / miss * f64::from(trials.saturating_sub(successes)) / f64::from(next);
        successes = next;
        cumulative += probability;
    }
    successes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_group_draw_has_the_rate_of_one_gate_per_slot_and_never_more_than_k_hits_per_column() {
        const COLUMNS: i32 = 64;
        const SPEED: u32 = 3;
        const STEPS: u32 = 100_000;
        let columns: Vec<ColumnPos> = (0..COLUMNS).map(|x| ColumnPos::new(x, 0)).collect();
        let mut random = LegacyRandom::new(0x5eed);
        let mut hits = Vec::new();
        let mut per_column = [0u64; COLUMNS as usize];
        for _ in 0..STEPS {
            hits.clear();
            precipitation_hits(&columns, SPEED, &mut random, &mut hits);
            let mut this_step = [0u32; COLUMNS as usize];
            for hit in &hits {
                let column = hit.x.div_euclid(16) as usize;
                this_step[column] += 1;
                per_column[column] += 1;
            }
            assert!(
                this_step.iter().all(|&count| count <= SPEED),
                "SIM-RND-004: a column gets at most k hits in a step"
            );
        }

        let gate = 1.0 / f64::from(PRECIPITATION_GATE);
        let slots = f64::from(STEPS) * f64::from(SPEED);
        let deviation = (slots * gate * (1.0 - gate)).sqrt();
        for (column, &count) in per_column.iter().enumerate() {
            let off = (count as f64 - slots * gate).abs() / deviation;
            assert!(
                off < 5.0,
                "column {column}: {count} hits, {off:.1} deviations"
            );
        }
        let total: u64 = per_column.iter().sum();
        let expected = slots * gate * f64::from(COLUMNS);
        let off = (total as f64 - expected).abs() / (deviation * f64::from(COLUMNS).sqrt());
        let mean = total as f64 / (f64::from(STEPS) * f64::from(COLUMNS));
        assert!(
            off < 5.0,
            "SIM-RND-004: the mean is {mean}, {off:.1} deviations from 3/48"
        );
    }
}
