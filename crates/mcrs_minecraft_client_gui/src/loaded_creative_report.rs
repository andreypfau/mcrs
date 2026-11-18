use bevy::log::error;
use bevy::prelude::Resource;
use mcrs_minecraft_creative_report::CreativeReport;

const CREATIVE_REPORT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/mcrs/reports/creative_tabs.json"
));

#[derive(Resource, Debug)]
pub struct LoadedCreativeReport(pub Option<CreativeReport>);

impl LoadedCreativeReport {
    #[must_use]
    pub fn embedded() -> Self {
        match CreativeReport::read(CREATIVE_REPORT) {
            Ok(report) => Self(Some(report)),
            Err(error) => {
                error!(%error, "the creative report is rejected; the creative screen has no items tab");
                Self(None)
            }
        }
    }
}
