use std::iter::once;

use bevy_ecs::lifecycle::{Add, Discard};
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Commands, Component, Entity, Has, Query, Res, ResMut, With, Without};
use bevy_ecs::resource::Resource;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::entity::player::chunk_view::PlayerViewDistance;
use mcrs_minecraft_engine_world::owned_columns::{self, OwnedColumns};
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::{ColumnLoading, LOADING_RINGS};
use mcrs_minecraft_engine_world::world::lifecycle::coverage::Coverage;
use mcrs_minecraft_engine_world::world::lifecycle::simulation::SimulationDistance;
use mcrs_voxel::ColumnPos;
use rustc_hash::FxHashMap;

/// The columns that a player keeps hot: the simulation area one ring wider, as far as the view
/// reaches. A ticket loads each of them in the dimension, so they tick and stay there while
/// the player is near. Beyond them, a view gets the columns from the data layer.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HotTicket {
    dim: Entity,
    center: ColumnPos,
    radius: i32,
}

/// The columns of a hot ticket, as they cross from one cell of a world to another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TicketSquare {
    pub(crate) center: ColumnPos,
    pub(crate) radius: i32,
}

/// The hot ticket of the player of `session` changed in its owner: `None` when the player left
/// the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TicketUpdate {
    pub(crate) session: PlayerSession,
    pub(crate) square: Option<TicketSquare>,
}

/// The ticket updates of this tick for the other cells of the world. Only a world with
/// [`OwnedColumns`] holds it.
#[derive(Resource, Debug, Default)]
pub(crate) struct TicketMail(pub(crate) Vec<TicketUpdate>);

/// Holds the hot ticket of a player of another cell of the world, so that this cell keeps hot
/// the columns of the ticket that it owns.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct RemoteTicket;

/// The entity that holds each [`RemoteTicket`], by session.
#[derive(Resource, Debug, Default)]
pub(crate) struct RemoteTickets(pub(crate) FxHashMap<PlayerSession, Entity>);

/// Marks a player that leaves its cell for another cell of the same world at the end of this
/// tick. What it takes along does not end for its viewers: no despawn, no drop of its ticket and
/// no close of its view.
#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct CellCrossing;

impl HotTicket {
    pub(crate) fn square(self) -> TicketSquare {
        TicketSquare {
            center: self.center,
            radius: self.radius,
        }
    }

    pub(crate) fn in_dim(dim: Entity, square: TicketSquare) -> Self {
        Self {
            dim,
            center: square.center,
            radius: square.radius,
        }
    }

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

    /// A cell of a world with several cells raises only the columns within the halo of its
    /// boxes, so the columns that it owns load exactly as in a world of one cell.
    fn change(
        &mut self,
        loading: &mut ColumnLoading,
        owned: Option<&OwnedColumns>,
        column: ColumnPos,
        change: Change,
    ) {
        if !owned_columns::reaches(owned, column) {
            return;
        }
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
    owned: Option<Res<OwnedColumns>>,
    mut mail: Option<ResMut<TicketMail>>,
    mut players: Query<
        (
            Entity,
            &Owner,
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
    let owned = owned.as_deref();
    let simulated = i32::from(distance.horizontal).saturating_add(1);
    for (player, &Owner(session), transform, in_dim, view, held) in &mut players {
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
            post(mail.as_deref_mut(), session, Some(wanted));
            continue;
        };
        let old = *held;
        if old == wanted {
            continue;
        }
        post(mail.as_deref_mut(), session, Some(wanted));
        if old.dim == wanted.dim
            && let Ok((mut loading, mut hot)) = dims.get_mut(wanted.dim)
        {
            for column in wanted.columns().filter(|column| !old.contains(*column)) {
                hot.change(&mut loading, owned, column, Change::Raise);
            }
            for column in old.columns().filter(|column| !wanted.contains(*column)) {
                hot.change(&mut loading, owned, column, Change::Lower);
            }
        } else {
            shift(&mut dims, owned, old, Change::Lower);
            shift(&mut dims, owned, wanted, Change::Raise);
        }
        *held = wanted;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
    Raise,
    Lower,
}

fn post(mail: Option<&mut TicketMail>, session: PlayerSession, ticket: Option<HotTicket>) {
    if let Some(mail) = mail {
        mail.0.push(TicketUpdate {
            session,
            square: ticket.map(HotTicket::square),
        });
    }
}

fn shift(dims: &mut HotDims, owned: Option<&OwnedColumns>, ticket: HotTicket, change: Change) {
    let Ok((mut loading, mut hot)) = dims.get_mut(ticket.dim) else {
        return;
    };
    for column in ticket.columns() {
        hot.change(&mut loading, owned, column, change);
    }
}

pub(crate) fn raise_hot_ticket(
    add: On<Add, HotTicket>,
    held: Query<&HotTicket>,
    owned: Option<Res<OwnedColumns>>,
    mut dims: HotDims,
) {
    if let Ok(ticket) = held.get(add.event().entity) {
        shift(&mut dims, owned.as_deref(), *ticket, Change::Raise);
    }
}

/// Lowers a ticket that goes away. A player of a world with several cells that leaves it tells
/// the other cells, which hold its ticket as a [`RemoteTicket`].
pub(crate) fn release_hot_ticket(
    discard: On<Discard, HotTicket>,
    held: Query<(&HotTicket, Option<&Owner>, Has<CellCrossing>), Without<RemoteTicket>>,
    remote: Query<&HotTicket, With<RemoteTicket>>,
    owned: Option<Res<OwnedColumns>>,
    mut mail: Option<ResMut<TicketMail>>,
    mut dims: HotDims,
) {
    let entity = discard.event().entity;
    if let Ok(ticket) = remote.get(entity) {
        shift(&mut dims, owned.as_deref(), *ticket, Change::Lower);
        return;
    }
    let Ok((ticket, owner, crossing)) = held.get(entity) else {
        return;
    };
    shift(&mut dims, owned.as_deref(), *ticket, Change::Lower);
    if let Some(&Owner(session)) = owner
        && !crossing
    {
        post(mail.as_deref_mut(), session, None);
    }
}
