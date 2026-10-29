/// How a modifier maps sources.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ForkMode {
    /// The modifier maps one source to one source, and its error fails the run.
    #[default]
    Single,
    /// The modifier maps one source to any number of sources, and an error of one source
    /// drops only that source.
    Fork,
}
