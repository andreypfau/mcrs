use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClick;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClose;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPlayerAction;
use mcrs_minecraft_protocol_game::serverbound::ServerboundSetCreativeModeSlot;

#[derive(Clone, Debug)]
pub(crate) enum Outgoing {
    Click(ServerboundContainerClick),
    Close(ServerboundContainerClose),
    Action(ServerboundPlayerAction),
    CreativeSlot(ServerboundSetCreativeModeSlot),
}
