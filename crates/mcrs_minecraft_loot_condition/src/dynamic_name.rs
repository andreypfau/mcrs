use mcrs_minecraft_resource::ResourceLocation;

/// A name of a `dynamic` entry that a vanilla source gives stacks for (LOOT-EVL-007).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DynamicName {
    /// `minecraft:sherds`: the decorations of a decorated pot.
    Sherds,
    /// `minecraft:contents`: the slots of a shulker box.
    Contents,
}

impl DynamicName {
    /// `None` for a name that no vanilla source gives.
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
