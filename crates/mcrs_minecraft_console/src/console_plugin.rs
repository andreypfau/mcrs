use std::collections::VecDeque;

use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use mcrs_minecraft_command::CommandLevel;
use mcrs_minecraft_dimension::dimension_keys::OVERWORLD;
use mcrs_minecraft_engine::channel_types::ToDim;
use mcrs_minecraft_engine::command_handoff::{CommandHandoff, SourceState};
use mcrs_minecraft_engine::coordinator::running_dimensions::RunningDimensions;
use mcrs_minecraft_engine::muxer::dim_inputs::DimInputs;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::session::PlayerSession;

use crate::console_lines::{ConsoleInput, spawn_console_thread};
use crate::language::Language;

/// The most console lines that one tick hands on. A burst of lines runs over several ticks, in
/// order, so that no tick runs them all.
const MAX_LINES_PER_TICK: usize = 2_000;

/// Runs each line of the standard input as a command of the server source, in the overworld.
/// The embedder runs [`log_console_answers`](crate::log_console_answers) after the host took the
/// output of each tick.
pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        let (send, receive) = flume::unbounded();
        spawn_console_thread(send);
        app.insert_resource(Console {
            receive,
            pending: VecDeque::new(),
            language: Language::default(),
        });
        app.add_systems(Update, send_console_lines);
    }
}

/// The console lines that wait for a running overworld, and the language of the answers.
#[derive(Resource)]
pub(crate) struct Console {
    receive: flume::Receiver<ConsoleInput>,
    pending: VecDeque<String>,
    pub(crate) language: Language,
}

/// Hands the oldest console lines, at most [`MAX_LINES_PER_TICK`], to the overworld, which runs
/// them in its next tick. The lines wait while no overworld runs.
fn send_console_lines(
    mut console: ResMut<Console>,
    running: Option<Res<RunningDimensions>>,
    mut inputs: ResMut<DimInputs>,
) {
    let console = &mut *console;
    for input in console.receive.try_iter() {
        match input {
            ConsoleInput::Language(language) => console.language = language,
            ConsoleInput::Line(line) => console.pending.push_back(line),
        }
    }
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
        let mut origin = SourceState::of_player(
            PlayerSession::SERVER,
            overworld,
            CommandLevel::Owners,
            &Transform::default(),
        );
        origin.player = None;
        let handoff = CommandHandoff {
            to: overworld,
            output: PlayerSession::SERVER,
            line,
            sources: vec![origin.clone()],
            origin,
            stage: 0,
            cost: 0,
        };
        if !inputs.send_control(overworld, ToDim::Command(Box::new(handoff))) {
            tracing::error!("the overworld takes no input; a console line is lost");
        }
    }
}
