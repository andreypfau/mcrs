#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ForkMode {
    #[default]
    Single,
    Fork,
}
