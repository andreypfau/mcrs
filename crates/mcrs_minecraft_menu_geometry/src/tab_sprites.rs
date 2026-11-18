use mcrs_minecraft_resource::ResourceLocation;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TabSprites {
    pub selected: ResourceLocation<'static>,
    pub unselected: ResourceLocation<'static>,
}
