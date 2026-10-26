use crate::command::{
    AppCommands, CommandContext, CommandFailure, CommandRegistrar, CommandResult, Invocation,
};
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_command::{Arg, BuildError};
use mcrs_minecraft_command::{CommandLevel, DimensionArgument, Lookup, PermissionCheck};
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
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
        app.add_command(build, run);
    }
}

#[derive(Resource, Clone, Copy)]
struct DimCommand {
    dimension: Arg<DimensionArgument>,
}

fn build(r: &mut CommandRegistrar<'_>) -> Result<DimCommand, BuildError> {
    let dim = r.literal(
        CommandRegistrar::ROOT,
        "dim",
        PermissionCheck::Require(CommandLevel::Gamemasters),
    )?;
    let dimension = r.argument(
        dim,
        "dimension",
        DimensionArgument,
        PermissionCheck::AlwaysPass,
    )?;
    r.executes(dimension.node())?;
    Ok(DimCommand { dimension })
}

const ARRIVAL: DVec3 = DVec3::new(0.0, 128.0, 0.0);

fn run(
    In(invocation): In<Invocation>,
    command: Res<DimCommand>,
    context: CommandContext,
    players: Query<(&GameProfile, &Owner)>,
    move_sender: Res<FromDimSender<FromDim>>,
    mut move_ids: ResMut<MoveIds>,
    mut commands: Commands,
) -> CommandResult {
    let lookup = context.lookup();
    let source = invocation.source(&lookup);
    let id = invocation
        .get(&context.tree, command.dimension, &source)
        .ok_or_else(|| CommandFailure::translate("command.failed", vec![]))??;
    let invalid = || {
        CommandFailure::translate(
            "argument.dimension.invalid",
            vec![id.to_string().into_text()],
        )
    };
    // Without the registry in this world, the host is the one that knows the dimensions.
    let dimensions = mcrs_minecraft_dimension::keys::DIMENSION.location();
    if lookup.entries(dimensions).next().is_some() && !lookup.has_entry(dimensions, id) {
        return Err(invalid());
    }
    let location = ResourceLocation::new(id.namespace, id.path).map_err(|_| invalid())?;
    let (profile, owner) = players
        .get(invocation.player)
        .map_err(|_| CommandFailure::translate("permissions.requires.player", vec![]))?;
    let move_id = move_ids.allocate();
    // The source keeps the player, hidden, until the target confirms the move or rolls it back.
    commands
        .entity(invocation.player)
        .insert(InTransit { move_id });
    let _ = move_sender.0.try_send(FromDim::MoveEntity {
        move_id,
        target: ResourceKey::<Dimension>::from_location(location),
        cause: ArrivalCause::CommandTeleport { pos: ARRIVAL },
        payload: MovePayload::Player {
            uuid: profile.id,
            username: profile.username.clone(),
        },
        player: Some(owner.0),
    });
    Ok(1)
}
