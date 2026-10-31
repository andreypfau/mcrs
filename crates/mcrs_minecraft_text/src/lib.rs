#![cfg_attr(
    test,
    expect(
        clippy::disallowed_macros,
        clippy::disallowed_types,
        reason = "test input is written as JSON"
    )
)]

mod chat_character;
mod click_event;
pub mod color;
mod data_source;
mod dialog_ref;
mod discriminator;
mod hover_event;
mod into_text;
mod object_info;
mod scoreboard_value_content;
mod style;
mod tagged_compound;
mod text;
mod text_content;
mod text_inner;
mod text_types;
mod translate_arg;
mod wire;

pub use chat_character::is_allowed_chat_character;
pub use click_event::ClickEvent;
pub use color::Color;
pub use data_source::DataSource;
pub use dialog_ref::DialogRef;
pub use hover_event::HoverEvent;
pub use into_text::IntoText;
pub use object_info::ObjectInfo;
pub use scoreboard_value_content::ScoreboardValueContent;
pub use style::Style;
pub use text::Text;
pub use text_content::TextContent;
pub use text_inner::TextInner;
pub use text_types::{Serde, TextMap, TextSerde, TextTypes, Value};
pub use translate_arg::TranslateArg;
