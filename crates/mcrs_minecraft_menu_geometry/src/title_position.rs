use mcrs_minecraft_client_widget::GuiPoint;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TitlePosition {
    Left(GuiPoint),
    Centred { y: i32 },
}
