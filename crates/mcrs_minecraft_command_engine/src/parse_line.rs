use crate::command_outbox::CommandOutbox;
use crate::command_queue::QueuedLine;
use crate::command_set::CommandSet;
use crate::context::CommandContext;
use crate::registrar::Handler;
use crate::source_state::{PlayerLevels, SourceState, player_source};
use crate::system_message::system_message;
use bevy_ecs::prelude::{InMut, Local, ResMut};
use mcrs_command::{Chain, ParseScratch};

/// A queued line and the chain its parse fills, which the pass reuses from line to line.
pub(crate) struct LineToParse {
    pub(crate) queued: QueuedLine,
    pub(crate) chain: Chain<Handler>,
}

/// Parses a queued line in the world of its source dimension, with the graph and the targets
/// lent to it. A parse error goes to the outbox, and the line does not run.
pub(crate) fn parse_line<S: CommandSet>(
    input: InMut<LineToParse>,
    context: CommandContext<S>,
    levels: PlayerLevels,
    mut outbox: ResMut<CommandOutbox>,
    mut scratch: Local<ParseScratch<S::Error>>,
) -> Option<SourceState> {
    let LineToParse {
        queued: QueuedLine { player, line },
        chain,
    } = input.0;
    let (state, _) = player_source(context.targets(), &levels, *player)?;
    let graph = context.tree.graph()?;
    let source = context.source_of(&state).ok()?;
    let parse = graph.parse(line, 0, &source, &mut scratch);
    if let Err(error) = parse.check(graph) {
        for message in mcrs_minecraft_command::error_feedback(line, &error) {
            outbox.packets.extend(system_message(*player, &message));
        }
        return None;
    }
    chain.copy_from(graph, &parse);
    Some(state)
}
