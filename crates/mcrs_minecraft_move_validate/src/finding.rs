#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Finding {
    PitchOutOfRange,
    PositionOutOfRange,
    UnknownMoveFlags,
    UnknownInputFlag,
    UnknownAbilityFlags,
    SecondMove,
    SilentTicks,
    TickRate,
    UnknownAnswer,
}
