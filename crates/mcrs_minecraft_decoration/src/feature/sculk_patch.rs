use crate::feature::{face_bit, holds};
use bevy_math::IVec3;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;
use mcrs_minecraft_worldgen::feature::placer::{StateMask, WorldGenVolume};
use mcrs_voxel_storage::VoxelId;

use crate::feature::multiface_growth::{MultifaceStates, spread_positions};
use crate::feature::tree::trunk::all_shuffled;
use mcrs_voxel_math::dist_manhattan;

/// `SculkSpreader.createWorldGenSpreader`, which is the only spreader a feature
/// ever builds.
const GROWTH_SPAWN_COST: i32 = 50;
const NO_GROWTH_RADIUS: i32 = 1;
const CHARGE_DECAY_RATE: i32 = 5;
const ADDITIONAL_DECAY_RATE: i32 = 10;
const MAX_GROWTH_RATE_RADIUS: i32 = 24;
const MAX_CURSORS: usize = 32;
const MAX_CHARGE: i32 = 1000;
const MAX_CURSOR_DISTANCE: i32 = 1024;
const SHRIEKER_PLACEMENT_RATE: i32 = 11;
const GROWTH_INHIBITOR_RANGE: i32 = 4;
/// `ChargeCursor.canMoveToPos`, whose twelve blocks are squared in the xz plane.
const MAX_WORLDGEN_SPREAD_SQ: i32 = 144;

/// One `sculk_patch` feature with every name it carries already resolved.
///
/// `sculk` and `sculk_vein` are the only two blocks carrying a `SculkBehaviour`,
/// so which of the three behaviours a state takes is read off these masks.
#[derive(Clone, Debug)]
pub struct CompiledSculkPatch {
    pub charge_count: i32,
    pub amount_per_charge: i32,
    pub spread_attempts: i32,
    pub growth_rounds: i32,
    pub spread_rounds: i32,
    /// `sculk_vein`'s own states; a state outside this table is not one.
    pub vein: MultifaceStates,
    pub sculk: VoxelId,
    pub sculk_states: StateMask,
    /// `SculkVeinSpreaderConfig.stateCanBeReplaced`: what a vein will not grow
    /// against.
    pub blocks_vein: StateMask,
    pub fire: StateMask,
    pub replaceable_world_gen: StateMask,
    /// `#sculk_replaceable`, which is what `hasSubstrateAccess` asks for.
    pub substrate: StateMask,
    pub growth_inhibitors: StateMask,
    /// `SculkBlock.getRandomGrowthState`, waterlogged in the second slot.
    pub sensor: [VoxelId; 2],
    pub shrieker: [VoxelId; 2],
}

#[derive(Clone, Copy, PartialEq)]
enum Behaviour {
    Default,
    Sculk,
    Vein,
}

/// How many of `spread_positions` a spread tries, in order: `DEFAULT_SPREAD_ORDER`
/// takes all three, `SAME_SPACE_ORDER` the first alone.
const DEFAULT_SPREAD_ORDER: usize = 3;
const SAME_SPACE_ORDER: usize = 1;

struct Cursor {
    pos: IVec3,
    charge: i32,
    update_delay: i32,
    decay_delay: i32,
    /// `MultifaceBlock.availableFaces` of whatever the cursor last moved onto,
    /// which is empty for a sculk block and absent until it moves at all.
    facings: Option<u8>,
}

impl CompiledSculkPatch {
    fn vein_state(&self, faces: u8, waterlogged: bool) -> VoxelId {
        self.vein.by_faces[usize::from(waterlogged)][usize::from(faces)]
    }

    fn behaviour(&self, state: VoxelId) -> Behaviour {
        if self.vein.faces_of(state).is_some() {
            Behaviour::Vein
        } else if holds(&self.sculk_states, state) {
            Behaviour::Sculk
        } else {
            Behaviour::Default
        }
    }
}

