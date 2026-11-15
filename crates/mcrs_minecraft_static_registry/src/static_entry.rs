use mcrs_minecraft_resource::{Id, Registered, ResourceLocation};

pub trait StaticEntry: Registered + Copy {
    fn id(self) -> Id<Self>;

    fn location(self) -> ResourceLocation<'static>;
}
