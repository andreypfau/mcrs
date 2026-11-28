use crate::ComponentType;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HashedComponent {
    pub component: ComponentType,
    pub hash: i32,
}
