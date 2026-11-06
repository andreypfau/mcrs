//! The host side of the muxer: it hands what each dimension published to the [`MuxerStage`] on
//! the link of the muxer, and hands what the stage sends back to the clients and to the
//! dimensions.

use bevy_ecs::entity::Entity;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Query, ResMut, Resource};
use bevy_ecs::world::{FromWorld, Mut, World};
use bevy_tasks::block_on;
use mcrs_engine_link::{Link, Placement};
use mcrs_engine_muxer::ViewerBytes;
use mcrs_minecraft_engine_world::session::{
    PlayerSession, Session, SessionAnchors, SessionPlacement,
};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChunkBatchReceived;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::bus::{PacketPayload, to};
use crate::muxer::column_views::{DimColumns, DimRequests};
use crate::muxer::dim_inputs::DimInputs;
use crate::muxer::dim_outputs;
use crate::muxer::entity_replication::EntityReplication;
use crate::muxer::muxer_placement::MuxerPlacement;
use crate::muxer::muxer_stage::{MuxerCall, MuxerOutput, MuxerStage};
use crate::muxer::session::HostAnchorRef;
use crate::muxer::tick_wake::TickWake;

/// Whether [`deliver`] waits for the muxer to answer every call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answers {
    /// Takes what the muxer has handed out so far.
    Ready,
    /// Waits until the muxer owes nothing, then takes what it handed out.
    All,
}

/// The link to the muxer, placed as the [`MuxerPlacement`] says, and what the host keeps of the
/// views and the entities that the muxer serves.
#[derive(Resource)]
pub struct MuxerLink {
    pub(crate) link: Link<MuxerStage>,
    /// The dimension whose view each session holds, as the dimensions reported it. The host
    /// holds the placement of each session, so it tells the stage which views wait for their
    /// session to arrive.
    view_dims: FxHashMap<PlayerSession, Entity>,
    pub(crate) entities: EntityReplication,
}

/// Placed as the [`MuxerPlacement`] says, inline without one. A stage on a thread of its own
/// wakes the [`TickWake`] each time it hands out output.
impl FromWorld for MuxerLink {
    fn from_world(world: &mut World) -> Self {
        let placement = world
            .get_resource::<MuxerPlacement>()
            .map_or(Placement::Inline, |placement| placement.0);
        let wake = world.get_resource_or_init::<TickWake>().waker();
        Self {
            link: Link::new_with_wake(MuxerStage::default(), placement, wake),
            view_dims: FxHashMap::default(),
            entities: EntityReplication::default(),
        }
    }
}

impl MuxerLink {
    /// The stage, when it runs on the host thread.
    #[cfg(feature = "world-access")]
    #[must_use]
    pub fn inline_stage(&self) -> Option<&MuxerStage> {
        self.link.inline_stage()
    }

    /// The tile deltas handed to the muxer so far. The muxer encodes each one once.
    #[cfg(feature = "world-access")]
    #[must_use]
    pub fn published_tiles(&self) -> u64 {
        self.entities.published_tiles
    }

    /// The client of `session` has taken a batch and says how many columns a tick it managed.
    pub fn acknowledge_batch(&mut self, session: PlayerSession, desired_columns_per_tick: f32) {
        self.link.send(MuxerCall::Acknowledge {
            session,
            desired_columns_per_tick,
        });
    }

    /// Hands `latency` of the player of `session` to the muxer of the world it watches.
    pub fn send_latency(&mut self, session: PlayerSession, latency: i32) {
        self.entities.send_latency(&mut self.link, session, latency);
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
        self.link.send(MuxerCall::Dim { dim, columns });
    }

    /// Lets go of every view and the entity muxer of the dimension `dim`.
    pub fn close_dim(&mut self, dim: Entity) {
        self.entities.close(&mut self.link, dim);
        self.view_dims.retain(|_, view_dim| *view_dim != dim);
        self.link.send(MuxerCall::CloseDim { dim });
    }
}

/// Hands what the dimension `dim` published in one tick to the views.
pub(crate) fn receive(world: &mut World, dim: Entity, columns: DimColumns) {
    if columns.is_empty() {
        return;
    }
    world
        .get_resource_or_init::<MuxerLink>()
        .take_dim(dim, columns);
}

/// Ends what the host took between two ticks: the views send their next batches to every
/// player whose session is in the dimension of its view.
pub(crate) fn send(world: &mut World) {
    let mut blocked = FxHashSet::default();
    if let Some(link) = world.get_resource::<MuxerLink>()
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
    world
        .get_resource_or_init::<MuxerLink>()
        .link
        .send(MuxerCall::Send { blocked });
}

/// Hands the packets of the views and the entity bytes that the muxer sent to the clients, in
/// the order it sent them, and the requests of the views to the dimensions.
pub fn deliver(world: &mut World, answers: Answers) {
    if !world.contains_resource::<MuxerLink>() {
        return;
    }
    world.resource_scope(|world, mut link: Mut<MuxerLink>| {
        if answers == Answers::All {
            block_on(link.link.answered());
        }
        let mut packets = Vec::new();
        for output in link.link.outputs() {
            match output {
                MuxerOutput::Packets(sent) => packets.extend(sent),
                MuxerOutput::Requests(DimRequests { dim, requests }) => {
                    if let Some(mut inputs) = world.get_resource_mut::<DimInputs>() {
                        inputs.send_column_requests(dim, requests);
                    }
                }
                MuxerOutput::Entities(ViewerBytes { viewer, chunks }) => packets.push(to(
                    PlayerSession(viewer.0),
                    PacketPayload::Replication(chunks),
                )),
            }
        }
        if !packets.is_empty() {
            dim_outputs::deliver_all(world, packets);
        }
    });
}

/// The client has taken a batch.
pub fn acknowledge_batch(
    on: On<Received<ServerboundChunkBatchReceived>>,
    connections: Query<&HostAnchorRef>,
    sessions: Query<&Session>,
    link: Option<ResMut<MuxerLink>>,
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
