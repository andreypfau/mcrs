use mcrs_minecraft_protocol::setting::ChatMode;

/// What the player can send and receive (CHAT-OPT-002).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChatAbilities {
    pub send_messages: bool,
    pub send_commands: bool,
    pub receive_player_messages: bool,
    pub receive_system_messages: bool,
}

impl ChatAbilities {
    /// `chat_allowed` is false when the launcher or the account forbids chat.
    pub fn new(mode: ChatMode, chat_allowed: bool) -> Self {
        let (messages, commands) = match mode {
            ChatMode::Enabled => (true, true),
            ChatMode::CommandsOnly => (false, true),
            ChatMode::Hidden => (false, false),
        };
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
        Self::new(ChatMode::Enabled, true)
    }
}
