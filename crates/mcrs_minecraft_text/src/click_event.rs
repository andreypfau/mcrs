use std::borrow::Cow;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::int_value;
use mcrs_minecraft_nbt::tag::NbtTag;
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::tagged_compound::{tagged_compound, variant};
use crate::{TextMap, TextSerde, TextTypes};

/// Action to take on click of the text.
#[derive(Clone, PartialEq, Debug, Serialize)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    bound(serialize = "I: TextSerde")
)]
pub enum ClickEvent<I: TextTypes = ()> {
    OpenUrl {
        url: Cow<'static, str>,
    },
    /// Only the client may create one; vanilla refuses to encode or decode it.
    #[serde(skip)]
    OpenFile {
        path: Cow<'static, str>,
    },
    RunCommand {
        command: Cow<'static, str>,
    },
    SuggestCommand {
        command: Cow<'static, str>,
    },
    ShowDialog {
        dialog: I::Dialog,
    },
    ChangePage {
        page: i32,
    },
    CopyToClipboard {
        value: Cow<'static, str>,
    },
    Custom {
        id: ResourceLocation,
        #[serde(skip_serializing_if = "Option::is_none")]
        payload: Option<NbtTag>,
    },
}

impl<'de, I: TextSerde> Deserialize<'de> for ClickEvent<I> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Url {
            #[serde(deserialize_with = "untrusted_uri")]
            url: String,
        }

        #[derive(Deserialize)]
        struct Command {
            #[serde(deserialize_with = "chat_string")]
            command: String,
        }

        #[derive(Deserialize)]
        #[serde(bound = "I: TextSerde")]
        struct Dialog<I: TextTypes> {
            dialog: I::Dialog,
        }

        #[derive(Deserialize)]
        struct Page {
            #[serde(deserialize_with = "positive_int")]
            page: i32,
        }

        #[derive(Deserialize)]
        struct Value {
            value: String,
        }

        #[derive(Deserialize)]
        struct Custom {
            id: ResourceLocation,
            #[serde(default)]
            payload: Option<NbtTag>,
        }

        let (action, compound) = tagged_compound(d, "action")?;
        Ok(match action.as_str() {
            "open_url" => ClickEvent::OpenUrl {
                url: variant::<D, Url>(compound)?.url.into(),
            },
            "run_command" => ClickEvent::RunCommand {
                command: variant::<D, Command>(compound)?.command.into(),
            },
            "suggest_command" => ClickEvent::SuggestCommand {
                command: variant::<D, Command>(compound)?.command.into(),
            },
            "show_dialog" => ClickEvent::ShowDialog {
                dialog: variant::<D, Dialog<I>>(compound)?.dialog,
            },
            "change_page" => ClickEvent::ChangePage {
                page: variant::<D, Page>(compound)?.page,
            },
            "copy_to_clipboard" => ClickEvent::CopyToClipboard {
                value: variant::<D, Value>(compound)?.value.into(),
            },
            "custom" => {
                let Custom { id, payload } = variant::<D, Custom>(compound)?;
                ClickEvent::Custom { id, payload }
            }
            other => {
                return Err(de::Error::unknown_variant(
                    other,
                    &[
                        "open_url",
                        "run_command",
                        "suggest_command",
                        "show_dialog",
                        "change_page",
                        "copy_to_clipboard",
                        "custom",
                    ],
                ));
            }
        })
    }
}

/// Any number is accepted and truncated to an int before the bound check.
fn positive_int<'de, D: Deserializer<'de>>(d: D) -> Result<i32, D::Error> {
    let value = int_value(d)?;
    if value < 1 {
        return Err(de::Error::custom(format_args!(
            "Value must be positive: {value}"
        )));
    }
    Ok(value)
}

fn chat_string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let value = String::deserialize(d)?;
    if let Some(c) = value
        .chars()
        .find(|&c| !crate::is_allowed_chat_character(c))
    {
        return Err(de::Error::custom(format_args!(
            "Disallowed chat character: '{c}'"
        )));
    }
    Ok(value)
}

/// A URI whose scheme is http or https, kept as written.
fn untrusted_uri<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let value = String::deserialize(d)?;
    let scheme = java_uri::scheme(&value).map_err(de::Error::custom)?;
    let Some(scheme) = scheme else {
        return Err(de::Error::custom(format_args!(
            "Missing protocol in URI: {value}: {value}"
        )));
    };
    if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") {
        return Err(de::Error::custom(format_args!(
            "Unsupported protocol in URI: {value}: {value}"
        )));
    }
    Ok(value)
}

/// The `java.net.URI` parser (RFC 2396 with the RFC 2732 brackets), as far
/// as deciding whether a string is a URI and what its scheme is.
mod java_uri {
    fn unreserved(c: char) -> bool {
        c.is_ascii_alphanumeric() || "-_.!~*'()".contains(c)
    }

    fn uric(c: char) -> bool {
        unreserved(c) || ";/?:@&=+$,[]".contains(c)
    }

    fn pchar(c: char) -> bool {
        unreserved(c) || ":@&=+$,".contains(c)
    }

