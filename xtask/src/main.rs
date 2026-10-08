mod checks;
mod git;
mod metadata;
mod scope;
mod trigger;

use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

const DEFAULT_BASE_REF: &str = "origin/main";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let trigger = match trigger::from_args(&args) {
        Ok(trigger) => trigger,
        Err(problem) => {
            eprintln!("{problem}\n\n{}", trigger::usage());
            return ExitCode::from(2);
        }
    };
    let root = match checks::root_of(Path::new(".")) {
        Ok(root) => root,
        Err(problem) => {
            eprintln!("{problem}");
            return ExitCode::from(1);
        }
    };
    let base_ref = std::env::var("CHECKS_BASE_REF")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_BASE_REF.to_owned());
    let ctx = checks::Context::new(trigger, root, base_ref);

    let (mut passed, mut failed) = (0, 0);
    for &check in trigger::checks(trigger) {
        let started = Instant::now();
        let outcome = checks::run(&ctx, check);
        let seconds = started.elapsed().as_secs_f32();
        match outcome {
            Ok(()) => {
                passed += 1;
                println!("{}: ok ({seconds:.1}s)", check.name());
            }
            Err(message) => {
                failed += 1;
                println!("{}: FAILED ({seconds:.1}s): {message}", check.name());
            }
        }
    }
    println!("checks: {passed} passed, {failed} failed");
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
