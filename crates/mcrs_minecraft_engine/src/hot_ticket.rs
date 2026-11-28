use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::lifecycle::{Add, Discard};
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Commands, Component, Entity, Has, Query, Res, ResMut, Single, With};
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::entity::player::chunk_view::PlayerViewDistance;
use mcrs_minecraft_engine_world::owned_columns::{self, OwnedColumns};
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;
use mcrs_minecraft_engine_world::world::lifecycle::simulation::{
    HeldSimulationArea, SimulationArea, SimulationDistance,
};
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::FxHashMap;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HotTicket {
    pub(crate) center: ColumnPos,
    pub(crate) radius: i32,
    pub(crate) simulation: SimulationArea,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TicketUpdate {
    pub(crate) session: PlayerSession,
    pub(crate) ticket: Option<HotTicket>,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct TicketMail(pub(crate) Vec<TicketUpdate>);

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct RemoteTicket;

#[derive(Resource, Debug, Default)]
pub(crate) struct RemoteTickets(pub(crate) FxHashMap<PlayerSession, Entity>);

#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct CellCrossing;

impl HotTicket {
    fn columns(self) -> impl Iterator<Item = ColumnPos> {
        self.center.square(self.radius)
    }

    fn contains(self, column: ColumnPos) -> bool {
        self.center.chebyshev_distance(column) <= self.radius
    }

    fn moved_from(self, old: Self, loading: &mut ColumnLoading, owned: Option<&OwnedColumns>) {
        let entered = self.columns().filter(|column| !old.contains(*column));
        shift(entered, loading, owned, Change::Raise);
        let left = old.columns().filter(|column| !self.contains(*column));
        shift(left, loading, owned, Change::Lower);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
    Raise,
    Lower,
}

fn shift(
    columns: impl Iterator<Item = ColumnPos>,
    loading: &mut ColumnLoading,
    owned: Option<&OwnedColumns>,
    change: Change,
) {
    for column in columns.filter(|column| owned_columns::reaches(owned, *column)) {
        match change {
            Change::Raise => loading.add_ticket(column),
            Change::Lower => loading.remove_ticket(column),
        }
    }
}

pub(crate) fn track_hot_tickets(
    distance: Res<SimulationDistance>,
    owned: Option<Res<OwnedColumns>>,
    mut mail: Option<ResMut<TicketMail>>,
    mut players: Query<
        (
            Entity,
            &Owner,
            &Transform,
            Option<&PlayerViewDistance>,
            Option<&mut HotTicket>,
        ),
        With<Player>,
    >,
    mut loading: Single<&mut ColumnLoading>,
    mut commands: Commands,
) {
    let owned = owned.as_deref();
    let simulated = i32::from(distance.horizontal).saturating_add(1);
    for (player, &Owner(session), transform, view, held) in &mut players {
        let viewed = view.map_or(simulated, |view| i32::from(view.distance).saturating_add(1));
        let wanted = HotTicket {
            center: ColumnPos::from(transform.translation),
            radius: simulated.min(viewed),
            simulation: SimulationArea::around(SectionPos::from(transform.translation), *distance),
        };
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
        wanted.moved_from(old, &mut loading, owned);
        held.set_if_neq(wanted);
    }
}

fn post(mail: Option<&mut TicketMail>, session: PlayerSession, ticket: Option<HotTicket>) {
    if let Some(mail) = mail {
        mail.0.push(TicketUpdate { session, ticket });
    }
}

pub(crate) fn move_remote_ticket(world: &mut World, holder: Entity, new: HotTicket) {
    let Some(old) = world.get::<HotTicket>(holder).copied() else {
        return;
    };
    let owned = world.get_resource::<OwnedColumns>().copied();
    let mut dims = world.query::<(Entity, &mut ColumnLoading)>();
    let Ok((dim, mut loading)) = dims.single_mut(world) else {
        return;
    };
    new.moved_from(old, &mut loading, owned.as_ref());
    if let Some(mut held) = world.get_mut::<HotTicket>(holder) {
        held.set_if_neq(new);
    }
    if let Some(mut area) = world.get_mut::<HeldSimulationArea>(holder) {
        area.set_if_neq(HeldSimulationArea::new(dim, new.simulation));
    }
}

pub(crate) fn raise_hot_ticket(
    add: On<Add<HotTicket>>,
    held: Query<&HotTicket>,
    owned: Option<Res<OwnedColumns>>,
    mut loading: Single<&mut ColumnLoading>,
) {
    if let Ok(ticket) = held.get(add.event().entity) {
        shift(
            ticket.columns(),
            &mut loading,
            owned.as_deref(),
            Change::Raise,
        );
    }
}

pub(crate) fn release_hot_ticket(
    discard: On<Discard<HotTicket>>,
    held: Query<(&HotTicket, Option<&Owner>, Has<CellCrossing>)>,
    owned: Option<Res<OwnedColumns>>,
    mut mail: Option<ResMut<TicketMail>>,
    mut loading: Single<&mut ColumnLoading>,
) {
    let Ok((ticket, owner, crossing)) = held.get(discard.event().entity) else {
        return;
    };
    shift(
        ticket.columns(),
        &mut loading,
        owned.as_deref(),
        Change::Lower,
    );
    if let Some(&Owner(session)) = owner
        && !crossing
    {
        post(mail.as_deref_mut(), session, None);
    }
}
