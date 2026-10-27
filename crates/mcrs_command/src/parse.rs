use crate::argument::{Argument, ArgumentSet};
use crate::argument_handle::ArgumentHandle;
use crate::contains::Contains;
use crate::error::{LibraryError, SyntaxError};
use crate::graph::Graph;
use crate::node_id::NodeId;
use crate::node_kind::NodeKind;
use crate::reader::{Reader, SEPARATOR};
use crate::requirement::Requirement;
use crate::span::Span;
use crate::suggest::{Suggestion, SuggestionBuilder, starts_with_ignore_case};
use alloc::vec::Vec;

/// Each redirect and each node below it nests one level of the parse.
// chisle: a deeper chain parses as if the node had no children; 512 levels is far past a
// 32500 code unit command block, and an explicit stack in place of recursion lifts it.
const MAX_DEPTH: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Entry {
    /// Starts a context: the top one, or the child context after a redirect.
    Context {
        root: NodeId,
        start: usize,
    },
    Node {
        node: NodeId,
        span: Span,
    },
}

/// The stacks of one parse, owned by the caller and reused between parses.
#[derive(Debug)]
pub struct ParseScratch<E> {
    entries: Vec<Entry>,
    errors: Vec<SyntaxError<E>>,
}

impl<E> Default for ParseScratch<E> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            errors: Vec::new(),
        }
    }
}

impl<E> ParseScratch<E> {
    /// Keeps the entries from `entry` and the errors from `error` in place of those from
    /// `entry_base` and `error_base`.
    fn keep(&mut self, entry_base: usize, entry: usize, error_base: usize, error: usize) {
        move_down(&mut self.entries, entry_base, entry);
        move_down(&mut self.errors, error_base, error);
    }
}

fn move_down<T>(items: &mut Vec<T>, base: usize, from: usize) {
    if from > base && from <= items.len() {
        items.drain(base..from);
    }
}

/// One context of a parse: its root, its start and the nodes it parsed.
#[derive(Clone, Copy, Debug)]
pub struct ContextView<'s> {
    root: NodeId,
    start: usize,
    nodes: &'s [Entry],
}

/// The top context, then the child context after each redirect.
pub(crate) fn contexts(entries: &[Entry]) -> impl Iterator<Item = ContextView<'_>> + '_ {
    entries
        .iter()
        .enumerate()
        .filter_map(move |(index, entry)| match *entry {
            Entry::Context { root, start } => {
                let rest = entries.get(index + 1..).unwrap_or_default();
                let len = rest
                    .iter()
                    .position(|entry| matches!(entry, Entry::Context { .. }))
                    .unwrap_or(rest.len());
                Some(ContextView {
                    root,
                    start,
                    nodes: rest.get(..len).unwrap_or_default(),
                })
            }
            Entry::Node { .. } => None,
        })
}

impl<'s> ContextView<'s> {
    /// The parsed nodes of the context with their spans, in input order.
    pub fn nodes(&self) -> impl Iterator<Item = (NodeId, Span)> + use<'s> {
        self.nodes.iter().filter_map(|entry| match *entry {
            Entry::Node { node, span } => Some((node, span)),
            Entry::Context { .. } => None,
        })
    }

    /// The last parsed node of the context.
    pub fn last(&self) -> Option<(NodeId, Span)> {
        self.nodes().last()
    }

    pub(crate) fn span(&self) -> Span {
        Span::new(
            self.start,
            self.last()
                .map_or(self.start, |(_, span)| span.end.max(self.start)),
        )
    }
}

/// Where the suggestions at a cursor come from.
#[derive(Clone, Copy, Debug)]
pub struct SuggestionParent<'s> {
    /// The node whose children suggest.
    pub node: NodeId,
    /// The start of the suggestions.
    pub start: usize,
    /// The nodes of the context of `node` before `start`.
    pub context: ContextView<'s>,
}

/// How well a child parsed: the whole input, then no error below it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Rank {
    complete: bool,
    clean: bool,
}

impl Rank {
    const BEST: Rank = Rank {
        complete: true,
        clean: true,
    };
}

#[derive(Clone, Copy, Debug)]
struct Best {
    rank: Rank,
    end: usize,
}

/// The nodes and the errors of one parse, borrowed from its [`ParseScratch`].
#[derive(Debug)]
pub struct ParseResult<'a, 's, E> {
    input: &'a str,
    cursor: usize,
    entries: &'s [Entry],
    errors: &'s [SyntaxError<E>],
}

