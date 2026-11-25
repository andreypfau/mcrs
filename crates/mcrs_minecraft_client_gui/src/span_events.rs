use std::collections::HashMap;

use crate::font::{ClickEvent, Hover};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventIndex(u32);

#[derive(Debug, Default)]
pub struct SpanEvents {
    hovers: Vec<Hover>,
    clicks: Vec<ClickEvent>,
}

impl SpanEvents {
    pub fn hover(&self, index: EventIndex) -> Option<&Hover> {
        self.hovers.get(index.0 as usize)
    }

    pub fn click(&self, index: EventIndex) -> Option<&ClickEvent> {
        self.clicks.get(index.0 as usize)
    }

    pub fn len(&self) -> usize {
        self.hovers.len() + self.clicks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&mut self) {
        self.hovers.clear();
        self.clicks.clear();
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
    stored: &mut Vec<T>,
    value: &T,
) -> Option<EventIndex> {
    *seen
        .entry(std::ptr::from_ref(value).addr())
        .or_insert_with(|| {
            let index = u32::try_from(stored.len()).ok()?;
            stored.push(value.clone());
            Some(EventIndex(index))
        })
}
