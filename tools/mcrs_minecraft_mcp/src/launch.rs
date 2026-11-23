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
use crate::operators;

pub fn build(workdir: &Path, targets: &[&str]) -> Result<String, String> {
    let start = Instant::now();
    for target in targets {
        let args: &[&str] = match *target {
            "server" => &["-p", "mcrs", "--bin", "mcrs"],
            "client" => &[
                "-p",
                "mcrs_minecraft_client",
                "--bin",
                "mcrs_minecraft_client",
                "--features",
                "control",
            ],
            other => return Err(format!("unknown target {other}: use server or client")),
        };
        let manifest =
            std::fs::read_to_string(workdir.join("crates/mcrs_minecraft_client/Cargo.toml"));
        if *target == "client"
            && !manifest.is_ok_and(|text| text.lines().any(|line| line.starts_with("control =")))
        {
            return Err(format!(
                "the client of {} has no feature control; build the client from a checkout that has it",
                workdir.display()
            ));
        }
        let output = Command::new("cargo")
            .current_dir(workdir)
            .args(["build", "--release"])
            .args(args)
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(first_errors(&String::from_utf8_lossy(&output.stderr)));
        }
    }
    Ok(format!("ok in {} s", start.elapsed().as_secs()))
}

fn first_errors(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.lines().collect();
    let first = lines
        .iter()
        .position(|line| line.starts_with("error"))
        .unwrap_or(0);
    lines
        .iter()
        .skip(first)
        .take(30)
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

pub struct ServerOptions<'a> {
    pub world: Option<&'a str>,
    pub seed: Option<&'a str>,
    pub gamemode: Option<&'a str>,
    pub ops: Vec<&'a str>,
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
        workdir,
        args: options.world.map(str::to_owned).into_iter().collect(),
        env,
        port: Some(port),
        control: None,
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
    let write = |name: &str, text: &str| {
        std::fs::write(run.path.join(name), text).map_err(|error| error.to_string())
    };
    write("eula.txt", "eula=true\n")?;
    write("server.properties", &vanilla_properties(port, options))?;
    write("ops.json", &operators::operators_json(&options.ops))?;
    supervisor.spawn(Launch {
        kind: Kind::Vanilla,
        program: PathBuf::from("java"),
        workdir,
        args: ["-Xmx2G", "-jar", &jar, "nogui"]
            .map(str::to_owned)
            .to_vec(),
        env: Vec::new(),
        port: Some(port),
        control: None,
        run: &run,
    })
}

fn vanilla_properties(port: u16, options: &ServerOptions<'_>) -> String {
    format!(
        "online-mode=false\nwhite-list=false\nenforce-whitelist=false\nserver-ip=127.0.0.1\nserver-port={port}\n\
level-seed={}\ngamemode={}\nforce-gamemode=true\nview-distance=6\nsimulation-distance=6\nspawn-protection=0\n\
enforce-secure-profile=false\nenable-rcon=false\nenable-query=false\n",
        options.seed.unwrap_or("42"),
        options.gamemode.unwrap_or("creative"),
    )
}

pub fn client(
    supervisor: &mut Supervisor,
    workdir: &Path,
    server: &Handle,
    options: ClientOptions<'_>,
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
    env.extend(
        options
            .username
            .map(|name| ("MCRS_USERNAME".to_owned(), name.to_owned())),
    );
    env.extend(options.env);
    let control = run.path.join("control.sock");
    let mut args = vec!["--control".to_owned(), control.display().to_string()];
    if options.headless {
        args.push("--headless".to_owned());
    }
    supervisor.spawn(Launch {
        kind: Kind::Client,
        program: workdir.join("target/release/mcrs_minecraft_client"),
        workdir,
        args,
        env,
        port: None,
        control: Some(control),
        run: &run,
    })
}

pub struct ClientOptions<'a> {
    pub username: Option<&'a str>,
    pub headless: bool,
    pub env: Vec<(String, String)>,
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

#[cfg(test)]
mod tests {
    use mcrs_minecraft_network::client::offline_player_uuid;
    use serde_json::Value;

    use super::*;

    #[test]
    fn a_local_offline_client_can_join_vanilla_and_run_commands() {
        let options = ServerOptions {
            world: None,
            seed: None,
            gamemode: None,
            ops: vec!["Notch", "Bob"],
            env: Vec::new(),
        };
        let properties = vanilla_properties(25565, &options);
        for line in [
            "online-mode=false",
            "white-list=false",
            "enforce-secure-profile=false",
            "spawn-protection=0",
        ] {
            assert!(
                properties.lines().any(|have| have == line),
                "{line} missing:\n{properties}"
            );
        }
        let operators: Value =
            serde_json::from_str(&operators::operators_json(&options.ops)).unwrap();
        for (operator, name) in operators.as_array().unwrap().iter().zip(options.ops) {
            assert_eq!(operator["name"], name);
            assert_eq!(operator["uuid"], offline_player_uuid(name).to_string());
            assert_eq!(operator["level"], 4);
        }
    }
}
