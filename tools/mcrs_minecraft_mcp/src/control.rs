use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::time::Duration;
use std::time::Instant;

use mcrs_minecraft_client_control::Reply;
use mcrs_minecraft_client_control::Request;

use crate::handle::Handle;

const POLL: Duration = Duration::from_millis(200);

const SECONDS_PER_TICK: f64 = 0.05;

pub struct Session {
    stream: BufReader<UnixStream>,
}

impl Session {
    pub fn open(handle: &Handle) -> Result<Self, String> {
        let path = handle
            .control
            .as_ref()
            .ok_or_else(|| format!("{} has no control socket", handle.id))?;
        let start = Instant::now();
        loop {
            match UnixStream::connect(path) {
                Ok(stream) => {
                    return Ok(Self {
                        stream: BufReader::new(stream),
                    });
                }
                Err(_) if handle.alive() && start.elapsed() < Duration::from_secs(60) => {
                    std::thread::sleep(POLL);
                }
                Err(error) => return Err(format!("{}: {error}", path.display())),
            }
        }
    }

    pub fn ask(&mut self, line: &str, timeout: Duration) -> Result<String, String> {
        let stream = self.stream.get_mut();
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|error| error.to_string())?;
        writeln!(stream, "{}", line.trim()).map_err(|error| error.to_string())?;
        let mut reply = String::new();
        match self.stream.read_line(&mut reply) {
            Ok(0) => Err("the client closed the control socket".to_owned()),
            Ok(_) => Ok(reply.trim_end().to_owned()),
            Err(error) => Err(format!("no reply to {line:?}: {error}")),
        }
    }
}

pub fn timeout_of(line: &str) -> Duration {
    let ticks = match Request::parse(line) {
        Ok(Request::Hold { ticks, .. } | Request::Attack { ticks }) => f64::from(ticks),
        _ => 1.0,
    };
    Duration::from_secs_f64(10.0 + ticks * SECONDS_PER_TICK * 4.0)
}

pub fn wait_until(
    handle: &Handle,
    request: &str,
    timeout: Duration,
    reached: fn(&Reply) -> bool,
) -> Result<String, String> {
    let mut session = Session::open(handle)?;
    let start = Instant::now();
    loop {
        let reply = session.ask(request, Duration::from_secs(10))?;
        let parsed: Reply = serde_json::from_str(&reply).map_err(|error| error.to_string())?;
        if reached(&parsed) {
            return Ok(reply);
        }
        if start.elapsed() > timeout {
            return Err(format!("timeout; last state: {reply}"));
        }
        std::thread::sleep(POLL);
    }
}

pub fn world_ready(reply: &Reply) -> bool {
    matches!(reply, Reply::Ready(ready) if ready.world_ready)
}

pub fn on_ground(reply: &Reply) -> bool {
    matches!(reply, Reply::Player(player) if player.on_ground)
}
