use crate::metadata::Workspace;
use crate::scope::{self, Head};
use crate::source::{self, Files};
use quote::ToTokens;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Attribute, ImplItem, Item, Meta, Token};

#[derive(PartialEq, Eq)]
struct Fingerprint {
    whole: BTreeMap<String, String>,
    items: Vec<String>,
}

fn requires_test(meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) => path.is_ident("test"),
        Meta::List(list) if list.path.is_ident("all") => list
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .is_ok_and(|inner| inner.iter().any(requires_test)),
        _ => false,
    }
}

fn is_cfg_test(attr: &Attribute) -> bool {
    attr.path().is_ident("cfg")
        && attr
            .parse_args::<Meta>()
            .is_ok_and(|meta| requires_test(&meta))
}

fn item_attrs(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(i) => &i.attrs,
        Item::Enum(i) => &i.attrs,
        Item::ExternCrate(i) => &i.attrs,
        Item::Fn(i) => &i.attrs,
        Item::ForeignMod(i) => &i.attrs,
        Item::Impl(i) => &i.attrs,
        Item::Macro(i) => &i.attrs,
        Item::Mod(i) => &i.attrs,
        Item::Static(i) => &i.attrs,
        Item::Struct(i) => &i.attrs,
        Item::Trait(i) => &i.attrs,
        Item::TraitAlias(i) => &i.attrs,
        Item::Type(i) => &i.attrs,
        Item::Union(i) => &i.attrs,
        Item::Use(i) => &i.attrs,
        _ => &[],
    }
}

fn impl_item_attrs(item: &ImplItem) -> &[Attribute] {
    match item {
        ImplItem::Const(i) => &i.attrs,
        ImplItem::Fn(i) => &i.attrs,
        ImplItem::Type(i) => &i.attrs,
        ImplItem::Macro(i) => &i.attrs,
        _ => &[],
    }
}

#[derive(Default)]
struct Collector {
    items: Vec<String>,
}

impl<'ast> Visit<'ast> for Collector {
    fn visit_item(&mut self, item: &'ast Item) {
        let attrs = item_attrs(item);
        let test_fn = matches!(item, Item::Fn(_)) && source::is_test(attrs);
        if test_fn || attrs.iter().any(is_cfg_test) {
            self.items.push(item.to_token_stream().to_string());
            return;
        }
        visit::visit_item(self, item);
    }

    fn visit_impl_item(&mut self, item: &'ast ImplItem) {
        let attrs = impl_item_attrs(item);
        let test_fn = matches!(item, ImplItem::Fn(_)) && source::is_test(attrs);
        if test_fn || attrs.iter().any(is_cfg_test) {
            self.items.push(item.to_token_stream().to_string());
            return;
        }
        visit::visit_impl_item(self, item);
    }
}

fn tokens_of(path: &str, text: &str) -> Result<String, String> {
    Ok(source::parse(path, text)?.to_token_stream().to_string())
}

fn test_modules(root: &str, files: &Files) -> Result<BTreeSet<String>, String> {
    let mut gated: BTreeSet<Vec<String>> = BTreeSet::new();
    let mut placed: BTreeMap<String, Vec<String>> = BTreeMap::new();
    source::walk_modules(root, files, &mut |view| {
        placed
            .entry(view.file.to_owned())
            .or_insert_with(|| view.module.to_vec());
        for item in view.items {
            if let Item::Mod(declared) = item
                && declared.attrs.iter().any(is_cfg_test)
            {
                let mut path = view.module.to_vec();
                path.push(declared.ident.to_string());
                gated.insert(path);
            }
        }
    })?;
    Ok(placed
        .into_iter()
        .filter(|(_, module)| (1..=module.len()).any(|n| gated.contains(&module[..n])))
        .map(|(file, _)| file)
        .collect())
}

fn relative<'a>(dir: &str, path: &'a str) -> Option<&'a str> {
    if dir.is_empty() {
        Some(path)
    } else {
        path.strip_prefix(dir)?.strip_prefix('/')
    }
}

fn is_surface(rest: &str, path: &str) -> bool {
    rest.starts_with("tests/")
        || rest.starts_with("benches/")
        || (rest.starts_with("src/") && path.ends_with(".rs"))
}

fn fingerprint(
    dir: &str,
    roots: &[String],
    files: &Files,
    changed: &[String],
) -> Result<Fingerprint, String> {
    let mut test_files = BTreeSet::new();
    for root in roots.iter().filter(|root| files.contains_key(*root)) {
        test_files.extend(test_modules(root, files)?);
    }
    let mut found = Fingerprint {
        whole: BTreeMap::new(),
        items: Vec::new(),
    };
    for path in changed {
        let Some(text) = files.get(path) else {
            continue;
        };
        let rest = relative(dir, path).unwrap_or_default();
        let in_rust = path.ends_with(".rs");
        if rest.starts_with("tests/") || rest.starts_with("benches/") {
            let canonical = if in_rust {
                tokens_of(path, text).unwrap_or_else(|_| text.clone())
            } else {
                text.clone()
            };
            found.whole.insert(path.clone(), canonical);
        } else if rest.starts_with("src/") && in_rust {
            if test_files.contains(path) {
                found.whole.insert(path.clone(), tokens_of(path, text)?);
            } else {
                let mut collector = Collector::default();
                collector.visit_file(&source::parse(path, text)?);
                found.items.extend(collector.items);
            }
        }
    }
    found.items.sort();
    Ok(found)
}

