use mcrs_minecraft_resource::ResourceLocationBuf;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PackDiagnostic {
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
        features: Vec<ResourceLocationBuf>,
    },
    PackFound {
        pack: String,
    },
    PackNotAppended {
        pack: String,
        features: Vec<ResourceLocationBuf>,
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
}

impl PackDiagnostic {
    pub fn level(&self) -> Level {
        match self {
            PackDiagnostic::PackFound { .. }
            | PackDiagnostic::PackNotAppended { .. }
            | PackDiagnostic::FeaturePackAppended { .. }
            | PackDiagnostic::EmptyStack => Level::Info,
            PackDiagnostic::MissingPack { .. }
            | PackDiagnostic::UnavailablePack { .. }
            | PackDiagnostic::UnknownCompatibility { .. }
            | PackDiagnostic::OverlaySkipped { .. }
            | PackDiagnostic::PackFeaturesDisabled { .. }
            | PackDiagnostic::InvalidFileName { zip: true, .. } => Level::Warning,
            PackDiagnostic::FilterIgnored { .. } | PackDiagnostic::InvalidFileName { .. } => {
                Level::Error
            }
        }
    }
}

fn joined(features: &[ResourceLocationBuf]) -> String {
    let names: Vec<&str> = features.iter().map(ResourceLocationBuf::as_str).collect();
    names.join(", ")
}

impl fmt::Display for PackDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackDiagnostic::MissingPack { pack } => write!(f, "missing data pack {pack}"),
            PackDiagnostic::UnavailablePack { pack, reason } => {
                write!(f, "data pack {pack} is not available: {reason}")
            }
            PackDiagnostic::UnknownCompatibility { pack, reason } => {
                write!(f, "data pack {pack} has unknown compatibility: {reason}")
            }
            PackDiagnostic::OverlaySkipped {
                pack,
                directory,
                reason,
            } => write!(
                f,
                "overlay {directory} of data pack {pack} skipped: {reason}"
            ),
            PackDiagnostic::PackFeaturesDisabled { pack, features } => write!(
                f,
                "data pack {pack} removed: it requests disabled features {}",
                joined(features)
            ),
            PackDiagnostic::PackFound { pack } => write!(f, "found new data pack {pack}"),
            PackDiagnostic::PackNotAppended { pack, features } => write!(
                f,
                "new data pack {pack} not enabled: it requests disabled features {}",
                joined(features)
            ),
            PackDiagnostic::FeaturePackAppended { pack } => {
                write!(f, "enabled feature pack {pack} for the world's features")
            }
            PackDiagnostic::EmptyStack => f.write_str("no data pack selected, using vanilla"),
            PackDiagnostic::FilterIgnored { pack, reason } => {
                write!(f, "filter of data pack {pack} ignored: {reason}")
            }
            PackDiagnostic::InvalidFileName {
                pack,
                namespace,
                path,
                ..
            } => write!(
                f,
                "file {path} in namespace {namespace} of data pack {pack} ignored: not a valid identifier"
            ),
        }
    }
}
