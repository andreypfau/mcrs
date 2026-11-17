use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClick;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClose;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Outgoing {
    Click(ServerboundContainerClick),
    Close(ServerboundContainerClose),
}
