use std::fs::{self, File, OpenOptions};
use std::future::Future;
use std::io::Read;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::schedule::{self, Chunk, Queue, REST, Ranges, Source};
use crate::{Artifact, Directory, Files, Progress, RELEASE, Release, font_files, verify};

const INSTALLED_JAR: &str = "versions/26.3/26.3.jar";
const CHUNK: u64 = 1 << 20;
const READ: usize = 64 << 10;
// ponytail: a fixed pool; make it adaptive only if measurements show four connections are wrong.
pub const WORKERS: usize = 4;
const MIN_BACKOFF: Duration = if cfg!(test) {
    Duration::from_millis(10)
} else {
    Duration::from_secs(1)
};
const MAX_BACKOFF: Duration = Duration::from_secs(30);

#[cfg(not(windows))]
fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .expect("HOME is set")
}

/// The game directory the official launcher uses.
pub fn official_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    return home().join("Library/Application Support/minecraft");
    #[cfg(windows)]
    return std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .expect("APPDATA is set")
        .join(".minecraft");
    #[cfg(not(any(target_os = "macos", windows)))]
    return home().join(".minecraft");
}

/// Where launchers keep an installed 26.3 client jar, most likely first.
pub fn candidates() -> Vec<PathBuf> {
    #[allow(unused_mut)]
    let mut paths = vec![official_dir().join(INSTALLED_JAR)];
    #[cfg(target_os = "macos")]
    {
        let support = home().join("Library/Application Support");
        paths.push(
            support.join(
                "PrismLauncher/libraries/com/mojang/minecraft/26.3/minecraft-26.3-client.jar",
            ),
        );
        for settings in ["tlauncher-2.0.properties", "legacy.properties"] {
            let Ok(text) = fs::read_to_string(support.join("tlauncher").join(settings)) else {
                continue;
            };
            paths.extend(
                text.lines()
                    .filter_map(|line| line.strip_prefix("minecraft.gamedir="))
                    .map(|dir| Path::new(dir.trim()).join(INSTALLED_JAR)),
            );
        }
    }
    paths
}

/// The first of `paths` whose size and SHA-1 are the artifact's. Anything else, including a
/// patched or half-written jar, is skipped rather than trusted.
pub fn locate(paths: &[PathBuf], artifact: &Artifact, progress: &Progress) -> Option<Vec<u8>> {
    paths.iter().find_map(|path| {
        if fs::metadata(path).ok()?.len() != artifact.size {
            return None;
        }
        progress.set_total(artifact.size);
        progress.set_status(format!("Checking {}", path.display()));
        let bytes = read_counting(path, progress).ok()?;
        if !verify(artifact, &bytes) {
            tracing::warn!(path = %path.display(), expected = artifact.sha1, "skipping a file with the wrong SHA-1");
            return None;
        }
        tracing::info!(path = %path.display(), "using {}", artifact.url);
        Some(bytes)
    })
}

fn read_counting(path: &Path, progress: &Progress) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    progress.set_done(0);
    while (&mut file).take(CHUNK).read_to_end(&mut bytes)? > 0 {
        progress.set_done(bytes.len() as u64);
    }
    Ok(bytes)
}

/// The picked `assets/` files of the 26.3 client jar, from whichever launcher installed it or,
/// failing that, downloaded into the official launcher's directory. Font files go to `fonts`
/// first. A download returns once the assets are in; the [`Remainder`] completes the jar.
pub fn resolve(
    progress: &Progress,
    pick: impl FnMut(&str) -> bool,
    fonts: impl FnMut(Files),
) -> (Files, Option<Remainder>) {
    match locate(&candidates(), &RELEASE.jar, progress) {
        Some(jar) => (local(&jar, pick, fonts), None),
        None => download(
            &RELEASE,
            &official_dir().join("versions").join(RELEASE.id),
            WORKERS,
            progress,
            pick,
            fonts,
        ),
    }
}

fn local(jar: &[u8], pick: impl FnMut(&str) -> bool, mut fonts: impl FnMut(Files)) -> Files {
    const CORRUPT: &str = "a jar that matched its pinned SHA-1 is a readable zip";
    let directory = Directory::of(jar).expect(CORRUPT);
    fonts(font_files(&directory, &[(0, jar)]).expect(CORRUPT));
    directory.unpack(&[(0, jar)], pick).expect(CORRUPT)
}