fn read_side(repo: &Path, rev: &str, wanted: &BTreeSet<&String>) -> Result<Files, String> {
    let specs: Vec<String> = wanted.iter().map(|path| format!("{rev}:{path}")).collect();
    let blobs = scope::blobs(repo, &specs)?;
    Ok(wanted
        .iter()
        .zip(blobs)
        .filter_map(|(path, blob)| {
            Some((
                (*path).clone(),
                String::from_utf8_lossy(&blob?).into_owned(),
            ))
        })
        .collect())
}

pub(crate) fn changed_crates(
    repo: &Path,
    workspace: &Workspace,
    base: &str,
    head: &str,
) -> Result<BTreeSet<String>, String> {
    let paths = scope::diff_names(repo, base, &Head::Commit(head.to_owned()))?;
    let mut candidates: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for path in &paths {
        let Some(owner) = workspace.owner(path) else {
            continue;
        };
        if relative(&owner.dir, path).is_some_and(|rest| is_surface(rest, path)) {
            candidates
                .entry(owner.name.as_str())
                .or_default()
                .push(path.clone());
        }
    }
    if candidates.is_empty() {
        return Ok(BTreeSet::new());
    }
    let at_base = scope::tracked(repo, &Head::Commit(base.to_owned()))?;
    let at_head = scope::tracked(repo, &Head::Commit(head.to_owned()))?;
    let mut found = BTreeSet::new();
    for (name, touched) in candidates {
        let package = workspace
            .packages
            .iter()
            .find(|package| package.name == name)
            .ok_or_else(|| format!("{name} is not a package of this workspace"))?;
        let sources = |tracked: &'_ [String]| -> BTreeSet<String> {
            tracked
                .iter()
                .filter(|path| path.ends_with(".rs"))
                .filter(|path| {
                    relative(&package.dir, path).is_some_and(|rest| rest.starts_with("src/"))
                })
                .filter(|path| {
                    workspace
                        .owner(path)
                        .is_some_and(|owner| owner.name == name)
                })
                .cloned()
                .collect()
        };
        let (base_sources, head_sources) = (sources(&at_base), sources(&at_head));
        let wanted = |sources: &'_ BTreeSet<String>| -> BTreeSet<String> {
            sources.iter().chain(touched.iter()).cloned().collect()
        };
        let (base_wanted, head_wanted) = (wanted(&base_sources), wanted(&head_sources));
        let base_files = read_side(repo, base, &base_wanted.iter().collect())?;
        let head_files = read_side(repo, head, &head_wanted.iter().collect())?;
        let roots: Vec<String> = package
            .targets
            .iter()
            .map(|target| target.path.clone())
            .filter(|path| {
                relative(&package.dir, path).is_some_and(|rest| rest.starts_with("src/"))
            })
            .collect();
        if changed(&package.dir, &roots, &base_files, &head_files, &touched)? {
            found.insert(name.to_owned());
        }
    }
    Ok(found)
}

