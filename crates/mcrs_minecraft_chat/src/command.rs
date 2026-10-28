use crate::ability::ChatAbilities;
use crate::byte_offset;
use mcrs_command::suggest::{recycle, starts_with_ignore_case};
use mcrs_command::{
    ArgumentSet, Node, NodeId, NodeKind, ParseResult, ParseScratch, Span, Suggestion,
    SuggestionBuilder, SuggestionText, build_list,
};
use mcrs_minecraft_command::{
    ArgumentError, ClientGraph, CommandSyntaxError, Lookup, Source, text_matches,
};
use mcrs_minecraft_protocol::command::CommandSuggestion;
use mcrs_minecraft_protocol::text::Text;
use std::ops::Range;

/// The most rows the suggestion list shows (CCMD-LIST-002).
pub const LIST_ROWS: usize = 10;

/// What the client parse and the suggestions read: the client tree, the client source and the
/// options and abilities that change the screen.
pub struct Context<'a, L> {
    pub tree: &'a ClientGraph,
    pub source: &'a Source<'a, L>,
    pub abilities: ChatAbilities,
    /// The option "Command Suggestions" (CHAT-OPT-001).
    pub auto_suggest: bool,
    /// The custom completion set of the connection (CCMD-SUG-004).
    pub completions: &'a [String],
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tooltip {
    Key(&'static str),
    Text(Text),
}

#[derive(Clone, Debug, PartialEq)]
struct Item {
    start: usize,
    text: Range<usize>,
    tooltip: Option<Tooltip>,
}

/// Suggestions over one input, their texts in one buffer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entries {
    texts: String,
    items: Vec<Item>,
    range: (usize, usize),
}

impl Entries {
    fn clear(&mut self) {
        self.texts.clear();
        self.items.clear();
        self.range = (0, 0);
    }

    fn push(&mut self, start: usize, text: impl Iterator<Item = char>, tooltip: Option<Tooltip>) {
        let from = self.texts.len();
        self.texts.extend(text);
        self.items.push(Item {
            start,
            text: from..self.texts.len(),
            tooltip,
        });
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The range of the input that each suggestion replaces, as byte offsets.
    pub fn range(&self) -> (usize, usize) {
        self.range
    }

    pub fn text(&self, index: usize) -> &str {
        self.items
            .get(index)
            .and_then(|item| self.texts.get(item.text.clone()))
            .unwrap_or_default()
    }

    pub fn tooltip(&self, index: usize) -> Option<&Tooltip> {
        self.items.get(index)?.tooltip.as_ref()
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        (0..self.len()).map(|index| self.text(index))
    }

    fn fill<T>(
        &mut self,
        list: &mcrs_command::SuggestionList<'_, '_, T>,
        tooltip: impl Fn(&T) -> Tooltip,
    ) {
        self.clear();
        self.range = (list.span.start, list.span.end);
        for (text, tip) in list.iter() {
            self.push(list.span.start, text.chars(), tip.map(&tooltip));
        }
    }

    /// Puts each suggestion that starts with the last word first, keeping the order within
    /// each group (CCMD-SUG-003).
    fn order_by_last_word(&mut self, input: &str, caret: usize) {
        let before = input.get(..caret).unwrap_or_default();
        let word_start = before.rfind(is_separator).map_or(0, |index| index + 1);
        let word = before.get(word_start..).unwrap_or_default().to_lowercase();
        let texts = &self.texts;
        self.items.sort_by_key(|item| {
            let text = texts.get(item.text.clone()).unwrap_or_default();
            let first = text.starts_with(&word)
                || text
                    .strip_prefix("minecraft:")
                    .is_some_and(|rest| rest.starts_with(&word));
            !first
        });
    }
}

fn is_separator(c: char) -> bool {
    matches!(c, '\u{9}'..='\u{d}' | ' ')
}

/// A line above the input box that is not a suggestion (CCMD-HINT-001).
#[derive(Clone, Debug, PartialEq)]
pub enum Hint {
    /// `command.context.parse_error` over the input (CCMD-HINT-002).
    Error(CommandSyntaxError),
    /// A usage line, a range of [`CommandInput::usage_text`].
    Usage(Range<usize>),
    /// A translation key, shown in red.
    Restriction(&'static str),
}

/// The colour role of a range of a command input (CCMD-HL-001).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Highlight {
    /// The argument with this index modulo 5 in the last context.
    Argument(u8),
    /// The text after the final cursor of the parse.
    Trailing,
}

/// A `command_suggestion` request: its id, and the end of the input it sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub id: i32,
    pub end: usize,
}

