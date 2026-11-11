use crate::ComponentType;

/// One value entry of a hashed stack: the type of a component and the hash of its saved form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HashedComponent {
    pub component: ComponentType,
    pub hash: i32,
}
