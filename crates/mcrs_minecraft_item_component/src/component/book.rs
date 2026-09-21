use std::ops::Not;

use mcrs_minecraft_core::codec::{self, is_default};
use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, INT_ID, LIST_ID, STRING_ID};
use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer, Serialize};

use crate::component::common::{BoundedString, Filterable};
use crate::harness::Sample;
use mcrs_minecraft_core::Bounded;
use mcrs_minecraft_text::IntoText;

use crate::Text;

/// The length check the transparent `Bounded` serde lacks.
pub fn size_limited<'de, D: Deserializer<'de>, T: DeserializeOwned, const MAX: usize>(
    d: D,
) -> Result<Bounded<Vec<T>, MAX>, D::Error> {
    let items = Vec::<T>::deserialize(d)?;
    if items.len() > MAX {
        return Err(D::Error::custom(format_args!(
            "List is too long: {}, expected range [0-{MAX}]",
            items.len()
        )));
    }
    Ok(Bounded(items))
}

fn no_pages<T, const MAX: usize>(pages: &Bounded<Vec<T>, MAX>) -> bool {
    pages.0.is_empty()
}

pub const MAX_WRITABLE_PAGES: usize = 100;
pub const WRITABLE_PAGE_CHARS: usize = 1024;

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WritableBookContent {
    #[serde(
        default,
        deserialize_with = "size_limited",
        skip_serializing_if = "no_pages"
    )]
    pub pages: Bounded<Vec<Filterable<BoundedString<WRITABLE_PAGE_CHARS>>>, MAX_WRITABLE_PAGES>,
}

impl Sample for WritableBookContent {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        if !self.pages.0.is_empty() {
            tags.push(("pages", LIST_ID));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            WritableBookContent::default(),
            WritableBookContent {
                pages: Bounded(vec![
                    Filterable::pass_through(BoundedString("hello".into())),
                    Filterable {
                        raw: BoundedString("raw".into()),
                        filtered: Some(BoundedString("filtered".into())),
                    },
                ]),
            },
        ]
    }
}

pub const TITLE_CHARS: usize = 32;
pub const WRITTEN_PAGE_CHARS: usize = 32767;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WrittenBookContent {
    pub title: Filterable<BoundedString<TITLE_CHARS>>,
    pub author: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub generation: codec::Bounded<0, 3, 0>,
    #[serde(
        default,
        deserialize_with = "restricted_pages",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub pages: Vec<Filterable<Text>>,
    #[serde(default, skip_serializing_if = "Not::not")]
    pub resolved: bool,
}

/// A page is measured by the length of its flat JSON text in UTF-16 units, as
/// Gson's `JsonWriter` emits it, which also escapes U+2028 and U+2029 as six
/// characters.
fn restricted_pages<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Filterable<Text>>, D::Error> {
    let pages = Vec::<Filterable<Text>>::deserialize(d)?;
    for page in &pages {
        for text in std::iter::once(&page.raw).chain(page.filtered.iter()) {
            let json = serde_json::to_string(text).map_err(D::Error::custom)?;
            let escaped = json
                .chars()
                .filter(|c| matches!(c, '\u{2028}' | '\u{2029}'))
                .count();
            if json.encode_utf16().count() + 5 * escaped > WRITTEN_PAGE_CHARS {
                return Err(D::Error::custom(format_args!(
                    "Component was too large: greater than max size {WRITTEN_PAGE_CHARS}"
                )));
            }
        }
    }
    Ok(pages)
}

impl Sample for WrittenBookContent {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![
            ("", COMPOUND_ID),
            ("title", COMPOUND_ID),
            ("title.raw", STRING_ID),
            ("author", STRING_ID),
        ];
        if !self.pages.is_empty() {
            tags.extend([
                ("generation", INT_ID),
                ("pages", LIST_ID),
                ("resolved", BYTE_ID),
            ]);
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            WrittenBookContent {
                title: Filterable::pass_through(BoundedString("T".into())),
                author: "me".into(),
                generation: codec::Bounded(0),
                pages: Vec::new(),
                resolved: false,
            },
            WrittenBookContent {
                title: Filterable {
                    raw: BoundedString("Title".into()),
                    filtered: Some(BoundedString("Filt".into())),
                },
                author: "author".into(),
                generation: codec::Bounded(2),
                pages: vec![
                    Filterable::pass_through(Text::text("page")),
                    Filterable {
                        raw: "a".bold(),
                        filtered: Some(Text::text("b")),
                    },
                ],
                resolved: true,
            },
        ]
    }
}
