use alloc::string::String;

use mcrs_minecraft_resource::ResourceLocation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchQuery {
    rule: Rule,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Rule {
    All,
    Name(Pattern),
    Tag(Pattern),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Pattern {
    Anywhere(String),
    Split(SplitPattern),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SplitPattern {
    namespace: String,
    path: String,
}

impl SearchQuery {
    #[must_use]
    pub fn new(text: &str) -> Self {
        let rule = if text.is_empty() {
            Rule::All
        } else if let Some(tag) = text.strip_prefix('#') {
            Rule::Tag(Pattern::new(tag))
        } else {
            Rule::Name(Pattern::new(text))
        };
        Self { rule }
    }

    #[must_use]
    pub fn matches<'t, 'p>(
        &self,
        item: ResourceLocation<'_>,
        item_tags: impl IntoIterator<Item = ResourceLocation<'t>>,
        search_parts: impl IntoIterator<Item = &'p str>,
    ) -> bool {
        match &self.rule {
            Rule::All => true,
            Rule::Name(Pattern::Anywhere(text)) => contained_in(text, search_parts),
            Rule::Name(Pattern::Split(split)) => {
                item.namespace().contains(split.namespace.as_str())
                    && (item.path().contains(split.path.as_str())
                        || contained_in(&split.path, search_parts))
            }
            Rule::Tag(Pattern::Anywhere(text)) => item_tags
                .into_iter()
                .any(|tag| tag.path().contains(text.as_str())),
            Rule::Tag(Pattern::Split(split)) => item_tags.into_iter().any(|tag| {
                tag.namespace().contains(split.namespace.as_str())
                    && tag.path().contains(split.path.as_str())
            }),
        }
    }
}

impl Pattern {
    fn new(text: &str) -> Self {
        let lower: String = text.chars().flat_map(char::to_lowercase).collect();
        match lower.split_once(':') {
            Some((namespace, path)) => Self::Split(SplitPattern {
                namespace: String::from(namespace.trim_matches(' ')),
                path: String::from(path.trim_matches(' ')),
            }),
            None => Self::Anywhere(lower),
        }
    }
}

fn contained_in<'p>(text: &str, search_parts: impl IntoIterator<Item = &'p str>) -> bool {
    search_parts.into_iter().any(|part| part.contains(text))
}