/// `SculkPatchFeature.place`: a fresh set of cursors each round, spent over the
/// configured number of update attempts.
pub fn place_sculk_patch<W: WorldGenVolume>(
    config: &CompiledSculkPatch,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    origin: IVec3,
) -> bool {
    if !can_spread_from(config, volume, origin) {
        return false;
    }
    let mut cursors: Vec<Cursor> = Vec::new();
    for round in 0..config.spread_rounds + config.growth_rounds {
        for _ in 0..config.charge_count {
            add_cursors(&mut cursors, origin, config.amount_per_charge);
        }
        let spread_veins = round < config.spread_rounds;
        for _ in 0..config.spread_attempts {
            update_cursors(config, volume, rng, &mut cursors, origin, spread_veins);
        }
        cursors.clear();
    }
    true
}

fn can_spread_from<W: WorldGenVolume>(
    config: &CompiledSculkPatch,
    volume: &W,
    origin: IVec3,
) -> bool {
    let state = volume.get(origin);
    if config.behaviour(state) != Behaviour::Default {
        return true;
    }
    if !holds(&volume.world().air_states, state) && state != volume.world().water {
        return false;
    }
    Direction::all().into_iter().any(|direction| {
        holds(
            &volume.world().sturdy_up,
            volume.get(direction.relative(origin, 1)),
        )
    })
}

fn add_cursors(cursors: &mut Vec<Cursor>, pos: IVec3, mut charge: i32) {
    while charge > 0 {
        let current = charge.min(MAX_CHARGE);
        if cursors.len() < MAX_CURSORS {
            cursors.push(Cursor {
                pos,
                charge: current,
                update_delay: 0,
                decay_delay: 1,
                facings: None,
            });
        }
        charge -= current;
    }
}

/// A world-generation spreader never merges cursors, so the pass over them
/// keeps every one whose charge survived and drops the rest.
fn update_cursors<W: WorldGenVolume>(
    config: &CompiledSculkPatch,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    cursors: &mut Vec<Cursor>,
    origin: IVec3,
    spread_veins: bool,
) {
    cursors.retain_mut(|cursor| {
        if (cursor.pos - origin).abs().max_element() > MAX_CURSOR_DISTANCE {
            return false;
        }
        update_cursor(config, volume, rng, cursor, origin, spread_veins);
        cursor.charge > 0
    });
}

fn update_cursor<W: WorldGenVolume>(
    config: &CompiledSculkPatch,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    cursor: &mut Cursor,
    origin: IVec3,
    spread_veins: bool,
) {
    if cursor.charge <= 0 {
        return;
    }
    if cursor.update_delay > 0 {
        cursor.update_delay -= 1;
        return;
    }

    let mut current = volume.get(cursor.pos);
    let mut behaviour = config.behaviour(current);
    if spread_veins
        && attempt_spread_vein(
            config,
            volume,
            cursor.pos,
            current,
            behaviour,
            cursor.facings,
        )
        && behaviour != Behaviour::Sculk
    {
        current = volume.get(cursor.pos);
        behaviour = config.behaviour(current);
    }

    cursor.charge =
        attempt_use_charge(config, volume, rng, cursor, behaviour, origin, spread_veins);
    if cursor.charge <= 0 {
        on_discharged(config, volume, current, cursor.pos);
        return;
    }

    let Some(next) = valid_movement_pos(config, volume, rng, cursor.pos, origin) else {
        on_discharged(config, volume, current, cursor.pos);
        cursor.charge = 0;
        return;
    };
    on_discharged(config, volume, current, cursor.pos);
    cursor.pos = next;
    current = volume.get(next);

    match config.behaviour(current) {
        Behaviour::Default => {}
        Behaviour::Sculk => cursor.facings = Some(0),
        Behaviour::Vein => {
            cursor.facings = config.vein.faces_of(current).map(|(faces, _)| faces);
        }
    }
    cursor.decay_delay = match behaviour {
        Behaviour::Default => (cursor.decay_delay - 1).max(0),
        _ => 1,
    };
    cursor.update_delay = 1;
}

fn attempt_spread_vein<W: WorldGenVolume>(
    config: &CompiledSculkPatch,
    volume: &mut W,
    pos: IVec3,
    state: VoxelId,
    behaviour: Behaviour,
    facings: Option<u8>,
) -> bool {
    match behaviour {
        Behaviour::Sculk | Behaviour::Vein => {
            spread_all(config, volume, state, pos, DEFAULT_SPREAD_ORDER)
        }
        Behaviour::Default => match facings {
            None => spread_all(config, volume, state, pos, SAME_SPACE_ORDER),
            Some(0) => spread_all(config, volume, state, pos, DEFAULT_SPREAD_ORDER),
            Some(faces) => {
                (holds(&volume.world().air_states, state)
                    || holds(&volume.world().water_fluid, state))
                    && regrow(config, volume, pos, state, faces)
            }
        },
    }
}

