use std::io;
use std::io::Read;
use std::io::Write;
use std::os::unix::net::UnixListener;
use std::os::unix::net::UnixStream;

use bevy_ecs::prelude::*;
use bevy_log::error;
use bevy_log::info;
use mcrs_minecraft_client_world::movement_input::MovementInput;

use crate::controlled_client::Button;
use crate::controlled_client::Chat;
use crate::controlled_client::ControlledClient;
use crate::controlled_client::Missing;
use crate::listener;
use crate::reply::Reply;
use crate::reply::Target;
use crate::request::Request;

const MAX_LINE: usize = 4096;

const READ_CHUNK: usize = 1024;

#[derive(Resource)]
pub struct Control {
    listener: UnixListener,
    address: String,
    connection: Option<Connection>,
    reply: Vec<u8>,
}

struct Connection {
    stream: UnixStream,
    received: Vec<u8>,
    pending: Option<Pending>,
}

enum Pending {
    Hold {
        keys: MovementInput,
        ticks_left: u32,
    },
    Attack {
        step: u32,
        ticks: u32,
        attacked: Target,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Peer {
    Open,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Line {
    Complete(usize),
    Partial,
    TooLong,
}

impl Control {
    pub fn bind(address: &str) -> io::Result<Self> {
        let listener = listener::bind(address)?;
        info!(address, "control socket");
        Ok(Self {
            listener,
            address: address.to_owned(),
            connection: None,
            reply: Vec::new(),
        })
    }
}

impl Drop for Control {
    fn drop(&mut self) {
        if let Err(error) = listener::remove(&self.address) {
            error!(address = self.address, %error, "the control socket stays on disk");
        }
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the allocations happen only when a request line arrives or an attack request selects its target"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(9))]
pub(crate) fn serve(mut control: ResMut<Control>, mut client: ControlledClient) {
    let control = &mut *control;
    if control.connection.is_none() {
        let Ok((stream, _)) = control.listener.accept() else {
            return;
        };
        if stream.set_nonblocking(true).is_err() {
            return;
        }
        control.connection = Some(Connection {
            stream,
            received: Vec::new(),
            pending: None,
        });
    }
    let Some(connection) = control.connection.as_mut() else {
        return;
    };
    if !matches!(
        step(connection, &mut client, &mut control.reply),
        Ok(Peer::Open)
    ) {
        client.release();
        control.connection = None;
    }
}

struct Output<'a> {
    stream: &'a mut UnixStream,
    reply: &'a mut Vec<u8>,
}

impl Output<'_> {
    fn send(&mut self, value: &Reply) -> io::Result<()> {
        self.reply.clear();
        serde_json::to_writer(&mut *self.reply, value).map_err(io::Error::other)?;
        self.reply.push(b'\n');
        self.stream.write_all(self.reply)
    }

    fn fail(&mut self, error: &str) -> io::Result<()> {
        self.send(&Reply::Error(error.to_owned()))
    }
}

fn step(
    connection: &mut Connection,
    client: &mut ControlledClient,
    reply: &mut Vec<u8>,
) -> io::Result<Peer> {
    let peer = read_available(&mut connection.stream, &mut connection.received)?;
    let mut output = Output {
        stream: &mut connection.stream,
        reply,
    };
    if let Some(pending) = connection.pending.take() {
        connection.pending = advance(pending, client, &mut output)?;
        return Ok(Peer::Open);
    }
    match next_line(&connection.received) {
        Line::Complete(end) => {
            match connection
                .received
                .get(..end)
                .map(core::str::from_utf8)
                .unwrap_or(Ok(""))
            {
                Ok(line) => connection.pending = handle(line, client, &mut output)?,
                Err(_) => output.fail("the request is not UTF-8")?,
            }
            connection.received.drain(..=end);
            Ok(Peer::Open)
        }
        Line::TooLong => {
            output.fail("the request line is too long")?;
            Err(io::ErrorKind::InvalidData.into())
        }
        Line::Partial => Ok(peer),
    }
}

fn next_line(received: &[u8]) -> Line {
    match received.iter().position(|byte| *byte == b'\n') {
        Some(end) if end <= MAX_LINE => Line::Complete(end),
        Some(_) => Line::TooLong,
        None if received.len() > MAX_LINE => Line::TooLong,
        None => Line::Partial,
    }
}

fn advance(
    pending: Pending,
    client: &mut ControlledClient,
    output: &mut Output<'_>,
) -> io::Result<Option<Pending>> {
    let next = match pending {
        Pending::Hold { ticks_left: 0, .. } => {
            client.hold(MovementInput::EMPTY);
            output.send(&Reply::Done)?;
            None
        }
        Pending::Hold { keys, ticks_left } => {
            client.hold(keys);
            Some(Pending::Hold {
                keys,
                ticks_left: ticks_left.saturating_sub(1),
            })
        }
        Pending::Attack {
            step,
            ticks,
            attacked,
        } if step > ticks => {
            client.press(Button::Up);
            output.send(&Reply::Attacked(attacked))?;
            None
        }
        Pending::Attack {
            step,
            ticks,
            attacked,
        } => {
            let button = match step {
                0 => Button::Up,
                1 => Button::Click,
                _ => Button::Down,
            };
            client.press(button);
            let attacked = match button {
                Button::Click => client.target().unwrap_or(Target::None),
                Button::Up | Button::Down => attacked,
            };
            Some(Pending::Attack {
                step: step.saturating_add(1),
                ticks,
                attacked,
            })
        }
    };
    Ok(next)
}

fn read_available(stream: &mut UnixStream, into: &mut Vec<u8>) -> io::Result<Peer> {
    let mut chunk = [0; READ_CHUNK];
    while into.len() <= MAX_LINE * 4 {
        match stream.read(&mut chunk) {
            Ok(0) => return Ok(Peer::Closed),
            Ok(read) => into.extend_from_slice(chunk.get(..read).unwrap_or_default()),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(Peer::Open),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(Peer::Open)
}

fn handle(
    line: &str,
    client: &mut ControlledClient,
    output: &mut Output<'_>,
) -> io::Result<Option<Pending>> {
    let request = match Request::parse(line) {
        Ok(request) => request,
        Err(error) => {
            output.fail(&error.to_string())?;
            return Ok(None);
        }
    };
    let answer = match request {
        Request::Ready => Ok(Reply::Ready(client.ready())),
        Request::Player => client.player().map(Reply::Player),
        Request::Target => client.target().map(Reply::Target),
        Request::Entities { radius } => client.entities(radius).map(Reply::Entities),
        Request::Block(pos) => client.block(pos).map(Reply::Block),
        Request::Look { yaw, pitch } => client.look(yaw, pitch).map(|()| Reply::Done),
        Request::LookAt(point) => client.look_at(point).map(|()| Reply::Done),
        Request::LookEntity(id) => client.look_at_entity(id).map(|()| Reply::Done),
        Request::Slot(slot) => client.select_slot(slot).map(|()| Reply::Done),
        Request::Chat(text) => client.send(text, Chat::Chat).map(|()| Reply::Done),
        Request::Command(line) => client.send(line, Chat::Command).map(|()| Reply::Done),
        Request::Hold { keys, ticks } => {
            return Ok(Some(Pending::Hold {
                keys,
                ticks_left: ticks,
            }));
        }
        Request::Attack { ticks } => {
            return Ok(Some(Pending::Attack {
                step: 0,
                ticks,
                attacked: Target::None,
            }));
        }
    };
    output
        .send(&answer.unwrap_or_else(|missing: Missing| Reply::Error(missing.text().to_owned())))?;
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_first_line_counts_toward_the_line_limit() {
        let pipelined = "player\n".repeat(MAX_LINE);
        let oversized = "x".repeat(MAX_LINE + 1);
        let cases: [(&str, &str, Line); 5] = [
            ("one line", "ready\n", Line::Complete(5)),
            ("a partial line", "rea", Line::Partial),
            (
                "a pipelined batch over the limit",
                &pipelined,
                Line::Complete(6),
            ),
            (
                "an oversized line without its end",
                &oversized,
                Line::TooLong,
            ),
            (
                "an oversized line with its end",
                &format!("{oversized}\nready\n"),
                Line::TooLong,
            ),
        ];
        for (name, received, expected) in cases {
            assert_eq!(next_line(received.as_bytes()), expected, "{name}");
        }
    }
}
