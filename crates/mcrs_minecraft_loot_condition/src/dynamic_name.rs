use mcrs_minecraft_resource::ResourceLocation;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DynamicName {
    Sherds,
    Contents,
}

impl DynamicName {
    #[must_use]
    pub fn from_location(name: ResourceLocation<'_>) -> Option<Self> {
        if name.namespace() != "minecraft" {
            return None;
        }
        match name.path() {
            "sherds" => Some(Self::Sherds),
            "contents" => Some(Self::Contents),
            _ => None,
        }
    }
}
