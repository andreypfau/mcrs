#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece<'a> {
    Literal(&'a str),
    Argument(usize),
}
