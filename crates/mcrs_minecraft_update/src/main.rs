#![expect(
    clippy::disallowed_types,
    reason = "the update tool parses the game's JSON reports as they come"
)]
#![cfg_attr(
    test,
    expect(clippy::disallowed_macros, reason = "test input is written as JSON")
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use mcrs_minecraft_client_jar::{RELEASE, get, official_dir};

mod blocks;
mod corpus;
mod definitions;
mod fixtures;
mod gradle;
mod keys;
mod names;
mod owners;
mod registries;
mod release;
#[cfg(test)]
mod testing;

const USAGE: &str = "\
usage: mcrs_minecraft_update <version id> [--allow-dirty] [--diff-out <directory>]
       mcrs_minecraft_update recapture <fixture name | --all>
       mcrs_minecraft_update names
       mcrs_minecraft_update keys

Replaces assets/minecraft from the client jar of the version, writes the client jar
descriptor, runs the data generator and dumps the block, item and game rule definitions, stops
if the block definitions disagree with the blocks report of the generator, then replaces the
reports in assets/mcrs/reports and assets/mcrs/block_definition, assets/mcrs/item_definition
and assets/mcrs/registry_definition. The
blocks report is checked and not stored. The protocol_id diff against the previous registries report is
printed and, with --diff-out, written to protocol_id.txt in that directory; the field diff
of the definitions is printed and written to definitions.txt there. The names report is
written after the reports, and its diff is printed and written to names.txt there. The sources
of the registry keys are written after the names report. Each diff
is computed before the files it describes are replaced. Two runs against one working tree at
the same time are not supported.

recapture runs the oracle task of one fixture into a temporary directory, copies the files
it wrote to their fixture directories and records the version id of assets/minecraft in
tools/captures.json. With --all it recaptures every fixture one after another, goes on after
a failure and exits non-zero if any fixture failed. It never updates the corpus.

names writes assets/mcrs/reports/names.json from the client jar the descriptor names: the
entry names of every registry of datapack.json that has elements and the tag names of every
registry, in sorted order. It prints the entries and tags added and removed against the stored
file, then writes the registry key sources from the stored reports. It touches nothing else.

keys writes the registry key sources from the stored reports alone: the src/keys module of
every crate that owns a registry type, crates/mcrs_minecraft_keys for the registries no crate
owns, and crates/mcrs_minecraft_registry_catalog. A source file there the generator no longer
produces is deleted.";

const CORPUS: &str = "assets/minecraft";
const DESCRIPTOR: &str = "crates/mcrs_minecraft_client_jar/src/release.json";
const FONT_HINT: &str = "crates/mcrs_minecraft_client_jar/src/font_hint.json";
const REPORTS: &str = "assets/mcrs/reports";
const REPORT_FILES: [&str; 3] = ["registries.json", "packets.json", "datapack.json"];
const NAMES_FILE: &str = "names.json";
const DEFINITIONS: [&str; 3] = ["block_definition", "item_definition", "registry_definition"];
const DEFINITIONS_ROOT: &str = "assets/mcrs";

