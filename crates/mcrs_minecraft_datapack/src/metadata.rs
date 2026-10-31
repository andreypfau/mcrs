use std::fmt;

use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_resource::ResourceLocation;
use serde::de::{self, IgnoredAny, Visitor};
use serde::{Deserialize, Deserializer};

use crate::PackDiagnostic;
use crate::filter::FilterSection;
use crate::json::read_json;

const LAST_LEGACY_FORMAT: u32 = 81;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Format(u32, u32);

fn current() -> Format {
    Format(
        VERSION.pack_version.data_major,
        VERSION.pack_version.data_minor,
    )
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum Lenient<T> {
    Valid(T),
    Invalid(IgnoredAny),
}

impl<T> Lenient<T> {
    fn get<'a>(field: &'a Option<Lenient<T>>, name: &str) -> Result<Option<&'a T>, String> {
        match field {
            None => Ok(None),
            Some(Lenient::Valid(value)) => Ok(Some(value)),
            Some(Lenient::Invalid(_)) => Err(format!("{name} is malformed")),
        }
    }
}

pub(crate) struct Metadata {
    pub(crate) features: Vec<ResourceLocation>,
    pub(crate) filter: Option<Lenient<FilterSection>>,
    pub(crate) overlays: Vec<String>,
}

#[derive(Deserialize)]
struct Root {
    pack: PackSection,
    features: Option<FeaturesSection>,
    filter: Option<Lenient<FilterSection>>,
    overlays: Option<OverlaysSection>,
}

#[derive(Deserialize)]
struct PackSection {
    #[serde(rename = "description")]
    _description: Description,
    min_format: Option<Lenient<Bound>>,
    max_format: Option<Lenient<Bound>>,
    pack_format: Option<Lenient<u32>>,
    supported_formats: Option<Lenient<Legacy>>,
}

#[derive(Deserialize)]
struct FeaturesSection {
    enabled: Vec<ResourceLocation>,
}

#[derive(Deserialize)]
struct OverlaysSection {
    entries: Vec<OverlayEntry>,
}

#[derive(Deserialize)]
struct OverlayEntry {
    directory: OverlayDirectory,
    min_format: Option<Lenient<Bound>>,
    max_format: Option<Lenient<Bound>>,
    formats: Option<Lenient<Legacy>>,
}

struct Description;

impl<'de> Deserialize<'de> for Description {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct DescriptionVisitor;

        impl<'de> Visitor<'de> for DescriptionVisitor {
            type Value = Description;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a text component")
            }

            fn visit_str<E>(self, _: &str) -> Result<Description, E> {
                Ok(Description)
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Description, A::Error> {
                while seq.next_element::<IgnoredAny>()?.is_some() {}
                Ok(Description)
            }

            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Description, A::Error> {
                while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                Ok(Description)
            }
        }

        deserializer.deserialize_any(DescriptionVisitor)
    }
}

struct OverlayDirectory(String);

