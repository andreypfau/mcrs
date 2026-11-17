use crate::{face_bit, holds};
use mcrs_minecraft_random::{shuffle, shuffled};
use rustc_hash::FxHashMap as HashMap;

use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_worldgen_feature::placer::{StateMask, WorldGenVolume};
use mcrs_voxel::BlockPos;

use crate::tree::trunk::all_shuffled;

#[derive(Clone, Debug)]
pub struct MultifaceStates {
    pub by_faces: [[VoxelId; 64]; 2],
    pub of_state: HashMap<u16, (u8, bool)>,
}

impl MultifaceStates {
    pub fn faces_of(&self, state: VoxelId) -> Option<(u8, bool)> {
        self.of_state.get(&state.0).copied()
    }

    pub fn state_with(&self, faces: u8, waterlogged: bool) -> Option<VoxelId> {
        let [dry, wet] = &self.by_faces;
        let table = if waterlogged { wet } else { dry };
        table.get(usize::from(faces)).copied()
    }

    fn with_face(&self, faces: u8, waterlogged: bool, direction: Direction) -> Option<VoxelId> {
        self.state_with(faces | face_bit(direction), waterlogged)
    }

    fn has_face(&self, state: VoxelId, direction: Direction) -> bool {
        self.faces_of(state)
            .is_some_and(|(faces, _)| faces & face_bit(direction) != 0)
    }

    pub fn state_for_placement<W: WorldGenVolume>(
        &self,
        volume: &W,
        old_state: VoxelId,
        pos: BlockPos,
        direction: Direction,
    ) -> Option<VoxelId> {
        if self.has_face(old_state, direction) {
            return None;
        }
        if !volume.holds(&volume.world().sturdy_up, pos + direction.normal()) {
            return None;
        }
        let (faces, waterlogged) = self
            .faces_of(old_state)
            .unwrap_or((0, holds(&volume.world().water_source, old_state)));
        self.with_face(faces, waterlogged, direction)
    }
}

#[derive(Clone, Debug)]
pub struct CompiledMultifaceGrowth {
    pub states: MultifaceStates,
    pub valid_directions: Vec<Direction>,
    pub chance_of_spreading: f32,
    pub can_be_placed_on: StateMask,
}

pub fn valid_directions(ceiling: bool, floor: bool, wall: bool) -> Vec<Direction> {
    let mut directions = Vec::with_capacity(6);
    if ceiling {
        directions.push(Direction::Up);
    }
    if floor {
        directions.push(Direction::Down);
    }
    if wall {
        directions.extend(Direction::HORIZONTAL);
    }
    directions
}
pub fn place_multiface_growth<W: WorldGenVolume>(
    cfg: &CompiledMultifaceGrowth,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    at: BlockPos,
) -> bool {
    let origin_state = volume.get(at);
    if !is_air_or_water(volume, origin_state) {
        return false;
    }

    let search_order = shuffled(&cfg.valid_directions, rng);
    if place_growth_if_possible(cfg, volume, rng, at, origin_state, &search_order) {
        return true;
    }

    for search in &search_order {
        let mut placement: Vec<Direction> = cfg
            .valid_directions
            .iter()
            .copied()
            .filter(|direction| *direction != search.opposite())
            .collect();
        shuffle(&mut placement, rng);

        let pos = at + search.normal();
        let state = volume.get(pos);
        if !is_air_or_water(volume, state) && cfg.states.faces_of(state).is_none() {
            continue;
        }
        if place_growth_if_possible(cfg, volume, rng, pos, state, &placement) {
            return true;
        }
    }
    false
}

fn is_air_or_water<W: WorldGenVolume>(volume: &W, state: VoxelId) -> bool {
    let world = volume.world();
    holds(&world.air_states, state) || holds(&world.water_states, state)
}