impl<'a, 's, E: Copy> ParseResult<'a, 's, E> {
    pub fn input(&self) -> &'a str {
        self.input
    }

    /// Where the parse stopped; the parse read the whole input when it equals its length.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The top context, then the child context after each redirect.
    pub fn contexts(&self) -> impl Iterator<Item = ContextView<'s>> + 's {
        contexts(self.entries)
    }

    /// The errors of the level where the parse stopped.
    pub fn errors(&self) -> impl Iterator<Item = SyntaxError<E>> + 's {
        self.errors.iter().copied()
    }

    pub(crate) fn entries(&self) -> &'s [Entry] {
        self.entries
    }

    /// The context that runs and the handler of its last node, or the error to report.
    pub fn check<'g, A, R, H>(
        &self,
        graph: &'g Graph<A, R, H>,
    ) -> Result<(ContextView<'s>, &'g H), SyntaxError<E>> {
        if self.cursor < self.input.len() {
            if let [only] = self.errors {
                return Err(*only);
            }
            let top_is_empty = self
                .contexts()
                .next()
                .is_none_or(|top| top.span().is_empty());
            let kind = if top_is_empty {
                LibraryError::UnknownCommand
            } else {
                LibraryError::UnknownArgument
            };
            return Err(SyntaxError::library(kind, self.cursor));
        }
        self.contexts()
            .last()
            .and_then(|context| {
                let (node, _) = context.last()?;
                Some((context, graph.node(node)?.command()?))
            })
            .ok_or(SyntaxError::library(
                LibraryError::UnknownCommand,
                self.cursor,
            ))
    }

    /// Where the suggestions at `cursor` come from.
    pub fn suggestion_parent_at(&self, cursor: usize) -> SuggestionParent<'s> {
        let cursor = cursor.min(self.input.len());
        let mut parent = (NodeId::ROOT, cursor);
        let mut parsed = ContextView {
            root: NodeId::ROOT,
            start: 0,
            nodes: &[],
        };
        let mut contexts = self.contexts().peekable();
        while let Some(context) = contexts.next() {
            parsed = context;
            let span = context.span();
            if span.end < cursor {
                if contexts.peek().is_some() {
                    continue;
                }
                parent = match context.last() {
                    Some((node, span)) => (node, span.end + 1),
                    None => (context.root, span.start),
                };
            } else {
                let mut previous = context.root;
                parent = (
                    context.last().map_or(context.root, |(node, _)| node),
                    span.start,
                );
                for (node, node_span) in context.nodes() {
                    if node_span.contains(cursor) {
                        parent = (previous, node_span.start);
                        break;
                    }
                    previous = node;
                }
            }
            break;
        }
        let start = parent.1.min(cursor);
        let before = parsed
            .nodes
            .iter()
            .position(|entry| matches!(*entry, Entry::Node { span, .. } if span.end >= start))
            .unwrap_or(parsed.nodes.len());
        parsed.nodes = parsed.nodes.get(..before).unwrap_or_default();
        SuggestionParent {
            node: parent.0,
            start,
            context: parsed,
        }
    }
}

impl<A, R, H> Graph<A, R, H> {
    /// Parses `input` from `start` against the tree, as the source sees it.
    pub fn parse<'a, 's, S>(
        &self,
        input: &'a str,
        start: usize,
        source: &S,
        scratch: &'s mut ParseScratch<A::Error>,
    ) -> ParseResult<'a, 's, A::Error>
    where
        A: ArgumentSet<S>,
        R: Requirement<S>,
    {
        scratch.entries.clear();
        scratch.errors.clear();
        let start = start.min(input.len());
        scratch.entries.push(Entry::Context {
            root: NodeId::ROOT,
            start,
        });
        let cursor = self.level(NodeId::ROOT, input, start, source, scratch, 0);
        ParseResult {
            input,
            cursor,
            entries: &scratch.entries,
            errors: &scratch.errors,
        }
    }

