use crate::error::TimeError;
use crate::time_lookup::TimeLookup;
use mcrs_command::{Argument, ContextView, NodeId, Reader, SuggestionBuilder, SyntaxError};
use mcrs_minecraft_command::{
    Identifier, Lookup, ResourceArgument, ResourceError, ResourceLocationArgument, Source,
    identifier_matches, text_matches,
};
use mcrs_minecraft_environment::keys::TIMELINE;

const TICKS_PER_DAY: f32 = 24_000.0;
const TICKS_PER_SECOND: f32 = 20.0;

/// `minecraft:time` (CMD-TIME-003): a tick count with an optional unit `d`, `s` or `t`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeArgument {
    pub min: i32,
}

impl TimeArgument {
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

/// The clock that a parsed context selects: the value of the node `clock`, or the default
/// clock of the source dimension when the branch has no such node. `None` when the context
/// lacks the node.
fn selected_clock<'i>(
    input: &'i str,
    context: &ContextView<'_>,
    clock: Option<NodeId>,
) -> Option<Option<Identifier<'i>>> {
    let Some(clock) = clock else {
        return Some(None);
    };
    let (_, span) = context.nodes().find(|&(node, _)| node == clock)?;
    Identifier::parse(span.text(input)).map(Some)
}

/// A time marker of the selected clock: read as `minecraft:resource_location`, suggested by
/// the server (CMD-TIME-011).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeMarkerArgument {
    /// The node of `<clock>` the branch reads, or `None` for the default clock.
    pub clock: Option<NodeId>,
}

impl TimeMarkerArgument {
    pub fn suggest<'t, L: TimeLookup, T>(
        &self,
        source: &'t Source<'_, L>,
        context: &ContextView<'_>,
        builder: &mut SuggestionBuilder<'t, '_, T>,
    ) {
        if let Some(clock) = selected_clock(builder.input(), context, self.clock) {
            let markers = source.lookup.suggested_time_markers(clock);
            builder.suggest_matching(markers, identifier_matches);
        }
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
/// server from the timelines of the selected clock (CMD-TIME-016).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimelineArgument {
    /// The node of `<clock>` the branch reads, or `None` for the default clock.
    pub clock: Option<NodeId>,
}

impl TimelineArgument {
    pub const RESOURCE: ResourceArgument = ResourceArgument {
        registry: TIMELINE.location(),
    };

    pub fn suggest<'t, L: TimeLookup, T>(
        &self,
        source: &'t Source<'_, L>,
        context: &ContextView<'_>,
        builder: &mut SuggestionBuilder<'t, '_, T>,
    ) {
        if let Some(clock) = selected_clock(builder.input(), context, self.clock) {
            let timelines = source.lookup.timelines_of_clock(clock);
            builder.suggest_matching(timelines, identifier_matches);
        }
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