struct Options {
    id: String,
    allow_dirty: bool,
    diff_out: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
enum Recapture {
    All,
    One(String),
}

enum Invocation {
    Recapture(Recapture),
    Names,
    Keys,
    Update(Options),
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match command(args).unwrap_or_else(|| usage()) {
        Invocation::Recapture(args) => recapture(&args),
        Invocation::Names => names_report(),
        Invocation::Keys => write_keys(&workspace_root(), None),
        Invocation::Update(options) => run(&options),
    };
    if let Err(error) = result {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn command(args: Vec<String>) -> Option<Invocation> {
    match args.first().map(String::as_str) {
        Some("recapture") => recapture_target(&args[1..]).map(Invocation::Recapture),
        Some("names") => (args.len() == 1).then_some(Invocation::Names),
        Some("keys") => (args.len() == 1).then_some(Invocation::Keys),
        _ => parse(args.into_iter()).map(Invocation::Update),
    }
}

fn recapture_target(args: &[String]) -> Option<Recapture> {
    match args {
        [all] if all == "--all" => Some(Recapture::All),
        [name] if !name.starts_with('-') => Some(Recapture::One(name.clone())),
        _ => None,
    }
}

fn recapture(target: &Recapture) -> Result<(), String> {
    recapture_in(&workspace_root(), target)
}

fn recapture_in(root: &Path, target: &Recapture) -> Result<(), String> {
    match target {
        Recapture::All => {
            let failed: Vec<&str> = mcrs_minecraft_fixtures::table()
                .iter()
                .filter(|fixture| !recapture_one(root, fixture))
                .map(|fixture| fixture.name.as_str())
                .collect();
            if failed.is_empty() {
                Ok(())
            } else {
                Err(format!("recapture failed for {}", failed.join(", ")))
            }
        }
        Recapture::One(name) => {
            let fixture =
                mcrs_minecraft_fixtures::entry(name).map_err(|error| error.to_string())?;
            if recapture_one(root, fixture) {
                Ok(())
            } else {
                Err(format!("recapture failed for {name}"))
            }
        }
    }
}

fn names_report() -> Result<(), String> {
    let root = workspace_root();
    let descriptor = &*RELEASE;
    let download = release::Download {
        url: descriptor.jar.url.to_owned(),
        sha1: descriptor.jar.sha1.to_owned(),
        size: descriptor.jar.size,
    };
    let jar = release::jar(&official_dir(), descriptor.id, &download, get)?;
    write_names(&root, &jar, None)
}

fn recapture_one(root: &Path, fixture: &mcrs_minecraft_fixtures::Entry) -> bool {
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
    let root = workspace_root();
    release::refuse_dirty(&status(&root)?, options.allow_dirty)?;

    let manifest = get(release::MANIFEST_URL)?;
    let listing = release::resolve(&manifest, id)?;
    let package = get(&listing.url)?;
    release::check(&listing.url, &listing.sha1, None, &package)?;
    let download = release::client(&package)?;
    let jar = release::jar(&official_dir(), &listing.id, &download, get)?;

    let (files, version) = corpus::from_jar(&jar)?;
    let stored = root.join(REPORTS);
    let names = corpus::stored_names(&stored.join("registries.json"), &stored.join(NAMES_FILE))?;
    let (files, built_in, diverged) = corpus::without_built_in(files, &names)?;
    release::check_version(&version, &listing.id)?;
    let descriptor = release::descriptor(&download, &jar, &listing, &package)?;
    let font_hint = release::font_hint(&jar)?;

    let report = corpus::replace(&root.join(CORPUS), &files, &version)?;
    write(&root.join(DESCRIPTOR), &descriptor)?;
    write(&root.join(FONT_HINT), &font_hint)?;

    for path in &report.deleted {
        println!("deleted {path}");
    }
    for path in &diverged {
        println!("differs from the built-in, kept as a file: {path}");
    }
    println!("{CORPUS}: {built_in} entries are built by code and ship no file");
    println!(
        "{CORPUS}: {} written, {} deleted",
        report.written.len(),
        report.deleted.len()
    );

    let diff_out = options.diff_out.as_deref();
    with_gradle_output(&root, "dumpReports", "reportsOut", |generated| {
        with_gradle_output(&root, "dumpDefinitions", "definitionsOut", |dumped| {
            blocks::check(
                &generated.join("reports/blocks.json"),
                &dumped.join("block_definition"),
            )?;
            replace_reports(&root, generated, diff_out)?;
            replace_definitions(&root, dumped, diff_out)
        })
    })?;
    write_names(&root, &jar, diff_out)
}

fn write_names(root: &Path, jar: &[u8], diff_out: Option<&Path>) -> Result<(), String> {
    let stored = root.join(REPORTS);
    let new = names::from_jar(jar, &datapack_report(&stored)?)?;
    let old = names::read(&stored.join(NAMES_FILE))?;
    let rows = names::diff(&old, &new);
    let text: String = rows.iter().map(|row| format!("{row}\n")).collect();

    println!("names diff: {} rows", rows.len());
    print!("{text}");
    write_diff(diff_out, "names.txt", &text)?;
    write(&stored.join(NAMES_FILE), &release::pretty(&new)?)?;

    write_keys(root, Some(&new))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn datapack_report(stored: &Path) -> Result<names::Datapack, String> {
    let path = stored.join("datapack.json");
    let text = fs::read_to_string(&path).map_err(|error| corpus::io(&path, error))?;
    names::Datapack::parse(&text).map_err(|error| format!("{}: {error}", path.display()))
}

fn keys_files(root: &Path, names: Option<&names::Names>) -> Result<keys::Files, String> {
    let stored = root.join(REPORTS);
    let read;
    let names = match names {
        Some(names) => names,
        None => {
            read = names::read(&stored.join(NAMES_FILE))?;
            &read
        }
    };
    let registries = registries::read(&stored.join("registries.json"))?;
    let above = keys::above_catalog(root, owners::OWNERS)?;
    keys::generate(
        &registries,
        &datapack_report(&stored)?,
        names,
        owners::OWNERS,
        &above,
    )
}

fn write_keys(root: &Path, names: Option<&names::Names>) -> Result<(), String> {
    keys::write(root, &keys_files(root, names)?)
}

fn with_gradle_output(
    root: &Path,
    task: &str,
    property: &str,
    then: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    fixtures::with_scratch(task, |out| {
        gradle::run(
            &root.join(fixtures::ORACLE),
            task,
            &[(property, gradle::utf8(out)?)],
        )?;
        then(out)
    })
}

fn write_diff(diff_out: Option<&Path>, name: &str, text: &str) -> Result<(), String> {
    let Some(directory) = diff_out else {
        return Ok(());
    };
    fs::create_dir_all(directory).map_err(|error| corpus::io(directory, error))?;
    write(&directory.join(name), text)
}

fn replace_reports(root: &Path, generated: &Path, diff_out: Option<&Path>) -> Result<(), String> {
    let stored = root.join(REPORTS);
    let old = registries::read(&stored.join("registries.json"))?;
    let new = registries::read(&generated.join("reports/registries.json"))?;
    let rows = registries::diff(&old, &new);
    let text: String = rows.iter().map(|row| format!("{row}\n")).collect();

    println!("protocol_id diff: {} rows", rows.len());
    print!("{text}");
    write_diff(diff_out, "protocol_id.txt", &text)?;

    for name in REPORT_FILES {
        let (from, to) = (generated.join("reports").join(name), stored.join(name));
        fs::copy(&from, &to)
            .map_err(|error| format!("{} to {}: {error}", from.display(), to.display()))?;
    }
    Ok(())
}

fn replace_definitions(root: &Path, dumped: &Path, diff_out: Option<&Path>) -> Result<(), String> {
    let stored = root.join(DEFINITIONS_ROOT);
    let mut text = String::new();
    for name in DEFINITIONS {
        text.push_str(&definitions::diff(&stored.join(name), &dumped.join(name))?.summary(name));
    }
    print!("{text}");
    write_diff(diff_out, "definitions.txt", &text)?;
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

#[cfg(test)]
mod tests {
    use super::Invocation;
    use super::{Recapture, command, recapture, recapture_in, recapture_target};
    use crate::testing::scratch;

    #[test]
    fn a_subcommand_takes_no_stray_arguments() {
        for (args, accepted) in [
            (&["keys"][..], true),
            (&["keys", "--allow-dirty"], false),
            (&["keys", "1.0"], false),
            (&["names"], true),
            (&["names", "--allow-dirty"], false),
        ] {
            let parsed = command(args.iter().map(|arg| arg.to_string()).collect());
            assert_eq!(
                matches!(parsed, Some(Invocation::Keys | Invocation::Names)),
                accepted,
                "{args:?}"
            );
            assert!(!matches!(parsed, Some(Invocation::Update(_))), "{args:?}");
        }
    }

    #[test]
    fn a_recapture_names_one_fixture_or_all_and_nothing_else() {
        for (args, expected) in [
            (&["--all"][..], Some(Recapture::All)),
            (&["beard"], Some(Recapture::One("beard".to_owned()))),
            (&["-x"], None),
            (&["--other"], None),
            (&[], None),
            (&["beard", "surface"], None),
            (&["--all", "beard"], None),
        ] {
            let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
            let target = recapture_target(&args);
            assert_eq!(target, expected, "{args:?}");
            let invoked = command(
                ["recapture".to_owned()]
                    .into_iter()
                    .chain(args.clone())
                    .collect(),
            );
            let parsed = match invoked {
                Some(Invocation::Recapture(parsed)) => Some(parsed),
                _ => None,
            };
            assert_eq!(parsed, target, "{args:?}");
        }
    }

    #[test]
    fn an_unknown_fixture_name_is_refused_before_any_task_runs() {
        let error = recapture(&Recapture::One("no_such_fixture".to_owned())).unwrap_err();
        assert!(error.starts_with("unknown fixture no_such_fixture; known fixtures: density, "));
    }

    #[cfg(unix)]
    fn oracle_root(script: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let root = scratch("recapture");
        let project = root.join(crate::fixtures::ORACLE);
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(root.join("assets/minecraft")).unwrap();
        std::fs::write(
            root.join("assets/minecraft/version.json"),
            mcrs_minecraft_core::VERSION_JSON,
        )
        .unwrap();
        for output in mcrs_minecraft_fixtures::table()
            .iter()
            .flat_map(|fixture| &fixture.outputs)
        {
            std::fs::create_dir_all(root.join(&output.to)).unwrap();
        }
        let gradlew = project.join("gradlew");
        std::fs::write(&gradlew, script).unwrap();
        std::fs::set_permissions(&gradlew, std::fs::Permissions::from_mode(0o755)).unwrap();
        root
    }

    #[cfg(unix)]
    const ONLY_BEARD_SUCCEEDS: &str = r#"#!/bin/sh
echo "$1" >> "$(dirname "$0")/calls.log"
for argument in "$@"; do
    case "$argument" in
        -PoracleOut=*) out="${argument#-PoracleOut=}" ;;
    esac
done
[ "$1" = dumpBeard ] || exit 1
printf MC > "$out/beard.bin"
"#;

    #[cfg(unix)]
    fn calls(root: &std::path::Path) -> Vec<String> {
        std::fs::read_to_string(root.join(crate::fixtures::ORACLE).join("calls.log"))
            .map(|log| log.lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }

    #[cfg(unix)]
    #[test]
    fn recapturing_one_fixture_runs_its_task_stores_its_files_and_records_the_corpus_id() {
        let root = oracle_root(ONLY_BEARD_SUCCEEDS);
        recapture_in(&root, &Recapture::One("beard".to_owned())).unwrap();
        assert_eq!(calls(&root), ["dumpBeard"]);
        assert_eq!(
            std::fs::read(
                root.join("crates/mcrs_minecraft_worldgen/tests/fixtures/vanilla/beard.bin")
            )
            .unwrap(),
            b"MC"
        );
        let manifest =
            std::fs::read_to_string(root.join(mcrs_minecraft_fixtures::MANIFEST)).unwrap();
        assert_eq!(
            manifest,
            format!(
                "{{\n  \"beard\": \"{}\"\n}}\n",
                mcrs_minecraft_core::VERSION.id
            )
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_failing_task_fails_the_recapture_of_that_fixture() {
        let root = oracle_root(ONLY_BEARD_SUCCEEDS);
        let error = recapture_in(&root, &Recapture::One("surface".to_owned())).unwrap_err();
        assert_eq!(error, "recapture failed for surface");
        assert_eq!(calls(&root), ["dumpSurface"]);
        assert!(!root.join(mcrs_minecraft_fixtures::MANIFEST).exists());
    }

    #[cfg(unix)]
    #[test]
    fn an_unknown_fixture_runs_no_task_and_lists_the_known_names() {
        let root = oracle_root(ONLY_BEARD_SUCCEEDS);
        let error = recapture_in(&root, &Recapture::One("no_such_fixture".to_owned())).unwrap_err();
        assert!(
            error
                .starts_with("unknown fixture no_such_fixture; known fixtures: density, surface, ")
        );
        assert!(error.ends_with(", registry_values"), "{error}");
        assert!(calls(&root).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn recapturing_all_goes_on_after_a_failure_and_names_every_fixture_that_failed() {
        let root = oracle_root(ONLY_BEARD_SUCCEEDS);
        let error = recapture_in(&root, &Recapture::All).unwrap_err();
        let table = mcrs_minecraft_fixtures::table();
        let tasks: Vec<&str> = table.iter().map(|fixture| fixture.task.as_str()).collect();
        assert_eq!(calls(&root), tasks);
        let failed: Vec<&str> = table
            .iter()
            .filter(|fixture| fixture.name != "beard")
            .map(|fixture| fixture.name.as_str())
            .collect();
        assert_eq!(error, format!("recapture failed for {}", failed.join(", ")));
        assert!(
            root.join("crates/mcrs_minecraft_worldgen/tests/fixtures/vanilla/beard.bin")
                .is_file()
        );
    }

    #[cfg(unix)]
    fn every_task_succeeds_script() -> String {
        let mut script = String::from(
            "#!/bin/sh\necho \"$1\" >> \"$(dirname \"$0\")/calls.log\"\nkey=\"$1\"\nfor argument in \"$@\"; do\n    case \"$argument\" in\n        -PoracleOut=*) out=\"${argument#-PoracleOut=}\" ;;\n        -PgoldenOut=*) out=\"${argument#-PgoldenOut=}\" ;;\n        -Pgolden=*) key=\"${argument#-Pgolden=}\" ;;\n    esac\ndone\ncase \"$key\" in\n",
        );
        for fixture in mcrs_minecraft_fixtures::table() {
            let key = if fixture.is_golden() {
                &fixture.name
            } else {
                &fixture.task
            };
            script.push_str(&format!("{key}) "));
            for output in &fixture.outputs {
                let file = match &output.files {
                    mcrs_minecraft_fixtures::Files::Named(name) => name.clone(),
                    mcrs_minecraft_fixtures::Files::Matching(pattern) => pattern.replace('*', "0"),
                };
                script.push_str(&format!("printf x > \"$out/{file}\"; "));
            }
            script.push_str(";;\n");
        }
        script.push_str("*) exit 1 ;;\nesac\n");
        script
    }

    #[cfg(unix)]
    #[test]
    fn recapturing_all_succeeds_when_every_task_does_and_records_every_fixture() {
        let root = oracle_root(&every_task_succeeds_script());
        recapture_in(&root, &Recapture::All).unwrap();
        let table = mcrs_minecraft_fixtures::table();
        assert_eq!(calls(&root).len(), table.len());
        let manifest =
            std::fs::read_to_string(root.join(mcrs_minecraft_fixtures::MANIFEST)).unwrap();
        let recorded: std::collections::BTreeMap<String, String> =
            serde_json::from_str(&manifest).unwrap();
        let names: std::collections::BTreeSet<&str> =
            table.iter().map(|fixture| fixture.name.as_str()).collect();
        assert_eq!(
            recorded
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>(),
            names
        );
        assert!(
            recorded
                .values()
                .all(|id| *id == mcrs_minecraft_core::VERSION.id)
        );
    }

    #[test]
    fn the_descriptor_names_the_corpus_version() {
        assert_eq!(
            mcrs_minecraft_client_jar::RELEASE.id,
            mcrs_minecraft_core::VERSION.id
        );
    }
}
