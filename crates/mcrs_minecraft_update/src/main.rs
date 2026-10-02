use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use mcrs_minecraft_client_jar::{Directory, get, official_dir};
use mcrs_minecraft_update::{corpus, definitions, fixtures, gradle, registries, release};

const USAGE: &str = "\
usage: mcrs_minecraft_update <version id> [--allow-dirty] [--diff-out <directory>]
       mcrs_minecraft_update recapture <fixture name | --all>

Replaces assets/minecraft from the client jar of the version, writes the client jar
descriptor, runs the data generator and replaces the reports in assets/mcrs/reports, then
dumps the block and item definitions and replaces assets/mcrs/block_definition and
assets/mcrs/item_definition. The protocol_id diff against the previous registries report is
printed and, with --diff-out, written to protocol_id.txt in that directory; the field diff
of the definitions is printed and written to definitions.txt there. Each diff is computed
before the files it describes are replaced. Two runs against one working tree at the same
time are not supported.

recapture runs the oracle task of one fixture into a temporary directory, copies the files
it wrote to their fixture directories and records the version id of assets/minecraft in
tools/captures.json. With --all it recaptures every fixture one after another, goes on after
a failure and exits non-zero if any fixture failed. It never updates the corpus.";

const CORPUS: &str = "assets/minecraft";
const DESCRIPTOR: &str = "crates/mcrs_minecraft_client_jar/src/release.json";
const FONT_HINT: &str = "crates/mcrs_minecraft_client_jar/src/font_hint.json";
const REPORTS: &str = "assets/mcrs/reports";
const REPORT_FILES: [&str; 4] = [
    "registries.json",
    "packets.json",
    "blocks.json",
    "datapack.json",
];
const DEFINITIONS: [&str; 2] = ["block_definition", "item_definition"];
const DEFINITIONS_ROOT: &str = "assets/mcrs";

struct Options {
    id: String,
    allow_dirty: bool,
    diff_out: Option<PathBuf>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("recapture") => recapture(&args[1..]),
        _ => {
            let options = parse(args.into_iter()).unwrap_or_else(|| usage());
            run(&options)
        }
    };
    if let Err(error) = result {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn recapture(args: &[String]) -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    match args {
        [all] if all == "--all" => {
            let failed: Vec<&str> = fixtures::FIXTURES
                .iter()
                .filter(|fixture| !recapture_one(&root, fixture))
                .map(|fixture| fixture.name)
                .collect();
            if failed.is_empty() {
                Ok(())
            } else {
                Err(format!("recapture failed for {}", failed.join(", ")))
            }
        }
        [name] if !name.starts_with('-') => {
            let fixture = fixtures::lookup(name)?;
            if recapture_one(&root, fixture) {
                Ok(())
            } else {
                Err(format!("recapture failed for {name}"))
            }
        }
        _ => usage(),
    }
}

fn recapture_one(root: &Path, fixture: &fixtures::Fixture) -> bool {
    match fixtures::recapture(root, fixture) {
        Ok(copied) => {
            let paths: Vec<String> = copied
                .iter()
                .map(|path| {
                    path.strip_prefix(root)
                        .unwrap_or(path)
                        .display()
                        .to_string()
                })
                .collect();
            println!("recapture {}: recaptured {}", fixture.name, paths.join(" "));
            true
        }
        Err(error) => {
            println!("recapture {}: failed {error}", fixture.name);
            false
        }
    }
}

fn parse(args: impl Iterator<Item = String>) -> Option<Options> {
    let mut args = args;
    let (mut id, mut allow_dirty, mut diff_out) = (None, false, None);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--allow-dirty" => allow_dirty = true,
            "--diff-out" if diff_out.is_none() => diff_out = Some(PathBuf::from(args.next()?)),
            _ if !arg.starts_with('-') && id.is_none() => id = Some(arg),
            _ => return None,
        }
    }
    Some(Options {
        id: id?,
        allow_dirty,
        diff_out,
    })
}

fn usage() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(2);
}

