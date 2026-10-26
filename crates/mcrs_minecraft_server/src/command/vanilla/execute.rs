use crate::command::vanilla::{PlayerCandidate, candidates, select};
use crate::command::{
    Call, CommandContext, CommandFailure, CommandFeedback, CommandRegistrar, CommandResult,
    Modified, ModifyResult, SourceState,
};
use crate::world::entity::player::ability::PlayerGameMode;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use bevy_math::DVec3;
use mcrs_command::{Arg, BuildError, NodeId};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::keys::BLOCK;
use mcrs_minecraft_command::{
    BlockOrTag, BlockPosArgument, BlockPredicateArgument, CommandLevel, DimensionArgument,
    EntityAnchorArgument, EntityArgument, HeightmapArgument, Identifier, Lookup, PermissionCheck,
    RotationArgument, SwizzleArgument, Vec3Argument, anchored, block_floor, rotation_toward,
};
use mcrs_minecraft_core::heightmap::HeightmapName;
use mcrs_minecraft_core::{BlockStateId, ColumnPos, ResourceKey, ResourceLocation, SectionPos};
use mcrs_minecraft_dimension::keys::DIMENSION;
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_entity::PLAYER_EYE_HEIGHT;
use mcrs_minecraft_heightmap::{
    MotionHeightmap, NoLeavesHeightmap, SolidHeightmap, SurfaceHeightmap,
};
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::palette::ChunkBlocks;
use mcrs_minecraft_level::world::dimension::{Dimension, DimensionTypeConfig};
use mcrs_minecraft_level::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_level::world::storage::column::{ColumnIndex, ColumnSections, SectionLookup};
use mcrs_minecraft_protocol::packets::game::clientbound::LookAnchor;
use mcrs_minecraft_protocol::text::{IntoText, Text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Test {
    Block,
    Loaded,
    Dimension,
    Entity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Branch {
    As,
    At,
    Positioned,
    PositionedAs,
    PositionedOver,
    Rotated,
    RotatedAs,
    Facing,
    FacingEntity,
    Align,
    Anchored,
    In,
    /// A condition under `if` (`expect` true) or `unless`.
    Condition {
        test: Test,
        expect: bool,
    },
}

#[derive(Clone, Copy)]
struct ConditionArgs {
    block_pos: Arg<BlockPosArgument>,
    block: Arg<BlockPredicateArgument>,
    loaded: Arg<BlockPosArgument>,
    dimension: Arg<DimensionArgument>,
    entities: Arg<EntityArgument>,
}

#[derive(Clone, Copy)]
pub(super) struct ExecuteCommand {
    as_targets: Arg<EntityArgument>,
    at_targets: Arg<EntityArgument>,
    positioned: Arg<Vec3Argument>,
    positioned_as: Arg<EntityArgument>,
    heightmap: Arg<HeightmapArgument>,
    rotated: Arg<RotationArgument>,
    rotated_as: Arg<EntityArgument>,
    facing: Arg<Vec3Argument>,
    facing_targets: Arg<EntityArgument>,
    facing_anchor: Arg<EntityAnchorArgument>,
    align: Arg<SwizzleArgument>,
    anchored: Arg<EntityAnchorArgument>,
    dimension: Arg<DimensionArgument>,
    when: ConditionArgs,
    unless: ConditionArgs,
}

impl ExecuteCommand {
    fn condition(&self, expect: bool) -> &ConditionArgs {
        if expect { &self.when } else { &self.unless }
    }
}

/// The condition subtree of `if` or `unless` (CMD-EX-002), with the branches of the core set.
fn conditions(
    r: &mut CommandRegistrar<'_, Branch>,
    parent: NodeId,
    execute: NodeId,
    expect: bool,
) -> Result<ConditionArgs, BuildError> {
    let branch = |test| Branch::Condition { test, expect };
    let block = r.literal(parent, "block").id()?;
    let block_pos = r.arg(block, "pos", BlockPosArgument).id()?;
    let block = r
        .arg(block_pos, "block", BlockPredicateArgument)
        .runs(branch(Test::Block))
        .fork(execute, branch(Test::Block))
        .id()?;
    let loaded = r.literal(parent, "loaded").id()?;
    let loaded = r
        .arg(loaded, "pos", BlockPosArgument)
        .runs(branch(Test::Loaded))
        .fork(execute, branch(Test::Loaded))
        .id()?;
    let dimension = r.literal(parent, "dimension").id()?;
    let dimension = r
        .arg(dimension, "dimension", DimensionArgument)
        .runs(branch(Test::Dimension))
        .fork(execute, branch(Test::Dimension))
        .id()?;
    let entity = r.literal(parent, "entity").id()?;
    let entities = r
        .arg(entity, "entities", EntityArgument::ENTITIES)
        .runs(branch(Test::Entity))
        .fork(execute, branch(Test::Entity))
        .id()?;
    Ok(ConditionArgs {
        block_pos,
        block,
        loaded,
        dimension,
        entities,
    })
}

/// CMD-EX-001, the branches of the core set (CMD-EX section 8.1).
pub(super) fn build(r: &mut CommandRegistrar<'_, Branch>) -> Result<ExecuteCommand, BuildError> {
    let execute = r
        .literal(NodeId::ROOT, "execute")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    r.literal(execute, "run").redirect(NodeId::ROOT).id()?;
    let when = r.literal(execute, "if").id()?;
    let when = conditions(r, when, execute, true)?;
    let unless = r.literal(execute, "unless").id()?;
    let unless = conditions(r, unless, execute, false)?;
    let as_ = r.literal(execute, "as").id()?;
    let as_targets = r
        .arg(as_, "targets", EntityArgument::ENTITIES)
        .fork(execute, Branch::As)
        .id()?;
    let at = r.literal(execute, "at").id()?;
    let at_targets = r
        .arg(at, "targets", EntityArgument::ENTITIES)
        .fork(execute, Branch::At)
        .id()?;
    let positioned_node = r.literal(execute, "positioned").id()?;
    let positioned = r
        .arg(positioned_node, "pos", Vec3Argument)
        .modify(execute, Branch::Positioned)
        .id()?;
    let positioned_as_node = r.literal(positioned_node, "as").id()?;
    let positioned_as = r
        .arg(positioned_as_node, "targets", EntityArgument::ENTITIES)
        .fork(execute, Branch::PositionedAs)
        .id()?;
    let over = r.literal(positioned_node, "over").id()?;
    let heightmap = r
        .arg(over, "heightmap", HeightmapArgument)
        .modify(execute, Branch::PositionedOver)
        .id()?;
    let rotated_node = r.literal(execute, "rotated").id()?;
    let rotated = r
        .arg(rotated_node, "rot", RotationArgument)
        .modify(execute, Branch::Rotated)
        .id()?;
    let rotated_as_node = r.literal(rotated_node, "as").id()?;
    let rotated_as = r
        .arg(rotated_as_node, "targets", EntityArgument::ENTITIES)
        .fork(execute, Branch::RotatedAs)
        .id()?;
    let facing_node = r.literal(execute, "facing").id()?;
    let facing_entity = r.literal(facing_node, "entity").id()?;
    let facing_targets = r
        .arg(facing_entity, "targets", EntityArgument::ENTITIES)
        .id()?;
    let facing_anchor = r
        .arg(facing_targets, "anchor", EntityAnchorArgument)
        .fork(execute, Branch::FacingEntity)
        .id()?;
    let facing = r
        .arg(facing_node, "pos", Vec3Argument)
        .modify(execute, Branch::Facing)
        .id()?;
    let align_node = r.literal(execute, "align").id()?;
    let align = r
        .arg(align_node, "axes", SwizzleArgument)
        .modify(execute, Branch::Align)
        .id()?;
    let anchored_node = r.literal(execute, "anchored").id()?;
    let anchored = r
        .arg(anchored_node, "anchor", EntityAnchorArgument)
        .modify(execute, Branch::Anchored)
        .id()?;
    let in_node = r.literal(execute, "in").id()?;
    let dimension = r
        .arg(in_node, "dimension", DimensionArgument)
        .modify(execute, Branch::In)
        .id()?;
    Ok(ExecuteCommand {
        as_targets,
        at_targets,
        positioned,
        positioned_as,
        heightmap,
        rotated,
        rotated_as,
        facing,
        facing_targets,
        facing_anchor,
        align,
        anchored,
        dimension,
        when,
        unless,
    })
}

const MAX_XZ: i32 = 30_000_000;

fn unloaded() -> CommandFailure {
    CommandFailure::translate("argument.pos.unloaded", vec![])
}

/// The blocks, the columns and the heightmaps of the dimension of this world.
#[derive(SystemParam)]
pub(crate) struct DimensionView<'w, 's> {
    dimension: Query<
        'w,
        's,
        (
            &'static ResourceKey<mcrs_minecraft_dimension::Dimension>,
            &'static ColumnIndex,
            &'static DimensionTypeConfig,
        ),
        With<Dimension>,
    >,
    columns: Query<
        'w,
        's,
        (
            &'static ColumnSections,
            Option<&'static SurfaceHeightmap>,
            Option<&'static SolidHeightmap>,
            Option<&'static MotionHeightmap>,
            Option<&'static NoLeavesHeightmap>,
        ),
    >,
    sections: Query<'w, 's, (&'static ChunkBlocks, &'static SectionStage)>,
    blocks: Option<Res<'w, Blocks>>,
}

impl DimensionView<'_, '_> {
    fn is(&self, id: Identifier<'_>) -> bool {
        self.dimension
            .single()
            .is_ok_and(|(key, ..)| key.namespace() == id.namespace && key.path() == id.path)
    }

    fn column(&self, x: i32, z: i32) -> Option<&ColumnSections> {
        let (_, index, _) = self.dimension.single().ok()?;
        let slot = index.get(&ColumnPos::new(
            x >> SectionPos::BITS,
            z >> SectionPos::BITS,
        ))?;
        self.columns
            .get(slot.entity)
            .ok()
            .map(|(sections, ..)| sections)
    }

    /// The column holds every section of its height, each at the loaded stage.
    // chisle: a loaded chunk and a ticking chunk are the same test: the dimension has no
    // chunk levels for entity ticking yet. A ticking stage of the column lifts it.
    fn loaded(&self, x: i32, z: i32) -> bool {
        self.column(x, z).is_some_and(|column| {
            column.iter().all(|section| match section {
                SectionLookup::Loaded(entity) => self
                    .sections
                    .get(entity)
                    .is_ok_and(|(_, stage)| *stage == SectionStage::Loaded),
                SectionLookup::Unloaded | SectionLookup::OutOfRange => false,
            })
        })
    }

    /// CMD-EX-050: the position must be in a loaded chunk and inside the world.
    fn readable(&self, [x, y, z]: [i32; 3]) -> Result<(), CommandFailure> {
        if !self.loaded(x, z) {
            return Err(unloaded());
        }
        let (_, _, config) = self.dimension.single().map_err(|_| unloaded())?;
        let height = i32::try_from(config.height).unwrap_or(i32::MAX);
        let inside = (config.min_y..config.min_y.saturating_add(height)).contains(&y)
            && (-MAX_XZ..MAX_XZ).contains(&x)
            && (-MAX_XZ..MAX_XZ).contains(&z);
        if !inside {
            return Err(CommandFailure::translate("argument.pos.outofworld", vec![]));
        }
        Ok(())
    }

    fn block_state(&self, [x, y, z]: [i32; 3]) -> Option<BlockStateId> {
        let section = match self.column(x, z)?.lookup(y >> SectionPos::BITS) {
            SectionLookup::Loaded(section) => section,
            SectionLookup::Unloaded | SectionLookup::OutOfRange => return None,
        };
        let (blocks, _) = self.sections.get(section).ok()?;
        let local = |axis: i32| usize::try_from(axis & SectionPos::MASK as i32).ok();
        blocks.get_cell(local(x)?, local(y)?, local(z)?)
    }

    /// HM-006 for a column of a loaded chunk.
    fn height(&self, heightmap: HeightmapName, x: i32, z: i32) -> Option<i32> {
        let (_, index, _) = self.dimension.single().ok()?;
        let slot = index.get(&ColumnPos::new(
            x >> SectionPos::BITS,
            z >> SectionPos::BITS,
        ))?;
        let (_, surface, solid, motion, no_leaves) = self.columns.get(slot.entity).ok()?;
        let heights = match heightmap {
            HeightmapName::WorldSurface | HeightmapName::WorldSurfaceWg => &surface?.0,
            HeightmapName::OceanFloor | HeightmapName::OceanFloorWg => &solid?.0,
            HeightmapName::MotionBlocking => &motion?.0,
            HeightmapName::MotionBlockingNoLeaves => &no_leaves?.0,
        };
        let local = |axis: i32| usize::try_from(axis & SectionPos::MASK as i32).ok();
        Some(heights.get(local(x)?, local(z)?))
    }
}

/// Everything a stage of `execute` reads.
#[derive(SystemParam)]
pub(super) struct ExecuteView<'w, 's> {
    context: CommandContext<'w, 's>,
    players: Query<
        'w,
        's,
        (
            Entity,
            &'static GameProfile,
            &'static Transform,
            &'static PlayerGameMode,
        ),
        With<Player>,
    >,
    dimension: DimensionView<'w, 's>,
}

/// The result of a condition: a boolean, or a count (CMD-EX section 0.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Boolean(bool),
    Count(i32),
}

impl ExecuteView<'_, '_> {
    fn candidates(&self) -> Vec<PlayerCandidate<'_>> {
        candidates(self.players.iter())
    }

    /// One source for each entity of `targets`, from `each`.
    fn fork(
        &self,
        call: &Call<Branch, ExecuteCommand>,
        targets: Arg<EntityArgument>,
        each: impl Fn(&SourceState, &PlayerCandidate) -> SourceState,
    ) -> ModifyResult {
        let all = self.candidates();
        let found = select(&self.context, call, targets, &all)?;
        Ok(Modified::Many(
            found
                .into_iter()
                .map(|target| each(&call.source, target))
                .collect(),
        ))
    }

    fn condition(
        &self,
        call: &Call<Branch, ExecuteCommand>,
        test: Test,
        expect: bool,
    ) -> Result<Outcome, CommandFailure> {
        let args = call.command.condition(expect);
        let context = &self.context;
        let source = context.source(call);
        match test {
            Test::Entity => {
                let all = self.candidates();
                let found = select(context, call, args.entities, &all)?;
                Ok(Outcome::Count(
                    i32::try_from(found.len()).unwrap_or(i32::MAX),
                ))
            }
            Test::Dimension => {
                let id = dimension(context, call, args.dimension)?;
                Ok(Outcome::Boolean(self.dimension.is(id)))
            }
            Test::Loaded => {
                let [x, _, z] = block_floor(context.arg(call, args.loaded)?.position(&source));
                Ok(Outcome::Boolean(self.dimension.loaded(x, z)))
            }
            Test::Block => {
                let pos = block_floor(context.arg(call, args.block_pos)?.position(&source));
                self.dimension.readable(pos)?;
                let predicate = context.arg(call, args.block)?;
                let state = self.dimension.block_state(pos).ok_or_else(unloaded)?;
                let entry = self
                    .dimension
                    .blocks
                    .as_deref()
                    .and_then(|blocks| blocks.0.owner(state));
                let Some(entry) = entry else {
                    return Ok(Outcome::Boolean(false));
                };
                let name = entry.identifier.as_str();
                let in_subject = match predicate.subject {
                    BlockOrTag::Block(id) => id.eq_str(name),
                    BlockOrTag::Tag(tag) => context
                        .tag_entries(BLOCK.location(), tag)
                        .any(|member| member == name),
                };
                Ok(Outcome::Boolean(predicate.test(in_subject, entry, state)))
            }
        }
    }
}

