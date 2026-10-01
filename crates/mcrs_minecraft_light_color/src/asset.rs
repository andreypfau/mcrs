use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct LightColor(pub [u8; 3]);

impl<'de> Deserialize<'de> for LightColor {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        let invalid = || de::Error::custom(format!("expected a `#rrggbb` colour, got `{text}`"));
        // `from_str_radix` alone would accept a sign such as `#+6d9e6`.
        let hex = text
            .strip_prefix('#')
            .filter(|hex| hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(invalid)?;
        let [_, r, g, b] = u32::from_str_radix(hex, 16)
            .map_err(|_| invalid())?
            .to_be_bytes();
        Ok(LightColor([r, g, b]))
    }
}

impl Serialize for LightColor {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let [r, g, b] = self.0;
        s.collect_str(&format_args!("#{r:02x}{g:02x}{b:02x}"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateTarget {
    Block(ResourceLocation<Arc<str>>),
    Tag(ResourceLocation<Arc<str>>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockStateRef {
    pub target: StateTarget,
    pub properties: Vec<(String, String)>,
}

impl FromStr for BlockStateRef {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let (id, list) = match text.split_once('[') {
            None => (text, None),
            Some((id, rest)) => {
                let list = rest
                    .strip_suffix(']')
                    .ok_or_else(|| format!("`{text}` does not end its property list with `]`"))?;
                (id, Some(list))
            }
        };
        let (tag, id) = match id.strip_prefix('#') {
            Some(id) => (true, id),
            None => (false, id),
        };
        if id.is_empty() {
            return Err(format!("`{text}` names no block or tag"));
        }
        let id = ResourceLocation::read(id).map_err(|e| e.to_string())?;
        let target = if tag {
            StateTarget::Tag(id)
        } else {
            StateTarget::Block(id)
        };

        let mut properties: Vec<(String, String)> = Vec::new();
        if let Some(list) = list {
            if list.is_empty() || list.contains(['[', ']']) {
                return Err(format!("`{text}` has a malformed property list"));
            }
            for pair in list.split(',') {
                let (name, value) = pair
                    .split_once('=')
                    .filter(|(name, value)| !name.is_empty() && !value.is_empty())
                    .ok_or_else(|| format!("`{pair}` in `{text}` is not `name=value`"))?;
                if properties.iter().any(|(seen, _)| seen == name) {
                    return Err(format!("`{text}` states `{name}` twice"));
                }
                properties.push((name.to_owned(), value.to_owned()));
            }
        }
        Ok(BlockStateRef { target, properties })
    }
}

impl fmt::Display for BlockStateRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.target {
            StateTarget::Block(id) => f.write_str(id.as_str())?,
            StateTarget::Tag(id) => write!(f, "#{}", id.as_str())?,
        }
        if self.properties.is_empty() {
            return Ok(());
        }
        f.write_str("[")?;
        for (i, (name, value)) in self.properties.iter().enumerate() {
            if i > 0 {
                f.write_str(",")?;
            }
            write!(f, "{name}={value}")?;
        }
        f.write_str("]")
    }
}

impl<'de> Deserialize<'de> for BlockStateRef {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(de::Error::custom)
    }
}

impl Serialize for BlockStateRef {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightColorFile {
    pub color: LightColor,
    pub blocks: Vec<BlockStateRef>,
}
