pub(crate) enum Reply<O> {
    Output(O),
    Answered { seen: u64 },
}
