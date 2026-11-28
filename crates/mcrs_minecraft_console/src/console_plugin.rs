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

const MAX_LINES_PER_TICK: usize = 2_000;

pub struct ConsolePlugin {
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
        app.add_systems(Update, send_console_lines.run_if(console_has_lines));
    }
}

#[derive(Resource)]
pub(crate) struct Console {
    receive: flume::Receiver<String>,
    pending: VecDeque<String>,
    pub(crate) log: AnswerLog,
}

#[cfg_attr(mcrs_lint, mcrs::event_condition)]
fn console_has_lines(console: Res<Console>) -> bool {
    !console.pending.is_empty() || !console.receive.is_empty()
}

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
