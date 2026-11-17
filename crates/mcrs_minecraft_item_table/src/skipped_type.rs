use mcrs_minecraft_resource::ResourceLocationBuf;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SkippedType {
    pub component: ResourceLocationBuf,
    pub prototypes: usize,
}