/// `SculkVeinBlock.regrow`, which rebuilds the faces the cursor remembers out of
/// whichever of them still have something to hang on.
fn regrow<W: WorldGenVolume>(
    config: &CompiledSculkPatch,
    volume: &mut W,
    pos: IVec3,
    existing: VoxelId,
    faces: u8,
) -> bool {
    let mut grown = 0u8;
    for direction in Direction::all() {
        if faces & face_bit(direction) != 0
            && holds(
                &volume.world().sturdy_up,
                volume.get(direction.relative(pos, 1)),
            )
        {
            grown |= face_bit(direction);
        }
    }
    if grown == 0 {
        return false;
    }
    let waterlogged = holds(&volume.world().any_fluid, existing);
    volume.set(pos, config.vein_state(grown, waterlogged));
    true
}

/// `MultifaceSpreader.spreadAll`, whose source state is the one the caller read
/// before any of this wrote, however far behind the world it falls.
fn spread_all<W: WorldGenVolume>(
    config: &CompiledSculkPatch,
    volume: &mut W,
    source: VoxelId,
    pos: IVec3,
    spreads: usize,
) -> bool {
    let source_faces = config.vein.faces_of(source).map(|(faces, _)| faces);
    let mut placed = false;
    for from_face in Direction::all() {
        if source_faces.is_some_and(|faces| faces & face_bit(from_face) == 0) {
            continue;
        }
        for spread in Direction::all() {
            placed |= spread_toward(
                config,
                volume,
                source_faces,
                pos,
                from_face,
                spread,
                spreads,
            );
        }
    }
    placed
}

fn spread_toward<W: WorldGenVolume>(
    config: &CompiledSculkPatch,
    volume: &mut W,
    source_faces: Option<u8>,
    pos: IVec3,
    from_face: Direction,
    spread: Direction,
    spreads: usize,
) -> bool {
    if spread.axis() == from_face.axis() {
        return false;
    }
    if source_faces
        .is_some_and(|faces| faces & face_bit(from_face) == 0 || faces & face_bit(spread) != 0)
    {
        return false;
    }
    for (target, face) in spread_positions(pos, spread, from_face)
        .into_iter()
        .take(spreads)
    {
        if !can_spread_into(config, volume, pos, target, face) {
            continue;
        }
        let old = volume.get(target);
        let Some(state) = config.vein.state_for_placement(volume, old, target, face) else {
            return false;
        };
        volume.set(target, state);
        return true;
    }
    false
}

fn can_spread_into<W: WorldGenVolume>(
    config: &CompiledSculkPatch,
    volume: &W,
    source: IVec3,
    target: IVec3,
    face: Direction,
) -> bool {
    if volume.holds(&config.blocks_vein, face.relative(target, 1)) {
        return false;
    }
    if dist_manhattan(source, target) == 2
        && holds(
            &volume.world().sturdy_up,
            volume.get(face.opposite().relative(source, 1)),
        )
    {
        return false;
    }
    let existing = volume.get(target);
    if holds(&volume.world().any_fluid, existing) && !holds(&volume.world().water_fluid, existing) {
        return false;
    }
    if holds(&config.fire, existing) {
        return false;
    }
    let replaceable = holds(&volume.world().replaceable, existing)
        || holds(&volume.world().air_states, existing)
        || config.vein.faces_of(existing).is_some()
        || existing == volume.world().water;
    replaceable
        && config
            .vein
            .state_for_placement(volume, existing, target, face)
            .is_some()
}

