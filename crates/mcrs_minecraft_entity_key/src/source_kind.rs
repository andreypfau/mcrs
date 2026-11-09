/// The kind of the source of a birth, in the order of the comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SourceKind {
    Load,
    Player,
    Command,
    Spawn,
    Block,
    Entity,
}
