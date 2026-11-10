use std::path::Path;

use anyhow::Context;
use serde::Deserialize;

/// The parts of `report.json` the summary reads; the kit's own report types borrow `'static` names and only serialize.
#[derive(Deserialize)]
pub struct ReportIn {
    pub profile: String,
    pub options: OptionsIn,
    pub mob: Option<Vec<MobRunIn>>,
    pub client: Option<ClientIn>,
    pub server_start: Option<ServerStartIn>,
    pub worldgen: Option<Vec<WorldgenIn>>,
    pub errors: Vec<ErrorIn>,
}

#[derive(Deserialize)]
pub struct OptionsIn {
    pub sections: Vec<String>,
}

#[derive(Deserialize)]
pub struct MobRunIn {
    pub name: String,
    pub report: String,
    pub phases: Vec<PhaseIn>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct PhaseIn {
    pub phase: String,
    pub skipped: Option<String>,
    pub console_tps: Option<f64>,
    pub sprint_wall_ms_per_tick: Option<f64>,
}

#[derive(Deserialize)]
pub struct ClientIn {
    pub scenarios: Vec<ScenarioIn>,
}

#[derive(Deserialize)]
pub struct ScenarioIn {
    pub scenario: String,
    pub mode: String,
    pub clock: String,
    pub median: MedianIn,
    pub runs: Vec<RunIn>,
}

#[derive(Deserialize)]
pub struct MedianIn {
    pub fps: Option<f64>,
    pub frame_median_ms: Option<f64>,
}

#[derive(Deserialize)]
pub struct RunIn {
    pub error: Option<String>,
    pub window_mismatch: Option<String>,
}

#[derive(Deserialize)]
pub struct ServerStartIn {
    pub playing_seconds_median: Option<f64>,
}

#[derive(Deserialize)]
pub struct WorldgenIn {
    pub bench: String,
    pub per_column_ms: Option<f64>,
    pub throughput_chunks_per_second: Option<f64>,
}

#[derive(Deserialize)]
pub struct ErrorIn {
    pub section: String,
    pub message: String,
}

#[derive(Default, Deserialize)]
pub struct HygieneIn {
    pub checks: Vec<CheckIn>,
}

#[derive(Deserialize)]
pub struct CheckIn {
    pub name: String,
    pub value: String,
    pub ok: bool,
}

#[derive(Default, Deserialize)]
pub struct SystemIn {
    pub os: Option<OsIn>,
    pub cpu: Option<CpuIn>,
    pub memory_bytes: Option<u64>,
    pub gpus: Option<Vec<GpuIn>>,
}

#[derive(Deserialize)]
pub struct OsIn {
    pub name: Option<String>,
    pub version: Option<String>,
}

#[derive(Deserialize)]
pub struct CpuIn {
    pub brand: Option<String>,
}

#[derive(Deserialize)]
pub struct GpuIn {
    pub model: Option<String>,
    pub cores: Option<u32>,
}

#[derive(Deserialize)]
struct PigsIn {
    pigs: Option<u64>,
}

pub fn read_report(dir: &Path) -> anyhow::Result<ReportIn> {
    let path = dir.join("report.json");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// A missing or unreadable optional file reads as empty: the summary prints "-" for it.
pub fn read_optional<T: Default + for<'de> Deserialize<'de>>(path: &Path) -> T {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// The crowd size of one mob run, from the run's own report next to `report.json`.
pub fn read_pigs(dir: &Path, run: &MobRunIn) -> Option<u64> {
    let text = std::fs::read_to_string(dir.join(&run.report)).ok()?;
    serde_json::from_str::<PigsIn>(&text).ok()?.pigs
}
