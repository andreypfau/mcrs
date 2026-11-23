use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;
use std::time::Instant;

use regex::Regex;

use crate::handle::Handle;

const MAX_LINE: usize = 400;

static ANSI: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"\x1b\[[0-9;]*[A-Za-z]").ok());

pub fn lines(path: &Path, from: usize) -> Vec<String> {
    String::from_utf8_lossy(&read_from(path, from))
        .lines()
        .map(clean)
        .collect()
}

fn read_from(path: &Path, from: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    let Ok(mut file) = File::open(path) else {
        return bytes;
    };
    let offset = u64::try_from(from).unwrap_or(u64::MAX);
    if file.seek(SeekFrom::Start(offset)).is_ok() {
        file.read_to_end(&mut bytes).ok();
    }
    bytes
}

pub fn len(path: &Path) -> usize {
    std::fs::metadata(path).map_or(0, |metadata| {
        usize::try_from(metadata.len()).unwrap_or(usize::MAX)
    })
}

fn clean(line: &str) -> String {
    let line = match &*ANSI {
        Some(ansi) => ansi.replace_all(line, "").into_owned(),
        None => line.to_owned(),
    };
    match line.char_indices().nth(MAX_LINE) {
        Some((end, _)) => format!("{}…", &line[..end]),
        None => line,
    }
}

pub fn tail(lines: &[String], filter: Option<&Regex>, limit: usize) -> Vec<String> {
    let matching: Vec<&String> = lines
        .iter()
        .filter(|line| filter.is_none_or(|filter| filter.is_match(line)))
        .collect();
    let skip = matching.len().saturating_sub(limit);
    matching.into_iter().skip(skip).cloned().collect()
}

pub enum Waited {
    Matched(String),
    TimedOut(Vec<String>),
    Exited(Vec<String>),
}

pub fn wait(handle: &Handle, pattern: &Regex, from: usize, timeout: Duration) -> Waited {
    let start = Instant::now();
    let log = handle.log();
    let mut from = from;
    loop {
        let bytes = read_from(&log, from);
        let complete = bytes
            .iter()
            .rposition(|&byte| byte == b'\n')
            .map_or(0, |end| end + 1);
        from += complete;
        let text = String::from_utf8_lossy(bytes.get(..complete).unwrap_or_default());
        if let Some(line) = text.lines().map(clean).find(|line| pattern.is_match(line)) {
            return Waited::Matched(line);
        }
        if !handle.alive() {
            return Waited::Exited(tail(&lines(&log, 0), None, 20));
        }
        if start.elapsed() >= timeout {
            return Waited::TimedOut(tail(&lines(&log, 0), None, 20));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub fn replies(
    handle: &Handle,
    from: usize,
    reply: impl Fn(&str) -> Option<String>,
    window: Duration,
) -> Vec<String> {
    let start = Instant::now();
    let quiet = Duration::from_millis(300);
    let mut found = Vec::new();
    let mut last_change = start;
    while start.elapsed() < window {
        std::thread::sleep(Duration::from_millis(50));
        let now: Vec<String> = lines(&handle.log(), from)
            .into_iter()
            .filter_map(|line| reply(&line))
            .collect();
        if now.len() != found.len() {
            found = now;
            last_change = Instant::now();
        } else if !found.is_empty() && last_change.elapsed() >= quiet {
            break;
        }
    }
    found.truncate(20);
    found
}
