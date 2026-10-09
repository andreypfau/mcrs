mod checks;
mod gate;
mod git;
mod marker;
mod metadata;
mod scope;
mod size;
mod trigger;

use scope::Head;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;
use trigger::Trigger;

const DEFAULT_BASE_REF: &str = "origin/main";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let invocation = match trigger::from_args(&args) {
        Ok(invocation) => invocation,
        Err(problem) => {
            eprintln!("{problem}\n\n{}", trigger::usage());
            return ExitCode::from(2);
        }
    };
    let root = match checks::root_of(Path::new(".")) {
        Ok(root) => root,
        Err(problem) => {
            eprintln!("{problem}");
            return ExitCode::from(1);
        }
    };
    let head = match resolve_head(&root, invocation.trigger, invocation.head.as_deref()) {
        Ok(head) => head,
        Err(problem) => {
            eprintln!("{problem}");
            return ExitCode::from(1);
        }
    };
    let base_ref = std::env::var("CHECKS_BASE_REF")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_BASE_REF.to_owned());
    let ctx = checks::Context::new(invocation.trigger, root, base_ref, head);

    let (mut passed, mut failed) = (0, 0);
    for &check in trigger::checks(invocation.trigger) {
        let started = Instant::now();
        let outcome = checks::run(&ctx, check);
        let seconds = started.elapsed().as_secs_f32();
        let name = check.name();
        match outcome {
            Ok(detail) => {
                passed += 1;
                let line = if detail.is_empty() {
                    format!("{name}: ok ({seconds:.1}s)")
                } else {
                    one_line(&format!("{name}: ok ({seconds:.1}s): {detail}"))
                };
                println!("{line}");
                let notice = (!detail.is_empty())
                    .then(|| format!("::notice title={name}::{}", escape_data(&detail)));
                publish(&line, notice);
            }
            Err(message) => {
                failed += 1;
                let line = one_line(&format!("{name}: FAILED ({seconds:.1}s): {message}"));
                println!("{line}");
                publish(
                    &line,
                    Some(format!("::error title={name}::{}", escape_data(&message))),
                );
            }
        }
    }
    println!("checks: {passed} passed, {failed} failed");
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn resolve_head(root: &Path, trigger: Trigger, requested: Option<&str>) -> Result<Head, String> {
    let rev = match (trigger, requested) {
        (Trigger::Edit | Trigger::Commit, _) => return Ok(Head::Worktree),
        (Trigger::Policy, Some(rev)) => rev,
        (Trigger::Policy, None) => return Err("policy needs the head commit".to_owned()),
        _ => "HEAD",
    };
    let commit = format!("{rev}^{{commit}}");
    let sha = git::text(root, &["rev-parse", "--verify", "--quiet", &commit])
        .map_err(|_| format!("the head {rev} is not a commit here; fetch it first"))?;
    Ok(Head::Commit(sha.trim().to_owned()))
}

fn one_line(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() {
                c.escape_default().to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

fn escape_data(text: &str) -> String {
    text.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn publish(line: &str, annotation: Option<String>) {
    if std::env::var("GITHUB_ACTIONS").as_deref() != Ok("true") {
        return;
    }
    if let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") {
        let written = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&path)
            .and_then(|mut file| writeln!(file, "{line}\n"));
        if let Err(e) = written {
            eprintln!("could not append to the job summary: {e}");
        }
    }
    if let Some(annotation) = annotation {
        println!("{annotation}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotation_data_cannot_end_its_command_early_or_start_another() {
        let escaped = escape_data("100% done\r\n::error title=x::forged");
        assert!(!escaped.contains(['\r', '\n']), "{escaped}");
        assert_eq!(escaped, "100%25 done%0D%0A::error title=x::forged");
    }

    #[test]
    fn a_path_cannot_start_a_workflow_command_on_the_printed_line() {
        let printed = one_line("size: FAILED (0.1s): a\n::add-mask::x\r\u{1b}[0m b.txt");
        assert!(!printed.contains(|c: char| c.is_control()), "{printed}");
        assert_eq!(
            printed,
            "size: FAILED (0.1s): a\\n::add-mask::x\\r\\u{1b}[0m b.txt"
        );
    }
}
