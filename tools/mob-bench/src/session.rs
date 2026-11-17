use std::net::SocketAddr;
use std::time::{Duration, Instant};

use mcrs_minecraft_bench_support::{ServerProcess, answers_status, mcrs_tick_millis};

use crate::probe;
use crate::scene::WATCHER_NAME;
use crate::watcher::Watcher;

pub const READY_TIMEOUT: Duration = Duration::from_secs(600);
pub const CONSOLE_TIMEOUT: Duration = Duration::from_secs(20);
pub const JOIN_TIMEOUT: Duration = Duration::from_secs(60);

pub fn wait_ready(
    server: &mut ServerProcess,
    runtime: &tokio::runtime::Runtime,
    address: SocketAddr,
    started: Instant,
) -> Option<Duration> {
    loop {
        server.pump();
        if server.exited() {
            return None;
        }
        if runtime.block_on(answers_status(address)) {
            return Some(started.elapsed());
        }
        if started.elapsed() > READY_TIMEOUT {
            return None;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

pub fn idle(server: &mut ServerProcess, duration: Duration) {
    let until = Instant::now() + duration;
    while Instant::now() < until {
        server.pump();
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub fn answered(server: &mut ServerProcess, line: &str) -> Option<String> {
    let sent = Instant::now();
    let _ = server.send(line);
    server.wait_for(sent, Duration::from_secs(1), |text| {
        mcrs_tick_millis(text).is_none().then(|| text.to_owned())
    })
}

pub fn game_time(server: &mut ServerProcess) -> Option<i64> {
    let sent = Instant::now();
    server.send("time query gametime").ok()?;
    server.wait_for(sent, CONSOLE_TIMEOUT, probe::game_time)
}

pub fn join_watcher(
    server: &mut ServerProcess,
    runtime: &tokio::runtime::Runtime,
    address: SocketAddr,
) -> anyhow::Result<Watcher> {
    let watcher = Watcher::spawn(runtime, address);
    let deadline = Instant::now() + JOIN_TIMEOUT;
    while Instant::now() < deadline {
        if watcher.finished() {
            let log = watcher.stop(runtime);
            anyhow::bail!(
                log.error
                    .unwrap_or_else(|| "the watcher stopped".to_owned())
            );
        }
        if watcher.placed(runtime) {
            return Ok(watcher);
        }
        idle(server, Duration::from_millis(500));
    }
    let _ = watcher.stop(runtime);
    anyhow::bail!("{WATCHER_NAME} was not placed in {JOIN_TIMEOUT:?}")
}