/// Downloads the release's jar into `dir` as a sparse `.part` file, which a `.ranges` sidecar
/// makes resumable, and returns the picked assets once they are in.
pub fn download(
    release: &'static Release<'static>,
    dir: &Path,
    workers: usize,
    progress: &Progress,
    pick: impl FnMut(&str) -> bool,
    mut fonts: impl FnMut(Files),
) -> (Files, Option<Remainder>) {
    let jar = dir.join(format!("{}.jar", release.id));
    let part = with_suffix(&jar, ".part");
    let sidecar = with_suffix(&part, ".ranges");
    progress.set_status(format!("Opening {}", part.display()));
    let (file, held) = match retry(progress, || open(release, &jar, &part, &sidecar)) {
        Opened::Installed(bytes) => return (local(&bytes, pick, fonts), None),
        Opened::Part(file, held) => (file, held),
    };
    tracing::info!(part = %part.display(), ranges = ?held, "downloading {}", release.jar.url);
    let shared = Arc::new(Shared {
        file,
        url: release.jar.url.to_owned(),
        sidecar,
        queue: Mutex::new(Queue::resume(held)),
        changed: Condvar::new(),
        stop: AtomicBool::new(false),
    });
    let workers = (0..workers)
        .map(|id| {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name(format!("client jar {id}"))
                .spawn(move || work(&shared, id))
                .expect("a thread for a download worker")
        })
        .collect();
    let files = block_on(schedule::assets(
        &*shared, release, progress, pick, &mut fonts,
    ));
    shared.with_queue(|queue| queue.enqueue(REST, &[0..release.jar.size], false));
    let remainder = Remainder {
        shared,
        workers,
        release,
        jar,
        part,
    };
    (files, Some(remainder))
}

enum Opened {
    Installed(Vec<u8>),
    Part(File, Ranges),
}

