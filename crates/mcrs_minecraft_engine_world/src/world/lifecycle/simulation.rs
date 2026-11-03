use std::ops::RangeInclusive;

use bevy_ecs::lifecycle::{Add, Remove};
use bevy_ecs::prelude::*;
use mcrs_voxel::SectionPos;

use crate::entity::physics::Transform;
use crate::entity::player::Player;
use crate::world::dimension::InDimension;
use crate::world::lifecycle::column_loading::ColumnLoading;
use crate::world::lifecycle::coverage::Coverage;
use crate::world::storage::section::SectionIndex;

/// The reference's `simulation-distance` server property, counted in sections, with a vertical
/// reach of its own. A section ticks while it is within `horizontal` sections of a player on
/// both horizontal axes and within `vertical` sections above or below. A vertical reach as tall
/// as the dimension ticks whole columns, as the reference does.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimulationDistance {
    pub horizontal: u8,
    pub vertical: u8,
}

impl Default for SimulationDistance {
    /// The reference's default across, and every section of the column up and down.
    fn default() -> Self {
        Self {
            horizontal: 10,
            vertical: u8::MAX,
        }
    }
}

/// The box of sections a player simulates around the section they stand in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimulationArea {
    pub center: SectionPos,
    pub horizontal: u8,
    pub vertical: u8,
}

impl SimulationArea {
    /// One ring wider every way: the reference runs scheduled ticks one ring past the area in
    /// which entities and random ticks run.
    fn widened(self) -> Self {
        Self {
            horizontal: self.horizontal.saturating_add(1),
            vertical: self.vertical.saturating_add(1),
            ..self
        }
    }

