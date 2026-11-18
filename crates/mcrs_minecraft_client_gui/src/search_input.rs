use bevy::input::keyboard::KeyboardInput;
use mcrs_minecraft_client_widget::KeyModifiers;

#[derive(Clone, Debug)]
pub(crate) enum SearchInput {
    Key {
        input: KeyboardInput,
        modifiers: KeyModifiers,
    },
    SelectAll,
}
