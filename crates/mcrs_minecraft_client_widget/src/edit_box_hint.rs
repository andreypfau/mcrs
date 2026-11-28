#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditBoxHint<'a> {
    pub text: &'a str,
    pub color: u32,
}

impl EditBoxHint<'_> {
    pub const DEFAULT_COLOR: u32 = 0xFF55_5555;
}