impl ExecuteView<'_, '_> {
    /// CMD-EX-021. The source leaves for another dimension after its stage, where its x and z
    /// are scaled.
    fn move_to(&self, call: &Call<Branch, ExecuteCommand>, state: SourceState) -> ModifyResult {
        let id = dimension(&self.context, call, call.command.dimension)?;
        if self.dimension.is(id) {
            return Ok(Modified::One(state));
        }
        let location =
            ResourceLocation::new(id.namespace, id.path).map_err(|_| invalid_dimension(id))?;
        Ok(Modified::One(SourceState {
            destination: Some(ResourceKey::from_location(location)),
            ..state
        }))
    }
}

fn invalid_dimension(id: Identifier<'_>) -> CommandFailure {
    CommandFailure::translate(
        "argument.dimension.invalid",
        vec![id.to_string().into_text()],
    )
}

/// The dimension an argument names. A world without the dimension registry leaves the test of
/// an unknown name to the host, which knows the dimensions that run.
fn dimension<'a>(
    context: &'a CommandContext,
    call: &Call<Branch, ExecuteCommand>,
    arg: Arg<DimensionArgument>,
) -> Result<Identifier<'a>, CommandFailure> {
    let id = context.arg(call, arg)?;
    let dimensions = DIMENSION.location();
    if context.entries(dimensions).next().is_some() && !context.has_entry(dimensions, id) {
        return Err(invalid_dimension(id));
    }
    Ok(id)
}