    /// The sections of the area that lie within `section_y`.
    fn within(self, section_y: &RangeInclusive<i32>) -> SectionBox {
        let horizontal = i32::from(self.horizontal);
        let vertical = i32::from(self.vertical);
        let center = self.center;
        SectionBox {
            min: SectionPos::new(
                center.x - horizontal,
                center.y.saturating_sub(vertical).max(*section_y.start()),
                center.z - horizontal,
            ),
            max: SectionPos::new(
                center.x + horizontal,
                center.y.saturating_add(vertical).min(*section_y.end()),
                center.z + horizontal,
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SectionBox {
    min: SectionPos,
    max: SectionPos,
}

impl SectionBox {
    fn is_empty(self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y || self.min.z > self.max.z
    }

    fn overlaps(self, other: Self) -> bool {
        !self.is_empty()
            && !other.is_empty()
            && self.min.x <= other.max.x
            && other.min.x <= self.max.x
            && self.min.y <= other.max.y
            && other.min.y <= self.max.y
            && self.min.z <= other.max.z
            && other.min.z <= self.max.z
    }

    /// The sections of this box outside `other`, as up to six boxes that do not overlap.
    fn minus(self, other: Option<Self>) -> [Option<Self>; 6] {
        let mut parts = [None; 6];
        let Some(other) = other.filter(|other| self.overlaps(*other)) else {
            parts[0] = Some(self);
            return parts;
        };
        let mut rest = self;
        if rest.min.x < other.min.x {
            let max = SectionPos::new(other.min.x - 1, rest.max.y, rest.max.z);
            parts[0] = Some(Self { min: rest.min, max });
            rest.min.x = other.min.x;
        }
        if rest.max.x > other.max.x {
            let min = SectionPos::new(other.max.x + 1, rest.min.y, rest.min.z);
            parts[1] = Some(Self { min, max: rest.max });
            rest.max.x = other.max.x;
        }
        if rest.min.y < other.min.y {
            let max = SectionPos::new(rest.max.x, other.min.y - 1, rest.max.z);
            parts[2] = Some(Self { min: rest.min, max });
            rest.min.y = other.min.y;
        }
        if rest.max.y > other.max.y {
            let min = SectionPos::new(rest.min.x, other.max.y + 1, rest.min.z);
            parts[3] = Some(Self { min, max: rest.max });
            rest.max.y = other.max.y;
        }
        if rest.min.z < other.min.z {
            let max = SectionPos::new(rest.max.x, rest.max.y, other.min.z - 1);
            parts[4] = Some(Self { min: rest.min, max });
        }
        if rest.max.z > other.max.z {
            let min = SectionPos::new(rest.min.x, rest.min.y, other.max.z + 1);
            parts[5] = Some(Self { min, max: rest.max });
        }
        parts
    }

    fn sections(self) -> impl Iterator<Item = SectionPos> {
        let SectionBox { min, max } = self;
        (min.y..=max.y).flat_map(move |y| {
            (min.z..=max.z)
                .flat_map(move |z| (min.x..=max.x).map(move |x| SectionPos::new(x, y, z)))
        })
    }
}

/// Scheduled ticks and block events run in this section: it is within one ring of a player's
/// simulation area.
// Sparse: the marker comes and goes as players move, and each toggle would move a table row.
#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct BlockTicking;

/// Entities and random ticks run in this section: it is within a player's simulation area.
#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct EntityTicking;

/// The sections of a dimension that some player's simulation area covers. Every section entity
/// of the dimension carries [`BlockTicking`] and [`EntityTicking`] exactly while it is covered.
#[derive(Component, Debug, Default)]
pub struct SectionSimulation {
    block_ticking: Coverage<SectionPos>,
    entity_ticking: Coverage<SectionPos>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ticking {
    Block,
    Entity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
    Started,
    Stopped,
}

impl SectionSimulation {
    pub fn is_block_ticking(&self, pos: SectionPos) -> bool {
        self.block_ticking.contains(pos)
    }

    pub fn is_entity_ticking(&self, pos: SectionPos) -> bool {
        self.entity_ticking.contains(pos)
    }

    /// Gives `section`, spawned at `pos`, the markers its position calls for.
    pub(crate) fn mark_spawned(&self, section: &mut EntityCommands, pos: SectionPos) {
        if self.is_block_ticking(pos) {
            section.insert(BlockTicking);
        }
        if self.is_entity_ticking(pos) {
            section.insert(EntityTicking);
        }
    }

    /// Moves an area from `old` to `new`, either of which may be absent, within the sections
    /// `section_y` holds. Only the sections it leaves or enters are touched, and each one that
    /// starts or stops ticking is passed on.
    fn shift(
        &mut self,
        old: Option<SimulationArea>,
        new: Option<SimulationArea>,
        section_y: &RangeInclusive<i32>,
        mut changed: impl FnMut(SectionPos, Ticking, Change),
    ) {
        shift_coverage(
            &mut self.block_ticking,
            old.map(|area| area.widened().within(section_y)),
            new.map(|area| area.widened().within(section_y)),
            |pos, change| changed(pos, Ticking::Block, change),
        );
        shift_coverage(
            &mut self.entity_ticking,
            old.map(|area| area.within(section_y)),
            new.map(|area| area.within(section_y)),
            |pos, change| changed(pos, Ticking::Entity, change),
        );
    }
}

// A move that the dimension's top or bottom clips to the same box, such as a step up or down
// with a reach taller than the dimension, changes nothing and is not walked.
fn shift_coverage(
    coverage: &mut Coverage<SectionPos>,
    old: Option<SectionBox>,
    new: Option<SectionBox>,
    mut changed: impl FnMut(SectionPos, Change),
) {
    if old == new {
        return;
    }
    if let Some(old) = old {
        let left = old
            .minus(new)
            .into_iter()
            .flatten()
            .flat_map(SectionBox::sections);
        coverage.lower(left, |pos| changed(pos, Change::Stopped));
    }
    if let Some(new) = new {
        let entered = new
            .minus(old)
            .into_iter()
            .flatten()
            .flat_map(SectionBox::sections);
        coverage.raise(entered, |pos| changed(pos, Change::Started));
    }
}

fn mark(
    commands: &mut Commands,
    index: &SectionIndex,
    pos: SectionPos,
    ticking: Ticking,
    change: Change,
) {
    let Some(section) = index.get(pos) else {
        return;
    };
    let mut section = commands.entity(section);
    match (ticking, change) {
        (Ticking::Block, Change::Started) => section.try_insert(BlockTicking),
        (Ticking::Block, Change::Stopped) => section.try_remove::<BlockTicking>(),
        (Ticking::Entity, Change::Started) => section.try_insert(EntityTicking),
        (Ticking::Entity, Change::Stopped) => section.try_remove::<EntityTicking>(),
    };
}

/// The simulation area a player holds, kept so that exactly this area is handed back.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimulationTicket {
    dim: Entity,
    area: SimulationArea,
}

type Dimensions<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut SectionSimulation,
        &'static ColumnLoading,
        &'static SectionIndex,
    ),
>;

fn shift_in(
    dims: &mut Dimensions,
    commands: &mut Commands,
    dim: Entity,
    old: Option<SimulationArea>,
    new: Option<SimulationArea>,
) {
    let Ok((mut simulation, loading, index)) = dims.get_mut(dim) else {
        return;
    };
    simulation.shift(old, new, &loading.section_y, |pos, ticking, change| {
        mark(commands, index, pos, ticking, change);
    });
}

pub(crate) fn track_simulation_tickets(
    distance: Res<SimulationDistance>,
    mut players: Query<
        (
            Entity,
            &Transform,
            &InDimension,
            Option<&mut SimulationTicket>,
        ),
        With<Player>,
    >,
    mut dims: Dimensions,
    mut commands: Commands,
) {
    for (player, transform, in_dim, held) in &mut players {
        let wanted = SimulationTicket {
            dim: in_dim.0,
            area: SimulationArea {
                center: SectionPos::from(transform.translation),
                horizontal: distance.horizontal,
                vertical: distance.vertical,
            },
        };
        // The first area is raised once the ticket lands, so a player despawned before then
        // holds nothing that would never be released.
        let Some(mut held) = held else {
            commands.entity(player).try_insert(wanted);
            continue;
        };
        let old = *held;
        if old == wanted {
            continue;
        }
        if old.dim == wanted.dim {
            shift_in(
                &mut dims,
                &mut commands,
                wanted.dim,
                Some(old.area),
                Some(wanted.area),
            );
        } else {
            shift_in(&mut dims, &mut commands, old.dim, Some(old.area), None);
            shift_in(
                &mut dims,
                &mut commands,
                wanted.dim,
                None,
                Some(wanted.area),
            );
        }
        *held = wanted;
    }
}

pub(crate) fn raise_simulation_ticket(
    add: On<Add, SimulationTicket>,
    held: Query<&SimulationTicket>,
    mut dims: Dimensions,
    mut commands: Commands,
) {
    let Ok(held) = held.get(add.event().entity) else {
        return;
    };
    shift_in(&mut dims, &mut commands, held.dim, None, Some(held.area));
}

pub(crate) fn release_simulation_ticket(
    remove: On<Remove, SimulationTicket>,
    held: Query<&SimulationTicket>,
    mut dims: Dimensions,
    mut commands: Commands,
) {
    let Ok(held) = held.get(remove.event().entity) else {
        return;
    };
    shift_in(&mut dims, &mut commands, held.dim, Some(held.area), None);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::lifecycle::ticket::TicketPlugin;
    use crate::world::storage::section::SectionBundle;
    use bevy_app::{App, FixedUpdate};
    use mcrs_voxel::ColumnPos;
    use rustc_hash::FxHashSet;

    /// Within `ring` more than the area's reach on every axis, and within the dimension.
    fn covered(areas: &[Option<SimulationArea>], pos: SectionPos, ring: u32) -> bool {
        (-4..=4).contains(&pos.y)
            && areas.iter().flatten().any(|area| {
                let horizontal = u32::from(area.horizontal) + ring;
                let vertical = u32::from(area.vertical) + ring;
                pos.x.abs_diff(area.center.x) <= horizontal
                    && pos.z.abs_diff(area.center.z) <= horizontal
                    && pos.y.abs_diff(area.center.y) <= vertical
            })
    }

    #[test]
    fn coverage_and_its_changes_match_the_areas_after_every_move() {
        let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = |bound: u64| {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 33) % bound) as i32
        };
        let verticals = [0, 1, 3, u8::MAX];
        let mut simulation = SectionSimulation::default();
        let mut areas: [Option<SimulationArea>; 3] = [None; 3];
        let mut block_ticking = FxHashSet::default();
        let mut entity_ticking = FxHashSet::default();

        for step in 0..200 {
            let slot = next(3) as usize;
            let area = (next(5) != 0).then(|| SimulationArea {
                center: SectionPos::new(next(13) - 6, next(13) - 6, next(13) - 6),
                horizontal: next(3) as u8,
                vertical: verticals[next(4) as usize],
            });
            simulation.shift(areas[slot], area, &(-4..=4), |pos, ticking, change| {
                let set = match ticking {
                    Ticking::Block => &mut block_ticking,
                    Ticking::Entity => &mut entity_ticking,
                };
                let applied = match change {
                    Change::Started => set.insert(pos),
                    Change::Stopped => set.remove(&pos),
                };
                assert!(applied, "step {step}: {pos:?} changed to {change:?} twice");
            });
            areas[slot] = area;

            for x in -10..=10 {
                for y in -8..=8 {
                    for z in -10..=10 {
                        let pos = SectionPos::new(x, y, z);
                        let block = covered(&areas, pos, 1);
                        let entity = covered(&areas, pos, 0);
                        assert_eq!(
                            simulation.is_block_ticking(pos),
                            block,
                            "step {step}: {pos:?}"
                        );
                        assert_eq!(
                            simulation.is_entity_ticking(pos),
                            entity,
                            "step {step}: {pos:?}"
                        );
                        assert_eq!(block_ticking.contains(&pos), block, "step {step}: {pos:?}");
                        assert_eq!(
                            entity_ticking.contains(&pos),
                            entity,
                            "step {step}: {pos:?}"
                        );
                    }
                }
            }
        }
    }

    fn section(app: &mut App, dim: Entity, pos: SectionPos) -> Entity {
        let section = app
            .world_mut()
            .spawn(SectionBundle::new(InDimension(dim), pos))
            .id();
        app.world_mut()
            .get_mut::<SectionIndex>(dim)
            .expect("section index")
            .insert(pos, section);
        section
    }

    fn markers(app: &App, section: SectionPos, dim: Entity) -> (bool, bool) {
        let section = app
            .world()
            .get::<SectionIndex>(dim)
            .and_then(|index| index.get(section))
            .expect("the section exists");
        (
            app.world().get::<BlockTicking>(section).is_some(),
            app.world().get::<EntityTicking>(section).is_some(),
        )
    }

    #[test]
    fn section_markers_follow_the_players_area_up_and_down() {
        let mut app = App::new();
        app.add_plugins(TicketPlugin);
        app.insert_resource(SimulationDistance {
            horizontal: 2,
            vertical: 1,
        });
        let dim = app
            .world_mut()
            .spawn((
                ColumnLoading::bounded(-8..=8),
                SectionSimulation::default(),
                SectionIndex::new(),
            ))
            .id();
        let inside = SectionPos::new(2, 1, -2);
        let ring = SectionPos::new(3, 0, 0);
        let above = SectionPos::new(0, 2, 0);
        let deep = SectionPos::new(0, -3, 0);
        for pos in [inside, ring, above, deep] {
            section(&mut app, dim, pos);
        }
        let player = app
            .world_mut()
            .spawn((Player, Transform::default(), InDimension(dim)))
            .id();

        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(markers(&app, inside, dim), (true, true));
        assert_eq!(markers(&app, ring, dim), (true, false));
        assert_eq!(markers(&app, above, dim), (true, false));
        assert_eq!(
            markers(&app, deep, dim),
            (false, false),
            "three sections down is out of reach"
        );

        app.world_mut()
            .get_mut::<ColumnLoading>(dim)
            .expect("column loading")
            .add_ticket(ColumnPos::new(0, 0));
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(
            markers(&app, SectionPos::new(1, 0, 1), dim),
            (true, true),
            "a section spawned inside the area is marked as it spawns"
        );
        assert_eq!(markers(&app, SectionPos::new(1, 5, 1), dim), (false, false));

        app.world_mut()
            .get_mut::<Transform>(player)
            .expect("transform")
            .translation
            .y = -3.0 * 16.0;
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(
            markers(&app, deep, dim),
            (true, true),
            "the player went down to it"
        );
        assert_eq!(markers(&app, inside, dim), (false, false));

        app.world_mut().despawn(player);
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(markers(&app, deep, dim), (false, false));
    }
}
