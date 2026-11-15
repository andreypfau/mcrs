use mcrs_minecraft_predicate::NoWorld;

/// The scoreboard of the server, as the provider `score` reads it (LOOT-NUM-019). `E` is the
/// entity handle of the host.
pub trait ScoreView<E> {
    /// The score of `holder` in the objective with the name `objective`; `None` when the
    /// scoreboard has no such objective or the holder has no score in it (LOOT-CND-023).
    // chisle: the host resolves a name on each read; a handle that the host resolves one time lifts it.
    fn score(&self, holder: ScoreHolder<'_, E>, objective: &str) -> Option<i32>;
}

/// The score holder of a `score` provider (LOOT-CND-023).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreHolder<'a, E> {
    /// The entity of an entity target; the host takes its score holder name.
    Entity(&'a E),
    /// The `name` of the scoreboard name provider `fixed`.
    Name(&'a str),
}

impl<E> ScoreView<E> for NoWorld {
    fn score(&self, _: ScoreHolder<'_, E>, _: &str) -> Option<i32> {
        None
    }
}
