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
    BlockChange, CellBlockWrite, HaloBlockChanges, VoxelUpdateSet,
};
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_simulation::{ChainLedger, Message};
use mcrs_voxel::ColumnPos;

use crate::block_simulation::BlockSimulation;
use crate::cell_crossing::{self, Crossing};
use crate::column_asks::{ColumnAnswer, ColumnAsk, ColumnHold};
use crate::column_production::ColumnProduction;
use crate::column_store::ColumnStoreLink;
use crate::dim_tick::{DimInboxDrain, drain_dim_inbox};
use crate::foreign_views::ForeignViews;
use crate::hot_ticket::{RemoteTickets, TicketMail, TicketUpdate};
use crate::player_view::PlayerView;
use crate::world_kinds::WorldKinds;

/// What a cell hands the other cells of its world after one tick, or what the other cells
/// handed it for its next tick, each kind in the order of the cells.
#[derive(Debug, Default)]
pub struct CellMail {
    /// The hot tickets of the players of a cell that changed.
    pub(crate) tickets: Vec<TicketUpdate>,
    /// The changes of the blocks of the columns of a cell, for the halo of the cells that read
    /// them.
    pub(crate) halo: Vec<BlockChange>,
    /// The writes of a cell into the columns that another cell owns.
    pub(crate) writes: Vec<CellBlockWrite>,
    /// The players of a cell that walked into a column of another cell.
    pub(crate) crossings: Vec<Crossing>,
    /// The writes, ticks, events and updates of the block kernels, each for the cell that owns
    /// its section.
    pub(crate) messages: Vec<Message<WorldKinds>>,
    /// How far each chain that sent updates got, for every other cell.
    pub(crate) ledgers: Vec<ChainLedger>,
    /// The views of the players of a cell that changed.
    pub(crate) views: Vec<PlayerView>,
    /// The columns of a cell that another cell can read and that became hot, went cold or were
    /// written.
    pub(crate) holds: Vec<ColumnHold>,
    /// The columns that a cell asks of their owner.
    pub(crate) asks: Vec<ColumnAsk>,
    /// The answers of the owner of a column that a cell asked for.
    pub(crate) answers: Vec<ColumnAnswer>,
}

impl CellMail {
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.tickets.is_empty()
            && self.halo.is_empty()
            && self.writes.is_empty()
            && self.crossings.is_empty()
            && self.messages.is_empty()
            && self.ledgers.is_empty()
            && self.views.is_empty()
            && self.holds.is_empty()
            && self.asks.is_empty()
            && self.answers.is_empty()
    }

    /// Moves every message of `other` after the messages of the same kind.
    pub(crate) fn append(&mut self, other: &mut Self) {
        self.tickets.append(&mut other.tickets);
        self.halo.append(&mut other.halo);
        self.writes.append(&mut other.writes);
        self.crossings.append(&mut other.crossings);
        self.messages.append(&mut other.messages);
        self.ledgers.append(&mut other.ledgers);
        self.views.append(&mut other.views);
        self.holds.append(&mut other.holds);
        self.asks.append(&mut other.asks);
        self.answers.append(&mut other.answers);
    }
}

/// What the other cells sent for this tick, until the sync point applies it.
#[derive(Resource, Debug, Default)]
pub(crate) struct CellInbox(pub(crate) CellMail);

/// What this tick hands the other cells, as the systems of the tick write it.
#[derive(Resource, Debug, Default)]
pub(crate) struct CellOutbox(pub(crate) CellMail);

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
        app.init_resource::<CellOutbox>();
        app.init_resource::<TicketMail>();
        app.init_resource::<RemoteTickets>();
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

/// Applies the messages of the other cells: each kind in the order the coordinator gave, and the
/// halo, the writes and then the players that arrive last.
fn receive_cell_mail(world: &mut World) {
    let Some(mut inbox) = world.get_resource_mut::<CellInbox>() else {
        return;
    };
    let CellMail {
        tickets,
        halo,
        writes,
        crossings,
        mut messages,
        mut ledgers,
        views,
        holds,
        asks,
        answers,
    } = mem::take(&mut inbox.0);
    let dimension = world
        .query_filtered::<Entity, With<Dimension>>()
        .single(world)
        .ok();
    for update in tickets {
        cell_crossing::update_remote(world, update.session, update.ticket);
    }
    if let Some(mut kernels) = world.get_resource_mut::<BlockSimulation>() {
        kernels.receive(&mut messages, &mut ledgers);
    }
    if let Some(mut foreign) = world.get_resource_mut::<ForeignViews>() {
        for view in views {
            foreign.apply(view);
        }
    }
    if let Some(mut store) = world.get_resource_mut::<ColumnStoreLink>() {
        for hold in holds {
            store.note_hold(hold);
        }
        for ask in asks {
            store.receive_ask(ask);
        }
        // After the reports of the same tick, so a column that its owner let go of is asked of
        // the data layer when its owner wrote it.
        for answer in answers {
            store.receive_answer(answer);
        }
    }
    if let Some(mut production) = world.get_resource_mut::<ColumnProduction>() {
        production.note_halo_changes(halo.iter().copied());
    }
    if let Some(mut changes) = world.get_resource_mut::<HaloBlockChanges>() {
        changes.0.extend(halo);
    }
    if let Some(dimension) = dimension {
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

/// Keeps each change of a column of this cell that another cell reads, for its halo.
fn collect_halo_changes(
    mut placed: MessageReader<BlockPlaced>,
    owned: Res<OwnedColumns>,
    mut outbox: ResMut<CellOutbox>,
) {
    for placed in placed.read() {
        let column = ColumnPos::from(placed.block_pos);
        if owned.owns(column) && owned.is_read_by_others(column) {
            outbox.0.halo.push(BlockChange {
                pos: placed.block_pos,
                state: placed.new_state,
            });
        }
    }
}

/// Moves what this tick hands the other cells into `mail`, after the players that leave the
/// world and the cell.
pub(crate) fn take_cell_mail(world: &mut World, mail: &mut CellMail) {
    if let Some(mut outbox) = world.get_resource_mut::<CellOutbox>() {
        mail.append(&mut outbox.0);
    }
    cell_crossing::cross_players(world, &mut mail.crossings);
    if let Some(mut tickets) = world.get_resource_mut::<TicketMail>() {
        mail.tickets.append(&mut tickets.0);
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
