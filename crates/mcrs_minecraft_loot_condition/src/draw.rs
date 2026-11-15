use mcrs_minecraft_random::Random;

/// The draw `B(bound)` of LOOT-RND-006: `None`, with no draw, for a bound outside `1..2³¹ − 1`.
pub fn bounded<R: Random>(random: &mut R, bound: i64) -> Option<i32> {
    let bound = i32::try_from(bound).ok().filter(|&bound| bound >= 1)?;
    let value = random.next_u32_bound(bound.cast_unsigned());
    i32::try_from(value).ok()
}

/// The draw `between(low, high)` of LOOT-RND-006: `low` with no draw when `low >= high`, and
/// `None` with no draw when `high − low + 1` is outside `1..2³¹ − 1`.
pub fn between<R: Random>(random: &mut R, low: i32, high: i32) -> Option<i32> {
    if low >= high {
        return Some(low);
    }
    let offset = bounded(random, i64::from(high) - i64::from(low) + 1)?;
    low.checked_add(offset)
}

/// The number of `trials` draws `F` that are less than `chance`; no draw when `trials ≤ 0`.
pub fn binomial<R: Random>(random: &mut R, trials: i32, chance: f32) -> i32 {
    (0..trials)
        .map(|_| i32::from(random.next_f32() < chance))
        .sum()
}
