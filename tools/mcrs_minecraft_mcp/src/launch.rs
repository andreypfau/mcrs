use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use serde_json::Value;

use crate::handle::Handle;
use crate::handle::Kind;
use crate::handle::Launch;
use crate::handle::RunDir;
use crate::handle::Supervisor;

pub fn build(workdir: &Path, targets: &[&str]) -> Result<String, String> {
    let mut command = Command::new("cargo");
    command.current_dir(workdir).args(["build", "--release"]);
    for target in targets {
        let (package, bin) = match *target {
            "server" => ("mcrs", "mcrs"),
            "client" => ("mcrs_minecraft_client", "mcrs_minecraft_client"),
            other => return Err(format!("unknown target {other}: use server or client")),
        };
        command.args(["-p", package, "--bin", bin]);
    }
    let start = Instant::now();
    let output = command.output().map_err(|error| error.to_string())?;
    if output.status.success() {
        return Ok(format!("ok in {} s", start.elapsed().as_secs()));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let lines: Vec<&str> = stderr.lines().collect();
    let first = lines
        .iter()
        .position(|line| line.starts_with("error"))
        .unwrap_or(0);
    Err(lines
        .iter()
        .skip(first)
        .take(30)
        .copied()
        .collect::<Vec<_>>()
        .join("\n"))
}

pub struct ServerOptions<'a> {
    pub world: Option<&'a str>,
    pub seed: Option<&'a str>,
    pub gamemode: Option<&'a str>,
    pub env: Vec<(String, String)>,
}

pub fn server(
    supervisor: &mut Supervisor,
    workdir: &Path,
    options: ServerOptions<'_>,
) -> Result<Handle, String> {
    let run = RunDir::create(Kind::Server)?;
    let port = crate::handle::free_port()?;
    let mut env = vec![
        asset_root(workdir),
        ("MCRS_PORT".to_owned(), port.to_string()),
    ];
    env.extend(
        options
            .seed
            .map(|seed| ("MCRS_WORLD_SEED".to_owned(), seed.to_owned())),
    );
    env.extend(
        options
            .gamemode
            .map(|mode| ("MCRS_DEFAULT_GAMEMODE".to_owned(), mode.to_owned())),
    );
    env.extend(options.env);
    supervisor.spawn(Launch {
        kind: Kind::Server,
        program: workdir.join("target/release/mcrs"),
        args: options.world.map(str::to_owned).into_iter().collect(),
        env,
        port: Some(port),
        run: &run,
    })
}

pub fn vanilla(
    supervisor: &mut Supervisor,
    workdir: &Path,
    options: &ServerOptions<'_>,
) -> Result<Handle, String> {
    let fetch = Command::new(workdir.join("tools/vanilla-server/run.sh"))
        .arg("fetch")
        .output()
        .map_err(|error| error.to_string())?;
    let jar = String::from_utf8_lossy(&fetch.stdout).trim().to_owned();
    if !fetch.status.success() || jar.is_empty() {
        return Err(format!(
            "the vanilla server jar is not available: {}",
            String::from_utf8_lossy(&fetch.stderr).trim()
        ));
    }
    let run = RunDir::create(Kind::Vanilla)?;
    let port = crate::handle::free_port()?;
    let properties = format!(
        "online-mode=false\nserver-ip=127.0.0.1\nserver-port={port}\nlevel-seed={}\ngamemode={}\n\
force-gamemode=true\nview-distance=6\nsimulation-distance=6\nspawn-protection=0\n\
enforce-secure-profile=false\nenable-rcon=false\nenable-query=false\n",
        options.seed.unwrap_or("42"),
        options.gamemode.unwrap_or("creative"),
    );
    let write = |name: &str, text: &str| {
        std::fs::write(run.path.join(name), text).map_err(|error| error.to_string())
    };
    write("eula.txt", "eula=true\n")?;
    write("server.properties", &properties)?;
    supervisor.spawn(Launch {
        kind: Kind::Vanilla,
        program: PathBuf::from("java"),
        args: ["-Xmx2G", "-jar", &jar, "nogui"]
            .map(str::to_owned)
            .to_vec(),
        env: Vec::new(),
        port: Some(port),
        run: &run,
    })
}

pub fn client(
    supervisor: &mut Supervisor,
    workdir: &Path,
    server: &Handle,
    username: Option<&str>,
    extra: Vec<(String, String)>,
) -> Result<Handle, String> {
    let port = server
        .port
        .ok_or_else(|| format!("{} is not a server", server.id))?;
    let run = RunDir::create(Kind::Client)?;
    let dir = run.path.display().to_string();
    let mut env = vec![
        asset_root(workdir),
        ("MCRS_SERVER".to_owned(), format!("127.0.0.1:{port}")),
        ("MCRS_GAME_DIR".to_owned(), dir.clone()),
        (
            "MCRS_SCREENSHOT_DIR".to_owned(),
            format!("{dir}/screenshots"),
        ),
    ];
    env.extend(username.map(|name| ("MCRS_USERNAME".to_owned(), name.to_owned())));
    env.extend(extra);
    supervisor.spawn(Launch {
        kind: Kind::Client,
        program: workdir.join("target/release/mcrs_minecraft_client"),
        args: Vec::new(),
        env,
        port: None,
        run: &run,
    })
}

pub fn env_of(value: &Value) -> Vec<(String, String)> {
    value.as_object().map_or_else(Vec::new, |map| {
        map.iter()
            .map(|(key, value)| {
                let text = value
                    .as_str()
                    .map_or_else(|| value.to_string(), str::to_owned);
                (key.clone(), text)
            })
            .collect()
    })
}

fn asset_root(workdir: &Path) -> (String, String) {
    ("BEVY_ASSET_ROOT".to_owned(), workdir.display().to_string())
}
