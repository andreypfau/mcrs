use std::collections::BTreeMap;

use mcrs_engine_link::{Link, Placement};

use crate::cell_id::CellId;
use crate::cell_output::CellOutput;
use crate::cell_stage::{CellStage, Command, Reply};
use crate::cell_world::CellWorld;
use crate::lent::Lent;
use crate::tick::Tick;
use crate::tick_error::TickError;

/// The worlds that tick in lockstep: every world ticks `T` before any world ticks `T + 1`.
///
/// Each world runs where its [`Placement`] says. Dropping `Cells` stops every world.
pub struct Cells<W: CellWorld> {
    cells: BTreeMap<CellId, Cell<W>>,
    next_id: u64,
}

struct Cell<W: CellWorld> {
    link: Link<CellStage<W>>,
    whereabouts: Whereabouts,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Whereabouts {
    Home,
    Lent,
}

impl<W: CellWorld> Default for Cells<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: CellWorld> Cells<W> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            cells: BTreeMap::new(),
            next_id: 0,
        }
    }

    /// Places `world` as `placement` asks. It ticks first on the next [`Self::tick`].
    pub fn add(&mut self, world: W, placement: Placement) -> CellId {
        let cell = CellId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        let link = Link::new(CellStage::new(world), placement);
        self.cells.insert(
            cell,
            Cell {
                link,
                whereabouts: Whereabouts::Home,
            },
        );
        cell
    }

    /// Stops the world of `cell` and hands it back. It gives `None` when the world is lent
    /// out or has stopped, and then the world goes when its [`Lent`] goes.
    pub async fn remove(&mut self, cell: CellId) -> Option<W> {
        let lent = self.lend(cell).await;
        self.cells.remove(&cell);
        Some(lent?.world)
    }

    /// Sends `tick` with the input from `input` to every world, waits until every world has
    /// ticked, and gives the outputs in [`CellId`] order. A world that has stopped gives no
    /// output.
    pub async fn tick(
        &mut self,
        tick: Tick,
        mut input: impl FnMut(CellId) -> W::Input,
    ) -> Result<Vec<CellOutput<W::Output>>, TickError> {
        let lent = self
            .cells
            .iter()
            .find(|(_, entry)| entry.whereabouts == Whereabouts::Lent);
        if let Some((&cell, _)) = lent {
            return Err(TickError::Lent { cell });
        }
        for (&cell, entry) in &mut self.cells {
            entry.link.send(Command::Tick {
                tick,
                input: input(cell),
            });
        }
        for entry in self.cells.values_mut() {
            entry.link.answered().await;
        }
        let mut outputs = Vec::with_capacity(self.cells.len());
        for (&cell, entry) in &mut self.cells {
            for reply in entry.link.outputs() {
                if let Reply::Ticked(output) = reply {
                    outputs.push(CellOutput { cell, output });
                }
            }
        }
        Ok(outputs)
    }

    /// Takes the world of `cell` out between two ticks. No world ticks until it is given back.
    /// It gives `None` when the world is lent out already or has stopped.
    ///
    /// Transitional, as [`Lent`] says.
    pub async fn lend(&mut self, cell: CellId) -> Option<Lent<W>> {
        let entry = self.cells.get_mut(&cell)?;
        if entry.whereabouts == Whereabouts::Lent {
            return None;
        }
        entry.link.send(Command::Lend);
        entry.link.answered().await;
        let world = entry.link.outputs().find_map(|reply| match reply {
            Reply::Lent(world) => Some(world),
            Reply::Ticked(_) => None,
        })?;
        entry.whereabouts = Whereabouts::Lent;
        Some(Lent { cell, world })
    }

    /// Puts a lent world back in its cell. A world whose cell was removed meanwhile is dropped.
    pub fn give_back(&mut self, lent: Lent<W>) {
        let Some(entry) = self.cells.get_mut(&lent.cell) else {
            return;
        };
        entry.link.send(Command::Return(lent.world));
        entry.whereabouts = Whereabouts::Home;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::mpsc;
    use std::sync::mpsc::{Receiver, Sender};

    use bevy_tasks::block_on;

    /// Adds its tick and its input to a running total, and gives the total.
    struct Sum {
        total: u64,
    }

    impl CellWorld for Sum {
        type Input = u64;
        type Output = u64;

        fn tick(&mut self, tick: Tick, input: u64) -> u64 {
            self.total += tick.0 + input;
            self.total
        }
    }

    fn sums(placements: [Placement; 3]) -> Vec<CellOutput<u64>> {
        let mut cells = Cells::new();
        let mut ids = Vec::new();
        for (start, placement) in placements.into_iter().enumerate() {
            ids.push(cells.add(
                Sum {
                    total: start as u64 * 100,
                },
                placement,
            ));
        }
        let input = |cell| ids.iter().position(|&id| id == cell).unwrap() as u64 * 10;
        let mut outputs = Vec::new();
        for tick in 0..4 {
            let ticked = block_on(cells.tick(Tick(tick), input)).expect("no world is lent");
            outputs.extend(ticked);
        }
        outputs
    }

    #[test]
    fn every_placement_gives_the_same_outputs() {
        let inline = sums([Placement::Inline; 3]);
        assert_eq!(inline.len(), 12);
        assert_eq!(inline, sums([Placement::Thread; 3]));
        assert_eq!(
            inline,
            sums([Placement::Thread, Placement::Inline, Placement::Thread])
        );
    }

    #[test]
    fn a_lent_world_keeps_what_the_borrower_changed() {
        for placement in [Placement::Inline, Placement::Thread] {
            let mut cells = Cells::new();
            let cell = cells.add(Sum { total: 0 }, placement);
            let mut lent = block_on(cells.lend(cell)).expect("the world is home");
            assert_eq!(
                block_on(cells.tick(Tick(1), |_| 0)),
                Err(TickError::Lent { cell })
            );
            lent.world_mut().total = 40;
            cells.give_back(lent);

            let outputs = block_on(cells.tick(Tick(1), |_| 1)).expect("the world is back");

            assert_eq!(outputs, [CellOutput { cell, output: 42 }]);
        }
    }

    /// Signals the gate of another world, or waits on a gate of its own, and gives its name.
    struct Gated {
        name: u8,
        signal: Option<Sender<()>>,
        gate: Option<Receiver<()>>,
    }

    impl CellWorld for Gated {
        type Input = ();
        type Output = u8;

        fn tick(&mut self, _tick: Tick, _input: ()) -> u8 {
            if let Some(signal) = &self.signal {
                signal.send(()).expect("the waiting world is alive");
            }
            if let Some(gate) = &self.gate {
                gate.recv().expect("the signalling world is alive");
            }
            self.name
        }
    }

    #[test]
    fn outputs_come_in_cell_order_whatever_world_finishes_first() {
        let (signal, gate) = mpsc::channel();
        let mut cells = Cells::new();
        let first = cells.add(
            Gated {
                name: 1,
                signal: None,
                gate: Some(gate),
            },
            Placement::Thread,
        );
        let second = cells.add(
            Gated {
                name: 2,
                signal: Some(signal),
                gate: None,
            },
            Placement::Thread,
        );

        let outputs = block_on(cells.tick(Tick(0), |_| ())).expect("no world is lent");

        assert_eq!(
            outputs,
            [
                CellOutput {
                    cell: first,
                    output: 1
                },
                CellOutput {
                    cell: second,
                    output: 2
                },
            ]
        );
    }

    /// Holds the only sender of a channel, so the channel disconnects once the world is gone.
    struct Alive {
        _alive: Sender<()>,
    }

    impl CellWorld for Alive {
        type Input = ();
        type Output = ();

        fn tick(&mut self, _tick: Tick, _input: ()) {}
    }

    #[test]
    fn dropping_the_cells_stops_the_threads() {
        let (alive, gone) = mpsc::channel();
        let mut cells = Cells::new();
        cells.add(Alive { _alive: alive }, Placement::Thread);
        block_on(cells.tick(Tick(0), |_| ())).expect("no world is lent");

        drop(cells);

        assert!(gone.recv().is_err(), "the world sent nothing");
    }
}
