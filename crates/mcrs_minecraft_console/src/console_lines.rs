use std::io;
use std::io::BufRead;

/// Starts the thread that reads the standard input, line by line, until the input ends or the
/// server stops listening.
pub(crate) fn spawn_console_thread(send: flume::Sender<String>) -> io::Result<()> {
    std::thread::Builder::new()
        .name("console".to_owned())
        .spawn(move || send_lines(io::stdin().lock(), &send))?;
    Ok(())
}

/// Sends each command line of `input`. A line that is not UTF-8 is read with the replacement
/// character, so that one bad byte does not end the console.
fn send_lines(input: impl BufRead, send: &flume::Sender<String>) {
    for bytes in input.split(b'\n') {
        let Ok(bytes) = bytes else {
            return;
        };
        let line = String::from_utf8_lossy(&bytes);
        let Some(command) = command_of(&line) else {
            continue;
        };
        if send.send(command.to_owned()).is_err() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_that_is_not_utf8_does_not_end_the_console() {
        let (send, receive) = flume::unbounded();
        send_lines(&b"say \xff\r\n\n  /list \nkill"[..], &send);
        let lines: Vec<String> = receive.try_iter().collect();
        assert_eq!(lines, ["say \u{fffd}", "list", "kill"]);
    }
}