    fn level<S>(
        &self,
        node: NodeId,
        input: &str,
        cursor: usize,
        source: &S,
        scratch: &mut ParseScratch<A::Error>,
        depth: usize,
    ) -> usize
    where
        A: ArgumentSet<S>,
        R: Requirement<S>,
    {
        let Some(parent) = self.node(node).filter(|_| depth < MAX_DEPTH) else {
            return cursor;
        };
        let entry_base = scratch.entries.len();
        let error_base = scratch.errors.len();
        let rest = input.get(cursor..).unwrap_or_default();
        let token = rest.split(SEPARATOR).next().unwrap_or_default();
        let literal = parent.children().iter().copied().find(|&child| {
            self.node(child).is_some_and(|child| {
                matches!(child.kind(), NodeKind::Literal(name) if **name == *token)
                    && child.requirement().test(source)
            })
        });
        let only_literal = literal.as_slice();
        let candidates = if literal.is_some() {
            only_literal
        } else {
            parent.children()
        };

        let mut best: Option<Best> = None;
        for &child in candidates {
            let Some(child_node) = self.node(child) else {
                continue;
            };
            if literal.is_none()
                && (child_node.is_literal() || !child_node.requirement().test(source))
            {
                continue;
            }
            let entry_start = scratch.entries.len();
            let error_start = scratch.errors.len();
            let mut reader = Reader::at(input, cursor);
            let parsed = match child_node.kind() {
                NodeKind::Root => continue,
                NodeKind::Literal(name) => {
                    reader.set_cursor(cursor + name.len());
                    Ok(())
                }
                NodeKind::Argument { argument, .. } => argument.check(&mut reader, source),
            }
            .and_then(|()| {
                if reader.at_end() || reader.peek_is(SEPARATOR) {
                    Ok(())
                } else {
                    Err(SyntaxError::library(
                        LibraryError::ExpectedSeparator,
                        reader.cursor(),
                    ))
                }
            });
            if let Err(error) = parsed {
                if best.is_none() {
                    scratch.errors.push(error);
                }
                continue;
            }

            let end = reader.cursor();
            scratch.entries.push(Entry::Node {
                node: child,
                span: Span::new(cursor, end),
            });
            let remaining = input.len() - end;
            if let Some(target) = self
                .followed_redirect(child, source)
                .filter(|_| remaining >= 1)
            {
                scratch.entries.push(Entry::Context {
                    root: target,
                    start: end + 1,
                });
                let nested = self.level(target, input, end + 1, source, scratch, depth + 1);
                scratch.keep(entry_base, entry_start, error_base, error_start);
                return nested;
            }
            let nested = if remaining >= 2 {
                self.level(child, input, end + 1, source, scratch, depth + 1)
            } else {
                end
            };
            let rank = Rank {
                complete: nested >= input.len(),
                clean: scratch.errors.len() == error_start,
            };
            match best {
                Some(best) if rank <= best.rank => {
                    scratch.entries.truncate(entry_start);
                    scratch.errors.truncate(error_start);
                }
                _ => {
                    scratch.keep(entry_base, entry_start, error_base, error_start);
                    best = Some(Best { rank, end: nested });
                }
            }
            if rank == Rank::BEST {
                break;
            }
        }
        best.map_or(cursor, |best| best.end)
    }

    /// Suggests what can replace the input before its end, in no order.
    pub fn suggest<'t, S, E: Copy>(
        &'t self,
        parse: &ParseResult<'t, '_, E>,
        source: &'t S,
        out: &mut Vec<Suggestion<'t, A::Tooltip>>,
    ) where
        A: ArgumentSet<S>,
        R: Requirement<S>,
    {
        let SuggestionParent {
            node,
            start,
            context,
        } = parse.suggestion_parent_at(parse.input().len());
        let Some(parent) = self.node(node) else {
            return;
        };
        let mut builder = SuggestionBuilder::new(parse.input(), start, out);
        for &child in parent.children() {
            let Some(child) = self
                .node(child)
                .filter(|child| child.requirement().test(source))
            else {
                continue;
            };
            match child.kind() {
                NodeKind::Root => {}
                NodeKind::Literal(name) => {
                    builder.suggest_matching([&**name], starts_with_ignore_case)
                }
                NodeKind::Argument { argument, .. } => {
                    argument.suggest(source, &context, &mut builder)
                }
            }
        }
    }

    /// Reads the value of an argument of a parsed context again. The parse already
    /// validated the span, so this read fails only when the source changed in between.
    pub fn value<'a, S, T>(
        &self,
        input: &'a str,
        span: Span,
        arg: ArgumentHandle<T>,
        source: &S,
    ) -> Option<Result<T::Value<'a>, SyntaxError<T::Error>>>
    where
        A: Contains<T>,
        T: Argument<S>,
    {
        let argument = self.node(arg.node())?.argument()?.get()?;
        let mut reader = Reader::at(input, span.start);
        Some(argument.parse(&mut reader, source))
    }
}
