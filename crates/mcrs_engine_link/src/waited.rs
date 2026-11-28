pub(crate) enum Waited<I, O> {
    Input(I),
    Outcome(O),
    Closed,
}