fn anchor_point(anchor: LookAnchor, position: DVec3) -> DVec3 {
    anchored(anchor, position, PLAYER_EYE_HEIGHT)
}

pub(super) fn modify(
    In(call): In<Call<Branch, ExecuteCommand>>,
    view: ExecuteView,
) -> ModifyResult {
    let command = call.command;
    let context = &view.context;
    let source = context.source(&call);
    let state = call.source.clone();
    let one = |state| Ok(Modified::One(state));
    match call.branch {
        Branch::As => view.fork(&call, command.as_targets, |state, target| SourceState {
            entity: Some(target.entity),
            ..state.clone()
        }),
        Branch::At => view.fork(&call, command.at_targets, |state, target| SourceState {
            position: target.transform.translation,
            yaw: target.transform.rotation.yaw(),
            pitch: target.transform.rotation.pitch(),
            ..state.clone()
        }),
        Branch::PositionedAs => {
            view.fork(&call, command.positioned_as, |state, target| SourceState {
                position: target.transform.translation,
                ..state.clone()
            })
        }
        Branch::RotatedAs => view.fork(&call, command.rotated_as, |state, target| SourceState {
            yaw: target.transform.rotation.yaw(),
            pitch: target.transform.rotation.pitch(),
            ..state.clone()
        }),
        Branch::FacingEntity => {
            let anchor = context.arg(&call, command.facing_anchor)?;
            view.fork(&call, command.facing_targets, |state, target| {
                let to = anchor_point(anchor, target.transform.translation);
                let (yaw, pitch) = rotation_toward(anchor_point(state.anchor, state.position), to);
                SourceState {
                    yaw,
                    pitch,
                    ..state.clone()
                }
            })
        }
        Branch::Positioned => one(SourceState {
            position: context.arg(&call, command.positioned)?.position(&source),
            anchor: LookAnchor::Feet,
            ..state
        }),
        Branch::PositionedOver => {
            let heightmap = context.arg(&call, command.heightmap)?;
            let [x, _, z] = block_floor(state.position);
            if !view.dimension.loaded(x, z) {
                return Err(unloaded());
            }
            let height = view
                .dimension
                .height(heightmap, x, z)
                .ok_or_else(unloaded)?;
            one(SourceState {
                position: DVec3::new(state.position.x, f64::from(height), state.position.z),
                ..state
            })
        }
        Branch::Rotated => {
            let (yaw, pitch) = context.arg(&call, command.rotated)?.resolve(&source);
            one(SourceState {
                yaw,
                pitch,
                ..state
            })
        }
        Branch::Facing => {
            let to = context.arg(&call, command.facing)?.position(&source);
            let (yaw, pitch) = rotation_toward(anchor_point(state.anchor, state.position), to);
            one(SourceState {
                yaw,
                pitch,
                ..state
            })
        }
        Branch::Align => one(SourceState {
            position: context.arg(&call, command.align)?.align(state.position),
            ..state
        }),
        Branch::Anchored => one(SourceState {
            anchor: context.arg(&call, command.anchored)?,
            ..state
        }),
        Branch::In => view.move_to(&call, state),
        Branch::Condition { test, expect } => {
            let passes = match view.condition(&call, test, expect)? {
                Outcome::Boolean(value) => value == expect,
                Outcome::Count(count) => (count > 0) == expect,
            };
            Ok(if passes {
                Modified::One(state)
            } else {
                Modified::None
            })
        }
    }
}

