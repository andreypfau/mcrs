use crate::command_limits::CommandLimits;
use crate::command_outbox::CommandOutbox;
use crate::command_queue::CommandQueue;
use crate::command_set::CommandSet;
use crate::command_targets::CommandTargets;
use crate::command_tree::CommandTree;
use crate::dimension_entry::DimensionEntry;
use crate::executor::PassExecutor;
use crate::player_entry::PlayerEntry;
use crate::registrar::Handler;
use crate::running_command::RunningCommand;
use crate::source_state::SourceState;
use bevy_app::{App, SubApp};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::With;
use bevy_ecs::query::QueryState;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::{World, WorldId};
use mcrs_command::{Limits, SourceLists, Stage};
use mcrs_minecraft_core::{Id, ResourceKey};
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine::dimension_order::{DimensionWorld, in_dimension_order};
use mcrs_minecraft_engine::session::{GameProfile, HostAnchor};
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::session::{Session, SessionPlacement};
use mcrs_minecraft_level::world::dimension::{Dimension, DimensionTypeId};
use mcrs_minecraft_level::world::sub_app::DimensionLabel;
use mcrs_minecraft_registries::CurrentRegistries;

/// The host's buffers for running command lines, reused from one run to the next.
#[derive(Resource)]
pub(crate) struct PassBuffers<S: CommandSet> {
    tree: CommandTree<S>,
    targets: CommandTargets,
    running: RunningCommand,
    outbox: CommandOutbox,
    stages: Vec<Stage<Handler>>,
    lists: SourceLists<SourceState>,
    queue: CommandQueue,
    queries: Vec<WorldQueries>,
    found: Vec<DimensionWorld<'static, FoundDimension>>,
    worlds: Vec<&'static mut World>,
    query_of: Vec<usize>,
}

impl<S: CommandSet> Default for PassBuffers<S> {
    fn default() -> Self {
        Self {
            tree: CommandTree::default(),
            targets: CommandTargets::default(),
            running: RunningCommand::default(),
            outbox: CommandOutbox::default(),
            stages: Vec::new(),
            lists: SourceLists::default(),
            queue: CommandQueue::default(),
            queries: Vec::new(),
            found: Vec::new(),
            worlds: Vec::new(),
            query_of: Vec::new(),
        }
    }
}

/// A dimension world that a pass found, and the index of its queries.
struct FoundDimension {
    entry: DimensionEntry,
    queries: usize,
}

type PlayerRow = (
    Entity,
    &'static HostAnchor,
    &'static GameProfile,
    &'static Transform,
    &'static PlayerGameMode,
);

type DimensionRow = (&'static ResourceKey<DimensionKey>, &'static DimensionTypeId);

/// The queries a pass runs in one dimension world, kept from one pass to the next.
struct WorldQueries {
    world: WorldId,
    players: QueryState<PlayerRow, With<Player>>,
    dimension: QueryState<DimensionRow, With<Dimension>>,
}

impl WorldQueries {
    /// The index of the queries of `world`, built on its first pass.
    fn index(queries: &mut Vec<WorldQueries>, world: &mut World) -> usize {
        let id = world.id();
        queries
            .iter()
            .position(|query| query.world == id)
            .unwrap_or_else(|| {
                queries.push(WorldQueries {
                    world: id,
                    players: world.query_filtered(),
                    dimension: world.query_filtered(),
                });
                queries.len() - 1
            })
    }
}

/// Parses and runs the command lines the host received, one after another in the order they
/// arrived, then answers the suggestion requests, with every dimension world in reach. The
/// server calls it between two ticks, after the dimensions hand over their packets; an embedder
/// that drives the app itself calls it at the same point, with the set its
/// [`CommandPlugin`](crate::CommandPlugin) names.
pub fn run_pending_commands<S: CommandSet>(app: &mut App) {
    let sub_apps = app.sub_apps_mut();
    let host = sub_apps.main.world_mut();
    if host
        .get_resource::<CommandQueue>()
        .is_none_or(CommandQueue::is_empty)
    {
        return;
    }
    let Some(mut buffers) = host.remove_resource::<PassBuffers<S>>() else {
        return;
    };
    let limits = host
        .get_resource::<CommandLimits>()
        .copied()
        .unwrap_or_default()
        .0;
    if let Some(mut received) = host.get_resource_mut::<CommandQueue>() {
        std::mem::swap(&mut *received, &mut buffers.queue);
    }
    buffers.swap_tree(host);
    buffers.queries.retain(|query| {
        sub_apps
            .sub_apps
            .values()
            .any(|sub| sub.world().id() == query.world)
    });
    let mut worlds = buffers.order_worlds(sub_apps.sub_apps.values_mut().map(SubApp::world_mut));
    buffers.collect_players(host, &worlds);
    buffers.run_queue(host, &mut worlds, limits);
    buffers.worlds = mcrs_command::recycle(worlds);
    buffers.swap_tree(host);
    host.insert_resource(buffers);
}