/// The suggestion list while it shows (CCMD-LIST-001 to CCMD-LIST-006).
#[derive(Clone, Debug, PartialEq)]
pub struct SuggestionList {
    entries: Entries,
    origin: String,
    result: String,
    selected: usize,
    offset: usize,
    /// The flag "cycle" of CCMD-LIST-004.
    pub(crate) cycle: bool,
}

impl SuggestionList {
    fn new(entries: &Entries, origin: &str) -> Self {
        let mut list = Self {
            entries: entries.clone(),
            origin: origin.to_owned(),
            result: String::new(),
            selected: 0,
            offset: 0,
            cycle: false,
        };
        list.select(0);
        list
    }

    pub fn entries(&self) -> &Entries {
        &self.entries
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    /// The input with the range replaced by the selected suggestion (CCMD-LIST-003, step 3).
    pub(crate) fn result(&self) -> &str {
        &self.result
    }

    /// The caret after an insert of the selected suggestion (CCMD-LIST-005, step 2).
    pub(crate) fn result_caret(&self) -> usize {
        self.entries.range.0 + self.entries.text(self.selected).len()
    }

    /// The rest of the result after the current input, when the result starts with it.
    pub fn ghost<'s>(&'s self, input: &str) -> Option<&'s str> {
        self.result
            .strip_prefix(input)
            .filter(|rest| !rest.is_empty())
    }

    /// Selects entry `index` modulo the count, and scrolls it into the rows (CCMD-LIST-003,
    /// CCMD-LIST-006).
    pub fn select(&mut self, index: isize) {
        let count = self.entries.len();
        let Some(selected) = isize::try_from(count)
            .ok()
            .filter(|&count| count > 0)
            .map(|count| index.rem_euclid(count).unsigned_abs())
        else {
            return;
        };
        self.selected = selected;
        if selected < self.offset {
            self.offset = selected;
        } else if selected > self.offset + LIST_ROWS - 1 {
            self.offset = selected + 1 - LIST_ROWS;
        }
        self.offset = self.offset.min(count.saturating_sub(LIST_ROWS));
        let (start, end) = self.entries.range;
        self.result.clear();
        self.result
            .push_str(self.origin.get(..start).unwrap_or_default());
        self.result.push_str(self.entries.text(selected));
        self.result
            .push_str(self.origin.get(end..).unwrap_or_default());
    }

    pub fn step(&mut self, delta: isize) {
        self.select(self.selected as isize + delta);
    }

    /// A wheel step over the list, upwards positive; the selection stays.
    pub fn scroll(&mut self, step: isize) {
        let max = self.entries.len().saturating_sub(LIST_ROWS);
        self.offset = self.offset.saturating_add_signed(-step).min(max);
    }

    /// The entries in the rows, from the top row down.
    pub fn rows(&self) -> impl Iterator<Item = (usize, &str)> {
        (self.offset..self.entries.len().min(self.offset + LIST_ROWS))
            .map(|index| (index, self.entries.text(index)))
    }
}

