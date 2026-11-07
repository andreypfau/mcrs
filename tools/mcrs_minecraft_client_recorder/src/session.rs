use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use mcrs_minecraft_client_ticks::Abilities;
use mcrs_minecraft_client_ticks::Attribute;
use mcrs_minecraft_client_ticks::Effect;
use mcrs_minecraft_client_ticks::Event;
use mcrs_minecraft_client_ticks::Header;
use mcrs_minecraft_client_ticks::Recording;
use mcrs_minecraft_core::VERSION;

use crate::console::Console;
use crate::proxy::Observed;
use crate::scenario::Scenario;
use crate::scenario::Step;

/// The client ticks between the teleport before a step and the recorded teleport that starts
/// it, so that the client has the chunks and stands still.
const SETTLE_TICKS: u32 = 40;

enum Phase {
    /// No client has loaded the world yet.
    Waiting,
    /// The instruction of the step is on the terminal; Enter sets the step up.
    Prompt(usize),
    /// The step is set up; the recorded teleport follows after the client ticks left.
    Settling {
        step: usize,
        ticks_left: u32,
    },
    /// The recorded teleport is sent; its application starts the recording.
    Starting(usize),
    Recording {
        step: usize,
        segment: Segment,
    },
    Done,
}

struct Segment {
    /// The time of each event and the event.
    events: Vec<(u64, Event)>,
    /// Where the events that the client applies before its next tick go.
    gap: usize,
    ticks: u32,
    rewritten: bool,
}

/// The state of the player that the client applied last, written at the start of each step.
#[derive(Default)]
struct Snapshot {
    abilities: Option<Abilities>,
    attributes: BTreeMap<String, Attribute>,
    effects: BTreeMap<String, Effect>,
}

impl Snapshot {
    fn apply(&mut self, event: &Event) {
        match event {
            Event::Abilities(abilities) => self.abilities = Some(*abilities),
            Event::Attribute(attribute) => {
                self.attributes
                    .insert(attribute.name.clone(), attribute.clone());
            }
            Event::Effect(effect) => {
                self.effects.insert(effect.name.clone(), effect.clone());
            }
            Event::RemoveEffect(name) => {
                self.effects.remove(name);
            }
            _ => {}
        }
    }

    fn events(&self) -> Vec<Event> {
        let abilities = self.abilities.map(Event::Abilities);
        let attributes = self.attributes.values().cloned().map(Event::Attribute);
        let effects = self.effects.values().cloned().map(Event::Effect);
        abilities
            .into_iter()
            .chain(attributes)
            .chain(effects)
            .collect()
    }
}

/// Drives the scenario of one experiment and writes one recording per step.
pub(crate) struct Session {
    pub(crate) console: Console,
    scenario: Scenario,
    out: PathBuf,
    /// Where the input agent of the client reads the script of the step; none when a human plays.
    input: Option<PathBuf>,
    /// The proxy emulates a cheat module: a step that it did not change is not a recording of it.
    cheating: bool,
    phase: Phase,
    snapshot: Snapshot,
}

impl Session {
    pub(crate) fn new(
        console: Console,
        scenario: Scenario,
        out: PathBuf,
        input: Option<PathBuf>,
        cheating: bool,
    ) -> Self {
        Self {
            console,
            scenario,
            out,
            input,
            cheating,
            phase: Phase::Waiting,
            snapshot: Snapshot::default(),
        }
    }

    pub(crate) fn is_done(&self) -> bool {
        matches!(self.phase, Phase::Done)
    }

    /// True while a disconnect loses no recorded data.
    pub(crate) fn is_scripted(&self) -> bool {
        self.input.is_some()
    }

    pub(crate) fn may_disconnect(&self) -> bool {
        matches!(self.phase, Phase::Waiting | Phase::Prompt(_) | Phase::Done)
    }

    pub(crate) fn observe(&mut self, observed: Observed) -> anyhow::Result<()> {
        match observed {
            Observed::Loaded => {
                if matches!(self.phase, Phase::Waiting) {
                    self.prompt(0);
                }
            }
            Observed::Applied(time, event) => {
                if let (Event::Teleport(_), Phase::Starting(step)) = (&event, &self.phase) {
                    let mut events: Vec<(u64, Event)> = self
                        .snapshot
                        .events()
                        .into_iter()
                        .map(|before| (time, before))
                        .collect();
                    events.push((time, event.clone()));
                    let gap = events.len();
                    self.phase = Phase::Recording {
                        step: *step,
                        segment: Segment {
                            events,
                            gap,
                            ticks: 0,
                            rewritten: false,
                        },
                    };
                } else if let Phase::Recording { segment, .. } = &mut self.phase {
                    segment.events.insert(segment.gap, (time, event.clone()));
                    segment.gap += 1;
                }
                self.snapshot.apply(&event);
            }
            Observed::Sent(time, event) => self.sent(time, event)?,
            Observed::Rewritten => {
                if let Phase::Recording { segment, .. } = &mut self.phase {
                    segment.rewritten = true;
                }
            }
        }
        Ok(())
    }

