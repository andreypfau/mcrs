use crate::set::{VanillaCommands, VanillaContext};
use bevy_ecs::prelude::*;
use mcrs_command::{ArgumentHandle, BuildError, NodeId};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument::PlayerNameArgument;
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, CommandTargets,
    EffectQueue, Feedback, Worlds,
};
use mcrs_minecraft_engine::coordinator::ops::{EditOps, OpList, OpListEntry};
use mcrs_minecraft_engine::dim_mail;
use mcrs_minecraft_network::client::offline_player_uuid;
use mcrs_minecraft_text::{IntoText, Text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Branch {
    Op,
    Deop,
}

#[derive(Clone, Copy)]
pub(crate) struct OpCommand {
    op: ArgumentHandle<PlayerNameArgument>,
    deop: ArgumentHandle<PlayerNameArgument>,
}

pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, Branch>,
) -> Result<OpCommand, BuildError> {
    let op = registrar
        .literal(NodeId::ROOT, "op")
        .requires(PermissionCheck::Require(CommandLevel::Admins))
        .id()?;
    let op = registrar
        .arg(op, "targets", PlayerNameArgument::Online)
        .runs(Branch::Op)
        .id()?;
    let deop = registrar
        .literal(NodeId::ROOT, "deop")
        .requires(PermissionCheck::Require(CommandLevel::Admins))
        .id()?;
    let deop = registrar
        .arg(deop, "targets", PlayerNameArgument::Online)
        .runs(Branch::Deop)
        .id()?;
    Ok(OpCommand { op, deop })
}

pub(crate) fn apply(worlds: &mut Worlds, edit: EditOps) {
    dim_mail::edit_ops(worlds.world_mut(), edit);
}

/// The profile of the online player with `name`, else of a listed operator with it, else the
/// offline profile of the name.
fn profile(targets: &CommandTargets, ops: &OpList, name: &str) -> OpListEntry {
    let players = targets.players();
    let online = players
        .iter()
        .find(|player| player.name == name)
        .or_else(|| {
            players
                .iter()
                .find(|player| player.name.eq_ignore_ascii_case(name))
        });
    if let Some(player) = online {
        return OpListEntry::owner(player.uuid, player.name.clone());
    }
    match ops.find_by_name(name) {
        Some(entry) => entry.clone(),
        None => OpListEntry::owner(offline_player_uuid(name), name.to_owned()),
    }
}

pub(crate) fn run(
    In(call): In<Call<Branch, OpCommand>>,
    context: VanillaContext,
    ops: Option<Res<OpList>>,
    targets: Res<CommandTargets>,
    mut output: CommandOutput,
    mut edits: ResMut<EffectQueue<EditOps>>,
) -> CommandResult {
    let none = OpList::default();
    let ops = ops.as_deref().unwrap_or(&none);
    let handle = match call.branch {
        Branch::Op => call.command.op,
        Branch::Deop => call.command.deop,
    };
    let name = context.arg(&call.invocation, handle)?;
    let name = name.as_unescaped().ok_or_else(CommandFailure::failed)?;
    let entry = profile(&targets, ops, name);
    let listed = ops.get(&entry.uuid).is_some();
    let (success, edit) = match call.branch {
        Branch::Op if listed => {
            return Err(CommandFailure::translate("commands.op.failed", vec![]));
        }
        Branch::Op => ("commands.op.success", EditOps::Add(entry.clone())),
        Branch::Deop if !listed => {
            return Err(CommandFailure::translate("commands.deop.failed", vec![]));
        }
        Branch::Deop => ("commands.deop.success", EditOps::Remove(entry.uuid)),
    };
    output.success(
        &call.invocation,
        Text::translate(success, vec![entry.name.into_text()]),
        Feedback::SourceAndOperators,
    );
    output.effect(&mut edits, edit);
    Ok(1)
}
