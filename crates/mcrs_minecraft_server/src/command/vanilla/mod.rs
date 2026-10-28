mod execute;
pub(crate) mod gamemode;
mod locate;
pub(crate) mod teleport;
pub(crate) mod time;

use crate::command::{AppCommands, CommandContext, CommandFailure, Invocation, PlayerEntry};
use bevy_app::{App, Plugin};
use mcrs_command::Arg;
use mcrs_minecraft_command::{EntityArgument, argument_text};

/// The vanilla commands: `execute`, `gamemode`, `teleport` with its alias `tp`, `locate` and
/// `time`.
pub struct VanillaCommandsPlugin;

impl Plugin for VanillaCommandsPlugin {
    fn build(&self, app: &mut App) {
        let added = [
            (
                "execute",
                app.add_modifying_command(execute::build, execute::run, execute::modify)
                    .map(drop),
            ),
            (
                "gamemode",
                app.add_command(gamemode::build, gamemode::run).map(drop),
            ),
            (
                "teleport",
                app.add_command(teleport::build, teleport::run).map(drop),
            ),
            (
                "locate",
                app.add_command(locate::build, locate::run).map(drop),
            ),
            ("time", app.add_command(time::build, time::run).map(drop)),
        ];
        for (name, result) in added {
            if let Err(error) = result {
                tracing::error!("{name}: {error}");
            }
        }
    }
}

/// The players a selector argument finds in every dimension (ARG-ENT-013, ARG-ENT-014), or the
/// failure of its search.
pub(crate) fn select<'c>(
    context: &'c CommandContext,
    call: &Invocation,
    arg: Arg<EntityArgument>,
) -> Result<Vec<&'c PlayerEntry>, CommandFailure> {
    select_if_present(context, call, arg)?.ok_or_else(CommandFailure::failed)
}

fn select_if_present<'c>(
    context: &'c CommandContext,
    call: &Invocation,
    arg: Arg<EntityArgument>,
) -> Result<Option<Vec<&'c PlayerEntry>>, CommandFailure> {
    let Some(selector) = context.opt(call, arg)? else {
        return Ok(None);
    };
    let source = context.source(call)?;
    let targets = context.targets();
    let me = call.source.entity.and_then(|anchor| targets.player(anchor));
    let mut out = Vec::new();
    let mut random = |bound: usize| rand::random_range(0..bound.max(1));
    selector
        .select(&source, me, targets.players().iter(), &mut random, &mut out)
        .map_err(|error| CommandFailure(argument_text(context.line(), error)))?;
    Ok(Some(out))
}

/// The players a selector finds, or the source player when the branch has no selector.
pub(crate) fn targets_or_self<'c>(
    context: &'c CommandContext,
    call: &Invocation,
    arg: Arg<EntityArgument>,
    none_found: &'static str,
) -> Result<Vec<&'c PlayerEntry>, CommandFailure> {
    let Some(found) = select_if_present(context, call, arg)? else {
        return Ok(call
            .source
            .entity
            .and_then(|anchor| context.targets().player(anchor))
            .into_iter()
            .collect());
    };
    if found.is_empty() {
        return Err(CommandFailure::translate(none_found, vec![]));
    }
    Ok(found)
}
