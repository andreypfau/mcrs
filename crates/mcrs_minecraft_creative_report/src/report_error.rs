use core::fmt;

use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::Group;
use crate::ListKind;

#[derive(Debug)]
#[non_exhaustive]
pub enum ReportError {
    Malformed(serde_json::Error),
    UnknownTab(ResourceLocationBuf),
    MissingTab(ResourceLocationBuf),
    SplitGroup {
        tab: ResourceLocationBuf,
        list: ListKind,
        group: Group,
    },
}

impl fmt::Display for ReportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(error) => {
                write!(formatter, "the creative report is malformed: {error}")
            }
            Self::UnknownTab(tab) => write!(formatter, "{tab} is not an items tab"),
            Self::MissingTab(tab) => write!(formatter, "the items tab {tab} has no entry"),
            Self::SplitGroup { tab, list, group } => write!(
                formatter,
                "the {list} of {tab} holds the group {group} in more than one span"
            ),
        }
    }
}

impl core::error::Error for ReportError {}