impl<S: CommandSet> PassBuffers<S> {
    /// Lends the graph of the host to the pass; a second swap gives it back.
    fn swap_tree(&mut self, host: &mut World) {
        if let Some(mut owned) = host.get_resource_mut::<CommandTree<S>>() {
            std::mem::swap(&mut *owned, &mut self.tree);
        }
    }

    /// The dimension worlds in the dimension order, with their entries in the targets and the
    /// index of their queries in `query_of`.
    fn order_worlds<'w>(
        &mut self,
        worlds: impl Iterator<Item = &'w mut World>,
    ) -> Vec<&'w mut World> {
        let queries = &mut self.queries;
        let mut ordered = mcrs_command::recycle(core::mem::take(&mut self.found));
        in_dimension_order(
            worlds,
            |world| {
                let index = WorldQueries::index(queries, world);
                let entry = dimension_entry(world, queries.get_mut(index)?)?;
                Some(FoundDimension {
                    entry,
                    queries: index,
                })
            },
            |found| &found.entry.key,
            &mut ordered,
        );
        self.targets.dimensions.clear();
        self.query_of.clear();
        let mut in_order = mcrs_command::recycle(core::mem::take(&mut self.worlds));
        for DimensionWorld { read, world } in ordered.drain(..) {
            self.targets.dimensions.push(read.entry);
            self.query_of.push(read.queries);
            in_order.push(world);
        }
        self.found = mcrs_command::recycle(ordered);
        in_order
    }

    /// Runs the queued lines in the order they arrived, then answers the suggestion requests.
    fn run_queue(&mut self, host: &mut World, worlds: &mut [&mut World], limits: Limits) {
        let mut executor = PassExecutor {
            host,
            worlds,
            tree: &mut self.tree,
            targets: &mut self.targets,
            running: &mut self.running,
            outbox: &mut self.outbox,
            output: Entity::PLACEHOLDER,
        };
        for queued in self.queue.lines.drain(..) {
            executor.output = queued.player;
            executor.run_line(queued, &mut self.stages, &mut self.lists, limits);
            let line = std::mem::take(&mut executor.running.line);
            if line.capacity() > 0 {
                self.queue.spare.push(line);
            }
        }
        for queued in self.queue.suggestions.drain(..) {
            executor.output = queued.player;
            executor.answer(&queued);
        }
    }

    /// Every player that its session places in the world that holds it, in the order of the
    /// player list. A player that leaves has no place, so no command finds it. The entries keep
    /// their names' capacity from one pass to the next.
    fn collect_players(&mut self, host: &World, worlds: &[&mut World]) {
        let targets = &mut self.targets;
        let mut filled = 0;
        for (dimension, world) in worlds.iter().enumerate() {
            let Some(label) = targets.dimensions.get(dimension).map(|entry| entry.label) else {
                continue;
            };
            let Some(query) = self
                .query_of
                .get(dimension)
                .and_then(|&index| self.queries.get_mut(index))
            else {
                continue;
            };
            for (entity, &HostAnchor(anchor), profile, transform, mode) in query.players.iter(world)
            {
                let (Some(session), Some(placement)) = (
                    host.get::<Session>(anchor),
                    host.get::<SessionPlacement>(anchor),
                ) else {
                    continue;
                };
                if placement.place().dim() != Some(label) {
                    continue;
                }
                let entry = PlayerEntry {
                    anchor,
                    dimension,
                    entity,
                    uuid: profile.id,
                    name: String::new(),
                    transform: *transform,
                    mode: mode.0,
                    list_order: session.0.0,
                };
                match targets.players.get_mut(filled) {
                    Some(reused) => {
                        let name = std::mem::take(&mut reused.name);
                        *reused = PlayerEntry { name, ..entry };
                        reused.name.clone_from(&profile.username);
                    }
                    None => targets.players.push(PlayerEntry {
                        name: profile.username.clone(),
                        ..entry
                    }),
                }
                filled += 1;
            }
        }
        targets.players.truncate(filled);
        targets.index_players();
    }
}

fn dimension_entry(world: &World, queries: &mut WorldQueries) -> Option<DimensionEntry> {
    let label = world.get_resource::<DimensionLabel>()?.0;
    let (key, type_id) = queries
        .dimension
        .single(world)
        .ok()
        .map(|(key, type_id)| (key.clone(), type_id.0))?;
    let types = world
        .get_resource::<CurrentRegistries>()?
        .registry::<DimensionTypeFile>()?;
    let id = Id::checked(type_id.number(), types.len())?;
    let coordinate_scale = types.get(id)?.coordinate_scale;
    Some(DimensionEntry {
        label,
        key,
        coordinate_scale,
    })
}
