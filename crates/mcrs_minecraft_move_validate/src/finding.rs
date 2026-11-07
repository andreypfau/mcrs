/// A failed check that does not end the connection. A layer reports it to the layer above.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Finding {
    /// The absolute pitch is more than 90 degrees; the server clamps it.
    PitchOutOfRange,
    /// A coordinate is past the range that the server clamps it to.
    PositionOutOfRange,
    /// A move packet sets a bit of its flags other than the on-ground and collision bits.
    UnknownMoveFlags,
    /// `player_input` sets its high bit.
    UnknownInputFlag,
    /// `player_abilities` sets a bit other than the request to fly.
    UnknownAbilityFlags,
    /// A second move packet in one client tick; the server ignores it.
    SecondMove,
    /// The 20th client tick or a later one without a position packet.
    SilentTicks,
    /// More client ticks than the time after the newest answered probe allows.
    TickRate,
    /// An answer whose id is not the id of a probe without an answer; the server ignores it.
    UnknownAnswer,
    /// An answer to a probe that is not the oldest one without an answer.
    SkippedAnswer,
}
