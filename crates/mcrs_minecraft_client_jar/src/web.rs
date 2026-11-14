use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashSet};
use std::ops::Range;
use std::rc::Rc;

use js_sys::{Array, Promise, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, future_to_promise, spawn_local};
use web_sys::{
    Cache, CacheStorage, Headers, ReadableStreamDefaultReader, Request, RequestInit, Response,
};

use crate::schedule::{self, Queue, Source};
use crate::{AssetObject, FONT_OBJECTS, Files, Progress, RELEASE};

const WORKERS: usize = 4;
const CACHE: &str = "mcrs-client-jar";
const STALL_MS: i32 = 30_000;
const OBJECT_BODY_MS: i32 = 60_000;
const IDLE_MS: i32 = 50;
const MIN_BACKOFF_MS: i32 = 1_000;
const MAX_BACKOFF_MS: i32 = 30_000;
const OBJECT_ATTEMPTS: u32 = 4;
/// The longest wait for the font objects; after it the text keeps to the jar's glyphs.
const FONT_DEADLINE_MS: i32 = 120_000;

/// The picked `assets/` files of the pinned client jar, fetched in priority order by Range
/// requests and kept per chunk in Cache Storage. The rest of the jar is never fetched.
pub async fn fetch(
    progress: &Progress,
    pick: impl FnMut(&str) -> bool,
    fonts: impl FnMut(Files),
) -> Files {
    let source = Rc::new(WebSource {
        queue: RefCell::default(),
        pieces: RefCell::default(),
        served: RefCell::default(),
        stop: Cell::new(false),
        whole: Cell::new(false),
        cache: open_cache().await,
    });
    for id in 0..WORKERS {
        spawn_local(work(source.clone(), id));
    }
    let files = schedule::assets(&*source, &RELEASE, progress, pick, fonts).await;
    source.stop.set(true);
    files
}

/// The asset index layer's font files, laid over the jar's by path. The browser's HTTP cache
/// keeps them: an object's address is its digest, so a hit is never stale. The objects are
/// fetched side by side, each retried with a growing wait.
pub async fn font_assets() -> Result<Files, String> {
    let promises = Array::new();
    for object in FONT_OBJECTS {
        promises.push(&future_to_promise(async move {
            fetch_object(object)
                .await
                .map(|bytes| Uint8Array::from(bytes.as_slice()).into())
                .map_err(JsValue::from)
        }));
    }
    let bodies = within(Promise::all(&promises), FONT_DEADLINE_MS).await?;
    Ok(FONT_OBJECTS
        .iter()
        .zip(Array::from(&bodies).iter())
        .map(|(object, body)| (object.path.to_owned(), Uint8Array::new(&body).to_vec()))
        .collect())
}

async fn fetch_object(object: &AssetObject<'_>) -> Result<Vec<u8>, String> {
    let mut backoff = MIN_BACKOFF_MS;
    let mut failure = String::new();
    for attempt in 0..OBJECT_ATTEMPTS {
        if attempt > 0 {
            sleep(backoff).await;
            backoff = (backoff * 2).min(MAX_BACKOFF_MS);
        }
        match object_body(object).await {
            Ok(bytes) if object.holds(&bytes) => return Ok(bytes),
            Ok(_) => failure = "it is not the pinned object".to_owned(),
            Err(error) => failure = error,
        }
    }
    Err(format!("{}: {failure}", object.url()))
}

async fn object_body(object: &AssetObject<'_>) -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or("no window")?;
    // Browser fetch has no timeout of its own, so each wait is raced against a timer.
    let response: Response = within(window.fetch_with_str(&object.url()), STALL_MS)
        .await?
        .unchecked_into();
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    let buffer = within(response.array_buffer().map_err(describe)?, OBJECT_BODY_MS).await?;
    Ok(Uint8Array::new(&buffer).to_vec())
}

/// The promise's value, or an error when it neither settles nor fails within `ms`.
async fn within(promise: Promise, ms: i32) -> Result<JsValue, String> {
    let settled = JsFuture::from(Promise::race(&Array::of2(&promise, &timeout(ms))))
        .await
        .map_err(describe)?;
    if settled.is_undefined() {
        return Err("the download stalled".to_owned());
    }
    Ok(settled)
}

