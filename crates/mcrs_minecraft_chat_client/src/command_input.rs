use crate::abilities::ChatAbilities;
use crate::entries::{Entries, Tooltip, is_separator};
use crate::suggestion_menu::SuggestionMenu;
use mcrs_command::Span;
use mcrs_command::{
    ArgumentSet, Node, NodeId, NodeKind, ParseResult, ParseScratch, Presence, Suggestion,
    SuggestionBuilder, SuggestionList, SuggestionParent, SuggestionText, UsageDepth,
    starts_with_ignore_case,
};
use mcrs_minecraft_command::{Source, text_matches};
use mcrs_minecraft_command_argument_vanilla::{
    ArgumentError, ClientGraph, CommandSyntaxError, VanillaLookup,
};
use mcrs_minecraft_protocol::CommandSuggestion;
use mcrs_minecraft_protocol::text::Text;
use mcrs_minecraft_wire::utf16_byte_offset;
use std::mem;
use std::ops::Range;

/// What the client parse and the suggestions read: the client tree, the client source and the
/// options and abilities that change the screen.
pub struct Context<'a, L> {
    pub tree: &'a ClientGraph,
    pub source: &'a Source<'a, L>,
    pub abilities: ChatAbilities,
    /// The option "Command Suggestions".
    pub auto_suggest: bool,
    /// The custom completion set of the connection.
    pub completions: &'a [String],
}

/// A line above the input box that is not a suggestion.
#[derive(Clone, Debug, PartialEq)]
pub enum Hint {
    /// `command.context.parse_error` over the input.
    Error(CommandSyntaxError),
    /// A usage line, a range of [`CommandInput::usage_text`].
    Usage(Range<usize>),
    /// A translation key, shown in red.
    Restriction(&'static str),
}

/// How many colours the arguments of a command cycle through.
pub const ARGUMENT_COLOURS: u8 = 5;

/// The colour role of a range of a command input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Highlight {
    /// The argument with this index modulo [`ARGUMENT_COLOURS`] in the last context.
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
    list: Option<SuggestionMenu>,
    hints: Vec<Hint>,
    usage: String,
    pub auto: bool,
}

/// Whether the parse passes a message argument, which picks `chat_command_signed`.
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
enum TooltipRef<'a> {
    Key(&'static str),
    Text(&'a Text),
}

/// What a refresh does with the suggestion list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListOnRefresh {
    /// The list hides, and the suggestion state is computed again.
    Rebuild,
    /// The list stays, and no suggestion state is computed.
    Keep,
}

/// Whether a parse computes the suggestions of the caret.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Suggest {
    Skip,
    Compute,
}

impl CommandInput {
    /// The input of the last refresh.
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Whether the input starts with `/`.
    pub fn is_command(&self) -> bool {
        self.command
    }

    /// The colour roles of a command input, in input order.
    pub fn highlights(&self) -> &[(Span, Highlight)] {
        &self.highlights
    }

    /// The parse contains a value of the type `minecraft:message`.
    pub fn has_message(&self) -> bool {
        self.message
    }

    /// Whether the suggestions of the input are complete.
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// The suggestions of the input at the caret.
    pub fn suggestions(&self) -> &Entries {
        &self.entries
    }

    /// The id of the server request that waits for its reply.
    pub fn open_request(&self) -> Option<i32> {
        self.open
    }

    /// The suggestion list, while it shows.
    pub fn list(&self) -> Option<&SuggestionMenu> {
        self.list.as_ref()
    }

    /// The suggestion list, while it shows.
    pub fn list_mut(&mut self) -> Option<&mut SuggestionMenu> {
        self.list.as_mut()
    }

    /// Hides the suggestion list.
    pub fn hide_list(&mut self) {
        self.list = None;
    }

    /// The hint lines; they show only while the list is hidden.
    pub fn hints(&self) -> &[Hint] {
        if self.list.is_some() {
            &[]
        } else {
            &self.hints
        }
    }

    /// The text of a [`Hint::Usage`].
    pub fn usage_text(&self, range: Range<usize>) -> &str {
        self.usage.get(range).unwrap_or_default()
    }

    /// Shows the list when the state is ready and holds a suggestion.
    pub fn show_list(&mut self) -> bool {
        if !self.ready || self.entries.is_empty() {
            return false;
        }
        self.list = Some(SuggestionMenu::new(&self.entries, &self.input));
        true
    }

    /// The input changed. Returns the server request to send.
    pub fn refresh<L: VanillaLookup>(
        &mut self,
        input: &str,
        caret: usize,
        cx: &Context<'_, L>,
    ) -> Option<Request> {
        self.update(input, caret, cx, ListOnRefresh::Rebuild)
    }

    /// The input changed by an insert from the list: the list stays and no suggestion state is
    /// computed. Returns the server request to send.
    pub fn refresh_after_insert<L: VanillaLookup>(
        &mut self,
        input: &str,
        caret: usize,
        cx: &Context<'_, L>,
    ) -> Option<Request> {
        self.update(input, caret, cx, ListOnRefresh::Keep)
    }

    fn update<L: VanillaLookup>(
        &mut self,
        input: &str,
        caret: usize,
        cx: &Context<'_, L>,
        list: ListOnRefresh,
    ) -> Option<Request> {
        let keep_list = list == ListOnRefresh::Keep;
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
        let suggest = if !keep_list && self.caret >= 1 {
            Suggest::Compute
        } else {
            Suggest::Skip
        };
        self.parse_and_suggest(cx, suggest)
    }

