/// A character a player may send in chat or in a command line.
pub const fn is_allowed_chat_character(c: char) -> bool {
    !matches!(c, '\0'..='\u{1f}' | '\u{7f}' | '§')
}
