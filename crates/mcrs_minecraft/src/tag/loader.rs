use bevy_asset::io::Reader;
use bevy_asset::{
    Asset, AssetLoader, AssetPath, Handle, LoadContext, LoadDirectError, UntypedAssetId,
    VisitAssetDependencies,
};
use bevy_reflect::TypePath;
use bevy_tasks::ConditionalSendFuture;
use mcrs_protocol::Ident;
use mcrs_protocol::ident::IdentError;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Display;
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, TypePath)]
pub struct ResourcePackTags {
    pub values: Vec<TagEntry>,
    pub replace: bool,
}

#[derive(Debug, Deserialize)]
pub struct SerializedTagFile {
    pub values: Vec<SerializedTagEntry>,
    #[serde(default)]
    pub replace: bool,
}

#[derive(Debug)]
pub struct TagEntry {
    pub tag: TagOrTagFileHandle,
    pub required: bool,
}

#[derive(Debug)]
pub enum TagOrTagFileHandle {
    Tag(Ident<String>),
    TagFile(Handle<ResourcePackTags>),
}

impl Asset for ResourcePackTags {}

impl VisitAssetDependencies for ResourcePackTags {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        for entry in &self.values {
            if let TagOrTagFileHandle::TagFile(handle) = &entry.tag {
                let untyped = UntypedAssetId::from(handle);
                visit(untyped);
            }
        }
    }
}

#[derive(Default, TypePath)]
pub struct ResourcePackTagsLoader;

#[derive(Debug, Error)]
pub enum ResourcePackTagsLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    LoadDirectError(#[from] LoadDirectError),
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct TagFileLoaderSettings {
    pub directory: String,
}

impl AssetLoader for ResourcePackTagsLoader {
    type Asset = ResourcePackTags;
    type Settings = TagFileLoaderSettings;
    type Error = ResourcePackTagsLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<ResourcePackTags, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let serialized_tag_file: SerializedTagFile = serde_json::de::from_slice(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let values = serialized_tag_file
            .values
            .into_iter()
            .map(|e| {
                if e.loc.tag {
                    let path = Path::new(&_settings.directory)
                        .join(e.loc.id.path())
                        .with_extension("json");
                    let settings = _settings.clone();

                    let dependency = load_context
                        .loader()
                        .with_settings::<TagFileLoaderSettings>(move |s| {
                            *s = settings.clone();
                        })
                        .load::<ResourcePackTags>(path);

                    TagEntry {
                        tag: TagOrTagFileHandle::TagFile(dependency),
                        required: e.required,
                    }
                } else {
                    TagEntry {
                        tag: TagOrTagFileHandle::Tag(e.loc.id),
                        required: e.required,
                    }
                }
            })
            .collect();

        Ok(ResourcePackTags {
            values,
            replace: serialized_tag_file.replace,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SerializedTagEntry {
    pub loc: TagOrElementLocation,
    pub required: bool,
}

impl<'de> Deserialize<'de> for SerializedTagEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match TagEntryRepr::deserialize(deserializer)? {
            TagEntryRepr::Short(loc) => Ok(SerializedTagEntry {
                loc,
                required: true,
            }),
            TagEntryRepr::Full(full) => Ok(SerializedTagEntry {
                loc: full.id,
                required: full.required,
            }),
        }
    }
}

impl Serialize for SerializedTagEntry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.required {
            self.loc.serialize(serializer)
        } else {
            let mut st = serializer.serialize_struct("TagEntry", 2)?;
            st.serialize_field("id", &self.loc)?;
            st.serialize_field("required", &self.required)?;
            st.end()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TagOrElementLocation {
    pub id: Ident<String>,
    pub tag: bool,
}

impl Display for TagOrElementLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.tag {
            write!(f, "#{}", self.id.as_str())
        } else {
            write!(f, "{}", self.id.as_str())
        }
    }
}

impl FromStr for TagOrElementLocation {
    type Err = IdentError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(rest) = s.strip_prefix('#') {
            Ok(TagOrElementLocation {
                id: Ident::from_str(rest)?,
                tag: true,
            })
        } else {
            Ok(TagOrElementLocation {
                id: Ident::from_str(s)?,
                tag: false,
            })
        }
    }
}

impl Serialize for TagOrElementLocation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for TagOrElementLocation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        TagOrElementLocation::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TagEntryRepr {
    Short(TagOrElementLocation),
    Full(TagEntryFull),
}

#[inline]
fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct TagEntryFull {
    pub id: TagOrElementLocation,
    #[serde(default = "default_true")]
    pub required: bool,
}
