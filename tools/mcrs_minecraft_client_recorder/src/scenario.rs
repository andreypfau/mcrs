use std::str::FromStr;

use anyhow::Context;
use anyhow::bail;
use mcrs_minecraft_client_ticks::Fill;

pub(crate) struct Scenario {
    pub(crate) experiment: String,
    pub(crate) properties: Vec<String>,
    pub(crate) arena: Fill,
    pub(crate) steps: Vec<Step>,
}

pub(crate) struct Step {
    pub(crate) name: String,
    pub(crate) instruction: String,
    inputs: Vec<String>,
    pub(crate) fills: Vec<Fill>,
    pub(crate) commands: Vec<String>,
    pub(crate) start: [f64; 5],
    go: u32,
    pub(crate) ticks: u32,
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
                inputs: Vec::new(),
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
            "input" => {
                check_input(rest)?;
                step.inputs.push(rest.to_owned());
            }
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

impl Step {
    pub(crate) fn input_script(&self) -> String {
        format!("go {}\n{}\n", self.go, self.inputs.join("\n"))
    }
}

fn check_input(text: &str) -> anyhow::Result<()> {
    const KEYS: [&str; 13] = [
        "forward",
        "backward",
        "left",
        "right",
        "jump",
        "shift",
        "sprint",
        "use",
        "leave_bed",
        "drop",
        "swap_hands",
        "inventory",
        "close",
    ];
    let words: Vec<&str> = text.split_whitespace().collect();
    let [tick, keys, rotation @ ..] = &words[..] else {
        bail!("input needs a tick and keys");
    };
    tick.parse::<u32>()?;
    let keys = *keys;
    let known = |key: &str| {
        KEYS.contains(&key)
            || key
                .strip_prefix("click:")
                .is_some_and(|slot| slot.parse::<u16>().is_ok())
    };
    if keys != "none" && !keys.split(',').all(known) {
        bail!("the keys are `none` or a comma list of {KEYS:?} and `click:<slot>`");
    }
    match rotation {
        [] => {}
        [yaw, pitch] => {
            yaw.parse::<f32>()?;
            pitch.parse::<f32>()?;
        }
        _ => bail!("the rotation is a yaw and a pitch"),
    }
    Ok(())
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
