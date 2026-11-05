//! The host side of the views: it hands what each dimension published for the views to the
//! [`ColumnViewStage`] on the link of the muxer, and hands what the stage sends back to the
//! clients and to the dimensions.

use bevy_ecs::entity::Entity;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Query, ResMut, Resource};
use bevy_ecs::world::{Mut, World};
use bevy_tasks::block_on;
use flume::Sender;
use mcrs_engine_link::{Link, Placement};
use mcrs_minecraft_engine_world::session::{
    PlayerSession, Session, SessionAnchors, SessionPlacement,
};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChunkBatchReceived;
use rustc_hash::FxHashMap;

use crate::muxer::column_view_stage::{ColumnViewInput, ColumnViewOutput, ColumnViewStage};
use crate::muxer::column_views::{DimColumns, DimRequests};
use crate::muxer::dim_inputs::DimInputs;
use crate::muxer::dim_outputs;
use crate::muxer::entity_replication::Answers;
use crate::muxer::muxer_placement::MuxerPlacement;
use crate::muxer::session::HostAnchorRef;
use crate::muxer::tick_wake::TickWake;

/// The link to the views of the players, placed as the muxer is.
#[derive(Resource)]
pub struct ColumnViewLink {
    link: Link<ColumnViewStage>,
    /// The dimension whose view each session holds, as the dimensions reported it. The host
    /// holds the placement of each session, so it tells the stage which views wait for their
    /// session to arrive.
    view_dims: FxHashMap<PlayerSession, Entity>,
}

impl ColumnViewLink {
    /// Stages on a thread of their own send `wake` a message each time they hand out output.
    #[must_use]
    pub fn new(placement: Placement, wake: Sender<()>) -> Self {
        Self {
            link: Link::new_with_wake(ColumnViewStage::default(), placement, wake),
            view_dims: FxHashMap::default(),
        }
    }

    /// The stage, when it runs on the host thread.
    #[must_use]
    pub fn inline_stage(&self) -> Option<&ColumnViewStage> {
        self.link.inline_stage()
    }

    /// The client of `session` has taken a batch and says how many columns a tick it managed.
    pub fn acknowledge_batch(&mut self, session: PlayerSession, desired_columns_per_tick: f32) {
        self.link.send(ColumnViewInput::Acknowledge {
            session,
            desired_columns_per_tick,
        });
    }

    fn take_dim(&mut self, dim: Entity, columns: DimColumns) {
        for view in &columns.views {
            match view.view {
                Some(_) => {
                    self.view_dims.insert(view.session, dim);
                }
                None => {
                    if self.view_dims.get(&view.session) == Some(&dim) {
                        self.view_dims.remove(&view.session);
                    }
                }
            }
        }
        self.link.send(ColumnViewInput::Dim { dim, columns });
    }

    /// Lets go of every view of the dimension `dim`.
    pub fn close_dim(&mut self, dim: Entity) {
        self.view_dims.retain(|_, view_dim| *view_dim != dim);
        self.link.send(ColumnViewInput::CloseDim { dim });
    }
}

/// Hands what the dimension `dim` published in one tick to the views.
pub(crate) fn receive(world: &mut World, dim: Entity, columns: DimColumns) {
    if columns.is_empty() {
        return;
    }
    link_mut(world).take_dim(dim, columns);
}

/// Ends what the host took between two ticks: the views send their next batches to every
/// player whose session is in the dimension of its view.
pub(crate) fn send(world: &mut World) {
    let mut blocked = Vec::new();
    if let Some(link) = world.get_resource::<ColumnViewLink>()
        && let Some(anchors) = world.get_resource::<SessionAnchors>()
    {
        // A session placed in another dimension than its view gets no column of the old one
        // after a respawn.
        blocked.extend(
            link.view_dims
                .iter()
                .filter(|(session, dim)| {
                    anchors
                        .get(**session)
                        .and_then(|anchor| world.get::<SessionPlacement>(anchor))
                        .is_some_and(|placement| placement.place().dim() != Some(**dim))
                })
                .map(|(session, _)| *session),
        );
    }
    link_mut(world).link.send(ColumnViewInput::Send { blocked });
}

/// Hands the packets that the views sent to the clients, and their requests to the
/// dimensions.
pub fn deliver(world: &mut World, answers: Answers) {
    if !world.contains_resource::<ColumnViewLink>() {
        return;
    }
    world.resource_scope(|world, mut link: Mut<ColumnViewLink>| {
        if answers == Answers::All {
            block_on(link.link.answered());
        }
        for output in link.link.outputs() {
            match output {
                ColumnViewOutput::Packets(packets) => dim_outputs::deliver_all(world, packets),
                ColumnViewOutput::Requests(DimRequests { dim, requests }) => {
                    if let Some(mut inputs) = world.get_resource_mut::<DimInputs>() {
                        inputs.send_column_requests(dim, requests);
                    }
                }
            }
        }
    });
}

/// The client has taken a batch.
pub fn acknowledge_batch(
    on: On<Received<ServerboundChunkBatchReceived>>,
    connections: Query<&HostAnchorRef>,
    sessions: Query<&Session>,
    link: Option<ResMut<ColumnViewLink>>,
) {
    let (Some(ack), Some(mut link)) = (on.packet(), link) else {
        return;
    };
    let Ok(&HostAnchorRef(anchor)) = connections.get(on.entity) else {
        return;
    };
    if let Ok(&Session(session)) = sessions.get(anchor) {
        link.acknowledge_batch(session, ack.desired_chunks_per_tick);
    }
}

fn link_mut(world: &mut World) -> Mut<'_, ColumnViewLink> {
    if !world.contains_resource::<ColumnViewLink>() {
        let placement = world
            .get_resource::<MuxerPlacement>()
            .map_or(Placement::Inline, |placement| placement.0);
        let wake = world.get_resource_or_init::<TickWake>().waker();
        world.insert_resource(ColumnViewLink::new(placement, wake));
    }
    world.resource_mut::<ColumnViewLink>()
}