    fn path(c: char) -> bool {
        pchar(c) || c == '/' || c == ';'
    }

    fn authority(c: char) -> bool {
        unreserved(c) || "$,;:@&=+[]".contains(c)
    }

    fn other(c: char) -> bool {
        (c as u32) > 128 && !c.is_whitespace() && !c.is_control()
    }

    struct Parser<'a> {
        input: &'a str,
        chars: Vec<char>,
    }

    impl Parser<'_> {
        fn fail(&self, reason: &str, at: usize) -> String {
            format!("{reason} at index {at}: {}", self.input)
        }

        fn until(&self, mut p: usize, stop: &str) -> usize {
            while self.chars.get(p).is_some_and(|&c| !stop.contains(c)) {
                p += 1;
            }
            p
        }

        fn check(
            &self,
            start: usize,
            end: usize,
            allowed: fn(char) -> bool,
            what: &str,
        ) -> Result<(), String> {
            let mut p = start;
            while p < end {
                let Some(&c) = self.chars.get(p) else {
                    break;
                };
                if allowed(c) || other(c) {
                    p += 1;
                } else if c == '%' {
                    let hex = |i: usize| self.chars.get(i).is_some_and(char::is_ascii_hexdigit);
                    if p + 2 >= end || !hex(p + 1) || !hex(p + 2) {
                        return Err(self.fail("Malformed escape pair", p));
                    }
                    p += 3;
                } else {
                    return Err(self.fail(&format!("Illegal character in {what}"), p));
                }
            }
            Ok(())
        }

        fn hierarchical(&self, mut p: usize) -> Result<usize, String> {
            let n = self.chars.len();
            if self.chars.get(p) == Some(&'/') && self.chars.get(p + 1) == Some(&'/') {
                p += 2;
                let q = self.until(p, "/?#");
                if q > p {
                    self.check(p, q, authority, "authority")?;
                    p = q;
                } else if q >= n {
                    return Err(self.fail("Expected authority", p));
                }
            }
            let q = self.until(p, "?#");
            self.check(p, q, path, "path")?;
            p = q;
            if self.chars.get(p) == Some(&'?') {
                p += 1;
                let q = self.until(p, "#");
                self.check(p, q, uric, "query")?;
                p = q;
            }
            Ok(p)
        }
    }

    pub(super) fn scheme(input: &str) -> Result<Option<String>, String> {
        let parser = Parser {
            input,
            chars: input.chars().collect(),
        };
        let chars = &parser.chars;
        let n = chars.len();
        let colon = parser.until(0, ":/?#");
        let (scheme, mut p) = if chars.get(colon) == Some(&':') {
            if colon == 0 {
                return Err(parser.fail("Expected scheme name", 0));
            }
            let name = chars.get(..colon).unwrap_or_default();
            if !name.first().is_some_and(char::is_ascii_alphabetic) {
                return Err(parser.fail("Illegal character in scheme name", 0));
            }
            if let Some(bad) = name
                .iter()
                .position(|&c| !(c.is_ascii_alphanumeric() || "+-.".contains(c)))
            {
                return Err(parser.fail("Illegal character in scheme name", bad));
            }
            let p = colon + 1;
            let end = if chars.get(p) == Some(&'/') {
                parser.hierarchical(p)?
            } else {
                let q = parser.until(p, "#");
                if q <= p {
                    return Err(parser.fail("Expected scheme-specific part", p));
                }
                parser.check(p, q, uric, "opaque part")?;
                q
            };
            (Some(name.iter().collect()), end)
        } else {
            (None, parser.hierarchical(0)?)
        };
        if chars.get(p) == Some(&'#') {
            parser.check(p + 1, n, uric, "fragment")?;
            p = n;
        }
        if p < n {
            return Err(parser.fail("end of URI", p));
        }
        Ok(scheme)
    }
}

impl<A: TextTypes> ClickEvent<A> {
    pub fn map<B: TextTypes, T: TextMap<A, B>>(&self, map: &T) -> Result<ClickEvent<B>, T::Error> {
        Ok(match self {
            ClickEvent::OpenUrl { url } => ClickEvent::OpenUrl { url: url.clone() },
            ClickEvent::OpenFile { path } => ClickEvent::OpenFile { path: path.clone() },
            ClickEvent::RunCommand { command } => ClickEvent::RunCommand {
                command: command.clone(),
            },
            ClickEvent::SuggestCommand { command } => ClickEvent::SuggestCommand {
                command: command.clone(),
            },
            ClickEvent::ShowDialog { dialog } => ClickEvent::ShowDialog {
                dialog: map.dialog(dialog).map_err(|e| T::at(e, "dialog"))?,
            },
            ClickEvent::ChangePage { page } => ClickEvent::ChangePage { page: *page },
            ClickEvent::CopyToClipboard { value } => ClickEvent::CopyToClipboard {
                value: value.clone(),
            },
            ClickEvent::Custom { id, payload } => ClickEvent::Custom {
                id: id.clone(),
                payload: payload.clone(),
            },
        })
    }
}
