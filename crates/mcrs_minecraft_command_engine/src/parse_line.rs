use crate::command_outbox::CommandOutbox;
use crate::command_set::CommandSet;
use crate::context::CommandContext;
use crate::registrar::Handler;
use crate::source_state::{PlayerLevels, SourceState, player_source};
use crate::system_message::system_message;
use bevy_ecs::prelude::{InMut, Local, ResMut};
use mcrs_command::{Chain, ParseScratch};
use mcrs_minecraft_engine_world::session::PlayerSession;

/// The source a line parses for.
pub(crate) enum ParseSource {
    /// The player who sent it, as this world holds it.
    Player,
    /// A source that another world parsed the line for.
    Given(SourceState),
}

/// A line, the source it parses for, and the chain its parse fills, which a world reuses from
/// line to line.
pub(crate) struct LineToParse {
    pub(crate) player: PlayerSession,
    pub(crate) line: String,
    pub(crate) source: ParseSource,
    pub(crate) chain: Chain<Handler>,
}

/// Parses a line in this world. A parse error goes to the outbox, and the line does not run.
pub(crate) fn parse_line<S: CommandSet>(
    input: InMut<LineToParse>,
    context: CommandContext<S>,
    levels: PlayerLevels,
    mut outbox: ResMut<CommandOutbox>,
    mut scratch: Local<ParseScratch<S::Error>>,
) -> Option<SourceState> {
    let LineToParse {
        player,
        line,
        source,
        chain,
    } = input.0;
    let state = match source {
        ParseSource::Player => player_source(context.targets(), &levels, *player)?.0,
        ParseSource::Given(state) => state.clone(),
    };
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
