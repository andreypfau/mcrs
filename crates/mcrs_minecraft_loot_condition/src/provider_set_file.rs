use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum ProviderSetFile<P> {
    Tag(ProviderTag),
    Many(Vec<P>),
    One(P),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct ProviderTag(pub(crate) ResourceLocationBuf);

impl TryFrom<String> for ProviderTag {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        let tag = text.strip_prefix('#').ok_or("not a #tag")?;
        ResourceLocationBuf::read(tag)
            .map(ProviderTag)
            .map_err(|error| error.to_string())
    }
}
