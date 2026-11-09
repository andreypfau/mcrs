use crate::json::{Json, JsonObject};

/// The scale fields of `scripts`, in the order of [`Scripts::scale`].
pub const SCALE_FIELDS: [&str; 4] = ["scale", "scalex", "scaley", "scalez"];

/// An element of the `animate` list.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimateItem {
    /// In lower case.
    pub short_name: String,
    /// `None` for the weight `1`.
    pub weight: Option<Json>,
}

/// The fields of `scripts` that the animation host reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scripts {
    pub initialize: Vec<String>,
    pub pre_animation: Vec<String>,
    pub animate: Vec<AnimateItem>,
    /// The fields of [`SCALE_FIELDS`].
    pub scale: [Option<Json>; 4],
}

impl Scripts {
    pub(crate) fn read(object: &JsonObject) -> Self {
        let field = |name: &str| {
            object
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value)
        };
        let strings = |name: &str| -> Vec<String> {
            match field(name) {
                Some(Json::Array(entries)) => entries
                    .iter()
                    .filter_map(Json::as_str)
                    .map(String::from)
                    .collect(),
                Some(Json::String(text)) => vec![text.clone()],
                _ => Vec::new(),
            }
        };
        let animate = field("animate")
            .and_then(Json::as_array)
            .unwrap_or_default()
            .iter()
            .filter_map(|element| match element {
                Json::String(short_name) => Some(AnimateItem {
                    short_name: short_name.to_ascii_lowercase(),
                    weight: None,
                }),
                Json::Object(object) => {
                    let (short_name, weight) = object.iter().next()?;
                    Some(AnimateItem {
                        short_name: short_name.to_ascii_lowercase(),
                        weight: Some(weight.clone()),
                    })
                }
                _ => None,
            })
            .collect();
        Self {
            initialize: strings("initialize"),
            pre_animation: strings("pre_animation"),
            animate,
            scale: SCALE_FIELDS.map(|name| field(name).cloned()),
        }
    }
}
