use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::analysis::Read;
use mcrs_molang::{Body, Rate};

use crate::corpus::{Corpus, Domain, Entry};
use crate::extract::{Extraction, ValueForm};
use crate::rate::{QueryRate, RATES, SpecQueries};

/// Writes the aggregate statistics of the corpus as Markdown. The text has names of queries,
/// functions and fields, and no expression text.
pub fn render(extraction: &Extraction, corpus: &Corpus, spec: &SpecQueries) -> String {
    let mut out = String::new();
    files(&mut out, extraction);
    contexts(&mut out, corpus);
    failures(&mut out, corpus);
    queries(&mut out, corpus, spec);
    math_functions(&mut out, corpus);
    features(&mut out, corpus);
    variables(&mut out, corpus);
    sizes(&mut out, corpus);
    rates(&mut out, corpus);
    out
}

fn parsed(corpus: &Corpus) -> impl Iterator<Item = (&Entry, &crate::corpus::Parse)> {
    corpus
        .entries
        .iter()
        .filter_map(|entry| entry.outcome.as_ref().ok().map(|parse| (entry, parse)))
}

fn files(out: &mut String, extraction: &Extraction) {
    let _ = writeln!(out, "## Files\n");
    let mut versions: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
    for file in &extraction.files {
        *versions
            .entry(file.kind)
            .or_default()
            .entry(file.format_version.as_str())
            .or_default() += 1;
    }
    let _ = writeln!(
        out,
        "| Kind | Files | `format_version` values (files) |\n|---|---|---|"
    );
    for (kind, histogram) in &versions {
        let total: usize = histogram.values().sum();
        let list: Vec<String> = histogram
            .iter()
            .map(|(version, count)| format!("`{version}` ({count})"))
            .collect();
        let _ = writeln!(out, "| {kind} | {total} | {} |", list.join(", "));
    }
    let _ = writeln!(
        out,
        "\nFiles that are not JSON, even with comments: {}",
        extraction.broken.len()
    );
    for broken in &extraction.broken {
        let _ = writeln!(out, "- `{}`: {}", broken.file, broken.error);
    }
    for manifest in &extraction.manifests {
        let _ = writeln!(
            out,
            "\nManifest `{}`: `min_engine_version` {}",
            manifest.file, manifest.min_engine_version
        );
    }
}

#[derive(Default)]
struct Tally {
    occurrences: usize,
    distinct: BTreeSet<String>,
    failures: usize,
}

fn contexts(out: &mut String, corpus: &Corpus) {
    let mut by_context: BTreeMap<String, Tally> = BTreeMap::new();
    let mut by_form: BTreeMap<ValueForm, usize> = BTreeMap::new();
    let mut all_distinct: BTreeSet<&str> = BTreeSet::new();
    for entry in &corpus.entries {
        let tally = by_context.entry(entry.context()).or_default();
        tally.occurrences += 1;
        *by_form.entry(entry.occurrence.form).or_default() += 1;
        match &entry.outcome {
            Ok(parse) => {
                tally.distinct.insert(parse.normal.clone());
                all_distinct.insert(&parse.normal);
            }
            Err(_) => tally.failures += 1,
        }
    }
    let _ = writeln!(
        out,
        "\n## Expressions by context\n\nOccurrences: {}. Distinct parsed expressions: {}.\n",
        corpus.entries.len(),
        all_distinct.len()
    );
    let _ = writeln!(
        out,
        "| Context | Occurrences | Distinct | Parse failures |\n|---|---|---|---|"
    );
    for (context, tally) in &by_context {
        let _ = writeln!(
            out,
            "| `{context}` | {} | {} | {} |",
            tally.occurrences,
            tally.distinct.len(),
            tally.failures
        );
    }
    let _ = writeln!(out, "\n| JSON form | Occurrences |\n|---|---|");
    for (form, count) in &by_form {
        let _ = writeln!(out, "| {} | {count} |", form.name());
    }
    let versions: BTreeSet<&str> = corpus
        .entries
        .iter()
        .filter_map(|entry| entry.occurrence.version.as_deref())
        .collect();
    let _ = writeln!(out, "\nVersions of versioned objects: {versions:?}");
    let split: usize = corpus.entries.iter().map(|entry| entry.split_entries).sum();
    let arrays = corpus
        .entries
        .iter()
        .filter(|entry| entry.split_entries > 0)
        .count();
    let _ = writeln!(
        out,
        "Script arrays with entries that do not parse alone: {arrays} arrays, {split} entries."
    );
}

