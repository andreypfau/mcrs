use mcrs_minecraft_client_widget::GuiPoint;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntryCorner {
    pub index: u16,
    pub corner: GuiPoint,
}
