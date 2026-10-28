mod execute;
mod gamemode;
mod locate;
mod teleport;
mod time;

pub use gamemode::SetGameMode;
pub use teleport::{Facing, Teleport, TeleportPlayer};

use crate::command::{AppCommands, CommandContext, CommandFailure, Invocation, PlayerEntry};
use bevy_app::{App, Plugin};
use mcrs_command::{Arg, recycle};
use mcrs_minecraft_command::{EntityArgument, argument_text};

/// The vanilla commands: `execute`, `gamemode`, `teleport` with its alias `tp`, `locate` and
/// `time`, and their effects: [`TeleportPlayer`], [`SetGameMode`] and
/// [`EditClock`](crate::world::clock::EditClock).
pub struct VanillaCommandsPlugin;

impl Plugin for VanillaCommandsPlugin {
    fn build(&self, app: &mut App) {
        app.add_command_effect(teleport::apply)
            .add_command_effect(gamemode::apply)
            .add_command_effect(time::apply);
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

/// The players a selector found, which a handler keeps between its runs so that a search
/// allocates only when it finds more players than every search before it.
#[derive(Default)]
pub(crate) struct FoundPlayers(Vec<&'static PlayerEntry>);

impl FoundPlayers {
    /// Lends the buffer to `search` and takes it back on every path out of it.
    pub(crate) fn with<'c, R>(&mut self, search: impl FnOnce(&mut Vec<&'c PlayerEntry>) -> R) -> R {
        let mut found = recycle(core::mem::take(&mut self.0));
        let result = search(&mut found);
        self.0 = recycle(found);
        result
    }
}

/// The players a selector argument finds in every dimension (ARG-ENT-013, ARG-ENT-014), in
/// `out`, or the failure of its search.
pub(crate) fn select<'c>(
    context: &'c CommandContext,
    call: &Invocation,
    arg: Arg<EntityArgument>,
    out: &mut Vec<&'c PlayerEntry>,
) -> Result<(), CommandFailure> {
    if select_if_present(context, call, arg, out)? {
        Ok(())
    } else {
        Err(CommandFailure::failed())
    }
}

fn select_if_present<'c>(
    context: &'c CommandContext,
    call: &Invocation,
    arg: Arg<EntityArgument>,
    out: &mut Vec<&'c PlayerEntry>,
) -> Result<bool, CommandFailure> {
    let Some(selector) = context.opt(call, arg)? else {
        return Ok(false);
    };
    let source = context.source(call)?;
    let targets = context.targets();
    let me = call.source.entity.and_then(|anchor| targets.player(anchor));
    let mut random = |bound: usize| rand::random_range(0..bound.max(1));
    selector
        .select(&source, me, targets.players().iter(), &mut random, out)
        .map_err(|error| CommandFailure(argument_text(context.line(), error)))?;
    Ok(true)
}

/// The players a selector finds, or the source player when the branch has no selector, in
/// `out`.
pub(crate) fn targets_or_self<'c>(
    context: &'c CommandContext,
    call: &Invocation,
    arg: Arg<EntityArgument>,
    none_found: &'static str,
    out: &mut Vec<&'c PlayerEntry>,
) -> Result<(), CommandFailure> {
    if !select_if_present(context, call, arg, out)? {
        out.clear();
        out.extend(
            call.source
                .entity
                .and_then(|anchor| context.targets().player(anchor)),
        );
        return Ok(());
    }
    if out.is_empty() {
        return Err(CommandFailure::translate(none_found, vec![]));
    }
    Ok(())
}