fn pass(
    feedback: &mut CommandFeedback,
    call: &Call<Branch, ExecuteCommand>,
    key: &'static str,
    with: Vec<Text>,
) {
    feedback.success(call, Text::translate(key, with), false);
}

/// A condition as the run stage (CMD-EX-052).
pub(super) fn run(
    In(call): In<Call<Branch, ExecuteCommand>>,
    view: ExecuteView,
    mut feedback: CommandFeedback,
) -> CommandResult {
    let Branch::Condition { test, expect } = call.branch else {
        return Err(CommandFailure::failed());
    };
    let fail = || CommandFailure::translate("commands.execute.conditional.fail", vec![]);
    match (view.condition(&call, test, expect)?, expect) {
        (Outcome::Boolean(value), _) if value == expect => {
            pass(
                &mut feedback,
                &call,
                "commands.execute.conditional.pass",
                vec![],
            );
            Ok(1)
        }
        (Outcome::Count(count), true) if count > 0 => {
            pass(
                &mut feedback,
                &call,
                "commands.execute.conditional.pass_count",
                vec![count.to_string().into_text()],
            );
            Ok(count)
        }
        (Outcome::Count(0), false) => {
            pass(
                &mut feedback,
                &call,
                "commands.execute.conditional.pass",
                vec![],
            );
            Ok(1)
        }
        (Outcome::Count(count), false) => Err(CommandFailure::translate(
            "commands.execute.conditional.fail_count",
            vec![count.to_string().into_text()],
        )),
        (Outcome::Boolean(_) | Outcome::Count(_), _) => Err(fail()),
    }
}
