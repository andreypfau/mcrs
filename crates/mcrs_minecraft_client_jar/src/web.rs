use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashSet};
use std::ops::Range;
use std::rc::Rc;

use js_sys::{Array, Promise, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    Cache, CacheStorage, Headers, ReadableStreamDefaultReader, Request, RequestInit, Response,
};

use crate::schedule::{self, Queue, Source};
use crate::{Files, Progress, RELEASE};

// ponytail: a fixed pool; make it adaptive only if measurements show four connections are wrong.
const WORKERS: usize = 4;
const CACHE: &str = "mcrs-client-jar";
const STALL_MS: i32 = 30_000;
const IDLE_MS: i32 = 50;
const MIN_BACKOFF_MS: i32 = 1_000;
const MAX_BACKOFF_MS: i32 = 30_000;

/// The picked `assets/` files of the 26.3 client jar, fetched in priority order by Range
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
        cache: open_cache().await,
    });
    for id in 0..WORKERS {
        spawn_local(work(source.clone(), id));
    }
    let files = schedule::assets(&*source, &RELEASE, progress, pick, fonts).await;
    source.stop.set(true);
    files
}

struct WebSource {
    queue: RefCell<Queue>,
    pieces: RefCell<BTreeMap<u64, Vec<u8>>>,
    served: RefCell<HashSet<String>>,
    stop: Cell<bool>,
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
            bytes[(from - span.start) as usize..(to - span.start) as usize]
                .copy_from_slice(&piece[(from - start) as usize..(to - start) as usize]);
            filled += to - from;
        }
        if filled == span.end - span.start {
            Ok(bytes)
        } else {
            Err(format!("bytes {span:?} are not all held"))
        }
    }
}

impl WebSource {
    /// Drops whatever an earlier fetch left inside `cut`, which is about to be fetched again.
    fn trim(&self, cut: &Range<u64>) {
        let mut pieces = self.pieces.borrow_mut();
        let overlapping: Vec<u64> = pieces
            .range(..cut.end)
            .filter(|(start, piece)| **start + piece.len() as u64 > cut.start)
            .map(|(start, _)| *start)
            .collect();
        for start in overlapping {
            let piece = pieces.remove(&start).unwrap();
            let end = start + piece.len() as u64;
            if start < cut.start {
                pieces.insert(start, piece[..(cut.start - start) as usize].to_vec());
            }
            if end > cut.end {
                pieces.insert(cut.end, piece[(cut.end - start) as usize..].to_vec());
            }
        }
    }

    fn keep(&self, at: u64, bytes: Vec<u8>) {
        let range = at..at + bytes.len() as u64;
        self.pieces.borrow_mut().insert(at, bytes);
        self.queue.borrow_mut().wrote(range);
    }

    async fn fetch(&self, range: &Range<u64>, reached: &mut u64) -> Result<(), String> {
        let key = format!("{}?bytes={}-{}", RELEASE.jar.url, range.start, range.end);
        self.trim(range);
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
        let mut position = match response.status() {
            206 => range.start,
            200 => 0,
            status => return Err(format!("HTTP {status}")),
        };
        let reader: ReadableStreamDefaultReader = response
            .body()
            .ok_or("a response without a body")?
            .get_reader()
            .unchecked_into();
        while *reached < range.end {
            if self.stop.get() {
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
                return Err(format!("the connection closed at byte {reached}"));
            }
            let data =
                Uint8Array::new(&Reflect::get(&read, &"value".into()).map_err(describe)?).to_vec();
            let from = (*reached).max(position);
            let to = (position + data.len() as u64).min(range.end);
            if from < to {
                self.keep(
                    from,
                    data[(from - position) as usize..(to - position) as usize].to_vec(),
                );
                *reached = to;
            }
            position += data.len() as u64;
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

async fn work(source: Rc<WebSource>, id: usize) {
    let mut backoff = MIN_BACKOFF_MS;
    while !source.stop.get() {
        let Some(chunk) = source.queue.borrow_mut().next() else {
            sleep(IDLE_MS).await;
            continue;
        };
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
