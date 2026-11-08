//! Starts one server in a fresh folder, builds the pig scene through its console, and measures
//! each phase. See `README.md` in `benchmarks/mob-bench` for the method.

mod adapter;
mod args;
mod os;
#[cfg(target_os = "macos")]
mod os_macos;
mod phase;
mod probe;
mod process;
mod report;
mod run;
mod scene;
mod status;
mod watcher;

use anyhow::bail;

fn main() -> anyhow::Result<()> {
    let all: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, options)) = all.split_first() else {
        bail!("usage: mob-bench run [options]");
    };
    if command != "run" {
        bail!("unknown command {command}; usage: mob-bench run [options]");
    }
    let args = args::parse(options)?;
    let report = run::run(&args)?;
    let text = serde_json::to_string_pretty(&report)?;
    std::fs::write(&args.out, &text)?;
    println!("{}", args.out.display());
    Ok(())
}
