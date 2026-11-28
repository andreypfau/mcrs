use glam::IVec3;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_core::Rotation;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_worldgen_feature::placer::WorldGenVolume;
use mcrs_voxel::BlockPos;

use crate::tree::trunk::random_horizontal;
use mcrs_minecraft_random::shuffle;

pub fn place_coral_tree<W>(
    volume: &mut W,
    rng: &mut WorldgenRandom,
    origin: BlockPos,
    mut place_block: impl FnMut(&mut W, &mut WorldgenRandom, BlockPos) -> bool,
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

    for direction in directions.iter().take(branches as usize) {
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

pub fn place_coral_claw<W>(
    volume: &mut W,
    rng: &mut WorldgenRandom,
    origin: BlockPos,
    mut place_block: impl FnMut(&mut W, &mut WorldgenRandom, BlockPos) -> bool,
) -> bool
where
    W: WorldGenVolume,
{
    if !place_block(volume, rng, origin) {
        return false;
    }
    let claw = random_horizontal(rng);
    let branches = rng.next_i32_bound(2) + 2;
    let mut directions = [
        claw,
        claw.clockwise(),
        Rotation::Counterclockwise90.rotate(claw),
    ];
    shuffle(&mut directions, rng);

    for direction in directions.iter().take(branches as usize) {
        let sideways = rng.next_i32_bound(2) + 1;
        let mut pos = origin + direction.normal();
        let (segment_direction, inward) = if *direction == claw {
            (claw, rng.next_i32_bound(3) + 2)
        } else {
            pos += IVec3::Y;
            let picked = match rng.next_i32_bound(2) {
                0 => *direction,
                _ => Direction::Up,
            };
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
    use mcrs_minecraft_random::worldgen::WorldgenRandom;

    use super::*;
    use crate::tree::provider::fake::FakeVolume;

    const AT: BlockPos = BlockPos::new(0, 60, 0);

    #[test]
    fn a_coral_tree_offers_its_trunk_upward_from_the_origin() {
        let mut seen: Vec<BlockPos> = Vec::new();
        let mut volume = FakeVolume::default();
        let mut rng = WorldgenRandom::new(3);

        assert!(place_coral_tree(&mut volume, &mut rng, AT, |_, _, pos| {
            seen.push(pos);
            true
        }));

        let trunk = WorldgenRandom::new(3).next_i32_bound(3) + 1;
        assert_eq!(
            seen[..trunk as usize],
            (0..trunk).map(|y| AT + IVec3::Y * y).collect::<Vec<_>>()[..],
        );
    }

    #[test]
    fn a_refused_trunk_stops_the_tree_after_one_placement() {
        let mut seen: Vec<BlockPos> = Vec::new();
        let mut volume = FakeVolume::default();
        let mut rng = WorldgenRandom::new(3);

        assert!(place_coral_tree(&mut volume, &mut rng, AT, |_, _, pos| {
            seen.push(pos);
            false
        }));

        let mut replay = WorldgenRandom::new(3);
        replay.next_i32_bound(3);
        assert_eq!(rng, replay, "only the trunk height was drawn");
        assert_eq!(seen, vec![AT]);
    }

    #[test]
    fn a_refused_origin_stops_the_claw_before_it_draws() {
        let mut seen: Vec<BlockPos> = Vec::new();
        let mut volume = FakeVolume::default();
        let mut rng = WorldgenRandom::new(5);
        let before = rng.clone();

        assert!(!place_coral_claw(&mut volume, &mut rng, AT, |_, _, pos| {
            seen.push(pos);
            false
        }));

        assert_eq!(rng, before);
        assert_eq!(seen, vec![AT]);
    }

    #[test]
    fn a_coral_claw_walks_every_branch_in_shuffled_order() {
        let mut seen: Vec<BlockPos> = Vec::new();
        let mut volume = FakeVolume::default();
        let mut rng = WorldgenRandom::new(5);

        assert!(place_coral_claw(&mut volume, &mut rng, AT, |_, _, pos| {
            seen.push(pos);
            pos == AT
        }));

        let mut replay = WorldgenRandom::new(5);
        let claw = Direction::HORIZONTAL[replay.next_i32_bound(4) as usize];
        let branches = replay.next_i32_bound(2) + 2;
        let mut directions = [
            claw,
            claw.clockwise(),
            Rotation::Counterclockwise90.rotate(claw),
        ];
        shuffle(&mut directions, &mut replay);

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