/// The client parse, the suggestion state, the server requests, the suggestion list and the
/// hint lines of one chat screen (`client-commands.md`). It owns its buffers, so a parse after
/// each key reuses them.
#[derive(Debug, Default)]
pub struct CommandInput {
    scratch: ParseScratch<ArgumentError>,
    local: Vec<Suggestion<'static, &'static str>>,
    input: String,
    caret: usize,
    command: bool,
    cursor: usize,
    errors: Vec<CommandSyntaxError>,
    reported: Option<CommandSyntaxError>,
    highlights: Vec<(Span, Highlight)>,
    message: bool,
    parent: Option<NodeId>,
    entries: Entries,
    spare: Entries,
    ready: bool,
    single_child: bool,
    next_id: i32,
    open: Option<i32>,
    list: Option<SuggestionList>,
    hints: Vec<Hint>,
    usage: String,
    /// The flag "auto" of CCMD-LIST-001.
    pub auto: bool,
}

/// Whether the parse passes a message argument, which picks `chat_command_signed`
/// (CHAT-SEND-004).
fn has_message<E: Copy>(tree: &ClientGraph, parse: &ParseResult<'_, '_, E>) -> bool {
    parse
        .contexts()
        .flat_map(|context| context.nodes())
        .any(|(node, _)| {
            tree.node(node)
                .and_then(Node::argument)
                .is_some_and(|argument| argument.message)
        })
}

