use fancy_regex::Regex;
use serde::Deserialize;

use crate::metadata::Lenient;

#[derive(Debug)]
pub(crate) struct Filter {
    patterns: Vec<Pattern>,
}

#[derive(Debug)]
struct Pattern {
    namespace: Option<Regex>,
    path: Option<Regex>,
}

#[derive(Deserialize)]
pub(crate) struct FilterSection {
    block: Vec<PatternSource>,
}

#[derive(Deserialize)]
struct PatternSource {
    namespace: Option<String>,
    path: Option<String>,
}

fn compile(pattern: Option<&str>) -> Result<Option<Regex>, String> {
    pattern
        .map(|source| Regex::new(source).map_err(|e| format!("pattern {source:?}: {e}")))
        .transpose()
}

fn matches(pattern: Option<&Regex>, text: &str) -> Result<bool, String> {
    match pattern {
        None => Ok(true),
        Some(regex) => regex
            .is_match(text)
            .map_err(|e| format!("pattern {:?} on {text:?}: {e}", regex.as_str())),
    }
}

fn any_matches<'p>(
    mut patterns: impl Iterator<Item = Option<&'p Regex>>,
    text: &str,
) -> Result<bool, String> {
    patterns.try_fold(false, |found, pattern| Ok(found || matches(pattern, text)?))
}

impl Filter {
    pub(crate) fn compile(section: &Lenient<FilterSection>) -> Result<Self, String> {
        let Lenient::Valid(section) = section else {
            return Err("the filter section is malformed".to_owned());
        };
        let patterns = section
            .block
            .iter()
            .map(|source| {
                Ok(Pattern {
                    namespace: compile(source.namespace.as_deref())?,
                    path: compile(source.path.as_deref())?,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Self { patterns })
    }

    pub(crate) fn hides(&self, namespace: &str, path: &str) -> Result<bool, String> {
        let namespaces = self.patterns.iter().map(|p| p.namespace.as_ref());
        let paths = self.patterns.iter().map(|p| p.path.as_ref());
        Ok(any_matches(namespaces, namespace)? && any_matches(paths, path)?)
    }
}