fn run(options: &Options) -> Result<(), String> {
    let id = options.id.as_str();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    release::refuse_dirty(&status(&root)?, options.allow_dirty)?;

    let manifest = get(release::MANIFEST_URL)?;
    let listing = release::resolve(&manifest, id)?;
    let package = get(&listing.url)?;
    release::check(&listing.url, &listing.sha1, None, &package)?;
    let download = release::client(&package)?;
    let jar = release::jar(&official_dir(), &listing.id, &download, get)?;

    let directory = Directory::of(&jar)?;
    let version = directory
        .read(&jar, |name| name == "version.json")?
        .pop()
        .ok_or("the jar has no version.json")?
        .1;
    release::check_version(&version, &listing.id)?;
    let mut files = Vec::new();
    for (name, bytes) in directory.read(&jar, |name| name.starts_with("data/minecraft/"))? {
        if let Some(path) = corpus::entry_path(&name)? {
            files.push((path, bytes));
        }
    }
    let descriptor = release::descriptor(&download, &jar, &listing, &package)?;
    let font_hint = release::font_hint(&jar)?;

    let report = corpus::replace(&root.join(CORPUS), &files, &version)?;
    write(&root.join(DESCRIPTOR), &descriptor)?;
    write(&root.join(FONT_HINT), &font_hint)?;

    for path in &report.deleted {
        println!("deleted {path}");
    }
    println!(
        "{CORPUS}: {} written, {} deleted, {} kept",
        report.written.len(),
        report.deleted.len(),
        report.kept.len()
    );

    update_reports(&root, options.diff_out.as_deref())?;
    update_definitions(&root, options.diff_out.as_deref())
}

fn update_reports(root: &Path, diff_out: Option<&Path>) -> Result<(), String> {
    let generated =
        std::env::temp_dir().join(format!("mcrs-update-reports-{}", std::process::id()));
    let result =
        generate(root, &generated).and_then(|()| replace_reports(root, &generated, diff_out));
    let _ = fs::remove_dir_all(&generated);
    result
}

fn generate(root: &Path, generated: &Path) -> Result<(), String> {
    fs::create_dir_all(generated).map_err(|error| format!("{}: {error}", generated.display()))?;
    let out = generated
        .to_str()
        .ok_or_else(|| format!("{}: not valid UTF-8", generated.display()))?;
    gradle::run(
        &root.join("tools/vanilla-oracle"),
        "dumpReports",
        &[("reportsOut", out)],
    )
}

fn replace_reports(root: &Path, generated: &Path, diff_out: Option<&Path>) -> Result<(), String> {
    let stored = root.join(REPORTS);
    let old = registries::read(&stored.join("registries.json"))?;
    let new = registries::read(&generated.join("reports/registries.json"))?;
    let rows = registries::diff(&old, &new);
    let text: String = rows.iter().map(|row| format!("{row}\n")).collect();

    println!("protocol_id diff: {} rows", rows.len());
    print!("{text}");
    if let Some(directory) = diff_out {
        fs::create_dir_all(directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
        write(&directory.join("protocol_id.txt"), &text)?;
    }

    for name in REPORT_FILES {
        let from = generated.join("reports").join(name);
        let bytes = fs::read(&from).map_err(|error| format!("{}: {error}", from.display()))?;
        let to = stored.join(name);
        fs::write(&to, bytes).map_err(|error| format!("{}: {error}", to.display()))?;
    }
    Ok(())
}

fn update_definitions(root: &Path, diff_out: Option<&Path>) -> Result<(), String> {
    let dumped =
        std::env::temp_dir().join(format!("mcrs-update-definitions-{}", std::process::id()));
    let result = dump(root, &dumped).and_then(|()| replace_definitions(root, &dumped, diff_out));
    let _ = fs::remove_dir_all(&dumped);
    result
}

fn dump(root: &Path, dumped: &Path) -> Result<(), String> {
    fs::create_dir_all(dumped).map_err(|error| format!("{}: {error}", dumped.display()))?;
    let out = dumped
        .to_str()
        .ok_or_else(|| format!("{}: not valid UTF-8", dumped.display()))?;
    gradle::run(
        &root.join("tools/vanilla-oracle"),
        "dumpDefinitions",
        &[("definitionsOut", out)],
    )
}

fn replace_definitions(root: &Path, dumped: &Path, diff_out: Option<&Path>) -> Result<(), String> {
    let stored = root.join(DEFINITIONS_ROOT);
    let mut text = String::new();
    for name in DEFINITIONS {
        text.push_str(&definitions::diff(&stored.join(name), &dumped.join(name))?.summary(name));
    }
    print!("{text}");
    if let Some(directory) = diff_out {
        fs::create_dir_all(directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
        write(&directory.join("definitions.txt"), &text)?;
    }
    for name in DEFINITIONS {
        let report = definitions::replace(&stored.join(name), &dumped.join(name))?;
        println!(
            "{name}: {} written, {} deleted",
            report.written.len(),
            report.deleted.len()
        );
    }
    Ok(())
}

fn status(root: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "status",
            "--porcelain",
            "--",
            "assets",
            DESCRIPTOR,
            FONT_HINT,
        ])
        .output()
        .map_err(|error| format!("git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git status failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| format!("git status: {error}"))
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
}
