use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, bail};

use crate::lang::Lang;
use crate::summary_input::{
    self, ClientIn, HygieneIn, MobRunIn, PhaseIn, ReportIn, ScenarioIn, SystemIn, WorldgenIn,
};

pub const USAGE: &str = "usage: perf-kit summary <run dir> [--lang en|ru] [--zip <path>]";

const HEADLINE_PHASE: &str = "crowd-stir";
const NOT_ASKED: &str = "not asked";
const MESSAGE_WIDTH: usize = 110;

struct Arguments {
    dir: PathBuf,
    lang: Lang,
    zip: Option<String>,
}

/// Prints the summary of one run folder; only an unreadable `report.json` fails.
#[must_use]
pub fn run(arguments: &[String]) -> ExitCode {
    let parsed = match parse(arguments) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{error:#}");
            return ExitCode::from(2);
        }
    };
    let report = match summary_input::read_report(&parsed.dir) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("{error:#}");
            return ExitCode::FAILURE;
        }
    };
    let hygiene: HygieneIn = summary_input::read_optional(&parsed.dir.join("hygiene.json"));
    let system: SystemIn = summary_input::read_optional(&parsed.dir.join("system.json"));
    let pigs = report
        .mob
        .iter()
        .flatten()
        .find(|run| run.name == "mcrs-none")
        .and_then(|run| summary_input::read_pigs(&parsed.dir, run));
    let lines = render(
        parsed.lang,
        &Inputs {
            report: &report,
            hygiene: &hygiene,
            system: &system,
            pigs,
            zip: parsed.zip.as_deref(),
        },
    );
    println!("{}", lines.join("\n"));
    ExitCode::SUCCESS
}

fn parse(arguments: &[String]) -> anyhow::Result<Arguments> {
    let mut dir = None;
    let mut lang = Lang::from_env();
    let mut zip = None;
    let mut options = arguments.iter();
    while let Some(option) = options.next() {
        match option.as_str() {
            "--lang" => {
                lang = options
                    .next()
                    .context("--lang needs en or ru")?
                    .parse()
                    .context(USAGE)?;
            }
            "--zip" => zip = Some(options.next().context("--zip needs a path")?.clone()),
            other if other.starts_with("--") => bail!("unknown option {other}\n{USAGE}"),
            other => dir = Some(PathBuf::from(other)),
        }
    }
    let Some(dir) = dir else {
        bail!("{USAGE}");
    };
    Ok(Arguments { dir, lang, zip })
}

struct Inputs<'a> {
    report: &'a ReportIn,
    hygiene: &'a HygieneIn,
    system: &'a SystemIn,
    pigs: Option<u64>,
    zip: Option<&'a str>,
}

fn render(lang: Lang, inputs: &Inputs) -> Vec<String> {
    let report = inputs.report;
    let mut lines = vec![
        format!(
            "{}: {}",
            lang.pick("Machine", "Машина"),
            machine(inputs.system)
        ),
        format!(
            "{}: {}; {}: {}",
            lang.pick("Profile", "Профиль"),
            report.profile,
            lang.pick("sections", "секции"),
            report.options.sections.join(", ")
        ),
    ];
    if let Some(mob) = &report.mob {
        lines.push(String::new());
        lines.extend(mob_lines(lang, mob, inputs.pigs));
    }
    if let Some(client) = &report.client {
        lines.push(String::new());
        lines.extend(client_lines(lang, client));
    }
    if report.server_start.is_some() || report.worldgen.is_some() {
        lines.push(String::new());
        lines.extend(start_lines(lang, report));
    }
    lines.push(String::new());
    lines.extend(problem_lines(lang, report, inputs.hygiene));
    if let Some(zip) = inputs.zip {
        lines.push(String::new());
        lines.push(format!("{}: {zip}", lang.pick("Zip", "Архив")));
    }
    lines
}

fn machine(system: &SystemIn) -> String {
    let cpu = system.cpu.as_ref().and_then(|cpu| cpu.brand.as_deref());
    let memory = system
        .memory_bytes
        .map(|bytes| format!("{} GB", bytes.saturating_add(1 << 29) >> 30));
    let gpu = system
        .gpus
        .iter()
        .flatten()
        .find_map(|gpu| gpu.model.as_deref());
    let os = system.os.as_ref().map(|os| {
        format!(
            "{} {}",
            os.name.as_deref().unwrap_or("-"),
            os.version.as_deref().unwrap_or("")
        )
        .trim_end()
        .to_owned()
    });
    [cpu.map(str::to_owned), memory, gpu.map(str::to_owned), os]
        .map(|part| part.unwrap_or_else(|| "-".to_owned()))
        .join(", ")
}

