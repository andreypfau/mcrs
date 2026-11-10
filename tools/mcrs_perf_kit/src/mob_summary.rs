use serde::{Deserialize, Serialize};

/// The numbers of one phase of a mob-bench run report that the kit keeps.
#[derive(Debug, Serialize)]
pub struct MobPhase {
    pub phase: String,
    pub skipped: Option<String>,
    pub server_tick_mean_ms: Option<f64>,
    pub server_tick_p50_ms: Option<f64>,
    pub server_tick_p99_ms: Option<f64>,
    pub console_tps: Option<f64>,
    pub sprint_wall_ms_per_tick: Option<f64>,
    pub server_cores: Option<f64>,
    pub other_cores: Option<f64>,
    pub server_rss_mib_mean: Option<f64>,
    pub server_rss_mib_max: Option<f64>,
    pub contended: bool,
}

#[derive(Deserialize)]
pub struct RunIn {
    phases: Vec<PhaseIn>,
}

#[derive(Deserialize)]
struct PhaseIn {
    phase: String,
    skipped: Option<String>,
    server_mspt: Option<MsptIn>,
    external: ExternalIn,
    cpu: Option<CpuIn>,
    server_rss_mib_mean: Option<f64>,
    server_rss_mib_max: Option<f64>,
    contended: bool,
}

#[derive(Deserialize)]
struct MsptIn {
    mean: f64,
    p50: f64,
    p99: f64,
}

#[derive(Deserialize)]
struct ExternalIn {
    console_game_time: Option<RateIn>,
    sprint: Option<SprintIn>,
}

#[derive(Deserialize)]
struct RateIn {
    tps: f64,
}

#[derive(Deserialize)]
struct SprintIn {
    wall_ms_per_tick: f64,
}

#[derive(Deserialize)]
struct CpuIn {
    server_cores: f64,
    other_cores: f64,
}

impl RunIn {
    pub fn into_phases(self) -> impl Iterator<Item = MobPhase> {
        self.phases.into_iter().map(|phase| MobPhase {
            phase: phase.phase,
            skipped: phase.skipped,
            server_tick_mean_ms: phase.server_mspt.as_ref().map(|mspt| mspt.mean),
            server_tick_p50_ms: phase.server_mspt.as_ref().map(|mspt| mspt.p50),
            server_tick_p99_ms: phase.server_mspt.as_ref().map(|mspt| mspt.p99),
            console_tps: phase.external.console_game_time.map(|rate| rate.tps),
            sprint_wall_ms_per_tick: phase.external.sprint.map(|sprint| sprint.wall_ms_per_tick),
            server_cores: phase.cpu.as_ref().map(|cpu| cpu.server_cores),
            other_cores: phase.cpu.as_ref().map(|cpu| cpu.other_cores),
            server_rss_mib_mean: phase.server_rss_mib_mean,
            server_rss_mib_max: phase.server_rss_mib_max,
            contended: phase.contended,
        })
    }
}