fn open(release: &Release, jar: &Path, part: &Path, sidecar: &Path) -> Result<Opened, String> {
    let failed = |error: std::io::Error| format!("{}: {error}", part.display());
    if let Some(dir) = part.parent() {
        fs::create_dir_all(dir).map_err(failed)?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(part)
        .map_err(failed)?;
    file.lock().map_err(failed)?;
    if let Some(bytes) = locate(&[jar.to_owned()], &release.jar, &Progress::default()) {
        if file.metadata().is_ok_and(|metadata| metadata.len() == 0) {
            let _ = fs::remove_file(part);
        }
        return Ok(Opened::Installed(bytes));
    }
    let held = if file.metadata().map_err(failed)?.len() == release.jar.size {
        load_ranges(sidecar, release.jar.size)
    } else {
        file.set_len(release.jar.size).map_err(failed)?;
        Ranges::default()
    };
    Ok(Opened::Part(file, held))
}

struct Shared {
    file: File,
    url: String,
    sidecar: PathBuf,
    queue: Mutex<Queue>,
    changed: Condvar,
    stop: AtomicBool,
}

impl Source for Shared {
    fn with_queue<R>(&self, act: impl FnOnce(&mut Queue) -> R) -> R {
        let result = act(&mut self.queue.lock().unwrap());
        self.changed.notify_all();
        result
    }

    async fn wait(&self) {
        let queue = self.queue.lock().unwrap();
        drop(
            self.changed
                .wait_timeout(queue, Duration::from_millis(100))
                .unwrap(),
        );
    }

    fn read(&self, span: Range<u64>) -> Result<Vec<u8>, String> {
        let mut bytes = vec![0; (span.end - span.start) as usize];
        read_at(&self.file, &mut bytes, span.start).map_err(|error| error.to_string())?;
        Ok(bytes)
    }
}

impl Shared {
    fn next_chunk(&self) -> Option<Chunk> {
        let mut queue = self.queue.lock().unwrap();
        loop {
            if self.stop.load(Ordering::Relaxed) {
                return None;
            }
            if let Some(chunk) = queue.next() {
                return Some(chunk);
            }
            queue = self.changed.wait(queue).unwrap();
        }
    }

    fn stop(&self) {
        let queue = self.queue.lock().unwrap();
        self.stop.store(true, Ordering::Relaxed);
        drop(queue);
        self.changed.notify_all();
    }

    /// Writes what arrives for `range` at its own offset, advancing `reached` as it goes.
    fn fetch(
        &self,
        agent: &ureq::Agent,
        range: &Range<u64>,
        reached: &mut u64,
    ) -> Result<(), String> {
        let response = agent
            .get(&self.url)
            .header("Range", format!("bytes={}-{}", range.start, range.end - 1))
            .call()
            .map_err(|error| error.to_string())?;
        let mut position = match response.status().as_u16() {
            206 => response
                .headers()
                .get("content-range")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| {
                    value
                        .strip_prefix("bytes ")?
                        .split('-')
                        .next()?
                        .parse()
                        .ok()
                })
                .ok_or("a partial response without a readable Content-Range")?,
            _ => 0,
        };
        if position > range.start {
            return Err(format!(
                "asked for byte {} and got byte {position}",
                range.start
            ));
        }
        let mut body = response.into_body().into_reader();
        let mut buffer = vec![0; READ];
        while *reached < range.end {
            if self.stop.load(Ordering::Relaxed) {
                return Err("stopped".to_owned());
            }
            let read = body.read(&mut buffer).map_err(|error| error.to_string())?;
            if read == 0 {
                return Err(format!("the connection closed at byte {reached}"));
            }
            let from = (*reached).max(position);
            let to = (position + read as u64).min(range.end);
            if from < to {
                let bytes = &buffer[(from - position) as usize..(to - position) as usize];
                write_at(&self.file, bytes, from).map_err(|error| error.to_string())?;
                self.queue.lock().unwrap().wrote(from..to);
                *reached = to;
            }
            position += read as u64;
        }
        Ok(())
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_recv_response(Some(Duration::from_secs(15)))
        // ureq has no idle timeout, so a stalled socket would hang forever; capping each request
        // costs nothing because the next one resumes where this one stopped.
        .timeout_recv_body(Some(Duration::from_secs(30)))
        .build()
        .into()
}

fn work(shared: &Shared, id: usize) {
    let agent = agent();
    let mut backoff = MIN_BACKOFF;
    while let Some(chunk) = shared.next_chunk() {
        tracing::debug!(
            worker = id,
            group = chunk.group,
            start = chunk.range.start,
            end = chunk.range.end,
            "fetching"
        );
        let mut reached = chunk.range.start;
        let result = shared.fetch(&agent, &chunk.range, &mut reached);
        if reached > chunk.range.start {
            backoff = MIN_BACKOFF;
        }
        let failure = result
            .err()
            .map(|error| format!("{error}; retrying in {backoff:?}"));
        {
            let mut queue = shared.queue.lock().unwrap();
            queue.release(&chunk, reached);
            queue.error.clone_from(&failure);
            save_ranges(&shared.sidecar, queue.held());
        }
        shared.changed.notify_all();
        if let Some(failure) = failure {
            if shared.stop.load(Ordering::Relaxed) {
                return;
            }
            tracing::warn!(worker = id, "{failure}");
            std::thread::sleep(backoff);
            backoff = (backoff * 2).min(MAX_BACKOFF);
        }
    }
}

/// The part of a download still running after the assets are in: the rest of the jar, then
/// the move into place and the version JSON next to it. Dropping it stops the workers; the
/// sidecar keeps what they fetched.
pub struct Remainder {
    shared: Arc<Shared>,
    workers: Vec<JoinHandle<()>>,
    release: &'static Release<'static>,
    jar: PathBuf,
    part: PathBuf,
}

impl Remainder {
    pub fn finish(mut self) {
        let whole = [0..self.release.jar.size];
        let quiet = Progress::default();
        loop {
            block_on(schedule::until_held(&*self.shared, &whole, &quiet, ""));
            match self.shared.read(whole[0].clone()) {
                Ok(bytes) if verify(&self.release.jar, &bytes) => break,
                _ => {
                    tracing::warn!("the downloaded jar fails its SHA-1; fetching it again");
                    self.shared.with_queue(|queue| {
                        queue.forget(&whole);
                        queue.enqueue(REST, &whole, false);
                    });
                }
            }
        }
        self.shared.stop();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
        retry(&quiet, || {
            fs::rename(&self.part, &self.jar)
                .map_err(|error| format!("cannot move {} into place: {error}", self.jar.display()))
        });
        let _ = fs::remove_file(&self.shared.sidecar);
        tracing::info!(path = %self.jar.display(), "downloaded the client jar");
        write_json(
            self.release,
            &self.jar.with_file_name(format!("{}.json", self.release.id)),
            &quiet,
        );
    }
}

impl Drop for Remainder {
    fn drop(&mut self) {
        self.shared.stop();
    }
}

/// Writes the version JSON the official launcher expects next to the jar, unless a valid one
/// is already there.
fn write_json(release: &Release, path: &Path, progress: &Progress) {
    if locate(&[path.to_owned()], &release.json, progress).is_some() {
        return;
    }
    let agent = agent();
    let bytes = retry(progress, || {
        let bytes = agent
            .get(release.json.url)
            .call()
            .map_err(|error| error.to_string())?
            .into_body()
            .read_to_vec()
            .map_err(|error| error.to_string())?;
        if verify(&release.json, &bytes) {
            Ok(bytes)
        } else {
            Err(format!("{} fails its SHA-1", release.json.url))
        }
    });
    let temporary = with_suffix(path, ".part");
    retry(progress, || {
        fs::write(&temporary, &bytes)
            .and_then(|()| fs::rename(&temporary, path))
            .map_err(|error| format!("cannot write {}: {error}", path.display()))
    });
}

fn retry<T>(progress: &Progress, mut attempt: impl FnMut() -> Result<T, String>) -> T {
    let mut backoff = MIN_BACKOFF;
    loop {
        match attempt() {
            Ok(value) => return value,
            Err(error) => {
                let status = format!("{error}; retrying in {backoff:?}");
                tracing::warn!("{status}");
                progress.set_status(status);
                std::thread::sleep(backoff);
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
        }
    }
}

// The native source blocks inside `wait` instead of returning pending, so a single poll always
// completes and this never spins.
fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn load_ranges(path: &Path, size: u64) -> Ranges {
    let mut ranges = Ranges::default();
    for line in fs::read_to_string(path).unwrap_or_default().lines() {
        if let Some((start, end)) = line.split_once(' ')
            && let (Ok(start), Ok(end)) = (start.parse::<u64>(), end.parse::<u64>())
        {
            ranges.insert(start..end.min(size));
        }
    }
    ranges
}

fn save_ranges(path: &Path, ranges: &Ranges) {
    let text: String = ranges
        .iter()
        .map(|range| format!("{} {}\n", range.start, range.end))
        .collect();
    let temporary = with_suffix(path, ".tmp");
    if let Err(error) = fs::write(&temporary, text).and_then(|()| fs::rename(&temporary, path)) {
        tracing::warn!("cannot record the download in {}: {error}", path.display());
    }
}

#[cfg(unix)]
fn write_at(file: &File, bytes: &[u8], at: u64) -> std::io::Result<()> {
    std::os::unix::fs::FileExt::write_all_at(file, bytes, at)
}

#[cfg(unix)]
fn read_at(file: &File, bytes: &mut [u8], at: u64) -> std::io::Result<()> {
    std::os::unix::fs::FileExt::read_exact_at(file, bytes, at)
}

#[cfg(windows)]
fn write_at(file: &File, mut bytes: &[u8], mut at: u64) -> std::io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !bytes.is_empty() {
        let written = file.seek_write(bytes, at)?;
        bytes = &bytes[written..];
        at += written as u64;
    }
    Ok(())
}

#[cfg(windows)]
fn read_at(file: &File, mut bytes: &mut [u8], mut at: u64) -> std::io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !bytes.is_empty() {
        let read = file.seek_read(bytes, at)?;
        if read == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        bytes = &mut bytes[read..];
        at += read as u64;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::AtomicUsize;
    use std::time::Instant;

    use crate::fonts::{FontHint, is_texture};
    use crate::schedule::{ASSETS, DIRECTORY, FONTS, HINTED_FONTS, TEXTURES};
    use crate::tests::sample_jar;
    use crate::{CLIENT_JAR, entries, sha1_hex};

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mcrs_client_jar-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[derive(Clone, Copy, Default)]
    struct Behaviour {
        ignore_range: bool,
        cut_after: Option<usize>,
        delay: Duration,
    }

    struct Server {
        url: String,
        requests: Arc<Mutex<Vec<Range<u64>>>>,
        peak: Arc<AtomicUsize>,
    }

    fn serve(jar: &[u8], json: &[u8], behaviour: Behaviour) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let server = Server {
            url: format!("http://{}", listener.local_addr().unwrap()),
            requests: Arc::default(),
            peak: Arc::default(),
        };
        let (requests, peak) = (server.requests.clone(), server.peak.clone());
        let (jar, json) = (Arc::new(jar.to_vec()), Arc::new(json.to_vec()));
        let live = Arc::new(AtomicUsize::new(0));
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let (jar, json, requests, peak, live) = (
                    jar.clone(),
                    json.clone(),
                    requests.clone(),
                    peak.clone(),
                    live.clone(),
                );
                std::thread::spawn(move || {
                    peak.fetch_max(live.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
                    respond(stream.unwrap(), &jar, &json, behaviour, &requests);
                    live.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
        server
    }

    fn respond(
        mut stream: TcpStream,
        jar: &[u8],
        json: &[u8],
        behaviour: Behaviour,
        requests: &Mutex<Vec<Range<u64>>>,
    ) {
        let mut head = Vec::new();
        let mut byte = [0];
        while !head.ends_with(b"\r\n\r\n") {
            if stream.read(&mut byte).unwrap_or(0) == 0 {
                return;
            }
            head.push(byte[0]);
        }
        let head = String::from_utf8_lossy(&head).to_ascii_lowercase();
        let wants_json = head
            .split_whitespace()
            .nth(1)
            .unwrap_or("")
            .ends_with(".json");
        let body = if wants_json { json } else { jar };
        let range = head
            .lines()
            .find_map(|line| line.strip_prefix("range: bytes="))
            .and_then(|range| {
                let (start, end) = range.trim().split_once('-')?;
                Some(start.parse::<u64>().ok()?..end.parse::<u64>().ok()? + 1)
            });
        if !wants_json && let Some(range) = &range {
            requests.lock().unwrap().push(range.clone());
        }
        std::thread::sleep(behaviour.delay);
        let (status, start, end) = match range {
            Some(range) if !behaviour.ignore_range => (
                "206 Partial Content",
                range.start as usize,
                (range.end as usize).min(body.len()),
            ),
            _ => ("200 OK", 0, body.len()),
        };
        let header = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{}/{}\r\nConnection: close\r\n\r\n",
            end - start,
            end - 1,
            body.len()
        );
        let sent = behaviour
            .cut_after
            .map_or(end, |cut| (start + cut).min(end));
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(&body[start..sent]);
    }

    const JSON: &[u8] = br#"{"id":"26.3"}"#;

    fn leak(text: String) -> &'static str {
        Box::leak(text.into_boxed_str())
    }

    fn release(jar: &[u8], url: &str, hint: String) -> &'static Release<'static> {
        let start = Directory::of(jar).unwrap().start();
        let jar_url = leak(format!("{url}/client.jar"));
        Box::leak(Box::new(Release {
            id: "26.3",
            jar: Artifact {
                url: jar_url,
                sha1: leak(sha1_hex(jar)),
                size: jar.len() as u64,
            },
            directory: Artifact {
                url: jar_url,
                sha1: leak(sha1_hex(&jar[start as usize..])),
                size: jar.len() as u64 - start,
            },
            json: Artifact {
                url: leak(format!("{url}/26.3.json")),
                sha1: leak(sha1_hex(JSON)),
                size: JSON.len() as u64,
            },
            font_hint: leak(hint),
        }))
    }

    fn hint(jar: &[u8]) -> FontHint {
        FontHint::of(jar, &sha1_hex(jar)).unwrap()
    }

    fn hint_text(hint: &FontHint) -> String {
        serde_json::to_string(hint).unwrap()
    }

    fn font_names(files: &Files) -> Vec<&str> {
        let mut names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
        names.sort_unstable();
        names
    }

    fn assert_complete(dir: &Path, release: &Release) {
        assert!(verify(
            &release.jar,
            &fs::read(dir.join("26.3.jar")).unwrap()
        ));
        assert!(verify(
            &release.json,
            &fs::read(dir.join("26.3.json")).unwrap()
        ));
        assert!(!dir.join("26.3.jar.part").exists());
        assert!(!dir.join("26.3.jar.part.ranges").exists());
    }

    fn group_of(jar: &[u8], range: &Range<u64>) -> u32 {
        let directory = Directory::of(jar).unwrap();
        let within = |spans: Vec<Range<u64>>| spans.iter().any(|span| span.contains(&range.start));
        if within(hint(jar).spans()) {
            HINTED_FONTS
        } else if range.start >= directory.start() {
            DIRECTORY
        } else if within(directory.spans(is_texture)) {
            TEXTURES
        } else if within(directory.spans(|_| true)) {
            ASSETS
        } else {
            REST
        }
    }

    #[test]
    fn a_candidate_of_the_wrong_size_is_not_read() {
        let dir = scratch("wrong-size");
        let jar = dir.join("client.jar");
        fs::write(&jar, b"too short").unwrap();
        let sha1 = sha1_hex(b"client jar");
        let artifact = Artifact {
            url: "",
            sha1: &sha1,
            size: 10,
        };
        let progress = Progress::default();

        assert_eq!(locate(&[jar], &artifact, &progress), None);
        assert_eq!(progress.status(), "");
        assert_eq!(progress.total(), 0);
    }

    #[test]
    fn a_candidate_with_the_wrong_digest_is_skipped_for_the_next() {
        let dir = scratch("wrong-digest");
        let patched = dir.join("patched.jar");
        let genuine = dir.join("genuine.jar");
        fs::write(&patched, b"client jaR").unwrap();
        fs::write(&genuine, b"client jar").unwrap();
        let sha1 = sha1_hex(b"client jar");
        let artifact = Artifact {
            url: "",
            sha1: &sha1,
            size: 10,
        };
        let progress = Progress::default();

        let found = locate(
            &[dir.join("missing.jar"), patched, genuine],
            &artifact,
            &progress,
        );

        assert_eq!(found.as_deref(), Some(&b"client jar"[..]));
        assert_eq!(progress.done(), 10);
    }

    #[test]
    fn fonts_come_first_through_cut_connections_and_the_jar_completes_behind() {
        let jar = sample_jar();
        let server = serve(
            &jar,
            JSON,
            Behaviour {
                cut_after: Some(20_000),
                ..Behaviour::default()
            },
        );
        let release = release(&jar, &server.url, hint_text(&hint(&jar)));
        let dir = scratch("in-order");
        let progress = Arc::new(Progress::default());
        let watching = Arc::new(AtomicBool::new(true));
        let watcher = {
            let (progress, watching) = (progress.clone(), watching.clone());
            std::thread::spawn(move || {
                let mut seen = Vec::new();
                while watching.load(Ordering::Relaxed) {
                    seen.push(progress.status());
                    std::thread::sleep(Duration::from_millis(1));
                }
                seen
            })
        };
        let mut delivered = Vec::new();

        let (files, rest) = download(
            release,
            &dir,
            1,
            &progress,
            |_| true,
            |fonts| delivered.push(fonts),
        );
        watching.store(false, Ordering::Relaxed);
        rest.unwrap().finish();

        assert_eq!(files, entries(&jar, |_| true));
        assert_eq!(delivered.len(), 1);
        assert_eq!(
            font_names(&delivered[0]),
            [
                "minecraft/font/default.json",
                "minecraft/font/include/default.json",
                "minecraft/textures/font/ascii.png"
            ]
        );
        assert_complete(&dir, release);
        let groups: Vec<u32> = server
            .requests
            .lock()
            .unwrap()
            .iter()
            .map(|range| group_of(&jar, range))
            .collect();
        assert_eq!(groups[0], HINTED_FONTS, "{groups:?}");
        assert!(groups.is_sorted(), "{groups:?}");
        assert!(
            groups.contains(&TEXTURES) && groups.contains(&REST),
            "{groups:?}"
        );
        let seen = watcher.join().unwrap();
        assert!(
            seen.iter().any(|status| status.contains("retrying in")),
            "{seen:?}"
        );
    }

    #[test]
    fn several_workers_fetch_at_once() {
        let jar = sample_jar();
        let server = serve(
            &jar,
            JSON,
            Behaviour {
                delay: Duration::from_millis(100),
                ..Behaviour::default()
            },
        );
        let release = release(&jar, &server.url, hint_text(&hint(&jar)));
        let dir = scratch("parallel");

        let (files, rest) = download(release, &dir, 4, &Progress::default(), |_| true, drop);
        rest.unwrap().finish();

        assert_eq!(files, entries(&jar, |_| true));
        assert_complete(&dir, release);
        assert!(server.peak.load(Ordering::SeqCst) >= 3);
    }

    #[test]
    fn a_wrong_hint_falls_back_to_the_central_directory() {
        let jar = sample_jar();
        let fonts = font_files(&Directory::of(&jar).unwrap(), &[(0, &jar)]).unwrap();
        let server = serve(&jar, JSON, Behaviour::default());
        let wrong_crc = {
            let mut hint = hint(&jar);
            hint.fonts[0].crc32 ^= 1;
            hint_text(&hint)
        };
        let shifted = {
            let mut hint = hint(&jar);
            hint.fonts.iter_mut().for_each(|font| font.offset += 3);
            hint_text(&hint)
        };
        let overlong = {
            let mut hint = hint(&jar);
            hint.fonts[0].span += 7;
            hint_text(&hint)
        };
        let other_jar = {
            let mut hint = hint(&jar);
            hint.jar_sha1 = CLIENT_JAR.sha1.to_owned();
            hint_text(&hint)
        };
        for (name, hint, deliveries) in [
            ("wrong-crc", wrong_crc, 1),
            ("shifted", shifted, 1),
            ("overlong", overlong, 2),
            ("other-jar", other_jar, 1),
            ("garbled", "{".to_owned(), 1),
            ("missing", String::new(), 1),
        ] {
            let release = release(&jar, &server.url, hint);
            let dir = scratch(name);
            let mut delivered = Vec::new();

            let (files, rest) = download(
                release,
                &dir,
                2,
                &Progress::default(),
                |_| true,
                |f| delivered.push(f),
            );
            rest.unwrap().finish();

            assert_eq!(files, entries(&jar, |_| true), "{name}");
            assert_eq!(delivered.len(), deliveries, "{name}");
            assert_eq!(
                font_names(delivered.last().unwrap()),
                font_names(&fonts),
                "{name}"
            );
            assert_eq!(delivered.last(), Some(&fonts), "{name}");
            assert_complete(&dir, release);
        }
        assert!(FONTS > HINTED_FONTS);
    }

    #[test]
    fn a_server_ignoring_range_still_yields_the_jar() {
        let jar = sample_jar();
        let server = serve(
            &jar,
            JSON,
            Behaviour {
                ignore_range: true,
                ..Behaviour::default()
            },
        );
        let release = release(&jar, &server.url, hint_text(&hint(&jar)));
        let dir = scratch("no-range");

        let (files, rest) = download(release, &dir, 4, &Progress::default(), |_| true, drop);
        rest.unwrap().finish();

        assert_eq!(files, entries(&jar, |_| true));
        assert_complete(&dir, release);
    }

    #[test]
    fn a_corrupt_part_is_fetched_again() {
        let jar = sample_jar();
        let server = serve(&jar, JSON, Behaviour::default());
        let release = release(&jar, &server.url, hint_text(&hint(&jar)));
        let dir = scratch("corrupt");
        fs::write(dir.join("26.3.jar.part"), vec![0x5a; jar.len()]).unwrap();
        fs::write(
            dir.join("26.3.jar.part.ranges"),
            format!("0 {}\n", jar.len()),
        )
        .unwrap();

        let (files, rest) = download(release, &dir, 2, &Progress::default(), |_| true, drop);
        rest.unwrap().finish();

        assert_eq!(files, entries(&jar, |_| true));
        assert_complete(&dir, release);
    }

    #[test]
    fn a_restart_resumes_from_the_sidecar() {
        let jar = sample_jar();
        let server = serve(
            &jar,
            JSON,
            Behaviour {
                delay: Duration::from_millis(20),
                ..Behaviour::default()
            },
        );
        let release = release(&jar, &server.url, hint_text(&hint(&jar)));
        let dir = scratch("restart");

        let (first, rest) = download(release, &dir, 2, &Progress::default(), |_| true, drop);
        drop(rest);
        let before = server.requests.lock().unwrap().len();
        let held = load_ranges(&dir.join("26.3.jar.part.ranges"), jar.len() as u64);
        let mut delivered = Vec::new();
        let (second, rest) = download(
            release,
            &dir,
            2,
            &Progress::default(),
            |_| true,
            |f| delivered.push(f),
        );
        rest.unwrap().finish();

        assert_eq!(first, second);
        assert_eq!(delivered.len(), 1);
        assert!(held.covers(&Directory::of(&jar).unwrap().spans(|_| true)));
        let again: Vec<u32> = server.requests.lock().unwrap()[before..]
            .iter()
            .map(|range| group_of(&jar, range))
            .collect();
        assert!(again.iter().all(|&group| group == REST), "{again:?}");
        assert_complete(&dir, release);
    }

    #[test]
    fn the_font_hint_matches_the_pinned_jar() {
        let jar = locate(&candidates(), &CLIENT_JAR, &Progress::default()).expect(
            "a 26.3 client jar installed by a launcher, or downloaded by one run of the client",
        );
        let generated = FontHint::of(&jar, CLIENT_JAR.sha1).unwrap();
        if std::env::var_os("MCRS_WRITE_FONT_HINT").is_some() {
            let text = serde_json::to_string_pretty(&generated).unwrap() + "\n";
            fs::write(
                concat!(env!("CARGO_MANIFEST_DIR"), "/src/font_hint.json"),
                text,
            )
            .unwrap();
        }
        assert_eq!(
            serde_json::from_str::<FontHint>(RELEASE.font_hint).unwrap(),
            generated,
            "src/font_hint.json is stale; rerun this test with MCRS_WRITE_FONT_HINT=1"
        );
    }

    #[test]
    #[ignore = "reads the launchers installed on this machine"]
    fn locates_a_launcher_jar_on_this_machine() {
        let candidates = candidates();
        let progress = Progress::default();
        let found = locate(&candidates, &CLIENT_JAR, &progress);
        println!("checked {candidates:#?}");
        println!("{}", progress.status());
        assert!(found.is_some());
    }

    #[test]
    #[ignore = "downloads the 26.3 client jar from Mojang into the temp directory"]
    fn downloads_the_pinned_jar() {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .try_init();
        let workers = std::env::var("MCRS_JAR_WORKERS")
            .ok()
            .and_then(|workers| workers.parse().ok())
            .unwrap_or(WORKERS);
        let dir = std::env::temp_dir().join("mcrs-client-jar-download");
        if dir.join("26.3.jar").exists() {
            fs::remove_dir_all(&dir).unwrap();
        }
        let held = load_ranges(&dir.join("26.3.jar.part.ranges"), RELEASE.jar.size);
        println!(
            "{} holds {} bytes before the run: {held:?}",
            dir.display(),
            held.iter()
                .map(|range| range.end - range.start)
                .sum::<u64>()
        );
        let started = Instant::now();
        let progress = Progress::default();

        let (files, rest) = download(
            &RELEASE,
            &dir,
            workers,
            &progress,
            |_| true,
            |fonts| {
                println!(
                    "fonts: {} files after {:?}, {} of {} bytes in",
                    fonts.len(),
                    started.elapsed(),
                    progress.done(),
                    progress.total()
                )
            },
        );
        println!(
            "ready: {} asset files after {:?} with {workers} workers, {} bytes of the jar needed",
            files.len(),
            started.elapsed(),
            progress.total()
        );
        rest.unwrap().finish();
        println!("complete after {:?}", started.elapsed());

        assert_complete(&dir, &RELEASE);
    }
}
