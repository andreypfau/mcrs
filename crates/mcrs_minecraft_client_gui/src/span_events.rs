use std::collections::{HashMap, VecDeque};

use crate::font::{ClickEvent, Hover, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventIndex(u64);

#[derive(Debug, Default)]
pub struct SpanEvents {
    hovers: Table<Hover>,
    clicks: Table<ClickEvent>,
}

#[derive(Debug)]
struct Table<T> {
    first: u64,
    stored: VecDeque<T>,
}

impl<T> Default for Table<T> {
    fn default() -> Self {
        Self {
            first: 0,
            stored: VecDeque::new(),
        }
    }
}

impl<T> Table<T> {
    fn get(&self, index: EventIndex) -> Option<&T> {
        let offset = index.0.checked_sub(self.first)?;
        self.stored.get(usize::try_from(offset).ok()?)
    }

    fn end(&self) -> u64 {
        self.first
            .saturating_add(u64::try_from(self.stored.len()).unwrap_or(u64::MAX))
    }

    fn push(&mut self, value: T) -> Option<EventIndex> {
        let index = self.end();
        if index == u64::MAX {
            return None;
        }
        self.stored.push_back(value);
        Some(EventIndex(index))
    }

    fn drop_before(&mut self, keep: Option<EventIndex>) {
        let keep = keep.map_or(self.end(), |index| index.0);
        while self.first < keep && self.stored.pop_front().is_some() {
            self.first = self.first.saturating_add(1);
        }
    }

    fn clear(&mut self) {
        self.first = self.end();
        self.stored.clear();
    }
}

impl SpanEvents {
    pub fn hover(&self, index: EventIndex) -> Option<&Hover> {
        self.hovers.get(index)
    }

    pub fn click(&self, index: EventIndex) -> Option<&ClickEvent> {
        self.clicks.get(index)
    }

    pub fn len(&self) -> usize {
        self.hovers.stored.len() + self.clicks.stored.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&mut self) {
        self.hovers.clear();
        self.clicks.clear();
    }

    pub fn retain_referenced<'s>(&mut self, spans: impl IntoIterator<Item = &'s Span>) {
        let mut hover: Option<EventIndex> = None;
        let mut click: Option<EventIndex> = None;
        for span in spans {
            hover = oldest(hover, span.hover);
            click = oldest(click, span.click);
        }
        self.hovers.drop_before(hover);
        self.clicks.drop_before(click);
    }
}

pub(crate) struct Interner<'e> {
    events: &'e mut SpanEvents,
    hovers: HashMap<usize, Option<EventIndex>>,
    clicks: HashMap<usize, Option<EventIndex>>,
}

impl<'e> Interner<'e> {
    pub(crate) fn new(events: &'e mut SpanEvents) -> Self {
        Self {
            events,
            hovers: HashMap::new(),
            clicks: HashMap::new(),
        }
    }

    pub(crate) fn hover(&mut self, hover: &Hover) -> Option<EventIndex> {
        intern(&mut self.hovers, &mut self.events.hovers, hover)
    }

    pub(crate) fn click(&mut self, click: &ClickEvent) -> Option<EventIndex> {
        intern(&mut self.clicks, &mut self.events.clicks, click)
    }
}

fn intern<T: Clone>(
    seen: &mut HashMap<usize, Option<EventIndex>>,
    stored: &mut Table<T>,
    value: &T,
) -> Option<EventIndex> {
    *seen
        .entry(std::ptr::from_ref(value).addr())
        .or_insert_with(|| stored.push(value.clone()))
}

fn oldest(current: Option<EventIndex>, candidate: Option<EventIndex>) -> Option<EventIndex> {
    match (current, candidate) {
        (Some(current), Some(candidate)) => Some(EventIndex(current.0.min(candidate.0))),
        (current, candidate) => current.or(candidate),
    }
}
