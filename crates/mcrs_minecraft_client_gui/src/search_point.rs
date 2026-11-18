use mcrs_minecraft_client_widget::GuiRect;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SearchPoint {
    pub(crate) rect: GuiRect,
    pub(crate) x: f64,
}