#[derive(PartialEq)]
enum Tip<'a> {
    Key(&'static str),
    Text(&'a Text),
}

impl CommandInput {
    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn is_command(&self) -> bool {
        self.command
    }

    /// The colour roles of a command input, in input order.
    pub fn highlights(&self) -> &[(Span, Highlight)] {
        &self.highlights
    }

    /// The parse contains a value of the type `minecraft:message` (CCMD-PARSE-002).
    pub fn has_message(&self) -> bool {
        self.message
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }

    pub fn suggestions(&self) -> &Entries {
        &self.entries
    }

    pub fn open_request(&self) -> Option<i32> {
        self.open
    }

    pub fn list(&self) -> Option<&SuggestionList> {
        self.list.as_ref()
    }

    pub fn list_mut(&mut self) -> Option<&mut SuggestionList> {
        self.list.as_mut()
    }

    pub fn hide_list(&mut self) {
        self.list = None;
    }

    /// The hint lines; they show only while the list is hidden (CCMD-HINT-001).
    pub fn hints(&self) -> &[Hint] {
        if self.list.is_some() {
            &[]
        } else {
            &self.hints
        }
    }

    pub fn usage_text(&self, range: Range<usize>) -> &str {
        self.usage.get(range).unwrap_or_default()
    }

    /// Shows the list when the state is ready and holds a suggestion (CCMD-LIST-001).
    pub fn show_list(&mut self) -> bool {
        if !self.ready || self.entries.is_empty() {
            return false;
        }
        self.list = Some(SuggestionList::new(&self.entries, &self.input));
        true
    }

    /// The input changed (CCMD-PARSE-001). With `keep_list`, an insert from the list, the
    /// list stays and no suggestion state is computed (CCMD-LIST-005). Returns the server
    /// request to send.
    pub fn refresh<L: Lookup>(
        &mut self,
        input: &str,
        caret: usize,
        cx: &Context<'_, L>,
        keep_list: bool,
    ) -> Option<Request> {
        self.input.clear();
        self.input.push_str(input);
        self.caret = caret.min(input.len());
        self.command = input.starts_with('/');
        self.cursor = input.len();
        self.errors.clear();
        self.reported = None;
        self.highlights.clear();
        self.message = false;
        self.parent = None;
        self.entries.clear();
        self.ready = false;
        self.hints.clear();
        self.usage.clear();
        if !keep_list {
            self.list = None;
        }
        if !self.command {
            self.open = None;
            if !keep_list {
                self.chat_suggestions(cx);
            }
            return None;
        }
        self.parse_and_suggest(cx, !keep_list && self.caret >= 1)
    }

    fn parse_and_suggest<L: Lookup>(
        &mut self,
        cx: &Context<'_, L>,
        suggest: bool,
    ) -> Option<Request> {
        let input = self.input.as_str();
        let parse = cx.tree.parse(input, 1, cx.source, &mut self.scratch);
        self.cursor = parse.cursor();
        self.errors.extend(parse.errors());
        self.reported = parse.check(cx.tree).err();
        self.message = has_message(cx.tree, &parse);
        if let Some(last) = parse.contexts().last() {
            let arguments = last
                .nodes()
                .filter(|&(node, _)| cx.tree.node(node).and_then(Node::argument).is_some());
            for (index, (_, span)) in arguments.enumerate() {
                self.highlights
                    .push((span, Highlight::Argument((index % 5) as u8)));
            }
        }
        if self.cursor < input.len() {
            self.highlights
                .push((Span::new(self.cursor, input.len()), Highlight::Trailing));
        }
        if !suggest {
            return None;
        }
        let (parent, start) = parse.suggestion_parent_at(self.caret);
        self.parent = Some(parent);
        let truncated = input.get(..self.caret).unwrap_or_default();
        let children = cx
            .tree
            .node(parent)
            .map(|node| node.children())
            .unwrap_or_default();
        let mut local = recycle(std::mem::take(&mut self.local));
        let mut needs_server = false;
        let mut builder = SuggestionBuilder::new(truncated, start, &mut local);
        for child in children.iter().filter_map(|&child| cx.tree.node(child)) {
            match child.kind() {
                NodeKind::Root => {}
                NodeKind::Literal(name) => {
                    builder.suggest_matching([&**name], starts_with_ignore_case)
                }
                NodeKind::Argument { argument, .. } => {
                    if argument.needs_server(cx.source.lookup) {
                        needs_server = true;
                    } else {
                        argument.suggest(cx.source, &mut builder);
                    }
                }
            }
        }
        let request = if needs_server {
            for suggestion in &local {
                self.entries.push(
                    suggestion.start,
                    suggestion.text.chars(),
                    suggestion.tooltip.map(Tooltip::Key),
                );
            }
            self.single_child = children.len() == 1;
            let id = self.next_id;
            self.next_id += 1;
            self.open = Some(id);
            Some(Request {
                id,
                end: self.caret,
            })
        } else {
            self.open = None;
            let list = build_list(truncated, &mut local);
            self.entries.fill(&list, |&key| Tooltip::Key(key));
            None
        };
        self.local = recycle(local);
        if request.is_none() {
            self.finish(cx);
        }
        request
    }

    /// Applies a `command_suggestions` reply (CCMD-REQ-003). `start` and `length` count UTF-16
    /// code units of the request text. Returns whether the reply was for the open request.
    pub fn reply<L: Lookup>(
        &mut self,
        id: i32,
        start: usize,
        length: usize,
        suggestions: &[CommandSuggestion<'_>],
        cx: &Context<'_, L>,
    ) -> bool {
        if self.open != Some(id) {
            return false;
        }
        self.next_id = 0;
        self.open = None;
        let truncated = self.input.get(..self.caret).unwrap_or_default();
        let start = byte_offset(truncated, start);
        let end = start + byte_offset(truncated.get(start..).unwrap_or_default(), length);
        let tip =
            |suggestion: &CommandSuggestion<'_>| suggestion.tooltip.clone().map(Tooltip::Text);
        if self.single_child {
            self.entries.clear();
            self.entries.range = (start, end);
            for suggestion in suggestions {
                self.entries
                    .push(start, suggestion.text.chars(), tip(suggestion));
            }
        } else {
            std::mem::swap(&mut self.entries, &mut self.spare);
            let pending = &self.spare;
            let mut all: Vec<Suggestion<'_, Tip<'_>>> = (0..pending.len())
                .filter_map(|index| {
                    let item = pending.items.get(index)?;
                    Some(Suggestion {
                        start: item.start,
                        text: SuggestionText::new(pending.text(index)),
                        tooltip: match &item.tooltip {
                            Some(Tooltip::Key(key)) => Some(Tip::Key(key)),
                            Some(Tooltip::Text(text)) => Some(Tip::Text(text)),
                            None => None,
                        },
                    })
                })
                .chain(suggestions.iter().map(|suggestion| Suggestion {
                    start,
                    text: SuggestionText::new(&suggestion.text),
                    tooltip: suggestion.tooltip.as_ref().map(Tip::Text),
                }))
                .collect();
            let list = build_list(truncated, &mut all);
            self.entries.fill(&list, |tip| match tip {
                Tip::Key(key) => Tooltip::Key(key),
                Tip::Text(text) => Tooltip::Text((*text).clone()),
            });
        }
        self.finish(cx);
        true
    }

    /// The suggestion state is ready (CCMD-SUG-001, step 5).
    fn finish<L: Lookup>(&mut self, cx: &Context<'_, L>) {
        self.entries.order_by_last_word(&self.input, self.caret);
        self.ready = true;
        self.build_hints(cx);
        self.list = None;
        if self.auto && cx.auto_suggest {
            self.show_list();
        }
    }

    fn build_hints<L: Lookup>(&mut self, cx: &Context<'_, L>) {
        self.hints.clear();
        self.usage.clear();
        let len = self.input.len();
        let mut trailing = false;
        if self.caret == len {
            if self.entries.is_empty() && !self.errors.is_empty() {
                self.hints
                    .extend(self.errors.iter().copied().map(Hint::Error));
            } else if self.cursor < len {
                trailing = true;
            }
        }
        if self.hints.is_empty()
            && let Some(parent) = self.parent.and_then(|parent| cx.tree.node(parent))
        {
            let optional = parent.command().is_some();
            for &child in parent.children() {
                if cx
                    .tree
                    .node(child)
                    .is_none_or(|child| matches!(child.kind(), NodeKind::Literal(_)))
                {
                    continue;
                }
                let start = self.usage.len();
                if cx
                    .tree
                    .short_usage(child, cx.source, optional, false, &mut self.usage)
                {
                    self.hints.push(Hint::Usage(start..self.usage.len()));
                }
            }
            if self.hints.is_empty()
                && trailing
                && let Some(reported) = self.reported
            {
                self.hints.push(Hint::Error(reported));
            }
        }
        if !cx.abilities.send_commands {
            self.hints
                .push(Hint::Restriction("chat_screen.commands_not_allowed"));
        }
        if self.message && !cx.abilities.send_messages {
            self.hints
                .push(Hint::Restriction("chat_screen.messages_not_allowed"));
        }
    }

    /// The suggestions of chat text: player names and custom completions that match the last
    /// word (CCMD-SUG-004).
    fn chat_suggestions<L: Lookup>(&mut self, cx: &Context<'_, L>) {
        let input = self.input.as_str();
        if input.trim_matches(is_separator).is_empty() {
            return;
        }
        if !cx.abilities.send_messages {
            self.hints
                .push(Hint::Restriction("chat_screen.messages_not_allowed"));
        }
        let truncated = input.get(..self.caret).unwrap_or_default();
        let start = truncated.rfind(is_separator).map_or(0, |index| index + 1);
        let mut local = recycle(std::mem::take(&mut self.local));
        let mut builder = SuggestionBuilder::new(truncated, start, &mut local);
        let names = cx
            .source
            .lookup
            .player_names()
            .chain(cx.completions.iter().map(String::as_str));
        builder.suggest_matching(names, text_matches);
        let list = build_list(truncated, &mut local);
        self.entries.fill(&list, |&key| Tooltip::Key(key));
        self.local = recycle(local);
        self.ready = true;
    }

    /// Whether `line`, a command line without its `/`, has a message argument, which picks
    /// `chat_command_signed` (CHAT-SEND-004).
    pub fn line_has_message<L: Lookup>(&mut self, line: &str, cx: &Context<'_, L>) -> bool {
        let parse = cx.tree.parse(line, 0, cx.source, &mut self.scratch);
        has_message(cx.tree, &parse)
    }
}
