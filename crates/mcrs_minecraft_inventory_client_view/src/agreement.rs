/// Whether the stack of the server matches the client view of an entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Agreement {
    #[default]
    Matches,
    Differs,
}
