use mcrs_minecraft_protocol::setting::ChatMode;

/// Whether the launcher and the account let the player chat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChatAccess {
    #[default]
    Allowed,
    Forbidden,
}

/// What the player can send and receive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChatAbilities {
    pub send_messages: bool,
    pub send_commands: bool,
    pub receive_player_messages: bool,
    pub receive_system_messages: bool,
}

impl ChatAbilities {
    pub fn new(mode: ChatMode, access: ChatAccess) -> Self {
        let (messages, commands) = match mode {
            ChatMode::Enabled => (true, true),
            ChatMode::CommandsOnly => (false, true),
            ChatMode::Hidden => (false, false),
        };
        let chat_allowed = access == ChatAccess::Allowed;
        Self {
            send_messages: messages && chat_allowed,
            send_commands: commands,
            receive_player_messages: messages && chat_allowed,
            receive_system_messages: commands,
        }
    }
}

impl Default for ChatAbilities {
    fn default() -> Self {
        Self::new(ChatMode::Enabled, ChatAccess::Allowed)
    }
}