fn failures(out: &mut String, corpus: &Corpus) {
    let mut buckets: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for entry in &corpus.entries {
        let Err(error) = &entry.outcome else {
            continue;
        };
        *buckets
            .entry(error.kind.to_string())
            .or_default()
            .entry(entry.context())
            .or_default() += 1;
    }
    let _ = writeln!(out, "\n## Parse failures (MOL-SYN-001)\n");
    if buckets.is_empty() {
        let _ = writeln!(out, "None.");
    }
    for (kind, contexts) in &buckets {
        let total: usize = contexts.values().sum();
        let list: Vec<String> = contexts
            .iter()
            .map(|(context, count)| format!("`{context}` {count}"))
            .collect();
        let _ = writeln!(out, "- {kind}: {total} ({})", list.join(", "));
    }
}

#[derive(Default)]
struct QueryTally {
    uses: usize,
    expressions: usize,
    contexts: BTreeMap<String, usize>,
    arities: BTreeSet<usize>,
    non_literal: usize,
}

fn queries(out: &mut String, corpus: &Corpus, spec: &SpecQueries) {
    let mut tallies: BTreeMap<&str, QueryTally> = BTreeMap::new();
    let (mut calls, mut literal, mut with_args, mut via_arrow) = (0, 0, 0, 0);
    for (entry, parse) in parsed(corpus) {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for query in &parse.facts.queries {
            let tally = tallies.entry(&query.name).or_default();
            tally.uses += 1;
            calls += 1;
            if let Some(arity) = query.arity.filter(|arity| *arity > 0) {
                tally.arities.insert(arity);
                with_args += 1;
                if query.literal_args {
                    literal += 1;
                } else {
                    tally.non_literal += 1;
                }
            }
            via_arrow += usize::from(query.via_arrow);
            if seen.insert(&query.name) {
                tally.expressions += 1;
                *tally
                    .contexts
                    .entry(entry.occurrence.kind.to_owned())
                    .or_default() += 1;
            }
        }
    }
    let _ = writeln!(
        out,
        "\n## Queries\n\nDistinct names: {}. Calls: {calls}. Calls with arguments: {with_args}, of which {literal} have only literal arguments. Calls through `->`: {via_arrow}.\n",
        tallies.len()
    );
    let mut ranked: Vec<(&str, &QueryTally)> =
        tallies.iter().map(|(name, tally)| (*name, tally)).collect();
    ranked.sort_by(|left, right| right.1.uses.cmp(&left.1.uses).then(left.0.cmp(right.0)));
    let _ = writeln!(
        out,
        "| Query | Uses | Expressions | MOLQ | Rate | Arities | Non-literal arg calls | File kinds (expressions) |\n|---|---|---|---|---|---|---|---|"
    );
    for (name, tally) in &ranked {
        let (defined, rate) = match spec.rate(name) {
            QueryRate::Spec(rate) => ("yes", rate.class().to_string()),
            QueryRate::Pure => ("yes", "of arguments".to_owned()),
            QueryRate::Guess(rate) => ("no", rate.class().to_string()),
        };
        let rate = if defined == "no" {
            format!("{rate} (guess)")
        } else {
            rate.to_owned()
        };
        let contexts: Vec<String> = tally
            .contexts
            .iter()
            .map(|(kind, count)| format!("{kind} {count}"))
            .collect();
        let arities: Vec<String> = tally.arities.iter().map(usize::to_string).collect();
        let _ = writeln!(
            out,
            "| `{name}` | {} | {} | {defined} | {rate} | {} | {} | {} |",
            tally.uses,
            tally.expressions,
            arities.join(","),
            tally.non_literal,
            contexts.join(", ")
        );
    }
    let undefined: Vec<&(&str, &QueryTally)> = ranked
        .iter()
        .filter(|(name, _)| !spec.defines(name))
        .collect();
    let undefined_uses: usize = undefined.iter().map(|(_, tally)| tally.uses).sum();
    let _ = writeln!(
        out,
        "\nQueries that MOLQ does not define: {} names, {undefined_uses} uses.",
        undefined.len()
    );
    let spec_unused: Vec<&String> = spec
        .rates
        .keys()
        .filter(|name| !tallies.contains_key(name.as_str()))
        .collect();
    let _ = writeln!(
        out,
        "MOLQ queries that the corpus does not use: {spec_unused:?}"
    );
}