// chisle: the module walk starts only at the roots it is given, so a source file no root
// reaches counts as plain source. Lifted when a crate keeps tests in an unreachable file.
pub(crate) fn changed(
    dir: &str,
    roots: &[String],
    base: &Files,
    head: &Files,
    changed: &[String],
) -> Result<bool, String> {
    Ok(fingerprint(dir, roots, base, changed)? != fingerprint(dir, roots, head, changed)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIR: &str = "crates/x";
    const LIB: &str = "crates/x/src/lib.rs";

    fn lib(body: &str, tests: &str) -> String {
        format!(
            "pub fn double(x: i32) -> i32 {{\n    x * 2\n}}\n\n{body}#[cfg(test)]\nmod tests {{\n    use super::*;\n\n    fn helper() -> i32 {{\n        2\n    }}\n\n    #[test]\n    fn doubles() {{\n{tests}    }}\n}}\n"
        )
    }

    const ASSERT: &str = "        assert_eq!(double(2), 4);\n";

    struct Row {
        name: &'static str,
        base: Vec<(&'static str, String)>,
        head: Vec<(&'static str, String)>,
        changed: bool,
    }

    fn only_lib(name: &'static str, base: String, head: String, changed: bool) -> Row {
        Row {
            name,
            base: vec![(LIB, base)],
            head: vec![(LIB, head)],
            changed,
        }
    }

    fn files(rows: &[(&'static str, String)]) -> Files {
        rows.iter()
            .map(|(path, text)| ((*path).to_owned(), text.clone()))
            .collect()
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "a table of formatting-only edits that leave the test surface unchanged"
    )]
    fn formatting_alone_leaves_the_test_surface_unchanged() {
        let base = || lib("", ASSERT);
        let rows = vec![
            only_lib(
                "a reformatted test",
                base(),
                lib(
                    "",
                    "        assert_eq!(\n            double(2),\n            4\n        );\n",
                ),
                false,
            ),
            only_lib(
                "a comment inside a test",
                base(),
                lib("", &format!("        // doubling twice\n{ASSERT}")),
                false,
            ),
            only_lib(
                "an added assertion",
                base(),
                lib("", &format!("{ASSERT}        assert_eq!(double(0), 0);\n")),
                true,
            ),
            only_lib(
                "a changed helper under cfg(test)",
                base(),
                base().replace(
                    "        2\n    }\n\n    #[test]",
                    "        3\n    }\n\n    #[test]",
                ),
                true,
            ),
            only_lib(
                "a changed function outside the tests",
                base(),
                base().replace("x * 2", "x + x"),
                false,
            ),
            only_lib(
                "a changed item under cfg(not(test))",
                format!(
                    "{}#[cfg(not(test))]\nfn shim() -> u8 {{\n    1\n}}\n",
                    base()
                ),
                format!(
                    "{}#[cfg(not(test))]\nfn shim() -> u8 {{\n    2\n}}\n",
                    base()
                ),
                false,
            ),
            only_lib(
                "a test added to a source file",
                base(),
                format!("{}#[test]\nfn another() {{}}\n", base()),
                true,
            ),
            Row {
                name: "a new file under tests",
                base: vec![(LIB, base())],
                head: vec![
                    (LIB, base()),
                    (
                        "crates/x/tests/it/main.rs",
                        "#[test]\nfn it() {}\n".to_owned(),
                    ),
                ],
                changed: true,
            },
            Row {
                name: "a deleted bench file",
                base: vec![
                    (LIB, base()),
                    ("crates/x/benches/speed.rs", "fn main() {}\n".to_owned()),
                ],
                head: vec![(LIB, base())],
                changed: true,
            },
            Row {
                name: "a reformatted file under tests",
                base: vec![(
                    "crates/x/tests/it/main.rs",
                    "#[test]\nfn it() { assert!(true); }\n".to_owned(),
                )],
                head: vec![(
                    "crates/x/tests/it/main.rs",
                    "#[test]\nfn it() {\n    assert!(true);\n}\n".to_owned(),
                )],
                changed: false,
            },
            Row {
                name: "a changed Rust fixture under tests that does not parse",
                base: vec![("crates/x/tests/ui/broken.rs", "fn broken( {\n".to_owned())],
                head: vec![("crates/x/tests/ui/broken.rs", "fn broken( {{\n".to_owned())],
                changed: true,
            },
            Row {
                name: "a changed fixture under tests",
                base: vec![("crates/x/tests/data/a.json", "{\"a\": 1}\n".to_owned())],
                head: vec![("crates/x/tests/data/a.json", "{\"a\": 2}\n".to_owned())],
                changed: true,
            },
            Row {
                name: "a changed file declared as a test module out of line",
                base: vec![
                    (LIB, "#[cfg(test)]\nmod tests;\npub fn a() {}\n".to_owned()),
                    (
                        "crates/x/src/tests.rs",
                        "fn helper() -> u8 {\n    1\n}\n".to_owned(),
                    ),
                ],
                head: vec![
                    (LIB, "#[cfg(test)]\nmod tests;\npub fn a() {}\n".to_owned()),
                    (
                        "crates/x/src/tests.rs",
                        "fn helper() -> u8 {\n    2\n}\n".to_owned(),
                    ),
                ],
                changed: true,
            },
            Row {
                name: "a changed file declared as a plain module out of line",
                base: vec![
                    (LIB, "mod inner;\npub fn a() {}\n".to_owned()),
                    (
                        "crates/x/src/inner.rs",
                        "pub fn helper() -> u8 {\n    1\n}\n".to_owned(),
                    ),
                ],
                head: vec![
                    (LIB, "mod inner;\npub fn a() {}\n".to_owned()),
                    (
                        "crates/x/src/inner.rs",
                        "pub fn helper() -> u8 {\n    2\n}\n".to_owned(),
                    ),
                ],
                changed: false,
            },
        ];
        let mut wrong = Vec::new();
        for Row {
            name,
            base,
            head,
            changed: expected,
        } in rows
        {
            let (base, head) = (files(&base), files(&head));
            let mut touched: Vec<String> = base
                .keys()
                .chain(head.keys())
                .filter(|path| base.get(*path) != head.get(*path))
                .cloned()
                .collect();
            touched.sort();
            touched.dedup();
            let found = changed(DIR, &[LIB.to_owned()], &base, &head, &touched);
            if found != Ok(expected) {
                wrong.push(format!("{name}: expected {expected}, found {found:?}"));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }
}