fn mob_lines(lang: Lang, mob: &[MobRunIn], pigs: Option<u64>) -> Vec<String> {
    let mut lines = vec![
        lang.pick(
            "Mob, no client: TPS and how many times mcrs is faster by full tick time",
            "Мобы, без клиента: TPS и во сколько раз mcrs быстрее по полному тику",
        )
        .to_owned(),
    ];
    let mcrs = mob.iter().find(|run| run.name == "mcrs-none");
    let vanilla = mob.iter().find(|run| run.name == "vanilla-none");
    let (Some(mcrs), Some(vanilla)) = (mcrs, vanilla) else {
        lines.push("-".to_owned());
        return lines;
    };
    let mut rows = vec![vec![
        lang.pick("phase", "фаза").to_owned(),
        lang.pick("mcrs TPS", "mcrs TPS").to_owned(),
        lang.pick("vanilla TPS", "vanilla TPS").to_owned(),
        lang.pick("faster", "быстрее").to_owned(),
    ]];
    for ours in &mcrs.phases {
        let theirs = vanilla
            .phases
            .iter()
            .find(|phase| phase.phase == ours.phase);
        rows.push(phase_row(lang, ours, theirs));
    }
    lines.extend(table(&rows));
    if let (Some(ours), Some(theirs)) = (headline(mcrs), headline(vanilla)) {
        lines.push(headline_sentence(lang, pigs, ours, theirs));
    }
    for run in mob {
        if let Some(error) = &run.error {
            lines.push(format!("{}: {}", run.name, first_line(error)));
        }
    }
    lines.extend(watcher_line(lang, mob));
    lines
}

fn headline(run: &MobRunIn) -> Option<&PhaseIn> {
    run.phases
        .iter()
        .find(|phase| phase.phase == HEADLINE_PHASE)
}

fn phase_row(lang: Lang, ours: &PhaseIn, theirs: Option<&PhaseIn>) -> Vec<String> {
    let skipped = ours
        .skipped
        .as_ref()
        .or_else(|| theirs.and_then(|phase| phase.skipped.as_ref()));
    if let Some(reason) = skipped {
        return vec![
            ours.phase.clone(),
            format!("{}: {reason}", lang.pick("skipped", "пропущено")),
        ];
    }
    let ratio = match (
        theirs.and_then(|phase| phase.sprint_wall_ms_per_tick),
        ours.sprint_wall_ms_per_tick,
    ) {
        (Some(vanilla), Some(mcrs)) if mcrs > 0.0 => Some(vanilla / mcrs),
        _ => None,
    };
    vec![
        ours.phase.clone(),
        number(ours.console_tps, 1),
        number(theirs.and_then(|phase| phase.console_tps), 1),
        ratio.map_or_else(|| "-".to_owned(), |ratio| format!("{ratio:.1}x")),
    ]
}

fn headline_sentence(lang: Lang, pigs: Option<u64>, ours: &PhaseIn, theirs: &PhaseIn) -> String {
    let pigs = pigs.map_or_else(|| "-".to_owned(), |pigs| thousands(lang, pigs));
    let (mcrs_tps, vanilla_tps) = (number(ours.console_tps, 1), number(theirs.console_tps, 1));
    let (mcrs_ms, vanilla_ms) = (
        milliseconds(ours.sprint_wall_ms_per_tick),
        milliseconds(theirs.sprint_wall_ms_per_tick),
    );
    match lang {
        Lang::En => format!(
            "With {pigs} pigs crowding ({HEADLINE_PHASE}), mcrs runs at {mcrs_tps} TPS and vanilla at {vanilla_tps} TPS; a full mcrs tick takes {mcrs_ms} ms against {vanilla_ms} ms."
        ),
        Lang::Ru => format!(
            "С {pigs} свиней в загоне ({HEADLINE_PHASE}) mcrs держит {mcrs_tps} TPS, vanilla {vanilla_tps} TPS; полный тик mcrs занимает {mcrs_ms} мс против {vanilla_ms} мс."
        ),
    }
}

