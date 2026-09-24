use std::collections::BTreeSet;
use std::ops::Range;

use crate::fonts::{FontHint, font_textures, is_font_definition, is_texture};
use crate::{Directory, Files, Progress, Release, verify};

pub(crate) const HINTED_FONTS: u32 = 0;
pub(crate) const DIRECTORY: u32 = 1;
pub(crate) const FONTS: u32 = 2;
pub(crate) const TEXTURES: u32 = 3;
pub(crate) const ASSETS: u32 = 4;
pub(crate) const REST: u32 = 5;

const CHUNK: u64 = 1 << 20;

/// Disjoint, sorted byte ranges.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Ranges(Vec<Range<u64>>);

impl Ranges {
    pub(crate) fn iter(&self) -> impl Iterator<Item = &Range<u64>> {
        self.0.iter()
    }

    pub(crate) fn insert(&mut self, range: Range<u64>) {
        if range.is_empty() {
            return;
        }
        let mut joined = range;
        self.0.retain(|held| {
            let apart = held.end < joined.start || held.start > joined.end;
            if !apart {
                joined = joined.start.min(held.start)..joined.end.max(held.end);
            }
            apart
        });
        let at = self.0.partition_point(|held| held.start < joined.start);
        self.0.insert(at, joined);
    }

    pub(crate) fn remove(&mut self, cut: &Range<u64>) {
        self.0 = self
            .0
            .iter()
            .flat_map(|held| {
                [
                    held.start..held.end.min(cut.start),
                    held.start.max(cut.end)..held.end,
                ]
            })
            .filter(|piece| !piece.is_empty())
            .collect();
    }

    pub(crate) fn gaps(&self, within: &Range<u64>) -> Vec<Range<u64>> {
        let mut gaps = Vec::new();
        let mut at = within.start;
        for held in &self.0 {
            if held.end <= at {
                continue;
            }
            if held.start >= within.end {
                break;
            }
            if held.start > at {
                gaps.push(at..held.start);
            }
            at = held.end;
        }
        if at < within.end {
            gaps.push(at..within.end);
        }
        gaps
    }

    pub(crate) fn covers(&self, spans: &[Range<u64>]) -> bool {
        spans.iter().all(|span| self.gaps(span).is_empty())
    }

    fn len(&self) -> u64 {
        self.0.iter().map(|range| range.end - range.start).sum()
    }

