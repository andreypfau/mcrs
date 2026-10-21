use std::cmp::Ordering;
use std::fmt;
use std::ops::Deref;

use mcrs_minecraft_core::{ResourceLocation, StaticResourceLocation};

use crate::order::identifier_order;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Referrer {
    Entry {
        registry: ResourceLocation,
        entry: ResourceLocation,
    },
    TagFile {
        tag: ResourceLocation,
        pack: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    pub name: ResourceLocation,
    pub tag: bool,
    pub referrer: Referrer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Diagnostic {
    MissingPack {
        pack: String,
    },
    UnavailablePack {
        pack: String,
        reason: String,
    },
    UnknownCompatibility {
        pack: String,
        reason: String,
    },
    OverlaySkipped {
        pack: String,
        directory: String,
        reason: String,
    },
    PackFeaturesDisabled {
        pack: String,
        features: Vec<ResourceLocation>,
    },
    PackFound {
        pack: String,
    },
    PackNotAppended {
        pack: String,
        features: Vec<ResourceLocation>,
    },
    FeaturePackAppended {
        pack: String,
    },
    EmptyStack,
    FilterIgnored {
        pack: String,
        reason: String,
    },
    InvalidFileName {
        pack: String,
        zip: bool,
        namespace: String,
        path: String,
    },
    TagFileSkipped {
        registry: ResourceLocation,
        tag: ResourceLocation,
        pack: String,
        file: String,
        reason: String,
    },
    TagFailed {
        registry: ResourceLocation,
        tag: ResourceLocation,
        missing: Vec<Missing>,
    },
    Entry {
        registry: ResourceLocation,
        entry: ResourceLocation,
        pack: Option<String>,
        file: Option<String>,
        field: Option<String>,
        message: String,
    },
    Unresolved {
        registry: ResourceLocation,
        missing: Vec<Missing>,
    },
    EmptyRegistry {
        registry: ResourceLocation,
    },
    TooManyEntries {
        registry: ResourceLocation,
        count: usize,
    },
    TooManyTags {
        registry: ResourceLocation,
        count: usize,
    },
    Validation {
        registry: ResourceLocation,
        message: String,
    },
    Declaration {
        registry: ResourceLocation,
        message: String,
    },
    MissingTag {
        registry: ResourceLocation,
        tag: ResourceLocation,
    },
}

impl Diagnostic {
    pub fn level(&self) -> Level {
        match self {
            Diagnostic::PackFound { .. }
            | Diagnostic::PackNotAppended { .. }
            | Diagnostic::FeaturePackAppended { .. }
            | Diagnostic::EmptyStack => Level::Info,
            Diagnostic::MissingPack { .. }
            | Diagnostic::UnavailablePack { .. }
            | Diagnostic::UnknownCompatibility { .. }
            | Diagnostic::OverlaySkipped { .. }
            | Diagnostic::PackFeaturesDisabled { .. } => Level::Warning,
            Diagnostic::InvalidFileName { zip: true, .. } => Level::Warning,
            _ => Level::Error,
        }
    }

    pub fn report_key(&self) -> Option<(ResourceLocation, ResourceLocation)> {
        match self {
            Diagnostic::Entry {
                registry, entry, ..
            } => Some((registry.clone(), entry.clone())),
            Diagnostic::Unresolved { registry, .. }
            | Diagnostic::EmptyRegistry { registry }
            | Diagnostic::TooManyEntries { registry, .. }
            | Diagnostic::TooManyTags { registry, .. }
            | Diagnostic::Validation { registry, .. }
            | Diagnostic::Declaration { registry, .. } => Some((ROOT.into(), registry.clone())),
            _ => None,
        }
    }
}

pub(crate) const ROOT: StaticResourceLocation = ResourceLocation::new_static("minecraft:root");

fn list<T: fmt::Display>(f: &mut fmt::Formatter<'_>, items: &[T]) -> fmt::Result {
    for (n, item) in items.iter().enumerate() {
        if n > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{item}")?;
    }
    Ok(())
}

impl fmt::Display for Missing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hash = if self.tag { "#" } else { "" };
        write!(f, "{hash}{}", self.name)?;
        match &self.referrer {
            Referrer::Entry { registry, entry } => write!(f, " (from {registry}/{entry})"),
            Referrer::TagFile { tag, pack } => write!(f, " (from #{tag} in pack {pack})"),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some((registry, key)) = self.report_key() {
            write!(f, "{registry}/{key}: ")?;
        }
        match self {
            Diagnostic::MissingPack { pack } => write!(f, "missing data pack {pack}"),
            Diagnostic::UnavailablePack { pack, reason } => {
                write!(f, "data pack {pack} is not available: {reason}")
            }
            Diagnostic::UnknownCompatibility { pack, reason } => {
                write!(f, "data pack {pack} has unknown compatibility: {reason}")
            }
            Diagnostic::OverlaySkipped {
                pack,
                directory,
                reason,
            } => write!(
                f,
                "overlay {directory} of data pack {pack} skipped: {reason}"
            ),
            Diagnostic::PackFeaturesDisabled { pack, features } => {
                write!(
                    f,
                    "data pack {pack} removed: it requests disabled features "
                )?;
                list(f, features)
            }
            Diagnostic::PackFound { pack } => write!(f, "found new data pack {pack}"),
            Diagnostic::PackNotAppended { pack, features } => {
                write!(
                    f,
                    "new data pack {pack} not enabled: it requests disabled features "
                )?;
                list(f, features)
            }
            Diagnostic::FeaturePackAppended { pack } => {
                write!(f, "enabled feature pack {pack} for the world's features")
            }
            Diagnostic::EmptyStack => f.write_str("no data pack selected, using vanilla"),
            Diagnostic::FilterIgnored { pack, reason } => {
                write!(f, "filter of data pack {pack} ignored: {reason}")
            }
            Diagnostic::InvalidFileName {
                pack,
                namespace,
                path,
                ..
            } => write!(
                f,
                "file {path} in namespace {namespace} of data pack {pack} ignored: not a valid identifier"
            ),
            Diagnostic::TagFileSkipped {
                registry,
                tag,
                pack,
                file,
                reason,
            } => write!(
                f,
                "tag file {file} of #{tag} in {registry} from data pack {pack} skipped: {reason}"
            ),
            Diagnostic::TagFailed {
                registry,
                tag,
                missing,
            } => {
                write!(f, "tag #{tag} in {registry} failed, missing ")?;
                list(f, missing)
            }
            Diagnostic::Entry {
                pack,
                file,
                field,
                message,
                ..
            } => {
                if let Some(field) = field {
                    write!(f, "{field}: ")?;
                }
                f.write_str(message)?;
                if let Some(file) = file {
                    write!(f, " in {file}")?;
                }
                if let Some(pack) = pack {
                    write!(f, " of data pack {pack}")?;
                }
                Ok(())
            }
            Diagnostic::Unresolved { missing, .. } => {
                f.write_str("unresolved references ")?;
                list(f, missing)
            }
            Diagnostic::EmptyRegistry { .. } => f.write_str("the registry must not be empty"),
            Diagnostic::TooManyEntries { count, .. } => over_range(f, *count, "entries"),
            Diagnostic::TooManyTags { count, .. } => over_range(f, *count, "tags"),
            Diagnostic::Validation { message, .. } | Diagnostic::Declaration { message, .. } => {
                f.write_str(message)
            }
            Diagnostic::MissingTag { registry, tag } => missing_tag(f, registry, tag),
        }
    }
}

fn missing_tag(
    f: &mut fmt::Formatter<'_>,
    registry: &ResourceLocation,
    tag: &ResourceLocation,
) -> fmt::Result {
    write!(
        f,
        "{registry}/#{tag}: registry {registry} holds no tag named {tag}"
    )
}

fn over_range(f: &mut fmt::Formatter<'_>, count: usize, what: &str) -> fmt::Result {
    write!(f, "{count} {what} is more than a registry can number")
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diagnostics(Vec<Diagnostic>);

impl Deref for Diagnostics {
    type Target = [Diagnostic];

    fn deref(&self) -> &[Diagnostic] {
        &self.0
    }
}

impl fmt::Display for Diagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (n, diagnostic) in self.0.iter().enumerate() {
            if n > 0 {
                f.write_str("\n")?;
            }
            write!(f, "{diagnostic}")?;
        }
        Ok(())
    }
}

impl Extend<Diagnostic> for Diagnostics {
    fn extend<I: IntoIterator<Item = Diagnostic>>(&mut self, diagnostics: I) {
        self.0.extend(diagnostics);
    }
}

impl Diagnostics {
    pub(crate) fn push_report(&mut self, errors: Vec<Diagnostic>) {
        let mut keyed: Vec<_> = errors.into_iter().map(|d| (d.report_key(), d)).collect();
        keyed.sort_by(|(a, _), (b, _)| match (a, b) {
            (Some(a), Some(b)) => {
                identifier_order(&a.0, &b.0).then_with(|| identifier_order(&a.1, &b.1))
            }
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
        });
        self.0
            .extend(keyed.into_iter().map(|(_, diagnostic)| diagnostic));
    }
}