fn watcher_line(lang: Lang, mob: &[MobRunIn]) -> Option<String> {
    let range = |name: &str| -> Option<String> {
        let run = mob.iter().find(|run| run.name == name)?;
        let values: Vec<f64> = run
            .phases
            .iter()
            .filter(|phase| phase.skipped.is_none())
            .filter_map(|phase| phase.console_tps)
            .collect();
        let low = values.iter().copied().reduce(f64::min)?;
        let high = values.iter().copied().reduce(f64::max)?;
        Some(if low == high {
            format!("{low:.1}")
        } else {
            format!("{low:.1}-{high:.1}")
        })
    };
    let (mcrs, vanilla) = (range("mcrs-watcher"), range("vanilla-watcher"));
    if mcrs.is_none() && vanilla.is_none() {
        return None;
    }
    Some(format!(
        "{}: mcrs {} TPS, vanilla {} TPS",
        lang.pick("With a watcher client", "С клиентом-наблюдателем"),
        mcrs.as_deref().unwrap_or("-"),
        vanilla.as_deref().unwrap_or("-"),
    ))
}

fn client_lines(lang: Lang, client: &ClientIn) -> Vec<String> {
    let mut lines = vec![
        lang.pick(
            "Client: fps and frame median, cold / hot",
            "Клиент: fps и медиана кадра, холодный / горячий запуск",
        )
        .to_owned(),
    ];
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for entry in &client.scenarios {
        let key = (entry.scenario.as_str(), entry.mode.as_str());
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let group: Vec<_> = client
            .scenarios
            .iter()
            .filter(|other| (other.scenario.as_str(), other.mode.as_str()) == key)
            .collect();
        let of_clock = |clock: &str| group.iter().find(|other| other.clock == clock);
        let (cold, hot) = (of_clock("cold"), of_clock("hot"));
        let fps = |entry: Option<&&ScenarioIn>| number(entry.and_then(|entry| entry.median.fps), 0);
        let frame = |entry: Option<&&ScenarioIn>| {
            number(entry.and_then(|entry| entry.median.frame_median_ms), 2)
        };
        let mut marks = Vec::new();
        if group
            .iter()
            .flat_map(|entry| &entry.runs)
            .any(|run| run.error.is_some())
        {
            marks.push(lang.pick("run errors", "ошибки в прогонах"));
        }
        if group
            .iter()
            .flat_map(|entry| &entry.runs)
            .any(|run| run.window_mismatch.is_some())
        {
            marks.push(lang.pick("window size differs", "размер окна другой"));
        }
        rows.push(vec![
            key.0.to_owned(),
            key.1.to_owned(),
            format!("{} / {} fps", fps(cold), fps(hot)),
            format!("{} / {} {}", frame(cold), frame(hot), lang.pick("ms", "мс")),
            marks.join(", "),
        ]);
    }
    lines.extend(table(&rows));
    lines
}

fn start_lines(lang: Lang, report: &ReportIn) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(start) = &report.server_start {
        lines.push(format!(
            "{}: {} {}",
            lang.pick("Server start", "Запуск сервера"),
            number(start.playing_seconds_median, 1),
            lang.pick("s", "с")
        ));
    }
    if let Some(benches) = &report.worldgen {
        lines.push(worldgen_line(lang, benches));
    }
    lines
}

fn worldgen_line(lang: Lang, benches: &[WorldgenIn]) -> String {
    let columns: Vec<String> = benches
        .iter()
        .filter(|bench| bench.bench == "overworld_pipeline")
        .map(|bench| number(bench.per_column_ms, 2))
        .collect();
    let chunks = benches
        .iter()
        .find(|bench| bench.bench == "beta_chunks")
        .map(|bench| number(bench.throughput_chunks_per_second, 0));
    let columns = if columns.is_empty() {
        "-".to_owned()
    } else {
        columns.join(" / ")
    };
    let mut line = format!(
        "{}: {columns} {}",
        lang.pick("World generation", "Генерация мира"),
        lang.pick("ms per column", "мс на колонку")
    );
    if let Some(chunks) = chunks {
        line.push_str(&format!(
            "; beta_chunks {chunks} {}",
            lang.pick("chunks/s", "чанков/с")
        ));
    }
    line
}

fn problem_lines(lang: Lang, report: &ReportIn, hygiene: &HygieneIn) -> Vec<String> {
    let mut problems: Vec<String> = report
        .errors
        .iter()
        .map(|error| format!("[{}] {}", error.section, first_line(&error.message)))
        .collect();
    for check in hygiene
        .checks
        .iter()
        .filter(|check| !check.ok && check.value != NOT_ASKED)
    {
        problems.push(format!("{}: {}", check.name, check.value));
    }
    let title = lang.pick("Problems", "Проблемы");
    if problems.is_empty() {
        return vec![format!("{title}: {}", lang.pick("none", "нет"))];
    }
    let mut lines = vec![format!("{title}:")];
    lines.extend(problems.into_iter().map(|problem| format!("  {problem}")));
    lines
}

