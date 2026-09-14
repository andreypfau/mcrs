use bevy_math::IVec3;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;
use mcrs_minecraft_worldgen::feature::placer::WorldGenVolume;

use crate::feature::tree::trunk::random_horizontal;
use mcrs_minecraft_random::shuffle;

fn counter_clockwise(direction: Direction) -> Direction {
    direction.clockwise().opposite()
}

/// `CoralTreeFeature.place`: a trunk of one to three blocks, then two to four
/// branches that step outward as they climb.
///
/// `place_block` runs the nested placed feature — its own modifier chain over
/// the same random source — and a refusal ends whatever run it is in.
pub fn place_coral_tree<W>(
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    origin: IVec3,
    place_block: &mut dyn FnMut(&mut W, &mut XoroshiroRandom, IVec3) -> bool,
) -> bool
where
    W: WorldGenVolume,
{
    let trunk_height = rng.next_i32_bound(3) + 1;
    let mut top = origin;
    for _ in 0..trunk_height {
        if !place_block(volume, rng, top) {
            return true;
        }
        top += IVec3::Y;
    }

    let branches = rng.next_i32_bound(3) + 2;
    let mut directions = Direction::HORIZONTAL;
    shuffle(&mut directions, rng);

    for direction in &directions[..branches as usize] {
        let mut pos = top + direction.normal();
        let branch_height = rng.next_i32_bound(5) + 2;
        let mut segment = 0;
        for rung in 0..branch_height {
            if !place_block(volume, rng, pos) {
                break;
            }
            segment += 1;
            pos += IVec3::Y;
            if rung == 0 || (segment >= 2 && rng.next_f32() < 0.25) {
                pos += direction.normal();
                segment = 0;
            }
        }
    }
    true
}

/// `CoralClawFeature.place`: two or three branches out of the origin, each
/// running sideways before it curls back in along the claw's own direction.
pub fn place_coral_claw<W>(
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    origin: IVec3,
    place_block: &mut dyn FnMut(&mut W, &mut XoroshiroRandom, IVec3) -> bool,
) -> bool
where
    W: WorldGenVolume,
{
    if !place_block(volume, rng, origin) {
        return false;
    }
    let claw = random_horizontal(rng);
    let branches = rng.next_i32_bound(2) + 2;
    let mut directions = [claw, claw.clockwise(), counter_clockwise(claw)];
    shuffle(&mut directions, rng);

    for direction in &directions[..branches as usize] {
        let sideways = rng.next_i32_bound(2) + 1;
        let mut pos = origin + direction.normal();
        let (segment_direction, inward) = if *direction == claw {
            (claw, rng.next_i32_bound(3) + 2)
        } else {
            pos += IVec3::Y;
            let choices = [*direction, Direction::Up];
            let picked = choices[rng.next_i32_bound(choices.len() as i32) as usize];
            (picked, rng.next_i32_bound(3) + 3)
        };

        for _ in 0..sideways {
            if !place_block(volume, rng, pos) {
                break;
            }
            pos += segment_direction.normal();
        }
        pos += segment_direction.opposite().normal() + IVec3::Y;

        for _ in 0..inward {
            pos += claw.normal();
            if !place_block(volume, rng, pos) {
                break;
            }
            if rng.next_f32() < 0.25 {
                pos += IVec3::Y;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    use super::*;
    use crate::feature::tree::provider::fake::FakeVolume;

    const AT: IVec3 = IVec3::new(0, 60, 0);

    /// The trunk height comes first, and the trunk is offered to the nested
    /// feature one cell at a time going up.
    #[test]
    fn a_coral_tree_offers_its_trunk_upward_from_the_origin() {
        let mut seen: Vec<IVec3> = Vec::new();
        let mut volume = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(3);

        assert!(place_coral_tree(
            &mut volume,
            &mut rng,
            AT,
            &mut |_, _, pos| {
                seen.push(pos);
                true
            }
        ));

        let trunk = XoroshiroRandom::new(3).next_i32_bound(3) + 1;
        assert_eq!(
            seen[..trunk as usize],
            (0..trunk).map(|y| AT + IVec3::Y * y).collect::<Vec<_>>()[..],
        );
    }

    /// A nested feature that refuses the first trunk block ends the run: the
    /// branch count, the shuffle and every branch draw never happen.
    #[test]
    fn a_refused_trunk_stops_the_tree_after_one_placement() {
        let mut seen: Vec<IVec3> = Vec::new();
        let mut volume = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(3);

        assert!(place_coral_tree(
            &mut volume,
            &mut rng,
            AT,
            &mut |_, _, pos| {
                seen.push(pos);
                false
            }
        ));

        let mut replay = XoroshiroRandom::new(3);
        replay.next_i32_bound(3);
        assert_eq!(rng, replay, "only the trunk height was drawn");
        assert_eq!(seen, vec![AT]);
    }

    /// A claw whose first placement is refused places nothing else and spends
    /// no draw of its own.
    #[test]
    fn a_refused_origin_stops_the_claw_before_it_draws() {
        let mut seen: Vec<IVec3> = Vec::new();
        let mut volume = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(5);
        let before = rng.clone();

        assert!(!place_coral_claw(
            &mut volume,
            &mut rng,
            AT,
            &mut |_, _, pos| {
                seen.push(pos);
                false
            }
        ));

        assert_eq!(rng, before);
        assert_eq!(seen, vec![AT]);
    }

    /// The claw draws its direction, its branch count and the three-way shuffle
    /// before it walks any branch, and each branch then spends its sideways
    /// length, its segment direction where it has a choice, and its reach.
    #[test]
    fn a_coral_claw_walks_every_branch_in_shuffled_order() {
        let mut seen: Vec<IVec3> = Vec::new();
        let mut volume = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(5);

        assert!(place_coral_claw(
            &mut volume,
            &mut rng,
            AT,
            &mut |_, _, pos| {
                seen.push(pos);
                pos == AT
            }
        ));

        let mut replay = XoroshiroRandom::new(5);
        let claw = Direction::HORIZONTAL[replay.next_i32_bound(4) as usize];
        let branches = replay.next_i32_bound(2) + 2;
        let mut directions = [claw, claw.clockwise(), counter_clockwise(claw)];
        shuffle(&mut directions, &mut replay);

        // Every branch is refused at its first sideways cell and again at its
        // first inward cell, so each offers exactly two positions.
        let mut expected = vec![AT];
        for direction in &directions[..branches as usize] {
            replay.next_i32_bound(2);
            let mut pos = AT + direction.normal();
            let segment_direction = if *direction == claw {
                replay.next_i32_bound(3);
                claw
            } else {
                pos += IVec3::Y;
                let choices = [*direction, Direction::Up];
                let picked = choices[replay.next_i32_bound(2) as usize];
                replay.next_i32_bound(3);
                picked
            };
            expected.push(pos);
            pos += segment_direction.opposite().normal() + IVec3::Y + claw.normal();
            expected.push(pos);
        }
        assert_eq!(seen, expected);
        assert_eq!(rng, replay);
    }
}
