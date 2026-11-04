use std::collections::VecDeque;
use std::convert::Infallible;

use mcrs_engine_link::{Stage, Step, TaskOutcome};

use crate::cell_world::CellWorld;
use crate::tick::Tick;

pub(crate) enum Command<W: CellWorld> {
    Tick { tick: Tick, input: W::Input },
    Lend,
    Return(W),
    Stop,
}

pub(crate) enum Reply<W: CellWorld> {
    Ticked(W::Output),
    Lent(W),
    Stopped(W),
}

/// The far end of the link to one world. The world moves out on a lend and back on a return,
/// so a lend works the same on every placement.
pub(crate) struct CellStage<W: CellWorld> {
    world: Option<W>,
    commands: VecDeque<Command<W>>,
}

impl<W: CellWorld> CellStage<W> {
    pub(crate) fn new(world: W) -> Self {
        Self {
            world: Some(world),
            commands: VecDeque::new(),
        }
    }
}

impl<W: CellWorld> Stage for CellStage<W> {
    const NAME: &'static str = "cell";

    type Input = Command<W>;
    type Output = Reply<W>;
    type TaskOutput = Infallible;

    fn receive(&mut self, input: Command<W>) {
        self.commands.push_back(input);
    }

    fn finish(&mut self, _outcome: TaskOutcome<Infallible>) {}

    fn step(&mut self, step: &mut Step<'_, Self>) {
        for command in self.commands.drain(..) {
            match command {
                Command::Tick { tick, input } => {
                    if let Some(world) = &mut self.world {
                        step.emit(Reply::Ticked(world.tick(tick, input)));
                    }
                }
                Command::Lend => {
                    if let Some(world) = self.world.take() {
                        step.emit(Reply::Lent(world));
                    }
                }
                Command::Return(world) => self.world = Some(world),
                Command::Stop => {
                    if let Some(world) = self.world.take() {
                        step.emit(Reply::Stopped(world));
                    }
                }
            }
        }
    }

    fn has_work(&self) -> bool {
        !self.commands.is_empty()
    }

    fn is_idle(&self) -> bool {
        self.commands.is_empty()
    }
}
