use mcrs_minecraft_random::Random;

/// `Mth.nextInt`, which — unlike `randomBetweenInclusive` — spends no draw on a
/// degenerate range. The height providers rely on that to keep their draw counts.
pub(crate) fn next_int_guarded<R: Random>(rng: &mut R, min: i32, max_inclusive: i32) -> i32 {
    if min >= max_inclusive {
        min
    } else {
        rng.next_int_between_inclusive(min, max_inclusive)
    }
}

pub(crate) fn normal<R: Random>(rng: &mut R, mean: f32, deviation: f32) -> f32 {
    mean + rng.next_gaussian() as f32 * deviation
}
