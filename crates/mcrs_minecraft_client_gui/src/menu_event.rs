use bevy::prelude::Message;
use mcrs_minecraft_client_widget::WidgetEvent;

/// What happened in the open menu, in the order of the input.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuEvent {
    Widget(WidgetEvent),
    /// Escape, which no widget takes; the screen decides what closing means (WID-SCR-002,
    /// WID-FOC-003).
    CloseRequested,
}
