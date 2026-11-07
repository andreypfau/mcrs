use crate::set::{VanillaCommands, VanillaContext};
use bevy_ecs::prelude::*;
use mcrs_command::{ArgumentHandle, BuildError, NodeId, StringArgument};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument::PlayerNameArgument;
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, CommandTargets,
    EffectQueue, Feedback, Worlds,
};
use mcrs_minecraft_engine::banlist::{BanEntry, BanList, EditBans};
use mcrs_minecraft_engine::dim_mail;
use mcrs_minecraft_network::client::offline_player_uuid;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_text::{IntoText, Text};

const DEFAULT_REASON: &str = "multiplayer.disconnect.banned.reason.default";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Branch {
    Ban,
    BanWithReason,
    Pardon,
    List,
}

#[derive(Clone, Copy)]
pub(crate) struct BanCommand {
    ban: ArgumentHandle<PlayerNameArgument>,
    reason: ArgumentHandle<StringArgument>,
    pardon: ArgumentHandle<PlayerNameArgument>,
}

pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, Branch>,
) -> Result<BanCommand, BuildError> {
    let ban = registrar
        .literal(NodeId::ROOT, "ban")
        .requires(PermissionCheck::Require(CommandLevel::Admins))
        .id()?;
    let ban = registrar
        .arg(ban, "targets", PlayerNameArgument::Online)
        .runs(Branch::Ban)
        .id()?;
    let reason = registrar
        .arg(ban, "reason", StringArgument::Greedy)
        .runs(Branch::BanWithReason)
        .id()?;
    let pardon = registrar
        .literal(NodeId::ROOT, "pardon")
        .requires(PermissionCheck::Require(CommandLevel::Admins))
        .id()?;
    let pardon = registrar
        .arg(pardon, "targets", PlayerNameArgument::Banned)
        .runs(Branch::Pardon)
        .id()?;
    let banlist = registrar
        .literal(NodeId::ROOT, "banlist")
        .requires(PermissionCheck::Require(CommandLevel::Admins))
        .runs(Branch::List)
        .id()?;
    registrar
        .literal(banlist, "players")
        .runs(Branch::List)
        .id()?;
    Ok(BanCommand {
        ban,
        reason,
        pardon,
    })
}

pub(crate) fn apply(worlds: &mut Worlds, edit: EditBans) {
    dim_mail::edit_bans(worlds.world_mut(), edit);
}

fn online_or_offline(targets: &CommandTargets, name: &str) -> (Uuid, String) {
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
        Some(player) => (player.uuid, player.name.clone()),
        None => (offline_player_uuid(name), name.to_owned()),
    }
}

fn reason_text(reason: Option<&str>) -> Text {
    match reason {
        Some(reason) => reason.to_owned().into_text(),
        None => Text::translate(DEFAULT_REASON, vec![]),
    }
}

pub(crate) fn run(
    In(call): In<Call<Branch, BanCommand>>,
    context: VanillaContext,
    bans: Res<BanList>,
    targets: Res<CommandTargets>,
    mut output: CommandOutput,
    mut edits: ResMut<EffectQueue<EditBans>>,
) -> CommandResult {
    match call.branch {
        Branch::List => list(&call, &bans, &mut output),
        Branch::Pardon => {
            let name = context.arg(&call.invocation, call.command.pardon)?;
            let name = name.as_unescaped().ok_or_else(CommandFailure::failed)?;
            let id = match bans.find_by_name(name) {
                Some(entry) => entry.uuid,
                None => online_or_offline(&targets, name).0,
            };
            let Some(entry) = bans.get(&id) else {
                return Err(CommandFailure::translate("commands.pardon.failed", vec![]));
            };
            output.success(
                &call.invocation,
                Text::translate(
                    "commands.pardon.success",
                    vec![entry.name.clone().into_text()],
                ),
                Feedback::SourceAndOperators,
            );
            output.effect(&mut edits, EditBans::Pardon(id));
            Ok(1)
        }
        Branch::Ban | Branch::BanWithReason => {
            let name = context.arg(&call.invocation, call.command.ban)?;
            let name = name.as_unescaped().ok_or_else(CommandFailure::failed)?;
            let reason = if call.branch == Branch::BanWithReason {
                let reason = context.arg(&call.invocation, call.command.reason)?;
                Some(
                    reason
                        .as_unescaped()
                        .ok_or_else(CommandFailure::failed)?
                        .to_owned(),
                )
            } else {
                None
            };
            let (id, name) = online_or_offline(&targets, name);
            if bans.active(&id).is_some() {
                return Err(CommandFailure::translate("commands.ban.failed", vec![]));
            }
            let source = call
                .invocation
                .source
                .player
                .and_then(|session| targets.player(session))
                .map_or_else(|| "Server".to_owned(), |player| player.name.clone());
            output.success(
                &call.invocation,
                Text::translate(
                    "commands.ban.success",
                    vec![name.clone().into_text(), reason_text(reason.as_deref())],
                ),
                Feedback::SourceAndOperators,
            );
            output.effect(
                &mut edits,
                EditBans::Add(BanEntry::new(id, name, source, reason)),
            );
            Ok(1)
        }
    }
}

fn list(
    call: &Call<Branch, BanCommand>,
    bans: &BanList,
    output: &mut CommandOutput,
) -> CommandResult {
    let count = bans.entries().count() as i32;
    if count == 0 {
        output.success(
            &call.invocation,
            Text::translate("commands.banlist.none", vec![]),
            Feedback::Source,
        );
        return Ok(0);
    }
    output.success(
        &call.invocation,
        Text::translate_args("commands.banlist.list", vec![count.into()]),
        Feedback::Source,
    );
    for entry in bans.entries() {
        output.success(
            &call.invocation,
            Text::translate(
                "commands.banlist.entry",
                vec![
                    entry.name.clone().into_text(),
                    entry.source.clone().into_text(),
                    reason_text(entry.reason.as_deref()),
                ],
            ),
            Feedback::Source,
        );
    }
    Ok(count)
}
