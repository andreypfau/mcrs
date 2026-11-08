use crate::{VanillaCommands, VanillaContext};
use bevy_ecs::prelude::*;
use mcrs_command::{ArgumentHandle, BuildError, NodeId, StringArgument};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, CommandTargets,
    EffectQueue, Feedback, Worlds,
};
use mcrs_minecraft_engine::dim_mail;
use mcrs_minecraft_engine::whitelist::{EditWhitelist, Whitelist, WhitelistEntry};
use mcrs_minecraft_text::{IntoText, Text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Branch {
    List,
    Add,
    Remove,
    On,
    Off,
    Reload,
}

#[derive(Clone, Copy)]
pub(crate) struct WhitelistCommand {
    add: ArgumentHandle<StringArgument>,
    remove: ArgumentHandle<StringArgument>,
}

pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, Branch>,
) -> Result<WhitelistCommand, BuildError> {
    let whitelist = registrar
        .literal(NodeId::ROOT, "whitelist")
        .requires(PermissionCheck::Require(CommandLevel::Admins))
        .id()?;
    registrar
        .literal(whitelist, "list")
        .runs(Branch::List)
        .id()?;
    registrar.literal(whitelist, "on").runs(Branch::On).id()?;
    registrar.literal(whitelist, "off").runs(Branch::Off).id()?;
    registrar
        .literal(whitelist, "reload")
        .runs(Branch::Reload)
        .id()?;
    let add = registrar.literal(whitelist, "add").id()?;
    let add = registrar
        .arg(add, "targets", StringArgument::Word)
        .runs(Branch::Add)
        .id()?;
    let remove = registrar.literal(whitelist, "remove").id()?;
    let remove = registrar
        .arg(remove, "targets", StringArgument::Word)
        .runs(Branch::Remove)
        .id()?;
    Ok(WhitelistCommand { add, remove })
}

pub(crate) fn apply(worlds: &mut Worlds, edit: EditWhitelist) {
    dim_mail::edit_whitelist(worlds.world_mut(), edit);
}

fn profile(targets: &CommandTargets, name: &str) -> WhitelistEntry {
    let players = targets.players();
    let online = players
        .iter()
        .find(|player| player.name == name)
        .or_else(|| {
            players
                .iter()
                .find(|player| player.name.eq_ignore_ascii_case(name))
        });
    match online {
        Some(player) => WhitelistEntry {
            uuid: player.uuid,
            name: player.name.clone(),
        },
        None => WhitelistEntry::offline(name),
    }
}

pub(crate) fn run(
    In(call): In<Call<Branch, WhitelistCommand>>,
    context: VanillaContext,
    whitelist: Res<Whitelist>,
    targets: Res<CommandTargets>,
    mut output: CommandOutput,
    mut edits: ResMut<EffectQueue<EditWhitelist>>,
) -> CommandResult {
    let handle = match call.branch {
        Branch::List => return list(&call, &whitelist, &mut output),
        Branch::On | Branch::Off | Branch::Reload => {
            return switch(&call, &whitelist, &mut output, &mut edits);
        }
        Branch::Add => call.command.add,
        Branch::Remove => call.command.remove,
    };
    let name = context.arg(&call.invocation, handle)?;
    let name = name.as_unescaped().ok_or_else(CommandFailure::failed)?;
    let entry = profile(&targets, name);
    let listed = whitelist.contains(&entry.uuid);
    let (success, edit) = match call.branch {
        Branch::Add if listed => {
            return Err(CommandFailure::translate(
                "commands.whitelist.add.failed",
                vec![],
            ));
        }
        Branch::Add => (
            "commands.whitelist.add.success",
            EditWhitelist::Add(entry.clone()),
        ),
        _ if !listed => {
            return Err(CommandFailure::translate(
                "commands.whitelist.remove.failed",
                vec![],
            ));
        }
        _ => (
            "commands.whitelist.remove.success",
            EditWhitelist::Remove(entry.uuid),
        ),
    };
    output.success(
        &call.invocation,
        Text::translate(success, vec![entry.name.into_text()]),
        Feedback::SourceAndOperators,
    );
    output.effect(&mut edits, edit);
    Ok(1)
}

fn switch(
    call: &Call<Branch, WhitelistCommand>,
    whitelist: &Whitelist,
    output: &mut CommandOutput,
    edits: &mut EffectQueue<EditWhitelist>,
) -> CommandResult {
    let (success, edit) = match call.branch {
        Branch::On if whitelist.is_enabled() => {
            return Err(CommandFailure::translate(
                "commands.whitelist.alreadyOn",
                vec![],
            ));
        }
        Branch::On => (
            "commands.whitelist.enabled",
            EditWhitelist::SetEnabled(true),
        ),
        Branch::Off if !whitelist.is_enabled() => {
            return Err(CommandFailure::translate(
                "commands.whitelist.alreadyOff",
                vec![],
            ));
        }
        Branch::Off => (
            "commands.whitelist.disabled",
            EditWhitelist::SetEnabled(false),
        ),
        _ => ("commands.whitelist.reloaded", EditWhitelist::Reload),
    };
    output.success(
        &call.invocation,
        Text::translate(success, vec![]),
        Feedback::SourceAndOperators,
    );
    output.effect(edits, edit);
    Ok(1)
}

fn list(
    call: &Call<Branch, WhitelistCommand>,
    whitelist: &Whitelist,
    output: &mut CommandOutput,
) -> CommandResult {
    let names = whitelist.names();
    if names.is_empty() {
        output.success(
            &call.invocation,
            Text::translate("commands.whitelist.none", vec![]),
            Feedback::Source,
        );
        return Ok(0);
    }

    let count = names.len() as i32;
    output.success(
        &call.invocation,
        Text::translate_args(
            "commands.whitelist.list",
            vec![count.into(), names.join(", ").into()],
        ),
        Feedback::Source,
    );
    Ok(count)
}
