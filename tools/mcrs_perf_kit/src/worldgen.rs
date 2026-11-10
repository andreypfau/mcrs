use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, bail};
use serde::Serialize;

use crate::options::Options;
use crate::profile::Profile;
use crate::progress::Progress;
use crate::section::Section;
use crate::shown_path;

const PACKAGE: &str = "mcrs_minecraft_worldgen_generator";

#[derive(Debug, Serialize)]
pub struct WorldgenBench {
    pub bench: &'static str,
    pub args: &'static [&'static str],
    pub wall_ms: Option<f64>,
    /// `overworld_pipeline`: the cost of one column over every stage.
    pub per_column_ms: Option<f64>,
    /// `beta_chunks`: chunks a second over the slowest worker, and the mean cost of a chunk.
    pub throughput_chunks_per_second: Option<f64>,
    pub per_chunk_mean_ms: Option<f64>,
    pub stdout: String,
    pub raw_log: PathBuf,
    pub error: Option<String>,
}

pub fn run(options: &Options, errors: &mut Vec<String>) -> Vec<WorldgenBench> {
    let mut benches: Vec<(&'static str, &'static [&'static str])> =
        vec![("overworld_pipeline", &["64", "777", "0"])];
    if options.profile == Profile::Full {
        benches.push(("beta_chunks", &["32", "1", "12345"]));
        benches.push(("overworld_pipeline", &["64", "845", "0"]));
    }
    let mut progress = Progress::new(Section::Worldgen, benches.len(), Duration::from_secs(120));
    let mut reports = Vec::new();
    for (index, (bench, args)) in benches.into_iter().enumerate() {
        progress.step(&format!("{bench} {}", args.join(" ")));
        let raw_log = options
            .out
            .join("raw")
            .join("worldgen")
            .join(format!("{index}-{bench}.log"));
        let mut report = WorldgenBench {
            bench,
            args,
            wall_ms: None,
            per_column_ms: None,
            throughput_chunks_per_second: None,
            per_chunk_mean_ms: None,
            stdout: String::new(),
            raw_log: shown_path::under_out(&raw_log, &options.out),
            error: None,
        };
        match run_bench(options, bench, args, &raw_log) {
            Ok(stdout) => {
                report.wall_ms =
                    number_after(&stdout, "wall=").or_else(|| number_after(&stdout, "  wall "));
                report.per_column_ms = number_after(&stdout, "per column: ");
                report.throughput_chunks_per_second = number_after(&stdout, "throughput ");
                report.per_chunk_mean_ms = number_after(&stdout, "per chunk   mean ");
                report.stdout = stdout;
            }
            Err(error) => {
                errors.push(format!("worldgen {bench}: {error:#}"));
                report.error = Some(format!("{error:#}"));
            }
        }
        reports.push(report);
    }
    reports
}

fn run_bench(
    options: &Options,
    bench: &str,
    args: &[&str],
    raw_log: &std::path::Path,
) -> anyhow::Result<String> {
    let output = Command::new("cargo")
        .args(["bench", "-p", PACKAGE, "--bench", bench, "--"])
        .args(args)
        .current_dir(&options.repo)
        .output()
        .context("running cargo bench")?;
    if let Some(parent) = raw_log.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    std::fs::write(raw_log, format!("{stdout}\n--- stderr ---\n{stderr}"))?;
    if !output.status.success() {
        bail!("cargo bench exited with {}", output.status);
    }
    Ok(stdout)
}

/// The first number after `marker`, past any spaces.
fn number_after(text: &str, marker: &str) -> Option<f64> {
    let (_, rest) = text.split_once(marker)?;
    let number: String = rest
        .trim_start()
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '.')
        .collect();
    number.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_after_reads_the_number_past_the_marker() {
        let cases = [
            ("done wall=812.5ms", "wall=", Some(812.5)),
            ("per column: 3.25 ms", "per column: ", Some(3.25)),
            ("throughput    1200 chunks/s", "throughput ", Some(1200.0)),
            ("per chunk   mean 4.0 ms", "per chunk   mean ", Some(4.0)),
            ("throughput n/a", "throughput ", None),
            ("per column: 3.25 ms", "wall=", None),
        ];
        for (text, marker, expected) in cases {
            assert_eq!(
                number_after(text, marker),
                expected,
                "{text} after {marker:?}"
            );
        }
    }
}