    fn covered(&self, spans: &Ranges) -> u64 {
        spans
            .0
            .iter()
            .map(|span| {
                let missing: u64 = self.gaps(span).iter().map(|gap| gap.end - gap.start).sum();
                span.end - span.start - missing
            })
            .sum()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Chunk {
    pub(crate) group: u32,
    pub(crate) range: Range<u64>,
}

/// The chunks still to fetch, ordered by priority group and then offset, and what is held.
#[derive(Debug, Default)]
pub(crate) struct Queue {
    pending: BTreeSet<(u32, u64, u64)>,
    claimed: Ranges,
    held: Ranges,
    counted: Ranges,
    pub(crate) error: Option<String>,
}

impl Queue {
    pub(crate) fn resume(held: Ranges) -> Self {
        Self {
            claimed: held.clone(),
            held,
            ..Self::default()
        }
    }

    pub(crate) fn held(&self) -> &Ranges {
        &self.held
    }

    /// Queues the parts of `spans` nobody holds or fetches yet. Counted spans are what the
    /// progress bar measures.
    pub(crate) fn enqueue(&mut self, group: u32, spans: &[Range<u64>], counted: bool) {
        for span in spans {
            if counted {
                self.counted.insert(span.clone());
            }
            for gap in self.claimed.gaps(span) {
                self.claimed.insert(gap.clone());
                let mut at = gap.start;
                while at < gap.end {
                    let end = (at + CHUNK).min(gap.end);
                    self.pending.insert((group, at, end));
                    at = end;
                }
            }
        }
    }

    pub(crate) fn next(&mut self) -> Option<Chunk> {
        self.pending.pop_first().map(|(group, start, end)| Chunk {
            group,
            range: start..end,
        })
    }

    pub(crate) fn wrote(&mut self, range: Range<u64>) {
        self.held.insert(range);
    }

    /// Puts back whatever part of `chunk` from `reached` on did not arrive, at its own priority.
    pub(crate) fn release(&mut self, chunk: &Chunk, reached: u64) {
        if reached < chunk.range.end {
            self.pending
                .insert((chunk.group, reached.max(chunk.range.start), chunk.range.end));
        }
    }

    pub(crate) fn forget(&mut self, spans: &[Range<u64>]) {
        for span in spans {
            self.held.remove(span);
            self.claimed.remove(span);
        }
    }

    pub(crate) fn holds(&self, spans: &[Range<u64>]) -> bool {
        self.held.covers(spans)
    }

    fn report(&self, progress: &Progress, phase: &str) {
        progress.set_total(self.counted.len());
        progress.set_done(self.held.covered(&self.counted));
        progress.set_status(self.error.as_deref().unwrap_or(phase));
    }
}

/// The bytes of one jar as they arrive, fetched by workers pulling chunks off the queue.
pub(crate) trait Source {
    fn with_queue<R>(&self, act: impl FnOnce(&mut Queue) -> R) -> R;

    /// Returns once the queue may have changed, or after a short while.
    async fn wait(&self);

    /// Bytes that are held.
    fn read(&self, span: Range<u64>) -> Result<Vec<u8>, String>;
}

pub(crate) async fn until_held(
    source: &impl Source,
    spans: &[Range<u64>],
    progress: &Progress,
    phase: &str,
) {
    while !source.with_queue(|queue| {
        queue.report(progress, phase);
        queue.holds(spans)
    }) {
        source.wait().await;
    }
}

fn unpack(
    source: &impl Source,
    directory: &Directory,
    spans: &[Range<u64>],
    pick: impl FnMut(&str) -> bool,
) -> Result<Files, String> {
    let chunks = spans
        .iter()
        .map(|span| Ok((span.start, source.read(span.clone())?)))
        .collect::<Result<Vec<_>, String>>()?;
    let views: Vec<(u64, &[u8])> = chunks
        .iter()
        .map(|(start, bytes)| (*start, bytes.as_slice()))
        .collect();
    directory.unpack(&views, pick)
}

/// Fetches the jar's `assets/` half in priority order: fonts, then the central directory,
/// then textures, then everything else. Font files go to `fonts` as soon as they are in.
pub(crate) async fn assets(
    source: &impl Source,
    release: &Release<'_>,
    progress: &Progress,
    mut pick: impl FnMut(&str) -> bool,
    mut fonts: impl FnMut(Files),
) -> Files {
    let tail = [release.jar.size - release.directory.size..release.jar.size];
    let hint = FontHint::for_release(release);
    let hinted = hint.as_ref().map(FontHint::spans).unwrap_or_default();
    source.with_queue(|queue| {
        queue.enqueue(HINTED_FONTS, &hinted, true);
        queue.enqueue(DIRECTORY, &tail, true);
    });

    let mut fonts_in = false;
    if let Some(hint) = &hint {
        until_held(source, &hinted, progress, "Downloading fonts").await;
        match unpack(source, &hint.directory(), &hinted, |_| true) {
            Ok(files) => {
                tracing::info!(files = files.len(), "fonts arrived where the hint put them");
                fonts(files);
                fonts_in = true;
            }
            Err(error) => {
                tracing::warn!(
                    "the font hint is wrong, waiting for the central directory: {error}"
                );
                source.with_queue(|queue| queue.forget(&hinted));
            }
        }
    }

    let directory = loop {
        until_held(source, &tail, progress, "Downloading the jar index").await;
        let parsed = source.read(tail[0].clone()).and_then(|bytes| {
            if verify(&release.directory, &bytes) {
                Directory::parse(&bytes, tail[0].start)
            } else {
                Err("the central directory fails its SHA-1".to_owned())
            }
        });
        match parsed {
            Ok(directory) => break directory,
            Err(error) => {
                tracing::warn!("{error}; fetching it again");
                source.with_queue(|queue| {
                    queue.forget(&tail);
                    queue.enqueue(DIRECTORY, &tail, true);
                });
            }
        }
    };

    if fonts_in
        && hint
            .as_ref()
            .is_some_and(|hint| !hint.agrees_with(&directory))
    {
        tracing::warn!(
            "the font hint disagrees with the central directory; dropping what it fetched"
        );
        source.with_queue(|queue| queue.forget(&hinted));
        fonts_in = false;
    }
    while !fonts_in {
        let definitions = directory.spans(is_font_definition);
        source.with_queue(|queue| queue.enqueue(FONTS, &definitions, true));
        until_held(source, &definitions, progress, "Downloading fonts").await;
        let mut files = match unpack(source, &directory, &definitions, is_font_definition) {
            Ok(files) => files,
            Err(error) => {
                tracing::warn!("{error}; fetching the font definitions again");
                source.with_queue(|queue| queue.forget(&definitions));
                continue;
            }
        };
        let names = font_textures(&files);
        let textures = directory.spans(|name| names.contains(name));
        source.with_queue(|queue| queue.enqueue(FONTS, &textures, true));
        until_held(source, &textures, progress, "Downloading fonts").await;
        match unpack(source, &directory, &textures, |name| names.contains(name)) {
            Ok(more) => {
                files.extend(more);
                tracing::info!(files = files.len(), "fonts arrived");
                fonts(files);
                fonts_in = true;
            }
            Err(error) => {
                tracing::warn!("{error}; fetching the font textures again");
                source.with_queue(|queue| queue.forget(&textures));
            }
        }
    }

    let textures = directory.spans(is_texture);
    let everything = directory.spans(|_| true);
    source.with_queue(|queue| {
        queue.enqueue(TEXTURES, &textures, true);
        queue.enqueue(ASSETS, &everything, true);
    });
    until_held(source, &textures, progress, "Downloading textures").await;
    tracing::info!("textures arrived");
    loop {
        until_held(source, &everything, progress, "Downloading models").await;
        match unpack(source, &directory, &everything, &mut pick) {
            Ok(files) => {
                tracing::info!(files = files.len(), "assets arrived");
                return files;
            }
            Err(error) => {
                tracing::warn!("{error}; fetching the assets again");
                source.with_queue(|queue| {
                    queue.forget(&everything);
                    queue.enqueue(ASSETS, &everything, true);
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_join_split_and_report_gaps() {
        let mut ranges = Ranges::default();
        ranges.insert(10..20);
        ranges.insert(30..40);
        ranges.insert(20..25);
        assert_eq!(ranges.0, [10..25, 30..40]);
        assert_eq!(ranges.gaps(&(0..50)), [0..10, 25..30, 40..50]);
        ranges.insert(24..31);
        assert_eq!(ranges.0, [10..40]);
        ranges.remove(&(15..18));
        assert_eq!(ranges.0, [10..15, 18..40]);
        assert!(ranges.covers(&[18..40, 11..12]));
        assert!(!ranges.covers(&[14..19]));
    }

    #[test]
    fn the_queue_hands_out_chunks_by_group_then_offset_and_takes_back_the_rest() {
        let mut queue = Queue::resume(Ranges(vec![100..200]));
        queue.enqueue(ASSETS, &[0..(3 << 20)], true);
        queue.enqueue(DIRECTORY, &[50..150, 5 << 20..(5 << 20) + 10], true);

        let first = queue.next().unwrap();
        assert_eq!(
            first,
            Chunk {
                group: DIRECTORY,
                range: 5 << 20..(5 << 20) + 10
            }
        );
        queue.release(&first, (5 << 20) + 4);
        assert_eq!(
            queue.next().unwrap().range,
            (5 << 20) + 4..(5 << 20) + 10,
            "a failed chunk comes back ahead of lower groups"
        );
        assert_eq!(queue.next().unwrap().range, 0..100);
        assert_eq!(queue.next().unwrap().range, 200..200 + (1 << 20));
    }
}
