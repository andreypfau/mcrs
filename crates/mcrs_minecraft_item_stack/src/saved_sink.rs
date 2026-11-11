use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_text::Text;

use crate::saved_form::Fault;

pub(crate) const TAG_MARKER: &str = "#";

/// A node of the saved form without children.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Leaf<'a> {
    Int(i32),
    Float(f32),
    Double(f64),
    Boolean(bool),
    String(&'a str),
    /// A string node that holds the identifier.
    Name(ResourceLocation<'a>),
    /// A string node that holds [`TAG_MARKER`] and the identifier of a tag.
    TagName(ResourceLocation<'a>),
}

/// A field of a map of fixed shape. A field without a value is omitted.
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

/// An entry of a map whose keys are registry names.
pub(crate) struct NamedEntry<'a> {
    pub(crate) name: ResourceLocation<'a>,
    pub(crate) value: Leaf<'a>,
}

/// What the walk of the saved form of a component emits each node to.
pub(crate) trait SavedSink {
    type Node;

    fn leaf(&self, leaf: Leaf<'_>) -> Self::Node;

    /// `None` when the text has no node of the saved form.
    fn text(&mut self, text: &Text) -> Option<Self::Node>;

    fn fields(&self, fields: impl IntoIterator<Item = Field<Self::Node>>) -> Self::Node;

    fn named_map<'a>(
        &mut self,
        entries: impl IntoIterator<Item = Result<NamedEntry<'a>, Fault>>,
    ) -> Result<Self::Node, Fault>;

    /// A list node with one element for each item, in order.
    fn list<T>(
        &mut self,
        items: impl IntoIterator<Item = T>,
        element: impl FnMut(&mut Self, T) -> Result<Self::Node, Fault>,
    ) -> Result<Self::Node, Fault>;
}
