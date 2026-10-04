use std::path::Path;
use std::process::Command;

pub fn utf8(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("{}: not valid UTF-8", path.display()))
}

pub fn arguments(task: &str, properties: &[(&str, impl AsRef<str>)]) -> Vec<String> {
    let mut arguments = vec![
        task.to_owned(),
        "--console=plain".into(),
        "--no-daemon".into(),
    ];
    arguments.extend(
        properties
            .iter()
            .map(|(name, value)| format!("-P{name}={}", value.as_ref())),
    );
    arguments
}

pub fn run(
    project: &Path,
    task: &str,
    properties: &[(&str, impl AsRef<str>)],
) -> Result<(), String> {
    let status = Command::new(project.join("gradlew"))
        .args(arguments(task, properties))
        .current_dir(project)
        .status()
        .map_err(|error| format!("gradle task {task}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("gradle task {task} failed: {status}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_property_value_with_spaces_quotes_and_a_semicolon_stays_one_argument() {
        let value = "/tmp/a b\"c';d";
        assert_eq!(
            arguments("dumpReports", &[("reportsOut", value), ("other", "1")]),
            [
                "dumpReports",
                "--console=plain",
                "--no-daemon",
                "-PreportsOut=/tmp/a b\"c';d",
                "-Pother=1",
            ]
        );
    }
}
