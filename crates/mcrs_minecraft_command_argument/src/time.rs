use crate::resource::{ResourceArgument, ResourceLocationArgument};
use crate::time_error::TimeError;
use crate::time_lookup::TimeLookup;
use mcrs_command::{Argument, ContextView, NodeId, Reader, SuggestionBuilder, SyntaxError};
use mcrs_minecraft_command::{
    Identifier, Lookup, ResourceError, Source, identifier_matches, text_matches,
};

const TICKS_PER_DAY: f32 = 24_000.0;
const TICKS_PER_SECOND: f32 = 20.0;

/// `minecraft:time`: a tick count with an optional unit `d`, `s` or `t`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeArgument {
    pub min: i32,
}

impl TimeArgument {
    /// Suggests the units after the number.
    pub fn suggest<T>(builder: &mut SuggestionBuilder<'_, '_, T>) {
        let mut reader = Reader::at(builder.input(), builder.start());
        if reader.read_float::<()>().is_err() {
            return;
        }
        builder
            .at(reader.cursor())
            .suggest_matching(["d", "s", "t"], text_matches);
    }
}

/// The product rounded half up, then held in the 32-bit range.
fn round_ticks(product: f32) -> i32 {
    let floor = product.floor();
    let rounded = if product - floor >= 0.5 {
        floor + 1.0
    } else {
        floor
    };
    rounded as i32
}

impl<S> Argument<S> for TimeArgument {
    type Value<'a> = i32;
    type Error = TimeError;

    fn parse<'a>(&self, reader: &mut Reader<'a>, _: &S) -> Result<i32, SyntaxError<TimeError>> {
        let value: f32 = reader.read_float()?;
        let unit = reader.read_unquoted();
        let per_unit = match unit.text(reader.input()) {
            "d" => TICKS_PER_DAY,
            "s" => TICKS_PER_SECOND,
            "t" | "" => 1.0,
            _ => {
                return Err(SyntaxError::argument(
                    TimeError::TimeInvalidUnit,
                    reader.cursor(),
                ));
            }
        };
        let ticks = round_ticks(value * per_unit);
        if ticks < self.min {
            return Err(SyntaxError::argument(
                TimeError::TickCountTooLow {
                    min: self.min,
                    ticks,
                },
                reader.cursor(),
            ));
        }
        Ok(ticks)
    }
}

/// The clock that a parsed context selects.
enum SelectedClock<'i> {
    /// The branch has no clock node, so the default clock of the source dimension.
    Default,
    /// The value of the clock node.
    Named(Identifier<'i>),
    /// The context lacks the clock node or holds no identifier there.
    Unread,
}

impl<'i> SelectedClock<'i> {
    fn read(input: &'i str, context: &ContextView<'_>, clock: Option<NodeId>) -> Self {
        let Some(clock) = clock else {
            return SelectedClock::Default;
        };
        context
            .nodes()
            .find(|&(node, _)| node == clock)
            .and_then(|(_, span)| Identifier::parse(span.text(input)))
            .map_or(SelectedClock::Unread, SelectedClock::Named)
    }
}

/// A time marker of the selected clock: read as `minecraft:resource_location`, suggested by
/// the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeMarkerArgument {
    /// The node of `<clock>` the branch reads, or `None` for the default clock.
    pub clock: Option<NodeId>,
}

impl TimeMarkerArgument {
    /// Suggests the time markers of the selected clock.
    pub fn suggest<'t, L: TimeLookup, T>(
        &self,
        source: &'t Source<'_, L>,
        context: &ContextView<'_>,
        builder: &mut SuggestionBuilder<'t, '_, T>,
    ) {
        let clock = match SelectedClock::read(builder.input(), context, self.clock) {
            SelectedClock::Default => None,
            SelectedClock::Named(clock) => Some(clock),
            SelectedClock::Unread => return,
        };
        let markers = source.lookup.suggested_time_markers(clock);
        builder.suggest_matching(markers, identifier_matches);
    }
}

impl<S> Argument<S> for TimeMarkerArgument {
    type Value<'a> = Identifier<'a>;
    type Error = ResourceError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        source: &S,
    ) -> Result<Identifier<'a>, SyntaxError<ResourceError>> {
        ResourceLocationArgument.parse(reader, source)
    }
}

/// A timeline: read as `minecraft:resource` of the timeline registry, and suggested by the
/// server from the timelines of the selected clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimelineArgument {
    /// The node of `<clock>` the branch reads, or `None` for the default clock.
    pub clock: Option<NodeId>,
}

impl TimelineArgument {
    pub const RESOURCE: ResourceArgument = ResourceArgument {
        registry: mcrs_minecraft_core::rl!("minecraft:timeline"),
    };

    /// Suggests the timelines of the selected clock.
    pub fn suggest<'t, L: TimeLookup, T>(
        &self,
        source: &'t Source<'_, L>,
        context: &ContextView<'_>,
        builder: &mut SuggestionBuilder<'t, '_, T>,
    ) {
        let clock = match SelectedClock::read(builder.input(), context, self.clock) {
            SelectedClock::Default => None,
            SelectedClock::Named(clock) => Some(clock),
            SelectedClock::Unread => return,
        };
        let timelines = source.lookup.timelines_of_clock(clock);
        builder.suggest_matching(timelines, identifier_matches);
    }
}

impl<L: Lookup> Argument<Source<'_, L>> for TimelineArgument {
    type Value<'a> = Identifier<'a>;
    type Error = ResourceError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        source: &Source<'_, L>,
    ) -> Result<Identifier<'a>, SyntaxError<ResourceError>> {
        Self::RESOURCE.parse(reader, source)
    }
}