fn math_functions(out: &mut String, corpus: &Corpus) {
    let mut tallies: BTreeMap<&str, (usize, BTreeSet<Option<usize>>)> = BTreeMap::new();
    for (_, parse) in parsed(corpus) {
        for call in &parse.facts.math {
            let tally = tallies.entry(&call.name).or_default();
            tally.0 += 1;
            tally.1.insert(call.arity);
        }
    }
    let mut ranked: Vec<_> = tallies.iter().collect();
    ranked.sort_by(|left, right| right.1.0.cmp(&left.1.0).then(left.0.cmp(right.0)));
    let _ = writeln!(
        out,
        "\n## Math functions\n\n| Function | Uses | Arities in corpus | MOL-MATH-002 arity |\n|---|---|---|---|"
    );
    for (name, (uses, arities)) in ranked {
        let found: Vec<String> = arities
            .iter()
            .map(|arity| arity.map_or_else(|| "no list".to_owned(), |arity| arity.to_string()))
            .collect();
        let spec = mcrs_molang::math_arity(name)
            .map_or_else(|| "not listed".to_owned(), |arity| arity.to_string());
        let _ = writeln!(out, "| `{name}` | {uses} | {} | {spec} |", found.join(","));
    }
}

fn features(out: &mut String, corpus: &Corpus) {
    let mut uses: BTreeMap<&str, usize> = BTreeMap::new();
    let mut expressions: BTreeMap<&str, usize> = BTreeMap::new();
    let mut kinds: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (entry, parse) in parsed(corpus) {
        for (feature, count) in &parse.facts.features {
            *uses.entry(feature).or_default() += count;
            *expressions.entry(feature).or_default() += 1;
            kinds
                .entry(feature)
                .or_default()
                .insert(entry.occurrence.kind);
        }
    }
    let _ = writeln!(
        out,
        "\n## Constructs\n\n| Construct | Uses | Expressions | File kinds |\n|---|---|---|---|"
    );
    let mut ranked: Vec<_> = uses.iter().collect();
    ranked.sort_by(|left, right| right.1.cmp(left.1).then(left.0.cmp(right.0)));
    for (feature, count) in ranked {
        let in_expressions = expressions.get(feature).copied().unwrap_or_default();
        let list: Vec<&str> = kinds
            .get(feature)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "| {feature} | {count} | {in_expressions} | {} |",
            list.join(", ")
        );
    }
    let temp_struct = parsed(corpus)
        .filter(|(_, parse)| parse.facts.temp_struct)
        .count();
    let this_outside = parsed(corpus)
        .filter(|(entry, parse)| {
            parse.facts.reads.contains(&Read::This) && entry.occurrence.kind != "animation"
        })
        .count();
    let resource_outside = parsed(corpus)
        .filter(|(entry, parse)| {
            parse.facts.reads.contains(&Read::Resource)
                && entry.occurrence.kind != "render_controller"
        })
        .count();
    let _ = writeln!(
        out,
        "\n`t.a.b` paths: {temp_struct} expressions. `this` outside animations: {this_outside}. Resource or array references outside render controllers: {resource_outside}."
    );
}

fn variables(out: &mut String, corpus: &Corpus) {
    let mut names: BTreeMap<Domain, BTreeSet<&str>> = BTreeMap::new();
    let mut contexts: BTreeSet<&str> = BTreeSet::new();
    let mut struct_paths: BTreeSet<&str> = BTreeSet::new();
    let mut external_reads: BTreeMap<&str, usize> = BTreeMap::new();
    for (entry, parse) in parsed(corpus) {
        for name in &parse.facts.variable_names {
            names.entry(entry.domain()).or_default().insert(name);
            if name.contains('.') {
                struct_paths.insert(name);
            }
        }
        for read in &parse.facts.reads {
            match read {
                Read::Context(name) => {
                    contexts.insert(name);
                }
                Read::Variable(name) => {
                    let key = crate::corpus::VariableKey {
                        domain: entry.domain(),
                        name: name.clone(),
                    };
                    if corpus.external.contains(&key) {
                        *external_reads.entry(name).or_default() += 1;
                    }
                }
                _ => {}
            }
        }
    }
    let _ = writeln!(out, "\n## Variables\n");
    for (domain, set) in &names {
        let _ = writeln!(out, "- {domain:?} variables: {} distinct names.", set.len());
    }
    let _ = writeln!(
        out,
        "- Struct paths (`v.a.b`): {} distinct.",
        struct_paths.len()
    );
    let _ = writeln!(out, "- `context.` names: {contexts:?}");
    let mut by_rate: BTreeMap<(Domain, Rate), usize> = BTreeMap::new();
    for (key, rate) in &corpus.variables {
        *by_rate.entry((key.domain, rate.class())).or_default() += 1;
    }
    let list: Vec<String> = by_rate
        .iter()
        .map(|((domain, class), count)| format!("{domain:?} {} {count}", class))
        .collect();
    let _ = writeln!(
        out,
        "- Written variables by rate class: {}",
        list.join(", ")
    );
    let mut ranked: Vec<_> = external_reads.into_iter().collect();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(right.0)));
    let list: Vec<String> = ranked
        .iter()
        .map(|(name, count)| format!("`{name}` {count}"))
        .collect();
    let _ = writeln!(
        out,
        "- Read but never written in the corpus ({}): {}",
        ranked.len(),
        list.join(", ")
    );
}

