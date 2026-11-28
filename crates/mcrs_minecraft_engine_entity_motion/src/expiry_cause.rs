#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpiryCause {
    Lifetime,
    Death,
}