fn first_line(message: &str) -> String {
    let line = message.lines().next().unwrap_or("");
    if line.chars().count() <= MESSAGE_WIDTH {
        return line.to_owned();
    }
    let cut: String = line.chars().take(MESSAGE_WIDTH).collect();
    format!("{cut}...")
}

fn number(value: Option<f64>, decimals: usize) -> String {
    value.map_or_else(|| "-".to_owned(), |value| format!("{value:.decimals$}"))
}

/// One decimal below 100 ms, whole milliseconds above.
fn milliseconds(value: Option<f64>) -> String {
    match value {
        Some(value) if value < 100.0 => number(Some(value), 1),
        other => number(other, 0),
    }
}

fn thousands(lang: Lang, value: u64) -> String {
    let digits = value.to_string();
    let separator = lang.pick(",", " ");
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push_str(separator);
        }
        out.push(digit);
    }
    out
}

/// Pads every column but the last to its widest cell.
fn table(rows: &[Vec<String>]) -> Vec<String> {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..columns)
        .map(|column| {
            rows.iter()
                .filter(|row| row.len() > column + 1)
                .filter_map(|row| row.get(column))
                .map(|cell| cell.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    rows.iter()
        .map(|row| {
            let mut line = String::from("  ");
            for (column, cell) in row.iter().enumerate() {
                line.push_str(cell);
                if column + 1 < row.len() {
                    let width = widths.get(column).copied().unwrap_or(0);
                    let pad = (width + 2).saturating_sub(cell.chars().count());
                    line.push_str(&" ".repeat(pad));
                }
            }
            line.trim_end().to_owned()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_json(name: &str, tps: f64, sprint: f64) -> String {
        format!(
            r#"{{"name": "{name}", "report": "mob/{name}.json", "error": null, "phases": [
                {{"phase": "idle", "skipped": null, "console_tps": 20.0, "sprint_wall_ms_per_tick": 1.0}},
                {{"phase": "stimulus", "skipped": "no pigs", "console_tps": null, "sprint_wall_ms_per_tick": null}},
                {{"phase": "crowd-stir", "skipped": null, "console_tps": {tps}, "sprint_wall_ms_per_tick": {sprint}}}
            ]}}"#
        )
    }

    fn report() -> ReportIn {
        let text = format!(
            r#"{{"profile": "quick", "options": {{"sections": ["mob"]}}, "mob": [{}, {}],
                "client": null, "server_start": null, "worldgen": null, "errors": []}}"#,
            run_json("mcrs-none", 20.0, 13.43),
            run_json("vanilla-none", 2.09, 479.24),
        );
        serde_json::from_str(&text).expect("the hand-built report matches the input types")
    }

    #[test]
    fn headline_sentence_and_skipped_phase_in_both_languages() {
        let report = report();
        let cases = [
            (
                Lang::En,
                "With 50,000 pigs crowding (crowd-stir), mcrs runs at 20.0 TPS and vanilla at 2.1 TPS; a full mcrs tick takes 13.4 ms against 479 ms.",
                "skipped: no pigs",
                "Problems: none",
            ),
            (
                Lang::Ru,
                "С 50 000 свиней в загоне (crowd-stir) mcrs держит 20.0 TPS, vanilla 2.1 TPS; полный тик mcrs занимает 13.4 мс против 479 мс.",
                "пропущено: no pigs",
                "Проблемы: нет",
            ),
        ];
        for (lang, sentence, skipped, problems) in cases {
            let lines = render(
                lang,
                &Inputs {
                    report: &report,
                    hygiene: &HygieneIn::default(),
                    system: &SystemIn::default(),
                    pigs: Some(50_000),
                    zip: None,
                },
            );
            assert!(
                lines.iter().any(|line| line == sentence),
                "{lang:?}: {lines:#?}"
            );
            assert!(
                lines.iter().any(|line| line.contains(skipped)),
                "{lang:?}: {lines:#?}"
            );
            assert!(
                lines.iter().any(|line| line == problems),
                "{lang:?}: {lines:#?}"
            );
        }
    }
}
