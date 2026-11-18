#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ItemAvailability {
    #[default]
    Enabled,
    Disabled,
}
