use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;
use std::time::Instant;

use regex::Regex;

use crate::handle::Handle;

const MAX_LINE: usize = 400;

static ANSI: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"\x1b\[[0-9;]*[A-Za-z]").ok());

pub fn lines(path: &Path, from: usize) -> Vec<String> {
    let bytes = std::fs::read(path).unwrap_or_default();
    let text = String::from_utf8_lossy(bytes.get(from..).unwrap_or_default());
    text.lines().map(clean).collect()
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

pub fn wait(handle: &Handle, pattern: &Regex, timeout: Duration) -> Waited {
    let start = Instant::now();
    let log = handle.log();
    loop {
        let all = lines(&log, 0);
        if let Some(line) = all.iter().find(|line| pattern.is_match(line)) {
            return Waited::Matched(line.clone());
        }
        if !handle.alive() {
            return Waited::Exited(tail(&all, None, 20));
        }
        if start.elapsed() >= timeout {
            return Waited::TimedOut(tail(&all, None, 20));
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
