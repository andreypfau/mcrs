/// What a far end on a thread sends to the near end.
pub(crate) enum Reply<O> {
    Output(O),
    /// The far end owes no output for the first `seen` inputs it was sent.
    Answered {
        seen: u64,
    },
}
