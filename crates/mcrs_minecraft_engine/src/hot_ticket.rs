use std::iter::once;

use bevy_ecs::lifecycle::{Add, Discard};
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Commands, Component, Entity, Query, Res, With};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::entity::player::chunk_view::PlayerViewDistance;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::{ColumnLoading, LOADING_RINGS};
use mcrs_minecraft_engine_world::world::lifecycle::coverage::Coverage;
use mcrs_minecraft_engine_world::world::lifecycle::simulation::SimulationDistance;
use mcrs_voxel::ColumnPos;

/// The columns that a player keeps hot: the simulation area one ring wider, as far as the view
/// reaches. A ticket loads each of them in the dimension, so they tick and stay there while
/// the player is near. Beyond them, a view gets the columns from the data layer.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HotTicket {
    dim: Entity,
    center: ColumnPos,
    radius: i32,
}

impl HotTicket {
    fn contains(self, column: ColumnPos) -> bool {
        (column.x - self.center.x).abs() <= self.radius
            && (column.z - self.center.z).abs() <= self.radius
    }

    fn columns(self) -> impl Iterator<Item = ColumnPos> {
        self.center.square(self.radius)
    }
}

/// The columns of a dimension that some hot ticket loads, so that a view takes them from the
/// dimension: they tick, or a block that ticks can reach them, so the dimension may hold a
/// version later than the data layer.
#[derive(Component, Debug, Default)]
pub struct HotColumns {
    centers: Coverage<ColumnPos>,
}

impl HotColumns {
    #[must_use]
    pub fn is_hot(&self, column: ColumnPos) -> bool {
        column
            .square(LOADING_RINGS)
            .any(|loaded| self.centers.contains(loaded))
    }

    /// Whether the dimension holds `column` and the eight around it, so it lights the column
    /// from the blocks of its neighbours and a view takes the column from the dimension. The
    /// outer ring of a hot area is hot but not lit this way: a view gets it from the data layer.
    #[must_use]
    pub fn lights(&self, column: ColumnPos) -> bool {
        column
            .square(LOADING_RINGS.saturating_sub(1))
            .any(|loaded| self.centers.contains(loaded))
    }

    /// Whether `column` is hot and the dimension does not light it from the blocks of all its
    /// neighbours.
    #[must_use]
    pub fn is_outer_ring(&self, column: ColumnPos) -> bool {
        self.is_hot(column) && !self.lights(column)
    }

    fn change(&mut self, loading: &mut ColumnLoading, column: ColumnPos, change: Change) {
        match change {
            Change::Raise => {
                self.centers.raise(once(column), |_| {});
                loading.add_ticket(column);
            }
            Change::Lower => {
                self.centers.lower(once(column), |_| {});
                loading.remove_ticket(column);
            }
        }
    }
}

type HotDims<'w, 's> = Query<'w, 's, (&'static mut ColumnLoading, &'static mut HotColumns)>;

pub(crate) fn track_hot_tickets(
    distance: Res<SimulationDistance>,
    mut players: Query<
        (
            Entity,
            &Transform,
            &InDimension,
            Option<&PlayerViewDistance>,
            Option<&mut HotTicket>,
        ),
        With<Player>,
    >,
    mut dims: HotDims,
    mut commands: Commands,
) {
    let simulated = i32::from(distance.horizontal).saturating_add(1);
    for (player, transform, in_dim, view, held) in &mut players {
        let viewed = view.map_or(simulated, |view| i32::from(view.distance).saturating_add(1));
        let wanted = HotTicket {
            dim: in_dim.0,
            center: ColumnPos::from(transform.translation),
            radius: simulated.min(viewed),
        };
        // The first ticket is raised once it lands, so a player despawned before then holds
        // nothing that would never be released.
        let Some(mut held) = held else {
            commands.entity(player).try_insert(wanted);
            continue;
        };
        let old = *held;
        if old == wanted {
            continue;
        }
        if old.dim == wanted.dim
            && let Ok((mut loading, mut hot)) = dims.get_mut(wanted.dim)
        {
            for column in wanted.columns().filter(|column| !old.contains(*column)) {
                hot.change(&mut loading, column, Change::Raise);
            }
            for column in old.columns().filter(|column| !wanted.contains(*column)) {
                hot.change(&mut loading, column, Change::Lower);
            }
        } else {
            shift(&mut dims, old, Change::Lower);
            shift(&mut dims, wanted, Change::Raise);
        }
        *held = wanted;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
    Raise,
    Lower,
}

fn shift(dims: &mut HotDims, ticket: HotTicket, change: Change) {
    let Ok((mut loading, mut hot)) = dims.get_mut(ticket.dim) else {
        return;
    };
    for column in ticket.columns() {
        hot.change(&mut loading, column, change);
    }
}

pub(crate) fn raise_hot_ticket(
    add: On<Add, HotTicket>,
    held: Query<&HotTicket>,
    mut dims: HotDims,
) {
    if let Ok(ticket) = held.get(add.event().entity) {
        shift(&mut dims, *ticket, Change::Raise);
    }
}

pub(crate) fn release_hot_ticket(
    discard: On<Discard, HotTicket>,
    held: Query<&HotTicket>,
    mut dims: HotDims,
) {
    if let Ok(ticket) = held.get(discard.event().entity) {
        shift(&mut dims, *ticket, Change::Lower);
    }
}
