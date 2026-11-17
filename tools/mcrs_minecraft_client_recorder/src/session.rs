use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use mcrs_minecraft_client_ticks::Abilities;
use mcrs_minecraft_client_ticks::Attribute;
use mcrs_minecraft_client_ticks::DataEntity;
use mcrs_minecraft_client_ticks::Effect;
use mcrs_minecraft_client_ticks::EntityData;
use mcrs_minecraft_client_ticks::Event;
use mcrs_minecraft_client_ticks::Header;
use mcrs_minecraft_client_ticks::Recording;
use mcrs_minecraft_core::VERSION;

use crate::console::Console;
use crate::proxy::Observed;
use crate::scenario::Scenario;
use crate::scenario::Step;

const SETTLE_TICKS: u32 = 40;

enum Phase {
    Waiting,
    Prompt(usize),
    Settling { step: usize, ticks_left: u32 },
    Starting(usize),
    Recording { step: usize, segment: Segment },
    Done,
}

struct Segment {
    events: Vec<(u64, Event)>,
    gap: usize,
    ticks: u32,
    rewritten: bool,
}

#[derive(Default)]
struct Snapshot {
    abilities: Option<Abilities>,
    attributes: BTreeMap<String, Attribute>,
    effects: BTreeMap<String, Effect>,
    data: BTreeMap<u8, EntityData>,
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
            Event::EntityData(data) if data.entity == DataEntity::Own => {
                self.data.insert(data.index, data.clone());
            }
            _ => {}
        }
    }

    fn events(&self) -> Vec<Event> {
        let abilities = self.abilities.map(Event::Abilities);
        let attributes = self.attributes.values().cloned().map(Event::Attribute);
        let effects = self.effects.values().cloned().map(Event::Effect);
        let data = self.data.values().cloned().map(Event::EntityData);
        abilities
            .into_iter()
            .chain(attributes)
            .chain(effects)
            .chain(data)
            .collect()
    }
}

pub(crate) struct Session {
    pub(crate) console: Console,
    scenario: Scenario,
    out: PathBuf,
    input: Option<PathBuf>,
    cheating: bool,
    phase: Phase,
    snapshot: Snapshot,
    rewritten_at_join: bool,
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
            rewritten_at_join: false,
        }
    }

    pub(crate) fn is_done(&self) -> bool {
        matches!(self.phase, Phase::Done)
    }

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
                            rewritten: self.rewritten_at_join,
                        },
                    };
                } else if let Phase::Recording { segment, .. } = &mut self.phase {
                    segment.events.insert(segment.gap, (time, event.clone()));
                    segment.gap += 1;
                }
                self.snapshot.apply(&event);
            }
            Observed::Sent(time, event) => self.sent(time, event)?,
            Observed::Rewritten => match &mut self.phase {
                Phase::Recording { segment, .. } => segment.rewritten = true,
                Phase::Waiting => self.rewritten_at_join = true,
                _ => {}
            },
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

    pub(crate) fn enter(&mut self) -> anyhow::Result<()> {
        let Phase::Prompt(step) = self.phase else {
            return Ok(());
        };
        let current = step_at(&self.scenario, step)?;
        if let Some(input) = &self.input {
            fs::write(input, current.input_script())?;
        }
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