struct WebSource {
    queue: RefCell<Queue>,
    pieces: RefCell<BTreeMap<u64, Vec<u8>>>,
    served: RefCell<HashSet<String>>,
    stop: Cell<bool>,
    whole: Cell<bool>,
    cache: Option<Cache>,
}

impl Source for WebSource {
    fn with_queue<R>(&self, act: impl FnOnce(&mut Queue) -> R) -> R {
        act(&mut self.queue.borrow_mut())
    }

    async fn wait(&self) {
        sleep(IDLE_MS).await;
    }

    fn read(&self, span: Range<u64>) -> Result<Vec<u8>, String> {
        let mut bytes = vec![0; (span.end - span.start) as usize];
        let mut filled = 0;
        for (&start, piece) in self.pieces.borrow().range(..span.end) {
            let end = start + piece.len() as u64;
            if end <= span.start {
                continue;
            }
            let (from, to) = (start.max(span.start), end.min(span.end));
            let target = bytes.get_mut((from - span.start) as usize..(to - span.start) as usize);
            let source = piece.get((from - start) as usize..(to - start) as usize);
            if let (Some(target), Some(source)) = (target, source) {
                target.copy_from_slice(source);
                filled += to - from;
            }
        }
        if filled == span.end - span.start {
            Ok(bytes)
        } else {
            Err(format!("bytes {span:?} are not all held"))
        }
    }
}

impl WebSource {
    fn trim(&self, cut: &Range<u64>) {
        let mut pieces = self.pieces.borrow_mut();
        let overlapping: Vec<u64> = pieces
            .range(..cut.end)
            .filter(|(start, piece)| **start + piece.len() as u64 > cut.start)
            .map(|(start, _)| *start)
            .collect();
        for start in overlapping {
            let Some(piece) = pieces.remove(&start) else {
                continue;
            };
            let end = start + piece.len() as u64;
            if start < cut.start
                && let Some(head) = piece.get(..(cut.start - start) as usize)
            {
                pieces.insert(start, head.to_vec());
            }
            if end > cut.end
                && let Some(tail) = piece.get((cut.end - start) as usize..)
            {
                pieces.insert(cut.end, tail.to_vec());
            }
        }
    }

    fn keep(&self, at: u64, bytes: Vec<u8>) {
        let range = at..at + bytes.len() as u64;
        self.trim(&range);
        self.pieces.borrow_mut().insert(at, bytes);
        self.queue.borrow_mut().wrote(range);
    }

    async fn fetch(&self, range: &Range<u64>, reached: &mut u64) -> Result<(), String> {
        let key = format!("{}?bytes={}-{}", RELEASE.jar.url, range.start, range.end);
        // A chunk asked for twice in one session was dropped as corrupt, so its cached copy
        // is not trusted again.
        let first = self.served.borrow_mut().insert(key.clone());
        if first
            && let Some(cache) = &self.cache
            && let Some(bytes) = cached(cache, &key).await
            && bytes.len() as u64 == range.end - range.start
        {
            self.keep(range.start, bytes);
            *reached = range.end;
            return Ok(());
        }

        let headers = Headers::new().map_err(describe)?;
        headers
            .set("Range", &format!("bytes={}-{}", range.start, range.end - 1))
            .map_err(describe)?;
        let init = RequestInit::new();
        init.set_method("GET");
        init.set_headers(&headers);
        let request = Request::new_with_str_and_init(RELEASE.jar.url, &init).map_err(describe)?;
        let window = web_sys::window().ok_or("no window")?;
        let response: Response = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(describe)?
            .unchecked_into();
        // The CDN answers the If-Range Chrome sends for a partly cached jar with the whole jar;
        // one worker keeps all of it while the others wait instead of fetching it again.
        let (mut position, end) = match response.status() {
            206 => (range.start, range.end),
            200 if self.whole.get() => {
                if let Some(body) = response.body() {
                    let _ = body.cancel();
                }
                return Ok(());
            }
            200 => (0, RELEASE.jar.size),
            status => return Err(format!("HTTP {status}")),
        };
        let whole = end != range.end;
        let _streaming = whole.then(|| Streaming::start(&self.whole));
        let reader: ReadableStreamDefaultReader = response
            .body()
            .ok_or("a response without a body")?
            .get_reader()
            .unchecked_into();
        while position < end {
            if self.stop.get() && !whole {
                let _ = reader.cancel();
                return Err("stopped".to_owned());
            }
            let read = JsFuture::from(Promise::race(&Array::of2(
                &reader.read(),
                &timeout(STALL_MS),
            )))
            .await
            .map_err(describe)?;
            if read.is_undefined() {
                let _ = reader.cancel();
                return Err("the download stalled".to_owned());
            }
            if Reflect::get(&read, &"done".into())
                .map_err(describe)?
                .as_bool()
                == Some(true)
            {
                return Err(format!("the connection closed at byte {position}"));
            }
            let mut data =
                Uint8Array::new(&Reflect::get(&read, &"value".into()).map_err(describe)?).to_vec();
            let to = (position + data.len() as u64).min(end);
            data.truncate((to - position) as usize);
            if data.is_empty() {
                continue;
            }
            self.keep(position, data);
            position = to;
            *reached = position.clamp(range.start, range.end);
        }
        let _ = reader.cancel();
        if let Some(cache) = self.cache.clone() {
            let mut bytes = self.read(range.clone())?;
            spawn_local(async move {
                let stored = Response::new_with_opt_u8_array(Some(&mut bytes))
                    .map(|response| cache.put_with_str(&key, &response));
                if let Ok(put) = stored
                    && let Err(error) = JsFuture::from(put).await
                {
                    tracing::warn!("cannot cache {key}: {}", describe(error));
                }
            });
        }
        Ok(())
    }
}

