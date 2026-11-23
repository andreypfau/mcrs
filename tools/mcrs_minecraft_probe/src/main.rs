mod operator;
mod vanilla_server;

use anyhow::Context;
use anyhow::bail;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::nbt_compress;
use mcrs_minecraft_probe::Host;
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
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use vanilla_server::VanillaServer;

const PLAYER_BANS: &str = "banned-players.json";
const PLAYER_DATA: &str = "world/players/data";
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
    fs::write(
        directory.join(PLAYER_BANS),
        serde_json::to_string_pretty(&scenario.target.banned_players)?,
    )?;
    let operators: Vec<Operator> = scenario
        .target
        .operators
        .iter()
        .map(|name| Operator::new(name, scenario.target.operator_level(name)))
        .collect();
    fs::write(
        directory.join("ops.json"),
        serde_json::to_string_pretty(&operators)?,
    )?;
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, free_port()?));
    let server = VanillaServer::start(&jar, &directory, address.port())?;
    let mut host = VanillaHost {
        server,
        jar,
        directory,
        port: address.port(),
    };

    let trace = Runtime::new()?.block_on(mcrs_minecraft_probe::run(
        &scenario, address, QUIET, &mut host,
    ));
    host.server.echo();
    let stopped = host.server.stop();
    let trace = trace?;
    stopped?;

    let mut text = serde_json::to_string_pretty(&trace)?;
    text.push('\n');
    fs::write(trace_path, text)?;
    Ok(())
}

struct VanillaHost {
    server: VanillaServer,
    jar: PathBuf,
    directory: PathBuf,
    port: u16,
}

impl Host for VanillaHost {
    fn read_player_bans(&mut self) -> anyhow::Result<Vec<PlayerBan>> {
        let text = fs::read_to_string(self.directory.join(PLAYER_BANS))?;
        Ok(serde_json::from_str(&text)?)
    }

    fn read_player_file(&mut self, player: &str) -> anyhow::Result<NbtCompound> {
        let path = self
            .directory
            .join(PLAYER_DATA)
            .join(format!("{}.dat", operator::offline_profile_id(player)));
        let file = fs::File::open(&path).with_context(|| format!("no player file at {path:?}"))?;
        Ok(nbt_compress::read_gzip_compound_tag(file)?)
    }

    fn console(&mut self, lines: &[String]) -> anyhow::Result<Vec<String>> {
        self.server.console(lines)
    }

    fn console_before(
        &mut self,
        lines: &[String],
        deadline: Instant,
    ) -> anyhow::Result<Vec<String>> {
        self.server.console_before(lines, deadline)
    }

    fn restart_server(&mut self) -> anyhow::Result<()> {
        self.server.echo();
        self.server.restart(&self.jar, &self.directory, self.port)
    }
}

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
