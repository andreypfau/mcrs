use mcrs_minecraft_core::codec::BoundedString;

use crate::ProfileError;

pub type PlayerName = BoundedString<16>;

pub(crate) fn player_name(text: &str) -> Result<PlayerName, String> {
    check_player_name(text).map_err(|error| error.to_string())?;
    Ok(BoundedString(text.to_owned()))
}

pub(crate) fn check_player_name(text: &str) -> Result<(), ProfileError<'_>> {
    BoundedString::<16>::check(text)?;
    if text.chars().any(|c| c <= ' ' || c >= '\x7f') {
        return Err(ProfileError::DisallowedNameCharacters(text));
    }
    Ok(())
}
