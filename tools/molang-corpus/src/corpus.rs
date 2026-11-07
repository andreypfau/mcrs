use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::{self, Facts, Read};
use crate::ast::{Expr, Form, Program, Stmt};
use crate::extract::{Occurrence, ValueForm};
use crate::lexer::{Token, TokenKind};
use crate::parser::{self, ParseError, Parsed};
use crate::rate::{Rate, RateClass, SpecQueries};

/// The store of `variable.` values that a field writes: particles have their own subjects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Domain {
    Entity,
    Particle,
}

#[derive(Clone, Debug)]
pub struct Parse {
    pub parsed: Parsed,
    pub facts: Facts,
    /// The token text with names folded, the key of a distinct expression.
    pub normal: String,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub occurrence: Occurrence,
    pub outcome: Result<Parse, ParseError>,
    /// Entries of a script array that fail to parse alone, while the join parses.
    pub split_entries: usize,
    pub rate: RateClass,
    pub random: bool,
}

impl Entry {
    pub fn context(&self) -> String {
        format!("{}/{}", self.occurrence.kind, self.occurrence.field)
    }

    /// `this` is the value of a bone channel in an animation (`entity-animation.md`); the
    /// other consumers do not state it, so the report takes a per-frame value.
    fn this_rate(&self) -> Rate {
        if self.occurrence.kind == "animation" {
            Rate::of(RateClass::AnimationFrame)
        } else {
            Rate::of(RateClass::EntityFrame)
        }
    }

    pub fn domain(&self) -> Domain {
        if self.occurrence.kind == "particle" {
            Domain::Particle
        } else {
            Domain::Entity
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct VariableKey {
    pub domain: Domain,
    pub name: String,
}

pub struct Corpus {
    pub entries: Vec<Entry>,
    pub variables: BTreeMap<VariableKey, Rate>,
    /// Variables that some expression reads and no expression of the corpus writes.
    pub external: BTreeSet<VariableKey>,
}

pub fn build(occurrences: Vec<Occurrence>, spec: &SpecQueries) -> Corpus {
    let mut entries: Vec<Entry> = occurrences.into_iter().map(parse_entry).collect();
    let (variables, external) = variable_rates(&entries, spec);
    for entry in &mut entries {
        let Ok(parse) = &entry.outcome else {
            continue;
        };
        let rate = reads_rate(
            &parse.facts.reads,
            entry.domain(),
            entry.this_rate(),
            &variables,
            spec,
        );
        entry.rate = rate.class();
        entry.random = parse.facts.reads.contains(&Read::Random);
    }
    Corpus {
        entries,
        variables,
        external,
    }
}

fn parse_entry(occurrence: Occurrence) -> Entry {
    let outcome = parse_value(&occurrence).map(|parsed| {
        let facts = analysis::analyse(&parsed.program);
        let normal = parsed
            .tokens
            .iter()
            .map(|token| token.kind.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        Parse {
            parsed,
            facts,
            normal,
        }
    });
    let split_entries = if outcome.is_ok() {
        occurrence
            .entries
            .iter()
            .filter(|entry| parser::parse(entry).is_err())
            .count()
    } else {
        0
    };
    Entry {
        occurrence,
        outcome,
        split_entries,
        rate: RateClass::Constant,
        random: false,
    }
}

/// Parses a string; a JSON number or Boolean is a constant with no parse (MOL-LOAD-001).
fn parse_value(occurrence: &Occurrence) -> Result<Parsed, ParseError> {
    let value = match occurrence.form {
        ValueForm::Number => occurrence.text.parse::<f64>().unwrap_or(0.0),
        ValueForm::Bool if occurrence.text == "true" => 1.0,
        ValueForm::Bool => 0.0,
        _ => return parser::parse(&occurrence.text),
    };
    Ok(Parsed {
        program: Program {
            form: Form::Simple,
            statements: vec![Stmt::Expr(Expr::Number(value))],
        },
        tokens: vec![Token {
            kind: TokenKind::Number(value),
            offset: 0,
        }],
    })
}

fn read_rate(
    read: &Read,
    domain: Domain,
    this_rate: Rate,
    variables: &BTreeMap<VariableKey, Rate>,
    spec: &SpecQueries,
) -> Rate {
    match read {
        Read::Query(name) => spec.rate(name).rate(),
        Read::Variable(name) => {
            let key = VariableKey {
                domain,
                name: name.clone(),
            };
            variables
                .get(&key)
                .copied()
                .unwrap_or(Rate::of(RateClass::EntityFrame))
        }
        Read::Context(_) => Rate::of(RateClass::EntityTick),
        Read::This => this_rate,
        Read::Random => Rate::CONSTANT,
        Read::Resource => Rate::of(RateClass::Load),
    }
}

fn reads_rate(
    reads: &BTreeSet<Read>,
    domain: Domain,
    this_rate: Rate,
    variables: &BTreeMap<VariableKey, Rate>,
    spec: &SpecQueries,
) -> Rate {
    reads.iter().fold(Rate::CONSTANT, |rate, read| {
        rate.join(read_rate(read, domain, this_rate, variables, spec))
    })
}

/// Gives each written variable the join of the rates of its writes, to a fixed point. A
/// variable that no expression writes is external: the engine supplies it.
fn variable_rates(
    entries: &[Entry],
    spec: &SpecQueries,
) -> (BTreeMap<VariableKey, Rate>, BTreeSet<VariableKey>) {
    let mut variables: BTreeMap<VariableKey, Rate> = BTreeMap::new();
    let mut read: BTreeSet<VariableKey> = BTreeSet::new();
    for entry in entries {
        let Ok(parse) = &entry.outcome else {
            continue;
        };
        for write in &parse.facts.writes {
            let key = VariableKey {
                domain: entry.domain(),
                name: write.name.clone(),
            };
            variables.insert(key, Rate::CONSTANT);
        }
        for value in &parse.facts.reads {
            if let Read::Variable(name) = value {
                read.insert(VariableKey {
                    domain: entry.domain(),
                    name: name.clone(),
                });
            }
        }
    }
    let external: BTreeSet<VariableKey> = read
        .into_iter()
        .filter(|key| !variables.contains_key(key))
        .collect();
    for _ in 0..32 {
        let mut changed = false;
        for entry in entries {
            let Ok(parse) = &entry.outcome else {
                continue;
            };
            for write in &parse.facts.writes {
                let mut rate = reads_rate(
                    &write.deps,
                    entry.domain(),
                    entry.this_rate(),
                    &variables,
                    spec,
                );
                if entry.occurrence.field == "scripts.initialize" {
                    rate = rate.frozen();
                }
                let key = VariableKey {
                    domain: entry.domain(),
                    name: write.name.clone(),
                };
                let slot = variables.entry(key).or_default();
                let joined = slot.join(rate);
                changed |= joined != *slot;
                *slot = joined;
            }
        }
        if !changed {
            break;
        }
    }
    (variables, external)
}