struct Streaming<'a>(&'a Cell<bool>);

impl<'a> Streaming<'a> {
    fn start(flag: &'a Cell<bool>) -> Self {
        flag.set(true);
        Self(flag)
    }
}

impl Drop for Streaming<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

async fn work(source: Rc<WebSource>, id: usize) {
    let mut backoff = MIN_BACKOFF_MS;
    while !source.stop.get() {
        if source.whole.get() {
            sleep(IDLE_MS).await;
            continue;
        }
        let Some(chunk) = source.queue.borrow_mut().next() else {
            sleep(IDLE_MS).await;
            continue;
        };
        if source
            .queue
            .borrow()
            .holds(std::slice::from_ref(&chunk.range))
        {
            continue;
        }
        tracing::debug!(
            worker = id,
            group = chunk.group,
            start = chunk.range.start,
            end = chunk.range.end,
            "fetching"
        );
        let mut reached = chunk.range.start;
        let result = source.fetch(&chunk.range, &mut reached).await;
        if reached > chunk.range.start {
            backoff = MIN_BACKOFF_MS;
        }
        let failure = result
            .err()
            .map(|error| format!("{error}; retrying in {} s", backoff / 1000));
        {
            let mut queue = source.queue.borrow_mut();
            queue.release(&chunk, reached);
            queue.error.clone_from(&failure);
        }
        if let Some(failure) = failure {
            tracing::warn!(worker = id, "{failure}");
            sleep(backoff).await;
            backoff = (backoff * 2).min(MAX_BACKOFF_MS);
        }
    }
}

// Outside a secure context `caches` is absent, and touching it as if it were there would trap.
async fn open_cache() -> Option<Cache> {
    let window = web_sys::window()?;
    let caches = Reflect::get(&window, &"caches".into())
        .ok()
        .filter(|caches| !caches.is_undefined())?
        .unchecked_into::<CacheStorage>();
    match JsFuture::from(caches.open(CACHE)).await {
        Ok(cache) => Some(cache.unchecked_into()),
        Err(error) => {
            tracing::warn!("no cache for the client jar: {}", describe(error));
            None
        }
    }
}

async fn cached(cache: &Cache, key: &str) -> Option<Vec<u8>> {
    let hit = JsFuture::from(cache.match_with_str(key)).await.ok()?;
    if hit.is_undefined() {
        return None;
    }
    let buffer = hit.unchecked_into::<Response>().array_buffer().ok()?;
    let buffer = JsFuture::from(buffer).await.ok()?;
    Some(Uint8Array::new(&buffer).to_vec())
}

fn timeout(ms: i32) -> Promise {
    Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    })
}

async fn sleep(ms: i32) {
    let _ = JsFuture::from(timeout(ms)).await;
}

fn describe(error: JsValue) -> String {
    error
        .as_string()
        .or_else(|| {
            error
                .dyn_ref::<js_sys::Error>()
                .map(|error| String::from(error.message()))
        })
        .unwrap_or_else(|| format!("{error:?}"))
}
