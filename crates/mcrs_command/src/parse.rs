use crate::argument::{Argument, ArgumentSet};
use crate::error::{LibraryError, Span, SyntaxError};
use crate::reader::{Reader, SEPARATOR};
use crate::suggest::{Suggestion, SuggestionBuilder, starts_with_ignore_case};
use crate::tree::{Arg, Contains, Graph, NodeId, NodeKind, Requirement};
use alloc::vec::Vec;

/// Each redirect and each node below it nests one level of the parse.
// chisle: a deeper chain parses as if the node had no children; 512 levels is far past a
// 32500 code unit command block, and an explicit stack in place of recursion lifts it.
const MAX_DEPTH: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Entry {
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

impl<'s> ContextView<'s> {
    pub fn nodes(&self) -> impl Iterator<Item = (NodeId, Span)> + 's {
        self.nodes.iter().filter_map(|entry| match *entry {
            Entry::Node { node, span } => Some((node, span)),
            Entry::Context { .. } => None,
        })
    }

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

#[derive(Debug)]
pub struct ParseResult<'a, 's, E> {
    input: &'a str,
    cursor: usize,
    entries: &'s [Entry],
    errors: &'s [SyntaxError<E>],
}

impl<'a, 's, E: Copy> ParseResult<'a, 's, E> {
    pub(crate) fn input(&self) -> &'a str {
        self.input
    }

    /// The top context, then the child context after each redirect.
    pub(crate) fn contexts(&self) -> impl Iterator<Item = ContextView<'s>> + 's {
        let entries = self.entries;
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

    /// The context that runs, or the error to report.
    pub fn check<A, R, H>(
        &self,
        graph: &Graph<A, R, H>,
    ) -> Result<ContextView<'s>, SyntaxError<E>> {
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
            .filter(|context| {
                context
                    .last()
                    .and_then(|(node, _)| graph.node(node))
                    .is_some_and(|node| node.command().is_some())
            })
            .ok_or(SyntaxError::library(
                LibraryError::UnknownCommand,
                self.cursor,
            ))
    }

    /// The node whose children suggest at the end of the input, and the start of those
    /// suggestions.
    pub(crate) fn suggestion_parent(&self) -> (NodeId, usize) {
        let cursor = self.input.len();
        let mut parent = (NodeId::ROOT, cursor);
        let mut contexts = self.contexts().peekable();
        while let Some(context) = contexts.next() {
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
        (parent.0, parent.1.min(cursor))
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

        let mut best: Option<((bool, bool), usize)> = None;
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
            let rank = (nested >= input.len(), scratch.errors.len() == error_start);
            match best {
                Some((best_rank, _)) if rank <= best_rank => {
                    scratch.entries.truncate(entry_start);
                    scratch.errors.truncate(error_start);
                }
                _ => {
                    scratch.keep(entry_base, entry_start, error_base, error_start);
                    best = Some((rank, nested));
                }
            }
            if rank == (true, true) {
                break;
            }
        }
        best.map_or(cursor, |(_, nested)| nested)
    }

    /// Suggests what can replace the input before its end, in no order (CMD-SUG-003).
    pub fn suggest<'t, S, E: Copy>(
        &'t self,
        parse: &ParseResult<'t, '_, E>,
        source: &'t S,
        out: &mut Vec<Suggestion<'t, A::Tooltip>>,
    ) where
        A: ArgumentSet<S>,
        R: Requirement<S>,
    {
        let (parent, start) = parse.suggestion_parent();
        let Some(parent) = self.node(parent) else {
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
                NodeKind::Literal(name) => builder
                    .suggest_matching([&**name], |remaining, name| {
                        starts_with_ignore_case(name, remaining)
                    }),
                NodeKind::Argument { argument, .. } => argument.suggest(source, &mut builder),
            }
        }
    }

    /// Reads the value of an argument of a parsed context again. The parse already
    /// validated the span, so this read fails only when the source changed in between.
    pub fn value<'a, S, T>(
        &self,
        input: &'a str,
        span: Span,
        arg: Arg<T>,
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
