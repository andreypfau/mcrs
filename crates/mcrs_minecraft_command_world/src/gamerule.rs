use bevy_ecs::prelude::*;
use mcrs_command::{ArgumentHandle, BoolArgument, BuildError, IntegerArgument, NodeId};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, EffectQueue, Feedback,
};
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaContext};
use mcrs_minecraft_engine::server_edit::ServerEdit;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_game_rule::{GameRule, GameRuleValueType};
use mcrs_minecraft_text::{IntoText, Text};

use crate::server_change::ServerChange;

const RULES: usize = GameRule::ALL.len();

/// What a branch of `gamerule` does to its rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Branch {
    Query(GameRule),
    Set(GameRule),
}

/// The value node of one rule under one of its two names.
#[derive(Clone, Copy, Debug)]
enum ValueNode {
    Bool(ArgumentHandle<BoolArgument>),
    Int(ArgumentHandle<IntegerArgument>),
    /// The rule needs a feature that the server does not enable, so it has no node.
    Absent,
}

#[derive(Clone, Copy)]
pub(crate) struct GameRuleCommand {
    /// The value nodes of each rule, by its id: under the short name, then under the identifier.
    values: [[ValueNode; 2]; RULES],
}

/// The name of a rule without its namespace.
fn short_name(rule: GameRule) -> &'static str {
    let name = rule.as_static_str();
    name.split_once(':').map_or(name, |(_, path)| path)
}

pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, Branch>,
) -> Result<GameRuleCommand, BuildError> {
    let gamerule = registrar
        .literal(NodeId::ROOT, "gamerule")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let mut values = [[ValueNode::Absent; 2]; RULES];
    for (&rule, slot) in GameRule::ALL.iter().zip(values.iter_mut()) {
        let definition = rule.definition();
        if !definition.required_features.is_empty() {
            continue;
        }
        for (name, value) in [short_name(rule), rule.as_static_str()]
            .into_iter()
            .zip(slot.iter_mut())
        {
            let named = registrar
                .literal(gamerule, name)
                .runs(Branch::Query(rule))
                .in_any_world()
                .id()?;
            *value = match definition.value {
                GameRuleValueType::Bool { .. } => ValueNode::Bool(
                    registrar
                        .arg(named, "value", BoolArgument)
                        .runs(Branch::Set(rule))
                        .in_any_world()
                        .id()?,
                ),
                GameRuleValueType::Int { min, max, .. } => ValueNode::Int(
                    registrar
                        .arg(named, "value", IntegerArgument { min, max })
                        .runs(Branch::Set(rule))
                        .in_any_world()
                        .id()?,
                ),
            };
        }
    }
    Ok(GameRuleCommand { values })
}

/// The text of a rule value and the result of a command that reads it.
fn value_text(rule: GameRule, value: i32) -> String {
    match rule.definition().value {
        GameRuleValueType::Bool { .. } => (value != 0).to_string(),
        GameRuleValueType::Int { .. } => value.to_string(),
    }
}

fn result_of(rule: GameRule, value: i32) -> i32 {
    match rule.definition().value {
        GameRuleValueType::Bool { .. } => i32::from(value != 0),
        GameRuleValueType::Int { .. } => value,
    }
}

/// The value that the line gives the rule, under whichever of its two names it holds.
fn given_value(
    context: &VanillaContext,
    call: &Call<Branch, GameRuleCommand>,
    rule: GameRule,
) -> Result<i32, CommandFailure> {
    let nodes = call
        .command
        .values
        .get(usize::from(rule.id().number()))
        .ok_or_else(CommandFailure::failed)?;
    for node in nodes {
        let value = match *node {
            ValueNode::Bool(handle) => context.opt(&call.invocation, handle)?.map(i32::from),
            ValueNode::Int(handle) => context.opt(&call.invocation, handle)?,
            ValueNode::Absent => None,
        };
        if let Some(value) = value {
            return Ok(value);
        }
    }
    Err(CommandFailure::failed())
}

pub(crate) fn run(
    In(call): In<Call<Branch, GameRuleCommand>>,
    context: VanillaContext,
    rules: Option<Res<GameRules>>,
    mut output: CommandOutput,
    mut changes: ResMut<EffectQueue<ServerChange>>,
) -> CommandResult {
    let rules = rules.as_deref().cloned().unwrap_or_default();
    match call.branch {
        Branch::Query(rule) => {
            let value = rules.get(rule);
            output.success(
                &call.invocation,
                Text::translate(
                    "commands.gamerule.query",
                    vec![
                        short_name(rule).into_text(),
                        value_text(rule, value).into_text(),
                    ],
                ),
                Feedback::Source,
            );
            Ok(result_of(rule, value))
        }
        Branch::Set(rule) => {
            let value = given_value(&context, &call, rule)?;
            let args = vec![
                short_name(rule).into_text().into(),
                value_text(rule, value).into_text().into(),
            ];
            if rules.get(rule) == value {
                return Err(CommandFailure::translate("commands.gamerule.not_set", args));
            }
            output.effect(
                &mut changes,
                ServerChange(ServerEdit::GameRule { rule, value }),
            );
            output.success(
                &call.invocation,
                Text::translate_args("commands.gamerule.set", args),
                Feedback::SourceAndOperators,
            );
            Ok(result_of(rule, value))
        }
    }
}
