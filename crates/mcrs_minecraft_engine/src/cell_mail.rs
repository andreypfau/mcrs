use std::mem;

use bevy_app::{App, FixedPostUpdate, FixedPreUpdate, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter, Messages};
use bevy_ecs::prelude::{Query, With};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::on_message;
use bevy_ecs::system::{Res, ResMut};
use bevy_ecs::world::World;
use mcrs_minecraft_engine_world::block_update::{BlockPlaced, BlockSetRequest};
use mcrs_minecraft_engine_world::owned_columns::OwnedColumns;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::voxel_update::{
    self, BlockChange, HaloBlockChanges, VoxelUpdateSet,
};
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::storage::view::WorldViewMut;
use mcrs_voxel::ColumnPos;

use crate::block_mail::{BlockMail, BlockOutbox};
use crate::block_simulation::BlockSimulation;
use crate::cell_crossing::{self, Crossing};
use crate::column_light::ColdStageLandSet;
use crate::column_production::ColumnProduction;
use crate::column_store::{ColumnStoreLink, StoredColumnLandSet};
use crate::column_store_mail::ColumnStoreMail;
use crate::dim_inbox_drain::DimInboxDrain;
use crate::foreign_views::ForeignViews;
use crate::hot_ticket::{RemoteTickets, TicketMail, TicketUpdate};
use crate::player_view::PlayerView;

#[derive(Debug, Default)]
pub struct CellMail {
    pub(crate) tickets: Vec<TicketUpdate>,
    pub(crate) halo: Vec<BlockChange>,
    pub(crate) crossings: Vec<Crossing>,
    pub(crate) views: Vec<PlayerView>,
    pub(crate) blocks: BlockMail,
    pub(crate) store: ColumnStoreMail,
}

impl CellMail {
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.tickets.is_empty()
            && self.halo.is_empty()
            && self.crossings.is_empty()
            && self.views.is_empty()
            && self.blocks.is_empty()
            && self.store.is_empty()
    }

    pub(crate) fn append(&mut self, other: &mut Self) {
        self.tickets.append(&mut other.tickets);
        self.halo.append(&mut other.halo);
        self.crossings.append(&mut other.crossings);
        self.views.append(&mut other.views);
        self.blocks.append(&mut other.blocks);
        self.store.append(&mut other.store);
    }
}

#[derive(Resource, Debug, Default)]
pub(crate) struct CellInbox(pub(crate) CellMail);

#[derive(Resource, Debug, Default)]
pub(crate) struct CellOutbox(pub(crate) CellMail);

pub struct CellMailPlugin {
    pub owned: OwnedColumns,
}

impl Plugin for CellMailPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.owned);
        app.init_resource::<CellInbox>();
        app.init_resource::<CellOutbox>();
        app.init_resource::<BlockOutbox>();
        app.init_resource::<TicketMail>();
        app.init_resource::<RemoteTickets>();
        app.init_resource::<HaloBlockChanges>();
        app.init_resource::<ForeignViews>();
        app.add_systems(
            FixedPreUpdate,
            receive_cell_mail
                .run_if(inbox_holds_messages)
                .in_set(DimInboxDrain::Before),
        );
        app.add_systems(
            FixedUpdate,
            apply_halo_changes
                .run_if(halo_changes_pending)
                .in_set(TickSet::DomainInput)
                .after(StoredColumnLandSet)
                .after(ColdStageLandSet),
        );
        app.add_systems(
            FixedPostUpdate,
            collect_halo_changes
                .in_set(VoxelUpdateSet::NetworkSync)
                .run_if(on_message::<BlockPlaced>),
        );
    }
}

#[cfg_attr(mcrs_lint, mcrs::event_condition)]
fn inbox_holds_messages(inbox: Res<CellInbox>) -> bool {
    !inbox.0.is_empty()
}

fn halo_changes_pending(halo: Res<HaloBlockChanges>) -> bool {
    !halo.0.is_empty()
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::exclusive_system,
        reason = "spawns the players that cross from another cell with their persisted components, which needs the whole world"
    )
)]
fn receive_cell_mail(world: &mut World) {
    let Some(mut inbox) = world.get_resource_mut::<CellInbox>() else {
        return;
    };
    let CellMail {
        tickets,
        halo,
        crossings,
        views,
        blocks:
            BlockMail {
                writes,
                mut messages,
                mut ledgers,
            },
        store: ColumnStoreMail {
            holds,
            asks,
            answers,
        },
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
    if let Some(dimension) = dimension
        && let Some(mut requests) = world.get_resource_mut::<Messages<BlockSetRequest>>()
    {
        requests.write_batch(writes.into_iter().map(|write| BlockSetRequest {
            dimension,
            pos: write.pos,
            new_state: write.new_state,
            flags: write.flags,
            recursion_left: write.recursion_left,
        }));
    }
    for crossing in crossings {
        if let Some(mut views) = world.get_resource_mut::<ForeignViews>() {
            views.forget(crossing.session);
        }
        if let Err(error) = cell_crossing::arrive(world, crossing) {
            tracing::error!("a player did not arrive from another cell: {error}");
        }
    }
}

fn apply_halo_changes(
    mut blocks: WorldViewMut,
    mut placed: MessageWriter<BlockPlaced>,
    dimensions: Query<Entity, With<Dimension>>,
    mut halo: ResMut<HaloBlockChanges>,
) {
    let Ok(dimension) = dimensions.single() else {
        return;
    };
    voxel_update::apply_halo_changes(&mut blocks, &mut placed, dimension, &mut halo.0);
}

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

pub(crate) fn take_cell_mail(world: &mut World, mail: &mut CellMail) {
    if let Some(mut outbox) = world.get_resource_mut::<CellOutbox>() {
        mail.append(&mut outbox.0);
    }
    if let Some(mut outbox) = world.get_resource_mut::<BlockOutbox>() {
        mail.blocks.append(&mut outbox.0);
    }
    cell_crossing::cross_players(world, &mut mail.crossings);
    if let Some(mut tickets) = world.get_resource_mut::<TicketMail>() {
        mail.tickets.append(&mut tickets.0);
    }
    if let Some(mut store) = world.get_resource_mut::<ColumnStoreLink>() {
        store.take_mail(&mut mail.store);
    }
    if world.contains_resource::<OwnedColumns>()
        && let Some(views) = world.get_resource::<Messages<PlayerView>>()
    {
        mail.views
            .extend(views.iter_current_update_messages().copied());
    }
}
