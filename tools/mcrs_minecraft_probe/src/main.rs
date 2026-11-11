//! Records the trace of one scenario against the vanilla dedicated server.

mod operator;
mod vanilla_server;

use anyhow::Context;
use anyhow::bail;
use mcrs_minecraft_probe::PlayerBan;
use mcrs_minecraft_probe::Scenario;
use operator::Operator;
use std::env;
use std::fs;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tokio::runtime::Runtime;
use vanilla_server::VanillaServer;

const PLAYER_BANS: &str = "banned-players.json";
// Each recorded trace is made with this quiet window.
const QUIET: Duration = Duration::from_secs(1);

fn main() -> anyhow::Result<()> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let [jar, scenario_path, trace_path] = arguments.as_slice() else {
        bail!("usage: mcrs_minecraft_probe <server.jar> <scenario.json> <trace.json>");
    };
    let jar = fs::canonicalize(jar).with_context(|| format!("no server jar at {jar}"))?;
    let scenario: Scenario = serde_json::from_str(&fs::read_to_string(scenario_path)?)
        .with_context(|| format!("{scenario_path} is not a scenario"))?;

    let directory = work_directory(&jar)?;
    let bans_path = directory.join(PLAYER_BANS);
    fs::write(
        &bans_path,
        serde_json::to_string_pretty(&scenario.target.banned_players)?,
    )?;
    let operators: Vec<Operator> = scenario
        .target
        .operators
        .iter()
        .map(|name| Operator::new(name))
        .collect();
    fs::write(
        directory.join("ops.json"),
        serde_json::to_string_pretty(&operators)?,
    )?;
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, free_port()?));
    let server = VanillaServer::start(&jar, &directory, address.port())?;

    let trace =
        Runtime::new()?.block_on(mcrs_minecraft_probe::run(&scenario, address, QUIET, || {
            let bans: Vec<PlayerBan> = serde_json::from_str(&fs::read_to_string(&bans_path)?)?;
            Ok(bans)
        }));
    server.echo();
    server.stop()?;

    let mut text = serde_json::to_string_pretty(&trace?)?;
    text.push('\n');
    fs::write(trace_path, text)?;
    Ok(())
}

/// An empty directory beside the jar, so that each recording starts from a new world and new
/// player lists.
fn work_directory(jar: &Path) -> anyhow::Result<PathBuf> {
    let directory = jar
        .parent()
        .context("the server jar has no directory")?
        .join("probe");
    if directory.exists() {
        fs::remove_dir_all(&directory)?;
    }
    fs::create_dir_all(&directory)?;
    Ok(directory)
}

fn free_port() -> anyhow::Result<u16> {
    Ok(TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?
        .local_addr()?
        .port())
}
