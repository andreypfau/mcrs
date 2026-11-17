use bevy::prelude::Message;
use mcrs_minecraft_client_widget::WidgetEvent;

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuEvent {
    Widget(WidgetEvent),
    CloseRequested,
    Confirmed,
}
