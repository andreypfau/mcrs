use std::fmt;
use std::str::FromStr;

use crate::Event;
use crate::FORMAT_LINE;
use crate::ParseError;
use crate::fields::Fields;

/// One recorded step of an experiment: the world it starts in and the packets of its client
/// ticks.
#[derive(Clone, Debug, PartialEq)]
pub struct Recording {
    pub header: Header,
    /// The events in the order the client applied or sent them. The first events give the state
    /// of the player before the step: abilities, attributes and effects, then the teleport.
    pub events: Vec<Event>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    /// The game version of the server and the client.
    pub version: String,
    /// The experiment of `docs/spec/entity-physics.md`, section 15, for example `E9`.
    pub experiment: String,
    pub step: String,
    /// The text the human read before the step.
    pub instruction: String,
    /// The blocks that the step placed, in order, into an empty world.
    pub fills: Vec<Fill>,
}

/// The cuboid from `min` to `max`, both included, filled with one block state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fill {
    pub min: [i32; 3],
    pub max: [i32; 3],
    /// The state as `name[property=value,...]`.
    pub state: String,
}

impl Recording {
    /// The client ticks: each slice ends with the [`Event::TickEnd`] of its tick. Events after
    /// the last tick end are not in a tick.
    pub fn ticks(&self) -> impl Iterator<Item = &[Event]> {
        self.events
            .split_inclusive(|event| *event == Event::TickEnd)
            .filter(|tick| tick.last() == Some(&Event::TickEnd))
    }
}

/// The arguments of a `fill`: the corners and the state.
impl fmt::Display for Fill {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [x0, y0, z0] = self.min;
        let [x1, y1, z1] = self.max;
        write!(f, "{x0} {y0} {z0} {x1} {y1} {z1} {}", self.state)
    }
}

impl fmt::Display for Recording {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let header = &self.header;
        writeln!(f, "{FORMAT_LINE}")?;
        writeln!(f, "version {}", header.version)?;
        writeln!(f, "experiment {}", header.experiment)?;
        writeln!(f, "step {}", header.step)?;
        writeln!(f, "instruction {}", header.instruction)?;
        for fill in &header.fills {
            writeln!(f, "fill {fill}")?;
        }
        writeln!(f, "events")?;
        for event in &self.events {
            writeln!(f, "{event}")?;
        }
        Ok(())
    }
}

impl FromStr for Recording {
    type Err = ParseError;

    /// Reads the header lines in the order that [`fmt::Display`] writes them.
    fn from_str(text: &str) -> Result<Self, ParseError> {
        let lines: Vec<&str> = text.lines().collect();
        let field = |index: usize, keyword: &str| {
            lines
                .get(index)
                .and_then(|line| line.strip_prefix(keyword)?.strip_prefix(' '))
                .map(str::to_owned)
                .ok_or_else(|| ParseError {
                    line: index + 1,
                    message: format!("expected `{keyword}`"),
                })
        };
        if lines.first() != Some(&FORMAT_LINE) {
            return Err(ParseError {
                line: 1,
                message: format!("the recording does not start with `{FORMAT_LINE}`"),
            });
        }
        let version = field(1, "version")?;
        let experiment = field(2, "experiment")?;
        let step = field(3, "step")?;
        let instruction = field(4, "instruction")?;
        let error = |line: usize| move |message: String| ParseError { line, message };
        let start = lines
            .iter()
            .position(|line| *line == "events")
            .ok_or_else(|| error(lines.len())("no `events` line".to_owned()))?;
        let numbered = |range: std::ops::Range<usize>| {
            lines.iter().enumerate().take(range.end).skip(range.start)
        };
        let fills = numbered(5..start)
            .map(|(index, text)| parse_fill(text).map_err(error(index + 1)))
            .collect::<Result<Vec<_>, _>>()?;
        let events = numbered(start + 1..lines.len())
            .map(|(index, text)| Event::parse(text).map_err(error(index + 1)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            header: Header {
                version,
                experiment,
                step,
                instruction,
                fills,
            },
            events,
        })
    }
}

fn parse_fill(line: &str) -> Result<Fill, String> {
    let mut fields = Fields::new(line);
    if fields.word("fill")? != "fill" {
        return Err("expected `fill` or `events`".to_owned());
    }
    let fill = Fill {
        min: fields.array3("min")?,
        max: fields.array3("max")?,
        state: fields.word("state")?.to_owned(),
    };
    fields.end()?;
    Ok(fill)
}
