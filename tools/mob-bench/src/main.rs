//! Starts one server in a fresh folder, builds the pig scene through its console, and measures
//! each phase. See `README.md` in `benchmarks/mob-bench` for the method.

use anyhow::bail;
use mcrs_mob_bench::{args, run};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
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