fn attempt_use_charge<W: WorldGenVolume>(
    config: &CompiledSculkPatch,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    cursor: &Cursor,
    behaviour: Behaviour,
    origin: IVec3,
    spread_veins: bool,
) -> i32 {
    match behaviour {
        Behaviour::Default => {
            if cursor.decay_delay > 0 {
                cursor.charge
            } else {
                0
            }
        }
        Behaviour::Vein => {
            if spread_veins && attempt_place_sculk(config, volume, rng, cursor.pos) {
                cursor.charge - 1
            } else if rng.next_i32_bound(CHARGE_DECAY_RATE) == 0 {
                (cursor.charge as f32 * 0.5).floor() as i32
            } else {
                cursor.charge
            }
        }
        Behaviour::Sculk => sculk_use_charge(config, volume, rng, cursor, origin),
    }
}

/// `SculkVeinBlock.attemptPlaceSculk`: the first face whose support this pass
/// may replace turns into sculk, and every vein around it is asked to let go of
/// the face it just lost.
fn attempt_place_sculk<W: WorldGenVolume>(
    config: &CompiledSculkPatch,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    pos: IVec3,
) -> bool {
    let state = volume.get(pos);
    let Some((faces, _)) = config.vein.faces_of(state) else {
        return false;
    };
    for support in all_shuffled(rng) {
        if faces & face_bit(support) == 0 {
            continue;
        }
        let support_pos = support.relative(pos, 1);
        if !volume.holds(&config.replaceable_world_gen, support_pos) {
            continue;
        }
        volume.set(support_pos, config.sculk);
        spread_all(
            config,
            volume,
            config.sculk,
            support_pos,
            DEFAULT_SPREAD_ORDER,
        );
        let skip = support.opposite();
        for direction in Direction::all() {
            if direction == skip {
                continue;
            }
            let vein_pos = direction.relative(support_pos, 1);
            let vein_state = volume.get(vein_pos);
            if config.vein.faces_of(vein_state).is_some() {
                on_discharged(config, volume, vein_state, vein_pos);
            }
        }
        return true;
    }
    false
}

fn sculk_use_charge<W: WorldGenVolume>(
    config: &CompiledSculkPatch,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    cursor: &Cursor,
    origin: IVec3,
) -> i32 {
    let charge = cursor.charge;
    if charge == 0 || rng.next_i32_bound(CHARGE_DECAY_RATE) != 0 {
        return charge;
    }
    let distance = (cursor.pos - origin).as_dvec3().length_squared();
    let close = distance < f64::from(NO_GROWTH_RADIUS * NO_GROWTH_RADIUS);
    if !close && can_place_growth(config, volume, cursor.pos) {
        if rng.next_i32_bound(GROWTH_SPAWN_COST) < charge {
            let growth = cursor.pos + IVec3::Y;
            let state = random_growth_state(config, volume, rng, growth);
            volume.set(growth, state);
        }
        return (charge - GROWTH_SPAWN_COST).max(0);
    }
    if rng.next_i32_bound(ADDITIONAL_DECAY_RATE) != 0 {
        return charge;
    }
    charge
        - if close {
            1
        } else {
            decay_penalty(distance, charge)
        }
}

fn decay_penalty(distance_sq: f64, charge: i32) -> i32 {
    let outer = ((distance_sq.sqrt() as f32) - NO_GROWTH_RADIUS as f32).powi(2);
    let max_reach = ((MAX_GROWTH_RATE_RADIUS - NO_GROWTH_RADIUS).pow(2)) as f32;
    let factor = (outer / max_reach).min(1.0);
    ((charge as f32 * factor * 0.5) as i32).max(1)
}

fn random_growth_state<W: WorldGenVolume>(
    config: &CompiledSculkPatch,
    volume: &W,
    rng: &mut XoroshiroRandom,
    pos: IVec3,
) -> VoxelId {
    let family = if rng.next_i32_bound(SHRIEKER_PLACEMENT_RATE) == 0 {
        &config.shrieker
    } else {
        &config.sensor
    };
    family[usize::from(volume.holds(&volume.world().any_fluid, pos))]
}

