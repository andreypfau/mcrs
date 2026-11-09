use std::collections::VecDeque;

use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use mcrs_minecraft_dimension::dimension_keys::OVERWORLD;
use mcrs_minecraft_engine::channel_types::ToDim;
use mcrs_minecraft_engine::command_handoff::{CommandHandoff, SourceState};
use mcrs_minecraft_engine::coordinator::running_dimensions::RunningDimensions;
use mcrs_minecraft_engine::muxer::dim_inputs::DimInputs;
use mcrs_minecraft_engine_world::session::PlayerSession;

use mcrs_minecraft_lang::Language;

use crate::answer_log::AnswerLog;
use crate::console_lines::spawn_console_thread;

/// The most console lines that one tick hands on. A burst of lines runs over several ticks, in
/// order, so that no tick runs them all.
const MAX_LINES_PER_TICK: usize = 2_000;

/// Runs each line of the standard input as a command of the server source, in the overworld.
/// The embedder runs [`log_console_answers`](crate::log_console_answers) after the host took the
/// output of each tick. Without its threads the server runs with no console.
pub struct ConsolePlugin {
    /// The language of the answers in the log; an empty one writes translation keys.
    pub language: Language,
}

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        let log = match AnswerLog::spawn(self.language.clone()) {
            Ok(log) => log,
            Err(error) => {
                tracing::error!(%error, "the console log did not start; the server runs without a console");
                return;
            }
        };
        let (send, receive) = flume::unbounded();
        if let Err(error) = spawn_console_thread(send) {
            tracing::error!(%error, "the console did not start; no command line runs from the standard input");
            return;
        }
        app.insert_resource(Console {
            receive,
            pending: VecDeque::new(),
            log,
        });
        app.add_systems(Update, send_console_lines);
    }
}

/// The console lines that wait for a running overworld, and the log of the answers.
#[derive(Resource)]
pub(crate) struct Console {
    receive: flume::Receiver<String>,
    pending: VecDeque<String>,
    pub(crate) log: AnswerLog,
}

/// Hands the oldest console lines, at most [`MAX_LINES_PER_TICK`], to the overworld, which runs
/// them in its next tick. The lines wait while no overworld runs.
fn send_console_lines(
    mut console: ResMut<Console>,
    running: Option<Res<RunningDimensions>>,
    mut inputs: ResMut<DimInputs>,
) {
    let console = &mut *console;
    console.pending.extend(console.receive.try_iter());
    if console.pending.is_empty() {
        return;
    }
    let Some(overworld) = running
        .as_deref()
        .and_then(|running| running.iter().find(|dimension| dimension.key == OVERWORLD))
        .map(|dimension| dimension.label)
    else {
        return;
    };
    let count = console.pending.len().min(MAX_LINES_PER_TICK);
    for line in console.pending.drain(..count) {
        let handoff = CommandHandoff::single(
            overworld,
            PlayerSession::SERVER,
            line,
            SourceState::server(overworld),
            0,
        );
        if !inputs.send_control(overworld, ToDim::Command(Box::new(handoff))) {
            tracing::error!("the overworld takes no input; a console line is lost");
        }
    }
}
