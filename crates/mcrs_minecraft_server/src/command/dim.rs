use crate::command::{
    AppCommands, Call, CommandContext, CommandFailure, CommandRegistrar, CommandResult,
};
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_command::{Arg, BuildError, NodeId};
use mcrs_minecraft_command::{CommandLevel, DimensionArgument, Lookup, PermissionCheck};
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_engine::bus::{ArrivalCause, MovePayload};
use mcrs_minecraft_engine::channel_types::FromDim;
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_level::entity::InTransit;
use mcrs_minecraft_level::session::Owner;
use mcrs_minecraft_level::world::channels::FromDimSender;
use mcrs_minecraft_level::world::in_flight::MoveIds;
use mcrs_minecraft_protocol::text::IntoText;

/// Commands of this server that vanilla does not have: `dim <dimension>` moves the source to
/// the top of another dimension.
pub struct DebugCommandsPlugin;

impl Plugin for DebugCommandsPlugin {
    fn build(&self, app: &mut App) {
        if let Err(error) = app.add_command(build, run) {
            tracing::error!("dim: {error}");
        }
    }
}

#[derive(Clone, Copy)]
struct DimCommand {
    dimension: Arg<DimensionArgument>,
}

fn build(r: &mut CommandRegistrar<'_, ()>) -> Result<DimCommand, BuildError> {
    let dim = r
        .literal(NodeId::ROOT, "dim")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let dimension = r.arg(dim, "dimension", DimensionArgument).runs(()).id()?;
    Ok(DimCommand { dimension })
}

const ARRIVAL: DVec3 = DVec3::new(0.0, 128.0, 0.0);

fn run(
    In(call): In<Call<(), DimCommand>>,
    context: CommandContext,
    players: Query<(&GameProfile, &Owner)>,
    move_sender: Res<FromDimSender<FromDim>>,
    mut move_ids: ResMut<MoveIds>,
    mut commands: Commands,
) -> CommandResult {
    let command = call.command;
    let id = context.arg(&call, command.dimension)?;
    let invalid = || {
        CommandFailure::translate(
            "argument.dimension.invalid",
            vec![id.to_string().into_text()],
        )
    };
    // Without the registry in this world, the host is the one that knows the dimensions.
    let dimensions = mcrs_minecraft_dimension::keys::DIMENSION.location();
    if context.entries(dimensions).next().is_some() && !context.has_entry(dimensions, id) {
        return Err(invalid());
    }
    let location = id.to_location().ok_or_else(invalid)?;
    let (profile, owner) = players
        .get(call.source.entity)
        .map_err(|_| CommandFailure::translate("permissions.requires.player", vec![]))?;
    let move_id = move_ids.allocate();
    move_sender
        .0
        .try_send(FromDim::MoveEntity {
            move_id,
            target: ResourceKey::<Dimension>::from_location(location),
            cause: ArrivalCause::CommandTeleport { pos: ARRIVAL },
            payload: MovePayload::Player {
                uuid: profile.id,
                username: profile.username.clone(),
            },
            player: Some(owner.0),
        })
        .map_err(|_| CommandFailure::failed())?;
    // The source keeps the player, hidden, until the target confirms the move or rolls it back.
    commands
        .entity(call.source.entity)
        .insert(InTransit { move_id });
    Ok(1)
}