/// `SculkBlock.canPlaceGrowth`: an open cell above, and no more than two of the
/// growths already there to inhibit another.
fn can_place_growth<W: WorldGenVolume>(
    config: &CompiledSculkPatch,
    volume: &W,
    pos: IVec3,
) -> bool {
    let above = volume.get(pos + IVec3::Y);
    if !holds(&volume.world().air_states, above) && !holds(&volume.world().water_states, above) {
        return false;
    }
    let mut found = 0;
    for z in pos.z - GROWTH_INHIBITOR_RANGE..=pos.z + GROWTH_INHIBITOR_RANGE {
        for y in pos.y..=pos.y + 2 {
            for x in pos.x - GROWTH_INHIBITOR_RANGE..=pos.x + GROWTH_INHIBITOR_RANGE {
                if volume.holds(&config.growth_inhibitors, IVec3::new(x, y, z)) {
                    found += 1;
                    if found > 2 {
                        return false;
                    }
                }
            }
        }
    }
    true
}

fn on_discharged<W: WorldGenVolume>(
    config: &CompiledSculkPatch,
    volume: &mut W,
    state: VoxelId,
    pos: IVec3,
) {
    let Some((mut faces, waterlogged)) = config.vein.faces_of(state) else {
        return;
    };
    for direction in Direction::all() {
        if faces & face_bit(direction) != 0
            && volume.holds(&config.sculk_states, direction.relative(pos, 1))
        {
            faces &= !face_bit(direction);
        }
    }
    let next = if faces == 0 {
        if waterlogged {
            volume.world().water
        } else {
            volume.world().air
        }
    } else {
        config.vein_state(faces, waterlogged)
    };
    volume.set(pos, next);
}

/// `BlockPos.betweenClosedStream` over the unit cube less its corners, in the
/// order the shuffle permutes: x fastest, then y, then z.
const NON_CORNER_NEIGHBOURS: [IVec3; 18] = [
    IVec3::new(0, -1, -1),
    IVec3::new(-1, 0, -1),
    IVec3::new(0, 0, -1),
    IVec3::new(1, 0, -1),
    IVec3::new(0, 1, -1),
    IVec3::new(-1, -1, 0),
    IVec3::new(0, -1, 0),
    IVec3::new(1, -1, 0),
    IVec3::new(-1, 0, 0),
    IVec3::new(1, 0, 0),
    IVec3::new(-1, 1, 0),
    IVec3::new(0, 1, 0),
    IVec3::new(1, 1, 0),
    IVec3::new(0, -1, 1),
    IVec3::new(-1, 0, 1),
    IVec3::new(0, 0, 1),
    IVec3::new(1, 0, 1),
    IVec3::new(0, 1, 1),
];

fn shuffled_neighbours(rng: &mut XoroshiroRandom) -> [IVec3; 18] {
    let mut offsets = NON_CORNER_NEIGHBOURS;
    mcrs_minecraft_random::shuffle(&mut offsets, rng);
    offsets
}

/// `ChargeCursor.getValidMovementPos`: the last reachable neighbour carrying a
/// sculk block, stopping early at the first one already rooted in something the
/// spread may replace.
fn valid_movement_pos<W: WorldGenVolume>(
    config: &CompiledSculkPatch,
    volume: &W,
    rng: &mut XoroshiroRandom,
    pos: IVec3,
    origin: IVec3,
) -> Option<IVec3> {
    let mut found = pos;
    for offset in shuffled_neighbours(rng) {
        let neighbour = pos + offset;
        let dx = origin.x - neighbour.x;
        let dz = origin.z - neighbour.z;
        if dx * dx + dz * dz > MAX_WORLDGEN_SPREAD_SQ {
            continue;
        }
        let transferee = volume.get(neighbour);
        if config.behaviour(transferee) == Behaviour::Default
            || !movement_unobstructed(volume, pos, neighbour)
        {
            continue;
        }
        found = neighbour;
        if has_substrate_access(config, volume, transferee, neighbour) {
            break;
        }
    }
    (found != pos).then_some(found)
}

fn has_substrate_access<W: WorldGenVolume>(
    config: &CompiledSculkPatch,
    volume: &W,
    state: VoxelId,
    pos: IVec3,
) -> bool {
    let Some((faces, _)) = config.vein.faces_of(state) else {
        return false;
    };
    Direction::all().into_iter().any(|direction| {
        faces & face_bit(direction) != 0
            && volume.holds(&config.substrate, direction.relative(pos, 1))
    })
}