    fn sent(&mut self, time: u64, event: Event) -> anyhow::Result<()> {
        let tick_end = event == Event::TickEnd;
        match &mut self.phase {
            Phase::Settling { step, ticks_left } if tick_end => {
                *ticks_left = ticks_left.saturating_sub(1);
                if *ticks_left == 0 {
                    let step = *step;
                    self.phase = Phase::Starting(step);
                    self.teleport(step)?;
                }
            }
            Phase::Recording { step, segment } => {
                segment.events.push((time, event));
                if !tick_end {
                    return Ok(());
                }
                segment.gap = segment.events.len();
                segment.ticks += 1;
                let step = *step;
                let tick = segment.ticks;
                self.on_tick(step, tick)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn on_tick(&mut self, step: usize, tick: u32) -> anyhow::Result<()> {
        let current = step_at(&self.scenario, step)?;
        for timed in current.timed.iter().filter(|timed| timed.tick == tick) {
            self.console.run(&timed.command)?;
        }
        if tick >= current.ticks {
            self.finish(step)?;
        }
        Ok(())
    }

    /// Sets the step up after Enter on the terminal.
    pub(crate) fn enter(&mut self) -> anyhow::Result<()> {
        let Phase::Prompt(step) = self.phase else {
            return Ok(());
        };
        let current = step_at(&self.scenario, step)?;
        if let Some(input) = &self.input {
            fs::write(input, current.input_script())?;
        }
        // `fill` fails as a whole when a chunk of it is not loaded, as with a short view distance.
        let [x0, _, z0] = self.scenario.arena.min;
        let [x1, _, z1] = self.scenario.arena.max;
        self.console
            .run(&format!("forceload add {x0} {z0} {x1} {z1}"))?;
        self.console.run(&format!("fill {}", self.scenario.arena))?;
        for fill in &current.fills {
            self.console.run(&format!("fill {fill}"))?;
        }
        for command in &current.commands {
            self.console.run(command)?;
        }
        self.teleport(step)?;
        self.phase = Phase::Settling {
            step,
            ticks_left: SETTLE_TICKS,
        };
        Ok(())
    }

    fn teleport(&mut self, step: usize) -> anyhow::Result<()> {
        let [x, y, z, yaw, pitch] = step_at(&self.scenario, step)?.start;
        self.console
            .run(&format!("tp @a {x} {y} {z} {yaw} {pitch}"))
    }

    fn finish(&mut self, step: usize) -> anyhow::Result<()> {
        let Phase::Recording { segment, .. } = std::mem::replace(&mut self.phase, Phase::Done)
        else {
            return Ok(());
        };
        let current = step_at(&self.scenario, step)?;
        if self.cheating && !segment.rewritten {
            println!(
                "the cheat module changed nothing in step {}: no recording",
                current.name
            );
        } else {
            self.write(current, segment)?;
        }
        if step + 1 < self.scenario.steps.len() {
            self.prompt(step + 1);
        }
        Ok(())
    }

    fn write(&self, current: &Step, segment: Segment) -> anyhow::Result<()> {
        let (times, events) = segment.events.into_iter().unzip();
        let recording = Recording {
            header: Header {
                version: VERSION.id.to_owned(),
                experiment: self.scenario.experiment.clone(),
                step: current.name.clone(),
                instruction: current.instruction.clone(),
                fills: current.fills.clone(),
            },
            events,
            times,
        };
        fs::create_dir_all(&self.out)?;
        let path = self.out.join(format!(
            "{}_{}.txt",
            self.scenario.experiment.to_lowercase(),
            current.name
        ));
        fs::write(&path, recording.to_string())?;
        println!("wrote {}", path.display());
        Ok(())
    }

    fn prompt(&mut self, step: usize) {
        if let Some(current) = self.scenario.steps.get(step) {
            println!(
                "\n[{} {}] {}\nPress Enter here, then go to the game. Release all keys; the step \
                 starts with a teleport, and you act when the title Go shows.",
                self.scenario.experiment, current.name, current.instruction
            );
        }
        self.phase = Phase::Prompt(step);
    }
}

fn step_at(scenario: &Scenario, index: usize) -> anyhow::Result<&Step> {
    scenario
        .steps
        .get(index)
        .with_context(|| format!("no step {index}"))
}
