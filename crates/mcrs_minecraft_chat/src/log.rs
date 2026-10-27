use std::collections::VecDeque;

const MAX_MESSAGES: usize = 100;
const MAX_LINES: usize = 100;
const LIFETIME: f64 = 200.0;
const OVERLAY_TICKS: u32 = 60;
const OVERLAY_FADE: f32 = 20.0;

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
pub struct Logged<M> {
    pub message: M,
    pub tick: u64,
    pub tag: Option<Tag>,
}

/// One row of a wrapped message.
#[derive(Clone, Debug, PartialEq)]
pub struct Line<L> {
    pub content: L,
    pub tick: u64,
    pub tag: Option<Tag>,
    /// Whether this row is the bottom row of its message (CHAT-LOG-005).
    pub last: bool,
}

/// The kept messages and their lines, newest first, and the scroll of the open chat
/// (CHAT-LOG-001, CHAT-LOG-004). `wrap` writes the rows of one message, top row first.
#[derive(Clone, Debug)]
pub struct ChatLog<M, L> {
    messages: VecDeque<Logged<M>>,
    lines: VecDeque<Line<L>>,
    rows: Vec<L>,
    scroll: usize,
    new_since_scroll: bool,
}

impl<M, L> Default for ChatLog<M, L> {
    fn default() -> Self {
        Self {
            messages: VecDeque::new(),
            lines: VecDeque::new(),
            rows: Vec::new(),
            scroll: 0,
            new_since_scroll: false,
        }
    }
}

/// The alpha of a line of the closed chat `age` ticks after its message arrived
/// (CHAT-LOG-003).
pub fn fade(age: u64) -> f32 {
    let t = (10.0 * (1.0 - age as f64 / LIFETIME)).clamp(0.0, 1.0);
    (t * t) as f32
}

impl<M, L> ChatLog<M, L> {
    pub fn messages(&self) -> impl DoubleEndedIterator<Item = &Logged<M>> {
        self.messages.iter()
    }

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

    fn add_lines(&mut self, index: usize, scrolls: bool, wrap: &mut impl FnMut(&M, &mut Vec<L>)) {
        let Some(logged) = self.messages.get(index) else {
            return;
        };
        self.rows.clear();
        wrap(&logged.message, &mut self.rows);
        let rows = self.rows.len();
        for (row, content) in self.rows.drain(..).enumerate() {
            if scrolls && self.scroll > 0 {
                self.scroll += 1;
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

    /// Adds a message that passed the visibility filter (CHAT-LOG-001). `open` is whether the
    /// chat screen is open.
    pub fn push(
        &mut self,
        message: M,
        tick: u64,
        tag: Option<Tag>,
        open: bool,
        mut wrap: impl FnMut(&M, &mut Vec<L>),
    ) {
        self.messages.push_front(Logged { message, tick, tag });
        self.messages.truncate(MAX_MESSAGES);
        self.add_lines(0, open, &mut wrap);
    }

    /// Wraps every kept message again, after a change of the width, the scale or the spacing.
    pub fn rewrap(&mut self, mut wrap: impl FnMut(&M, &mut Vec<L>)) {
        self.lines.clear();
        self.scroll = 0;
        for index in (0..self.messages.len()).rev() {
            self.add_lines(index, false, &mut wrap);
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

    pub fn clear(&mut self) {
        self.messages.clear();
        self.lines.clear();
        self.reset_scroll();
    }
}

/// The text above the hotbar (CHAT-RECV-005).
#[derive(Clone, Debug)]
pub struct Overlay<M> {
    message: Option<M>,
    time_left: u32,
}

impl<M> Default for Overlay<M> {
    fn default() -> Self {
        Self {
            message: None,
            time_left: 0,
        }
    }
}

impl<M> Overlay<M> {
    pub fn set(&mut self, message: M) {
        self.message = Some(message);
        self.time_left = OVERLAY_TICKS;
    }

    pub fn tick(&mut self) {
        self.time_left = self.time_left.saturating_sub(1);
    }

    /// The message and its alpha from 0 to 255, while it shows.
    pub fn shown(&self, partial_tick: f32) -> Option<(&M, u8)> {
        let t = self.time_left as f32 - partial_tick;
        let alpha = (t * 255.0 / OVERLAY_FADE).floor().min(255.0);
        if alpha <= 0.0 {
            return None;
        }
        Some((self.message.as_ref()?, alpha as u8))
    }
}
