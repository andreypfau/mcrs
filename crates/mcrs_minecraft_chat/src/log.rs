use std::collections::VecDeque;

const MAX_MESSAGES: usize = 100;
const MAX_LINES: usize = 100;
const LIFETIME: f64 = 200.0;

/// A mark on a message (CHAT-LOG-005).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    SystemSinglePlayer,
    System,
    NotSecure,
    Modified,
    Error,
}

impl Tag {
    /// The RGB colour of the bar left of the line.
    pub fn indicator(self) -> u32 {
        match self {
            Tag::SystemSinglePlayer | Tag::System | Tag::NotSecure => 0xD0_D0D0,
            Tag::Modified => 0x60_6060,
            Tag::Error => 0xFF_5555,
        }
    }

    /// The translation key of the tooltip of the indicator.
    pub fn tooltip_key(self) -> &'static str {
        match self {
            Tag::SystemSinglePlayer => "chat.tag.system_single_player",
            Tag::System => "chat.tag.system",
            Tag::NotSecure => "chat.tag.not_secure",
            Tag::Modified => "chat.tag.modified",
            Tag::Error => "chat.tag.error",
        }
    }

    /// A tag with an icon makes the wrap width 15 GUI units smaller (CHAT-LOG-001).
    pub fn has_icon(self) -> bool {
        self == Tag::Modified
    }
}

/// A message of the log, with the GUI tick when it arrived.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Logged<M> {
    pub message: M,
    pub tick: u64,
    pub tag: Option<Tag>,
}

/// One row of a wrapped message.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Line<L> {
    pub content: L,
    pub tick: u64,
    pub tag: Option<Tag>,
    /// Whether this row is the bottom row of its message (CHAT-LOG-005).
    pub last: bool,
}

/// The kept messages, newest first (CHAT-LOG-001).
#[derive(Clone, Debug)]
pub struct ChatLog<M> {
    messages: VecDeque<Logged<M>>,
    /// The count of messages ever pushed, so a projection finds the ones it has not seen.
    pushed: u64,
    /// Counts the clears, so a projection drops its lines with the messages.
    cleared: u64,
}

impl<M> Default for ChatLog<M> {
    fn default() -> Self {
        Self {
            messages: VecDeque::new(),
            pushed: 0,
            cleared: 0,
        }
    }
}

impl<M> ChatLog<M> {
    /// Adds a message that passed the visibility filter (CHAT-LOG-001).
    pub fn push(&mut self, message: M, tick: u64, tag: Option<Tag>) {
        self.messages.push_front(Logged { message, tick, tag });
        self.messages.truncate(MAX_MESSAGES);
        self.pushed = self.pushed.saturating_add(1);
    }

    pub fn clear(&mut self) {
        self.messages.clear();
        self.cleared = self.cleared.wrapping_add(1);
    }
}

/// The lines of the kept messages, newest first, and the scroll of the open chat
/// (CHAT-LOG-001, CHAT-LOG-004): the projection of a [`ChatLog`] for one wrap. `wrap` writes
/// the rows of one message with its tag, top row first.
#[derive(Clone, Debug)]
pub struct ChatLines<L> {
    lines: VecDeque<Line<L>>,
    rows: Vec<L>,
    scroll: usize,
    new_since_scroll: bool,
    seen: u64,
    cleared: u64,
}

impl<L> Default for ChatLines<L> {
    fn default() -> Self {
        Self {
            lines: VecDeque::new(),
            rows: Vec::new(),
            scroll: 0,
            new_since_scroll: false,
            seen: 0,
            cleared: 0,
        }
    }
}

/// The alpha of a line of the closed chat `age` ticks after its message arrived
/// (CHAT-LOG-003).
#[must_use]
pub fn fade(age: u64) -> f32 {
    let t = (10.0 * (1.0 - age as f64 / LIFETIME)).clamp(0.0, 1.0);
    (t * t) as f32
}

impl<L> ChatLines<L> {
    /// The lines, newest first.
    pub fn lines(&self) -> impl ExactSizeIterator<Item = &Line<L>> {
        self.lines.iter()
    }

    pub fn scroll(&self) -> usize {
        self.scroll
    }

    pub fn new_since_scroll(&self) -> bool {
        self.new_since_scroll
    }

    fn add<M>(
        &mut self,
        logged: &Logged<M>,
        scrolls: bool,
        wrap: &mut impl FnMut(&M, Option<Tag>, &mut Vec<L>),
    ) {
        self.rows.clear();
        wrap(&logged.message, logged.tag, &mut self.rows);
        let rows = self.rows.len();
        for (row, content) in self.rows.drain(..).enumerate() {
            if scrolls && self.scroll > 0 {
                self.scroll = self.scroll.saturating_add(1);
                self.new_since_scroll = true;
            }
            self.lines.push_front(Line {
                content,
                tick: logged.tick,
                tag: logged.tag,
                last: row + 1 == rows,
            });
        }
        self.lines.truncate(MAX_LINES);
    }

    /// Adds the lines of the messages pushed since the last sync, and drops every line when the
    /// log was cleared. `open` is the `perPage` of the open chat, `None` when the chat is
    /// closed.
    pub fn sync<M>(
        &mut self,
        log: &ChatLog<M>,
        open: Option<usize>,
        mut wrap: impl FnMut(&M, Option<Tag>, &mut Vec<L>),
    ) {
        if self.cleared != log.cleared {
            self.cleared = log.cleared;
            self.lines.clear();
            self.reset_scroll();
        }
        let new = usize::try_from(log.pushed.saturating_sub(self.seen)).unwrap_or(usize::MAX);
        self.seen = log.pushed;
        for logged in log.messages.iter().take(new).rev() {
            self.add(logged, open.is_some(), &mut wrap);
        }
        if let Some(per_page) = open {
            self.scroll_by(0, per_page);
        }
    }

    /// Wraps every kept message again, after a change of the width, the scale or the spacing
    /// (CHAT-LOG-001).
    pub fn rebuild<M>(
        &mut self,
        log: &ChatLog<M>,
        mut wrap: impl FnMut(&M, Option<Tag>, &mut Vec<L>),
    ) {
        self.lines.clear();
        self.scroll = 0;
        self.seen = log.pushed;
        self.cleared = log.cleared;
        for logged in log.messages.iter().rev() {
            self.add(logged, false, &mut wrap);
        }
    }

    /// Scrolls the open chat by `lines`, upwards positive (CHAT-LOG-004).
    pub fn scroll_by(&mut self, lines: isize, per_page: usize) {
        let max = self.lines.len().saturating_sub(per_page);
        let scroll = self.scroll.saturating_add_signed(lines).min(max);
        self.scroll = scroll;
        if scroll == 0 {
            self.new_since_scroll = false;
        }
    }

    pub fn reset_scroll(&mut self) {
        self.scroll = 0;
        self.new_since_scroll = false;
    }

    /// The lines in the slots of the view, slot 0 at the bottom.
    pub fn visible(&self, per_page: usize) -> impl Iterator<Item = (usize, &Line<L>)> {
        self.lines
            .iter()
            .skip(self.scroll)
            .take(per_page)
            .enumerate()
    }
}
