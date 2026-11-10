//! Finds every Molang expression in Bedrock packs and reports aggregate statistics.
//!
//! The parse is `mcrs_molang::parse`, so that the tool and the kernel cannot disagree on the
//! grammar. The tool embeds no pack content. It reads the packs from the path that the caller gives,
//! prints statistics as Markdown, and writes the expressions to a JSON Lines file only on
//! request, for a parse coverage gate outside the repository.

#![expect(
    clippy::disallowed_types,
    reason = "a pack file has no fixed shape, so the extractor walks a JSON value"
)]

mod analysis;
mod corpus;
mod extract;
mod json;
mod rate;
mod report;

use std::fs::{self, File};
use std::io::{BufWriter, Write as _};
use std::path::PathBuf;

use anyhow::{Context as _, bail};
use serde::Serialize;

use crate::corpus::Entry;
use crate::rate::SpecQueries;

const USAGE: &str = "usage: molang-corpus <pack-or-samples-dir> --queries <molang-queries.md> [--jsonl <out.jsonl>]";

struct Args {
    root: PathBuf,
    queries: PathBuf,
    jsonl: Option<PathBuf>,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut root = None;
    let mut queries = None;
    let mut jsonl = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--queries") => queries = args.next().map(PathBuf::from),
            Some("--jsonl") => jsonl = args.next().map(PathBuf::from),
            Some("--help" | "-h") => bail!(USAGE),
            _ if root.is_none() => root = Some(PathBuf::from(arg)),
            _ => bail!(USAGE),
        }
    }
    let (Some(root), Some(queries)) = (root, queries) else {
        bail!(USAGE);
    };
    Ok(Args {
        root,
        queries,
        jsonl,
    })
}

fn main() -> anyhow::Result<()> {
    let args = parse_args()?;
    let markdown = fs::read_to_string(&args.queries)
        .with_context(|| format!("read {}", args.queries.display()))?;
    let spec = SpecQueries::parse(&markdown);
    let extraction = extract::extract(&args.root)?;
    let corpus = corpus::build(extraction.occurrences.clone(), &spec);
    if let Some(path) = &args.jsonl {
        write_jsonl(path, &corpus.entries)?;
    }
    print!("{}", report::render(&extraction, &corpus, &spec));
    Ok(())
}

/// One line of the JSON Lines file: an expression and its context.
#[derive(Serialize)]
struct Line<'entry> {
    expr: &'entry str,
    form: &'static str,
    kind: &'static str,
    field: &'entry str,
    file: &'entry str,
    pointer: &'entry str,
    format_version: &'entry str,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'entry str>,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    entries: &'entry [String],
    parse: String,
}

fn write_jsonl(path: &PathBuf, entries: &[Entry]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut writer =
        BufWriter::new(File::create(path).with_context(|| format!("create {}", path.display()))?);
    for entry in entries {
        let occurrence = &entry.occurrence;
        let parse = match &entry.outcome {
            Ok(_) => "ok".to_owned(),
            Err(error) => format!(
                "error at {}: {}: {}",
                error.offset, error.kind, error.message
            ),
        };
        let line = Line {
            expr: &occurrence.text,
            form: occurrence.form.name(),
            kind: occurrence.kind,
            field: &occurrence.field,
            file: &occurrence.file,
            pointer: &occurrence.pointer,
            format_version: &occurrence.format_version,
            version: occurrence.version.as_deref(),
            entries: &occurrence.entries,
            parse,
        };
        serde_json::to_writer(&mut writer, &line)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}