impl<'de> Deserialize<'de> for OverlayDirectory {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        let valid = name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
        match valid {
            true => Ok(Self(name)),
            false => Err(de::Error::custom(format!(
                "overlay directory {name:?} is not over [-_a-zA-Z0-9.]"
            ))),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Bound {
    Major(u32),
    List(Vec<u32>),
}

impl Bound {
    fn parts(&self) -> Result<(u32, Option<u32>), String> {
        match self {
            Bound::Major(major) => Ok((*major, None)),
            Bound::List(list) => match list[..] {
                [major] => Ok((major, None)),
                [major, minor] => Ok((major, Some(minor))),
                _ => Err(format!(
                    "a format is [major] or [major, minor], not {list:?}"
                )),
            },
        }
    }

    fn lower(&self) -> Result<Format, String> {
        self.parts()
            .map(|(major, minor)| Format(major, minor.unwrap_or(0)))
    }

    fn upper(&self) -> Result<Format, String> {
        self.parts()
            .map(|(major, minor)| Format(major, minor.unwrap_or(u32::MAX)))
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Legacy {
    One(u32),
    Pair([u32; 2]),
    Range {
        min_inclusive: u32,
        max_inclusive: u32,
    },
}

impl Legacy {
    fn range(&self) -> (Format, Format) {
        let (low, high) = match *self {
            Legacy::One(major) => (major, major),
            Legacy::Pair([low, high]) => (low, high),
            Legacy::Range {
                min_inclusive,
                max_inclusive,
            } => (min_inclusive, max_inclusive),
        };
        (Format(low, 0), Format(high, u32::MAX))
    }
}

fn bounded(
    min: &Option<Lenient<Bound>>,
    max: &Option<Lenient<Bound>>,
) -> Result<Option<(Format, Format)>, String> {
    let min = Lenient::get(min, "min_format")?;
    let max = Lenient::get(max, "max_format")?;
    match (min, max) {
        (Some(min), Some(max)) => {
            let (low, high) = (min.lower()?, max.upper()?);
            match low <= high {
                true => Ok(Some((low, high))),
                false => Err("min_format is above max_format".to_owned()),
            }
        }
        (None, None) => Ok(None),
        _ => Err("min_format and max_format must be given together".to_owned()),
    }
}

fn pack_range(section: &PackSection) -> Result<(Format, Format), String> {
    let legacy = Lenient::get(&section.supported_formats, "supported_formats")?;
    let pack_format = Lenient::get(&section.pack_format, "pack_format")?.copied();
    match bounded(&section.min_format, &section.max_format)? {
        Some((low, high)) => {
            if low.0 <= LAST_LEGACY_FORMAT {
                if legacy.is_none() || pack_format.is_none() {
                    return Err(format!(
                        "a range reaching format {LAST_LEGACY_FORMAT} needs supported_formats and pack_format"
                    ));
                }
            } else if legacy.is_some() {
                return Err(format!(
                    "supported_formats is not allowed above format {LAST_LEGACY_FORMAT}"
                ));
            }
            if let Some(pack_format) = pack_format
                && !(low.0..=high.0).contains(&pack_format)
            {
                return Err(format!("pack_format {pack_format} is outside the range"));
            }
            Ok((low, high))
        }
        None => match pack_format {
            None => Err("no format is declared".to_owned()),
            Some(pack_format) if pack_format > LAST_LEGACY_FORMAT => Err(format!(
                "pack_format above {LAST_LEGACY_FORMAT} needs min_format and max_format"
            )),
            Some(pack_format) => Ok(legacy.map_or(
                (Format(pack_format, 0), Format(pack_format, u32::MAX)),
                Legacy::range,
            )),
        },
    }
}

fn overlay_range(entry: &OverlayEntry) -> Result<(Format, Format), String> {
    match bounded(&entry.min_format, &entry.max_format)? {
        Some(range) => Ok(range),
        None => Lenient::get(&entry.formats, "formats")?
            .map(Legacy::range)
            .ok_or_else(|| "no format is declared".to_owned()),
    }
}

pub(crate) fn read(
    pack: &str,
    bytes: &[u8],
    known_features: &[ResourceLocation],
    diagnostics: &mut Vec<PackDiagnostic>,
) -> Result<Metadata, String> {
    let root: Root = read_json(bytes).map_err(|e| e.message)?;

    if let Err(reason) = pack_range(&root.pack) {
        diagnostics.push(PackDiagnostic::UnknownCompatibility {
            pack: pack.to_owned(),
            reason,
        });
    }

    let features = root.features.map(|f| f.enabled).unwrap_or_default();
    let unknown: Vec<&str> = features
        .iter()
        .filter(|feature| !known_features.contains(feature))
        .map(ResourceLocation::as_str)
        .collect();
    if !unknown.is_empty() {
        return Err(format!("unknown features {}", unknown.join(", ")));
    }

    let mut overlays = Vec::new();
    for entry in root.overlays.map(|o| o.entries).unwrap_or_default() {
        match overlay_range(&entry) {
            Ok((low, high)) if (low..=high).contains(&current()) => {
                overlays.push(entry.directory.0)
            }
            Ok(_) => {}
            Err(reason) => diagnostics.push(PackDiagnostic::OverlaySkipped {
                pack: pack.to_owned(),
                directory: entry.directory.0,
                reason,
            }),
        }
    }

    Ok(Metadata {
        features,
        filter: root.filter,
        overlays,
    })
}
