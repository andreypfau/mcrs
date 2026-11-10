//! `perf-kit` runs the mcrs measurements on a tester's machine and writes one `report.json`.

mod client;
mod client_run;
mod clock;
mod java;
mod median;
mod mob;
mod mob_summary;
mod options;
mod process;
mod profile;
mod progress;
mod repo;
mod report;
mod scratch_dir;
mod section;
mod server_start;
mod settle;
mod shown_path;
mod stats_line;
mod vanilla_jar;
mod worldgen;

use std::process::ExitCode;

use crate::options::Options;
use crate::report::Report;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let options = match Options::parse(&arguments) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error:#}");
            return ExitCode::from(2);
        }
    };
    // The mcrs console writes translation keys instead of English until the client jar is on disk,
    // and mob-bench reads its answers in English, so the jar comes before any section starts.
    println!("[setup] client jar of {}", mcrs_minecraft_client_jar::RELEASE.id);
    if let Err(error) = mcrs_minecraft_client_jar::vanilla_data(
        &mcrs_minecraft_client_jar::ClientJar::InstalledOrDownloaded,
        None,
        &mcrs_minecraft_client_jar::Progress::default(),
    ) {
        eprintln!("[setup] the client jar is missing ({error}); the mob section will fail");
    }
    let out = options.out.clone();
    let report = Report::run(options);
    let path = out.join("report.json");
    let written = std::fs::create_dir_all(&out)
        .map_err(anyhow::Error::from)
        .and_then(|()| Ok(serde_json::to_vec_pretty(&report)?))
        .and_then(|json| Ok(std::fs::write(&path, json)?));
    if let Err(error) = written {
        eprintln!("writing {}: {error:#}", path.display());
        return ExitCode::FAILURE;
    }
    println!("wrote {}", path.display());
    if report.errors.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
