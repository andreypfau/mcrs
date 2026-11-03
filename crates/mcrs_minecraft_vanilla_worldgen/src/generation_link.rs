use std::time::Instant;

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Resource;
use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_engine_world::entity::player::chunk_view::ChunkTrackingView;
use mcrs_minecraft_engine_world::palette::{BiomePalette, ChunkBlocks};
use mcrs_minecraft_heightmap::ColumnHeightmapSet;
use mcrs_minecraft_simulation::PendingTick;
use mcrs_minecraft_worldgen_feature::placer::GenerationKinds;
use mcrs_minecraft_worldgen_feature_place::entity::GeneratedEntity;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_minecraft_worldgen_generator::task::ColumnSource;
use mcrs_voxel::ColumnPos;

/// What the dimension asks of its generation actor.
pub(crate) enum GenerationRequest {
    /// The context every stage runs against. Until it arrives the actor queues what is
    /// wanted and generates nothing.
    Context(Box<FillContext>),
    Want(WantedColumn),
    /// What the views and the players are this tick. The actor drops what left every view
    /// and orders what is left by the players' distance.
    Views(ViewUpdate),
}

/// Sections of one column that a view ticketed, to be delivered together.
pub(crate) struct WantedColumn {
    pub(crate) col: ColumnPos,
    /// Squared XZ distance to the nearest player when the sections were ticketed.
    pub(crate) distance_sq: i32,
    pub(crate) sections: Vec<RequestedSection>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RequestedSection {
    pub(crate) entity: Entity,
    pub(crate) y: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ViewUpdate {
    pub(crate) views: Vec<ChunkTrackingView>,
    pub(crate) players: Vec<ColumnPos>,
}

/// What the generation actor hands back to the dimension.
pub(crate) enum GenerationReport {
    Delivered(Box<DeliveredColumn>),
    /// Sections of columns that left every view before they were delivered.
    Abandoned(Vec<Entity>),
    /// The actor owes nothing for the first `seen` requests it was sent.
    Idle {
        seen: u64,
    },
}

/// A column at the top of its ladder, already in the form its sections take in the ECS.
pub(crate) struct DeliveredColumn {
    pub(crate) col: ColumnPos,
    /// When the column joined the queue.
    pub(crate) queued: Instant,
    pub(crate) source: ColumnSource,
    pub(crate) maps: Option<ColumnHeightmapSet>,
    pub(crate) block_entities: Vec<GeneratedBlockEntity>,
    pub(crate) entities: Vec<GeneratedEntity>,
    /// The sections the requests asked for, bottom first.
    pub(crate) sections: Vec<DeliveredSection>,
}

pub(crate) struct DeliveredSection {
    pub(crate) entity: Entity,
    pub(crate) content: SectionContent,
}

pub(crate) enum SectionContent {
    Blocks(SectionBlocks),
    /// The section has blocks, but no block table to check them against.
    Unchecked,
    Missing,
}

pub(crate) struct SectionBlocks {
    pub(crate) blocks: ChunkBlocks,
    pub(crate) biomes: BiomePalette,
    /// The ticks generation scheduled in the section, in the order it asked.
    pub(crate) ticks: Vec<PendingTick<GenerationKinds>>,
}

/// The dimension's end of the link to its generation actor.
#[derive(Resource)]
pub(crate) struct ColumnGenerator {
    requests: flume::Sender<GenerationRequest>,
    reports: flume::Receiver<GenerationReport>,
    /// Reports taken off the channel and not integrated yet, oldest first.
    held: Vec<GenerationReport>,
    sent: u64,
    idle_at: u64,
}

impl ColumnGenerator {
    pub(crate) fn send(&mut self, request: GenerationRequest) {
        // Fails only once the actor has gone, and then nothing in the dimension generates.
        if self.requests.send(request).is_ok() {
            self.sent = self.sent.wrapping_add(1);
        }
    }

    /// Every report not integrated yet, in the order the actor sent them.
    pub(crate) fn reports(&mut self) -> std::vec::Drain<'_, GenerationReport> {
        self.take_in();
        self.held.drain(..)
    }

    pub(crate) fn has_reports(&self) -> bool {
        !self.held.is_empty() || !self.reports.is_empty()
    }

    /// Whether the actor has answered every request sent so far. Takes its reports off the
    /// channel to tell, and integrates none of them.
    pub(crate) fn is_idle(&mut self) -> bool {
        self.take_in();
        self.idle_at == self.sent
    }

    fn take_in(&mut self) {
        for report in self.reports.try_iter() {
            match report {
                GenerationReport::Idle { seen } => self.idle_at = seen,
                report => self.held.push(report),
            }
        }
    }
}

/// The actor's end of the link, held until the actor starts.
#[derive(Resource)]
pub(crate) struct GeneratorInbox {
    pub(crate) requests: flume::Receiver<GenerationRequest>,
    pub(crate) reports: flume::Sender<GenerationReport>,
}

/// Both ends of a new link. Requests sent before the actor starts wait in the link.
pub(crate) struct GeneratorLink {
    pub(crate) generator: ColumnGenerator,
    pub(crate) inbox: GeneratorInbox,
}

impl Default for GeneratorLink {
    fn default() -> Self {
        Self::new()
    }
}

impl GeneratorLink {
    pub(crate) fn new() -> Self {
        let (request_sender, request_receiver) = flume::unbounded();
        let (report_sender, report_receiver) = flume::unbounded();
        Self {
            generator: ColumnGenerator {
                requests: request_sender,
                reports: report_receiver,
                held: Vec::new(),
                sent: 0,
                idle_at: 0,
            },
            inbox: GeneratorInbox {
                requests: request_receiver,
                reports: report_sender,
            },
        }
    }
}
