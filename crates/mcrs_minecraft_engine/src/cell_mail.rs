//! The messages between the cells of one world. A cell writes them during tick `T`, and the
//! coordinator hands them to the other cells in the input of tick `T + 1`.

use std::mem;

use bevy_app::{App, FixedPostUpdate, FixedPreUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, Messages};
use bevy_ecs::prelude::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::{Res, ResMut};
use bevy_ecs::world::World;
use mcrs_minecraft_engine_world::block_update::{BlockPlaced, BlockSetRequest};
use mcrs_minecraft_engine_world::entity::add_entity_to_chunk;
use mcrs_minecraft_engine_world::owned_columns::OwnedColumns;
use mcrs_minecraft_engine_world::voxel_update::{
    CellBlockWrite, ForeignBlockWrites, HaloBlockChange, HaloBlockChanges, VoxelUpdateSet,
};
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_simulation::BlockUpdateFlags;

use crate::block_simulation::{BlockSimulation, SimulationMail};
use crate::cell_crossing::{self, Crossing};
use crate::column_asks::{ColumnAnswer, ColumnAsk, ColumnHold};
use crate::column_production::{BlockChange, ColumnProduction};
use crate::column_store::ColumnStoreLink;
use crate::dim_tick::{DimInboxDrain, drain_dim_inbox};
use crate::foreign_views::ForeignViews;
use crate::hot_ticket::{RemoteTickets, TicketMail, TicketUpdate};
use crate::player_view::PlayerView;

/// A message from another cell of the same world.
#[derive(Clone, Debug)]
pub enum CellMessage {
    /// The hot ticket of a player of the other cell changed.
    Ticket(TicketUpdate),
    /// A block of a column of the other cell changed: the halo of this cell takes the change.
    Halo(CellBlockWrite),
    /// The other cell wrote a block of a column of this cell.
    Write(CellBlockWrite),
    /// A player of the other cell walked into a column of this cell.
    Cross(Box<Crossing>),
    /// A message of the block kernels of the other cell.
    Simulation(SimulationMail),
    /// The view of a player of the other cell changed.
    View(PlayerView),
    /// A column of the other cell that this cell can read became hot, went cold or was written.
    Hold(ColumnHold),
    /// The other cell asks for a column of this cell.
    Ask(ColumnAsk),
    /// The owner of a column that this cell asked for answers.
    Answer(ColumnAnswer),
}

/// What a cell hands the other cells of its world after one tick.
#[derive(Debug, Default)]
pub struct CellMail {
    pub(crate) tickets: Vec<TicketUpdate>,
    pub(crate) halo: Vec<CellBlockWrite>,
    pub(crate) writes: Vec<CellBlockWrite>,
    pub(crate) crossings: Vec<Crossing>,
    pub(crate) simulation: Vec<SimulationMail>,
    pub(crate) views: Vec<PlayerView>,
    pub(crate) holds: Vec<ColumnHold>,
    pub(crate) asks: Vec<ColumnAsk>,
    pub(crate) answers: Vec<ColumnAnswer>,
}

/// The messages of the other cells for this tick, until the sync point applies them.
#[derive(Resource, Debug, Default)]
pub(crate) struct CellInbox(pub(crate) Vec<CellMessage>);

/// The changes of this tick to the columns that this cell owns, for the halo of the others.
#[derive(Resource, Debug, Default)]
pub(crate) struct HaloMail(pub(crate) Vec<CellBlockWrite>);

/// The part of a cell that talks to the other cells of its world: it owns the columns of
/// `owned`, takes in the messages of the other cells first at the sync point, and hands its own
/// out at the end of the tick.
pub struct CellMailPlugin {
    pub owned: OwnedColumns,
}

impl Plugin for CellMailPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.owned);
        app.init_resource::<CellInbox>();
        app.init_resource::<HaloMail>();
        app.init_resource::<TicketMail>();
        app.init_resource::<RemoteTickets>();
        app.init_resource::<ForeignBlockWrites>();
        app.init_resource::<HaloBlockChanges>();
        app.init_resource::<ForeignViews>();
        app.add_systems(
            FixedPreUpdate,
            receive_cell_mail
                .run_if(inbox_holds_messages)
                .in_set(DimInboxDrain)
                .before(drain_dim_inbox)
                .before(add_entity_to_chunk),
        );
        app.add_systems(
            FixedPostUpdate,
            collect_halo_changes.in_set(VoxelUpdateSet::NetworkSync),
        );
    }
}

fn inbox_holds_messages(inbox: Res<CellInbox>) -> bool {
    !inbox.0.is_empty()
}

