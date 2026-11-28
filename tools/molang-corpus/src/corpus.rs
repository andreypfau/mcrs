use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::{self, Facts, Read};
use mcrs_molang::{Body, Diagnostic, Expr, ExprKind, Rate, Span, Syntax, parse};

use crate::extract::{Occurrence, ValueForm};
use crate::rate::{Axes, SpecQueries};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Domain {
    Entity,
    Particle,
}

#[derive(Clone, Debug)]
pub struct Parse {
    pub syntax: Syntax,
    pub facts: Facts,
    pub normal: String,
}

#[derive(Clone, Debug)]
pub struct ParseFailure {
    pub offset: u32,
    pub kind: &'static str,
    pub message: String,
}

impl From<Diagnostic> for ParseFailure {
    fn from(diagnostic: Diagnostic) -> Self {
        Self {
            offset: diagnostic.span.start,
            kind: diagnostic.kind.name(),
            message: diagnostic.message,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub occurrence: Occurrence,
    pub outcome: Result<Parse, ParseFailure>,
    pub split_entries: usize,
    pub rate: Rate,
    pub random: bool,
}

impl Entry {
    pub fn context(&self) -> String {
        format!("{}/{}", self.occurrence.kind, self.occurrence.field)
    }

    fn this_rate(&self) -> Axes {
        if self.occurrence.kind == "animation" {
            Axes::of(Rate::AnimationFrame)
        } else {
            Axes::of(Rate::EntityFrame)
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
    pub variables: BTreeMap<VariableKey, Axes>,
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
    let outcome = parse_value(&occurrence).map(|syntax| {
        let facts = analysis::analyse(&syntax);
        Parse {
            syntax,
            facts,
            normal: normalise(&occurrence.text),
        }
    });
    let split_entries = if outcome.is_ok() {
        occurrence
            .entries
            .iter()
            .filter(|entry| parse(entry).is_err())
            .count()
    } else {
        0
    };
    Entry {
        occurrence,
        outcome,
        split_entries,
        rate: Rate::Constant,
        random: false,
    }
}

fn parse_value(occurrence: &Occurrence) -> Result<Syntax, ParseFailure> {
    let value = match occurrence.form {
        ValueForm::Number => occurrence.text.parse::<f32>().unwrap_or(0.0),
        ValueForm::Bool if occurrence.text == "true" => 1.0,
        ValueForm::Bool => 0.0,
        _ => {
            return parse(&occurrence.text).map_err(|errors| {
                errors
                    .into_iter()
                    .next()
                    .map_or_else(unknown_failure, ParseFailure::from)
            });
        }
    };
    Ok(Syntax {
        body: Body::Simple(Expr {
            kind: ExprKind::Number(value),
            span: Span::default(),
        }),
    })
}

fn unknown_failure() -> ParseFailure {
    ParseFailure {
        offset: 0,
        kind: "syntax",
        message: "the parser reported no error".to_owned(),
    }
}

fn normalise(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    for c in text.chars() {
        if in_string {
            out.push(c);
            in_string = c != '\'';
        } else if c == '\'' {
            in_string = true;
            out.push(c);
        } else if !c.is_whitespace() {
            out.push(c.to_ascii_lowercase());
        }
    }
    out
}

fn read_rate(
    read: &Read,
    domain: Domain,
    this_rate: Axes,
    variables: &BTreeMap<VariableKey, Axes>,
    spec: &SpecQueries,
) -> Axes {
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
                .unwrap_or(Axes::of(Rate::EntityFrame))
        }
        Read::Context(_) => Axes::of(Rate::EntityTick),
        Read::This => this_rate,
        Read::Random => Axes::CONSTANT,
        Read::Resource => Axes::of(Rate::Load),
    }
}

fn reads_rate(
    reads: &BTreeSet<Read>,
    domain: Domain,
    this_rate: Axes,
    variables: &BTreeMap<VariableKey, Axes>,
    spec: &SpecQueries,
) -> Axes {
    reads.iter().fold(Axes::CONSTANT, |rate, read| {
        rate.join(read_rate(read, domain, this_rate, variables, spec))
    })
}

fn variable_rates(
    entries: &[Entry],
    spec: &SpecQueries,
) -> (BTreeMap<VariableKey, Axes>, BTreeSet<VariableKey>) {
    let mut variables: BTreeMap<VariableKey, Axes> = BTreeMap::new();
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
            variables.insert(key, Axes::CONSTANT);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::Extraction;
    use crate::report;

    fn occurrence(text: &str) -> Occurrence {
        Occurrence {
            text: text.to_owned(),
            form: ValueForm::String,
            entries: Vec::new(),
            version: None,
            kind: "client_entity",
            field: "scripts.pre_animation".to_owned(),
            file: "entity/a.json".to_owned(),
            pointer: "/a".to_owned(),
            format_version: "1.10.0".to_owned(),
        }
    }

    #[test]
    fn the_report_counts_rates_writes_and_parse_failures() {
        let spec = SpecQueries::parse(
            "| Name | Rate class |\n|---|---|\n| `life_time` | `entity_frame` |\n",
        );
        let occurrences = ["q.life_time * 2", "v.x = 1; return v.x;", "1 +"]
            .map(occurrence)
            .to_vec();
        let corpus = build(occurrences, &spec);
        assert_eq!(corpus.entries[0].rate, Rate::EntityFrame);
        assert_eq!(corpus.entries[1].rate, Rate::Constant);
        let facts = &corpus.entries[1].outcome.as_ref().unwrap().facts;
        assert_eq!(facts.writes.len(), 1);
        assert_eq!(
            corpus.entries[2].outcome.as_ref().unwrap_err().kind,
            "syntax"
        );
        let text = report::render(&Extraction::default(), &corpus, &spec);
        assert!(text.contains("- syntax: 1 (`client_entity/scripts.pre_animation` 1)"));
        assert!(text.contains("| `life_time` | 1 | 1 | yes | entity_frame |"));
    }
}
