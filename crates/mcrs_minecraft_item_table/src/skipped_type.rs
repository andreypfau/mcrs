use mcrs_minecraft_resource::ResourceLocationBuf;

/// A component type of the item definitions that has no Rust type, and the number of prototypes
/// whose component of that type the bake left out.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SkippedType {
    pub component: ResourceLocationBuf,
    pub prototypes: usize,
}
