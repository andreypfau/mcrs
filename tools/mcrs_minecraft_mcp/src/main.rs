#![expect(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::indexing_slicing,
    reason = "MCP messages and tool arguments are open JSON objects, and indexing a JSON value yields null instead of a panic"
)]

mod control;
mod handle;
mod launch;
mod log_file;
mod rpc;
mod screenshot;
mod tools;

use std::process::ExitCode;

use serde_json::Value;

use crate::handle::Supervisor;
use crate::tools::Output;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1).peekable();
    #[expect(
        clippy::disallowed_methods,
        reason = "the session of a command-line call comes from the shell of the agent"
    )]
    let session = args
        .next_if_eq("--session")
        .and_then(|_| args.next())
        .or_else(|| std::env::var("MCRS_MCP_SESSION").ok());
    let args: Vec<String> = args.collect();
    let workdir = std::env::current_dir().unwrap_or_default();
    let Some(tool) = args.first() else {
        let mut supervisor = match Supervisor::serving(workdir) {
            Ok(supervisor) => supervisor,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
        };
        rpc::serve(&tools::list(), |name, arguments| {
            match tools::call(&mut supervisor, name, arguments) {
                Ok(output) => output.to_mcp(false),
                Err(error) => Output::Text(error).to_mcp(true),
            }
        });
        return ExitCode::SUCCESS;
    };
    if tool == "help" || tool == "--help" {
        for tool in tools::list().as_array().into_iter().flatten() {
            println!("{}: {}", tool["name"], tool["description"]);
        }
        return ExitCode::SUCCESS;
    }
    let arguments: Value = match args.get(1).map(|json| serde_json::from_str(json)) {
        None => Value::Null,
        Some(Ok(arguments)) => arguments,
        Some(Err(error)) => {
            eprintln!("the arguments are not JSON: {error}");
            return ExitCode::FAILURE;
        }
    };
    match tools::call(
        &mut Supervisor::command_line(workdir, session),
        tool,
        &arguments,
    ) {
        Ok(Output::Text(text)) => println!("{text}"),
        Ok(Output::Image { path, .. }) => println!("{}", path.display()),
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