fn percentiles(mut values: Vec<usize>) -> String {
    values.sort_unstable();
    let at = |quantile: f64| {
        let rank = (quantile * values.len() as f64).ceil() as usize;
        values
            .get(rank.saturating_sub(1))
            .copied()
            .unwrap_or_default()
    };
    format!(
        "{} | {} | {} | {} | {}",
        values.len(),
        at(0.5),
        at(0.9),
        at(0.99),
        values.last().copied().unwrap_or_default()
    )
}

fn sizes(out: &mut String, corpus: &Corpus) {
    let mut distinct: BTreeMap<&str, &crate::corpus::Parse> = BTreeMap::new();
    for (_, parse) in parsed(corpus) {
        distinct.insert(&parse.normal, parse);
    }
    let all: Vec<&crate::corpus::Parse> = parsed(corpus).map(|(_, parse)| parse).collect();
    let unique: Vec<&crate::corpus::Parse> = distinct.values().copied().collect();
    let _ = writeln!(
        out,
        "\n## Sizes\n\n| Measure | Set | Count | p50 | p90 | p99 | max |\n|---|---|---|---|---|---|---|"
    );
    for (set_name, set) in [("occurrences", &all), ("distinct", &unique)] {
        let nodes = set.iter().map(|parse| parse.facts.nodes).collect();
        let complex: Vec<&&crate::corpus::Parse> = set
            .iter()
            .filter(|parse| matches!(parse.syntax.body, Body::Complex(_)))
            .collect();
        let statements = complex.iter().map(|parse| parse.facts.statements).collect();
        let _ = writeln!(out, "| AST nodes | {set_name} | {} |", percentiles(nodes));
        let _ = writeln!(
            out,
            "| statements of complex expressions | {set_name} | {} |",
            percentiles(statements)
        );
    }
    let simple = all
        .iter()
        .filter(|parse| matches!(parse.syntax.body, Body::Simple(_)))
        .count();
    let _ = writeln!(
        out,
        "\nSimple expressions: {simple} of {} occurrences.",
        all.len()
    );
}

fn rates(out: &mut String, corpus: &Corpus) {
    rate_table(
        out,
        corpus,
        "Rate classes (occurrences, all JSON forms)",
        RateSet::All,
    );
    rate_table(
        out,
        corpus,
        "Rate classes (occurrences of Molang text only)",
        RateSet::Text,
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RateSet {
    All,
    Text,
}

fn rate_table(out: &mut String, corpus: &Corpus, title: &str, set: RateSet) {
    let mut table: BTreeMap<&str, BTreeMap<Rate, usize>> = BTreeMap::new();
    let mut random: BTreeMap<&str, usize> = BTreeMap::new();
    for (entry, _) in parsed(corpus) {
        let is_text = !matches!(entry.occurrence.form, ValueForm::Number | ValueForm::Bool);
        if set == RateSet::Text && !is_text {
            continue;
        }
        let group = rate_group(entry);
        *table
            .entry(group)
            .or_default()
            .entry(entry.rate)
            .or_default() += 1;
        *table
            .entry("all")
            .or_default()
            .entry(entry.rate)
            .or_default() += 1;
        if entry.random {
            *random.entry(group).or_default() += 1;
            *random.entry("all").or_default() += 1;
        }
    }
    let header: Vec<String> = RATES.iter().map(Rate::to_string).collect();
    let _ = writeln!(
        out,
        "\n## {title}\n\n| Group | Total | {} | random |\n|---|---|{}---|",
        header.join(" | "),
        "---|".repeat(header.len())
    );
    for (group, counts) in &table {
        let total: usize = counts.values().sum();
        let cells: Vec<String> = RATES
            .iter()
            .map(|class| {
                let count = counts.get(class).copied().unwrap_or_default();
                format!(
                    "{count} ({:.1}%)",
                    100.0 * count as f64 / total.max(1) as f64
                )
            })
            .collect();
        let _ = writeln!(
            out,
            "| {group} | {total} | {} | {} |",
            cells.join(" | "),
            random.get(group).copied().unwrap_or_default()
        );
    }
}

fn rate_group(entry: &Entry) -> &'static str {
    let field = entry.occurrence.field.as_str();
    match entry.occurrence.kind {
        "animation" if field.starts_with("bones.") && field.ends_with(".keyframe") => {
            "animation keyframe"
        }
        "animation" if field.starts_with("bones.") => "animation bone channel",
        "animation" => "animation other",
        "client_entity" | "attachable" => "client entity and attachable",
        "animation_controller" => "animation controller",
        "render_controller" => "render controller",
        "particle" => "particle",
        _ => "other",
    }
}
