use std::ops::RangeInclusive;

use bevy_ecs::lifecycle::{Add, Remove};
use bevy_ecs::prelude::*;
use mcrs_voxel::{ColumnPos, SectionPos};

use crate::entity::physics::Transform;
use crate::entity::player::Player;
use crate::owned_columns::{self, OwnedColumns};
use crate::world::dimension::InDimension;
use crate::world::lifecycle::column_loading::ColumnLoading;
use crate::world::lifecycle::coverage::Coverage;
use crate::world::storage::entity_ticking::EntityTicking;
use crate::world::storage::section::SectionIndex;

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimulationDistance {
    pub horizontal: u8,
    pub vertical: u8,
}

impl Default for SimulationDistance {
    fn default() -> Self {
        Self {
            horizontal: 10,
            vertical: u8::MAX,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimulationArea {
    pub center: SectionPos,
    pub horizontal: u8,
    pub vertical: u8,
}

impl SimulationArea {
    #[must_use]
    pub fn around(center: SectionPos, distance: SimulationDistance) -> Self {
        Self {
            center,
            horizontal: distance.horizontal,
            vertical: distance.vertical,
        }
    }

    fn widened(self) -> Self {
        Self {
            horizontal: self.horizontal.saturating_add(1),
            vertical: self.vertical.saturating_add(1),
            ..self
        }
    }

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
    fn holds_column(self, column: ColumnPos) -> bool {
        (self.min.x..=self.max.x).contains(&column.x)
            && (self.min.z..=self.max.z).contains(&column.z)
    }

    fn outside(
        self,
        other: Option<Self>,
        owned: Option<&OwnedColumns>,
    ) -> impl Iterator<Item = SectionPos> {
        let SectionBox { min, max } = self;
        (min.z..=max.z)
            .flat_map(move |z| (min.x..=max.x).map(move |x| ColumnPos::new(x, z)))
            .filter(move |column| owned_columns::owns(owned, *column))
            .flat_map(move |column| {
                let (below, above) = match other.filter(|other| other.holds_column(column)) {
                    Some(other) => (
                        min.y..=max.y.min(other.min.y.saturating_sub(1)),
                        min.y.max(other.max.y.saturating_add(1))..=max.y,
                    ),
                    None => (min.y..=max.y, max.y.saturating_add(1)..=max.y),
                };
                below
                    .chain(above)
                    .map(move |y| SectionPos::new(column.x, y, column.z))
            })
    }
}

#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct BlockTicking;

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

    pub(crate) fn mark_spawned(&self, mut section: EntityCommands<'_>, pos: SectionPos) {
        if self.is_block_ticking(pos) {
            section.insert(BlockTicking);
        }
        if self.is_entity_ticking(pos) {
            section.insert(EntityTicking);
        }
    }

    fn shift(
        &mut self,
        old: Option<SimulationArea>,
        new: Option<SimulationArea>,
        section_y: &RangeInclusive<i32>,
        owned: Option<&OwnedColumns>,
        mut changed: impl FnMut(SectionPos, Ticking, Change),
    ) {
        shift_coverage(
            &mut self.block_ticking,
            old.map(|area| area.widened().within(section_y)),
            new.map(|area| area.widened().within(section_y)),
            owned,
            |pos, change| changed(pos, Ticking::Block, change),
        );
        shift_coverage(
            &mut self.entity_ticking,
            old.map(|area| area.within(section_y)),
            new.map(|area| area.within(section_y)),
            owned,
            |pos, change| changed(pos, Ticking::Entity, change),
        );
    }
}

fn shift_coverage(
    coverage: &mut Coverage<SectionPos>,
    old: Option<SectionBox>,
    new: Option<SectionBox>,
    owned: Option<&OwnedColumns>,
    mut changed: impl FnMut(SectionPos, Change),
) {
    if old == new {
        return;
    }
    if let Some(old) = old {
        coverage.lower(old.outside(new, owned), |pos| changed(pos, Change::Stopped));
    }
    if let Some(new) = new {
        coverage.raise(new.outside(old, owned), |pos| changed(pos, Change::Started));
    }
}

fn mark(
    mut commands: Commands<'_, '_>,
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

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimulationTicket {
    dim: Entity,
    area: SimulationArea,
}

impl SimulationTicket {
    #[must_use]
    pub fn new(dim: Entity, area: SimulationArea) -> Self {
        Self { dim, area }
    }
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
    mut dims: Dimensions<'_, '_>,
    mut commands: Commands<'_, '_>,
    owned: Option<&OwnedColumns>,
    dim: Entity,
    old: Option<SimulationArea>,
    new: Option<SimulationArea>,
) {
    let Ok((mut simulation, loading, index)) = dims.get_mut(dim) else {
        return;
    };
    simulation.shift(
        old,
        new,
        &loading.section_y,
        owned,
        |pos, ticking, change| {
            mark(commands.reborrow(), index, pos, ticking, change);
        },
    );
}

pub(crate) fn track_simulation_tickets(
    distance: Res<SimulationDistance>,
    mut players: Populated<
        (
            Entity,
            &Transform,
            &InDimension,
            Option<&mut SimulationTicket>,
        ),
        With<Player>,
    >,
    mut dims: Dimensions,
    owned: Option<Res<OwnedColumns>>,
    mut commands: Commands,
) {
    let owned = owned.as_deref();
    for (player, transform, in_dim, held) in &mut players {
        let wanted = SimulationTicket {
            dim: in_dim.0,
            area: SimulationArea::around(SectionPos::from(transform.translation), *distance),
        };
        follow(
            dims.reborrow(),
            commands.reborrow(),
            owned,
            player,
            wanted,
            held,
        );
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeldSimulationArea(SimulationTicket);

impl HeldSimulationArea {
    #[must_use]
    pub fn new(dim: Entity, area: SimulationArea) -> Self {
        Self(SimulationTicket::new(dim, area))
    }
}

pub(crate) fn track_held_simulation_areas(
    mut holders: Populated<
        (Entity, &HeldSimulationArea, Option<&mut SimulationTicket>),
        Changed<HeldSimulationArea>,
    >,
    mut dims: Dimensions,
    owned: Option<Res<OwnedColumns>>,
    mut commands: Commands,
) {
    let owned = owned.as_deref();
    for (holder, &HeldSimulationArea(wanted), held) in &mut holders {
        follow(
            dims.reborrow(),
            commands.reborrow(),
            owned,
            holder,
            wanted,
            held,
        );
    }
}

fn follow(
    mut dims: Dimensions<'_, '_>,
    mut commands: Commands<'_, '_>,
    owned: Option<&OwnedColumns>,
    holder: Entity,
    wanted: SimulationTicket,
    held: Option<Mut<SimulationTicket>>,
) {
    let Some(mut held) = held else {
        commands.entity(holder).try_insert(wanted);
        return;
    };
    let old = *held;
    if old == wanted {
        return;
    }
    if old.dim == wanted.dim {
        shift_in(
            dims,
            commands,
            owned,
            wanted.dim,
            Some(old.area),
            Some(wanted.area),
        );
    } else {
        shift_in(
            dims.reborrow(),
            commands.reborrow(),
            owned,
            old.dim,
            Some(old.area),
            None,
        );
        shift_in(dims, commands, owned, wanted.dim, None, Some(wanted.area));
    }
    held.set_if_neq(wanted);
}

pub(crate) fn raise_simulation_ticket(
    add: On<Add<SimulationTicket>>,
    held: Query<&SimulationTicket>,
    dims: Dimensions,
    owned: Option<Res<OwnedColumns>>,
    commands: Commands,
) {
    let Ok(held) = held.get(add.event().entity) else {
        return;
    };
    shift_in(
        dims,
        commands,
        owned.as_deref(),
        held.dim,
        None,
        Some(held.area),
    );
}

pub(crate) fn release_simulation_ticket(
    remove: On<Remove<SimulationTicket>>,
    held: Query<&SimulationTicket>,
    dims: Dimensions,
    owned: Option<Res<OwnedColumns>>,
    commands: Commands,
) {
    let Ok(held) = held.get(remove.event().entity) else {
        return;
    };
    shift_in(
        dims,
        commands,
        owned.as_deref(),
        held.dim,
        Some(held.area),
        None,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::lifecycle::ticket::TicketPlugin;
    use crate::world::storage::section::SectionBundle;
    use bevy_app::{App, FixedUpdate};
    use mcrs_voxel::ColumnPos;
    use rustc_hash::FxHashSet;

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
            simulation.shift(
                areas[slot],
                area,
                &(-4..=4),
                None,
                |pos, ticking, change| {
                    let set = match ticking {
                        Ticking::Block => &mut block_ticking,
                        Ticking::Entity => &mut entity_ticking,
                    };
                    let applied = match change {
                        Change::Started => set.insert(pos),
                        Change::Stopped => set.remove(&pos),
                    };
                    assert!(applied, "step {step}: {pos:?} changed to {change:?} twice");
                },
            );
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

        let mut lowered = *app.world().get::<Transform>(player).expect("transform");
        lowered.translation.y = -3.0 * 16.0;
        app.world_mut()
            .get_mut::<Transform>(player)
            .expect("transform")
            .set_if_neq(lowered);
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
