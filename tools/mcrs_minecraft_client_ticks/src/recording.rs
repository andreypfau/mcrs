use std::fmt;
use std::str::FromStr;

use crate::Event;
use crate::FORMAT_LINE;
use crate::ParseError;
use crate::ParseErrorKind;
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
    /// The network id of the player.
    pub entity_id: i32,
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
        writeln!(f, "entity {}", header.entity_id)?;
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

    fn from_str(text: &str) -> Result<Self, ParseError> {
        let mut lines = text
            .lines()
            .enumerate()
            .map(|(index, line)| (index + 1, line.trim()))
            .filter(|(_, line)| !line.is_empty() && !line.starts_with('#'));
        let error = |line: usize| move |kind: ParseErrorKind| ParseError { line, kind };
        if lines.next().map(|(_, line)| line) != Some(FORMAT_LINE) {
            return Err(ParseError {
                line: 1,
                kind: ParseErrorKind::Format,
            });
        }
        let mut header = HeaderBuilder::default();
        let mut last = 1;
        for (number, line) in lines.by_ref() {
            last = number;
            if line == "events" {
                break;
            }
            header.line(line).map_err(error(number))?;
        }
        let header = header.finish().map_err(error(last))?;
        let mut events = Vec::new();
        for (number, line) in lines {
            events.push(Event::parse(line).map_err(error(number))?);
        }
        Ok(Self { header, events })
    }
}

#[derive(Default)]
struct HeaderBuilder {
    version: Option<String>,
    experiment: Option<String>,
    step: Option<String>,
    instruction: String,
    entity_id: Option<i32>,
    fills: Vec<Fill>,
}

impl HeaderBuilder {
    fn line(&mut self, line: &str) -> Result<(), ParseErrorKind> {
        let mut fields = Fields::new(line);
        match fields.word("field")? {
            "version" => self.version = Some(fields.word("version")?.to_owned()),
            "experiment" => self.experiment = Some(fields.word("experiment")?.to_owned()),
            "step" => self.step = Some(fields.word("step")?.to_owned()),
            "instruction" => {
                self.instruction = fields.rest_text();
                return Ok(());
            }
            "entity" => self.entity_id = Some(fields.value("entity")?),
            "fill" => self.fills.push(Fill {
                min: fields.array3("min")?,
                max: fields.array3("max")?,
                state: fields.word("state")?.to_owned(),
            }),
            other => return Err(ParseErrorKind::UnknownHeader(other.to_owned())),
        }
        fields.end()
    }

    fn finish(self) -> Result<Header, ParseErrorKind> {
        Ok(Header {
            version: self
                .version
                .ok_or(ParseErrorKind::MissingHeader("version"))?,
            experiment: self
                .experiment
                .ok_or(ParseErrorKind::MissingHeader("experiment"))?,
            step: self.step.ok_or(ParseErrorKind::MissingHeader("step"))?,
            instruction: self.instruction,
            entity_id: self
                .entity_id
                .ok_or(ParseErrorKind::MissingHeader("entity"))?,
            fills: self.fills,
        })
    }
}
