//! Records the client ticks of a vanilla client for one experiment of
//! `docs/spec/entity-physics.md`, section 15. It starts the vanilla server, listens as a proxy
//! in front of it, sets each step of the scenario up through the server console and writes one
//! recording per step. See the README.

mod cheat;
mod console;
mod delay;
mod names;
mod play;
mod proxy;
mod scenario;
mod session;
#[cfg(test)]
mod tests;

use std::env;
use std::fs;
use std::io;
use std::net::TcpListener;
use std::net::TcpStream;
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::Context;
use anyhow::bail;

use crate::cheat::Module;
use crate::console::Console;
use crate::names::Names;
use crate::proxy::Connection;
use crate::scenario::Scenario;
use crate::session::Session;

/// The port the client connects to: the default port of the game.
const LISTEN_PORT: u16 = 25565;
const SERVER_PORT: u16 = 25568;

#[expect(
    clippy::disallowed_methods,
    reason = "record.sh passes the input file and the cheat module in the environment"
)]
fn main() -> anyhow::Result<()> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let usage =
        "usage: mcrs_minecraft_client_recorder SCENARIO SERVER_DIRECTORY OUT_DIRECTORY [TRACE]";
    let [scenario, server, out, rest @ ..] = arguments.as_slice() else {
        bail!(usage);
    };
    let trace = match rest {
        [] => Vec::new(),
        [trace] => delay::parse_trace(
            &fs::read_to_string(trace).with_context(|| format!("could not read {trace}"))?,
        )?,
        _ => bail!(usage),
    };
    let cheat: Option<Module> = env::var("CHEAT")
        .ok()
        .map(|name| name.parse())
        .transpose()?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let names = Names::load(&root)?;
    let mut scenario: Scenario = fs::read_to_string(scenario)
        .with_context(|| format!("could not read {scenario}"))?
        .parse()?;
    if cheat.is_some() {
        // The vanilla server kicks a player that floats; the recording would end at the kick.
        scenario.properties.push("allow-flight=true".to_owned());
    }
    let mut console = Console::start(Path::new(server), SERVER_PORT, &scenario.properties)?;
    for command in [
        "gamerule spawn_mobs false",
        "difficulty peaceful",
        "time set noon",
    ] {
        console.run(command)?;
    }
    let input = env::var_os("INPUT_FILE").map(PathBuf::from);
    let mut session = Session::new(
        console,
        scenario,
        PathBuf::from(out),
        input,
        cheat.is_some(),
    );
    let listener = TcpListener::bind(("127.0.0.1", LISTEN_PORT))?;
    listener.set_nonblocking(true)?;
    println!("connect the vanilla client to localhost:{LISTEN_PORT}");

    let (sender, terminal) = mpsc::channel();
    thread::spawn(move || {
        for line in io::stdin().lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                return;
            }
        }
    });

    let mut connection: Option<Connection> = None;
    let mut observed = Vec::new();
    while !session.is_done() {
        if connection.is_none() {
            connection = accept(&listener, &trace, cheat)?;
        }
        if let Some(open) = connection.as_mut() {
            let alive = open.poll(&names, &mut observed)?;
            for item in observed.drain(..) {
                session.observe(item)?;
            }
            if !alive {
                if !session.may_disconnect() {
                    bail!("the client left during a step");
                }
                connection = None;
            }
        }
        session.console.echo();
        if terminal.try_iter().count() > 0 || session.is_scripted() {
            session.enter()?;
        }
    }
    session.console.stop()
}

fn accept(
    listener: &TcpListener,
    trace: &[Duration],
    cheat: Option<Module>,
) -> anyhow::Result<Option<Connection>> {
    match listener.accept() {
        Ok((client, _)) => {
            client.set_nonblocking(false)?;
            let server = TcpStream::connect(("127.0.0.1", SERVER_PORT))?;
            Ok(Some(Connection::new(client, server, trace, cheat)?))
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
            thread::sleep(Duration::from_millis(5));
            Ok(None)
        }
        Err(error) => Err(error.into()),
    }
}