    fn parse_and_suggest<L: VanillaLookup>(
        &mut self,
        cx: &Context<'_, L>,
        suggest: Suggest,
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
                self.highlights.push((
                    span,
                    Highlight::Argument((index % usize::from(ARGUMENT_COLOURS)) as u8),
                ));
            }
        }
        if self.cursor < input.len() {
            self.highlights
                .push((Span::new(self.cursor, input.len()), Highlight::Trailing));
        }
        if suggest == Suggest::Skip {
            return None;
        }
        let SuggestionParent {
            node: parent,
            start,
            context,
        } = parse.suggestion_parent_at(self.caret);
        self.parent = Some(parent);
        let truncated = input.get(..self.caret).unwrap_or_default();
        let children = cx
            .tree
            .node(parent)
            .map(|node| node.children())
            .unwrap_or_default();
        let mut local = mcrs_command::recycle(mem::take(&mut self.local));
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
                        argument.suggest(cx.source, &context, &mut builder);
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
            self.next_id = self.next_id.wrapping_add(1);
            self.open = Some(id);
            Some(Request {
                id,
                end: self.caret,
            })
        } else {
            self.open = None;
            let list = SuggestionList::build(truncated, &mut local);
            self.entries.fill(&list, |&key| Tooltip::Key(key));
            None
        };
        self.local = mcrs_command::recycle(local);
        if request.is_none() {
            self.finish(cx);
        }
        request
    }

    /// Applies a `command_suggestions` reply. `start` and `length` count UTF-16
    /// code units of the request text, as on the wire. Returns whether the reply was for the
    /// open request.
    pub fn reply<L: VanillaLookup>(
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
        let start = utf16_byte_offset(truncated, start);
        let end = start + utf16_byte_offset(truncated.get(start..).unwrap_or_default(), length);
        let tip =
            |suggestion: &CommandSuggestion<'_>| suggestion.tooltip.clone().map(Tooltip::Text);
        if self.single_child {
            self.entries.clear();
            self.entries.set_range(Span::new(start, end));
            for suggestion in suggestions {
                self.entries
                    .push(start, suggestion.text.chars(), tip(suggestion));
            }
        } else {
            mem::swap(&mut self.entries, &mut self.spare);
            let pending = &self.spare;
            let mut all: Vec<Suggestion<'_, TooltipRef<'_>>> = pending
                .items()
                .map(|(item_start, text, tooltip)| {
                    let tooltip = match tooltip {
                        Some(Tooltip::Key(key)) => Some(TooltipRef::Key(key)),
                        Some(Tooltip::Text(value)) => Some(TooltipRef::Text(value)),
                        None => None,
                    };
                    Suggestion::new(item_start, SuggestionText::new(text), tooltip)
                })
                .chain(suggestions.iter().map(|suggestion| {
                    Suggestion::new(
                        start,
                        SuggestionText::new(&suggestion.text),
                        suggestion.tooltip.as_ref().map(TooltipRef::Text),
                    )
                }))
                .collect();
            let list = SuggestionList::build(truncated, &mut all);
            self.entries.fill(&list, |tooltip| match tooltip {
                TooltipRef::Key(key) => Tooltip::Key(key),
                TooltipRef::Text(text) => Tooltip::Text((*text).clone()),
            });
        }
        self.finish(cx);
        true
    }

    /// The suggestion state is ready.
    fn finish<L: VanillaLookup>(&mut self, cx: &Context<'_, L>) {
        self.entries.order_by_last_word(&self.input, self.caret);
        self.ready = true;
        self.build_hints(cx);
        self.list = None;
        if self.auto && cx.auto_suggest {
            self.show_list();
        }
    }

    fn build_hints<L: VanillaLookup>(&mut self, cx: &Context<'_, L>) {
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
            let presence = if parent.command().is_some() {
                Presence::Optional
            } else {
                Presence::Required
            };
            for &child in parent.children() {
                if cx
                    .tree
                    .node(child)
                    .is_none_or(|child| matches!(child.kind(), NodeKind::Literal(_)))
                {
                    continue;
                }
                let start = self.usage.len();
                if cx.tree.short_usage(
                    child,
                    cx.source,
                    presence,
                    UsageDepth::Children,
                    &mut self.usage,
                ) {
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
    /// word.
    fn chat_suggestions<L: VanillaLookup>(&mut self, cx: &Context<'_, L>) {
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
        let mut local = mcrs_command::recycle(mem::take(&mut self.local));
        let mut builder = SuggestionBuilder::new(truncated, start, &mut local);
        let names = cx
            .source
            .lookup
            .player_names()
            .chain(cx.completions.iter().map(String::as_str));
        builder.suggest_matching(names, text_matches);
        let list = SuggestionList::build(truncated, &mut local);
        self.entries.fill(&list, |&key| Tooltip::Key(key));
        self.local = mcrs_command::recycle(local);
        self.ready = true;
    }

    /// Whether `line`, a command line without its `/`, has a message argument, which picks
    /// `chat_command_signed`.
    pub fn line_has_message<L: VanillaLookup>(&mut self, line: &str, cx: &Context<'_, L>) -> bool {
        let parse = cx.tree.parse(line, 0, cx.source, &mut self.scratch);
        has_message(cx.tree, &parse)
    }
}