/// Applies the messages of the other cells: the tickets, the halo, the writes and then the
/// players that arrive, each kind in the order the coordinator gave.
fn receive_cell_mail(world: &mut World) {
    let Some(mut inbox) = world.get_resource_mut::<CellInbox>() else {
        return;
    };
    let messages = mem::take(&mut inbox.0);
    let dimension = world
        .query_filtered::<Entity, With<Dimension>>()
        .single(world)
        .ok();
    let mut halo = Vec::new();
    let mut writes = Vec::new();
    let mut crossings = Vec::new();
    let mut answers = Vec::new();
    for message in messages {
        match message {
            CellMessage::Ticket(update) => {
                cell_crossing::update_remote(world, update.session, update.ticket);
            }
            CellMessage::Halo(write) => halo.push(write),
            CellMessage::Write(write) => writes.push(write),
            CellMessage::Cross(crossing) => crossings.push(*crossing),
            CellMessage::Simulation(mail) => {
                if let Some(mut simulation) = world.get_resource_mut::<BlockSimulation>() {
                    simulation.receive(mail);
                }
            }
            CellMessage::View(view) => {
                if let Some(mut views) = world.get_resource_mut::<ForeignViews>() {
                    views.apply(view);
                }
            }
            CellMessage::Hold(hold) => {
                if let Some(mut store) = world.get_resource_mut::<ColumnStoreLink>() {
                    store.note_hold(hold);
                }
            }
            CellMessage::Ask(ask) => {
                if let Some(mut store) = world.get_resource_mut::<ColumnStoreLink>() {
                    store.receive_ask(ask);
                }
            }
            CellMessage::Answer(answer) => answers.push(answer),
        }
    }
    // After the reports of the same tick, so a column that its owner let go of is asked of the
    // data layer when its owner wrote it.
    if let Some(mut store) = world.get_resource_mut::<ColumnStoreLink>() {
        for answer in answers {
            store.receive_answer(answer);
        }
    }
    if let Some(mut production) = world.get_resource_mut::<ColumnProduction>() {
        production.note_halo_changes(halo.iter().map(|write| BlockChange {
            pos: write.pos,
            state: write.new_state,
        }));
    }
    if let Some(dimension) = dimension {
        if let Some(mut changes) = world.get_resource_mut::<HaloBlockChanges>() {
            changes.0.extend(
                halo.into_iter()
                    .map(|write| HaloBlockChange { dimension, write }),
            );
        }
        if let Some(mut requests) = world.get_resource_mut::<Messages<BlockSetRequest>>() {
            requests.write_batch(writes.into_iter().map(|write| BlockSetRequest {
                dimension,
                pos: write.pos,
                new_state: write.new_state,
                flags: write.flags,
                recursion_left: write.recursion_left,
            }));
        }
    }
    for crossing in crossings {
        if let Some(mut views) = world.get_resource_mut::<ForeignViews>() {
            views.forget(crossing.session());
        }
        if let Err(error) = cell_crossing::arrive(world, crossing) {
            tracing::error!("a player did not arrive from another cell: {error}");
        }
    }
}

/// Keeps each change of a column of this cell for the halo of the other cells. A halo copy
/// takes the state only, so the change goes with no flags.
fn collect_halo_changes(
    mut placed: MessageReader<BlockPlaced>,
    owned: Res<OwnedColumns>,
    mut mail: ResMut<HaloMail>,
) {
    for placed in placed.read() {
        if owned.owns_block(placed.block_pos) {
            mail.0.push(CellBlockWrite {
                pos: placed.block_pos,
                new_state: placed.new_state,
                flags: BlockUpdateFlags::empty(),
                recursion_left: 0,
            });
        }
    }
}

/// Moves what this tick hands the other cells into `mail`, after the players that leave the
/// world and the cell.
pub(crate) fn take_cell_mail(world: &mut World, mail: &mut CellMail) {
    cell_crossing::cross_players(world, &mut mail.crossings);
    if let Some(mut tickets) = world.get_resource_mut::<TicketMail>() {
        mail.tickets.append(&mut tickets.0);
    }
    if let Some(mut halo) = world.get_resource_mut::<HaloMail>() {
        mail.halo.append(&mut halo.0);
    }
    if let Some(mut writes) = world.get_resource_mut::<ForeignBlockWrites>() {
        mail.writes.append(&mut writes.0);
    }
    if let Some(mut simulation) = world.get_resource_mut::<BlockSimulation>() {
        simulation.take_mail(&mut mail.simulation);
    }
    if let Some(mut store) = world.get_resource_mut::<ColumnStoreLink>() {
        store.take_mail(mail);
    }
    if world.contains_resource::<OwnedColumns>()
        && let Some(views) = world.get_resource::<Messages<PlayerView>>()
    {
        mail.views
            .extend(views.iter_current_update_messages().copied());
    }
}
