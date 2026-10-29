use crate::dimension_view::unloaded;
use crate::execute_view::{ExecuteView, Outcome};
use crate::select::FoundPlayers;
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_command::{ArgumentHandle, BuildError, NodeId};
use mcrs_minecraft_command::{
    CommandLevel, DimensionArgument, EntityAnchorArgument, HeightmapArgument, PermissionCheck,
};
use mcrs_minecraft_command_block::BlockPredicateArgument;
use mcrs_minecraft_command_coordinates::{
    BlockPosArgument, RotationArgument, SwizzleArgument, Vec3Argument,
};
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, Feedback, Modified,
    ModifyResult, SourceState,
};
use mcrs_minecraft_command_entity::EntityArgument;
use mcrs_minecraft_entity::PLAYER_EYE_HEIGHT;
use mcrs_minecraft_protocol::text::{IntoText, Text};
use mcrs_minecraft_protocol::{Look, LookAnchor};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Test {
    Block,
    Loaded,
    Dimension,
    Entity,
}

/// Whether a condition passes when its test holds (`if`) or when it fails (`unless`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Polarity {
    If,
    Unless,
}

impl Polarity {
    fn expects(self) -> bool {
        self == Polarity::If
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Branch {
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
    Condition { test: Test, polarity: Polarity },
}

#[derive(Clone, Copy)]
pub(crate) struct ConditionArgs {
    pub(crate) block_pos: ArgumentHandle<BlockPosArgument>,
    pub(crate) block: ArgumentHandle<BlockPredicateArgument>,
    pub(crate) loaded: ArgumentHandle<BlockPosArgument>,
    pub(crate) dimension: ArgumentHandle<DimensionArgument>,
    pub(crate) entities: ArgumentHandle<EntityArgument>,
}

#[derive(Clone, Copy)]
struct PositionedArgs {
    position: ArgumentHandle<Vec3Argument>,
    as_targets: ArgumentHandle<EntityArgument>,
    heightmap: ArgumentHandle<HeightmapArgument>,
}

#[derive(Clone, Copy)]
struct RotatedArgs {
    rotation: ArgumentHandle<RotationArgument>,
    as_targets: ArgumentHandle<EntityArgument>,
}

#[derive(Clone, Copy)]
struct FacingArgs {
    position: ArgumentHandle<Vec3Argument>,
    targets: ArgumentHandle<EntityArgument>,
    anchor: ArgumentHandle<EntityAnchorArgument>,
}

#[derive(Clone, Copy)]
pub(crate) struct ExecuteCommand {
    as_targets: ArgumentHandle<EntityArgument>,
    at_targets: ArgumentHandle<EntityArgument>,
    positioned: PositionedArgs,
    rotated: RotatedArgs,
    facing: FacingArgs,
    align: ArgumentHandle<SwizzleArgument>,
    anchored: ArgumentHandle<EntityAnchorArgument>,
    pub(crate) dimension: ArgumentHandle<DimensionArgument>,
    when: ConditionArgs,
    unless: ConditionArgs,
}

impl ExecuteCommand {
    pub(crate) fn condition(&self, polarity: Polarity) -> &ConditionArgs {
        match polarity {
            Polarity::If => &self.when,
            Polarity::Unless => &self.unless,
        }
    }
}

/// The condition subtree of `if` or `unless` (CMD-EX-002), with the branches of the core set.
fn conditions(
    registrar: &mut CommandRegistrar<'_, Branch>,
    parent: NodeId,
    execute: NodeId,
    polarity: Polarity,
) -> Result<ConditionArgs, BuildError> {
    let branch = |test| Branch::Condition { test, polarity };
    let block = registrar.literal(parent, "block").id()?;
    let block_pos = registrar.arg(block, "pos", BlockPosArgument).id()?;
    let block = registrar
        .arg(block_pos, "block", BlockPredicateArgument)
        .runs(branch(Test::Block))
        .fork(execute, branch(Test::Block))
        .id()?;
    let loaded = registrar.literal(parent, "loaded").id()?;
    let loaded = registrar
        .arg(loaded, "pos", BlockPosArgument)
        .runs(branch(Test::Loaded))
        .fork(execute, branch(Test::Loaded))
        .id()?;
    let dimension = registrar.literal(parent, "dimension").id()?;
    let dimension = registrar
        .arg(dimension, "dimension", DimensionArgument)
        .runs(branch(Test::Dimension))
        .fork(execute, branch(Test::Dimension))
        .id()?;
    let entity = registrar.literal(parent, "entity").id()?;
    let entities = registrar
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

/// The `positioned` subtree: a position, `as` an entity, or `over` a heightmap.
fn positioned(
    registrar: &mut CommandRegistrar<'_, Branch>,
    execute: NodeId,
) -> Result<PositionedArgs, BuildError> {
    let positioned = registrar.literal(execute, "positioned").id()?;
    let position = registrar
        .arg(positioned, "pos", Vec3Argument)
        .modify(execute, Branch::Positioned)
        .id()?;
    let as_node = registrar.literal(positioned, "as").id()?;
    let as_targets = registrar
        .arg(as_node, "targets", EntityArgument::ENTITIES)
        .fork(execute, Branch::PositionedAs)
        .id()?;
    let over = registrar.literal(positioned, "over").id()?;
    let heightmap = registrar
        .arg(over, "heightmap", HeightmapArgument)
        .modify(execute, Branch::PositionedOver)
        .id()?;
    Ok(PositionedArgs {
        position,
        as_targets,
        heightmap,
    })
}

/// The `rotated` subtree: a rotation, or `as` an entity.
fn rotated(
    registrar: &mut CommandRegistrar<'_, Branch>,
    execute: NodeId,
) -> Result<RotatedArgs, BuildError> {
    let rotated = registrar.literal(execute, "rotated").id()?;
    let rotation = registrar
        .arg(rotated, "rot", RotationArgument)
        .modify(execute, Branch::Rotated)
        .id()?;
    let as_node = registrar.literal(rotated, "as").id()?;
    let as_targets = registrar
        .arg(as_node, "targets", EntityArgument::ENTITIES)
        .fork(execute, Branch::RotatedAs)
        .id()?;
    Ok(RotatedArgs {
        rotation,
        as_targets,
    })
}

/// The `facing` subtree: an `entity` and its anchor, or a position.
fn facing(
    registrar: &mut CommandRegistrar<'_, Branch>,
    execute: NodeId,
) -> Result<FacingArgs, BuildError> {
    let facing = registrar.literal(execute, "facing").id()?;
    let entity = registrar.literal(facing, "entity").id()?;
    let targets = registrar
        .arg(entity, "targets", EntityArgument::ENTITIES)
        .id()?;
    let anchor = registrar
        .arg(targets, "anchor", EntityAnchorArgument)
        .fork(execute, Branch::FacingEntity)
        .id()?;
    let position = registrar
        .arg(facing, "pos", Vec3Argument)
        .modify(execute, Branch::Facing)
        .id()?;
    Ok(FacingArgs {
        position,
        targets,
        anchor,
    })
}

/// CMD-EX-001, the branches of the core set (CMD-EX section 8.1).
pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, Branch>,
) -> Result<ExecuteCommand, BuildError> {
    let execute = registrar
        .literal(NodeId::ROOT, "execute")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    registrar
        .literal(execute, "run")
        .redirect(NodeId::ROOT)
        .id()?;
    let when = registrar.literal(execute, "if").id()?;
    let when = conditions(registrar, when, execute, Polarity::If)?;
    let unless = registrar.literal(execute, "unless").id()?;
    let unless = conditions(registrar, unless, execute, Polarity::Unless)?;
    let as_node = registrar.literal(execute, "as").id()?;
    let as_targets = registrar
        .arg(as_node, "targets", EntityArgument::ENTITIES)
        .fork(execute, Branch::As)
        .id()?;
    let at = registrar.literal(execute, "at").id()?;
    let at_targets = registrar
        .arg(at, "targets", EntityArgument::ENTITIES)
        .fork(execute, Branch::At)
        .id()?;
    let positioned = positioned(registrar, execute)?;
    let rotated = rotated(registrar, execute)?;
    let facing = facing(registrar, execute)?;
    let align = registrar.literal(execute, "align").id()?;
    let align = registrar
        .arg(align, "axes", SwizzleArgument)
        .modify(execute, Branch::Align)
        .id()?;
    let anchored = registrar.literal(execute, "anchored").id()?;
    let anchored = registrar
        .arg(anchored, "anchor", EntityAnchorArgument)
        .modify(execute, Branch::Anchored)
        .id()?;
    let in_node = registrar.literal(execute, "in").id()?;
    let dimension = registrar
        .arg(in_node, "dimension", DimensionArgument)
        .modify(execute, Branch::In)
        .id()?;
    Ok(ExecuteCommand {
        as_targets,
        at_targets,
        positioned,
        rotated,
        facing,
        align,
        anchored,
        dimension,
        when,
        unless,
    })
}

fn anchor_point(anchor: LookAnchor, position: DVec3) -> DVec3 {
    mcrs_minecraft_command::anchored(anchor, position, PLAYER_EYE_HEIGHT)
}

pub(crate) fn modify(
    In(call): In<Call<Branch, ExecuteCommand>>,
    view: ExecuteView,
    mut output: CommandOutput,
    mut found: Local<FoundPlayers>,
) -> ModifyResult {
    let command = call.command;
    let context = &view.context;
    let source = context.source(&call)?;
    let state = call.source.clone();
    let one = |state| Ok(Modified::One(state));
    match call.branch {
        Branch::As
        | Branch::At
        | Branch::PositionedAs
        | Branch::RotatedAs
        | Branch::FacingEntity => fork_each(&view, &call, &mut found, &mut output),
        Branch::Positioned => one(SourceState {
            position: context
                .arg(&call, command.positioned.position)?
                .position(&source),
            anchor: LookAnchor::Feet,
            ..state
        }),
        Branch::PositionedOver => {
            let heightmap = context.arg(&call, command.positioned.heightmap)?;
            let [x, _, z] = mcrs_minecraft_command_coordinates::block_floor(state.position);
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
            let Look { yaw, pitch } = context
                .arg(&call, command.rotated.rotation)?
                .resolve(&source);
            one(SourceState {
                yaw,
                pitch,
                ..state
            })
        }
        Branch::Facing => {
            let to = context
                .arg(&call, command.facing.position)?
                .position(&source);
            let from = anchor_point(state.anchor, state.position);
            let Look { yaw, pitch } = mcrs_minecraft_command_coordinates::rotation_toward(from, to);
            one(SourceState {
                yaw,
                pitch,
                ..state
            })
        }
        Branch::Align => {
            let axes = context.arg(&call, command.align)?;
            one(SourceState {
                position: mcrs_minecraft_command_coordinates::align(axes, state.position),
                ..state
            })
        }
        Branch::Anchored => one(SourceState {
            anchor: context.arg(&call, command.anchored)?,
            ..state
        }),
        Branch::In => view.move_to(&call, state),
        Branch::Condition { test, polarity } => {
            let expects = polarity.expects();
            let passes = match view.condition(&call, test, polarity, &mut found)? {
                Outcome::Boolean(value) => value == expects,
                Outcome::Count(count) => (count > 0) == expects,
            };
            Ok(if passes {
                Modified::One(state)
            } else {
                Modified::None
            })
        }
    }
}

/// The branches that give one source for each entity a selector finds (CMD-EX-024).
fn fork_each(
    view: &ExecuteView,
    call: &Call<Branch, ExecuteCommand>,
    found: &mut FoundPlayers,
    output: &mut CommandOutput,
) -> ModifyResult {
    let command = call.command;
    let context = &view.context;
    match call.branch {
        Branch::As => view.fork(call, command.as_targets, found, output, |state, target| {
            SourceState {
                entity: Some(target.anchor),
                ..state.clone()
            }
        }),
        Branch::At => view.fork(call, command.at_targets, found, output, |state, target| {
            SourceState {
                dimension: context
                    .targets()
                    .label_of(target.dimension)
                    .unwrap_or(state.dimension),
                position: target.transform.translation,
                yaw: target.transform.rotation.yaw(),
                pitch: target.transform.rotation.pitch(),
                ..state.clone()
            }
        }),
        Branch::PositionedAs => view.fork(
            call,
            command.positioned.as_targets,
            found,
            output,
            |state, target| SourceState {
                position: target.transform.translation,
                ..state.clone()
            },
        ),
        Branch::RotatedAs => view.fork(
            call,
            command.rotated.as_targets,
            found,
            output,
            |state, target| SourceState {
                yaw: target.transform.rotation.yaw(),
                pitch: target.transform.rotation.pitch(),
                ..state.clone()
            },
        ),
        Branch::FacingEntity => {
            let anchor = context.arg(call, command.facing.anchor)?;
            view.fork(
                call,
                command.facing.targets,
                found,
                output,
                |state, target| {
                    let to = anchor_point(anchor, target.transform.translation);
                    let from = anchor_point(state.anchor, state.position);
                    let Look { yaw, pitch } =
                        mcrs_minecraft_command_coordinates::rotation_toward(from, to);
                    SourceState {
                        yaw,
                        pitch,
                        ..state.clone()
                    }
                },
            )
        }
        _ => Err(CommandFailure::failed()),
    }
}

/// A condition as the run stage (CMD-EX-052).
pub(crate) fn run(
    In(call): In<Call<Branch, ExecuteCommand>>,
    view: ExecuteView,
    mut feedback: CommandOutput,
    mut found: Local<FoundPlayers>,
) -> CommandResult {
    let Branch::Condition { test, polarity } = call.branch else {
        return Err(CommandFailure::failed());
    };
    let outcome = view.condition(&call, test, polarity, &mut found)?;
    let (key, with, result) = match (outcome, polarity) {
        (Outcome::Boolean(value), _) if value == polarity.expects() => {
            ("commands.execute.conditional.pass", vec![], 1)
        }
        (Outcome::Count(count), Polarity::If) if count > 0 => (
            "commands.execute.conditional.pass_count",
            vec![count.to_string().into_text()],
            count,
        ),
        (Outcome::Count(0), Polarity::Unless) => ("commands.execute.conditional.pass", vec![], 1),
        (Outcome::Count(count), Polarity::Unless) => {
            return Err(CommandFailure::translate(
                "commands.execute.conditional.fail_count",
                vec![count.to_string().into_text()],
            ));
        }
        (Outcome::Boolean(_) | Outcome::Count(_), _) => {
            return Err(CommandFailure::translate(
                "commands.execute.conditional.fail",
                vec![],
            ));
        }
    };
    feedback.success(&call, Text::translate(key, with), Feedback::Source);
    Ok(result)
}
