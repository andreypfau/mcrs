use crate::set::VanillaCommands;
use bevy_app::SubApp;
use bevy_ecs::prelude::*;
use mcrs_command::{BuildError, NodeId};
use mcrs_minecraft_command_engine::{
    Call, CommandOutput, CommandRegistrar, CommandResult, CommandTargets, Feedback, PlayerEntry,
};
use mcrs_minecraft_engine_server::login::JoinLimits;
use mcrs_minecraft_engine_world::world::dimension_builders::DimensionBuild;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_text::{Color, IntoText, Text, TranslateArg};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Branch {
    Names,
    Uuids,
}

pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, Branch>,
) -> Result<(), BuildError> {
    let list = registrar
        .literal(NodeId::ROOT, "list")
        .runs(Branch::Names)
        .id()?;
    registrar.literal(list, "uuids").runs(Branch::Uuids).id()?;
    Ok(())
}

pub(crate) fn copy_join_limits(host: &World, _: &DimensionBuild<'_>, sub_app: &mut SubApp) {
    if let Some(limits) = host.get_resource::<JoinLimits>() {
        sub_app.insert_resource(*limits);
    }
}

fn name(player: &PlayerEntry) -> Text {
    player
        .name
        .clone()
        .into_text()
        .on_click_suggest_command(format!("/tell {} ", player.name))
        .insertion(player.name.clone())
        .on_hover_show_entity(
            ResourceLocationBuf::from(EntityType::Player.location()),
            player.uuid,
            Some(player.name.clone().into_text()),
        )
}

fn name_and_uuid(player: &PlayerEntry) -> Text {
    Text::translate(
        "commands.list.nameAndId",
        vec![
            player.name.clone().into_text(),
            player.uuid.hyphenated().to_string().into_text(),
        ],
    )
}

fn name_list(players: &[PlayerEntry], branch: Branch) -> Text {
    let entry = match branch {
        Branch::Names => name,
        Branch::Uuids => name_and_uuid,
    };
    if let [player] = players {
        return entry(player);
    }
    let mut text = Text::text("");
    for (index, player) in players.iter().enumerate() {
        if index > 0 {
            text = text.add_child(", ".color(Color::GRAY));
        }
        text = text.add_child(entry(player));
    }
    text
}

pub(crate) fn run(
    In(call): In<Call<Branch, ()>>,
    targets: Res<CommandTargets>,
    limits: Option<Res<JoinLimits>>,
    mut output: CommandOutput,
) -> CommandResult {
    let players = targets.players();
    let count = i32::try_from(players.len()).unwrap_or(i32::MAX);
    let limit = limits
        .map_or(JoinLimits::default(), |limits| *limits)
        .max_players;
    let limit = i32::try_from(limit).unwrap_or(i32::MAX);
    output.success(
        &call.invocation,
        Text::translate_args(
            "commands.list.players",
            vec![
                TranslateArg::from(count),
                TranslateArg::from(limit),
                TranslateArg::Text(name_list(players, call.branch)),
            ],
        ),
        Feedback::Source,
    );
    Ok(count)
}
