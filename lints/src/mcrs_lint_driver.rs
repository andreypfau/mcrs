#![feature(rustc_private)]

extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_infer;
extern crate rustc_interface;
extern crate rustc_lint;
extern crate rustc_lint_defs;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_trait_selection;

mod anchors;
mod body_scan;
mod callbacks;
mod collect;
mod ecs_lint;
mod lints;
mod resolve;
mod signature_rules;
mod system_rules;

use std::process::Command;
use std::process::ExitCode;

use callbacks::LintCallbacks;
use rustc_session::EarlyDiagCtxt;
use rustc_session::config::ErrorOutputType;

fn main() -> ExitCode {
    #[expect(
        clippy::disallowed_methods,
        reason = "rustup names the active toolchain in the environment"
    )]
    let active = std::env::var("RUSTUP_TOOLCHAIN");
    if let (Some(built), Ok(active)) = (option_env!("RUSTUP_TOOLCHAIN"), active)
        && built != active
    {
        eprintln!(
            "warning: mcrs_lint is built for {built}, the toolchain is {active}; rebuild it with `cargo build --release --manifest-path lints/Cargo.toml`"
        );
        return run_rustc();
    }
    let early = EarlyDiagCtxt::new(ErrorOutputType::default());
    rustc_driver::init_rustc_env_logger(&early);
    rustc_driver::install_ice_hook("https://github.com/andreypfau/mcrs/issues", |_| ());
    rustc_driver::catch_with_exit_code(move || {
        let args: Vec<String> = rustc_driver::args::raw_args(&early)
            .into_iter()
            .skip(1)
            .collect();
        rustc_driver::run_compiler(&args, &mut LintCallbacks);
    })
}

fn run_rustc() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(rustc) = args.next() else {
        return ExitCode::FAILURE;
    };
    match Command::new(rustc).args(args).status() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .map_or(ExitCode::FAILURE, ExitCode::from),
        Err(_) => ExitCode::FAILURE,
    }
}
