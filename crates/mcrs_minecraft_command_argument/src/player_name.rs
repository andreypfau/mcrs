use core::convert::Infallible;
use mcrs_command::{
    Argument, Reader, StringArgument, StringValue, SuggestionBuilder, SyntaxError,
    starts_with_ignore_case,
};
use mcrs_minecraft_command::{Lookup, Source};

/// The name of a player that may not be online: read as one word, and suggested by the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerNameArgument {
    Online,
    Banned,
}

impl PlayerNameArgument {
    pub fn suggest<'t, L: Lookup, T>(
        &self,
        source: &'t Source<'_, L>,
        builder: &mut SuggestionBuilder<'t, '_, T>,
    ) {
        match self {
            Self::Online => {
                builder.suggest_matching(source.lookup.player_names(), starts_with_ignore_case);
            }
            Self::Banned => {
                builder.suggest_matching(source.lookup.banned_names(), starts_with_ignore_case);
            }
        }
    }
}

impl<S> Argument<S> for PlayerNameArgument {
    type Value<'a> = StringValue<'a>;
    type Error = Infallible;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        source: &S,
    ) -> Result<StringValue<'a>, SyntaxError<Infallible>> {
        StringArgument::Word.parse(reader, source)
    }
}
