use std::fs;
use std::path::Path;
use std::process::Command;

use mcrs_minecraft_client_jar::{Directory, get, official_dir};
use mcrs_minecraft_update::{corpus, release};

const USAGE: &str = "\
usage: mcrs_minecraft_update <version id> [--allow-dirty]

Replaces assets/minecraft from the client jar of the version and writes the client jar
descriptor. Two runs against one working tree at the same time are not supported.";

const CORPUS: &str = "assets/minecraft";
const DESCRIPTOR: &str = "crates/mcrs_minecraft_client_jar/src/release.json";
const FONT_HINT: &str = "crates/mcrs_minecraft_client_jar/src/font_hint.json";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (id, allow_dirty) = match args.as_slice() {
        [id] if !id.starts_with('-') => (*id, false),
        [id, "--allow-dirty"] | ["--allow-dirty", id] if !id.starts_with('-') => (*id, true),
        _ => usage(),
    };
    if let Err(error) = run(id, allow_dirty) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn usage() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(2);
}

fn run(id: &str, allow_dirty: bool) -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    release::refuse_dirty(&status(&root)?, allow_dirty)?;

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