fn place_growth_if_possible<W: WorldGenVolume>(
    cfg: &CompiledMultifaceGrowth,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    pos: BlockPos,
    old_state: VoxelId,
    placement_directions: &[Direction],
) -> bool {
    for direction in placement_directions {
        let neighbour = pos + direction.normal();
        if !volume.holds(&cfg.can_be_placed_on, neighbour) {
            continue;
        }
        let Some(new_state) = cfg
            .states
            .state_for_placement(volume, old_state, pos, *direction)
        else {
            return false;
        };
        volume.set(pos, new_state);
        if rng.next_f32() < cfg.chance_of_spreading {
            spread_from_face(cfg, volume, rng, new_state, pos, *direction);
        }
        return true;
    }
    false
}

fn spread_from_face<W: WorldGenVolume>(
    cfg: &CompiledMultifaceGrowth,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    state: VoxelId,
    pos: BlockPos,
    from_face: Direction,
) {
    for spread in all_shuffled(rng) {
        if spread_toward(cfg, volume, state, pos, from_face, spread) {
            return;
        }
    }
}

pub(crate) fn spread_positions(
    pos: BlockPos,
    spread: Direction,
    from_face: Direction,
) -> [(BlockPos, Direction); 3] {
    let along = pos + spread.normal();
    [
        (pos, spread),
        (along, from_face),
        (along + from_face.normal(), spread.opposite()),
    ]
}

