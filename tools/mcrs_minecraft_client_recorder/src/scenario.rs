use std::str::FromStr;

use anyhow::Context;
use anyhow::bail;
use mcrs_minecraft_client_ticks::Fill;

/// The script of one experiment: the server setup of each step and what the human does in it.
pub(crate) struct Scenario {
    pub(crate) experiment: String,
    /// Lines of `server.properties` that the experiment needs, as `key=value`.
    pub(crate) properties: Vec<String>,
    /// The air fill that empties the cuboid of each step before its fills; the replay takes the
    /// world outside it as air.
    pub(crate) arena: Fill,
    pub(crate) steps: Vec<Step>,
}

pub(crate) struct Step {
    pub(crate) name: String,
    pub(crate) instruction: String,
    pub(crate) fills: Vec<Fill>,
    /// Console commands after the fills, for example a game mode or an effect.
    pub(crate) commands: Vec<String>,
    /// The position and the rotation of the teleport that starts the step.
    pub(crate) start: [f64; 5],
    /// The client tick of the step at which the human starts; it becomes a timed title.
    go: u32,
    /// The number of client ticks of the step.
    pub(crate) ticks: u32,
    /// Console commands at a client tick of the step.
    pub(crate) timed: Vec<Timed>,
}

pub(crate) struct Timed {
    pub(crate) tick: u32,
    pub(crate) command: String,
}

impl FromStr for Scenario {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> anyhow::Result<Self> {
        let mut scenario = Self {
            experiment: String::new(),
            properties: Vec::new(),
            arena: Fill {
                min: [0; 3],
                max: [0; 3],
                state: "minecraft:air".to_owned(),
            },
            steps: Vec::new(),
        };
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            scenario
                .line(line)
                .with_context(|| format!("line {}: {line}", index + 1))?;
        }
        if scenario.experiment.is_empty() || scenario.steps.is_empty() {
            bail!("a scenario needs an `experiment` line and at least one `step`");
        }
        for step in &mut scenario.steps {
            step.timed.push(Timed {
                tick: step.go,
                command: "title @a title \"Go\"".to_owned(),
            });
            step.timed.push(Timed {
                tick: step.ticks,
                command: "title @a title \"Stop\"".to_owned(),
            });
        }
        Ok(scenario)
    }
}

impl Scenario {
    fn line(&mut self, line: &str) -> anyhow::Result<()> {
        let (keyword, rest) = line.split_once(' ').unwrap_or((line, ""));
        if keyword == "experiment" {
            self.experiment = rest.to_owned();
            return Ok(());
        }
        if keyword == "property" {
            self.properties.push(rest.to_owned());
            return Ok(());
        }
        if keyword == "arena" {
            [self.arena.min, self.arena.max] = cuboid(rest)?;
            return Ok(());
        }
        if keyword == "step" {
            self.steps.push(Step {
                name: rest.to_owned(),
                instruction: String::new(),
                fills: Vec::new(),
                commands: Vec::new(),
                start: [0.0; 5],
                go: 60,
                ticks: 200,
                timed: Vec::new(),
            });
            return Ok(());
        }
        let Some(step) = self.steps.last_mut() else {
            bail!("`{keyword}` before the first step");
        };
        match keyword {
            "instruction" => step.instruction = rest.to_owned(),
            "fill" => {
                let (bounds, state) = rest.rsplit_once(' ').context("no block state")?;
                let [min, max] = cuboid(bounds)?;
                step.fills.push(Fill {
                    min,
                    max,
                    state: state.to_owned(),
                });
            }
            "run" => step.commands.push(rest.to_owned()),
            "start" => {
                let numbers: Vec<f64> = rest
                    .split_whitespace()
                    .map(str::parse)
                    .collect::<Result<_, _>>()?;
                step.start = numbers
                    .try_into()
                    .ok()
                    .context("start needs x y z yaw pitch")?;
            }
            "go" => step.go = rest.parse()?,
            "ticks" => step.ticks = rest.parse()?,
            "at" => {
                let (tick, command) = rest
                    .split_once(' ')
                    .context("at needs a tick and a command")?;
                step.timed.push(Timed {
                    tick: tick.parse()?,
                    command: command.to_owned(),
                });
            }
            _ => bail!("unknown keyword `{keyword}`"),
        }
        Ok(())
    }
}

fn cuboid(text: &str) -> anyhow::Result<[[i32; 3]; 2]> {
    let values: Vec<i32> = text
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    let [x0, y0, z0, x1, y1, z1] = values[..] else {
        bail!("a cuboid needs six integers");
    };
    Ok([[x0, y0, z0], [x1, y1, z1]])
}
