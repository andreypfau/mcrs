use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_text::Text;

use crate::saved_form::Fault;

pub(crate) const TAG_MARKER: &str = "#";

#[derive(Clone, Copy, Debug)]
pub(crate) enum Leaf<'a> {
    Int(i32),
    Float(f32),
    Double(f64),
    Boolean(bool),
    String(&'a str),
    Name(ResourceLocation<'a>),
    TagName(ResourceLocation<'a>),
}

pub(crate) struct Field<N> {
    pub(crate) key: &'static str,
    pub(crate) value: Option<N>,
}

impl<N> Field<N> {
    pub(crate) fn new(key: &'static str, value: N) -> Self {
        Self {
            key,
            value: Some(value),
        }
    }

    pub(crate) fn optional(key: &'static str, value: Option<N>) -> Self {
        Self { key, value }
    }
}

pub(crate) struct NamedEntry<'a> {
    pub(crate) name: ResourceLocation<'a>,
    pub(crate) value: Leaf<'a>,
}

pub(crate) struct KeyedEntry<'a, N> {
    pub(crate) key: &'a str,
    pub(crate) value: N,
}

pub(crate) trait SavedSink {
    type Node;

    fn leaf(&self, leaf: Leaf<'_>) -> Self::Node;

    fn text(&mut self, text: &Text) -> Option<Self::Node>;

    fn fields(&self, fields: impl IntoIterator<Item = Field<Self::Node>>) -> Self::Node;

    fn named_map<'a>(
        &mut self,
        entries: impl IntoIterator<Item = Result<NamedEntry<'a>, Fault>>,
    ) -> Result<Self::Node, Fault>;

    fn list<T>(
        &mut self,
        items: impl IntoIterator<Item = T>,
        element: impl FnMut(&mut Self, T) -> Result<Self::Node, Fault>,
    ) -> Result<Self::Node, Fault>;

    fn keyed_map<'a, T>(
        &mut self,
        items: impl IntoIterator<Item = T>,
        entry: impl FnMut(&mut Self, T) -> Result<KeyedEntry<'a, Self::Node>, Fault>,
    ) -> Result<Self::Node, Fault>;
}