fn spread_toward<W: WorldGenVolume>(
    cfg: &CompiledMultifaceGrowth,
    volume: &mut W,
    state: VoxelId,
    pos: BlockPos,
    from_face: Direction,
    spread: Direction,
) -> bool {
    if spread.axis() == from_face.axis() {
        return false;
    }
    if !cfg.states.has_face(state, from_face) || cfg.states.has_face(state, spread) {
        return false;
    }

    for (target, face) in spread_positions(pos, spread, from_face) {
        let existing = volume.get(target);
        let replaceable = holds(&volume.world().air_states, existing)
            || cfg.states.faces_of(existing).is_some()
            || existing == volume.world().water;
        if !replaceable {
            continue;
        }
        let Some(new_state) = cfg
            .states
            .state_for_placement(volume, existing, target, face)
        else {
            continue;
        };
        volume.set(target, new_state);
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_worldgen_feature::placer::mask_of;

    use mcrs_minecraft_random::worldgen::WorldgenRandom;

    use super::*;
    use crate::tree::provider::fake::FakeVolume;

    const STONE: VoxelId = VoxelId(1);
    const WATER: VoxelId = VoxelId(2);
    const FLOWING: VoxelId = VoxelId(3);
    const GROWTH: u16 = 100;
    const AT: BlockPos = BlockPos::new(0, 40, 0);

    fn growth(faces: u8, waterlogged: bool) -> VoxelId {
        VoxelId(GROWTH + 64 * u16::from(waterlogged) + u16::from(faces))
    }

    fn states() -> MultifaceStates {
        let mut by_faces = [[VoxelId(0); 64]; 2];
        let mut of_state = HashMap::default();
        for waterlogged in [false, true] {
            for faces in 0..64u8 {
                let state = growth(faces, waterlogged);
                by_faces[usize::from(waterlogged)][usize::from(faces)] = state;
                of_state.insert(state.0, (faces, waterlogged));
            }
        }
        MultifaceStates { by_faces, of_state }
    }

    fn cave(volume: FakeVolume) -> FakeVolume {
        let mut volume = volume;
        volume.world.sturdy_up = mask_of([STONE]);
        volume.world.water = WATER;
        volume.world.water_states = mask_of([WATER, FLOWING]);
        volume.world.water_fluid = mask_of([WATER, FLOWING]);
        volume.world.water_source = mask_of([WATER]);
        volume
    }

    fn config() -> CompiledMultifaceGrowth {
        CompiledMultifaceGrowth {
            states: states(),
            valid_directions: valid_directions(true, false, true),
            chance_of_spreading: 0.5,
            can_be_placed_on: mask_of([STONE]),
        }
    }

    #[test]
    fn the_valid_directions_are_the_ceiling_then_the_floor_then_the_plane() {
        assert_eq!(
            valid_directions(true, true, true),
            vec![
                Direction::Up,
                Direction::Down,
                Direction::North,
                Direction::East,
                Direction::South,
                Direction::West,
            ]
        );
        assert_eq!(valid_directions(false, true, false), vec![Direction::Down]);
    }

    #[test]
    fn a_growth_on_a_ceiling_shuffles_its_faces_then_spends_the_spread_draws() {
        let cfg = config();
        let mut volume = cave(FakeVolume::with([((AT.x, AT.y + 1, AT.z), STONE)]));
        let mut rng = WorldgenRandom::new(31);

        assert!(place_multiface_growth(&cfg, &mut volume, &mut rng, AT));

        let mut replay = WorldgenRandom::new(31);
        for size in (2..=5).rev() {
            replay.next_i32_bound(size);
        }
        let spreads = replay.next_f32() < cfg.chance_of_spreading;
        if spreads {
            for size in (2..=6).rev() {
                replay.next_i32_bound(size);
            }
        }
        assert_eq!(rng, replay, "four shuffle draws, one float, five more");

        assert_eq!(
            volume.writes[0],
            ((AT.x, AT.y, AT.z), growth(1 << Direction::Up as u8, false)),
            "the lichen takes the face it found stone on"
        );
    }

    #[test]
    fn a_growth_with_no_neighbour_writes_nothing_and_spends_the_shuffles() {
        let cfg = config();
        let mut volume = cave(FakeVolume::default());
        let mut rng = WorldgenRandom::new(5);

        assert!(!place_multiface_growth(&cfg, &mut volume, &mut rng, AT));

        let mut replay = WorldgenRandom::new(5);
        for size in (2..=5).rev() {
            replay.next_i32_bound(size);
        }
        let order = shuffled(&cfg.valid_directions, &mut WorldgenRandom::new(5));
        for search in &order {
            let kept = cfg
                .valid_directions
                .iter()
                .filter(|direction| **direction != search.opposite())
                .count();
            for size in (2..=kept).rev() {
                replay.next_i32_bound(size as i32);
            }
        }
        assert_eq!(rng, replay, "one shuffle per search direction");
        assert!(volume.writes.is_empty());
    }

    #[test]
    fn a_growth_in_water_is_waterlogged_only_by_a_source() {
        for (water, waterlogged) in [(WATER, true), (FLOWING, false)] {
            let mut cfg = config();
            cfg.chance_of_spreading = 0.0;
            let mut volume = cave(FakeVolume::with([
                ((AT.x, AT.y, AT.z), water),
                ((AT.x, AT.y + 1, AT.z), STONE),
            ]));
            let mut rng = WorldgenRandom::new(31);

            assert!(place_multiface_growth(&cfg, &mut volume, &mut rng, AT));
            assert_eq!(
                volume.writes[0],
                (
                    (AT.x, AT.y, AT.z),
                    growth(1 << Direction::Up as u8, waterlogged)
                ),
                "{water:?}"
            );
        }
    }

    #[test]
    fn a_spread_stays_off_the_axis_of_the_face_it_grew_on() {
        let cfg = config();
        let state = growth(1 << Direction::Up as u8, false);
        let mut volume = cave(FakeVolume::with([
            ((AT.x, AT.y, AT.z), state),
            ((AT.x, AT.y + 1, AT.z), STONE),
            ((AT.x, AT.y, AT.z - 1), STONE),
        ]));

        assert!(!spread_toward(
            &cfg,
            &mut volume,
            state,
            AT,
            Direction::Up,
            Direction::Down
        ));
        assert!(volume.writes.is_empty());

        assert!(spread_toward(
            &cfg,
            &mut volume,
            state,
            AT,
            Direction::Up,
            Direction::North
        ));
        assert_eq!(
            volume.writes,
            vec![(
                (AT.x, AT.y, AT.z),
                growth(
                    1 << Direction::Up as u8 | 1 << Direction::North as u8,
                    false
                )
            )],
            "SAME_POSITION comes first, so the north face joins the same cell"
        );
    }
}
