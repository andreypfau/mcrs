use std::fmt;
use std::str::FromStr;

use crate::Event;
use crate::FORMAT_LINE;
use crate::ParseError;
use crate::fields::Fields;
use crate::fields::parse_value;

#[derive(Clone, Debug, PartialEq)]
pub struct Recording {
    pub header: Header,
    pub events: Vec<Event>,
    pub times: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    pub version: String,
    pub experiment: String,
    pub step: String,
    pub instruction: String,
    pub fills: Vec<Fill>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fill {
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub state: String,
}

impl Recording {
    pub fn ticks(&self) -> impl Iterator<Item = &[Event]> {
        self.events
            .split_inclusive(|event| *event == Event::TickEnd)
            .filter(|tick| tick.last() == Some(&Event::TickEnd))
    }
}

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
        for (time, event) in self.times.iter().zip(&self.events) {
            writeln!(f, "{time} {event}")?;
        }
        Ok(())
    }
}

impl FromStr for Recording {
    type Err = ParseError;

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
        let mut events = Vec::new();
        let mut times = Vec::new();
        for (index, text) in numbered(start + 1..lines.len()) {
            let (time, event) = text.split_once(' ').unwrap_or_default();
            times.push(parse_value("time", time).map_err(error(index + 1))?);
            events.push(Event::parse(event).map_err(error(index + 1))?);
        }
        Ok(Self {
            header: Header {
                version,
                experiment,
                step,
                instruction,
                fills,
            },
            events,
            times,
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