/// `to` is one of the eighteen non-corner neighbours, so a nonzero component
/// of the delta is a unit step along its axis.
fn movement_unobstructed<W: WorldGenVolume>(volume: &W, from: IVec3, to: IVec3) -> bool {
    if dist_manhattan(from, to) == 1 {
        return true;
    }
    let delta = to - from;
    [IVec3::X, IVec3::Y, IVec3::Z]
        .into_iter()
        .map(|axis| axis * delta)
        .filter(|step| *step != IVec3::ZERO)
        .any(|step| !volume.holds(&volume.world().sturdy_up, from + step))
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_worldgen::feature::placer::mask_of;
    use rustc_hash::FxHashMap as HashMap;

    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    use super::*;
    use crate::feature::tree::provider::fake::{AIR, FakeVolume};

    const STONE: VoxelId = VoxelId(1);
    const SCULK: VoxelId = VoxelId(2);
    const VEIN_BASE: u16 = 100;
    const AT: IVec3 = IVec3::new(0, 60, 0);

    /// A vein whose state id is its face bits, dry and waterlogged one table
    /// apart, so the placer's own indexing is what the test reads back.
    fn vein_table() -> MultifaceStates {
        let mut by_faces = [[VoxelId(0); 64]; 2];
        let mut of_state = HashMap::default();
        for waterlogged in 0..2u16 {
            for faces in 0..64u16 {
                let id = VEIN_BASE + waterlogged * 64 + faces;
                by_faces[waterlogged as usize][faces as usize] = VoxelId(id);
                of_state.insert(id, (faces as u8, waterlogged == 1));
            }
        }
        MultifaceStates { by_faces, of_state }
    }

    fn config() -> CompiledSculkPatch {
        CompiledSculkPatch {
            charge_count: 1,
            amount_per_charge: 32,
            spread_attempts: 8,
            growth_rounds: 0,
            spread_rounds: 1,
            vein: vein_table(),
            sculk: SCULK,
            sculk_states: mask_of([SCULK]),
            blocks_vein: mask_of([SCULK]),
            fire: StateMask::default(),
            replaceable_world_gen: mask_of([STONE]),
            substrate: mask_of([STONE]),
            growth_inhibitors: StateMask::default(),
            sensor: [VoxelId(20), VoxelId(21)],
            shrieker: [VoxelId(22), VoxelId(23)],
        }
    }

    /// Stone and sculk hold a vein up; air is the one replaceable state; water
    /// is a state no block here holds.
    fn cave(mut volume: FakeVolume) -> FakeVolume {
        volume.world.water = VoxelId(9);
        volume.world.replaceable = mask_of([AIR]);
        volume.world.sturdy_up = mask_of([STONE, SCULK]);
        volume
    }

    fn floor() -> FakeVolume {
        cave(FakeVolume::with(
            (-6..=6)
                .flat_map(|x| (-6..=6).map(move |z| ((x, AT.y - 1, z), STONE)))
                .collect::<Vec<_>>(),
        ))
    }

    /// An origin with nothing to hang on refuses before the spreader exists, so
    /// it writes nothing and spends no draw of the parent's source.
    #[test]
    fn a_patch_with_no_support_places_nothing_and_draws_nothing() {
        let mut volume = cave(FakeVolume::default());
        let mut rng = XoroshiroRandom::new(7);
        let before = rng.clone();

        assert!(!place_sculk_patch(&config(), &mut volume, &mut rng, AT));
        assert_eq!(rng, before);
        assert!(volume.writes.is_empty());
    }

    /// The first cursor grows a vein on the face it can reach, and that vein
    /// turns the block it hangs on into sculk.
    #[test]
    fn a_patch_over_a_floor_veins_the_origin_and_sculks_what_it_hangs_on() {
        let config = config();
        let mut volume = floor();
        let mut rng = XoroshiroRandom::new(7);

        assert!(place_sculk_patch(&config, &mut volume, &mut rng, AT));

        let (down_face, _) = (config.vein.faces_of(volume.writes[0].1)).expect("a vein first");
        assert_eq!(down_face, face_bit(Direction::Down));
        assert_eq!(volume.writes[0].0, (AT.x, AT.y, AT.z));
        assert!(
            volume.writes.iter().any(|(_, state)| *state == SCULK),
            "no sculk grew under the patch"
        );
    }
}
