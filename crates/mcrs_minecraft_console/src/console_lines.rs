use std::io::BufRead;

use mcrs_minecraft_client_jar::{Directory, Progress, RELEASE, candidates, locate};

use crate::language::{EN_US, Language};

/// What the console thread hands the server: first the language of the answers, then each
/// command line.
pub(crate) enum ConsoleInput {
    Language(Language),
    Line(String),
}

/// Starts the thread that reads the language and then the standard input, line by line, until
/// the input ends or the server stops listening.
pub(crate) fn spawn_console_thread(send: flume::Sender<ConsoleInput>) {
    let started = std::thread::Builder::new()
        .name("console".to_owned())
        .spawn(move || read_console(&send));
    if let Err(error) = started {
        tracing::error!(%error, "the console did not start; no command line runs from the standard input");
    }
}

fn read_console(send: &flume::Sender<ConsoleInput>) {
    if send.send(ConsoleInput::Language(english())).is_err() {
        return;
    }
    send_lines(std::io::stdin().lock(), send);
}

/// Sends each command line of `input`. A line that is not UTF-8 is read with the replacement
/// character, so that one bad byte does not end the console.
fn send_lines(input: impl BufRead, send: &flume::Sender<ConsoleInput>) {
    for bytes in input.split(b'\n') {
        let Ok(bytes) = bytes else {
            return;
        };
        let line = String::from_utf8_lossy(&bytes);
        let Some(command) = command_of(&line) else {
            continue;
        };
        if send.send(ConsoleInput::Line(command.to_owned())).is_err() {
            return;
        }
    }
}

/// The command of a console line: the line without its surrounding whitespace and one leading
/// slash. A blank line holds no command.
fn command_of(line: &str) -> Option<&str> {
    let line = line.trim();
    let command = line.strip_prefix('/').unwrap_or(line);
    (!command.is_empty()).then_some(command)
}

/// The English language of the installed client jar. Without the jar, an answer reads as its
/// translation key and its arguments.
fn english() -> Language {
    let Some(jar) = locate(&candidates(), &RELEASE.jar, &Progress::default()) else {
        tracing::warn!("no client jar; the console writes translation keys, not English");
        return Language::default();
    };
    let read = Directory::of(&jar)
        .and_then(|directory| directory.read(&jar, |name| name == EN_US))
        .and_then(|files| {
            let (_, bytes) = files
                .into_iter()
                .next()
                .ok_or_else(|| format!("the client jar holds no {EN_US}"))?;
            Language::from_json(&bytes).map_err(|error| error.to_string())
        });
    read.unwrap_or_else(|error| {
        tracing::warn!(%error, "the console writes translation keys, not English");
        Language::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_that_is_not_utf8_does_not_end_the_console() {
        let (send, receive) = flume::unbounded();
        send_lines(&b"say \xff\r\n\n  /list \nkill"[..], &send);
        let lines: Vec<String> = receive
            .try_iter()
            .filter_map(|input| match input {
                ConsoleInput::Line(line) => Some(line),
                ConsoleInput::Language(_) => None,
            })
            .collect();
        assert_eq!(lines, ["say \u{fffd}", "list", "kill"]);
    }
}
