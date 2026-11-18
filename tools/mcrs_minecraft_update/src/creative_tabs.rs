use std::fs;
use std::path::Path;

use mcrs_minecraft_creative_report::CreativeReport;
use mcrs_minecraft_creative_report::ListKind;

use crate::corpus;
use crate::fixtures;
use crate::gradle;

pub const FILE: &str = "creative_tabs.json";
const TASK: &str = "dumpCreativeTabs";

pub fn write(root: &Path, reports: &Path) -> Result<(), String> {
    fixtures::with_scratch(TASK, |out| {
        let generated = out.join(FILE);
        gradle::run(
            &root.join(fixtures::ORACLE),
            TASK,
            &[("creativeTabsOut", gradle::utf8(&generated)?)],
        )?;
        let bytes = fs::read(&generated).map_err(|error| corpus::io(&generated, error))?;
        let report = CreativeReport::read(&bytes)
            .map_err(|error| format!("{}: {error}", generated.display()))?;
        println!("{FILE}: {}", summary(&report));
        let stored = reports.join(FILE);
        fs::write(&stored, &bytes).map_err(|error| corpus::io(&stored, error))
    })
}

fn summary(report: &CreativeReport) -> String {
    let stacks = |list: ListKind| -> usize {
        report
            .tabs()
            .iter()
            .map(|tab| tab.list(list).stacks().len())
            .sum()
    };
    format!(
        "{} tabs, {} tab list stacks, {} search list stacks",
        report.tabs().len(),
        stacks(ListKind::TabList),
        stacks(ListKind::SearchList)
    )
}
