use crate::feature::{face_bit, holds};
use mcrs_minecraft_random::{shuffle, shuffled};
use rustc_hash::FxHashMap as HashMap;

use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;
use mcrs_minecraft_worldgen::feature::placer::{StateMask, WorldGenVolume};

use crate::feature::tree::trunk::all_shuffled;

/// The growth block's own states, as the six face bits in `Direction.values()`
/// order, with `waterlogged` selecting the second table.
#[derive(Clone, Debug)]
pub struct MultifaceStates {
    pub by_faces: [[VoxelId; 64]; 2],
    pub of_state: HashMap<u16, (u8, bool)>,
}

impl MultifaceStates {
    pub fn faces_of(&self, state: VoxelId) -> Option<(u8, bool)> {
        self.of_state.get(&state.0).copied()
    }

    fn with_face(&self, faces: u8, waterlogged: bool, direction: Direction) -> VoxelId {
        self.by_faces[usize::from(waterlogged)][usize::from(faces | face_bit(direction))]
    }

    fn has_face(&self, state: VoxelId, direction: Direction) -> bool {
        self.faces_of(state)
            .is_some_and(|(faces, _)| faces & face_bit(direction) != 0)
    }

    /// `MultifaceBlock.getStateForPlacement`: a face added to whatever is there,
    /// waterlogged when it displaces a water source.
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
        // `MultifaceBlock.canAttachTo` as the full-collision-cube flag; the upgrade
        // path is a per-face table over the support and collision boxes at freeze.
        if !volume.holds(&volume.world().sturdy_up, pos + direction.normal()) {
            return None;
        }
        let (faces, waterlogged) = self
            .faces_of(old_state)
            .unwrap_or((0, holds(&volume.world().water_source, old_state)));
        Some(self.with_face(faces, waterlogged, direction))
    }
}

/// One `multiface_growth` feature with every name it carries already resolved.
#[derive(Clone, Debug)]
pub struct CompiledMultifaceGrowth {
    pub states: MultifaceStates,
    /// The faces the configuration allows, in the reference's order: the
    /// ceiling, the floor, then the horizontal plane. Build it with
    /// [`valid_directions`], since the order is what the shuffle permutes.
    pub valid_directions: Vec<Direction>,
    pub chance_of_spreading: f32,
    pub can_be_placed_on: StateMask,
}

/// `MultifaceGrowthFeature.validDirections`, whose order the shuffles permute.
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
///
/// The reference's search loop never advances its cursor: it re-reads
/// `origin.relative(searchDirection)` on every one of the `search_range`
/// rounds. A round that places anything returns at once and a round that does
/// not draws nothing and writes nothing, so the repetition is unobservable and
/// `search_range` reaches the world through nothing at all.
pub fn place_multiface_growth<W: WorldGenVolume>(
    cfg: &CompiledMultifaceGrowth,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
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
    holds(&world.air_states, state) || holds(&world.water_fluid, state)
}

fn place_growth_if_possible<W: WorldGenVolume>(
    cfg: &CompiledMultifaceGrowth,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
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

/// `MultifaceSpreader.spreadFromFaceTowardRandomDirection`: all six directions
/// shuffled, whatever the configuration allows, and the first that reaches a
/// cell it may grow into takes it.
fn spread_from_face<W: WorldGenVolume>(
    cfg: &CompiledMultifaceGrowth,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
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

/// The three spread types in `DEFAULT_SPREAD_ORDER`, as the cell and the face
/// each would grow on.
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
            || holds(&volume.world().water_source, existing);
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
    use mcrs_minecraft_worldgen::feature::placer::mask_of;

    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    use super::*;
    use crate::feature::tree::provider::fake::FakeVolume;

    const STONE: VoxelId = VoxelId(1);
    const WATER: VoxelId = VoxelId(2);
    /// The growth's states start here: `[waterlogged][faces]`, so a state id is
    /// `GROWTH + 64 * waterlogged + faces`.
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

    /// Glow lichen: the ceiling and the walls, never the floor, and half the
    /// placements spread on.
    /// Stone holds a growth up; water is the one fluid, source everywhere.
    fn cave(volume: FakeVolume) -> FakeVolume {
        let mut volume = volume;
        volume.world.sturdy_up = mask_of([STONE]);
        volume.world.water_fluid = mask_of([WATER]);
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

    /// The shuffle of the five valid faces, then a float for the spread, then
    /// the six shuffled again inside the spreader.
    #[test]
    fn a_growth_on_a_ceiling_shuffles_its_faces_then_spends_the_spread_draws() {
        let cfg = config();
        let mut volume = cave(FakeVolume::with([((AT.x, AT.y + 1, AT.z), STONE)]));
        let mut rng = XoroshiroRandom::new(31);

        assert!(place_multiface_growth(&cfg, &mut volume, &mut rng, AT));

        let mut replay = XoroshiroRandom::new(31);
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

    /// A cell with nothing to hold on to leaves the world alone, having spent
    /// its own shuffle and one shuffle for every search direction.
    #[test]
    fn a_growth_with_no_neighbour_writes_nothing_and_spends_the_shuffles() {
        let cfg = config();
        let mut volume = cave(FakeVolume::default());
        let mut rng = XoroshiroRandom::new(5);

        assert!(!place_multiface_growth(&cfg, &mut volume, &mut rng, AT));

        let mut replay = XoroshiroRandom::new(5);
        for size in (2..=5).rev() {
            replay.next_i32_bound(size);
        }
        let order = shuffled(&cfg.valid_directions, &mut XoroshiroRandom::new(5));
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

    /// A growth that displaces a water source waterlogs itself.
    #[test]
    fn a_growth_in_water_takes_the_waterlogged_state() {
        let mut cfg = config();
        cfg.chance_of_spreading = 0.0;
        let mut volume = cave(FakeVolume::with([
            ((AT.x, AT.y, AT.z), WATER),
            ((AT.x, AT.y + 1, AT.z), STONE),
        ]));
        let mut rng = XoroshiroRandom::new(31);

        assert!(place_multiface_growth(&cfg, &mut volume, &mut rng, AT));
        assert_eq!(
            volume.writes[0],
            ((AT.x, AT.y, AT.z), growth(1 << Direction::Up as u8, true))
        );
    }

    /// The spreader never grows along the axis of the face it came from, and it
    /// takes the first of the three spread types that fits.
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
